# Usage

```
akkutray [command]

Commands:
  status  Print the battery level of connected Logitech devices (default)
  tray    Show the battery level as a tray icon in the background
  help    Print this help
```

## Command line

```powershell
PS> akkutray
Signature M650: 85%
G915 X LS TKL: 99%
```

The device names are the ones the devices report. ` (charging)` is appended while a device is plugged in.
If no device is found, the exit code is 1.

## Tray icon

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

To start the tray automatically, see [Start with Windows](autostart.md).
