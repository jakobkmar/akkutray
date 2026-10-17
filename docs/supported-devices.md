# Supported devices

akkutray currently supports Logitech devices, without needing Logitech G Hub. It can run alongside G Hub.

A device works if it reports its battery via HID++ 2.0 (feature `0x1004` "Unified Battery") and is connected
through a LIGHTSPEED, Bolt or Unifying receiver or by USB cable, such as the G915 X LIGHTSPEED TKL.

Not supported:

- Devices connected by Bluetooth.
- Devices that only report their battery voltage (feature `0x1001`), such as some older G mice.
