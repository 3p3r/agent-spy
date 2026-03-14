#[cfg(target_os = "windows")]
mod imp {
    use std::ffi::c_void;
    use std::ptr::null_mut;

    use anyhow::{Result, anyhow};
    use windows::Win32::Foundation::{HWND, POINT};
    use windows::Win32::UI::WindowsAndMessaging::{
        GA_ROOT, GetAncestor, GetCursorPos, HWND_NOTOPMOST, HWND_TOP, HWND_TOPMOST, SWP_NOMOVE,
        SWP_NOSIZE, SWP_NOZORDER, SetForegroundWindow, SetWindowPos, WindowFromPoint,
    };

    use crate::platform::{PermissionStatus, Platform, WindowInfo};

    pub struct WindowsPlatform;

    impl WindowsPlatform {
        pub fn new() -> Self {
            Self
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
