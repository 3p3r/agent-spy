#![cfg_attr(windows, windows_subsystem = "windows")]

mod app;
mod cli;
mod core;
mod message;
mod platform;

fn main() -> iced::Result {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.iter().any(|arg| arg == "--cli") {
        let exit_code = cli::run_from_args(args);
        std::process::exit(exit_code);
    }

    app::run()
}
