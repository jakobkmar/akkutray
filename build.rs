fn main() {
    // On Windows 11 24H2 and later, the manifest stops Windows from opening a console window when the tray is
    // started from Explorer or autostart, while the CLI keeps using the console of the terminal it runs in.
    if std::env::var_os("CARGO_CFG_WINDOWS").is_some() {
        embed_manifest::embed_manifest_file("akkutray.manifest").expect("failed to embed the manifest");
    }
    println!("cargo:rerun-if-changed=akkutray.manifest");
}
