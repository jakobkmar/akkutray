//! Minimal HID++ 2.0 client reading the battery state of Logitech devices.

use crate::battery::{Battery, Kind};
use hidapi::{HidApi, HidDevice};
use std::time::{Duration, Instant};

const VENDOR_ID: u16 = 0x046d;
const USAGE_PAGE: u16 = 0xff00;
const SHORT_REPORT: u8 = 0x10;
const LONG_REPORT: u8 = 0x11;
/// Arbitrary non-zero id that tells our responses apart from those of G Hub and from device notifications.
const SOFTWARE_ID: u8 = 0x0b;
/// Sleeping wireless devices can take over a second to wake up and answer.
const TIMEOUT: Duration = Duration::from_secs(2);

const ROOT: u8 = 0x00;
const DEVICE_NAME_TYPE: u16 = 0x0005;
const UNIFIED_BATTERY: u16 = 0x1004;

/// Returns the battery state of every reachable device, on receivers as well as wired.
pub fn scan() -> Vec<Battery> {
    match HidApi::new() {
        Ok(api) => channels(&api).iter().flat_map(Channel::batteries).collect(),
        Err(_) => Vec::new(),
    }
}

/// The HID++ interface of a receiver or wired device. Windows splits it into one HID collection per report type.
struct Channel {
    short: HidDevice,
    long: HidDevice,
    /// Manufacturer and product strings from the USB descriptor of the receiver or wired device.
    manufacturer: String,
    product: String,
}

fn channels(api: &HidApi) -> Vec<Channel> {
    let open = |usage| {
        api.device_list()
            .filter(|d| d.vendor_id() == VENDOR_ID && d.usage_page() == USAGE_PAGE && d.usage() == usage)
            .filter_map(|d| {
                let dev = d.open_device(api).ok()?;
                let id = dev.get_container_id().ok()?;
                Some(((id.data1, id.data2, id.data3, id.data4), dev, d))
            })
            .collect::<Vec<_>>()
    };
    let mut shorts = open(1);
    open(2)
        .into_iter()
        .filter_map(|(id, long, info)| {
            let i = shorts.iter().position(|(other, ..)| *other == id)?;
            Some(Channel {
                short: shorts.swap_remove(i).1,
                long,
                manufacturer: info.manufacturer_string().unwrap_or_default().to_string(),
                product: info.product_string().unwrap_or_default().to_string(),
            })
        })
        .collect()
}

impl Channel {
    fn batteries(&self) -> Vec<Battery> {
        // Index 0xff addresses a wired device. Receivers reject it and expose their paired devices on slots 1 to 6.
        let indices = if self.feature(0xff, UNIFIED_BATTERY).is_some() {
            vec![0xff]
        } else {
            (1..=6).collect()
        };
        indices.into_iter().filter_map(|index| self.battery(index)).collect()
    }

    fn battery(&self, index: u8) -> Option<Battery> {
        let feature = self.feature(index, UNIFIED_BATTERY).filter(|&f| f != 0)?;
        // Response: state of charge in percent, level flags, charging status, external power.
        let status = self.request(index, feature, 1, &[])?;
        let (name, kind) = self
            .identity(index)
            .unwrap_or_else(|| (format!("Device {index}"), Kind::Other));
        // A wired device's product string is its own name, so only a receiver's tells how the device is connected.
        let connection = if index == 0xff {
            "USB cable".into()
        } else {
            self.product.clone()
        };
        Some(Battery {
            name,
            kind,
            manufacturer: self.manufacturer.clone(),
            connection,
            percent: status[0],
            charging: matches!(status[2], 1..=3),
        })
    }

    fn identity(&self, index: u8) -> Option<(String, Kind)> {
        let feature = self.feature(index, DEVICE_NAME_TYPE).filter(|&f| f != 0)?;
        let len = self.request(index, feature, 0, &[])?[0] as usize;
        let mut name = Vec::with_capacity(len);
        while name.len() < len {
            let chunk = self.request(index, feature, 1, &[name.len() as u8])?;
            name.extend_from_slice(&chunk[..chunk.len().min(len - name.len())]);
        }
        let kind = match self.request(index, feature, 2, &[])?[0] {
            0 | 2 => Kind::Keyboard,
            3..=5 => Kind::Mouse,
            8 => Kind::Headset,
            _ => Kind::Other,
        };
        Some((String::from_utf8_lossy(&name).into_owned(), kind))
    }

    /// Looks up the index of a feature; 0 means the device does not support it.
    fn feature(&self, index: u8, id: u16) -> Option<u8> {
        Some(self.request(index, ROOT, 0, &id.to_be_bytes())?[0])
    }

    fn request(&self, index: u8, feature: u8, function: u8, params: &[u8]) -> Option<[u8; 16]> {
        let id = function << 4 | SOFTWARE_ID;
        let mut buf = [0; 20];
        buf[..4].copy_from_slice(&[LONG_REPORT, index, feature, id]);
        buf[4..4 + params.len()].copy_from_slice(params);
        self.long.write(&buf).ok()?;

        let deadline = Instant::now() + TIMEOUT;
        while Instant::now() < deadline {
            // Receivers answer for unreachable devices with a HID++ 1.0 error, which arrives on the short collection.
            let n = self.short.read_timeout(&mut buf, 0).ok()?;
            if n == 7 && buf[..5] == [SHORT_REPORT, index, 0x8f, feature, id] {
                return None;
            }
            let n = self.long.read_timeout(&mut buf, 10).ok()?;
            if n == 20 && buf[1] == index {
                if buf[2..4] == [feature, id] {
                    return buf[4..].try_into().ok();
                }
                if buf[2..5] == [0xff, feature, id] {
                    return None;
                }
            }
        }
        None
    }
}
