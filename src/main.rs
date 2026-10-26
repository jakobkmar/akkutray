mod battery;
mod bluetooth;
mod hidpp;
mod host;
mod platform;
mod popup;
mod tray;

use std::process::{Command, ExitCode, Stdio};

const USAGE: &str = "\
Usage: akkutray [command]

Commands:
  status  Print the battery levels of this computer and connected devices (default)
  tray    Show the battery level as a tray icon in the background
  help    Print this help";

fn main() -> ExitCode {
    match std::env::args().nth(1).as_deref() {
        None | Some("status") => status(),
        Some("tray") => tray(),
        Some("help" | "-h" | "--help") => {
            println!("{USAGE}");
            ExitCode::SUCCESS
        }
        Some(other) => {
            eprintln!("Unknown command '{other}'\n\n{USAGE}");
            ExitCode::FAILURE
        }
    }
}

fn status() -> ExitCode {
    let batteries: Vec<_> = battery::scan()
        .into_iter()
        .filter(|battery| battery.connected)
        .collect();
    if batteries.is_empty() {
        eprintln!("No device with battery found");
        return ExitCode::FAILURE;
    }
    for battery in batteries {
        println!("{battery}");
    }
    ExitCode::SUCCESS
}

fn tray() -> ExitCode {
    if platform::tray::is_running() {
        println!("Tray is already running");
        return ExitCode::SUCCESS;
    }
    if !platform::tray::has_console() {
        return match tray::run() {
            Ok(()) => ExitCode::SUCCESS,
            Err(err) => {
                eprintln!("Failed to run tray: {err}");
                ExitCode::FAILURE
            }
        };
    }
    // Relaunch without a console, so the tray neither blocks the terminal nor keeps a console window open.
    let child = std::env::current_exe().and_then(|exe| {
        platform::tray::detach(Command::new(exe).arg("tray"))
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
    });
    match child {
        Ok(_) => ExitCode::SUCCESS,
        Err(err) => {
            eprintln!("Failed to start tray: {err}");
            ExitCode::FAILURE
        }
    }
}
