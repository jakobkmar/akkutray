# Supported devices

| Device | Works when | Shows charging | Extras |
| --- | --- | --- | --- |
| This computer | it has a battery (laptops, tablets) | yes | time estimate, battery health |
| Logitech mice and keyboards | connected by receiver or USB cable | yes | replaceable batteries marked |
| Bluetooth headphones and earbuds | they report their battery to Windows | no | last level after disconnecting |

## This computer

The built-in battery of a laptop or tablet, as Windows reports it. Desktop computers without a battery are left out.

Battery health needs a battery that reports its capacity in watt-hours, as nearly all laptop batteries do. Charge
cycles only show if the battery counts them.

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

- **No power state.** The hands-free profile doesn't tell whether a device is charging.
- **Manufacturer.** It is shown for the brands listed in [`data/vendors.txt`](../data/vendors.txt). Add a line there
  for any brand that is missing. The Device Manager shows the vendor id in the *Hardware Ids* of the device's
  profiles: `VID&0001009E` means Bluetooth SIG (`0001`) id `0x009E`, `VID&0002…` a USB-IF id.
- **Bluetooth Low Energy devices** that only offer the GATT battery service are not supported yet.
