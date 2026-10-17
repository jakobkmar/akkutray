use crate::{bluetooth, hidpp, host};
use std::fmt;

#[derive(Clone, Copy, PartialEq)]
pub enum Kind {
    Computer,
    Keyboard,
    Mouse,
    Headset,
    Other,
}

#[derive(Clone)]
pub struct Battery {
    pub name: String,
    pub kind: Kind,
    pub manufacturer: String,
    /// How the device is connected, e.g. the receiver it is paired with.
    pub connection: String,
    pub percent: u8,
    pub charging: bool,
}

impl fmt::Display for Battery {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}: {}%", self.name, self.percent)?;
        if self.charging {
            f.write_str(" (charging)")?;
        }
        Ok(())
    }
}

/// Returns the battery state of this computer, followed by that of every connected device.
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
