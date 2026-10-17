//! Battery of the computer itself, e.g. a laptop.

use crate::battery::{Battery, Kind};
use windows_sys::Win32::System::Power::{GetSystemPowerStatus, SYSTEM_POWER_STATUS};

/// `BatteryFlag` bit of computers without a battery.
const NO_BATTERY: u8 = 0x80;
/// `BatteryLifePercent` value when Windows can't tell the level.
const UNKNOWN_PERCENT: u8 = 255;
/// `ACLineStatus` value when the power supply is plugged in.
const AC_ONLINE: u8 = 1;

pub fn battery() -> Option<Battery> {
    let mut status: SYSTEM_POWER_STATUS = unsafe { std::mem::zeroed() };
    let ok = unsafe { GetSystemPowerStatus(&mut status) } != 0;
    if !ok || status.BatteryFlag & NO_BATTERY != 0 || status.BatteryLifePercent == UNKNOWN_PERCENT {
        return None;
    }
    Some(Battery {
        name: std::env::var("COMPUTERNAME").unwrap_or_else(|_| "This computer".into()),
        kind: Kind::Computer,
        manufacturer: String::new(),
        connection: "Built-in battery".into(),
        percent: status.BatteryLifePercent,
        charging: status.ACLineStatus == AC_ONLINE,
    })
}
