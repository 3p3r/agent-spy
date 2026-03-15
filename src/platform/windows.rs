#[cfg(target_os = "windows")]
mod imp {
    use std::ffi::c_void;
    use std::ptr::null_mut;

    use anyhow::{Result, anyhow};
    use windows::Win32::Foundation::{HWND, LPARAM, POINT, WPARAM};
    use windows::Win32::UI::Input::KeyboardAndMouse::{
        MAPVK_VK_TO_VSC, MapVirtualKeyW, VkKeyScanW,
    };
    use windows::Win32::UI::WindowsAndMessaging::{
        FindWindowExW, GA_ROOT, GUITHREADINFO, GetAncestor, GetCursorPos, GetForegroundWindow,
        GetGUIThreadInfo, GetWindowThreadProcessId, HWND_NOTOPMOST, HWND_TOP, HWND_TOPMOST,
        IsChild, SWP_NOMOVE, SWP_NOSIZE, SWP_NOZORDER, SendMessageW, SetForegroundWindow,
        SetWindowPos, WM_CHAR, WM_KEYDOWN, WM_KEYUP, WM_PASTE, WindowFromPoint,
    };
    use windows::core::PCWSTR;

    use crate::platform::{PermissionStatus, Platform, WindowInfo};

    pub struct WindowsPlatform;

    impl WindowsPlatform {
        pub fn new() -> Self {
            Self
        }

        fn to_utf16_null(value: &str) -> Vec<u16> {
            value.encode_utf16().chain(std::iter::once(0)).collect()
        }

        fn find_text_target(root: HWND) -> HWND {
            if let Some(focused) = Self::focused_descendant(root) {
                return focused;
            }

            let candidates = [
                "Edit",
                "RichEditD2DPT",
                "RichEdit20W",
                "RichEdit50W",
                "Scintilla",
            ];

            for class_name in candidates {
                if let Some(found) = Self::find_descendant_by_class(root, class_name, 5) {
                    return found;
                }
            }

            root
        }

        fn focused_descendant(root: HWND) -> Option<HWND> {
            if root.0 == null_mut() {
                return None;
            }

            let mut process_id = 0u32;
            let thread_id = unsafe { GetWindowThreadProcessId(root, Some(&mut process_id)) };
            if thread_id == 0 {
                return None;
            }

            let mut info = GUITHREADINFO {
                cbSize: std::mem::size_of::<GUITHREADINFO>() as u32,
                ..Default::default()
            };
            if unsafe { GetGUIThreadInfo(thread_id, &mut info) }.is_err() {
                return None;
            }

            let focused = info.hwndFocus;
            if focused.0 == null_mut() {
                return None;
            }

            let is_descendant = unsafe { IsChild(root, focused).as_bool() };
            if is_descendant || focused == root {
                Some(focused)
            } else {
                None
            }
        }

        fn key_lparam(vk: u16, keyup: bool) -> LPARAM {
            let scan = unsafe { MapVirtualKeyW(u32::from(vk), MAPVK_VK_TO_VSC) };
            let mut value = 1u32 | (scan << 16);
            if keyup {
                value |= 1 << 30;
                value |= 1 << 31;
            }
            LPARAM(value as isize)
        }

        fn send_utf16_unit(target: HWND, unit: u16) {
            if unit <= 0x7f {
                let vk_scan = unsafe { VkKeyScanW(unit) };
                if vk_scan != -1 {
                    let vk = (vk_scan as u16) & 0xff;
                    let shift_state = ((vk_scan as u16) >> 8) & 0xff;

                    if (shift_state & 0x01) != 0 {
                        unsafe {
                            SendMessageW(
                                target,
                                WM_KEYDOWN,
                                WPARAM(0x10),
                                Self::key_lparam(0x10, false),
                            );
                        }
                    }

                    unsafe {
                        SendMessageW(
                            target,
                            WM_KEYDOWN,
                            WPARAM(vk as usize),
                            Self::key_lparam(vk, false),
                        );
                        SendMessageW(target, WM_CHAR, WPARAM(unit as usize), LPARAM(1));
                        SendMessageW(
                            target,
                            WM_KEYUP,
                            WPARAM(vk as usize),
                            Self::key_lparam(vk, true),
                        );
                    }

                    if (shift_state & 0x01) != 0 {
                        unsafe {
                            SendMessageW(
                                target,
                                WM_KEYUP,
                                WPARAM(0x10),
                                Self::key_lparam(0x10, true),
                            );
                        }
                    }
                    return;
                }
            }

            unsafe {
                SendMessageW(target, WM_CHAR, WPARAM(unit as usize), LPARAM(1));
            }
        }

        fn find_descendant_by_class(root: HWND, class_name: &str, depth: u8) -> Option<HWND> {
            if depth == 0 || root.0 == null_mut() {
                return None;
            }

            let class_wide = Self::to_utf16_null(class_name);
            let direct = unsafe {
                FindWindowExW(
                    root,
                    HWND(null_mut()),
                    PCWSTR(class_wide.as_ptr()),
                    PCWSTR::null(),
                )
            }
            .ok();
            if let Some(found) = direct
                && found.0 != null_mut()
            {
                return Some(found);
            }

            let mut child =
                unsafe { FindWindowExW(root, HWND(null_mut()), PCWSTR::null(), PCWSTR::null()) }
                    .ok();
            while let Some(current_child) = child {
                if current_child.0 == null_mut() {
                    break;
                }

                if let Some(found) =
                    Self::find_descendant_by_class(current_child, class_name, depth - 1)
                {
                    return Some(found);
                }

                child =
                    unsafe { FindWindowExW(root, current_child, PCWSTR::null(), PCWSTR::null()) }
                        .ok();
            }

            None
        }
    }

    impl Platform for WindowsPlatform {
        fn check_permissions(&self) -> PermissionStatus {
            PermissionStatus {
                screen_capture: true,
                accessibility: true,
                input_simulation: true,
                cursor_tracking: true,
            }
        }

        fn cursor_position(&self) -> Option<(i32, i32)> {
            let mut point = POINT::default();
            unsafe { GetCursorPos(&mut point).ok()? };
            Some((point.x, point.y))
        }

        fn list_windows(&self) -> Result<Vec<WindowInfo>> {
            let windows = x_win::get_open_windows().map_err(|error| anyhow!(error.to_string()))?;
            Ok(windows
                .into_iter()
                .map(|window| WindowInfo {
                    id: window.id as u64,
                    title: window.title,
                    pid: window.info.process_id,
                    process_name: window.info.name,
                    exec_name: window.info.exec_name,
                    process_path: window.info.path,
                    x: window.position.x,
                    y: window.position.y,
                    width: window.position.width.max(0) as u32,
                    height: window.position.height.max(0) as u32,
                    is_minimized: false,
                    is_maximized: window.position.is_full_screen,
                })
                .collect())
        }

        fn window_at_point(&self, x: i32, y: i32) -> Result<Option<WindowInfo>> {
            let point = POINT { x, y };
            let hwnd = unsafe { WindowFromPoint(point) };
            if hwnd.0 == null_mut() {
                return Ok(None);
            }

            let root = unsafe { GetAncestor(hwnd, GA_ROOT) };
            let root_id = root.0 as usize as u32 as u64;
            Ok(self
                .list_windows()?
                .into_iter()
                .find(|window| window.id == root_id))
        }

        fn focused_window_id(&self) -> Result<Option<u64>> {
            let hwnd = unsafe { GetForegroundWindow() };
            if hwnd.0 == null_mut() {
                return Ok(None);
            }

            Ok(Some(hwnd.0 as usize as u64))
        }

        fn send_text_to_window(&self, id: u64, text: &str) -> Result<()> {
            let hwnd = HWND(id as usize as *mut c_void);
            if hwnd.0 == null_mut() {
                return Err(anyhow!("Invalid window id"));
            }

            let target = WindowsPlatform::find_text_target(hwnd);

            for unit in text.encode_utf16() {
                Self::send_utf16_unit(target, unit);
            }

            Ok(())
        }

        fn send_paste_to_window(&self, id: u64) -> Result<()> {
            let hwnd = HWND(id as usize as *mut c_void);
            if hwnd.0 == null_mut() {
                return Err(anyhow!("Invalid window id"));
            }

            let target = WindowsPlatform::find_text_target(hwnd);
            unsafe {
                SendMessageW(target, WM_PASTE, WPARAM(0), LPARAM(0));
            }

            Ok(())
        }

        fn focus_window(&self, id: u64) -> Result<()> {
            let hwnd = HWND(id as usize as *mut c_void);
            unsafe {
                let _ = SetForegroundWindow(hwnd);
            }
            Ok(())
        }

        fn set_position(&self, id: u64, x: i32, y: i32) -> Result<()> {
            let hwnd = HWND(id as usize as *mut c_void);
            unsafe {
                SetWindowPos(hwnd, HWND_TOP, x, y, 0, 0, SWP_NOSIZE | SWP_NOZORDER)?;
            }
            Ok(())
        }

        fn set_size(&self, id: u64, width: u32, height: u32) -> Result<()> {
            let hwnd = HWND(id as usize as *mut c_void);
            unsafe {
                SetWindowPos(
                    hwnd,
                    HWND_TOP,
                    0,
                    0,
                    width as i32,
                    height as i32,
                    SWP_NOMOVE | SWP_NOZORDER,
                )?;
            }
            Ok(())
        }

        fn set_always_on_top(&self, id: u64, enabled: bool) -> Result<()> {
            let hwnd = HWND(id as usize as *mut c_void);
            let insert_after = if enabled {
                HWND_TOPMOST
            } else {
                HWND_NOTOPMOST
            };
            unsafe {
                SetWindowPos(hwnd, insert_after, 0, 0, 0, 0, SWP_NOMOVE | SWP_NOSIZE)?;
            }
            Ok(())
        }
    }

    pub use WindowsPlatform as PlatformImpl;
}

#[cfg(target_os = "windows")]
pub use imp::PlatformImpl as WindowsPlatform;
