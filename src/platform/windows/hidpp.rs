//! Windows splits the HID++ interface of a device into one HID collection per report type, each with a path of its
//! own, but gives them the same container id.

use hidapi::HidDevice;

/// Tells which device a HID collection belongs to, so the collections of one device can be paired up.
pub fn device_key(collection: &HidDevice) -> Option<(u32, u16, u16, [u8; 8])> {
    let id = collection.get_container_id().ok()?;
    Some((id.data1, id.data2, id.data3, id.data4))
}
