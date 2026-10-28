# Troubleshooting

## "No device with battery found"

akkutray couldn't reach any device. Check that:

- the device is switched on,
- it is connected through its receiver, by cable or by Bluetooth,
- it is one of the [supported devices](supported-devices.md). Not every Bluetooth device works.

## A device is missing from the list

The device probably reports its battery in a way akkutray doesn't understand yet, for example older G mice that only
report their battery voltage. See [Supported devices](supported-devices.md) for what works.

## The first reading takes about a second

This is normal. A sleeping keyboard or mouse has to wake up before it can answer.

## The computer shows "Charging paused" while plugged in

A charge limit, such as *Battery Health Charging* on ASUS or *Conservation Mode* on Lenovo laptops, holds the battery
below full to make it last longer. Turn it off in the manufacturer's app if you need a full battery.

## The tray icon doesn't show up

Windows 11 hides new tray icons behind the `^` arrow in the taskbar. Drag the icon onto the taskbar, or turn it on
under *Settings → Personalization → Taskbar → Other system tray icons*.

## `cargo install` fails to replace akkutray.exe

The tray is still running, and Windows can't replace a running program. Quit it (right-click the icon → **Quit**)
and run the install again.

## Linux

**Logitech devices are missing.** akkutray needs read and write access to their `/dev/hidraw*` nodes, which only root
has by default. The udev rules of [Solaar](https://github.com/pwr-Solaar/Solaar), which most distributions package,
grant it to the logged-in user.

**The tray icon doesn't show up.** The panel has to show StatusNotifierItem icons. KDE Plasma, COSMIC and most other
panels do, GNOME needs the *AppIndicator and KStatusNotifierItem Support* extension.

**The window doesn't open at the icon.** Where the panel doesn't tell where its icon was clicked, as on COSMIC, the
window opens below the top panel. GNOME doesn't let programs place their windows, so it decides where it opens.
