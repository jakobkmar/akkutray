//! Battery levels that BlueZ receives from Bluetooth devices, read over D-Bus.

use crate::bluetooth::Device;
use std::collections::HashMap;
use zbus::blocking::Connection;
use zbus::blocking::fdo::ObjectManagerProxy;
use zbus::zvariant::OwnedValue;

/// Returns every connected Bluetooth device that reported its battery level. BlueZ forgets the level on disconnect.
pub fn devices() -> Vec<Device> {
    let Ok(objects) = objects() else {
        return Vec::new();
    };
    objects
        .values()
        .filter_map(|interfaces| {
            let interface = |name: &str| interfaces.iter().find(|(key, _)| key.as_str() == name).map(|(_, v)| v);
            let (device, battery) = (interface("org.bluez.Device1")?, interface("org.bluez.Battery1")?);
            Some(Device {
                name: get::<String>(device, "Alias").or_else(|| get(device, "Name"))?,
                connected: get(device, "Connected").unwrap_or(false),
                percent: get(battery, "Percentage")?,
                reported: None,
                class: get(device, "Class").unwrap_or(0),
                vendor: get::<String>(device, "Modalias").and_then(|modalias| vendor(&modalias)),
            })
        })
        .collect()
}

type Objects = HashMap<zbus::zvariant::OwnedObjectPath, HashMap<zbus::names::OwnedInterfaceName, Properties>>;
type Properties = HashMap<String, OwnedValue>;

fn objects() -> zbus::Result<Objects> {
    let connection = Connection::system()?;
    let manager = ObjectManagerProxy::builder(&connection)
        .destination("org.bluez")?
        .path("/")?
        .build()?;
    Ok(manager.get_managed_objects()?)
}

fn get<T: TryFrom<OwnedValue>>(properties: &Properties, name: &str) -> Option<T> {
    T::try_from(properties.get(name)?.try_clone().ok()?).ok()
}

/// Reads the source and vendor id from a modalias such as `usb:v046DpB023d0006`, numbering the sources as the Device ID
/// profile does.
fn vendor(modalias: &str) -> Option<(u8, u16)> {
    let (source, ids) = modalias.split_once(":v")?;
    let source = match source {
        "bluetooth" => 1,
        "usb" => 2,
        _ => return None,
    };
    Some((source, u16::from_str_radix(ids.get(..4)?, 16).ok()?))
}
