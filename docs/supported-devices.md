# Supported devices

## This computer

The built-in battery of a laptop or tablet, as reported by Windows. Computers without a battery are left out.

## Logitech devices

akkutray reads Logitech devices without needing Logitech G Hub, and can run alongside it.

A device works if it reports its battery via HID++ 2.0 (feature `0x1004` "Unified Battery") and is connected
through a LIGHTSPEED, Bolt or Unifying receiver or by USB cable, such as the G915 X LIGHTSPEED TKL.

Not supported:

- Logitech devices connected by Bluetooth, unless they are covered by the section below.
- Devices that only report their battery voltage (feature `0x1001`), such as some older G mice.

## Bluetooth devices

Connected Bluetooth devices that report their battery level to Windows via the hands-free profile, such as most
headphones and earbuds.

The hands-free profile doesn't report whether a device is charging, so Bluetooth devices always show as on battery.
Bluetooth Low Energy devices that only offer the GATT battery service are not supported yet.
