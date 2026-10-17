# How it works

Logitech receivers and devices speak the HID++ protocol on a vendor-specific HID interface (usage page `0xFF00`),
which Windows splits into a collection for short reports and one for long reports. akkutray sends requests to
every device slot of every receiver (and to wired devices) and reads the battery status. G Hub talks to the same
interface, so a distinct software id keeps the responses of both programs apart.

The code lives in `src/hidpp.rs` (protocol), `src/tray.rs` (tray icon), `src/popup.rs` (window, built with
[egui](https://github.com/emilk/egui)) and `src/main.rs` (CLI).
