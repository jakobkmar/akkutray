# Installation

Requires the [Rust toolchain](https://rustup.rs/).

```powershell
cargo install --path .
```

This builds `akkutray.exe` and puts it in `~/.cargo/bin`, which rustup adds to your `PATH`.
Alternatively run `cargo build --release` and copy `target/release/akkutray.exe` wherever you like.

To update, run `cargo install --path .` again. Quit a running tray first, as Windows can't replace a running program.
