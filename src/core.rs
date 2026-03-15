#[cfg(target_os = "linux")]
use std::collections::HashMap;
use std::path::Path;
use std::time::Duration;

use anyhow::{Context, Result, anyhow, bail};
use enigo::{Axis, Button, Coordinate, Direction, Enigo, Key, Keyboard, Mouse, Settings};

#[cfg(target_os = "windows")]
use windows::Win32::UI::Input::KeyboardAndMouse::{
    INPUT, INPUT_0, INPUT_KEYBOARD, KEYBDINPUT, KEYEVENTF_KEYUP, KEYEVENTF_UNICODE, SendInput,
    VIRTUAL_KEY,
};

#[cfg(target_os = "linux")]
use x11rb::connection::Connection;
#[cfg(target_os = "linux")]
use x11rb::protocol::xproto::{ConnectionExt as XprotoExt, Keysym, Window};
#[cfg(target_os = "linux")]
use x11rb::protocol::xtest::ConnectionExt as XtestExt;

use crate::platform::{PermissionStatus, Platform, WindowInfo, create_platform};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModifierKey {
    Shift,
    Control,
    Alt,
    Meta,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct ModifierState {
    pub shift: bool,
    pub control: bool,
    pub alt: bool,
    pub meta: bool,
}

impl ModifierState {
    fn is_pressed(self, modifier: ModifierKey) -> bool {
        match modifier {
            ModifierKey::Shift => self.shift,
            ModifierKey::Control => self.control,
            ModifierKey::Alt => self.alt,
            ModifierKey::Meta => self.meta,
        }
    }

    fn set(&mut self, modifier: ModifierKey, pressed: bool) {
        match modifier {
            ModifierKey::Shift => self.shift = pressed,
            ModifierKey::Control => self.control = pressed,
            ModifierKey::Alt => self.alt = pressed,
            ModifierKey::Meta => self.meta = pressed,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScrollAxisArg {
    Vertical,
    Horizontal,
}

impl ScrollAxisArg {
    fn to_enigo(self) -> Axis {
        match self {
            Self::Vertical => Axis::Vertical,
            Self::Horizontal => Axis::Horizontal,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeyArg {
    Named(NamedKey),
    Unicode(char),
}

impl KeyArg {
    pub fn parse(value: &str) -> Result<Self> {
        let trimmed = value.trim();
        if trimmed.is_empty() {
            bail!("Key cannot be empty.");
        }

        let normalized = trimmed.to_ascii_lowercase();
        let named = match normalized.as_str() {
            "alt" | "option" => Some(NamedKey::Alt),
            "backspace" => Some(NamedKey::Backspace),
            "ctrl" | "control" => Some(NamedKey::Control),
            "del" | "delete" => Some(NamedKey::Delete),
            "down" | "downarrow" => Some(NamedKey::DownArrow),
            "end" => Some(NamedKey::End),
            "enter" | "return" => Some(NamedKey::Return),
            "esc" | "escape" => Some(NamedKey::Escape),
            "home" => Some(NamedKey::Home),
            "left" | "leftarrow" => Some(NamedKey::LeftArrow),
            "meta" | "cmd" | "command" | "super" | "win" | "windows" => Some(NamedKey::Meta),
            "pagedown" | "page-down" => Some(NamedKey::PageDown),
            "pageup" | "page-up" => Some(NamedKey::PageUp),
            "right" | "rightarrow" => Some(NamedKey::RightArrow),
            "shift" => Some(NamedKey::Shift),
            "space" => Some(NamedKey::Space),
            "tab" => Some(NamedKey::Tab),
            "up" | "uparrow" => Some(NamedKey::UpArrow),
            "f1" => Some(NamedKey::F1),
            "f2" => Some(NamedKey::F2),
            "f3" => Some(NamedKey::F3),
            "f4" => Some(NamedKey::F4),
            "f5" => Some(NamedKey::F5),
            "f6" => Some(NamedKey::F6),
            "f7" => Some(NamedKey::F7),
            "f8" => Some(NamedKey::F8),
            "f9" => Some(NamedKey::F9),
            "f10" => Some(NamedKey::F10),
            "f11" => Some(NamedKey::F11),
            "f12" => Some(NamedKey::F12),
            _ => None,
        };

        if let Some(named) = named {
            return Ok(Self::Named(named));
        }

        let mut chars = trimmed.chars();
        let Some(ch) = chars.next() else {
            bail!("Key cannot be empty.");
        };
        if chars.next().is_some() {
            bail!("Unsupported key: {value}. Use a named key like 'enter' or a single character.");
        }

        Ok(Self::Unicode(ch))
    }

    fn modifier(self) -> Option<ModifierKey> {
        match self {
            Self::Named(NamedKey::Alt) => Some(ModifierKey::Alt),
            Self::Named(NamedKey::Control) => Some(ModifierKey::Control),
            Self::Named(NamedKey::Meta) => Some(ModifierKey::Meta),
            Self::Named(NamedKey::Shift) => Some(ModifierKey::Shift),
            Self::Named(_) | Self::Unicode(_) => None,
        }
    }

    fn to_enigo(self) -> Key {
        match self {
            Self::Named(named) => named.to_enigo(),
            Self::Unicode(ch) => Key::Unicode(ch),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NamedKey {
    Alt,
    Backspace,
    Control,
    Delete,
    DownArrow,
    End,
    Escape,
    F1,
    F2,
    F3,
    F4,
    F5,
    F6,
    F7,
    F8,
    F9,
    F10,
    F11,
    F12,
    Home,
    LeftArrow,
    Meta,
    PageDown,
    PageUp,
    Return,
    RightArrow,
    Shift,
    Space,
    Tab,
    UpArrow,
}

impl NamedKey {
    fn to_enigo(self) -> Key {
        match self {
            Self::Alt => Key::Alt,
            Self::Backspace => Key::Backspace,
            Self::Control => Key::Control,
            Self::Delete => Key::Delete,
            Self::DownArrow => Key::DownArrow,
            Self::End => Key::End,
            Self::Escape => Key::Escape,
            Self::F1 => Key::F1,
            Self::F2 => Key::F2,
            Self::F3 => Key::F3,
            Self::F4 => Key::F4,
            Self::F5 => Key::F5,
            Self::F6 => Key::F6,
            Self::F7 => Key::F7,
            Self::F8 => Key::F8,
            Self::F9 => Key::F9,
            Self::F10 => Key::F10,
            Self::F11 => Key::F11,
            Self::F12 => Key::F12,
            Self::Home => Key::Home,
            Self::LeftArrow => Key::LeftArrow,
            Self::Meta => Key::Meta,
            Self::PageDown => Key::PageDown,
            Self::PageUp => Key::PageUp,
            Self::Return => Key::Return,
            Self::RightArrow => Key::RightArrow,
            Self::Shift => Key::Shift,
            Self::Space => Key::Space,
            Self::Tab => Key::Tab,
            Self::UpArrow => Key::UpArrow,
        }
    }
}

#[derive(Default)]
struct ButtonState {
    left: bool,
    right: bool,
    middle: bool,
}

impl ButtonState {
    fn set(&mut self, button: MouseButtonArg, pressed: bool) {
        match button {
            MouseButtonArg::Left => self.left = pressed,
            MouseButtonArg::Right => self.right = pressed,
            MouseButtonArg::Middle => self.middle = pressed,
        }
    }
}

struct EnigoInputBackend {
    enigo: Enigo,
    modifiers: ModifierState,
    buttons: ButtonState,
}

impl EnigoInputBackend {
    fn new() -> Result<Self> {
        Ok(Self {
            enigo: Enigo::new(&Settings::default())?,
            modifiers: ModifierState::default(),
            buttons: ButtonState::default(),
        })
    }

    fn text(&mut self, text: &str) -> Result<()> {
        #[cfg(target_os = "windows")]
        {
            if send_unicode_text_windows(text).is_ok() {
                return Ok(());
            }
        }

        self.enigo.text(text)?;
        Ok(())
    }

    fn move_mouse(&mut self, x: i32, y: i32, coordinate: Coordinate) -> Result<()> {
        self.enigo.move_mouse(x, y, coordinate)?;
        Ok(())
    }

    fn mouse_button(&mut self, button: MouseButtonArg, direction: Direction) -> Result<()> {
        self.enigo.button(button.to_enigo(), direction)?;
        if matches!(direction, Direction::Click) {
            self.buttons.set(button, false);
        } else {
            self.buttons
                .set(button, matches!(direction, Direction::Press));
        }
        Ok(())
    }

    fn key(&mut self, key: KeyArg, direction: Direction) -> Result<()> {
        self.enigo.key(key.to_enigo(), direction)?;
        if let Some(modifier) = key.modifier() {
            if matches!(direction, Direction::Click) {
                self.modifiers.set(modifier, false);
            } else {
                self.modifiers
                    .set(modifier, matches!(direction, Direction::Press));
            }
        }
        Ok(())
    }

    fn key_tap(&mut self, key: KeyArg, modifiers: &[ModifierKey]) -> Result<()> {
        let mut temporary = Vec::new();
        for &modifier in modifiers {
            if !self.modifiers.is_pressed(modifier) {
                self.key(
                    KeyArg::Named(match modifier {
                        ModifierKey::Shift => NamedKey::Shift,
                        ModifierKey::Control => NamedKey::Control,
                        ModifierKey::Alt => NamedKey::Alt,
                        ModifierKey::Meta => NamedKey::Meta,
                    }),
                    Direction::Press,
                )?;
                temporary.push(modifier);
            }
        }
        self.key(key, Direction::Click)?;
        for modifier in temporary.into_iter().rev() {
            self.key(
                KeyArg::Named(match modifier {
                    ModifierKey::Shift => NamedKey::Shift,
                    ModifierKey::Control => NamedKey::Control,
                    ModifierKey::Alt => NamedKey::Alt,
                    ModifierKey::Meta => NamedKey::Meta,
                }),
                Direction::Release,
            )?;
        }
        Ok(())
    }

    fn scroll(&mut self, length: i32, axis: ScrollAxisArg) -> Result<()> {
        self.enigo.scroll(length, axis.to_enigo())?;
        Ok(())
    }
}

#[cfg(target_os = "windows")]
fn send_unicode_text_windows(text: &str) -> Result<()> {
    if text.is_empty() {
        return Ok(());
    }

    let mut inputs = Vec::with_capacity(text.encode_utf16().count() * 2);
    for unit in text.encode_utf16() {
        inputs.push(INPUT {
            r#type: INPUT_KEYBOARD,
            Anonymous: INPUT_0 {
                ki: KEYBDINPUT {
                    wVk: VIRTUAL_KEY(0),
                    wScan: unit,
                    dwFlags: KEYEVENTF_UNICODE,
                    time: 0,
                    dwExtraInfo: 0,
                },
            },
        });
        inputs.push(INPUT {
            r#type: INPUT_KEYBOARD,
            Anonymous: INPUT_0 {
                ki: KEYBDINPUT {
                    wVk: VIRTUAL_KEY(0),
                    wScan: unit,
                    dwFlags: KEYEVENTF_UNICODE | KEYEVENTF_KEYUP,
                    time: 0,
                    dwExtraInfo: 0,
                },
            },
        });
    }

    let sent = unsafe { SendInput(&inputs, std::mem::size_of::<INPUT>() as i32) };
    if sent != inputs.len() as u32 {
        bail!("Windows SendInput failed to inject full text.");
    }

    Ok(())
}

#[cfg(target_os = "linux")]
struct LinuxX11InputBackend {
    conn: x11rb::rust_connection::RustConnection,
    root: Window,
    keycodes: HashMap<Keysym, u8>,
    enigo_fallback: Option<EnigoInputBackend>,
    modifiers: ModifierState,
    buttons: ButtonState,
}

#[cfg(target_os = "linux")]
impl LinuxX11InputBackend {
    fn new() -> Result<Self> {
        let (conn, screen_num) = x11rb::connect(None)?;
        let root = conn
            .setup()
            .roots
            .get(screen_num)
            .ok_or_else(|| anyhow!("No X11 screen available"))?
            .root;

        conn.xtest_get_version(2, 2)?.reply()?;

        let min = conn.setup().min_keycode;
        let max = conn.setup().max_keycode;
        let count = max.saturating_sub(min).saturating_add(1);
        let mapping = conn.get_keyboard_mapping(min, count)?.reply()?;

        let mut keycodes = HashMap::new();
        for offset in 0..count {
            let keycode = min.saturating_add(offset);
            let start = usize::from(offset) * usize::from(mapping.keysyms_per_keycode);
            let end = start + usize::from(mapping.keysyms_per_keycode);
            for &keysym in &mapping.keysyms[start..end] {
                if keysym != 0 {
                    keycodes.entry(keysym).or_insert(keycode);
                }
            }
        }

        Ok(Self {
            conn,
            root,
            keycodes,
            enigo_fallback: EnigoInputBackend::new().ok(),
            modifiers: ModifierState::default(),
            buttons: ButtonState::default(),
        })
    }

    fn key(&mut self, key: KeyArg, direction: Direction) -> Result<()> {
        let keysym = keysym_for_key(key);
        let Some(&keycode) = self.keycodes.get(&keysym) else {
            if let Some(enigo) = self.enigo_fallback.as_mut() {
                let result = enigo.key(key, direction);
                self.update_modifier_state(key, direction);
                return result;
            }
            return Err(anyhow!("No keycode mapping found for key"));
        };

        match direction {
            Direction::Click => {
                self.fake_input(2, keycode)?;
                self.fake_input(3, keycode)?;
            }
            Direction::Press => self.fake_input(2, keycode)?,
            Direction::Release => self.fake_input(3, keycode)?,
        }

        self.update_modifier_state(key, direction);

        self.conn.flush()?;
        Ok(())
    }

    fn text(&mut self, text: &str) -> Result<()> {
        if let Some(enigo) = self.enigo_fallback.as_mut() {
            return enigo.text(text);
        }

        for ch in text.chars() {
            self.key(KeyArg::Unicode(ch), Direction::Click)?;
        }
        Ok(())
    }

    fn move_mouse(&mut self, x: i32, y: i32, coordinate: Coordinate) -> Result<()> {
        let (target_x, target_y) = if matches!(coordinate, Coordinate::Rel) {
            let pointer = self.conn.query_pointer(self.root)?.reply()?;
            (
                i32::from(pointer.root_x).saturating_add(x),
                i32::from(pointer.root_y).saturating_add(y),
            )
        } else {
            (x, y)
        };

        let clamped_x = target_x.clamp(i32::from(i16::MIN), i32::from(i16::MAX)) as i16;
        let clamped_y = target_y.clamp(i32::from(i16::MIN), i32::from(i16::MAX)) as i16;
        self.conn
            .xtest_fake_input(6, 0, 0, self.root, clamped_x, clamped_y, 0)?;
        self.conn.flush()?;
        Ok(())
    }

    fn mouse_button(&mut self, button: MouseButtonArg, direction: Direction) -> Result<()> {
        let detail = match button {
            MouseButtonArg::Left => 1,
            MouseButtonArg::Middle => 2,
            MouseButtonArg::Right => 3,
        };

        match direction {
            Direction::Click => {
                self.fake_input(4, detail)?;
                self.fake_input(5, detail)?;
                self.buttons.set(button, false);
            }
            Direction::Press => {
                self.fake_input(4, detail)?;
                self.buttons.set(button, true);
            }
            Direction::Release => {
                self.fake_input(5, detail)?;
                self.buttons.set(button, false);
            }
        }

        self.conn.flush()?;
        Ok(())
    }

    fn scroll(&mut self, length: i32, axis: ScrollAxisArg) -> Result<()> {
        let (forward, backward) = match axis {
            ScrollAxisArg::Vertical => (5u8, 4u8),
            ScrollAxisArg::Horizontal => (7u8, 6u8),
        };
        let detail = if length >= 0 { forward } else { backward };

        for _ in 0..length.unsigned_abs() {
            self.fake_input(4, detail)?;
            self.fake_input(5, detail)?;
        }

        self.conn.flush()?;
        Ok(())
    }

    fn key_tap(&mut self, key: KeyArg, modifiers: &[ModifierKey]) -> Result<()> {
        let mut temporary = Vec::new();
        for &modifier in modifiers {
            if !self.modifiers.is_pressed(modifier) {
                self.key(
                    KeyArg::Named(match modifier {
                        ModifierKey::Shift => NamedKey::Shift,
                        ModifierKey::Control => NamedKey::Control,
                        ModifierKey::Alt => NamedKey::Alt,
                        ModifierKey::Meta => NamedKey::Meta,
                    }),
                    Direction::Press,
                )?;
                temporary.push(modifier);
            }
        }

        self.key(key, Direction::Click)?;

        for modifier in temporary.into_iter().rev() {
            self.key(
                KeyArg::Named(match modifier {
                    ModifierKey::Shift => NamedKey::Shift,
                    ModifierKey::Control => NamedKey::Control,
                    ModifierKey::Alt => NamedKey::Alt,
                    ModifierKey::Meta => NamedKey::Meta,
                }),
                Direction::Release,
            )?;
        }
        Ok(())
    }

    fn fake_input(&self, type_: u8, detail: u8) -> Result<()> {
        self.conn
            .xtest_fake_input(type_, detail, 0, self.root, 0, 0, 0)?;
        Ok(())
    }

    fn update_modifier_state(&mut self, key: KeyArg, direction: Direction) {
        if let Some(modifier) = key.modifier() {
            if matches!(direction, Direction::Click) {
                self.modifiers.set(modifier, false);
            } else {
                self.modifiers
                    .set(modifier, matches!(direction, Direction::Press));
            }
        }
    }
}

#[cfg(target_os = "linux")]
fn keysym_for_key(key: KeyArg) -> Keysym {
    match key {
        KeyArg::Named(NamedKey::Backspace) => 0xff08,
        KeyArg::Named(NamedKey::Tab) => 0xff09,
        KeyArg::Named(NamedKey::Return) => 0xff0d,
        KeyArg::Named(NamedKey::Escape) => 0xff1b,
        KeyArg::Named(NamedKey::Space) => 0x0020,
        KeyArg::Named(NamedKey::Home) => 0xff50,
        KeyArg::Named(NamedKey::LeftArrow) => 0xff51,
        KeyArg::Named(NamedKey::UpArrow) => 0xff52,
        KeyArg::Named(NamedKey::RightArrow) => 0xff53,
        KeyArg::Named(NamedKey::DownArrow) => 0xff54,
        KeyArg::Named(NamedKey::PageUp) => 0xff55,
        KeyArg::Named(NamedKey::PageDown) => 0xff56,
        KeyArg::Named(NamedKey::End) => 0xff57,
        KeyArg::Named(NamedKey::Delete) => 0xffff,
        KeyArg::Named(NamedKey::Shift) => 0xffe1,
        KeyArg::Named(NamedKey::Control) => 0xffe3,
        KeyArg::Named(NamedKey::Alt) => 0xffe9,
        KeyArg::Named(NamedKey::Meta) => 0xffeb,
        KeyArg::Named(NamedKey::F1) => 0xffbe,
        KeyArg::Named(NamedKey::F2) => 0xffbf,
        KeyArg::Named(NamedKey::F3) => 0xffc0,
        KeyArg::Named(NamedKey::F4) => 0xffc1,
        KeyArg::Named(NamedKey::F5) => 0xffc2,
        KeyArg::Named(NamedKey::F6) => 0xffc3,
        KeyArg::Named(NamedKey::F7) => 0xffc4,
        KeyArg::Named(NamedKey::F8) => 0xffc5,
        KeyArg::Named(NamedKey::F9) => 0xffc6,
        KeyArg::Named(NamedKey::F10) => 0xffc7,
        KeyArg::Named(NamedKey::F11) => 0xffc8,
        KeyArg::Named(NamedKey::F12) => 0xffc9,
        KeyArg::Unicode(ch) => {
            let code = ch as u32;
            if code <= 0xff {
                code
            } else {
                0x0100_0000 | code
            }
        }
    }
}

enum InputBackend {
    Enigo(EnigoInputBackend),
    #[cfg(target_os = "linux")]
    LinuxX11(LinuxX11InputBackend),
}

impl InputBackend {
    fn new() -> Result<Self> {
        #[cfg(target_os = "linux")]
        {
            let is_wayland = std::env::var_os("WAYLAND_DISPLAY").is_some()
                || std::env::var("XDG_SESSION_TYPE")
                    .map(|value| value.eq_ignore_ascii_case("wayland"))
                    .unwrap_or(false);

            if !is_wayland && let Ok(x11) = LinuxX11InputBackend::new() {
                return Ok(Self::LinuxX11(x11));
            }
        }

        Ok(Self::Enigo(EnigoInputBackend::new()?))
    }

    fn backend_name(&self) -> &'static str {
        match self {
            Self::Enigo(_) => "enigo",
            #[cfg(target_os = "linux")]
            Self::LinuxX11(_) => "linux-x11-xtest",
        }
    }

    fn text(&mut self, text: &str) -> Result<()> {
        match self {
            Self::Enigo(enigo) => enigo.text(text),
            #[cfg(target_os = "linux")]
            Self::LinuxX11(x11) => x11.text(text),
        }
    }

    fn move_mouse(&mut self, x: i32, y: i32, coordinate: Coordinate) -> Result<()> {
        match self {
            Self::Enigo(enigo) => enigo.move_mouse(x, y, coordinate),
            #[cfg(target_os = "linux")]
            Self::LinuxX11(x11) => x11.move_mouse(x, y, coordinate),
        }
    }

    fn mouse_button(&mut self, button: MouseButtonArg, direction: Direction) -> Result<()> {
        match self {
            Self::Enigo(enigo) => enigo.mouse_button(button, direction),
            #[cfg(target_os = "linux")]
            Self::LinuxX11(x11) => x11.mouse_button(button, direction),
        }
    }

    fn key(&mut self, key: KeyArg, direction: Direction) -> Result<()> {
        match self {
            Self::Enigo(enigo) => enigo.key(key, direction),
            #[cfg(target_os = "linux")]
            Self::LinuxX11(x11) => x11.key(key, direction),
        }
    }

    fn key_tap(&mut self, key: KeyArg, modifiers: &[ModifierKey]) -> Result<()> {
        match self {
            Self::Enigo(enigo) => enigo.key_tap(key, modifiers),
            #[cfg(target_os = "linux")]
            Self::LinuxX11(x11) => x11.key_tap(key, modifiers),
        }
    }

    fn scroll(&mut self, length: i32, axis: ScrollAxisArg) -> Result<()> {
        match self {
            Self::Enigo(enigo) => enigo.scroll(length, axis),
            #[cfg(target_os = "linux")]
            Self::LinuxX11(x11) => x11.scroll(length, axis),
        }
    }

    fn modifier_state(&self) -> ModifierState {
        match self {
            Self::Enigo(enigo) => enigo.modifiers,
            #[cfg(target_os = "linux")]
            Self::LinuxX11(x11) => x11.modifiers,
        }
    }
}

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
    input: Option<InputBackend>,
}

impl Core {
    pub fn new() -> Self {
        let platform = create_platform();
        let permissions = platform.check_permissions();
        let input = if permissions.input_simulation {
            InputBackend::new().ok()
        } else {
            None
        };

        Self {
            platform,
            permissions,
            input,
        }
    }

    pub fn permissions(&self) -> &PermissionStatus {
        &self.permissions
    }

    pub fn input_backend_name(&self) -> &'static str {
        self.input
            .as_ref()
            .map(InputBackend::backend_name)
            .unwrap_or("unavailable")
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

    pub fn send_text(
        &mut self,
        text: &str,
        target_window: Option<u64>,
        allow_focus_swap_fallback: bool,
        send_via_clipboard_paste: bool,
    ) -> Result<()> {
        if text.is_empty() {
            return Ok(());
        }

        match (allow_focus_swap_fallback, send_via_clipboard_paste) {
            (false, false) => {
                if let Some(window_id) = target_window {
                    return self
                        .platform
                        .send_text_to_window(window_id, text)
                        .with_context(|| {
                            format!(
                                "Window-targeted text send failed for selected window {window_id}"
                            )
                        });
                }

                self.input()?.text(text)
            }
            (true, false) => {
                if let Some(window_id) = target_window {
                    return self.with_focus_swap(window_id, |core| {
                        match core.platform.send_text_to_window(window_id, text) {
                            Ok(()) => Ok(()),
                            Err(_) => core.input()?.text(text),
                        }
                    });
                }

                self.input()?.text(text)
            }
            (false, true) => {
                self.copy_to_clipboard(text)?;

                if let Some(window_id) = target_window {
                    return self
                        .platform
                        .send_paste_to_window(window_id)
                        .with_context(|| {
                            format!("Window-targeted paste failed for selected window {window_id}")
                        });
                }

                self.send_paste_shortcut()
            }
            (true, true) => {
                self.copy_to_clipboard(text)?;

                if let Some(window_id) = target_window {
                    return self.with_focus_swap(window_id, |core| core.send_paste_shortcut());
                }

                self.send_paste_shortcut()
            }
        }
    }

    pub fn move_mouse(&mut self, x: i32, y: i32) -> Result<()> {
        self.input()?.move_mouse(x, y, Coordinate::Abs)
    }

    pub fn click_mouse(&mut self, x: i32, y: i32, button: MouseButtonArg) -> Result<()> {
        let input = self.input()?;
        input.move_mouse(x, y, Coordinate::Abs)?;
        input.mouse_button(button, Direction::Click)
    }

    pub fn mouse_down(&mut self, x: i32, y: i32, button: MouseButtonArg) -> Result<()> {
        let input = self.input()?;
        input.move_mouse(x, y, Coordinate::Abs)?;
        input.mouse_button(button, Direction::Press)
    }

    pub fn mouse_up(&mut self, x: i32, y: i32, button: MouseButtonArg) -> Result<()> {
        let input = self.input()?;
        input.move_mouse(x, y, Coordinate::Abs)?;
        input.mouse_button(button, Direction::Release)
    }

    pub fn drag_mouse(
        &mut self,
        start_x: i32,
        start_y: i32,
        end_x: i32,
        end_y: i32,
        button: MouseButtonArg,
    ) -> Result<()> {
        let input = self.input()?;
        input.move_mouse(start_x, start_y, Coordinate::Abs)?;
        input.mouse_button(button, Direction::Press)?;
        input.move_mouse(end_x, end_y, Coordinate::Abs)?;
        input.mouse_button(button, Direction::Release)
    }

    pub fn scroll(&mut self, length: i32, axis: ScrollAxisArg) -> Result<()> {
        self.input()?.scroll(length, axis)
    }

    pub fn key_down(&mut self, key: KeyArg) -> Result<()> {
        self.input()?.key(key, Direction::Press)
    }

    pub fn key_up(&mut self, key: KeyArg) -> Result<()> {
        self.input()?.key(key, Direction::Release)
    }

    pub fn key_tap(&mut self, key: KeyArg, modifiers: &[ModifierKey]) -> Result<()> {
        self.input()?.key_tap(key, modifiers)
    }

    pub fn modifier_state(&self) -> ModifierState {
        self.input
            .as_ref()
            .map(InputBackend::modifier_state)
            .unwrap_or_default()
    }

    fn input(&mut self) -> Result<&mut InputBackend> {
        self.ensure(
            self.permissions.input_simulation,
            "Input simulation permission is unavailable.",
        )?;

        self.input
            .as_mut()
            .ok_or_else(|| anyhow!("Input simulation backend is unavailable."))
    }

    fn ensure(&self, condition: bool, message: &str) -> Result<()> {
        if condition {
            Ok(())
        } else {
            bail!(message.to_string())
        }
    }

    fn with_focus_swap<F>(&mut self, window_id: u64, action: F) -> Result<()>
    where
        F: FnOnce(&mut Self) -> Result<()>,
    {
        let previous_focus = self.platform.focused_window_id().ok().flatten();

        self.focus_window(window_id)
            .context("Focus-swap fallback failed before text input")?;
        std::thread::sleep(Duration::from_millis(50));

        let result = action(self);

        std::thread::sleep(Duration::from_millis(20));
        if let Some(previous_window_id) = previous_focus
            && previous_window_id != window_id
        {
            let _ = self.focus_window(previous_window_id);
        }

        result
    }

    fn copy_to_clipboard(&self, text: &str) -> Result<()> {
        let mut clipboard = arboard::Clipboard::new().context("Failed to open clipboard")?;
        clipboard
            .set_text(text.to_string())
            .context("Failed to write clipboard text")
    }

    fn send_paste_shortcut(&mut self) -> Result<()> {
        let modifier = if cfg!(target_os = "macos") {
            ModifierKey::Meta
        } else {
            ModifierKey::Control
        };
        self.key_tap(KeyArg::Unicode('v'), &[modifier])
    }
}

#[cfg(test)]
mod tests {
    use super::{KeyArg, ModifierKey, ModifierState, MouseButtonArg};

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

    #[test]
    fn parse_named_key() {
        assert_eq!(
            KeyArg::parse("enter").unwrap(),
            KeyArg::Named(super::NamedKey::Return)
        );
        assert_eq!(KeyArg::parse("A").unwrap(), KeyArg::Unicode('A'));
    }

    #[test]
    fn reject_invalid_named_key() {
        assert!(KeyArg::parse("unknown-key").is_err());
        assert!(KeyArg::parse("ab").is_err());
    }

    #[test]
    fn modifier_state_tracks_flags() {
        let mut state = ModifierState::default();
        state.set(ModifierKey::Shift, true);
        assert!(state.is_pressed(ModifierKey::Shift));
        state.set(ModifierKey::Shift, false);
        assert!(!state.is_pressed(ModifierKey::Shift));
    }
}
