use std::collections::HashMap;

use anyhow::{Result, anyhow, bail};
use enigo::{Enigo, Settings};
use x11rb::connection::Connection;
use x11rb::protocol::xproto::{
    ConfigureWindowAux, ConnectionExt, EventMask, InputFocus, KEY_PRESS_EVENT, KEY_RELEASE_EVENT,
    KeyButMask, KeyPressEvent, KeyReleaseEvent,
};

use crate::platform::{PermissionStatus, Platform, WindowInfo};

pub struct LinuxPlatform {
    is_wayland: bool,
}

impl LinuxPlatform {
    pub fn new() -> Self {
        let is_wayland = std::env::var_os("WAYLAND_DISPLAY").is_some()
            || std::env::var("XDG_SESSION_TYPE")
                .map(|value| value.eq_ignore_ascii_case("wayland"))
                .unwrap_or(false);
        Self { is_wayland }
    }

    fn x11_connection(&self) -> Result<(x11rb::rust_connection::RustConnection, usize)> {
        let (conn, screen_num) = x11rb::connect(None)?;
        Ok((conn, screen_num))
    }

    fn ensure_supported_session(&self) -> Result<()> {
        if self.is_wayland {
            bail!("Wayland sessions are not supported. Please use an X11 session.");
        }

        Ok(())
    }

    fn build_keycodes(
        &self,
        conn: &x11rb::rust_connection::RustConnection,
    ) -> Result<HashMap<u32, u8>> {
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

        Ok(keycodes)
    }

    fn key_for_char(ch: char) -> Result<(u32, KeyButMask)> {
        if ch == ' ' {
            return Ok((u32::from(b' '), KeyButMask::default()));
        }

        if ch.is_ascii() {
            let keysym = ch as u32;
            let state = if ch.is_ascii_uppercase() {
                KeyButMask::SHIFT
            } else {
                KeyButMask::default()
            };
            return Ok((keysym, state));
        }

        bail!("Only ASCII text and spaces are supported for Linux targeted text input.")
    }
}

impl Platform for LinuxPlatform {
    fn check_permissions(&self) -> PermissionStatus {
        let screen_capture = screenshots::Screen::all().is_ok();
        let input_simulation = Enigo::new(&Settings::default()).is_ok();
        let cursor_tracking = !self.is_wayland;
        let accessibility = !self.is_wayland;

        PermissionStatus {
            screen_capture,
            accessibility,
            input_simulation,
            cursor_tracking,
        }
    }

    fn cursor_position(&self) -> Option<(i32, i32)> {
        if self.is_wayland {
            return None;
        }

        let (conn, screen_num) = self.x11_connection().ok()?;
        let root = conn.setup().roots.get(screen_num)?.root;
        let reply = conn.query_pointer(root).ok()?.reply().ok()?;
        Some((i32::from(reply.root_x), i32::from(reply.root_y)))
    }

    fn list_windows(&self) -> Result<Vec<WindowInfo>> {
        self.ensure_supported_session()?;
        let windows = x_win::get_open_windows().map_err(|error| anyhow!(error.to_string()))?;

        let mut results = Vec::with_capacity(windows.len());
        for window in windows {
            let width = window.position.width.max(0) as u32;
            let height = window.position.height.max(0) as u32;
            results.push(WindowInfo {
                id: window.id as u64,
                title: window.title,
                pid: window.info.process_id,
                x: window.position.x,
                y: window.position.y,
                width,
                height,
                is_minimized: false,
                is_maximized: window.position.is_full_screen,
            });
        }

        Ok(results)
    }

    fn window_at_point(&self, x: i32, y: i32) -> Result<Option<WindowInfo>> {
        self.ensure_supported_session()?;
        let mut candidates: Vec<WindowInfo> = self
            .list_windows()?
            .into_iter()
            .filter(|window| {
                let right = window.x.saturating_add(window.width as i32);
                let bottom = window.y.saturating_add(window.height as i32);
                x >= window.x && x < right && y >= window.y && y < bottom
            })
            .collect();

        candidates.sort_by_key(|window| window.width.saturating_mul(window.height));
        Ok(candidates.into_iter().next())
    }

    fn focused_window_id(&self) -> Result<Option<u64>> {
        self.ensure_supported_session()?;

        let (conn, _) = self.x11_connection()?;
        let reply = conn.get_input_focus()?.reply()?;
        let focus = reply.focus;
        if focus == x11rb::NONE {
            return Ok(None);
        }

        Ok(Some(focus as u64))
    }

    fn send_text_to_window(&self, id: u64, text: &str) -> Result<()> {
        self.ensure_supported_session()?;

        if text.is_empty() {
            return Ok(());
        }

        let target = u32::try_from(id).map_err(|_| anyhow!("Invalid window id"))?;
        let (conn, screen_num) = self.x11_connection()?;
        let root = conn
            .setup()
            .roots
            .get(screen_num)
            .ok_or_else(|| anyhow!("No X11 screen"))?
            .root;
        let keycodes = self.build_keycodes(&conn)?;

        for ch in text.chars() {
            let (keysym, state) = Self::key_for_char(ch)?;
            let keycode = *keycodes
                .get(&keysym)
                .ok_or_else(|| anyhow!("No keycode mapping found for character '{ch}'"))?;

            let press = KeyPressEvent {
                response_type: KEY_PRESS_EVENT,
                detail: keycode,
                sequence: 0,
                time: x11rb::CURRENT_TIME,
                root,
                event: target,
                child: x11rb::NONE,
                root_x: 0,
                root_y: 0,
                event_x: 0,
                event_y: 0,
                state,
                same_screen: true,
            };
            let release = KeyReleaseEvent {
                response_type: KEY_RELEASE_EVENT,
                detail: keycode,
                sequence: 0,
                time: x11rb::CURRENT_TIME,
                root,
                event: target,
                child: x11rb::NONE,
                root_x: 0,
                root_y: 0,
                event_x: 0,
                event_y: 0,
                state,
                same_screen: true,
            };

            conn.send_event(false, target, EventMask::KEY_PRESS, press)?;
            conn.send_event(false, target, EventMask::KEY_RELEASE, release)?;
        }

        conn.flush()?;
        Ok(())
    }

    fn send_paste_to_window(&self, id: u64) -> Result<()> {
        self.ensure_supported_session()?;

        let target = u32::try_from(id).map_err(|_| anyhow!("Invalid window id"))?;
        let (conn, screen_num) = self.x11_connection()?;
        let root = conn
            .setup()
            .roots
            .get(screen_num)
            .ok_or_else(|| anyhow!("No X11 screen"))?
            .root;
        let keycodes = self.build_keycodes(&conn)?;

        let ctrl_keysym = if keycodes.contains_key(&0xffe3) {
            0xffe3
        } else {
            0xffe4
        };
        let ctrl_keycode = *keycodes
            .get(&ctrl_keysym)
            .ok_or_else(|| anyhow!("No keycode mapping found for Control key"))?;
        let v_keycode = *keycodes
            .get(&(u32::from(b'v')))
            .ok_or_else(|| anyhow!("No keycode mapping found for 'v' key"))?;

        let ctrl_press = KeyPressEvent {
            response_type: KEY_PRESS_EVENT,
            detail: ctrl_keycode,
            sequence: 0,
            time: x11rb::CURRENT_TIME,
            root,
            event: target,
            child: x11rb::NONE,
            root_x: 0,
            root_y: 0,
            event_x: 0,
            event_y: 0,
            state: KeyButMask::default(),
            same_screen: true,
        };
        let v_press = KeyPressEvent {
            response_type: KEY_PRESS_EVENT,
            detail: v_keycode,
            sequence: 0,
            time: x11rb::CURRENT_TIME,
            root,
            event: target,
            child: x11rb::NONE,
            root_x: 0,
            root_y: 0,
            event_x: 0,
            event_y: 0,
            state: KeyButMask::CONTROL,
            same_screen: true,
        };
        let v_release = KeyReleaseEvent {
            response_type: KEY_RELEASE_EVENT,
            detail: v_keycode,
            sequence: 0,
            time: x11rb::CURRENT_TIME,
            root,
            event: target,
            child: x11rb::NONE,
            root_x: 0,
            root_y: 0,
            event_x: 0,
            event_y: 0,
            state: KeyButMask::CONTROL,
            same_screen: true,
        };
        let ctrl_release = KeyReleaseEvent {
            response_type: KEY_RELEASE_EVENT,
            detail: ctrl_keycode,
            sequence: 0,
            time: x11rb::CURRENT_TIME,
            root,
            event: target,
            child: x11rb::NONE,
            root_x: 0,
            root_y: 0,
            event_x: 0,
            event_y: 0,
            state: KeyButMask::CONTROL,
            same_screen: true,
        };

        conn.send_event(false, target, EventMask::KEY_PRESS, ctrl_press)?;
        conn.send_event(false, target, EventMask::KEY_PRESS, v_press)?;
        conn.send_event(false, target, EventMask::KEY_RELEASE, v_release)?;
        conn.send_event(false, target, EventMask::KEY_RELEASE, ctrl_release)?;
        conn.flush()?;
        Ok(())
    }

    fn focus_window(&self, id: u64) -> Result<()> {
        self.ensure_supported_session()?;

        let (conn, _) = self.x11_connection()?;
        let window_id = u32::try_from(id).map_err(|_| anyhow!("Invalid window id"))?;
        conn.set_input_focus(InputFocus::POINTER_ROOT, window_id, x11rb::CURRENT_TIME)?;
        conn.flush()?;
        Ok(())
    }

    fn set_position(&self, id: u64, x: i32, y: i32) -> Result<()> {
        self.ensure_supported_session()?;

        let (conn, _) = self.x11_connection()?;
        let window_id = u32::try_from(id).map_err(|_| anyhow!("Invalid window id"))?;
        let values = ConfigureWindowAux::new().x(x).y(y);
        conn.configure_window(window_id, &values)?;
        conn.flush()?;
        Ok(())
    }

    fn set_size(&self, id: u64, width: u32, height: u32) -> Result<()> {
        self.ensure_supported_session()?;

        let (conn, _) = self.x11_connection()?;
        let window_id = u32::try_from(id).map_err(|_| anyhow!("Invalid window id"))?;
        let values = ConfigureWindowAux::new().width(width).height(height);
        conn.configure_window(window_id, &values)?;
        conn.flush()?;
        Ok(())
    }

    fn set_always_on_top(&self, id: u64, enabled: bool) -> Result<()> {
        self.ensure_supported_session()?;
        let (conn, screen_num) = self.x11_connection()?;
        let root = conn
            .setup()
            .roots
            .get(screen_num)
            .ok_or_else(|| anyhow!("No X11 screen"))?
            .root;
        let window_id = u32::try_from(id).map_err(|_| anyhow!("Invalid window id"))?;
        let net_wm_state = conn.intern_atom(false, b"_NET_WM_STATE")?.reply()?.atom;
        let net_wm_state_above = conn
            .intern_atom(false, b"_NET_WM_STATE_ABOVE")?
            .reply()?
            .atom;
        let action: u32 = if enabled { 1 } else { 0 };
        // Build raw 32-byte ClientMessage for _NET_WM_STATE
        let mut ev = [0u8; 32];
        ev[0] = 33u8; // CLIENT_MESSAGE_EVENT
        ev[1] = 32; // format = 32-bit data
        // ev[2..4] = sequence, leave as 0
        ev[4..8].copy_from_slice(&window_id.to_ne_bytes());
        ev[8..12].copy_from_slice(&net_wm_state.to_ne_bytes());
        ev[12..16].copy_from_slice(&action.to_ne_bytes());
        ev[16..20].copy_from_slice(&net_wm_state_above.to_ne_bytes());
        ev[20..24].copy_from_slice(&0u32.to_ne_bytes());
        ev[24..28].copy_from_slice(&1u32.to_ne_bytes()); // source = normal application
        ev[28..32].copy_from_slice(&0u32.to_ne_bytes());
        let mask = EventMask::SUBSTRUCTURE_REDIRECT | EventMask::SUBSTRUCTURE_NOTIFY;
        let _ = conn.send_event(false, root, mask, ev)?;
        conn.flush()?;
        Ok(())
    }
}
