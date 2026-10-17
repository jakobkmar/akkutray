# How it works

Logitech receivers and devices speak the HID++ protocol on a vendor-specific HID interface (usage page `0xFF00`),
which Windows splits into a collection for short reports and one for long reports. akkutray sends requests to
every device slot of every receiver (and to wired devices) and reads the battery status. G Hub talks to the same
interface, so a distinct software id keeps the responses of both programs apart.

The computer's own battery comes from the Windows `GetSystemPowerStatus` API, and its model and manufacturer from the
SMBIOS system information, which Windows mirrors in the registry under `HKLM\HARDWARE\DESCRIPTION\System\BIOS`.

For Bluetooth devices, Windows keeps the battery level as a property on one of the device's profile nodes. akkutray
reads it with the Configuration Manager API and pairs it with the name and connection state of the device node in
the same device container, so paired devices that are switched off don't show up with an outdated level.

The code lives in `src/battery.rs` (common battery type and scan), `src/host.rs` (this computer), `src/hidpp.rs`
(Logitech protocol), `src/bluetooth.rs` (Bluetooth devices), `src/tray.rs` (tray icon), `src/popup.rs` (window, built with
[egui](https://github.com/emilk/egui)) and `src/main.rs` (CLI).
Display names for unclearly named devices are listed in `data/names.txt`, which is compiled into the program.
