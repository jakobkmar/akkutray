use crate::{bluetooth, hidpp, host};
use std::fmt;
use std::time::{Duration, SystemTime};

#[derive(Clone, Copy, PartialEq)]
pub enum Kind {
    Computer,
    Keyboard,
    Mouse,
    Headset,
    Other,
}

/// The way a device is connected to the computer.
#[derive(Clone, Copy, PartialEq)]
pub enum Link {
    Builtin,
    Usb,
    Bluetooth,
}

/// What the power supply of a device is doing.
#[derive(Clone, Copy, PartialEq)]
pub enum Power {
    /// Running on its battery.
    Discharging,
    Charging,
    /// Plugged in, but the battery doesn't charge, e.g. because a charge limit holds it below full.
    PluggedIn,
    /// Plugged in with a full battery.
    Full,
    /// The device reports that it can't charge.
    Fault,
    /// The device doesn't report it.
    Unknown,
}

/// Capacity and wear of a computer's battery, summed up over its batteries if it has several.
#[derive(Clone, Copy)]
pub struct Pack {
    /// Energy stored now, when fully charged, and when fully charged as new, in milliwatt-hours.
    pub now: u32,
    pub full: u32,
    pub design: u32,
    /// Power flowing into the battery while charging, or out of it while discharging, in milliwatts.
    pub rate: Option<i32>,
    pub cycles: Option<u32>,
}

#[derive(Clone)]
pub struct Battery {
    pub name: String,
    pub kind: Kind,
    pub manufacturer: String,
    /// How the device is connected, e.g. the receiver it is paired with.
    pub connection: String,
    pub link: Link,
    pub percent: u8,
    pub power: Power,
    /// False for disposable batteries, which are replaced rather than charged.
    pub rechargeable: bool,
    /// Estimated time until the battery is empty, or full while charging.
    pub remaining: Option<Duration>,
    pub pack: Option<Pack>,
    /// When the device last reported its level, if it isn't read live.
    pub reported: Option<SystemTime>,
    /// False for paired devices that are out of reach, which show their last reported level.
    pub connected: bool,
}

impl fmt::Display for Battery {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}: {}%", self.name, self.percent)?;
        match self.power {
            Power::Charging => f.write_str(" (charging)"),
            Power::PluggedIn => f.write_str(" (plugged in, not charging)"),
            Power::Full => f.write_str(" (full)"),
            Power::Fault => f.write_str(" (charging problem)"),
            Power::Discharging | Power::Unknown => Ok(()),
        }
    }
}

/// Returns the battery state of this computer, followed by that of every connected device, and of paired devices
/// that were connected recently.
pub fn scan() -> Vec<Battery> {
    host::battery()
        .into_iter()
        .chain(hidpp::scan())
        .chain(bluetooth::scan())
        .map(|battery| Battery {
            name: display_name(battery.name),
            ..battery
        })
        .collect()
}

/// Replaces a reported name with the display name listed for it in `data/names.txt`, if any.
fn display_name(reported: String) -> String {
    include_str!("../data/names.txt")
        .lines()
        .filter(|line| !line.starts_with('#'))
        .filter_map(|line| line.split_once(" = "))
        .find(|(from, _)| from.trim() == reported)
        .map_or(reported, |(_, to)| to.trim().to_string())
}
