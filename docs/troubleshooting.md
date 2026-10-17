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

## The tray icon doesn't show up

Windows 11 hides new tray icons behind the `^` arrow in the taskbar. Drag the icon onto the taskbar, or turn it on
under *Settings → Personalization → Taskbar → Other system tray icons*.

## `cargo install` fails to replace akkutray.exe

The tray is still running, and Windows can't replace a running program. Quit it (right-click the icon → **Quit**)
and run the install again.
