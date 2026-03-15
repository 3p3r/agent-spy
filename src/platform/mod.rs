use anyhow::Result;

#[derive(Debug, Clone)]
pub struct WindowInfo {
    pub id: u64,
    pub title: String,
    pub pid: u32,
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
    pub is_minimized: bool,
    pub is_maximized: bool,
}

#[derive(Debug, Clone, Default)]
pub struct PermissionStatus {
    pub screen_capture: bool,
    pub accessibility: bool,
    pub input_simulation: bool,
    pub cursor_tracking: bool,
}

pub trait Platform: Send {
    fn check_permissions(&self) -> PermissionStatus;
    fn cursor_position(&self) -> Option<(i32, i32)>;
    fn list_windows(&self) -> Result<Vec<WindowInfo>>;
    fn window_at_point(&self, x: i32, y: i32) -> Result<Option<WindowInfo>>;
    fn focused_window_id(&self) -> Result<Option<u64>> {
        Ok(None)
    }
    fn send_text_to_window(&self, _id: u64, _text: &str) -> Result<()> {
        anyhow::bail!("Window-targeted text input is unavailable on this platform.")
    }
    fn send_paste_to_window(&self, _id: u64) -> Result<()> {
        anyhow::bail!("Window-targeted paste is unavailable on this platform.")
    }
    fn focus_window(&self, id: u64) -> Result<()>;
    fn set_position(&self, id: u64, x: i32, y: i32) -> Result<()>;
    fn set_size(&self, id: u64, width: u32, height: u32) -> Result<()>;
    fn set_always_on_top(&self, id: u64, enabled: bool) -> Result<()>;
}

#[cfg(target_os = "linux")]
mod linux;
#[cfg(target_os = "macos")]
mod macos;
#[cfg(target_os = "windows")]
mod windows;

pub fn create_platform() -> Box<dyn Platform> {
    #[cfg(target_os = "linux")]
    {
        return Box::new(linux::LinuxPlatform::new());
    }

    #[cfg(target_os = "windows")]
    {
        return Box::new(windows::WindowsPlatform::new());
    }

    #[cfg(target_os = "macos")]
    {
        return Box::new(macos::MacPlatform::new());
    }

    #[allow(unreachable_code)]
    Box::new(UnsupportedPlatform)
}

struct UnsupportedPlatform;

impl Platform for UnsupportedPlatform {
    fn check_permissions(&self) -> PermissionStatus {
        PermissionStatus::default()
    }

    fn cursor_position(&self) -> Option<(i32, i32)> {
        None
    }

    fn list_windows(&self) -> Result<Vec<WindowInfo>> {
        Ok(Vec::new())
    }

    fn window_at_point(&self, _x: i32, _y: i32) -> Result<Option<WindowInfo>> {
        Ok(None)
    }

    fn focus_window(&self, _id: u64) -> Result<()> {
        anyhow::bail!("Unsupported platform")
    }

    fn set_position(&self, _id: u64, _x: i32, _y: i32) -> Result<()> {
        anyhow::bail!("Unsupported platform")
    }

    fn set_size(&self, _id: u64, _width: u32, _height: u32) -> Result<()> {
        anyhow::bail!("Unsupported platform")
    }

    fn set_always_on_top(&self, _id: u64, _enabled: bool) -> Result<()> {
        anyhow::bail!("Unsupported platform")
    }
}
