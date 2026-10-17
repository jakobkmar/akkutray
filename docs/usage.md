# Usage

akkutray has two modes: print the battery levels once, or keep a tray icon running in the background.

```
akkutray [command]

Commands:
  status  Print the battery levels of this computer and connected devices (default)
  tray    Show the battery level as a tray icon in the background
  help    Print this help
```

## Print battery levels

Run `akkutray` (or `akkutray status`):

```powershell
PS> akkutray
Zenbook 14X Space Edition: 80% (charging)
Signature M650: 85%
G915 X LS TKL: 99%
Bose QC Ultra Headphones: 100%
```

Each line is one device with its battery level. `(charging)` is added while a device is plugged in.

- **This computer comes first**, if it has a battery. It is named after the model its firmware reports, or after
  the computer name if the firmware reports none.
- **Other devices** use the name they report themselves.
- **Unclear names** can be replaced by friendlier ones, see [Display names](#display-names).

If no device is found, akkutray prints `No device with battery found` and exits with code 1, so scripts can tell.

## Tray icon

Run `akkutray tray`. The tray icon starts in the background and the command returns at once. Only one tray icon
runs at a time: starting it again just prints `Tray is already running`.

### The icon

The icon is a battery filled to the **lowest** level among all your devices, so it warns you about whichever one
runs out first.

| Color | Lowest level |
| --- | --- |
| Green | above 50% |
| Amber | 21% to 50% |
| Red | 20% or below |
| Empty outline | no device reachable |

### Interacting with it

| Action | What happens |
| --- | --- |
| Hover | A tooltip lists every device with its battery level. |
| Left click | A small window opens at the icon, showing each device with its level and charging state. Click elsewhere or press <kbd>Esc</kbd> to close it. |
| Right click | A menu lists the devices and offers **Refresh** and **Quit**. |

Charging devices get a moving highlight on their bar in the window, unless *Animation effects* are turned off in
the Windows settings.

### When it updates

The battery levels are read again:

- every 5 minutes,
- whenever you open the window,
- when you choose **Refresh**.

### Can't see the icon?

Windows 11 puts new tray icons in the overflow area behind the `^` arrow. To keep the icon visible, drag it onto the
taskbar, or turn it on under *Settings → Personalization → Taskbar → Other system tray icons*.

To start the tray every time you log in, see [Start with Windows](autostart.md).

## Display names

Some devices report names that are hard to recognize, such as `Zenbook UX5401ZAS_UX5401ZAS`. The file
[`data/names.txt`](../data/names.txt) maps them to friendlier names, one per line:

```
Zenbook UX5401ZAS_UX5401ZAS = Zenbook 14X Space Edition
```

The name on the left has to match the reported name exactly. The file is built into the program, so after editing
it, [install](installation.md#update) again.
