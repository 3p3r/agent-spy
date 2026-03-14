#[cfg(target_os = "macos")]
#[cfg(target_os = "macos")]
mod imp {
    use anyhow::{Result, anyhow, bail};
    use std::ffi::{c_char, c_void};

    use crate::platform::{PermissionStatus, Platform, WindowInfo};

    // Mirror C structs; CGFloat is f64 on all 64-bit Apple targets.
    #[repr(C)]
    struct CgPoint {
        x: f64,
        y: f64,
    }

    #[repr(C)]
    struct CgSize {
        width: f64,
        height: f64,
    }

    // kAXValueCGPointType = 1, kAXValueCGSizeType = 2 (stable across macOS versions).
    const KAX_VALUE_CG_POINT_TYPE: i32 = 1;
    const KAX_VALUE_CG_SIZE_TYPE: i32 = 2;
    // CFStringEncoding: kCFStringEncodingUTF8 = 0x08000100
    const KCF_STRING_ENCODING_UTF8: u32 = 0x0800_0100;

    #[link(name = "CoreGraphics", kind = "framework")]
    unsafe extern "C" {
        fn CGEventCreate(source: *const c_void) -> *const c_void;
        fn CGEventGetLocation(event: *const c_void) -> CgPoint;
    }

    #[link(name = "CoreFoundation", kind = "framework")]
    unsafe extern "C" {
        fn CFRelease(cf: *const c_void);
        fn CFStringCreateWithCString(
            alloc: *const c_void,
            cstr: *const c_char,
            encoding: u32,
        ) -> *const c_void;
        static kCFBooleanTrue: *const c_void;
    }

    #[link(name = "ApplicationServices", kind = "framework")]
    unsafe extern "C" {
        fn AXIsProcessTrusted() -> bool;
        fn AXUIElementCreateApplication(pid: i32) -> *const c_void;
        fn AXUIElementSetAttributeValue(
            element: *const c_void,
            attribute: *const c_void,
            value: *const c_void,
        ) -> i32;
        fn AXValueCreate(type_: i32, value_ref: *const c_void) -> *const c_void;
        // Private but stable SPI since macOS 10.9.
        fn _AXUIElementCreateWithWindowID(
            application: *const c_void,
            wid: u32,
            out: *mut *const c_void,
        ) -> i32;
    }

    /// Create a CFStringRef from a null-terminated byte slice.
    fn cf_string(s: &[u8]) -> *const c_void {
        debug_assert_eq!(*s.last().unwrap(), 0, "must be null-terminated");
        unsafe {
            CFStringCreateWithCString(
                std::ptr::null(),
                s.as_ptr() as *const c_char,
                KCF_STRING_ENCODING_UTF8,
            )
        }
    }

    pub struct MacPlatform;

    impl MacPlatform {
        pub fn new() -> Self {
            Self
        }

        fn find_window(&self, id: u64) -> Result<WindowInfo> {
            self.list_windows()?
                .into_iter()
                .find(|w| w.id == id)
                .ok_or_else(|| anyhow!("Window {} not found", id))
        }
    }

    impl Platform for MacPlatform {
        fn check_permissions(&self) -> PermissionStatus {
            let accessibility = unsafe { AXIsProcessTrusted() };
            let screen_capture = screenshots::Screen::all().is_ok();
            PermissionStatus {
                screen_capture,
                accessibility,
                input_simulation: accessibility,
                cursor_tracking: true,
            }
        }

        fn cursor_position(&self) -> Option<(i32, i32)> {
            unsafe {
                let event = CGEventCreate(std::ptr::null());
                if event.is_null() {
                    return None;
                }
                let pt = CGEventGetLocation(event);
                CFRelease(event);
                Some((pt.x as i32, pt.y as i32))
            }
        }

        fn list_windows(&self) -> Result<Vec<WindowInfo>> {
            let windows = x_win::get_open_windows().map_err(|e| anyhow!(e.to_string()))?;
            Ok(windows
                .into_iter()
                .map(|w| WindowInfo {
                    id: w.id as u64,
                    title: w.title,
                    pid: w.info.process_id,
                    x: w.position.x,
                    y: w.position.y,
                    width: w.position.width.max(0) as u32,
                    height: w.position.height.max(0) as u32,
                    is_minimized: false,
                    is_maximized: w.position.is_full_screen,
                })
                .collect())
        }

        fn window_at_point(&self, x: i32, y: i32) -> Result<Option<WindowInfo>> {
            let mut candidates: Vec<WindowInfo> = self
                .list_windows()?
                .into_iter()
                .filter(|w| {
                    let right = w.x.saturating_add(w.width as i32);
                    let bottom = w.y.saturating_add(w.height as i32);
                    x >= w.x && x < right && y >= w.y && y < bottom
                })
                .collect();
            candidates.sort_by_key(|w| w.width.saturating_mul(w.height));
            Ok(candidates.into_iter().next())
        }

        fn focus_window(&self, id: u64) -> Result<()> {
            let win = self.find_window(id)?;
            let pid = win.pid as i32;
            unsafe {
                let app = AXUIElementCreateApplication(pid);
                if app.is_null() {
                    bail!("Could not create AX element for pid {}", pid);
                }
                let attr = cf_string(b"AXFrontmost\0");
                let result = AXUIElementSetAttributeValue(app, attr, kCFBooleanTrue);
                CFRelease(attr);
                CFRelease(app);
                if result != 0 {
                    bail!(
                        "AXUIElementSetAttributeValue(AXFrontmost) returned {}",
                        result
                    );
                }
            }
            Ok(())
        }

        fn set_position(&self, id: u64, x: i32, y: i32) -> Result<()> {
            let win = self.find_window(id)?;
            let pid = win.pid as i32;
            let point = CgPoint {
                x: x as f64,
                y: y as f64,
            };
            unsafe {
                let app = AXUIElementCreateApplication(pid);
                if app.is_null() {
                    bail!("Could not create AX element for pid {}", pid);
                }
                let mut win_elem: *const c_void = std::ptr::null();
                let rc = _AXUIElementCreateWithWindowID(app, id as u32, &mut win_elem);
                if rc != 0 || win_elem.is_null() {
                    CFRelease(app);
                    bail!("_AXUIElementCreateWithWindowID returned {}", rc);
                }
                let ax_val =
                    AXValueCreate(KAX_VALUE_CG_POINT_TYPE, &point as *const _ as *const c_void);
                let attr = cf_string(b"AXPosition\0");
                let result = AXUIElementSetAttributeValue(win_elem, attr, ax_val);
                CFRelease(attr);
                CFRelease(ax_val);
                CFRelease(win_elem);
                CFRelease(app);
                if result != 0 {
                    bail!(
                        "AXUIElementSetAttributeValue(AXPosition) returned {}",
                        result
                    );
                }
            }
            Ok(())
        }

        fn set_size(&self, id: u64, width: u32, height: u32) -> Result<()> {
            let win = self.find_window(id)?;
            let pid = win.pid as i32;
            let size = CgSize {
                width: width as f64,
                height: height as f64,
            };
            unsafe {
                let app = AXUIElementCreateApplication(pid);
                if app.is_null() {
                    bail!("Could not create AX element for pid {}", pid);
                }
                let mut win_elem: *const c_void = std::ptr::null();
                let rc = _AXUIElementCreateWithWindowID(app, id as u32, &mut win_elem);
                if rc != 0 || win_elem.is_null() {
                    CFRelease(app);
                    bail!("_AXUIElementCreateWithWindowID returned {}", rc);
                }
                let ax_val =
                    AXValueCreate(KAX_VALUE_CG_SIZE_TYPE, &size as *const _ as *const c_void);
                let attr = cf_string(b"AXSize\0");
                let result = AXUIElementSetAttributeValue(win_elem, attr, ax_val);
                CFRelease(attr);
                CFRelease(ax_val);
                CFRelease(win_elem);
                CFRelease(app);
                if result != 0 {
                    bail!("AXUIElementSetAttributeValue(AXSize) returned {}", result);
                }
            }
            Ok(())
        }

        fn set_always_on_top(&self, _id: u64, _enabled: bool) -> Result<()> {
            bail!(
                "macOS does not expose a public API for setting window level from an external process"
            )
        }
    }

    pub use MacPlatform as PlatformImpl;
}

#[cfg(target_os = "macos")]
pub use imp::PlatformImpl as MacPlatform;
