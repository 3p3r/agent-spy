use std::time::{Duration, Instant};

use anyhow::anyhow;
use eframe::egui::{
    self, Align, Align2, Color32, FontId, Pos2, RichText, Sense, Stroke, StrokeKind, TextureHandle,
    Vec2, ViewportBuilder, ViewportClass, ViewportCommand, ViewportId,
};
use eframe::{App, CreationContext, Frame, NativeOptions};

use crate::core::{Core, MouseButtonArg};
use crate::message::{AppSection, MouseButtonChoice};
use crate::modes::{ModeType, Rect};
use crate::overlay::OverlayState;
use crate::platform::{PermissionStatus, Platform, WindowInfo, create_platform};

const WINDOW_WIDTH: f32 = 960.0;
const WINDOW_HEIGHT: f32 = 600.0;
const AUTO_REFRESH_INTERVAL: Duration = Duration::from_millis(2000);
const TRACK_INTERVAL: Duration = Duration::from_millis(100);

fn even_bg() -> Color32 {
    Color32::from_rgba_unmultiplied(0, 3, 4, 48)
}

fn odd_bg() -> Color32 {
    Color32::from_rgba_unmultiplied(0, 0, 51, 48)
}

fn even_border() -> Color32 {
    Color32::from_rgba_unmultiplied(0, 4, 8, 178)
}

fn odd_border() -> Color32 {
    Color32::from_rgba_unmultiplied(0, 0, 71, 178)
}

fn label_color() -> Color32 {
    Color32::from_rgba_unmultiplied(255, 255, 221, 222)
}

fn pointer_color() -> Color32 {
    Color32::from_rgba_unmultiplied(237, 33, 33, 222)
}

fn history_color() -> Color32 {
    Color32::from_rgba_unmultiplied(51, 51, 51, 153)
}

fn area_border_color() -> Color32 {
    Color32::from_rgba_unmultiplied(250, 214, 71, 242)
}

#[derive(Debug, Clone, Copy)]
enum StatusTone {
    Info,
    Success,
    Warning,
    Error,
}

pub struct AgentSpyApp {
    core: Core,
    platform: Box<dyn Platform>,
    permissions: PermissionStatus,
    windows: Vec<WindowInfo>,
    selected_window_id: Option<u64>,
    search_query: String,
    track_mouse: bool,
    cursor_position: Option<(i32, i32)>,
    status: String,
    status_tone: StatusTone,
    screenshot: Option<TextureHandle>,
    overlay_snapshot: Option<TextureHandle>,
    move_x: String,
    move_y: String,
    size_w: String,
    size_h: String,
    always_on_top: bool,
    input_text: String,
    allow_focus_swap_fallback: bool,
    send_via_clipboard_paste: bool,
    click_x: String,
    click_y: String,
    click_button: MouseButtonChoice,
    record_click_macro: bool,
    recorded_macro_steps: Vec<String>,
    recorded_macro_command: String,
    active_section: AppSection,
    overlay_mode: ModeType,
    overlay_enabled: bool,
    overlay_visible: bool,
    overlay_state: Option<OverlayState>,
    overlay_viewport_id: ViewportId,
    last_refresh: Instant,
    last_track: Instant,
}

impl AgentSpyApp {
    fn new(cc: &CreationContext<'_>) -> Self {
        cc.egui_ctx.set_visuals(egui::Visuals::dark());

        let core = Core::new();
        let platform = create_platform();
        let permissions = core.permissions().clone();
        let now = Instant::now();

        let mut app = Self {
            core,
            platform,
            permissions,
            windows: Vec::new(),
            selected_window_id: None,
            search_query: String::new(),
            track_mouse: false,
            cursor_position: None,
            status: String::new(),
            status_tone: StatusTone::Info,
            screenshot: None,
            overlay_snapshot: None,
            move_x: String::new(),
            move_y: String::new(),
            size_w: String::new(),
            size_h: String::new(),
            always_on_top: false,
            input_text: String::new(),
            allow_focus_swap_fallback: false,
            send_via_clipboard_paste: false,
            click_x: String::new(),
            click_y: String::new(),
            click_button: MouseButtonChoice::Left,
            record_click_macro: false,
            recorded_macro_steps: Vec::new(),
            recorded_macro_command: String::new(),
            active_section: AppSection::Overview,
            overlay_mode: ModeType::Bisect,
            overlay_enabled: false,
            overlay_visible: false,
            overlay_state: None,
            overlay_viewport_id: ViewportId::from_hash_of("overlay"),
            last_refresh: now - AUTO_REFRESH_INTERVAL,
            last_track: now - TRACK_INTERVAL,
        };

        app.refresh_windows();
        app.set_startup_status();
        app.cursor_position = app.platform.cursor_position();
        app
    }

    fn set_startup_status(&mut self) {
        let mut missing = Vec::new();

        if !self.permissions.screen_capture {
            missing.push("screen capture");
        }
        if !self.permissions.accessibility {
            missing.push("window manipulation");
        }
        if !self.permissions.input_simulation {
            missing.push("input simulation");
        }
        if !self.permissions.cursor_tracking {
            missing.push("cursor tracking");
        }

        if missing.is_empty() {
            self.set_status_success("Ready.");
        } else {
            self.set_status_warning(format!(
                "Startup checks: unavailable features: {}.",
                missing.join(", ")
            ));
        }
    }

    fn refresh_windows(&mut self) {
        match self.platform.list_windows() {
            Ok(windows) => {
                self.windows = windows;
                if let Some(selected) = self.selected_window_id
                    && !self.windows.iter().any(|window| window.id == selected)
                {
                    self.selected_window_id = None;
                }
            }
            Err(error) => self.set_status_error(format!("Failed to list windows: {error}")),
        }
    }

    fn tick(&mut self) {
        let now = Instant::now();
        self.cursor_position = self.platform.cursor_position();

        if self.track_mouse {
            if now.duration_since(self.last_track) >= TRACK_INTERVAL {
                self.last_track = now;
                if let Some((x, y)) = self.cursor_position {
                    match self.platform.window_at_point(x, y) {
                        Ok(Some(window)) => {
                            self.selected_window_id = Some(window.id);
                        }
                        Ok(None) => {}
                        Err(error) => {
                            self.set_status_error(format!("Window lookup failed: {error}"));
                        }
                    }
                }
            }
        } else if now.duration_since(self.last_refresh) >= AUTO_REFRESH_INTERVAL {
            self.last_refresh = now;
            self.refresh_windows();
        }
    }

    fn selected_window(&self) -> Option<&WindowInfo> {
        let selected_id = self.selected_window_id?;
        self.windows.iter().find(|window| window.id == selected_id)
    }

    fn selected_window_mutate_fields(&mut self, window: &WindowInfo) {
        self.move_x = window.x.to_string();
        self.move_y = window.y.to_string();
        self.size_w = window.width.to_string();
        self.size_h = window.height.to_string();
    }

    fn set_status_info(&mut self, value: impl Into<String>) {
        self.status = value.into();
        self.status_tone = StatusTone::Info;
    }

    fn set_status_success(&mut self, value: impl Into<String>) {
        self.status = value.into();
        self.status_tone = StatusTone::Success;
    }

    fn set_status_warning(&mut self, value: impl Into<String>) {
        self.status = value.into();
        self.status_tone = StatusTone::Warning;
    }

    fn set_status_error(&mut self, value: impl Into<String>) {
        self.status = value.into();
        self.status_tone = StatusTone::Error;
    }

    fn status_prefix(&self) -> &'static str {
        match self.status_tone {
            StatusTone::Info => "ℹ",
            StatusTone::Success => "✓",
            StatusTone::Warning => "⚠",
            StatusTone::Error => "✕",
        }
    }

    fn status_color(&self) -> Color32 {
        match self.status_tone {
            StatusTone::Info => Color32::LIGHT_BLUE,
            StatusTone::Success => Color32::LIGHT_GREEN,
            StatusTone::Warning => Color32::YELLOW,
            StatusTone::Error => Color32::LIGHT_RED,
        }
    }

    fn selected_mouse_button(&self) -> MouseButtonArg {
        match self.click_button {
            MouseButtonChoice::Left => MouseButtonArg::Left,
            MouseButtonChoice::Right => MouseButtonArg::Right,
            MouseButtonChoice::Middle => MouseButtonArg::Middle,
        }
    }

    fn selected_button_cli_name(&self) -> &'static str {
        match self.click_button {
            MouseButtonChoice::Left => "left",
            MouseButtonChoice::Right => "right",
            MouseButtonChoice::Middle => "middle",
        }
    }

    fn mode_cli_name(mode: ModeType) -> &'static str {
        match mode {
            ModeType::Bisect => "bisect",
            ModeType::SplitX => "split-x",
            ModeType::SplitY => "split-y",
            ModeType::Tile => "tile",
            ModeType::Floating => "floating",
        }
    }

    fn finalize_macro_recording(&mut self) {
        if !self.record_click_macro || self.recorded_macro_steps.is_empty() {
            return;
        }

        let chain = self.recorded_macro_steps.join(",");
        let mut command = format!("agent-spy --cli select-region --chain \"{chain}\"");
        if self.click_button != MouseButtonChoice::Left {
            command.push_str(&format!(" --button {}", self.selected_button_cli_name()));
        }

        self.recorded_macro_command = command;
        self.set_status_success("Recorded click macro command updated.");
    }

    fn parse_i32(value: &str, field_name: &str) -> Result<i32, String> {
        value
            .trim()
            .parse::<i32>()
            .map_err(|_| format!("Invalid {field_name}: {value}"))
    }

    fn parse_u32(value: &str, field_name: &str) -> Result<u32, String> {
        value
            .trim()
            .parse::<u32>()
            .map_err(|_| format!("Invalid {field_name}: {value}"))
    }

    fn open_overlay(&mut self, ctx: &egui::Context) {
        if self.overlay_visible {
            return;
        }

        let monitors = match screenshots::Screen::all() {
            Ok(monitors) => monitors,
            Err(error) => {
                self.set_status_error(format!("Failed to enumerate monitors: {error}"));
                return;
            }
        };

        let (cx, cy) = self
            .cursor_position
            .or_else(|| self.platform.cursor_position())
            .unwrap_or((0, 0));

        let Some(monitor) = monitors
            .iter()
            .find(|screen| {
                let di = screen.display_info;
                cx >= di.x
                    && cy >= di.y
                    && cx < di.x + di.width as i32
                    && cy < di.y + di.height as i32
            })
            .or_else(|| monitors.first())
        else {
            self.set_status_error("No monitors found.");
            return;
        };

        let di = monitor.display_info;
        let viewport = Rect {
            x: di.x,
            y: di.y,
            w: di.width,
            h: di.height,
        };

        let mode = self.overlay_mode;

        match monitor.capture() {
            Ok(image) => {
                let image = egui::ColorImage::from_rgba_unmultiplied(
                    [image.width() as usize, image.height() as usize],
                    &image,
                );
                if let Some(texture) = &mut self.overlay_snapshot {
                    texture.set(image, egui::TextureOptions::LINEAR);
                } else {
                    self.overlay_snapshot = Some(ctx.load_texture(
                        "overlay-snapshot",
                        image,
                        egui::TextureOptions::LINEAR,
                    ));
                }
            }
            Err(error) => {
                self.overlay_snapshot = None;
                self.set_status_warning(format!(
                    "Overlay opened, but snapshot capture failed: {error}"
                ));
            }
        }

        if self.record_click_macro {
            self.recorded_macro_steps.clear();
            self.recorded_macro_command.clear();
        }

        self.overlay_state = Some(OverlayState::new(mode, viewport));
        self.overlay_visible = true;
    }

    fn close_overlay(&mut self, ctx: &egui::Context) {
        if self.overlay_visible {
            ctx.send_viewport_cmd_to(self.overlay_viewport_id, ViewportCommand::Close);
        }
        self.overlay_visible = false;
        self.overlay_state = None;
        self.overlay_snapshot = None;
    }

    fn set_texture_from_capture(
        &mut self,
        ctx: &egui::Context,
        name: &str,
        width: u32,
        height: u32,
        rgba: Vec<u8>,
    ) {
        let image =
            egui::ColorImage::from_rgba_unmultiplied([width as usize, height as usize], &rgba);

        if let Some(texture) = &mut self.screenshot {
            texture.set(image, egui::TextureOptions::LINEAR);
        } else {
            self.screenshot = Some(ctx.load_texture(name, image, egui::TextureOptions::LINEAR));
        }
    }

    fn capture_selected_window(&mut self, ctx: &egui::Context) {
        if !self.permissions.screen_capture {
            return;
        }

        let Some(window) = self.selected_window().cloned() else {
            self.set_status_warning("Select a window first.");
            return;
        };

        let capture_result =
            screenshots::Screen::from_point(window.x, window.y).and_then(|screen| {
                let rel_x = window.x - screen.display_info.x;
                let rel_y = window.y - screen.display_info.y;
                screen.capture_area(rel_x, rel_y, window.width.max(1), window.height.max(1))
            });

        match capture_result {
            Ok(image) => {
                self.set_texture_from_capture(
                    ctx,
                    "selected-window-capture",
                    image.width(),
                    image.height(),
                    image.into_raw(),
                );
                self.set_status_success("Captured selected window region.");
            }
            Err(error) => self.set_status_error(format!("Window capture failed: {error}")),
        }
    }

    fn capture_screen(&mut self, ctx: &egui::Context) {
        if !self.permissions.screen_capture {
            return;
        }

        let capture_result = if let Some((x, y)) = self.cursor_position {
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

        match capture_result {
            Ok(image) => {
                self.set_texture_from_capture(
                    ctx,
                    "screen-capture",
                    image.width(),
                    image.height(),
                    image.into_raw(),
                );
                self.set_status_success("Captured screen.");
            }
            Err(error) => self.set_status_error(format!("Screen capture failed: {error}")),
        }
    }

    fn save_overlay_screenshot(&mut self) {
        let Some(state) = &self.overlay_state else {
            return;
        };
        let viewport = state.viewport;
        let capture = screenshots::Screen::from_point(viewport.x, viewport.y)
            .and_then(|screen| screen.capture());
        match capture {
            Ok(image) => {
                let (w, h) = (image.width() as usize, image.height() as usize);
                let rgba = image.into_raw();
                let img_data = arboard::ImageData {
                    width: w,
                    height: h,
                    bytes: rgba.into(),
                };
                match arboard::Clipboard::new().and_then(|mut cb| cb.set_image(img_data)) {
                    Ok(()) => self.set_status_success("Overlay screenshot copied to clipboard."),
                    Err(e) => self.set_status_error(format!("Clipboard copy failed: {e}")),
                }
            }
            Err(e) => self.set_status_error(format!("Overlay screenshot failed: {e}")),
        }
    }

    fn focus_selected(&mut self) {
        if !self.permissions.accessibility {
            return;
        }

        if let Some(window_id) = self.selected_window_id {
            match self.platform.focus_window(window_id) {
                Ok(()) => self.set_status_success("Focused selected window."),
                Err(error) => self.set_status_error(format!("Focus failed: {error}")),
            }
        }
    }

    fn apply_move(&mut self) {
        if !self.permissions.accessibility {
            return;
        }

        let Some(window_id) = self.selected_window_id else {
            return;
        };

        let x = match Self::parse_i32(&self.move_x, "X") {
            Ok(value) => value,
            Err(error) => {
                self.set_status_error(error);
                return;
            }
        };
        let y = match Self::parse_i32(&self.move_y, "Y") {
            Ok(value) => value,
            Err(error) => {
                self.set_status_error(error);
                return;
            }
        };

        match self.platform.set_position(window_id, x, y) {
            Ok(()) => {
                self.set_status_success("Window moved.");
                self.refresh_windows();
            }
            Err(error) => self.set_status_error(format!("Move failed: {error}")),
        }
    }

    fn apply_size(&mut self) {
        if !self.permissions.accessibility {
            return;
        }

        let Some(window_id) = self.selected_window_id else {
            return;
        };

        let width = match Self::parse_u32(&self.size_w, "width") {
            Ok(value) => value,
            Err(error) => {
                self.set_status_error(error);
                return;
            }
        };
        let height = match Self::parse_u32(&self.size_h, "height") {
            Ok(value) => value,
            Err(error) => {
                self.set_status_error(error);
                return;
            }
        };

        match self.platform.set_size(window_id, width, height) {
            Ok(()) => {
                self.set_status_success("Window resized.");
                self.refresh_windows();
            }
            Err(error) => self.set_status_error(format!("Resize failed: {error}")),
        }
    }

    fn toggle_always_on_top(&mut self) {
        if !self.permissions.accessibility {
            return;
        }

        let Some(window_id) = self.selected_window_id else {
            return;
        };

        self.always_on_top = !self.always_on_top;
        match self
            .platform
            .set_always_on_top(window_id, self.always_on_top)
        {
            Ok(()) => {
                if self.always_on_top {
                    self.set_status_success("Always-on-top enabled.");
                } else {
                    self.set_status_info("Always-on-top disabled.");
                }
            }
            Err(error) => {
                self.always_on_top = !self.always_on_top;
                self.set_status_error(format!("Always-on-top failed: {error}"));
            }
        }
    }

    fn send_input_text(&mut self) {
        if !self.permissions.input_simulation {
            return;
        }

        if self.input_text.trim().is_empty() {
            self.set_status_warning("Enter text before sending.");
            return;
        }

        match self.core.send_text(
            &self.input_text,
            self.selected_window_id,
            self.allow_focus_swap_fallback,
            self.send_via_clipboard_paste,
        ) {
            Ok(()) => self.set_status_success("Sent keyboard text."),
            Err(error) => self.set_status_error(format!("Sending text failed: {error}")),
        }
    }

    fn send_mouse_click(&mut self) {
        if !self.permissions.input_simulation {
            return;
        }

        let x = match Self::parse_i32(&self.click_x, "click X") {
            Ok(value) => value,
            Err(error) => {
                self.set_status_error(error);
                return;
            }
        };
        let y = match Self::parse_i32(&self.click_y, "click Y") {
            Ok(value) => value,
            Err(error) => {
                self.set_status_error(error);
                return;
            }
        };

        match self.core.click_mouse(x, y, self.selected_mouse_button()) {
            Ok(()) => self.set_status_success(format!(
                "Mouse click sent at ({x}, {y}) with {} button.",
                self.click_button.label()
            )),
            Err(error) => self.set_status_error(format!("Mouse click failed: {error}")),
        }
    }

    fn move_mouse_to_point(&mut self) {
        if !self.permissions.input_simulation {
            return;
        }

        let x = match Self::parse_i32(&self.click_x, "mouse X") {
            Ok(value) => value,
            Err(error) => {
                self.set_status_error(error);
                return;
            }
        };
        let y = match Self::parse_i32(&self.click_y, "mouse Y") {
            Ok(value) => value,
            Err(error) => {
                self.set_status_error(error);
                return;
            }
        };

        match self.core.move_mouse(x, y) {
            Ok(()) => self.set_status_success(format!("Moved mouse to ({x}, {y}).")),
            Err(error) => self.set_status_error(format!("Mouse move failed: {error}")),
        }
    }

    fn draw_main_ui(&mut self, ctx: &egui::Context) {
        egui::TopBottomPanel::bottom("status_panel").show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.label(
                    RichText::new(format!("{} {}", self.status_prefix(), self.status))
                        .color(self.status_color()),
                );
                ui.separator();
                ui.label(format!(
                    "Cursor: {}",
                    self.cursor_position
                        .map(|(x, y)| format!("{x}, {y}"))
                        .unwrap_or_else(|| "unknown".to_string())
                ));
                ui.with_layout(egui::Layout::right_to_left(Align::Center), |ui| {
                    if ui.button("Clear").clicked() {
                        self.status.clear();
                        self.status_tone = StatusTone::Info;
                    }
                });
            });
        });

        egui::SidePanel::left("window_browser")
            .default_width(320.0)
            .resizable(true)
            .show(ctx, |ui| {
                card(ui, "Window Browser", |ui| {
                    ui.horizontal(|ui| {
                        if ui.button("Refresh").clicked() {
                            self.refresh_windows();
                            self.set_status_info("Window list refreshed.");
                        }
                        let track_label = if self.track_mouse {
                            "Stop Tracking"
                        } else {
                            "Track Mouse"
                        };
                        if ui
                            .add_enabled(
                                self.permissions.cursor_tracking,
                                egui::Button::new(track_label),
                            )
                            .clicked()
                        {
                            self.track_mouse = !self.track_mouse;
                            if self.track_mouse {
                                self.set_status_success("Tracking mouse enabled.");
                            } else {
                                self.set_status_info("Tracking mouse disabled.");
                            }
                        }
                    });

                    ui.add(
                        egui::TextEdit::singleline(&mut self.search_query)
                            .hint_text("Find window by name"),
                    );

                    ui.horizontal_wrapped(|ui| {
                        permission_badge(ui, self.permissions.screen_capture, "screen");
                        permission_badge(ui, self.permissions.accessibility, "window");
                        permission_badge(ui, self.permissions.input_simulation, "input");
                        permission_badge(ui, self.permissions.cursor_tracking, "cursor");
                    });

                    let filtered_windows: Vec<WindowInfo> = if self.search_query.trim().is_empty() {
                        self.windows.clone()
                    } else {
                        let query = self.search_query.to_lowercase();
                        self.windows
                            .iter()
                            .filter(|window| window.title.to_lowercase().contains(&query))
                            .cloned()
                            .collect()
                    };

                    egui::ScrollArea::vertical().show(ui, |ui| {
                        for window in filtered_windows {
                            let selected = self.selected_window_id == Some(window.id);
                            let title = if window.title.is_empty() {
                                format!(
                                    "#{} [{}x{} @ {},{}]",
                                    window.id, window.width, window.height, window.x, window.y
                                )
                            } else {
                                format!(
                                    "{} [{}x{} @ {},{}]",
                                    window.title, window.width, window.height, window.x, window.y
                                )
                            };

                            if ui.selectable_label(selected, title).clicked() {
                                self.selected_window_id = Some(window.id);
                                self.selected_window_mutate_fields(&window);
                                self.set_status_info(format!("Selected window: {}", window.title));
                            }
                        }
                    });
                });
            });

        egui::CentralPanel::default().show(ctx, |ui| {
            card(ui, "Sections", |ui| {
                ui.horizontal(|ui| {
                    for section in AppSection::ALL {
                        ui.selectable_value(&mut self.active_section, section, section.label());
                    }
                });
            });

            ui.add_space(8.0);

            card(ui, "Selected Window", |ui| {
                if let Some(window) = self.selected_window() {
                    ui.label(format!("Title: {}", window.title));
                    ui.label(format!("ID: {} | PID: {}", window.id, window.pid));
                    ui.label(format!("Position: {}, {}", window.x, window.y));
                    ui.label(format!("Size: {} x {}", window.width, window.height));
                    ui.label(format!(
                        "State: minimized={} maximized={}",
                        window.is_minimized, window.is_maximized
                    ));
                } else {
                    ui.label("No window selected");
                }
            });

            ui.add_space(8.0);

            match self.active_section {
                AppSection::Overview => self.ui_overview(ui, ctx),
                AppSection::Window => self.ui_window(ui),
                AppSection::Capture => self.ui_capture(ui, ctx),
                AppSection::Input => self.ui_input(ui),
            }
        });
    }

    fn ui_overview(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        card(ui, "Overview", |ui| {
            ui.label("Quick actions and session capabilities");
            ui.horizontal_wrapped(|ui| {
                permission_badge(ui, self.permissions.screen_capture, "screen");
                permission_badge(ui, self.permissions.accessibility, "window");
                permission_badge(ui, self.permissions.input_simulation, "input");
                permission_badge(ui, self.permissions.cursor_tracking, "cursor");
            });
            ui.horizontal(|ui| {
                if ui.button("Refresh").clicked() {
                    self.refresh_windows();
                    self.set_status_info("Window list refreshed.");
                }
                let track_label = if self.track_mouse {
                    "Stop Tracking"
                } else {
                    "Track Mouse"
                };
                if ui
                    .add_enabled(
                        self.permissions.cursor_tracking,
                        egui::Button::new(track_label),
                    )
                    .clicked()
                {
                    self.track_mouse = !self.track_mouse;
                    if self.track_mouse {
                        self.set_status_success("Tracking mouse enabled.");
                    } else {
                        self.set_status_info("Tracking mouse disabled.");
                    }
                }
                if ui
                    .add_enabled(
                        self.permissions.screen_capture,
                        egui::Button::new("Capture Screen"),
                    )
                    .clicked()
                {
                    self.capture_screen(ctx);
                    self.active_section = AppSection::Capture;
                }
            });
        });
    }

    fn ui_window(&mut self, ui: &mut egui::Ui) {
        card(ui, "Window Actions", |ui| {
            let perm = self.permissions.accessibility;

            ui.horizontal(|ui| {
                ui.label("X");
                ui.add(egui::TextEdit::singleline(&mut self.move_x).desired_width(80.0));
                ui.label("Y");
                ui.add(egui::TextEdit::singleline(&mut self.move_y).desired_width(80.0));
                if ui.add_enabled(perm, egui::Button::new("Move")).clicked() {
                    self.apply_move();
                }
            });

            ui.horizontal(|ui| {
                ui.label("W");
                ui.add(egui::TextEdit::singleline(&mut self.size_w).desired_width(80.0));
                ui.label("H");
                ui.add(egui::TextEdit::singleline(&mut self.size_h).desired_width(80.0));
                if ui.add_enabled(perm, egui::Button::new("Resize")).clicked() {
                    self.apply_size();
                }
            });

            ui.horizontal(|ui| {
                if ui
                    .add_enabled(self.permissions.accessibility, egui::Button::new("Focus"))
                    .clicked()
                {
                    self.focus_selected();
                }
                if ui
                    .add_enabled(
                        self.permissions.accessibility,
                        egui::Button::new("Toggle Always-On-Top"),
                    )
                    .clicked()
                {
                    self.toggle_always_on_top();
                }
            });
        });
    }

    fn ui_capture(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        card(ui, "Capture", |ui| {
            ui.horizontal(|ui| {
                if ui
                    .add_enabled(
                        self.permissions.screen_capture,
                        egui::Button::new("Capture Selected Window"),
                    )
                    .clicked()
                {
                    self.capture_selected_window(ctx);
                }
                if ui
                    .add_enabled(
                        self.permissions.screen_capture,
                        egui::Button::new("Capture Screen"),
                    )
                    .clicked()
                {
                    self.capture_screen(ctx);
                }
            });
            ui.separator();
            if let Some(texture) = &self.screenshot {
                egui::ScrollArea::both().show(ui, |ui| {
                    let available = ui.available_size();
                    let size = fit_size(texture.size_vec2(), available);
                    ui.add(egui::Image::from_texture(texture).fit_to_exact_size(size));
                });
            } else {
                ui.label("No screenshot captured yet");
            }
        });
    }

    fn ui_input(&mut self, ui: &mut egui::Ui) {
        card(ui, "Input Simulation", |ui| {
            let perm = self.permissions.input_simulation;

            ui.horizontal(|ui| {
                ui.add(
                    egui::TextEdit::singleline(&mut self.input_text)
                        .hint_text("Text to send")
                        .desired_width(ui.available_width() - ui.spacing().item_spacing.x - 80.0),
                );
                if ui
                    .add_enabled(
                        perm,
                        egui::Button::new("Send Text").min_size(egui::vec2(80.0, 0.0)),
                    )
                    .clicked()
                {
                    self.send_input_text();
                }
            });

            ui.checkbox(&mut self.allow_focus_swap_fallback, "Use focus-swap mode");

            ui.checkbox(&mut self.send_via_clipboard_paste, "Use copy-paste mode");

            ui.horizontal(|ui| {
                ui.label("X");
                ui.add(egui::TextEdit::singleline(&mut self.click_x).desired_width(80.0));
                ui.label("Y");
                ui.add(egui::TextEdit::singleline(&mut self.click_y).desired_width(80.0));
                if ui
                    .add_enabled(perm, egui::Button::new("Move Mouse"))
                    .clicked()
                {
                    self.move_mouse_to_point();
                }
                if ui
                    .add_enabled(perm, egui::Button::new("Send Click"))
                    .clicked()
                {
                    self.send_mouse_click();
                }
            });
            ui.horizontal(|ui| {
                ui.label("Button");
                for button in MouseButtonChoice::ALL {
                    ui.selectable_value(&mut self.click_button, button, button.label());
                }
            });

            let toggled = ui
                .checkbox(&mut self.record_click_macro, "Record Click Macro")
                .changed();
            if toggled {
                if self.record_click_macro {
                    self.overlay_enabled = true;
                    self.recorded_macro_steps.clear();
                    self.recorded_macro_command.clear();
                    self.set_status_success(
                        "Record Click Macro enabled. Hold Alt to record traversal.",
                    );
                } else {
                    self.overlay_enabled = false;
                    self.set_status_info("Record Click Macro disabled.");
                }
            }

            let overlay_mode = self.overlay_mode.label();
            ui.label(format!(
                "Mode: {overlay_mode} (hold Alt to activate, right-click to cycle, left-click to refine)"
            ));

            ui.label("Equivalent command");
            let cmd_response = ui.add_sized(
                [ui.available_width(), 68.0],
                egui::TextEdit::multiline(&mut self.recorded_macro_command)
                    .interactive(true)
                    .desired_rows(3),
            );
            if cmd_response.clicked() && !self.recorded_macro_command.is_empty() {
                ui.ctx().copy_text(self.recorded_macro_command.clone());
                self.set_status_success("Copied command to clipboard.");
            }
        });
    }

    fn show_overlay_viewport(&mut self, ctx: &egui::Context) -> bool {
        let Some(snapshot) = self.overlay_state.clone() else {
            return false;
        };

        let viewport = snapshot.viewport;
        let overlay_texture = self.overlay_snapshot.clone();
        let mut overlay_alt_down = false;
        let mut cycle_mode = false;
        let mut selected_point = None;
        let mut save_screenshot = false;

        ctx.show_viewport_immediate(
            self.overlay_viewport_id,
            ViewportBuilder::default()
                .with_title("agent-spy overlay")
                .with_position(Pos2::new(viewport.x as f32, viewport.y as f32))
                .with_inner_size(Vec2::new(viewport.w as f32, viewport.h as f32))
                .with_min_inner_size(Vec2::new(viewport.w as f32, viewport.h as f32))
                .with_max_inner_size(Vec2::new(viewport.w as f32, viewport.h as f32))
                .with_resizable(false)
                .with_decorations(false)
                .with_transparent(false)
                .with_always_on_top()
                .with_drag_and_drop(false)
                .with_taskbar(false),
            |overlay_ctx, class| {
                if matches!(class, ViewportClass::Embedded) {
                    return;
                }

                overlay_alt_down = overlay_ctx.input(|i| i.modifiers.alt);
                overlay_ctx.request_repaint_after(Duration::from_millis(16));

                egui::CentralPanel::default()
                    .frame(egui::Frame::NONE.fill(Color32::TRANSPARENT))
                    .show(overlay_ctx, |ui| {
                        let (rect, response) =
                            ui.allocate_exact_size(ui.available_size(), Sense::click());
                        let painter = ui.painter_at(rect);

                        if let Some(texture) = overlay_texture.as_ref() {
                            painter.image(
                                texture.id(),
                                rect,
                                egui::Rect::from_min_max(Pos2::new(0.0, 0.0), Pos2::new(1.0, 1.0)),
                                Color32::WHITE,
                            );
                        }

                        paint_overlay(&painter, rect.min, &snapshot);

                        let (primary_released, secondary_released, latest_pos, s_pressed) =
                            overlay_ctx.input(|i| {
                                (
                                    i.pointer.button_released(egui::PointerButton::Primary),
                                    i.pointer.button_released(egui::PointerButton::Secondary),
                                    i.pointer.latest_pos(),
                                    i.key_pressed(egui::Key::S),
                                )
                            });

                        if s_pressed {
                            save_screenshot = true;
                        }

                        if secondary_released || response.secondary_clicked() {
                            cycle_mode = true;
                        }

                        if (primary_released || response.clicked())
                            && let Some(pos) =
                                latest_pos.or_else(|| response.interact_pointer_pos())
                        {
                            selected_point = Some((
                                viewport.x + (pos.x - rect.min.x).round() as i32,
                                viewport.y + (pos.y - rect.min.y).round() as i32,
                            ));
                        }
                    });
            },
        );

        if save_screenshot {
            self.save_overlay_screenshot();
        }

        if cycle_mode && let Some(state) = &mut self.overlay_state {
            let next = state.mode.next();
            state.set_mode(next);
            self.overlay_mode = next;
            self.set_status_info(format!("Overlay mode: {}.", next.label()));
        }

        let selected_result = if let Some((x, y)) = selected_point {
            if let Some(state) = &mut self.overlay_state {
                let mode = state.mode;
                if let Some(label) = state.select_at(x, y) {
                    Some((state.area, mode, label))
                } else {
                    None
                }
            } else {
                None
            }
        } else {
            None
        };

        if let Some((area, mode, label)) = selected_result {
            let (cx, cy) = area.center();
            self.click_x = cx.to_string();
            self.click_y = cy.to_string();
            if self.record_click_macro {
                self.recorded_macro_steps
                    .push(format!("{}:{}", Self::mode_cli_name(mode), label));
            }
            self.set_status_info(format!(
                "Overlay region updated to {}x{} at {},{}.",
                area.w, area.h, area.x, area.y
            ));
        }

        overlay_alt_down
    }
}

impl App for AgentSpyApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut Frame) {
        self.tick();

        let alt_down = ctx.input(|input| input.modifiers.alt);
        if self.overlay_enabled && alt_down && !self.overlay_visible {
            self.open_overlay(ctx);
        }

        let overlay_alt_down = if self.overlay_enabled && self.overlay_visible {
            self.show_overlay_viewport(ctx)
        } else {
            false
        };

        if self.overlay_visible && (!self.overlay_enabled || (!alt_down && !overlay_alt_down)) {
            self.close_overlay(ctx);
            self.finalize_macro_recording();
        }

        self.draw_main_ui(ctx);

        if self.track_mouse || self.overlay_enabled || self.overlay_visible {
            ctx.request_repaint_after(Duration::from_millis(16));
        } else {
            ctx.request_repaint_after(Duration::from_millis(250));
        }
    }

    fn clear_color(&self, _visuals: &egui::Visuals) -> [f32; 4] {
        Color32::TRANSPARENT.to_normalized_gamma_f32()
    }

    fn persist_egui_memory(&self) -> bool {
        false
    }
}

fn card(ui: &mut egui::Ui, title: &str, add_contents: impl FnOnce(&mut egui::Ui)) {
    egui::Frame::group(ui.style()).show(ui, |ui| {
        ui.heading(title);
        ui.add_space(6.0);
        add_contents(ui);
    });
}

fn permission_badge(ui: &mut egui::Ui, enabled: bool, label: &str) {
    let color = if enabled {
        Color32::LIGHT_GREEN
    } else {
        Color32::LIGHT_RED
    };
    ui.label(RichText::new(format!("{} {label}", if enabled { "✓" } else { "✕" })).color(color));
}

fn fit_size(original: Vec2, available: Vec2) -> Vec2 {
    if original.x <= 0.0 || original.y <= 0.0 {
        return available;
    }

    let scale = (available.x / original.x)
        .min(available.y / original.y)
        .min(1.0);

    Vec2::new(original.x * scale, original.y * scale)
}

fn paint_overlay(painter: &egui::Painter, origin: Pos2, state: &OverlayState) {
    for hist in &state.history {
        painter.rect_stroke(
            to_egui_rect(origin, state.viewport, *hist),
            0.0,
            Stroke::new(1.0, history_color()),
            StrokeKind::Middle,
        );
    }

    painter.rect_stroke(
        to_egui_rect(origin, state.viewport, state.area),
        0.0,
        Stroke::new(2.0, area_border_color()),
        StrokeKind::Middle,
    );

    for (index, sub) in state.subdivisions.iter().enumerate() {
        let rect = to_egui_rect(origin, state.viewport, sub.rect);
        let fill = if index % 2 == 0 { even_bg() } else { odd_bg() };
        let border = if index % 2 == 0 {
            even_border()
        } else {
            odd_border()
        };
        painter.rect_filled(rect, 0.0, fill);
        painter.rect_stroke(rect, 0.0, Stroke::new(1.0, border), StrokeKind::Middle);

        let size = (sub.rect.h as f32 * 0.5).clamp(10.0, 48.0);
        painter.text(
            rect.center(),
            Align2::CENTER_CENTER,
            &sub.label,
            FontId::proportional(size),
            label_color(),
        );
    }

    let (cx, cy) = state.area.center();
    let center = Pos2::new(
        origin.x + (cx - state.viewport.x) as f32,
        origin.y + (cy - state.viewport.y) as f32,
    );
    let half = 10.0;
    painter.line_segment(
        [
            Pos2::new(center.x - half, center.y),
            Pos2::new(center.x + half, center.y),
        ],
        Stroke::new(1.5, pointer_color()),
    );
    painter.line_segment(
        [
            Pos2::new(center.x, center.y - half),
            Pos2::new(center.x, center.y + half),
        ],
        Stroke::new(1.5, pointer_color()),
    );
}

fn to_egui_rect(origin: Pos2, viewport: Rect, rect: Rect) -> egui::Rect {
    egui::Rect::from_min_size(
        Pos2::new(
            origin.x + (rect.x - viewport.x) as f32,
            origin.y + (rect.y - viewport.y) as f32,
        ),
        Vec2::new(rect.w as f32, rect.h as f32),
    )
}

pub fn run() -> eframe::Result {
    let options = NativeOptions {
        viewport: ViewportBuilder::default()
            .with_inner_size(Vec2::new(WINDOW_WIDTH, WINDOW_HEIGHT))
            .with_min_inner_size(Vec2::new(WINDOW_WIDTH, WINDOW_HEIGHT))
            .with_max_inner_size(Vec2::new(WINDOW_WIDTH, WINDOW_HEIGHT))
            .with_resizable(false)
            .with_title("agent-spy"),
        ..Default::default()
    };

    eframe::run_native(
        "agent-spy",
        options,
        Box::new(|cc| Ok(Box::new(AgentSpyApp::new(cc)))),
    )
}
