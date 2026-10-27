//! The computer's battery through the power API and the battery drivers, its power supply through WinRT, and its model
//! from the firmware.

use crate::battery::{Pack, Power, Supply};
use crate::host::Status;
use std::ffi::{OsString, c_void};
use std::fs::File;
use std::os::windows::ffi::OsStringExt;
use std::os::windows::io::AsRawHandle;
use std::ptr::{null, null_mut};
use std::time::Duration;
use windows_sys::Win32::Devices::DeviceAndDriverInstallation::{
    CM_GET_DEVICE_INTERFACE_LIST_PRESENT, CM_Get_Device_Interface_List_SizeW, CM_Get_Device_Interface_ListW, CR_SUCCESS,
};
use windows_sys::Win32::System::Com::{COINIT_MULTITHREADED, CoInitializeEx, CoUninitialize};
use windows_sys::Win32::System::IO::DeviceIoControl;
use windows_sys::Win32::System::Power::{
    BATTERY_CAPACITY_RELATIVE, BATTERY_INFORMATION, BATTERY_QUERY_INFORMATION, BATTERY_STATUS,
    BATTERY_UNKNOWN_CAPACITY, BATTERY_UNKNOWN_RATE, BATTERY_WAIT_STATUS, BatteryInformation, GUID_DEVICE_BATTERY,
    GetSystemPowerStatus, IOCTL_BATTERY_QUERY_INFORMATION, IOCTL_BATTERY_QUERY_STATUS, IOCTL_BATTERY_QUERY_TAG,
    SYSTEM_POWER_STATUS,
};
use windows_sys::Win32::System::Registry::{HKEY_LOCAL_MACHINE, RRF_RT_REG_SZ, RegGetValueW};
use windows_sys::core::{GUID, HRESULT, HSTRING, PCWSTR};
use windows_sys::w;

// windows-sys has no WinRT functions, so these come straight from the system.
#[link(name = "combase", kind = "raw-dylib")]
unsafe extern "system" {
    fn RoGetActivationFactory(class: HSTRING, iid: *const GUID, factory: *mut *mut c_void) -> HRESULT;
    fn WindowsCreateString(text: PCWSTR, len: u32, string: *mut HSTRING) -> HRESULT;
    fn WindowsDeleteString(string: HSTRING) -> HRESULT;
}

/// Id of `Windows.System.Power.IPowerManagerStatics`, hard-coded as only the WinRT metadata holds it.
const POWER_MANAGER_STATICS: GUID = GUID::from_u128(0x1394825d_62ce_4364_98d5_aa28c7fbd15b);

/// The methods of `IPowerManagerStatics` up to the one that is called, in the order of its metadata.
#[repr(C)]
struct PowerManagerStatics {
    /// `QueryInterface` and `AddRef`.
    _unused: [usize; 2],
    release: unsafe extern "system" fn(this: *mut c_void) -> u32,
    /// The rest of `IInspectable`, and the energy saver and battery status with their events.
    _skipped: [usize; 9],
    power_supply_status: unsafe extern "system" fn(this: *mut c_void, status: *mut i32) -> HRESULT,
}

/// `PowerSupplyStatus` values.
const INADEQUATE: i32 = 1;
const ADEQUATE: i32 = 2;

/// `BatteryFlag` bit of computers without a battery.
const NO_BATTERY: u8 = 0x80;
/// `BatteryFlag` bit while the battery charges. Being plugged in alone doesn't mean it does.
const CHARGING: u8 = 0x08;
/// `BatteryLifePercent` value when Windows can't tell the level.
const UNKNOWN_PERCENT: u8 = 255;
/// `BatteryLifeTime` value when Windows can't estimate the time left, e.g. right after unplugging.
const UNKNOWN_TIME: u32 = u32::MAX;
/// `ACLineStatus` value when the power supply is plugged in.
const AC_ONLINE: u8 = 1;

pub fn status() -> Option<Status> {
    let mut status = SYSTEM_POWER_STATUS::default();
    let ok = unsafe { GetSystemPowerStatus(&mut status) } != 0;
    if !ok || status.BatteryFlag & NO_BATTERY != 0 || status.BatteryLifePercent == UNKNOWN_PERCENT {
        return None;
    }
    let power = match (status.ACLineStatus == AC_ONLINE, status.BatteryFlag & CHARGING != 0) {
        (true, true) => Power::Charging,
        (true, false) if status.BatteryLifePercent >= 100 => Power::Full,
        (true, false) => Power::PluggedIn,
        (false, _) => Power::Discharging,
    };
    Some(Status {
        percent: status.BatteryLifePercent,
        power,
        remaining: (status.BatteryLifeTime != UNKNOWN_TIME).then(|| Duration::from_secs(status.BatteryLifeTime.into())),
    })
}

/// Whether the plugged in power supply is strong enough, from the `PowerManager` of WinRT.
pub fn supply() -> Option<Supply> {
    let class: Vec<u16> = "Windows.System.Power.PowerManager".encode_utf16().collect();
    let mut name = null_mut();
    if unsafe { WindowsCreateString(class.as_ptr(), class.len() as u32, &mut name) } < 0 {
        return None;
    }
    // WinRT needs COM on this thread. A thread that already uses COM in another mode works as well.
    let com = unsafe { CoInitializeEx(null(), COINIT_MULTITHREADED as u32) } >= 0;
    let mut factory = null_mut();
    let mut status = 0;
    unsafe {
        if RoGetActivationFactory(name, &POWER_MANAGER_STATICS, &mut factory) >= 0 {
            let statics = &**factory.cast::<*const PowerManagerStatics>();
            if (statics.power_supply_status)(factory, &mut status) < 0 {
                status = 0;
            }
            (statics.release)(factory);
        }
        if com {
            CoUninitialize();
        }
        WindowsDeleteString(name);
    }
    match status {
        ADEQUATE => Some(Supply::Adequate),
        INADEQUATE => Some(Supply::Inadequate),
        _ => None,
    }
}

pub fn model() -> Option<String> {
    firmware_value(w!("SystemProductName"))
}

pub fn manufacturer() -> Option<String> {
    firmware_value(w!("SystemManufacturer"))
}

pub fn hostname() -> Option<String> {
    std::env::var("COMPUTERNAME").ok()
}

/// Reads the capacity and wear of every battery from its driver.
pub fn packs() -> impl Iterator<Item = Pack> {
    battery_paths()
        .into_iter()
        .filter_map(|path| query(&File::open(path).ok()?))
}

fn query(battery: &File) -> Option<Pack> {
    // A zero timeout returns the tag at once, which identifies the battery that is inserted right now.
    let tag: u32 = ioctl(battery, IOCTL_BATTERY_QUERY_TAG, &0u32).filter(|&tag| tag != 0)?;
    let info: BATTERY_INFORMATION = ioctl(
        battery,
        IOCTL_BATTERY_QUERY_INFORMATION,
        &BATTERY_QUERY_INFORMATION {
            BatteryTag: tag,
            InformationLevel: BatteryInformation,
            AtRate: 0,
        },
    )?;
    let status: BATTERY_STATUS = ioctl(
        battery,
        IOCTL_BATTERY_QUERY_STATUS,
        &BATTERY_WAIT_STATUS {
            BatteryTag: tag,
            ..Default::default()
        },
    )?;
    // Some batteries report their capacity in percent rather than in milliwatt-hours, which says nothing about wear.
    let capacities = [info.DesignedCapacity, info.FullChargedCapacity, status.Capacity];
    if info.Capabilities & BATTERY_CAPACITY_RELATIVE != 0
        || info.DesignedCapacity == 0
        || capacities.contains(&BATTERY_UNKNOWN_CAPACITY)
    {
        return None;
    }
    Some(Pack {
        now: status.Capacity,
        full: info.FullChargedCapacity,
        design: info.DesignedCapacity,
        rate: (status.Rate != BATTERY_UNKNOWN_RATE as i32).then_some(status.Rate),
        // Batteries that don't count their cycles report 0.
        cycles: (info.CycleCount != 0).then_some(info.CycleCount),
    })
}

/// Sends a request to a battery driver, with the input and output types its control code expects.
fn ioctl<I, O: Default>(battery: &File, code: u32, input: &I) -> Option<O> {
    let mut output = O::default();
    let mut returned = 0;
    let ok = unsafe {
        DeviceIoControl(
            battery.as_raw_handle(),
            code,
            (input as *const I).cast(),
            size_of::<I>() as u32,
            (&mut output as *mut O).cast(),
            size_of::<O>() as u32,
            &mut returned,
            null_mut(),
        )
    } != 0;
    (ok && returned as usize == size_of::<O>()).then_some(output)
}

/// Device paths of all batteries.
fn battery_paths() -> Vec<OsString> {
    let flags = CM_GET_DEVICE_INTERFACE_LIST_PRESENT;
    let mut len = 0;
    if unsafe { CM_Get_Device_Interface_List_SizeW(&mut len, &GUID_DEVICE_BATTERY, null(), flags) } != CR_SUCCESS {
        return Vec::new();
    }
    let mut list = vec![0u16; len as usize];
    if unsafe { CM_Get_Device_Interface_ListW(&GUID_DEVICE_BATTERY, null(), list.as_mut_ptr(), len, flags) }
        != CR_SUCCESS
    {
        return Vec::new();
    }
    list.split(|&c| c == 0)
        .filter(|path| !path.is_empty())
        .map(OsString::from_wide)
        .collect()
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
