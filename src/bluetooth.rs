//! Battery levels that Bluetooth devices report to Windows, e.g. headphones via the hands-free profile.

use crate::battery::{Battery, Kind, Link};
use std::ptr::null_mut;
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
const CONNECTED: DEVPROPKEY = DEVPROPKEY {
    fmtid: GUID::from_u128(0x83da6326_97a6_4088_9453_a1923f573b29),
    pid: 15,
};
/// `DEVPKEY_Bluetooth_ClassOfDevice` from the WDK.
const CLASS_OF_DEVICE: DEVPROPKEY = DEVPROPKEY {
    fmtid: GUID::from_u128(0x2bd67d8b_8beb_48d5_87e0_6cda3428040a),
    pid: 10,
};

/// A Bluetooth device node, either for the device itself or for one of its profiles.
struct Node {
    container: u128,
    name: Option<String>,
    connected: bool,
    battery: Option<u8>,
    class: u32,
}

/// Returns the battery state of every connected Bluetooth device that reports one.
pub fn scan() -> Vec<Battery> {
    let nodes: Vec<Node> = device_ids("BTHENUM").iter().filter_map(|id| node(id)).collect();
    // The device node knows the name and connection state, while the battery level sits on a profile node
    // of the same device container. Paired but disconnected devices keep their last battery level.
    nodes
        .iter()
        .filter(|device| device.connected)
        .filter_map(|device| {
            let percent = nodes
                .iter()
                .filter(|n| n.container == device.container)
                .find_map(|n| n.battery)?;
            Some(Battery {
                name: device.name.clone()?,
                kind: kind(device.class),
                manufacturer: String::new(),
                connection: "Bluetooth".into(),
                link: Link::Bluetooth,
                percent,
                // The hands-free profile reports no charging state.
                charging: false,
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
    Some(Node {
        container: u128::from_le_bytes(get(&DEVPKEY_Device_ContainerId)?.get(..16)?.try_into().ok()?),
        name,
        connected: get(&CONNECTED)
            .and_then(|bytes| bytes.first().copied())
            .is_some_and(|b| b != 0),
        battery: get(&BATTERY).and_then(|bytes| bytes.first().copied()),
        class: get(&CLASS_OF_DEVICE)
            .and_then(|bytes| Some(u32::from_le_bytes(bytes.get(..4)?.try_into().ok()?)))
            .unwrap_or(0),
    })
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
