use std::time::Duration;

use iced::widget::image::{self as iced_image, Handle};
use iced::widget::{
    Column, Row, button, column, container, row, scrollable, text, text_input, tooltip,
};
use iced::{Element, Length, Size, Subscription, Task, window};

use crate::core::{Core, MouseButtonArg};
use crate::message::{AppSection, Message, MouseButtonChoice};
use crate::platform::{PermissionStatus, Platform, WindowInfo, create_platform};

const WINDOW_WIDTH: f32 = 960.0;
const WINDOW_HEIGHT: f32 = 600.0;
const PADDING_ROOT: u16 = 16;
const PADDING_CARD: u16 = 12;
const SPACING_XS: u32 = 6;
const SPACING_SM: u32 = 10;
const SPACING_MD: u32 = 14;
const PANEL_TITLE_SIZE: u32 = 16;
const STATUS_TEXT_SIZE: u32 = 14;
const AUTO_REFRESH_INTERVAL_MS: u64 = 2000;
const TRACK_INTERVAL_MS: u64 = 100;
const INPUT_FOCUS_SETTLE_MS: u64 = 80;

#[derive(Debug, Clone, Copy)]
enum StatusTone {
    Info,
    Success,
    Warning,
    Error,
}

pub struct AgentSpy {
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
    screenshot: Option<Handle>,
    move_x: String,
    move_y: String,
    size_w: String,
    size_h: String,
    always_on_top: bool,
    input_text: String,
    click_x: String,
    click_y: String,
    click_button: MouseButtonChoice,
    active_section: AppSection,
}

impl AgentSpy {
    fn new() -> Self {
        let core = Core::new();
        let platform = create_platform();
        let permissions = core.permissions().clone();

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
            move_x: String::new(),
            move_y: String::new(),
            size_w: String::new(),
            size_h: String::new(),
            always_on_top: false,
            input_text: String::new(),
            click_x: String::new(),
            click_y: String::new(),
            click_button: MouseButtonChoice::Left,
            active_section: AppSection::Overview,
        };

        app.refresh_windows();
        app.set_startup_status();
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
            self.status = "Ready.".to_string();
            self.status_tone = StatusTone::Success;
        } else {
            self.status = format!(
                "Startup checks: unavailable features: {}.",
                missing.join(", ")
            );
            self.status_tone = StatusTone::Warning;
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
            Err(error) => {
                self.set_status_error(format!("Failed to list windows: {error}"));
            }
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

    fn required_permission_tooltip(enabled: bool, reason: &'static str) -> Option<&'static str> {
        if enabled { None } else { Some(reason) }
    }

    fn action_button<'a>(
        label: &'a str,
        on_press: Option<Message>,
        tooltip_text: Option<&'a str>,
    ) -> Element<'a, Message> {
        let mut action = button(text(label)).padding([8, 12]);
        if let Some(message) = on_press {
            action = action.on_press(message);
        }

        let content: Element<'a, Message> = action.into();
        if let Some(message) = tooltip_text {
            tooltip(content, text(message), tooltip::Position::Bottom).into()
        } else {
            content
        }
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

    fn selected_mouse_button(&self) -> MouseButtonArg {
        match self.click_button {
            MouseButtonChoice::Left => MouseButtonArg::Left,
            MouseButtonChoice::Right => MouseButtonArg::Right,
            MouseButtonChoice::Middle => MouseButtonArg::Middle,
        }
    }

    fn status_prefix(&self) -> &'static str {
        match self.status_tone {
            StatusTone::Info => "ℹ",
            StatusTone::Success => "✓",
            StatusTone::Warning => "⚠",
            StatusTone::Error => "✕",
        }
    }

    fn panel<'a>(title: &'a str, body: Element<'a, Message>) -> Element<'a, Message> {
        container(
            column![text(title).size(PANEL_TITLE_SIZE), body]
                .spacing(SPACING_SM)
                .width(Length::Fill),
        )
        .padding(PADDING_CARD)
        .style(iced::widget::container::rounded_box)
        .width(Length::Fill)
        .into()
    }

    fn permission_badges(&self) -> Row<'_, Message> {
        row![
            text(format!(
                "{} screen",
                if self.permissions.screen_capture {
                    "✓"
                } else {
                    "✕"
                }
            )),
            text(format!(
                "{} window",
                if self.permissions.accessibility {
                    "✓"
                } else {
                    "✕"
                }
            )),
            text(format!(
                "{} input",
                if self.permissions.input_simulation {
                    "✓"
                } else {
                    "✕"
                }
            )),
            text(format!(
                "{} cursor",
                if self.permissions.cursor_tracking {
                    "✓"
                } else {
                    "✕"
                }
            )),
        ]
        .spacing(SPACING_SM)
    }

    fn section_tabs(&self) -> Row<'_, Message> {
        AppSection::ALL
            .iter()
            .fold(row!().spacing(SPACING_XS), |row, section| {
                let label = if *section == self.active_section {
                    format!("● {}", section.label())
                } else {
                    section.label().to_string()
                };
                row.push(
                    button(text(label))
                        .on_press(Message::SelectSection(*section))
                        .padding([7, 10]),
                )
            })
    }

    fn selected_window_card(&self) -> Element<'_, Message> {
        let content: Element<'_, Message> = if let Some(window) = self.selected_window() {
            column![
                text(format!("Title: {}", window.title)),
                text(format!("ID: {} | PID: {}", window.id, window.pid)),
                text(format!("Position: {}, {}", window.x, window.y)),
                text(format!("Size: {} x {}", window.width, window.height)),
                text(format!(
                    "State: minimized={} maximized={}",
                    window.is_minimized, window.is_maximized
                )),
            ]
            .spacing(SPACING_XS)
            .into()
        } else {
            text("No window selected").into()
        };

        Self::panel("Selected Window", content)
    }

    fn window_section_content(&self) -> Element<'_, Message> {
        let move_resize_controls = column![
            row![
                text("X"),
                text_input("x", &self.move_x)
                    .on_input(Message::MoveXChanged)
                    .width(80),
                text("Y"),
                text_input("y", &self.move_y)
                    .on_input(Message::MoveYChanged)
                    .width(80),
                Self::action_button(
                    "Move",
                    if self.permissions.accessibility {
                        Some(Message::ApplyMove)
                    } else {
                        None
                    },
                    Self::required_permission_tooltip(
                        self.permissions.accessibility,
                        "Window operations are unavailable in this session.",
                    ),
                ),
            ]
            .spacing(SPACING_XS),
            row![
                text("W"),
                text_input("w", &self.size_w)
                    .on_input(Message::SizeWChanged)
                    .width(80),
                text("H"),
                text_input("h", &self.size_h)
                    .on_input(Message::SizeHChanged)
                    .width(80),
                Self::action_button(
                    "Resize",
                    if self.permissions.accessibility {
                        Some(Message::ApplySize)
                    } else {
                        None
                    },
                    Self::required_permission_tooltip(
                        self.permissions.accessibility,
                        "Window operations are unavailable in this session.",
                    ),
                ),
            ]
            .spacing(SPACING_XS),
            row![
                Self::action_button(
                    "Focus",
                    if self.permissions.accessibility {
                        Some(Message::FocusSelected)
                    } else {
                        None
                    },
                    Self::required_permission_tooltip(
                        self.permissions.accessibility,
                        "Window operations are unavailable in this session.",
                    ),
                ),
                Self::action_button(
                    "Toggle Always-On-Top",
                    if self.permissions.accessibility {
                        Some(Message::ToggleAlwaysOnTop)
                    } else {
                        None
                    },
                    Self::required_permission_tooltip(
                        self.permissions.accessibility,
                        "Window operations are unavailable in this session.",
                    ),
                ),
            ]
            .spacing(SPACING_SM),
        ]
        .spacing(SPACING_SM)
        .into();

        Self::panel("Window Actions", move_resize_controls)
    }

    fn capture_section_content(&self) -> Element<'_, Message> {
        let controls = row![
            Self::action_button(
                "Capture Selected Window",
                if self.permissions.screen_capture {
                    Some(Message::CaptureSelectedWindow)
                } else {
                    None
                },
                Self::required_permission_tooltip(
                    self.permissions.screen_capture,
                    "Screen capture is unavailable in this session.",
                ),
            ),
            Self::action_button(
                "Capture Screen",
                if self.permissions.screen_capture {
                    Some(Message::CaptureScreen)
                } else {
                    None
                },
                Self::required_permission_tooltip(
                    self.permissions.screen_capture,
                    "Screen capture is unavailable in this session.",
                ),
            ),
        ]
        .spacing(SPACING_SM);

        let screenshot_panel: Element<'_, Message> = if let Some(handle) = &self.screenshot {
            iced_image::viewer(handle.clone())
                .width(Length::Fill)
                .height(Length::Fill)
                .into()
        } else {
            container(text("No screenshot captured yet"))
                .width(Length::Fill)
                .height(220)
                .center_x(Length::Fill)
                .center_y(Length::Fill)
                .into()
        };

        Self::panel(
            "Capture",
            column![controls, screenshot_panel]
                .spacing(SPACING_SM)
                .width(Length::Fill)
                .into(),
        )
    }

    fn input_section_content(&self) -> Element<'_, Message> {
        let click_button_selector: Row<'_, Message> = MouseButtonChoice::ALL.iter().fold(
            row![text("Button")].spacing(SPACING_XS),
            |row, button_choice| {
                let label = if *button_choice == self.click_button {
                    format!("● {}", button_choice.label())
                } else {
                    button_choice.label().to_string()
                };
                row.push(
                    button(text(label))
                        .on_press(Message::SelectMouseButton(*button_choice))
                        .padding([7, 10]),
                )
            },
        );

        let input_controls: Column<'_, Message> = column![
            row![
                text_input("Text to send", &self.input_text)
                    .on_input(Message::InputTextChanged)
                    .padding(8)
                    .width(Length::Fill),
                Self::action_button(
                    "Send Text",
                    if self.permissions.input_simulation {
                        Some(Message::SendInputText)
                    } else {
                        None
                    },
                    Self::required_permission_tooltip(
                        self.permissions.input_simulation,
                        "Input simulation is unavailable in this session.",
                    ),
                ),
            ]
            .spacing(SPACING_SM),
            row![
                text("X"),
                text_input("x", &self.click_x)
                    .on_input(Message::ClickXChanged)
                    .width(100),
                text("Y"),
                text_input("y", &self.click_y)
                    .on_input(Message::ClickYChanged)
                    .width(100),
                Self::action_button(
                    "Move Mouse",
                    if self.permissions.input_simulation {
                        Some(Message::MoveMouseToPoint)
                    } else {
                        None
                    },
                    Self::required_permission_tooltip(
                        self.permissions.input_simulation,
                        "Input simulation is unavailable in this session.",
                    ),
                ),
                Self::action_button(
                    "Send Click",
                    if self.permissions.input_simulation {
                        Some(Message::SendMouseClick)
                    } else {
                        None
                    },
                    Self::required_permission_tooltip(
                        self.permissions.input_simulation,
                        "Input simulation is unavailable in this session.",
                    ),
                ),
            ]
            .spacing(SPACING_SM),
            click_button_selector,
        ]
        .spacing(SPACING_SM);

        Self::panel("Input Simulation", input_controls.into())
    }

    fn overview_section_content(&self) -> Element<'_, Message> {
        let quick_actions = row![
            Self::action_button("Refresh", Some(Message::RefreshWindows), None),
            Self::action_button(
                if self.track_mouse {
                    "Stop Tracking"
                } else {
                    "Track Mouse"
                },
                if self.permissions.cursor_tracking {
                    Some(Message::ToggleTrackMouse)
                } else {
                    None
                },
                Self::required_permission_tooltip(
                    self.permissions.cursor_tracking,
                    "Cursor tracking is unavailable in this session.",
                ),
            ),
            Self::action_button(
                "Capture Screen",
                if self.permissions.screen_capture {
                    Some(Message::CaptureScreen)
                } else {
                    None
                },
                Self::required_permission_tooltip(
                    self.permissions.screen_capture,
                    "Screen capture is unavailable in this session.",
                ),
            ),
        ]
        .spacing(SPACING_SM);

        Self::panel(
            "Overview",
            column![
                text("Quick actions and session capabilities"),
                self.permission_badges(),
                quick_actions,
            ]
            .spacing(SPACING_SM)
            .into(),
        )
    }

    fn section_content(&self) -> Element<'_, Message> {
        match self.active_section {
            AppSection::Overview => self.overview_section_content(),
            AppSection::Window => self.window_section_content(),
            AppSection::Capture => self.capture_section_content(),
            AppSection::Input => self.input_section_content(),
        }
    }

    fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::SelectSection(section) => {
                self.active_section = section;
            }
            Message::RefreshWindows => {
                self.refresh_windows();
                self.set_status_info("Window list refreshed.");
            }
            Message::SearchChanged(value) => {
                self.search_query = value;
            }
            Message::SelectWindow(window_id) => {
                self.selected_window_id = Some(window_id);
                if let Some(window) = self.selected_window().cloned() {
                    self.selected_window_mutate_fields(&window);
                    self.set_status_info(format!("Selected window: {}", window.title));
                }
            }
            Message::ToggleTrackMouse => {
                if self.permissions.cursor_tracking {
                    self.track_mouse = !self.track_mouse;
                    if self.track_mouse {
                        self.set_status_success("Tracking mouse enabled.");
                    } else {
                        self.set_status_info("Tracking mouse disabled.");
                    }
                }
            }
            Message::Tick => {
                if self.track_mouse {
                    self.cursor_position = self.platform.cursor_position();
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
                } else {
                    self.refresh_windows();
                }
            }
            Message::FocusSelected => {
                if !self.permissions.accessibility {
                    return Task::none();
                }

                if let Some(window_id) = self.selected_window_id {
                    match self.platform.focus_window(window_id) {
                        Ok(()) => {
                            self.set_status_success("Focused selected window.");
                        }
                        Err(error) => {
                            self.set_status_error(format!("Focus failed: {error}"));
                        }
                    }
                }
            }
            Message::MoveXChanged(value) => self.move_x = value,
            Message::MoveYChanged(value) => self.move_y = value,
            Message::ApplyMove => {
                if !self.permissions.accessibility {
                    return Task::none();
                }
                if let Some(window_id) = self.selected_window_id {
                    let x = match Self::parse_i32(&self.move_x, "X") {
                        Ok(value) => value,
                        Err(error) => {
                            self.set_status_error(error);
                            return Task::none();
                        }
                    };
                    let y = match Self::parse_i32(&self.move_y, "Y") {
                        Ok(value) => value,
                        Err(error) => {
                            self.set_status_error(error);
                            return Task::none();
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
            }
            Message::SizeWChanged(value) => self.size_w = value,
            Message::SizeHChanged(value) => self.size_h = value,
            Message::ApplySize => {
                if !self.permissions.accessibility {
                    return Task::none();
                }
                if let Some(window_id) = self.selected_window_id {
                    let width = match Self::parse_u32(&self.size_w, "width") {
                        Ok(value) => value,
                        Err(error) => {
                            self.set_status_error(error);
                            return Task::none();
                        }
                    };
                    let height = match Self::parse_u32(&self.size_h, "height") {
                        Ok(value) => value,
                        Err(error) => {
                            self.set_status_error(error);
                            return Task::none();
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
            }
            Message::ToggleAlwaysOnTop => {
                if !self.permissions.accessibility {
                    return Task::none();
                }
                if let Some(window_id) = self.selected_window_id {
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
                            self.set_status_error(format!("Always-on-top failed: {error}"));
                            self.always_on_top = !self.always_on_top;
                        }
                    }
                }
            }
            Message::CaptureSelectedWindow => {
                if !self.permissions.screen_capture {
                    return Task::none();
                }

                if self.selected_window_id.is_none() {
                    self.set_status_warning("Select a window first.");
                    return Task::none();
                }

                if let Some(window) = self.selected_window() {
                    let width = window.width.max(1);
                    let height = window.height.max(1);

                    let capture_result = screenshots::Screen::from_point(window.x, window.y)
                        .and_then(|screen| {
                            let rel_x = window.x - screen.display_info.x;
                            let rel_y = window.y - screen.display_info.y;
                            screen.capture_area(rel_x, rel_y, width, height)
                        });

                    match capture_result {
                        Ok(image) => {
                            let width = image.width();
                            let height = image.height();
                            self.screenshot = Some(iced_image::Handle::from_rgba(
                                width,
                                height,
                                image.into_raw(),
                            ));
                            self.set_status_success("Captured selected window region.");
                        }
                        Err(error) => {
                            self.set_status_error(format!("Window capture failed: {error}"));
                        }
                    }
                } else {
                    self.set_status_warning("Selected window no longer available.");
                }
            }
            Message::CaptureScreen => {
                if !self.permissions.screen_capture {
                    return Task::none();
                }

                let capture_result = if let Some((x, y)) = self.cursor_position {
                    screenshots::Screen::from_point(x, y)
                        .and_then(|screen| screen.capture())
                        .or_else(|_| {
                            screenshots::Screen::all().and_then(|screens| {
                                screens
                                    .into_iter()
                                    .next()
                                    .ok_or_else(|| anyhow::anyhow!("No monitor found"))
                                    .and_then(|screen| screen.capture())
                            })
                        })
                } else {
                    screenshots::Screen::all().and_then(|screens| {
                        screens
                            .into_iter()
                            .next()
                            .ok_or_else(|| anyhow::anyhow!("No monitor found"))
                            .and_then(|screen| screen.capture())
                    })
                };

                match capture_result {
                    Ok(image) => {
                        let width = image.width();
                        let height = image.height();
                        self.screenshot = Some(iced_image::Handle::from_rgba(
                            width,
                            height,
                            image.into_raw(),
                        ));
                        self.set_status_success("Captured screen.");
                    }
                    Err(error) => {
                        self.set_status_error(format!("Screen capture failed: {error}"));
                    }
                }
            }
            Message::InputTextChanged(value) => self.input_text = value,
            Message::SendInputText => {
                if !self.permissions.input_simulation {
                    return Task::none();
                }

                if self.input_text.trim().is_empty() {
                    self.set_status_warning("Enter text before sending.");
                    return Task::none();
                }

                if let Some(window_id) = self.selected_window_id
                    && self.permissions.accessibility
                {
                    if let Err(error) = self.platform.focus_window(window_id) {
                        self.set_status_error(format!("Focus failed before text input: {error}"));
                        return Task::none();
                    }
                    std::thread::sleep(Duration::from_millis(INPUT_FOCUS_SETTLE_MS));
                }

                match self.core.send_text(&self.input_text) {
                    Ok(()) => {
                        self.set_status_success("Sent keyboard text.");
                    }
                    Err(error) => {
                        self.set_status_error(format!("Sending text failed: {error}"));
                    }
                }
            }
            Message::ClickXChanged(value) => self.click_x = value,
            Message::ClickYChanged(value) => self.click_y = value,
            Message::SelectMouseButton(button_choice) => self.click_button = button_choice,
            Message::SendMouseClick => {
                if !self.permissions.input_simulation {
                    return Task::none();
                }

                let x = match Self::parse_i32(&self.click_x, "click X") {
                    Ok(value) => value,
                    Err(error) => {
                        self.set_status_error(error);
                        return Task::none();
                    }
                };
                let y = match Self::parse_i32(&self.click_y, "click Y") {
                    Ok(value) => value,
                    Err(error) => {
                        self.set_status_error(error);
                        return Task::none();
                    }
                };

                match self.core.click_mouse(x, y, self.selected_mouse_button()) {
                    Ok(()) => {
                        self.set_status_success(format!(
                            "Mouse click sent at ({x}, {y}) with {} button.",
                            self.click_button.label()
                        ));
                    }
                    Err(error) => {
                        self.set_status_error(format!("Mouse click failed: {error}"));
                    }
                }
            }
            Message::MoveMouseToPoint => {
                if !self.permissions.input_simulation {
                    return Task::none();
                }

                let x = match Self::parse_i32(&self.click_x, "mouse X") {
                    Ok(value) => value,
                    Err(error) => {
                        self.set_status_error(error);
                        return Task::none();
                    }
                };
                let y = match Self::parse_i32(&self.click_y, "mouse Y") {
                    Ok(value) => value,
                    Err(error) => {
                        self.set_status_error(error);
                        return Task::none();
                    }
                };

                match self.core.move_mouse(x, y) {
                    Ok(()) => {
                        self.set_status_success(format!("Moved mouse to ({x}, {y})."));
                    }
                    Err(error) => {
                        self.set_status_error(format!("Mouse move failed: {error}"));
                    }
                }
            }
            Message::ClearStatus => {
                self.status.clear();
                self.status_tone = StatusTone::Info;
            }
        }

        Task::none()
    }

    fn subscription(&self) -> Subscription<Message> {
        if self.track_mouse && self.permissions.cursor_tracking {
            iced::time::every(Duration::from_millis(TRACK_INTERVAL_MS)).map(|_| Message::Tick)
        } else {
            iced::time::every(Duration::from_millis(AUTO_REFRESH_INTERVAL_MS))
                .map(|_| Message::Tick)
        }
    }

    fn view(&self) -> Element<'_, Message> {
        let filtered_windows: Vec<&WindowInfo> = if self.search_query.trim().is_empty() {
            self.windows.iter().collect()
        } else {
            let query = self.search_query.to_lowercase();
            self.windows
                .iter()
                .filter(|window| window.title.to_lowercase().contains(&query))
                .collect()
        };

        let window_list = filtered_windows
            .into_iter()
            .fold(column![], |column, window| {
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

                let label = if selected {
                    format!("● {title}")
                } else {
                    format!("  {title}")
                };

                column.push(
                    button(text(label))
                        .on_press(Message::SelectWindow(window.id))
                        .width(Length::Fill)
                        .padding([8, 10]),
                )
            });

        let finder_controls = row![
            Self::action_button("Refresh", Some(Message::RefreshWindows), None),
            Self::action_button(
                if self.track_mouse {
                    "Stop Tracking"
                } else {
                    "Track Mouse"
                },
                if self.permissions.cursor_tracking {
                    Some(Message::ToggleTrackMouse)
                } else {
                    None
                },
                Self::required_permission_tooltip(
                    self.permissions.cursor_tracking,
                    "Cursor tracking is unavailable in this session.",
                ),
            ),
        ]
        .spacing(SPACING_SM);

        let search = text_input("Find window by name", &self.search_query)
            .on_input(Message::SearchChanged)
            .padding(8)
            .width(Length::Fill);

        let left_panel = Self::panel(
            "Window Browser",
            column![
                finder_controls,
                search,
                self.permission_badges(),
                scrollable(window_list.spacing(SPACING_XS)).height(Length::Fill),
            ]
            .spacing(SPACING_SM)
            .into(),
        );

        let right_panel = column![
            Self::panel("Sections", self.section_tabs().into()),
            self.selected_window_card(),
            self.section_content(),
        ]
        .spacing(SPACING_SM)
        .width(Length::FillPortion(3));

        let status_panel = Self::panel(
            "Status",
            row![
                text(format!("{} {}", self.status_prefix(), self.status))
                    .size(STATUS_TEXT_SIZE)
                    .width(Length::FillPortion(3)),
                text(format!(
                    "Cursor: {}",
                    self.cursor_position
                        .map(|(x, y)| format!("{}, {}", x, y))
                        .unwrap_or_else(|| "unknown".to_string())
                ))
                .size(STATUS_TEXT_SIZE)
                .width(Length::FillPortion(2)),
                button(text("Clear"))
                    .on_press(Message::ClearStatus)
                    .padding([7, 10]),
            ]
            .spacing(SPACING_SM)
            .align_y(iced::Alignment::Center)
            .into(),
        );

        let root = column![
            row![
                container(left_panel).width(Length::FillPortion(2)),
                right_panel
            ]
            .spacing(SPACING_MD)
            .height(Length::Fill),
            status_panel,
        ]
        .padding(PADDING_ROOT)
        .spacing(SPACING_SM)
        .height(Length::Fill);

        container(root)
            .width(Length::Fill)
            .height(Length::Fill)
            .into()
    }
}

pub fn run() -> iced::Result {
    iced::application(AgentSpy::new, AgentSpy::update, AgentSpy::view)
        .title("agent-spy")
        .window(window::Settings {
            size: Size::new(WINDOW_WIDTH, WINDOW_HEIGHT),
            min_size: Some(Size::new(WINDOW_WIDTH, WINDOW_HEIGHT)),
            max_size: Some(Size::new(WINDOW_WIDTH, WINDOW_HEIGHT)),
            resizable: false,
            ..window::Settings::default()
        })
        .subscription(AgentSpy::subscription)
        .run()
}
