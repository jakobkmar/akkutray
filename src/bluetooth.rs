//! Battery levels that Bluetooth devices report to Windows, e.g. headphones via the hands-free profile.

use crate::battery::{Battery, Kind, Link, Power};
use std::ptr::null_mut;
use std::time::{Duration, SystemTime};
use windows_sys::Wdk::Devices::Bluetooth::{
    DEVPKEY_Bluetooth_ClassOfDevice, DEVPKEY_Bluetooth_DeviceVID, DEVPKEY_Bluetooth_DeviceVIDSource,
};
use windows_sys::Win32::Devices::DeviceAndDriverInstallation::{
    CM_GETIDLIST_FILTER_ENUMERATOR, CM_GETIDLIST_FILTER_PRESENT, CM_Get_DevNode_PropertyW, CM_Get_Device_ID_List_SizeW,
    CM_Get_Device_ID_ListW, CM_LOCATE_DEVNODE_NORMAL, CM_Locate_DevNodeW, CR_SUCCESS,
};
use windows_sys::Win32::Devices::Properties::{DEVPKEY_Device_ContainerId, DEVPKEY_Device_FriendlyName};
use windows_sys::Win32::Foundation::DEVPROPKEY;
use windows_sys::core::GUID;

// Windows keeps the battery level and connection state on the device nodes, but doesn't document these keys.
const BATTERY: DEVPROPKEY = DEVPROPKEY {
    fmtid: GUID::from_u128(0x104ea319_6ee2_4701_bd47_8ddbf425bbe5),
    pid: 2,
};
/// When Windows last received the battery level.
const BATTERY_TIME: DEVPROPKEY = DEVPROPKEY {
    fmtid: GUID::from_u128(0x104ea319_6ee2_4701_bd47_8ddbf425bbe5),
    pid: 7,
};
const CONNECTED: DEVPROPKEY = DEVPROPKEY {
    fmtid: GUID::from_u128(0x83da6326_97a6_4088_9453_a1923f573b29),
    pid: 15,
};
/// How long a disconnected device keeps showing its last level.
const RECENT: Duration = Duration::from_secs(24 * 60 * 60);
/// Instance ids of device nodes start with this, while those of profile nodes start with the profile's UUID.
const DEVICE_PREFIX: &str = "BTHENUM\\DEV_";

/// A Bluetooth device node, either for the device itself or for one of its profiles.
struct Node {
    device: bool,
    container: u128,
    name: Option<String>,
    connected: bool,
    battery: Option<u8>,
    battery_time: Option<SystemTime>,
    class: u32,
    vendor: Option<(u8, u16)>,
}

/// Returns the battery state of every connected Bluetooth device that reports one, and the last reported state of
/// paired devices that were connected recently.
pub fn scan() -> Vec<Battery> {
    let nodes: Vec<Node> = device_ids("BTHENUM").iter().filter_map(|id| node(id)).collect();
    // The device node knows the name and connection state, while the battery level sits on a profile node
    // of the same device container. Paired but disconnected devices keep their last battery level.
    nodes
        .iter()
        .filter(|device| device.device)
        .filter_map(|device| {
            let (percent, reported) = nodes
                .iter()
                .filter(|n| n.container == device.container)
                .find_map(|n| Some((n.battery?, n.battery_time)))?;
            // A time in the future, which a changed clock can cause, counts as just now.
            let age = reported.map(|time| time.elapsed().unwrap_or_default());
            if !device.connected && age.is_none_or(|age| age > RECENT) {
                return None;
            }
            Some(Battery {
                name: device.name.clone()?,
                kind: kind(device.class),
                manufacturer: device
                    .vendor
                    .and_then(|(source, id)| manufacturer(source, id))
                    .unwrap_or_default(),
                connection: "Bluetooth".into(),
                link: Link::Bluetooth,
                percent,
                // The hands-free profile reports no charging state.
                power: Power::Unknown,
                rechargeable: true,
                remaining: None,
                pack: None,
                reported,
                connected: device.connected,
            })
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

/// Instance ids of all present devices of an enumerator, each null-terminated.
fn device_ids(enumerator: &str) -> Vec<Vec<u16>> {
    let filter: Vec<u16> = enumerator.encode_utf16().chain([0]).collect();
    let flags = CM_GETIDLIST_FILTER_ENUMERATOR | CM_GETIDLIST_FILTER_PRESENT;
    let mut len = 0;
    if unsafe { CM_Get_Device_ID_List_SizeW(&mut len, filter.as_ptr(), flags) } != CR_SUCCESS {
        return Vec::new();
    }
    let mut list = vec![0u16; len as usize];
    if unsafe { CM_Get_Device_ID_ListW(filter.as_ptr(), list.as_mut_ptr(), len, flags) } != CR_SUCCESS {
        return Vec::new();
    }
    list.split(|&c| c == 0)
        .filter(|id| !id.is_empty())
        .map(|id| [id, &[0]].concat())
        .collect()
}

fn node(id: &[u16]) -> Option<Node> {
    let mut devinst = 0;
    if unsafe { CM_Locate_DevNodeW(&mut devinst, id.as_ptr(), CM_LOCATE_DEVNODE_NORMAL) } != CR_SUCCESS {
        return None;
    }
    let get = |key| property(devinst, key);
    let name = get(&DEVPKEY_Device_FriendlyName).map(|bytes| {
        let utf16: Vec<u16> = bytes
            .as_chunks::<2>()
            .0
            .iter()
            .map(|&c| u16::from_le_bytes(c))
            .collect();
        String::from_utf16_lossy(&utf16).trim_end_matches('\0').to_string()
    });
    // The source tells who assigned the vendor id: 1 for the Bluetooth SIG, 2 for the USB-IF.
    let vendor = get(&DEVPKEY_Bluetooth_DeviceVIDSource)
        .and_then(|bytes| bytes.first().copied())
        .zip(
            get(&DEVPKEY_Bluetooth_DeviceVID)
                .and_then(|bytes| Some(u16::from_le_bytes(bytes.get(..2)?.try_into().ok()?))),
        );
    Some(Node {
        device: String::from_utf16_lossy(id).to_uppercase().starts_with(DEVICE_PREFIX),
        container: u128::from_le_bytes(get(&DEVPKEY_Device_ContainerId)?.get(..16)?.try_into().ok()?),
        name,
        connected: get(&CONNECTED)
            .and_then(|bytes| bytes.first().copied())
            .is_some_and(|b| b != 0),
        battery: get(&BATTERY).and_then(|bytes| bytes.first().copied()),
        battery_time: get(&BATTERY_TIME).and_then(|bytes| filetime(bytes.get(..8)?.try_into().ok()?)),
        class: get(&DEVPKEY_Bluetooth_ClassOfDevice)
            .and_then(|bytes| Some(u32::from_le_bytes(bytes.get(..4)?.try_into().ok()?)))
            .unwrap_or(0),
        vendor,
    })
}

/// Converts a `FILETIME`, counting 100-nanosecond intervals since 1601.
fn filetime(bytes: [u8; 8]) -> Option<SystemTime> {
    const UNIX_EPOCH: u64 = 116_444_736_000_000_000;
    let since_unix = u64::from_le_bytes(bytes).checked_sub(UNIX_EPOCH)?;
    Some(SystemTime::UNIX_EPOCH + Duration::from_nanos(since_unix * 100))
}

fn property(devinst: u32, key: &DEVPROPKEY) -> Option<Vec<u8>> {
    let (mut kind, mut size) = (0, 0);
    // The first call only asks for the size of the value.
    unsafe { CM_Get_DevNode_PropertyW(devinst, key, &mut kind, null_mut(), &mut size, 0) };
    if size == 0 {
        return None;
    }
    let mut value = vec![0u8; size as usize];
    let result = unsafe { CM_Get_DevNode_PropertyW(devinst, key, &mut kind, value.as_mut_ptr(), &mut size, 0) };
    (result == CR_SUCCESS).then_some(value)
}
