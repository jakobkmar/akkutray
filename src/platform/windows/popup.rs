//! What egui doesn't cover for the popup: its corners, the work area of the screen, the fonts of Windows and its
//! animation setting.

use crate::popup::{Bounds, Font};
use eframe::egui;
use raw_window_handle::{HasWindowHandle, RawWindowHandle};
use std::path::PathBuf;
use windows_sys::Win32::Foundation::{HWND, POINT};
use windows_sys::Win32::Graphics::Dwm::{DWMWA_WINDOW_CORNER_PREFERENCE, DWMWCP_ROUND, DwmSetWindowAttribute};
use windows_sys::Win32::Graphics::Gdi::{GetMonitorInfoW, MONITOR_DEFAULTTONEAREST, MONITORINFO, MonitorFromPoint};
use windows_sys::Win32::UI::WindowsAndMessaging::{SPI_GETCLIENTAREAANIMATION, SystemParametersInfoW};

/// Glyphs of the Segoe Fluent Icons font, which `font` reads for `Font::Icons`.
pub mod glyph {
    pub const REFRESH: &str = "\u{e72c}";
    pub const CHARGING: &str = "\u{e945}";
    pub const LAPTOP: &str = "\u{e7f8}";
    pub const KEYBOARD: &str = "\u{e765}";
    pub const MOUSE: &str = "\u{e962}";
    pub const HEADSET: &str = "\u{e7f6}";
    pub const BATTERY: &str = "\u{e83f}";
    pub const USB: &str = "\u{e88e}";
    pub const BLUETOOTH: &str = "\u{e702}";
    pub const CHECK: &str = "\u{e73e}";
    pub const WARNING: &str = "\u{e7ba}";
    pub const CHEVRON_RIGHT: &str = "\u{e76c}";
    pub const BACK: &str = "\u{e72b}";
}

/// Windows 11 gives frameless windows square corners unless asked otherwise.
pub fn round_corners(window: &impl HasWindowHandle) {
    let Ok(RawWindowHandle::Win32(handle)) = window.window_handle().map(|handle| handle.as_raw()) else {
        return;
    };
    unsafe {
        DwmSetWindowAttribute(
            handle.hwnd.get() as HWND,
            DWMWA_WINDOW_CORNER_PREFERENCE as u32,
            (&DWMWCP_ROUND as *const i32).cast(),
            4,
        )
    };
}

/// The work area of the screen nearest to a point, which leaves out the taskbar.
pub fn work_area(x: i32, y: i32) -> Option<Bounds> {
    let mut monitor = MONITORINFO {
        cbSize: size_of::<MONITORINFO>() as u32,
        ..Default::default()
    };
    unsafe { GetMonitorInfoW(MonitorFromPoint(POINT { x, y }, MONITOR_DEFAULTTONEAREST), &mut monitor) };
    let area = monitor.rcWork;
    Some(Bounds {
        x: area.left,
        y: area.top,
        width: area.right - area.left,
        height: area.bottom - area.top,
    })
}

/// Whether the "Animation effects" setting of Windows is on.
pub fn animations_enabled() -> bool {
    let mut enabled = 1i32;
    unsafe { SystemParametersInfoW(SPI_GETCLIENTAREAANIMATION, 0, (&mut enabled as *mut i32).cast(), 0) };
    enabled != 0
}

/// The light or dark mode, which winit already reads from Windows.
pub fn theme() -> Option<egui::Theme> {
    None
}

/// A popup docked to the taskbar by the system, which Windows doesn't offer. The popup places itself instead.
pub enum Dock {}

impl Dock {
    pub fn new(_icon: (i32, i32)) -> Option<Self> {
        None
    }

    pub fn run(self, _ctx: &egui::Context, _background: [f32; 4], _ui: impl FnMut(&mut egui::Ui)) {
        match self {}
    }
}

/// Reads a font of the Windows UI.
pub fn font(font: Font) -> Option<Vec<u8>> {
    let files: &[&str] = match font {
        Font::Text => &["segoeui.ttf"],
        Font::Semibold => &["seguisb.ttf"],
        // Segoe Fluent Icons. Windows 10 lacks it but has its predecessor with the same glyphs.
        Font::Icons => &["SegoeIcons.ttf", "segmdl2.ttf"],
    };
    let dir = PathBuf::from(std::env::var_os("WINDIR")?).join("Fonts");
    files.iter().find_map(|file| std::fs::read(dir.join(file)).ok())
}
