//! Battery levels that Bluetooth devices report to the system, e.g. headphones via the hands-free profile.

use crate::battery::{Battery, Kind, Link, Power};
use crate::platform;
use std::time::{Duration, SystemTime};

/// How long a disconnected device keeps showing its last level.
const RECENT: Duration = Duration::from_secs(24 * 60 * 60);

/// A paired Bluetooth device that reported its battery level.
pub struct Device {
    pub name: String,
    pub connected: bool,
    pub percent: u8,
    /// When the system last received the level.
    pub reported: Option<SystemTime>,
    /// The Bluetooth class of device.
    pub class: u32,
    /// The source of the vendor id, as in the Device ID profile, and the id. The source tells who assigned the id:
    /// 1 for the Bluetooth SIG, 2 for the USB-IF.
    pub vendor: Option<(u8, u16)>,
}

/// Returns the battery state of every connected Bluetooth device that reports one, and the last reported state of
/// paired devices that were connected recently.
pub fn scan() -> Vec<Battery> {
    platform::bluetooth::devices()
        .into_iter()
        .filter(|device| {
            // A time in the future, which a changed clock can cause, counts as just now.
            let age = device.reported.map(|time| time.elapsed().unwrap_or_default());
            device.connected || age.is_some_and(|age| age <= RECENT)
        })
        .map(|device| Battery {
            name: device.name,
            kind: kind(device.class),
            manufacturer: device
                .vendor
                .and_then(|(source, id)| manufacturer(source, id))
                .unwrap_or_default(),
            connection: "Bluetooth".into(),
            link: Link::Bluetooth,
            percent: device.percent,
            // The hands-free profile reports no charging state.
            power: Power::Unknown,
            rechargeable: true,
            remaining: None,
            pack: None,
            supply: None,
            reported: device.reported,
            connected: device.connected,
        })
        .collect()
}

/// Maps a Bluetooth class of device, with the major class in bits 8 to 12 and the minor class in bits 2 to 7.
fn kind(class: u32) -> Kind {
    match ((class >> 8) & 0x1f, (class >> 2) & 0x3f) {
        (0x01, _) => Kind::Computer,
        (0x04, 0x01 | 0x02 | 0x06) => Kind::Headset,
        (0x05, minor) if minor & 0x10 != 0 => Kind::Keyboard,
        (0x05, minor) if minor & 0x20 != 0 => Kind::Mouse,
        _ => Kind::Other,
    }
}

/// Looks up the manufacturer of a vendor id in `data/vendors.txt`, as Windows only names the driver's manufacturer.
fn manufacturer(source: u8, id: u16) -> Option<String> {
    let source = match source {
        1 => "bluetooth",
        2 => "usb",
        _ => return None,
    };
    include_str!("../data/vendors.txt")
        .lines()
        .filter(|line| !line.starts_with('#'))
        .filter_map(|line| line.split_once(" = "))
        .find(|(key, _)| {
            key.trim()
                .split_once(':')
                .is_some_and(|(s, hex)| s == source && u16::from_str_radix(hex.trim_start_matches("0x"), 16) == Ok(id))
        })
        .map(|(_, name)| name.trim().to_string())
}
