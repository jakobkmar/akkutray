# Installation

## Install

1. Install the [Rust toolchain](https://rustup.rs/) if you don't have it yet.
2. In the project folder, run:

   ```powershell
   cargo install --path .
   ```

This builds `akkutray.exe` and puts it in `~/.cargo/bin`. rustup has already added that folder to your `PATH`, so
you can now run `akkutray` from any terminal.

> **Prefer a standalone exe?** Run `cargo build --release` instead and copy `target/release/akkutray.exe` wherever
> you like.

## Update

1. Quit the tray if it is running (right-click the icon → **Quit**). Windows can't replace a running program.
2. Run `cargo install --path .` again.

## Uninstall

1. Quit the tray if it is running.
2. Run `cargo uninstall akkutray`.
3. If you set up [Start at login](autostart.md), undo it.
