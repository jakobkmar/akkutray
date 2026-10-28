//! Linux lists the HID++ interface of a device once for each report type, with the same hidraw node as path.

use hidapi::HidDevice;
use std::ffi::CString;

/// Tells which device a HID collection belongs to, so the collections of one device can be paired up.
pub fn device_key(collection: &HidDevice) -> Option<CString> {
    Some(collection.get_device_info().ok()?.path().to_owned())
}
