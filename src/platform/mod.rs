//! Everything that depends on the operating system. Each platform provides the same modules, named after the module
//! they serve, so a new platform plugs in with a directory of its own:
//!
//! - `host`: the computer's battery and model
//! - `hidpp`: pairing of the HID collections of a HID++ device
//! - `bluetooth`: battery levels that the system collects from Bluetooth devices
//! - `tray`: event loop, single instance, notifications and detaching from the terminal
//! - `popup`: native window, system fonts and settings
//!
//! A platform without a feature returns nothing for it, e.g. no Bluetooth devices, and the rest works as before.

#[cfg(windows)]
mod windows;
#[cfg(windows)]
pub use windows::*;

#[cfg(not(windows))]
compile_error!("akkutray only supports Windows so far");
