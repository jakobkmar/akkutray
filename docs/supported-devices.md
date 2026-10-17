# Supported devices

| Device | Works when | Shows charging |
| --- | --- | --- |
| This computer | it has a battery (laptops, tablets) | yes |
| Logitech mice and keyboards | connected by receiver or USB cable | yes |
| Bluetooth headphones and earbuds | they report their battery to Windows | no |

## This computer

The built-in battery of a laptop or tablet, as Windows reports it. Desktop computers without a battery are left out.

## Logitech devices

Works without Logitech G Hub, and can run alongside it.

A Logitech device is supported if both of these are true:

- **It is connected through a receiver or by cable.** LIGHTSPEED, Bolt and Unifying receivers all work, as does a
  USB cable.
- **It reports its battery level in percent.** Technically, this is the HID++ 2.0 feature `0x1004` "Unified
  Battery". The G915 X LIGHTSPEED TKL, for example, has it.

Not supported:

- Logitech devices connected by **Bluetooth**, unless they qualify as a Bluetooth device below.
- Devices that only report their **battery voltage** (feature `0x1001`), such as some older G mice.

## Bluetooth devices

Most headphones and earbuds work: they report their battery level to Windows through the hands-free profile, and
akkutray reads it from there.

Limitations:

- **Always shown as on battery.** The hands-free profile doesn't tell whether a device is charging.
- **Bluetooth Low Energy devices** that only offer the GATT battery service are not supported yet.
