//! What egui doesn't cover for the popup: the work area of X11, the desktop's light or dark mode and its fonts.

use crate::popup::{Bounds, Font};
use eframe::egui;
use raw_window_handle::HasWindowHandle;
use std::process::Command;
use x11rb::connection::Connection;
use x11rb::protocol::xproto::{AtomEnum, ConnectionExt};

/// Glyphs of egui's bundled emoji fonts, as Linux has no common icon font.
pub mod glyph {
    pub const REFRESH: &str = "🔄";
    pub const CHARGING: &str = "⚡";
    pub const LAPTOP: &str = "💻";
    pub const KEYBOARD: &str = "⌨";
    pub const MOUSE: &str = "🖱";
    pub const HEADSET: &str = "🎧";
    pub const BATTERY: &str = "🔋";
    pub const USB: &str = "🔌";
    // The emoji fonts lack the Bluetooth logo.
    pub const BLUETOOTH: &str = "📶";
    pub const CHECK: &str = "✔";
    pub const WARNING: &str = "⚠";
    pub const CHEVRON_RIGHT: &str = "⏵";
    pub const BACK: &str = "⏴";
}

/// The window manager decides the corners.
pub fn round_corners(_window: &impl HasWindowHandle) {}

/// The work area of the X11 desktop, which leaves out the panels. X11 keeps one for all screens together. Wayland,
/// which winit prefers, doesn't let windows place themselves, so the system places the popup there.
pub fn work_area(_x: i32, _y: i32) -> Option<Bounds> {
    if std::env::var_os("WAYLAND_DISPLAY").is_some() {
        return None;
    }
    let (connection, screen) = x11rb::connect(None).ok()?;
    let root = connection.setup().roots.get(screen)?.root;
    let atom = connection.intern_atom(true, b"_NET_WORKAREA").ok()?.reply().ok()?.atom;
    // The work area of the first virtual desktop, as desktops share their panels.
    let reply = connection
        .get_property(false, root, atom, AtomEnum::CARDINAL, 0, 4)
        .ok()?
        .reply()
        .ok()?;
    let [x, y, width, height] = reply.value32()?.collect::<Vec<_>>()[..] else {
        return None;
    };
    Some(Bounds {
        x: x as i32,
        y: y as i32,
        width: width as i32,
        height: height as i32,
    })
}

/// Linux desktops share no setting for animations.
pub fn animations_enabled() -> bool {
    true
}

/// The desktop's light or dark mode, from the XDG desktop portal, as winit doesn't read it on Linux.
pub fn theme() -> Option<egui::Theme> {
    let connection = zbus::blocking::Connection::session().ok()?;
    let reply = connection
        .call_method(
            Some("org.freedesktop.portal.Desktop"),
            "/org/freedesktop/portal/desktop",
            Some("org.freedesktop.portal.Settings"),
            "ReadOne",
            &("org.freedesktop.appearance", "color-scheme"),
        )
        .ok()?;
    // 1 prefers dark, 2 prefers light, 0 has no preference.
    match u32::try_from(reply.body().deserialize::<zbus::zvariant::OwnedValue>().ok()?).ok()? {
        1 => Some(egui::Theme::Dark),
        2 => Some(egui::Theme::Light),
        _ => None,
    }
}

/// Reads the font that fontconfig picks for the desktop's sans-serif font.
pub fn font(font: Font) -> Option<Vec<u8>> {
    let pattern = match font {
        Font::Text => "sans-serif",
        Font::Semibold => "sans-serif:semibold",
        Font::Icons => return None,
    };
    let output = Command::new("fc-match")
        .args(["--format=%{file}", pattern])
        .output()
        .ok()?;
    std::fs::read(String::from_utf8(output.stdout).ok()?).ok()
}
