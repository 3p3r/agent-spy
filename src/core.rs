use std::path::Path;

use anyhow::{Context, Result, anyhow, bail};
use enigo::{Button, Coordinate, Direction, Enigo, Keyboard, Mouse, Settings};

use crate::platform::{PermissionStatus, Platform, WindowInfo, create_platform};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MouseButtonArg {
    Left,
    Right,
    Middle,
}

impl MouseButtonArg {
    #[cfg(test)]
    pub fn parse(value: &str) -> Result<Self> {
        match value.to_ascii_lowercase().as_str() {
            "left" => Ok(Self::Left),
            "right" => Ok(Self::Right),
            "middle" => Ok(Self::Middle),
            _ => bail!("Invalid mouse button: {value}. Use left, right, or middle."),
        }
    }

    fn to_enigo(self) -> Button {
        match self {
            Self::Left => Button::Left,
            Self::Right => Button::Right,
            Self::Middle => Button::Middle,
        }
    }
}

pub struct Core {
    platform: Box<dyn Platform>,
    permissions: PermissionStatus,
    enigo: Option<Enigo>,
}

impl Core {
    pub fn new() -> Self {
        let platform = create_platform();
        let permissions = platform.check_permissions();
        let enigo = if permissions.input_simulation {
            Enigo::new(&Settings::default()).ok()
        } else {
            None
        };

        Self {
            platform,
            permissions,
            enigo,
        }
    }

    pub fn permissions(&self) -> &PermissionStatus {
        &self.permissions
    }

    pub fn list_windows(&self, search: Option<&str>) -> Result<Vec<WindowInfo>> {
        let windows = self.platform.list_windows()?;
        let Some(search) = search else {
            return Ok(windows);
        };

        let query = search.trim().to_ascii_lowercase();
        if query.is_empty() {
            return Ok(windows);
        }

        Ok(windows
            .into_iter()
            .filter(|window| window.title.to_ascii_lowercase().contains(&query))
            .collect())
    }

    pub fn window_info(&self, id: u64) -> Result<WindowInfo> {
        self.platform
            .list_windows()?
            .into_iter()
            .find(|window| window.id == id)
            .ok_or_else(|| anyhow!("Window not found for id: {id}"))
    }

    pub fn window_at_point(&self, x: i32, y: i32) -> Result<Option<WindowInfo>> {
        self.platform.window_at_point(x, y)
    }

    pub fn cursor_position(&self) -> Result<(i32, i32)> {
        self.ensure(
            self.permissions.cursor_tracking,
            "Cursor tracking is unavailable on this session.",
        )?;

        self.platform
            .cursor_position()
            .ok_or_else(|| anyhow!("Could not read cursor position."))
    }

    pub fn focus_window(&self, id: u64) -> Result<()> {
        self.ensure(
            self.permissions.accessibility,
            "Window manipulation requires accessibility permission.",
        )?;
        self.platform.focus_window(id)
    }

    pub fn move_window(&self, id: u64, x: i32, y: i32) -> Result<()> {
        self.ensure(
            self.permissions.accessibility,
            "Window manipulation requires accessibility permission.",
        )?;
        self.platform.set_position(id, x, y)
    }

    pub fn resize_window(&self, id: u64, width: u32, height: u32) -> Result<()> {
        self.ensure(
            self.permissions.accessibility,
            "Window manipulation requires accessibility permission.",
        )?;
        self.platform.set_size(id, width, height)
    }

    pub fn set_always_on_top(&self, id: u64, enabled: bool) -> Result<()> {
        self.ensure(
            self.permissions.accessibility,
            "Window manipulation requires accessibility permission.",
        )?;
        self.platform.set_always_on_top(id, enabled)
    }

    pub fn capture_screen_to_file(&self, output: &Path) -> Result<()> {
        self.ensure(
            self.permissions.screen_capture,
            "Screen capture permission is unavailable.",
        )?;

        let capture_result = if let Some((x, y)) = self.platform.cursor_position() {
            screenshots::Screen::from_point(x, y)
                .and_then(|screen| screen.capture())
                .or_else(|_| {
                    screenshots::Screen::all().and_then(|screens| {
                        screens
                            .into_iter()
                            .next()
                            .ok_or_else(|| anyhow!("No monitor found"))
                            .and_then(|screen| screen.capture())
                    })
                })
        } else {
            screenshots::Screen::all().and_then(|screens| {
                screens
                    .into_iter()
                    .next()
                    .ok_or_else(|| anyhow!("No monitor found"))
                    .and_then(|screen| screen.capture())
            })
        };

        let image = capture_result.context("Failed to capture screen")?;
        image
            .save(output)
            .with_context(|| format!("Failed to save screenshot to {}", output.display()))
    }

    pub fn capture_window_to_file(&self, id: u64, output: &Path) -> Result<()> {
        self.ensure(
            self.permissions.screen_capture,
            "Screen capture permission is unavailable.",
        )?;

        let window = self.window_info(id)?;
        let width = window.width.max(1);
        let height = window.height.max(1);

        let image = screenshots::Screen::from_point(window.x, window.y)
            .and_then(|screen| {
                let rel_x = window.x - screen.display_info.x;
                let rel_y = window.y - screen.display_info.y;
                screen.capture_area(rel_x, rel_y, width, height)
            })
            .context("Failed to capture selected window")?;

        image
            .save(output)
            .with_context(|| format!("Failed to save screenshot to {}", output.display()))
    }

    pub fn send_text(&mut self, text: &str) -> Result<()> {
        self.ensure(
            self.permissions.input_simulation,
            "Input simulation permission is unavailable.",
        )?;
        let enigo = self
            .enigo
            .as_mut()
            .ok_or_else(|| anyhow!("Input simulation backend is unavailable."))?;
        enigo.text(text)?;
        Ok(())
    }

    pub fn move_mouse(&mut self, x: i32, y: i32) -> Result<()> {
        self.ensure(
            self.permissions.input_simulation,
            "Input simulation permission is unavailable.",
        )?;
        let enigo = self
            .enigo
            .as_mut()
            .ok_or_else(|| anyhow!("Input simulation backend is unavailable."))?;
        enigo.move_mouse(x, y, Coordinate::Abs)?;
        Ok(())
    }

    pub fn click_mouse(&mut self, x: i32, y: i32, button: MouseButtonArg) -> Result<()> {
        self.ensure(
            self.permissions.input_simulation,
            "Input simulation permission is unavailable.",
        )?;
        let enigo = self
            .enigo
            .as_mut()
            .ok_or_else(|| anyhow!("Input simulation backend is unavailable."))?;
        enigo.move_mouse(x, y, Coordinate::Abs)?;
        enigo.button(button.to_enigo(), Direction::Click)?;
        Ok(())
    }

    fn ensure(&self, condition: bool, message: &str) -> Result<()> {
        if condition {
            Ok(())
        } else {
            bail!(message.to_string())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::MouseButtonArg;

    #[test]
    fn parse_mouse_button_values() {
        assert_eq!(MouseButtonArg::parse("left").unwrap(), MouseButtonArg::Left);
        assert_eq!(
            MouseButtonArg::parse("RIGHT").unwrap(),
            MouseButtonArg::Right
        );
        assert_eq!(
            MouseButtonArg::parse("Middle").unwrap(),
            MouseButtonArg::Middle
        );
    }

    #[test]
    fn reject_invalid_mouse_button() {
        assert!(MouseButtonArg::parse("primary").is_err());
    }
}
