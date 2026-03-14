use std::time::Duration;

use enigo::{Button, Coordinate, Direction, Enigo, Keyboard, Mouse, Settings};
use iced::widget::image::{self as iced_image, Handle};
use iced::widget::{
    Column, Row, button, column, container, row, scrollable, text, text_input, tooltip,
};
use iced::{Element, Length, Size, Subscription, Task, window};

use crate::message::{Message, MouseButtonChoice};
use crate::platform::{PermissionStatus, Platform, WindowInfo, create_platform};

const WINDOW_WIDTH: f32 = 960.0;
const WINDOW_HEIGHT: f32 = 600.0;

pub struct AgentSpy {
    platform: Box<dyn Platform>,
    permissions: PermissionStatus,
    windows: Vec<WindowInfo>,
    selected_window_id: Option<u64>,
    search_query: String,
    track_mouse: bool,
    cursor_position: Option<(i32, i32)>,
    status: String,
    screenshot: Option<Handle>,
    enigo: Option<Enigo>,
    move_x: String,
    move_y: String,
    size_w: String,
    size_h: String,
    always_on_top: bool,
    input_text: String,
    click_x: String,
    click_y: String,
    click_button: MouseButtonChoice,
}

impl AgentSpy {
    fn new() -> Self {
        let platform = create_platform();
        let permissions = platform.check_permissions();
        let enigo = if permissions.input_simulation {
            Enigo::new(&Settings::default()).ok()
        } else {
            None
        };

        let mut app = Self {
            platform,
            permissions,
            windows: Vec::new(),
            selected_window_id: None,
            search_query: String::new(),
            track_mouse: false,
            cursor_position: None,
            status: String::new(),
            screenshot: None,
            enigo,
            move_x: String::new(),
            move_y: String::new(),
            size_w: String::new(),
            size_h: String::new(),
            always_on_top: false,
            input_text: String::new(),
            click_x: String::new(),
            click_y: String::new(),
            click_button: MouseButtonChoice::Left,
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
            missing.push("cursor tracking (Wayland limitation)");
        }

        if missing.is_empty() {
            self.status = "Ready.".to_string();
        } else {
            self.status = format!(
                "Startup checks: unavailable features: {}.",
                missing.join(", ")
            );
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
                self.status = format!("Failed to list windows: {error}");
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
        let mut action = button(text(label));
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

    fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::RefreshWindows => {
                self.refresh_windows();
            }
            Message::SearchChanged(value) => {
                self.search_query = value;
            }
            Message::SelectWindow(window_id) => {
                self.selected_window_id = Some(window_id);
                if let Some(window) = self.selected_window().cloned() {
                    self.selected_window_mutate_fields(&window);
                    self.status = format!("Selected window: {}", window.title);
                }
            }
            Message::ToggleTrackMouse => {
                if self.permissions.cursor_tracking {
                    self.track_mouse = !self.track_mouse;
                    self.status = if self.track_mouse {
                        "Tracking mouse enabled.".to_string()
                    } else {
                        "Tracking mouse disabled.".to_string()
                    };
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
                                self.status = format!("Window lookup failed: {error}");
                            }
                        }
                    }
                }
            }
            Message::FocusSelected => {
                if !self.permissions.accessibility {
                    return Task::none();
                }

                if let Some(window_id) = self.selected_window_id {
                    match self.platform.focus_window(window_id) {
                        Ok(()) => {
                            self.status = "Focused selected window.".to_string();
                        }
                        Err(error) => {
                            self.status = format!("Focus failed: {error}");
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
                            self.status = error;
                            return Task::none();
                        }
                    };
                    let y = match Self::parse_i32(&self.move_y, "Y") {
                        Ok(value) => value,
                        Err(error) => {
                            self.status = error;
                            return Task::none();
                        }
                    };

                    match self.platform.set_position(window_id, x, y) {
                        Ok(()) => {
                            self.status = "Window moved.".to_string();
                            self.refresh_windows();
                        }
                        Err(error) => self.status = format!("Move failed: {error}"),
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
                            self.status = error;
                            return Task::none();
                        }
                    };
                    let height = match Self::parse_u32(&self.size_h, "height") {
                        Ok(value) => value,
                        Err(error) => {
                            self.status = error;
                            return Task::none();
                        }
                    };

                    match self.platform.set_size(window_id, width, height) {
                        Ok(()) => {
                            self.status = "Window resized.".to_string();
                            self.refresh_windows();
                        }
                        Err(error) => self.status = format!("Resize failed: {error}"),
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
                            self.status = if self.always_on_top {
                                "Always-on-top enabled.".to_string()
                            } else {
                                "Always-on-top disabled.".to_string()
                            };
                        }
                        Err(error) => {
                            self.status = format!("Always-on-top failed: {error}");
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
                    self.status = "Select a window first.".to_string();
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
                            self.status = "Captured selected window region.".to_string();
                        }
                        Err(error) => {
                            self.status = format!("Window capture failed: {error}");
                        }
                    }
                } else {
                    self.status = "Selected window no longer available.".to_string();
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
                        self.status = "Captured screen.".to_string();
                    }
                    Err(error) => {
                        self.status = format!("Screen capture failed: {error}");
                    }
                }
            }
            Message::InputTextChanged(value) => self.input_text = value,
            Message::SendInputText => {
                if !self.permissions.input_simulation {
                    return Task::none();
                }

                if let Some(enigo) = self.enigo.as_mut() {
                    match enigo.text(&self.input_text) {
                        Ok(()) => {
                            self.status = "Sent keyboard text.".to_string();
                        }
                        Err(error) => {
                            self.status = format!("Sending text failed: {error}");
                        }
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
                        self.status = error;
                        return Task::none();
                    }
                };
                let y = match Self::parse_i32(&self.click_y, "click Y") {
                    Ok(value) => value,
                    Err(error) => {
                        self.status = error;
                        return Task::none();
                    }
                };

                if let Some(enigo) = self.enigo.as_mut() {
                    let button = match self.click_button {
                        MouseButtonChoice::Left => Button::Left,
                        MouseButtonChoice::Right => Button::Right,
                        MouseButtonChoice::Middle => Button::Middle,
                    };

                    let move_result = enigo.move_mouse(x, y, Coordinate::Abs);
                    let click_result = enigo.button(button, Direction::Click);
                    match (move_result, click_result) {
                        (Ok(()), Ok(())) => {
                            self.status = format!(
                                "Mouse click sent at ({x}, {y}) with {} button.",
                                self.click_button.label()
                            );
                        }
                        (Err(error), _) | (_, Err(error)) => {
                            self.status = format!("Mouse click failed: {error}");
                        }
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
                        self.status = error;
                        return Task::none();
                    }
                };
                let y = match Self::parse_i32(&self.click_y, "mouse Y") {
                    Ok(value) => value,
                    Err(error) => {
                        self.status = error;
                        return Task::none();
                    }
                };

                if let Some(enigo) = self.enigo.as_mut() {
                    match enigo.move_mouse(x, y, Coordinate::Abs) {
                        Ok(()) => {
                            self.status = format!("Moved mouse to ({x}, {y}).");
                        }
                        Err(error) => {
                            self.status = format!("Mouse move failed: {error}");
                        }
                    }
                }
            }
            Message::ClearStatus => {
                self.status.clear();
            }
        }

        Task::none()
    }

    fn subscription(&self) -> Subscription<Message> {
        if self.track_mouse && self.permissions.cursor_tracking {
            iced::time::every(Duration::from_millis(100)).map(|_| Message::Tick)
        } else {
            Subscription::none()
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
                let label = if window.title.is_empty() {
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
                column.push(
                    button(text(label))
                        .on_press(Message::SelectWindow(window.id))
                        .width(Length::Fill),
                )
            });

        let selected_info = if let Some(window) = self.selected_window() {
            column![
                text(format!("ID: {}", window.id)),
                text(format!("Title: {}", window.title)),
                text(format!("PID: {}", window.pid)),
                text(format!("Position: {}, {}", window.x, window.y)),
                text(format!("Size: {} x {}", window.width, window.height)),
                text(format!(
                    "State: minimized={} maximized={}",
                    window.is_minimized, window.is_maximized
                )),
            ]
        } else {
            column![text("No window selected")]
        };

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
                    "Cursor tracking is disabled on Wayland sessions.",
                ),
            ),
        ]
        .spacing(10);

        let search = text_input("Find window by name", &self.search_query)
            .on_input(Message::SearchChanged)
            .padding(8)
            .width(Length::Fill);

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
                        "Window move/resize/focus requires accessibility permission.",
                    ),
                ),
            ]
            .spacing(8),
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
                        "Window move/resize/focus requires accessibility permission.",
                    ),
                ),
            ]
            .spacing(8),
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
                        "Window move/resize/focus requires accessibility permission.",
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
                        "Window move/resize/focus requires accessibility permission.",
                    ),
                ),
            ]
            .spacing(8)
        ]
        .spacing(8);

        let screenshot_controls = row![
            Self::action_button(
                "Capture Selected Window",
                if self.permissions.screen_capture {
                    Some(Message::CaptureSelectedWindow)
                } else {
                    None
                },
                Self::required_permission_tooltip(
                    self.permissions.screen_capture,
                    "Screen capture permission is missing.",
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
                    "Screen capture permission is missing.",
                ),
            ),
        ]
        .spacing(8);

        let click_button_selector: Row<'_, Message> = MouseButtonChoice::ALL.iter().fold(
            row![text("Button:")].spacing(8),
            |row, button_choice| {
                let label = if *button_choice == self.click_button {
                    format!("[{}]", button_choice.label())
                } else {
                    button_choice.label().to_string()
                };
                row.push(button(text(label)).on_press(Message::SelectMouseButton(*button_choice)))
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
                        "Input simulation permission is missing.",
                    ),
                ),
            ]
            .spacing(8),
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
                        "Input simulation permission is missing.",
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
                        "Input simulation permission is missing.",
                    ),
                ),
            ]
            .spacing(8),
            click_button_selector,
        ]
        .spacing(8);

        let screenshot_panel: Element<'_, Message> = if let Some(handle) = &self.screenshot {
            iced_image::viewer(handle.clone())
                .width(Length::Fill)
                .height(Length::FillPortion(2))
                .into()
        } else {
            container(text("No screenshot captured yet"))
                .width(Length::Fill)
                .height(180)
                .center_x(Length::Fill)
                .center_y(Length::Fill)
                .into()
        };

        let left_panel = column![
            finder_controls,
            search,
            scrollable(window_list.spacing(4)).height(Length::Fill),
        ]
        .spacing(10)
        .width(Length::FillPortion(2));

        let right_panel = column![
            selected_info,
            move_resize_controls,
            screenshot_controls,
            input_controls,
            screenshot_panel,
        ]
        .spacing(10)
        .width(Length::FillPortion(3));

        let root = column![
            row![left_panel, right_panel]
                .spacing(14)
                .height(Length::Fill),
            row![
                text(format!(
                    "Cursor: {}",
                    self.cursor_position
                        .map(|(x, y)| format!("{}, {}", x, y))
                        .unwrap_or_else(|| "unknown".to_string())
                ))
                .size(14),
                text(&self.status).size(14),
                button(text("Clear")).on_press(Message::ClearStatus),
            ]
            .spacing(12),
        ]
        .padding(12)
        .spacing(10)
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
