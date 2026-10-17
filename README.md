# akkutray

See the battery level of your laptop, Logitech mouse and keyboard, and Bluetooth headphones at a glance, from
the command line or from a small tray icon on Windows.

<p align="center">
  <img src="docs/images/screenshot-tray-ui.png" width="360"
       alt="The akkutray window open above the Windows taskbar, listing a laptop, a Logitech mouse and keyboard, and Bluetooth headphones, each with its battery level and charging state">
</p>

Clicking the tray icon opens the window above. On the command line, `akkutray` prints the same readings:

```text
Zenbook 14X Space Edition: 80% (charging)
Signature M650: 85%
G915 X LS TKL: 99%
Bose QC Ultra Headphones: 100%
```

- **No Logitech G Hub needed.** akkutray talks to Logitech receivers directly, and can run next to G Hub.
- **One place for all batteries.** The computer itself, Logitech devices and Bluetooth headphones.
- **Light enough to run all day.** The tray sleeps between updates every 5 minutes.

## Quick start

You need Windows and the [Rust toolchain](https://rustup.rs/).

```powershell
cargo install --path .   # build and install
akkutray                 # print battery levels once
akkutray tray            # show a tray icon in the background
```

To have the tray icon appear every time you log in, see [Start with Windows](docs/autostart.md).

## Documentation

**Getting started**

- [Installation](docs/installation.md): install, update and uninstall
- [Usage](docs/usage.md): the command line and the tray icon
- [Start with Windows](docs/autostart.md): start the tray at login

**Reference**

- [Supported devices](docs/supported-devices.md): which devices work and which don't
- [Troubleshooting](docs/troubleshooting.md): missing devices, hidden icon, slow readings
- [Power consumption](docs/power-consumption.md): why the tray is fine on a laptop

**Under the hood**

- [How it works](docs/how-it-works.md): where the readings come from and how the code is laid out
