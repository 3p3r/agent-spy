use std::path::PathBuf;

use anyhow::{Result, anyhow, bail};
use clap::{CommandFactory, Parser, Subcommand, ValueEnum, error::ErrorKind};

use crate::core::{Core, KeyArg, ModifierKey, MouseButtonArg, ScrollAxisArg};
use crate::platform::WindowInfo;

pub fn run_from_args(args: Vec<String>) -> i32 {
    match run(args) {
        Ok(output) => {
            if !output.is_empty() {
                println!("{output}");
            }
            0
        }
        Err(error) => {
            eprintln!("Error: {error}");
            1
        }
    }
}

fn run(args: Vec<String>) -> Result<String> {
    let parse_args = std::iter::once("agent-spy".to_string())
        .chain(args)
        .collect::<Vec<_>>();
    let cli_args = match CliArgs::try_parse_from(parse_args) {
        Ok(args) => args,
        Err(error) => {
            return match error.kind() {
                ErrorKind::DisplayHelp | ErrorKind::DisplayVersion => Ok(error.to_string()),
                _ => Err(anyhow!(error.to_string())),
            };
        }
    };

    if !cli_args.cli {
        bail!("Missing --cli flag")
    }

    let Some(command) = cli_args.command else {
        return Ok(help_text());
    };

    let mut core = Core::new();

    match command {
        CliCommand::ListWindows { search } => {
            let windows = core.list_windows(search.as_deref())?;
            Ok(format_windows(&windows))
        }
        CliCommand::WindowInfo { id } => {
            let window = core.window_info(id)?;
            Ok(format_window(&window))
        }
        CliCommand::WindowAtPoint { x, y } => match core.window_at_point(x, y)? {
            Some(window) => Ok(format_window(&window)),
            None => Ok("No window found at that point.".to_string()),
        },
        CliCommand::CursorPosition => {
            let (x, y) = core.cursor_position()?;
            Ok(format!("{x} {y}"))
        }
        CliCommand::Focus { id } => {
            core.focus_window(id)?;
            Ok(format!("Focused window {id}."))
        }
        CliCommand::Move { id, x, y } => {
            core.move_window(id, x, y)?;
            Ok(format!("Moved window {id} to ({x}, {y})."))
        }
        CliCommand::Resize { id, width, height } => {
            core.resize_window(id, width, height)?;
            Ok(format!("Resized window {id} to {width}x{height}."))
        }
        CliCommand::AlwaysOnTop { id, state } => {
            let enabled = matches!(state, OnOff::On);
            core.set_always_on_top(id, enabled)?;
            Ok(if enabled {
                format!("Always-on-top enabled for window {id}.")
            } else {
                format!("Always-on-top disabled for window {id}.")
            })
        }
        CliCommand::CaptureScreen { output } => {
            core.capture_screen_to_file(&output)?;
            Ok(format!("Captured screen to {}.", output.display()))
        }
        CliCommand::CaptureWindow { id, output } => {
            core.capture_window_to_file(id, &output)?;
            Ok(format!("Captured window {id} to {}.", output.display()))
        }
        CliCommand::SendText { text } => {
            core.send_text(&text)?;
            Ok("Sent keyboard text.".to_string())
        }
        CliCommand::MoveMouse { x, y } => {
            core.move_mouse(x, y)?;
            Ok(format!("Moved mouse to ({x}, {y})."))
        }
        CliCommand::Click { x, y, button } => {
            core.click_mouse(x, y, button.into())?;
            Ok(format!("Clicked at ({x}, {y})."))
        }
        CliCommand::MouseDown { x, y, button } => {
            core.mouse_down(x, y, button.into())?;
            Ok(format!(
                "Pressed {} mouse button at ({x}, {y}).",
                button.label()
            ))
        }
        CliCommand::MouseUp { x, y, button } => {
            core.mouse_up(x, y, button.into())?;
            Ok(format!(
                "Released {} mouse button at ({x}, {y}).",
                button.label()
            ))
        }
        CliCommand::Drag {
            start_x,
            start_y,
            end_x,
            end_y,
            button,
        } => {
            core.drag_mouse(start_x, start_y, end_x, end_y, button.into())?;
            Ok(format!(
                "Dragged {} mouse button from ({start_x}, {start_y}) to ({end_x}, {end_y}).",
                button.label()
            ))
        }
        CliCommand::Scroll { amount, axis } => {
            core.scroll(amount, axis.into())?;
            Ok(format!("Scrolled {} {}.", axis.label(), amount))
        }
        CliCommand::KeyDown { key } => {
            let key = KeyArg::parse(&key)?;
            core.key_down(key)?;
            Ok("Pressed key.".to_string())
        }
        CliCommand::KeyUp { key } => {
            let key = KeyArg::parse(&key)?;
            core.key_up(key)?;
            Ok("Released key.".to_string())
        }
        CliCommand::KeyTap { key, modifiers } => {
            let key = KeyArg::parse(&key)?;
            let modifiers = modifiers
                .into_iter()
                .map(ModifierKey::from)
                .collect::<Vec<_>>();
            core.key_tap(key, &modifiers)?;
            Ok("Tapped key.".to_string())
        }
        CliCommand::CheckPermissions => {
            let permissions = core.permissions();
            let modifiers = core.modifier_state();
            let mut output = format!(
                "screen_capture={}\naccessibility={}\ninput_simulation={}\ncursor_tracking={}\ninput_backend={}\nmod_shift={}\nmod_control={}\nmod_alt={}\nmod_meta={}",
                permissions.screen_capture,
                permissions.accessibility,
                permissions.input_simulation,
                permissions.cursor_tracking,
                core.input_backend_name(),
                modifiers.shift,
                modifiers.control,
                modifiers.alt,
                modifiers.meta
            );

            let supported = permissions.screen_capture
                || permissions.accessibility
                || permissions.input_simulation
                || permissions.cursor_tracking;
            output.push_str(&format!("\nsession_supported={supported}"));

            Ok(output)
        }
        CliCommand::Version => Ok(format!("agent-spy {}", env!("CARGO_PKG_VERSION"))),
    }
}

#[derive(Debug, Parser)]
#[command(name = "agent-spy", disable_help_subcommand = true)]
struct CliArgs {
    #[arg(long)]
    cli: bool,
    #[command(subcommand)]
    command: Option<CliCommand>,
}

#[derive(Debug, Subcommand)]
enum CliCommand {
    ListWindows {
        #[arg(long)]
        search: Option<String>,
    },
    WindowInfo {
        id: u64,
    },
    WindowAtPoint {
        x: i32,
        y: i32,
    },
    CursorPosition,
    Focus {
        id: u64,
    },
    Move {
        id: u64,
        x: i32,
        y: i32,
    },
    Resize {
        id: u64,
        width: u32,
        height: u32,
    },
    AlwaysOnTop {
        id: u64,
        #[arg(value_enum)]
        state: OnOff,
    },
    CaptureScreen {
        #[arg(long)]
        output: PathBuf,
    },
    CaptureWindow {
        id: u64,
        #[arg(long)]
        output: PathBuf,
    },
    SendText {
        text: String,
    },
    MoveMouse {
        x: i32,
        y: i32,
    },
    Click {
        x: i32,
        y: i32,
        #[arg(long, value_enum, default_value_t = ButtonChoice::Left)]
        button: ButtonChoice,
    },
    MouseDown {
        x: i32,
        y: i32,
        #[arg(long, value_enum, default_value_t = ButtonChoice::Left)]
        button: ButtonChoice,
    },
    MouseUp {
        x: i32,
        y: i32,
        #[arg(long, value_enum, default_value_t = ButtonChoice::Left)]
        button: ButtonChoice,
    },
    Drag {
        start_x: i32,
        start_y: i32,
        end_x: i32,
        end_y: i32,
        #[arg(long, value_enum, default_value_t = ButtonChoice::Left)]
        button: ButtonChoice,
    },
    Scroll {
        amount: i32,
        #[arg(long, value_enum, default_value_t = ScrollAxisChoice::Vertical)]
        axis: ScrollAxisChoice,
    },
    KeyDown {
        key: String,
    },
    KeyUp {
        key: String,
    },
    KeyTap {
        key: String,
        #[arg(long = "mod", value_enum)]
        modifiers: Vec<ModifierChoice>,
    },
    CheckPermissions,
    Version,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
enum OnOff {
    On,
    Off,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
enum ButtonChoice {
    Left,
    Right,
    Middle,
}

impl ButtonChoice {
    fn label(self) -> &'static str {
        match self {
            Self::Left => "left",
            Self::Right => "right",
            Self::Middle => "middle",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
enum ModifierChoice {
    Shift,
    Control,
    Alt,
    Meta,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
enum ScrollAxisChoice {
    Vertical,
    Horizontal,
}

impl ScrollAxisChoice {
    fn label(self) -> &'static str {
        match self {
            Self::Vertical => "vertically",
            Self::Horizontal => "horizontally",
        }
    }
}

impl From<ButtonChoice> for MouseButtonArg {
    fn from(value: ButtonChoice) -> Self {
        match value {
            ButtonChoice::Left => Self::Left,
            ButtonChoice::Right => Self::Right,
            ButtonChoice::Middle => Self::Middle,
        }
    }
}

impl From<ModifierChoice> for ModifierKey {
    fn from(value: ModifierChoice) -> Self {
        match value {
            ModifierChoice::Shift => Self::Shift,
            ModifierChoice::Control => Self::Control,
            ModifierChoice::Alt => Self::Alt,
            ModifierChoice::Meta => Self::Meta,
        }
    }
}

impl From<ScrollAxisChoice> for ScrollAxisArg {
    fn from(value: ScrollAxisChoice) -> Self {
        match value {
            ScrollAxisChoice::Vertical => Self::Vertical,
            ScrollAxisChoice::Horizontal => Self::Horizontal,
        }
    }
}

fn help_text() -> String {
    let mut command = CliArgs::command();
    let mut output = Vec::new();
    command.write_long_help(&mut output).ok();
    String::from_utf8_lossy(&output).to_string()
}

fn format_windows(windows: &[WindowInfo]) -> String {
    if windows.is_empty() {
        return "No windows found.".to_string();
    }

    windows
        .iter()
        .map(|window| {
            format!(
                "id={} pid={} title=\"{}\" pos=({}, {}) size={}x{} minimized={} maximized={}",
                window.id,
                window.pid,
                window.title.replace('"', "\\\""),
                window.x,
                window.y,
                window.width,
                window.height,
                window.is_minimized,
                window.is_maximized
            )
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn format_window(window: &WindowInfo) -> String {
    format!(
        "id={}\npid={}\ntitle={}\nx={}\ny={}\nwidth={}\nheight={}\nminimized={}\nmaximized={}",
        window.id,
        window.pid,
        window.title,
        window.x,
        window.y,
        window.width,
        window.height,
        window.is_minimized,
        window.is_maximized
    )
}

#[cfg(test)]
mod tests {
    use super::{ButtonChoice, CliArgs, CliCommand, ModifierChoice, OnOff, ScrollAxisChoice};
    use clap::Parser;

    #[test]
    fn parse_list_windows_command() {
        let args =
            CliArgs::try_parse_from(["agent-spy", "--cli", "list-windows", "--search", "firefox"])
                .unwrap();

        match args.command.unwrap() {
            CliCommand::ListWindows { search } => {
                assert_eq!(search.as_deref(), Some("firefox"));
            }
            _ => panic!("expected list-windows command"),
        }
    }

    #[test]
    fn parse_always_on_top_command() {
        let args =
            CliArgs::try_parse_from(["agent-spy", "--cli", "always-on-top", "42", "on"]).unwrap();

        match args.command.unwrap() {
            CliCommand::AlwaysOnTop { id, state } => {
                assert_eq!(id, 42);
                assert!(matches!(state, OnOff::On));
            }
            _ => panic!("expected always-on-top command"),
        }
    }

    #[test]
    fn parse_click_button_default() {
        let args = CliArgs::try_parse_from(["agent-spy", "--cli", "click", "1", "2"]).unwrap();

        match args.command.unwrap() {
            CliCommand::Click { button, .. } => {
                assert!(matches!(button, ButtonChoice::Left));
            }
            _ => panic!("expected click command"),
        }
    }

    #[test]
    fn parse_version_command() {
        let args = CliArgs::try_parse_from(["agent-spy", "--cli", "version"]).unwrap();

        match args.command.unwrap() {
            CliCommand::Version => {}
            _ => panic!("expected version command"),
        }
    }

    #[test]
    fn parse_key_tap_with_modifiers() {
        let args = CliArgs::try_parse_from([
            "agent-spy",
            "--cli",
            "key-tap",
            "a",
            "--mod",
            "shift",
            "--mod",
            "control",
        ])
        .unwrap();

        match args.command.unwrap() {
            CliCommand::KeyTap { key, modifiers } => {
                assert_eq!(key, "a");
                assert_eq!(
                    modifiers,
                    vec![ModifierChoice::Shift, ModifierChoice::Control]
                );
            }
            _ => panic!("expected key-tap command"),
        }
    }

    #[test]
    fn parse_scroll_defaults_to_vertical() {
        let args = CliArgs::try_parse_from(["agent-spy", "--cli", "scroll", "3"]).unwrap();

        match args.command.unwrap() {
            CliCommand::Scroll { amount, axis } => {
                assert_eq!(amount, 3);
                assert!(matches!(axis, ScrollAxisChoice::Vertical));
            }
            _ => panic!("expected scroll command"),
        }
    }
}
