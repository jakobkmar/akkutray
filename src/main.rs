mod hidpp;
mod popup;
mod tray;

use std::os::windows::process::CommandExt;
use std::process::{Command, ExitCode, Stdio};
use windows_sys::Win32::System::Console::GetConsoleWindow;
use windows_sys::Win32::System::Threading::DETACHED_PROCESS;

const USAGE: &str = "\
Usage: akkutray [command]

Commands:
  status  Print the battery level of connected Logitech devices (default)
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
    let batteries = hidpp::scan();
    if batteries.is_empty() {
        eprintln!("No Logitech device with battery found");
        return ExitCode::FAILURE;
    }
    for battery in batteries {
        println!("{battery}");
    }
    ExitCode::SUCCESS
}

fn tray() -> ExitCode {
    if tray::is_running() {
        println!("Tray is already running");
        return ExitCode::SUCCESS;
    }
    if unsafe { GetConsoleWindow() }.is_null() {
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
        Command::new(exe)
            .arg("tray")
            .creation_flags(DETACHED_PROCESS)
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
