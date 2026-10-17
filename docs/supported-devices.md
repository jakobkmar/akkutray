# Supported devices

## This computer

The built-in battery of a laptop or tablet, as reported by Windows. Computers without a battery are left out.

## Logitech devices

akkutray reads Logitech devices without needing Logitech G Hub, and can run alongside it.

A device works if it reports its battery via HID++ 2.0 (feature `0x1004` "Unified Battery") and is connected
through a LIGHTSPEED, Bolt or Unifying receiver or by USB cable, such as the G915 X LIGHTSPEED TKL.

Not supported:

- Devices connected by Bluetooth.
- Devices that only report their battery voltage (feature `0x1001`), such as some older G mice.
