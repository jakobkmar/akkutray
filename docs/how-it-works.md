# How it works

Every reading collects batteries from three sources, in this order, then replaces unclear names with the display
names from `data/names.txt`:

1. **This computer**, through Windows
2. **Logitech devices**, through the HID++ protocol
3. **Bluetooth devices**, through the Windows device tree

## This computer

The battery level, charging state and time estimate come from the Windows `GetSystemPowerStatus` API. Being
plugged in doesn't mean charging: a charge limit can hold the battery at its level, which the API tells apart.

The capacity, wear, charge cycles and the current power flow come from the battery driver itself, through
`IOCTL_BATTERY_QUERY_INFORMATION` and `IOCTL_BATTERY_QUERY_STATUS`. Computers with several batteries get their sum.
While charging, the time until full is computed from the current power flow.

The model and manufacturer come from the SMBIOS system information, which Windows mirrors in the registry under
`HKLM\HARDWARE\DESCRIPTION\System\BIOS`.

## Logitech devices

Logitech receivers and devices speak the HID++ protocol on a vendor-specific HID interface (usage page `0xFF00`).
Windows splits that interface into two collections, one for short reports and one for long reports.

akkutray sends a battery request to every device slot of every receiver, and to every wired device, and reads the
answers. G Hub talks to the same interface, so akkutray uses its own software id to keep its answers apart from
those for G Hub.

Besides the level, the answer tells whether the device is charging, full, plugged in without charging, or unable to
charge. The battery capabilities tell whether it is rechargeable at all. As every receiver calls itself "USB
Receiver", akkutray names Bolt, Unifying and LIGHTSPEED receivers by their USB product ids.

## Bluetooth devices

Windows stores the battery level of a Bluetooth device as a property on one of the device's profile nodes, together
with the time it received it. akkutray reads both with the Configuration Manager API, then takes the name,
connection state and vendor id from the device node in the same device container.

Paired devices that are switched off keep their last level. akkutray shows them as not connected only if that level
is less than a day old, so devices unused for long don't clutter the window.

Windows names Microsoft as the manufacturer of every Bluetooth device, after its driver. akkutray looks up the vendor
id the device reports in `data/vendors.txt` instead.

## Code layout

| File | Contents |
| --- | --- |
| `src/main.rs` | Command line |
| `src/battery.rs` | Common battery type, scan of all sources and display names |
| `src/host.rs` | This computer |
| `src/hidpp.rs` | Logitech HID++ protocol |
| `src/bluetooth.rs` | Bluetooth devices |
| `src/tray.rs` | Tray icon, menu and update timer |
| `src/popup.rs` | Window opened by a left click, built with [egui](https://github.com/emilk/egui) |
| `data/names.txt` | Display names for unclearly named devices, compiled into the program |
| `data/vendors.txt` | Manufacturer names of Bluetooth devices by vendor id, compiled into the program |
