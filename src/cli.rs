use std::path::PathBuf;

use anyhow::{Result, anyhow, bail};
use clap::{CommandFactory, Parser, Subcommand, ValueEnum, error::ErrorKind};

use crate::core::{Core, MouseButtonArg};
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
        CliCommand::CheckPermissions => {
            let permissions = core.permissions();
            Ok(format!(
                "screen_capture={}\naccessibility={}\ninput_simulation={}\ncursor_tracking={}",
                permissions.screen_capture,
                permissions.accessibility,
                permissions.input_simulation,
                permissions.cursor_tracking
            ))
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

impl From<ButtonChoice> for MouseButtonArg {
    fn from(value: ButtonChoice) -> Self {
        match value {
            ButtonChoice::Left => Self::Left,
            ButtonChoice::Right => Self::Right,
            ButtonChoice::Middle => Self::Middle,
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
    use super::{ButtonChoice, CliArgs, CliCommand, OnOff};
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
}
