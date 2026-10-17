# akkutray

Shows the battery level of your wireless devices on Windows, on the command line or as a lightweight tray icon.

```powershell
cargo install --path .   # install
akkutray                 # print battery levels
akkutray tray            # run the tray icon
```

## Documentation

- [Installation](docs/installation.md): build and install with Cargo
- [Usage](docs/usage.md): command line and tray icon
- [Start with Windows](docs/autostart.md): run the tray at login
- [Supported devices](docs/supported-devices.md): this computer, Logitech devices and Bluetooth headphones
- [Power consumption](docs/power-consumption.md): why it's fine to run all day on a laptop
- [Troubleshooting](docs/troubleshooting.md): missing devices and slow readings
- [How it works](docs/how-it-works.md): the HID++ protocol and code layout
