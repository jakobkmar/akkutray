# How it works

Every reading collects batteries from three sources, in this order, then replaces unclear names with the display
names from `data/names.txt`:

1. **This computer**, through Windows
2. **Logitech devices**, through the HID++ protocol
3. **Bluetooth devices**, through the Windows device tree

## This computer

The battery level, charging state and time estimate come from the Windows `GetSystemPowerStatus` API. The capacity,
charge cycles and power flow come from the battery driver through `IOCTL_BATTERY_QUERY_INFORMATION` and
`IOCTL_BATTERY_QUERY_STATUS`. Whether the plugged in charger is strong enough comes from the `PowerManager` of
WinRT. The model and manufacturer come from the SMBIOS system information, which Windows mirrors in the registry under
`HKLM\HARDWARE\DESCRIPTION\System\BIOS`.

## Logitech devices

Logitech receivers and devices speak the HID++ protocol on a vendor-specific HID interface (usage page `0xFF00`).
Windows splits that interface into two collections, one for short reports and one for long reports.

akkutray sends a battery request to every device slot of every receiver, and to every wired device, and reads the
answers. G Hub talks to the same interface, so akkutray uses its own software id to keep its answers apart from
those for G Hub.

As every receiver calls itself "USB Receiver", akkutray names them by their USB product ids.

## Bluetooth devices

Windows stores the battery level of a Bluetooth device, and when it was received, as a property on one of the
device's profile nodes. akkutray reads it with the Configuration Manager API, then takes the name, connection state
and vendor id from the device node in the same device container. Switched-off devices are only listed while their
last level is less than a day old. The manufacturer is looked up by vendor id in `data/vendors.txt`.

## Linux

On Linux, the same sources come from elsewhere:

- **This computer**: the battery from `/sys/class/power_supply`, the model and manufacturer from `/sys/class/dmi/id`.
  Linux keeps no time estimate, so akkutray computes it from the power flow. Nor does it judge the charger, so akkutray
  calls a charger too weak while the battery still discharges on it.
- **Logitech devices**: the same HID++ requests through hidraw, which lists the HID++ interface once for each report
  type, with the same device node.
- **Bluetooth devices**: the `org.bluez.Battery1` interface of BlueZ, over D-Bus. BlueZ drops the level when a device
  disconnects, so only connected devices are listed.

The tray icon is a StatusNotifierItem, and low battery warnings go to `org.freedesktop.Notifications`, both over
D-Bus. Wayland doesn't let windows place themselves, so there the popup is a layer shell surface, like the panels' own
popups. It docks to the screen edge nearest to where the tray icon was clicked, or below the top panel where the tray
host doesn't tell, as COSMIC's. On X11, the popup is a window at the click, within the work area from
`_NET_WORKAREA`. As its size is fixed, tiling window managers such as i3 float it. GNOME, which lacks the layer shell,
places it itself.

## Code layout

| File | Contents |
| --- | --- |
| `src/main.rs` | Command line |
| `src/battery.rs` | Common battery type, scan of all sources and display names |
| `src/host.rs` | This computer |
| `src/hidpp.rs` | Logitech HID++ protocol |
| `src/bluetooth.rs` | Bluetooth devices |
| `src/tray.rs` | Tray icon, menu, update timer and low battery warnings |
| `src/popup.rs` | Window opened by a left click, built with [egui](https://github.com/emilk/egui) |
| `src/platform/mod.rs` | Picks the code of the platform the program is built for |
| `src/platform/windows/` | All calls to Windows, in one file for each of the modules above that needs them |
| `src/platform/linux/` | The same for Linux |
| `data/names.txt` | Display names for unclearly named devices, compiled into the program |
| `data/vendors.txt` | Manufacturer names of Bluetooth devices by vendor id, compiled into the program |
