# How it works

Every reading collects batteries from three sources, in this order, then replaces unclear names with the display
names from `data/names.txt`:

1. **This computer**, through Windows
2. **Logitech devices**, through the HID++ protocol
3. **Bluetooth devices**, through the Windows device tree

## This computer

The battery level and charging state come from the Windows `GetSystemPowerStatus` API. The model and manufacturer
come from the SMBIOS system information, which Windows mirrors in the registry under
`HKLM\HARDWARE\DESCRIPTION\System\BIOS`.

## Logitech devices

Logitech receivers and devices speak the HID++ protocol on a vendor-specific HID interface (usage page `0xFF00`).
Windows splits that interface into two collections, one for short reports and one for long reports.

akkutray sends a battery request to every device slot of every receiver, and to every wired device, and reads the
answers. G Hub talks to the same interface, so akkutray uses its own software id to keep its answers apart from
those for G Hub.

## Bluetooth devices

Windows stores the battery level of a Bluetooth device as a property on one of the device's profile nodes. akkutray
reads it with the Configuration Manager API, then takes the name and connection state from the device node in
the same device container. This way, paired devices that are switched off don't show up with an outdated level.

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
