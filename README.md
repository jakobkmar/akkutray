# akkutray

Shows the battery level of your wireless devices on Windows, either on the command line or as a lightweight tray
icon.

It currently supports Logitech devices, without needing Logitech G Hub: any device that reports its battery via
HID++ 2.0 (feature `0x1004` "Unified Battery"), connected through a LIGHTSPEED, Bolt or Unifying receiver or by USB
cable, such as the G915 X LIGHTSPEED TKL. It can run alongside G Hub.

## Installation

Requires the [Rust toolchain](https://rustup.rs/).

```powershell
cargo install --path .
```

This builds `akkutray.exe` and puts it in `%USERPROFILE%\.cargo\bin`, which rustup adds to your `PATH`.
Alternatively run `cargo build --release` and copy `target\release\akkutray.exe` wherever you like.

## Usage

```
akkutray [command]

Commands:
  status  Print the battery level of connected Logitech devices (default)
  tray    Show the battery level as a tray icon in the background
  help    Print this help
```

### Command line

```powershell
PS> akkutray
Signature M650: 85%
G915 X LS TKL: 99%
```

The device names are the ones the devices report. ` (charging)` is appended while a device is plugged in.
If no device is found, the exit code is 1.

### Tray icon

```powershell
akkutray tray
```

This starts the tray icon in the background and returns at once. Only one tray icon runs at a time; starting it
again just prints `Tray is already running`.

- **Icon**: a battery filled to the lowest level of all devices. It is green above 50%, amber down to 21% and red at
  20% or below, and shows an empty outline when no device is reachable.
- **Hover**: the tooltip lists every device with its battery level.
- **Left click**: a small window opens right at the tray icon, showing each device with its battery level and
  charging state. It closes when you click elsewhere or press <kbd>Esc</kbd>.
- **Right click**: a menu lists the devices and offers **Refresh** and **Quit**.

The battery level is updated every 5 minutes, whenever the window opens and when you choose **Refresh**.

> Windows 11 puts new tray icons in the overflow area (the `^` arrow). To keep the icon visible, drag it onto
> the taskbar or enable it under *Settings → Personalization → Taskbar → Other system tray icons*.

### Start with Windows

1. Press <kbd>Win</kbd>+<kbd>R</kbd>, enter `shell:startup` and press Enter.
2. In the folder that opens, right-click → *New → Shortcut* and enter the target
   `"%USERPROFILE%\.cargo\bin\akkutray.exe" tray` (or the path you copied the exe to).

On Windows 11 24H2 and later no console window appears. On older versions a console window flashes briefly at
login while the tray moves to the background.

## Power consumption

The tray is designed to run on a laptop all day:

- It does nothing between updates: no polling loops, only a blocking Windows message loop.
- The window exists only while it is open. Closing it destroys the window and its graphics context.
- Updates run every 5 minutes on a coalescable timer, so Windows can batch the wake-up with other timers. An update
  takes a few milliseconds of CPU and some short radio messages to the devices.

## Troubleshooting

- **"No Logitech device with battery found"**: check that the device is switched on and connected through its
  receiver or by cable. Devices connected by Bluetooth are not supported.
- **The first reading takes about a second**: a sleeping keyboard first has to wake up to answer. This is normal.
- **A device does not show up**: it probably reports its battery in a different way (e.g. older G mice that only
  report voltage, feature `0x1001`). Only the Unified Battery feature is supported.

## How it works

Logitech receivers and devices speak the HID++ protocol on a vendor-specific HID interface (usage page `0xFF00`),
which Windows splits into a collection for short reports and one for long reports. `akkutray` sends requests to
every device slot of every receiver (and to wired devices) and reads the battery status. G Hub talks to the same
interface, so a distinct software id keeps the responses of both programs apart.

The code lives in `src/hidpp.rs` (protocol), `src/tray.rs` (tray icon), `src/popup.rs` (window, built with
[egui](https://github.com/emilk/egui)) and `src/main.rs` (CLI).
