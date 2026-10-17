//! Battery of the computer itself, e.g. a laptop.

use crate::battery::{Battery, Kind, Link};
use std::ptr::null_mut;
use windows_sys::Win32::System::Power::{GetSystemPowerStatus, SYSTEM_POWER_STATUS};
use windows_sys::Win32::System::Registry::{HKEY_LOCAL_MACHINE, RRF_RT_REG_SZ, RegGetValueW};
use windows_sys::core::PCWSTR;
use windows_sys::w;

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
    let name = firmware_value(w!("SystemProductName"))
        .or_else(|| std::env::var("COMPUTERNAME").ok())
        .unwrap_or_else(|| "This computer".into());
    Some(Battery {
        name,
        kind: Kind::Computer,
        manufacturer: firmware_value(w!("SystemManufacturer")).unwrap_or_default(),
        connection: "Built-in battery".into(),
        link: Link::Builtin,
        percent: status.BatteryLifePercent,
        charging: status.ACLineStatus == AC_ONLINE,
    })
}

/// Reads a value of the SMBIOS system information, which Windows mirrors in the registry.
fn firmware_value(name: PCWSTR) -> Option<String> {
    let key = w!("HARDWARE\\DESCRIPTION\\System\\BIOS");
    let mut buffer = [0u16; 256];
    let mut size = size_of_val(&buffer) as u32;
    let result = unsafe {
        RegGetValueW(
            HKEY_LOCAL_MACHINE,
            key,
            name,
            RRF_RT_REG_SZ,
            null_mut(),
            buffer.as_mut_ptr().cast(),
            &mut size,
        )
    };
    let len = buffer.iter().position(|&c| c == 0)?;
    let value = String::from_utf16_lossy(&buffer[..len]).trim().to_string();
    (result == 0 && !value.is_empty()).then_some(value)
}
