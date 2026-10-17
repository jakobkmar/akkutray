use crate::hidpp::{Battery, Kind};
use crate::tray::Shared;
use eframe::egui::{
    self, Align, Align2, Color32, FontData, FontDefinitions, FontFamily, FontId, Frame, Key, Layout, RichText, Sense,
    Vec2, ViewportCommand,
};
use raw_window_handle::{HasWindowHandle, RawWindowHandle};
use std::ptr::null_mut;
use std::sync::Arc;
use std::time::Duration;
use windows_sys::Win32::Foundation::{HWND, POINT};
use windows_sys::Win32::Graphics::Dwm::{DWMWA_WINDOW_CORNER_PREFERENCE, DWMWCP_ROUND, DwmSetWindowAttribute};
use windows_sys::Win32::Graphics::Gdi::{GetMonitorInfoW, MONITOR_DEFAULTTONEAREST, MONITORINFO, MonitorFromPoint};
use windows_sys::Win32::UI::WindowsAndMessaging::{SWP_NOACTIVATE, SWP_NOZORDER, SetWindowPos};

const WIDTH: f32 = 340.0;
/// Minimum gap between the popup and the edges of the screen's work area, in points.
const SCREEN_MARGIN: f32 = 12.0;
const LOW_PERCENT: u8 = 20;
const MEDIUM_PERCENT: u8 = 50;

// Glyphs of the Segoe Fluent Icons font.
const REFRESH: &str = "\u{e72c}";
const CHARGING: &str = "\u{e945}";
const KEYBOARD: &str = "\u{e765}";
const MOUSE: &str = "\u{e962}";
const HEADSET: &str = "\u{e7f6}";
const BATTERY: &str = "\u{e83f}";

/// Color for a battery level, shared with the tray icon.
pub fn level_color(percent: u8) -> [u8; 3] {
    match percent {
        p if p <= LOW_PERCENT => [0xe5, 0x53, 0x4b],
        p if p <= MEDIUM_PERCENT => [0xd2, 0x99, 0x22],
        _ => [0x2e, 0xa0, 0x43],
    }
}

/// Opens the popup next to the tray icon at `anchor` and blocks until it is closed.
pub fn show(shared: &Shared, anchor: tray_icon::Rect) -> eframe::Result {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("akkutray")
            .with_inner_size([WIDTH, 200.0])
            .with_decorations(false)
            .with_resizable(false)
            .with_taskbar(false)
            .with_always_on_top(),
        ..Default::default()
    };
    eframe::run_native(
        "akkutray",
        options,
        Box::new(|cc| Ok(Box::new(Popup::new(cc, shared, anchor)))),
    )
}

struct Popup<'a> {
    shared: &'a Shared,
    anchor: tray_icon::Rect,
    hwnd: HWND,
    bounds: Option<(i32, i32, i32, i32)>,
    had_focus: bool,
}

impl<'a> Popup<'a> {
    fn new(cc: &eframe::CreationContext, shared: &'a Shared, anchor: tray_icon::Rect) -> Self {
        cc.egui_ctx.set_fonts(fonts());
        cc.egui_ctx.send_viewport_cmd(ViewportCommand::Focus);
        let hwnd = match cc.window_handle().map(|handle| handle.as_raw()) {
            Ok(RawWindowHandle::Win32(handle)) => handle.hwnd.get() as HWND,
            _ => null_mut(),
        };
        // Windows 11 gives frameless windows square corners unless asked otherwise.
        unsafe {
            DwmSetWindowAttribute(
                hwnd,
                DWMWA_WINDOW_CORNER_PREFERENCE as u32,
                (&DWMWCP_ROUND as *const i32).cast(),
                4,
            )
        };
        shared.lock().popup = Some(cc.egui_ctx.clone());
        shared.refresh();
        Self {
            shared,
            anchor,
            hwnd,
            bounds: None,
            had_focus: false,
        }
    }

    /// Centers the popup on the tray icon, kept within the screen's work area. As the taskbar lies outside of the
    /// work area, this puts the popup right next to the icon, whichever screen edge the taskbar is on.
    fn place(&mut self, size: Vec2, pixels_per_point: f32) {
        let icon = POINT {
            x: (self.anchor.position.x + f64::from(self.anchor.size.width) / 2.0) as i32,
            y: (self.anchor.position.y + f64::from(self.anchor.size.height) / 2.0) as i32,
        };
        let mut monitor = MONITORINFO {
            cbSize: size_of::<MONITORINFO>() as u32,
            ..Default::default()
        };
        unsafe { GetMonitorInfoW(MonitorFromPoint(icon, MONITOR_DEFAULTTONEAREST), &mut monitor) };
        let area = monitor.rcWork;
        let margin = (SCREEN_MARGIN * pixels_per_point) as i32;
        let (width, height) = (
            (size.x * pixels_per_point).round() as i32,
            (size.y * pixels_per_point).round() as i32,
        );
        let x = (icon.x - width / 2)
            .min(area.right - width - margin)
            .max(area.left + margin);
        let y = (icon.y - height / 2)
            .min(area.bottom - height - margin)
            .max(area.top + margin);
        if self.bounds != Some((x, y, width, height)) {
            self.bounds = Some((x, y, width, height));
            unsafe {
                SetWindowPos(
                    self.hwnd,
                    null_mut(),
                    x,
                    y,
                    width,
                    height,
                    SWP_NOZORDER | SWP_NOACTIVATE,
                )
            };
        }
    }
}

impl eframe::App for Popup<'_> {
    fn clear_color(&self, visuals: &egui::Visuals) -> [f32; 4] {
        Palette::new(visuals.dark_mode).background.to_normalized_gamma_f32()
    }

    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        let focused = ctx.input(|input| input.viewport().focused);
        self.had_focus |= focused == Some(true);
        if ctx.input(|input| input.key_pressed(Key::Escape)) || self.had_focus && focused == Some(false) {
            ctx.send_viewport_cmd(ViewportCommand::Close);
        }

        let palette = Palette::new(ui.visuals().dark_mode);
        let (batteries, updated, updating) = {
            let state = self.shared.lock();
            (state.batteries.clone(), state.updated, state.updating)
        };
        let status = match updated {
            None => "Looking for devices…".to_string(),
            Some(time) if time.elapsed() < Duration::from_secs(60) => "Updated just now".to_string(),
            Some(time) => format!("Updated {} min ago", time.elapsed().as_secs() / 60),
        };

        let content = Frame::new().inner_margin(16).show(ui, |ui| {
            ui.set_width(WIDTH - 32.0);
            ui.spacing_mut().item_spacing = Vec2::new(8.0, 10.0);
            ui.horizontal(|ui| {
                ui.vertical(|ui| {
                    ui.spacing_mut().item_spacing.y = 2.0;
                    ui.label(
                        RichText::new("Battery")
                            .family(semibold())
                            .size(18.0)
                            .color(palette.text),
                    );
                    ui.label(RichText::new(status).size(12.0).color(palette.weak));
                });
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    if updating {
                        ui.add(egui::Spinner::new().size(16.0).color(palette.weak));
                    } else if ui
                        .add(egui::Button::new(icon(REFRESH, 14.0).color(palette.weak)).frame(false))
                        .on_hover_text("Refresh")
                        .clicked()
                    {
                        self.shared.refresh();
                    }
                });
            });
            ui.add_space(4.0);

            if batteries.is_empty() && !updating {
                card(ui, &palette, |ui| {
                    ui.label(
                        RichText::new("No device found")
                            .family(semibold())
                            .size(14.0)
                            .color(palette.text),
                    );
                    ui.label(
                        RichText::new("Make sure your Logitech device is switched on and connected to its receiver.")
                            .size(12.0)
                            .color(palette.weak),
                    );
                });
            }
            for battery in &batteries {
                device(ui, &palette, battery);
            }
        });
        self.place(content.response.rect.size(), ctx.pixels_per_point());
        // Keeps "Updated … ago" current.
        ctx.request_repaint_after(Duration::from_secs(60));
    }

    fn on_exit(&mut self, _gl: Option<&eframe::glow::Context>) {
        self.shared.lock().popup = None;
    }
}

fn device(ui: &mut egui::Ui, palette: &Palette, battery: &Battery) {
    let [r, g, b] = level_color(battery.percent);
    let color = Color32::from_rgb(r, g, b);
    card(ui, palette, |ui| {
        ui.horizontal(|ui| {
            let (badge, _) = ui.allocate_exact_size(Vec2::splat(36.0), Sense::hover());
            ui.painter()
                .circle_filled(badge.center(), 18.0, color.gamma_multiply(0.15));
            let glyph = match battery.kind {
                Kind::Keyboard => KEYBOARD,
                Kind::Mouse => MOUSE,
                Kind::Headset => HEADSET,
                Kind::Other => BATTERY,
            };
            let font = FontId::new(16.0, icons());
            ui.painter()
                .text(badge.center(), Align2::CENTER_CENTER, glyph, font, color);

            ui.vertical(|ui| {
                ui.spacing_mut().item_spacing = Vec2::new(4.0, 2.0);
                ui.label(
                    RichText::new(&battery.name)
                        .family(semibold())
                        .size(14.0)
                        .color(palette.text),
                );
                let source = [battery.manufacturer.as_str(), battery.connection.as_str()];
                let source = source
                    .into_iter()
                    .filter(|part| !part.is_empty())
                    .collect::<Vec<_>>()
                    .join(" · ");
                ui.label(RichText::new(source).size(12.0).color(palette.weak));
                ui.horizontal(|ui| {
                    if battery.charging {
                        ui.label(icon(CHARGING, 11.0).color(color));
                    }
                    let state = if battery.charging { "Charging" } else { "On battery" };
                    ui.label(RichText::new(state).size(12.0).color(palette.weak));
                });
            });
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                let percent = RichText::new(format!("{}%", battery.percent))
                    .family(semibold())
                    .size(22.0);
                ui.label(percent.color(palette.text));
            });
        });

        let (bar, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 6.0), Sense::hover());
        ui.painter().rect_filled(bar, 3.0, palette.track);
        let mut fill = bar;
        fill.set_width(bar.width() * f32::from(battery.percent) / 100.0);
        ui.painter().rect_filled(fill, 3.0, color);
    });
}

fn card(ui: &mut egui::Ui, palette: &Palette, content: impl FnOnce(&mut egui::Ui)) {
    Frame::new()
        .fill(palette.card)
        .corner_radius(10)
        .inner_margin(14)
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            content(ui);
        });
}

/// Neutral colors in the style of Windows 11 flyouts.
struct Palette {
    background: Color32,
    card: Color32,
    text: Color32,
    weak: Color32,
    track: Color32,
}

impl Palette {
    fn new(dark: bool) -> Self {
        if dark {
            Self {
                background: Color32::from_rgb(0x20, 0x20, 0x20),
                card: Color32::from_rgb(0x2b, 0x2b, 0x2b),
                text: Color32::from_rgb(0xff, 0xff, 0xff),
                weak: Color32::from_rgb(0xa0, 0xa0, 0xa0),
                track: Color32::from_rgb(0x3d, 0x3d, 0x3d),
            }
        } else {
            Self {
                background: Color32::from_rgb(0xf3, 0xf3, 0xf3),
                card: Color32::from_rgb(0xff, 0xff, 0xff),
                text: Color32::from_rgb(0x1a, 0x1a, 0x1a),
                weak: Color32::from_rgb(0x6b, 0x6b, 0x6b),
                track: Color32::from_rgb(0xe3, 0xe3, 0xe3),
            }
        }
    }
}

fn semibold() -> FontFamily {
    FontFamily::Name("semibold".into())
}

fn icons() -> FontFamily {
    FontFamily::Name("icons".into())
}

fn icon(glyph: &str, size: f32) -> RichText {
    RichText::new(glyph).family(icons()).size(size)
}

/// Uses the Windows UI fonts, falling back to egui's bundled fonts where they are missing.
fn fonts() -> FontDefinitions {
    let mut fonts = FontDefinitions::default();
    let fallbacks = fonts.families[&FontFamily::Proportional].clone();
    fonts.families.insert(semibold(), fallbacks.clone());
    fonts.families.insert(icons(), fallbacks);

    let dir = std::path::PathBuf::from(std::env::var_os("WINDIR").unwrap_or_default()).join("Fonts");
    // Windows 10 lacks Segoe Fluent Icons but has its predecessor with the same glyphs.
    let sources = [
        (&["segoeui.ttf"][..], FontFamily::Proportional),
        (&["seguisb.ttf"], semibold()),
        (&["SegoeIcons.ttf", "segmdl2.ttf"], icons()),
    ];
    for (files, family) in sources {
        if let Some((file, data)) = files
            .iter()
            .find_map(|file| Some((*file, std::fs::read(dir.join(file)).ok()?)))
        {
            fonts
                .font_data
                .insert(file.into(), Arc::new(FontData::from_owned(data)));
            fonts.families.entry(family).or_default().insert(0, file.into());
        }
    }
    fonts
}
