use crate::battery::{Battery, Kind, Link, Pack, Power};
use crate::platform;
use crate::platform::popup::glyph::{
    BACK, BATTERY, BLUETOOTH, CHARGING, CHECK, CHEVRON_RIGHT, HEADSET, KEYBOARD, LAPTOP, MOUSE, REFRESH, USB, WARNING,
};
use crate::tray::Shared;
use eframe::egui::emath::easing;
use eframe::egui::epaint::PathStroke;
use eframe::egui::text::{LayoutJob, TextFormat};
use eframe::egui::{
    self, Align, Align2, Color32, CursorIcon, FontData, FontDefinitions, FontFamily, FontId, Frame, Id, InnerResponse,
    Key, Layout, Margin, Pos2, Rect, RichText, Sense, Stroke, UiBuilder, Vec2, ViewportCommand, pos2,
};
use std::f32::consts::{FRAC_PI_2, TAU};
use std::sync::Arc;
use std::time::{Duration, Instant};

const WIDTH: f32 = 340.0;
/// Minimum gap between the popup and the edges of the screen's work area, in points.
const SCREEN_MARGIN: f32 = 12.0;
/// Level at or below which a battery counts as low, shared with the tray's warnings.
pub const LOW_PERCENT: u8 = 20;
const MEDIUM_PERCENT: u8 = 50;
/// Seconds the details page takes to slide in or out.
const SLIDE_TIME: f32 = 0.25;
/// Room given to a page to lay itself out, far more than any page needs.
const PAGE_MAX_HEIGHT: f32 = 4000.0;

/// A rectangle on the screen, in physical pixels.
#[derive(Clone, Copy, PartialEq)]
pub struct Bounds {
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
}

/// The fonts of the popup, which the system provides.
pub enum Font {
    Text,
    Semibold,
    /// The font of the glyphs in `platform::popup::glyph`.
    Icons,
}

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
    bounds: Option<Bounds>,
    had_focus: bool,
    animate: bool,
    /// Whether the details page is shown or slides in, rather than the list.
    details: bool,
}

/// What a click in the popup asks for.
enum Action {
    Refresh,
    /// Shows the details page.
    Open,
    Back,
}

impl<'a> Popup<'a> {
    fn new(cc: &eframe::CreationContext, shared: &'a Shared, anchor: tray_icon::Rect) -> Self {
        cc.egui_ctx.set_fonts(fonts());
        cc.egui_ctx.send_viewport_cmd(ViewportCommand::Focus);
        platform::popup::round_corners(cc);
        let animate = platform::popup::animations_enabled();
        cc.egui_ctx.all_styles_mut(|style| {
            // Clicking a card opens its details, so its texts shouldn't take the click to select themselves.
            style.interaction.selectable_labels = false;
            if !animate {
                style.animation_time = 0.0;
            }
        });
        shared.lock().popup = Some(cc.egui_ctx.clone());
        shared.refresh();
        Self {
            shared,
            anchor,
            bounds: None,
            had_focus: false,
            animate,
            details: false,
        }
    }

    /// Centers the popup on the tray icon, kept within the screen's work area. As the taskbar lies outside of the
    /// work area, this puts the popup right next to the icon, whichever screen edge the taskbar is on.
    fn place(&mut self, ctx: &egui::Context, size: Vec2) {
        let pixels_per_point = ctx.pixels_per_point();
        let icon_x = (self.anchor.position.x + f64::from(self.anchor.size.width) / 2.0) as i32;
        let icon_y = (self.anchor.position.y + f64::from(self.anchor.size.height) / 2.0) as i32;
        let area = platform::popup::work_area(icon_x, icon_y);
        let margin = (SCREEN_MARGIN * pixels_per_point) as i32;
        let (width, height) = (
            (size.x * pixels_per_point).round() as i32,
            (size.y * pixels_per_point).round() as i32,
        );
        let bounds = Bounds {
            x: (icon_x - width / 2)
                .min(area.x + area.width - width - margin)
                .max(area.x + margin),
            y: (icon_y - height / 2)
                .min(area.y + area.height - height - margin)
                .max(area.y + margin),
            width,
            height,
        };
        if self.bounds != Some(bounds) {
            self.bounds = Some(bounds);
            // egui takes points, which it turns back into these pixels.
            let points = |pixels: i32| pixels as f32 / pixels_per_point;
            ctx.send_viewport_cmd(ViewportCommand::OuterPosition(pos2(points(bounds.x), points(bounds.y))));
            ctx.send_viewport_cmd(ViewportCommand::InnerSize(Vec2::new(points(width), points(height))));
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
        if self.had_focus && focused == Some(false) {
            ctx.send_viewport_cmd(ViewportCommand::Close);
        }
        // Escape leads back from the details page first.
        if ctx.input(|input| input.key_pressed(Key::Escape)) {
            if self.details {
                self.details = false;
            } else {
                ctx.send_viewport_cmd(ViewportCommand::Close);
            }
        }

        let palette = Palette::new(ui.visuals().dark_mode);
        let (batteries, updated, updating) = {
            let state = self.shared.lock();
            (state.batteries.clone(), state.updated, state.updating)
        };
        let animate = self.animate;
        let with_details = batteries.iter().find(|battery| has_details(battery));
        self.details &= with_details.is_some();
        let time = if animate { SLIDE_TIME } else { 0.0 };
        let slide_id = ui.make_persistent_id("details");
        let slide = ctx.animate_bool_with_time_and_easing(slide_id, self.details, time, easing::cubic_out);

        let mut actions = Vec::new();
        let width = WIDTH - 32.0;
        let content = Frame::new().inner_margin(16).show(ui, |ui| {
            ui.set_width(width);
            ui.spacing_mut().item_spacing = Vec2::new(8.0, 10.0);
            let origin = ui.cursor().min;
            // Pages slide within the content area, so they mustn't show in the margins.
            let clip = Rect::from_x_y_ranges(origin.x..=origin.x + width, ui.clip_rect().y_range());
            // The list leaves to the left while the details page comes in from the right.
            let list_at = pos2(origin.x - slide * width, origin.y);
            let mut height = page(ui, "list", list_at, clip, slide < 1.0, |ui| {
                list(ui, &palette, &batteries, updated, updating, animate, &mut actions);
            });
            // The popup takes the height of its taller page, so it doesn't resize while a page slides in. While the
            // details page isn't shown, it is only laid out to measure it.
            if let Some(battery) = with_details {
                let at = pos2(origin.x + (1.0 - slide) * width, origin.y);
                let details_height = page(ui, "details", at, clip, slide > 0.0, |ui| {
                    details_page(ui, &palette, battery, animate, &mut actions);
                });
                height = height.max(details_height);
            }
            ui.allocate_space(Vec2::new(width, height));
        });
        for action in actions {
            match action {
                Action::Refresh => self.shared.refresh(),
                Action::Open => self.details = true,
                Action::Back => self.details = false,
            }
        }
        self.place(&ctx, content.response.rect.size());
        // Keeps "Updated … ago" current.
        ctx.request_repaint_after(Duration::from_secs(60));
    }

    fn on_exit(&mut self, _gl: Option<&eframe::glow::Context>) {
        self.shared.lock().popup = None;
    }
}

/// Lays out a page at `min`, clipped to `clip`, and returns its height. An invisible page is only measured.
fn page(
    ui: &mut egui::Ui,
    id_salt: &str,
    min: Pos2,
    clip: Rect,
    visible: bool,
    content: impl FnOnce(&mut egui::Ui),
) -> f32 {
    let rect = Rect::from_min_size(min, Vec2::new(clip.width(), PAGE_MAX_HEIGHT));
    let mut builder = UiBuilder::new()
        .id_salt(id_salt)
        .max_rect(rect)
        .layout(Layout::top_down(Align::Min));
    if !visible {
        builder = builder.invisible();
    }
    let mut page = ui.new_child(builder);
    page.set_clip_rect(clip.intersect(page.clip_rect()));
    page.set_width(clip.width());
    content(&mut page);
    page.min_rect().height()
}

/// The header with the refresh button, followed by a card for every device.
fn list(
    ui: &mut egui::Ui,
    palette: &Palette,
    batteries: &[Battery],
    updated: Option<Instant>,
    updating: bool,
    animate: bool,
    actions: &mut Vec<Action>,
) {
    let status = match updated {
        None => "Looking for devices…".to_string(),
        Some(time) => format!("Updated {}", ago(time.elapsed())),
    };
    let (connected, disconnected): (Vec<&Battery>, Vec<&Battery>) =
        batteries.iter().partition(|battery| battery.connected);

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
            } else if icon_button(ui, palette, REFRESH, palette.weak, "Refresh") {
                actions.push(Action::Refresh);
            }
        });
    });
    ui.add_space(4.0);

    if connected.is_empty() && !updating {
        card(ui, palette, None, |ui| {
            ui.label(
                RichText::new("No device found")
                    .family(semibold())
                    .size(14.0)
                    .color(palette.text),
            );
            ui.label(
                RichText::new("Make sure your devices are switched on and connected.")
                    .size(12.0)
                    .color(palette.weak),
            );
        });
    }
    for battery in connected {
        if device(ui, palette, battery, animate) {
            actions.push(Action::Open);
        }
    }
    if !disconnected.is_empty() {
        ui.add_space(6.0);
        ui.label(
            RichText::new("Not connected")
                .family(semibold())
                .size(12.0)
                .color(palette.weak),
        );
        for battery in disconnected {
            absent(ui, palette, battery);
        }
    }
}

/// A device's card in the list. Returns whether it was clicked, which opens its details if it has any.
fn device(ui: &mut egui::Ui, palette: &Palette, battery: &Battery, animate: bool) -> bool {
    let [r, g, b] = level_color(battery.percent);
    let color = Color32::from_rgb(r, g, b);
    let has_details = has_details(battery);
    let id = ui.make_persistent_id(("device", &battery.name, &battery.connection));

    let card = card(ui, palette, has_details.then_some(id), |ui| {
        let right = ui.max_rect().right();
        let last_row = ui
            .horizontal(|ui| {
                badge(ui, battery.kind, color, color.gamma_multiply(0.15));
                // The percentage is placed first, so long names wrap in the remaining width instead of overlapping it.
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    let percent = RichText::new(format!("{}%", battery.percent))
                        .family(semibold())
                        .size(22.0);
                    ui.label(percent.color(palette.text));
                    ui.with_layout(Layout::top_down(Align::Min), |ui| {
                        ui.spacing_mut().item_spacing = Vec2::new(4.0, 2.0);
                        let name = RichText::new(&battery.name).family(semibold()).size(14.0);
                        ui.add(egui::Label::new(name.color(palette.text)).wrap());
                        let source = ui.add(egui::Label::new(source(battery, palette.weak)).wrap());
                        status_row(ui, palette, battery, color).map_or(source.rect, |row| row.rect)
                    })
                    .inner
                })
                .inner
            })
            .inner;

        bar(
            ui,
            palette,
            battery.percent,
            color,
            battery.power == Power::Charging && animate,
        );
        // The chevron sits below the percentage, on the line of the state.
        pos2(right, last_row.center().y)
    });

    if has_details {
        let color = if card.response.hovered() {
            palette.text
        } else {
            palette.weak
        };
        let font = FontId::new(10.0, icons());
        ui.painter()
            .text(card.inner, Align2::RIGHT_CENTER, CHEVRON_RIGHT, font, color);
    }
    card.response.clicked()
}

/// The battery of one device in depth, below a header that leads back to the list.
fn details_page(ui: &mut egui::Ui, palette: &Palette, battery: &Battery, animate: bool, actions: &mut Vec<Action>) {
    let Some(pack) = battery.pack else { return };
    let [r, g, b] = level_color(battery.percent);
    let color = Color32::from_rgb(r, g, b);

    ui.vertical(|ui| {
        ui.spacing_mut().item_spacing.y = 2.0;
        // The back arrow sits on the line of the name, and the connection below lines up with the name.
        let indent = ui
            .horizontal(|ui| {
                let back = icon_button(ui, palette, BACK, palette.text, "Back");
                if back {
                    actions.push(Action::Back);
                }
                let name = RichText::new(&battery.name).family(semibold()).size(18.0);
                let name = ui.add(egui::Label::new(name.color(palette.text)).wrap());
                name.rect.left() - ui.min_rect().left()
            })
            .inner;
        ui.horizontal(|ui| {
            ui.add_space(indent);
            ui.add(egui::Label::new(source(battery, palette.weak)).wrap());
        });
    });
    ui.add_space(4.0);

    card(ui, palette, None, |ui| {
        ui.horizontal(|ui| {
            let charging = battery.power == Power::Charging && animate;
            let ring = ring(ui, palette, battery.percent, color, charging);
            ui.add_space(8.0);
            centered(ui, ring.height(), |ui| {
                ui.spacing_mut().item_spacing = Vec2::new(4.0, 2.0);
                status_row(ui, palette, battery, color);
                let stored = format!("{} of {}", watt_hours(pack.now), watt_hours(pack.full));
                ui.label(RichText::new(stored).size(12.0).color(palette.weak));
            });
        });
    });

    card(ui, palette, None, |ui| health(ui, palette, &pack));
    let mut stats = Vec::new();
    if let Some(cycles) = pack.cycles {
        stats.push(("Charge cycles", cycles.to_string()));
    }
    match pack.rate {
        Some(rate) if rate > 0 => stats.push(("Charging power", watts(rate))),
        Some(rate) if rate < 0 => stats.push(("Power draw", watts(rate))),
        _ => {}
    }
    if !stats.is_empty() {
        ui.columns(stats.len(), |columns| {
            for (ui, (label, value)) in columns.iter_mut().zip(&stats) {
                stat(ui, palette, label, value);
            }
        });
    }
}

/// A ring filled clockwise from the top up to `percent` around the percentage, with a moving highlight while
/// `charging`. Returns where it was placed.
fn ring(ui: &mut egui::Ui, palette: &Palette, percent: u8, color: Color32, charging: bool) -> Rect {
    const THICKNESS: f32 = 8.0;
    let (rect, _) = ui.allocate_exact_size(Vec2::splat(88.0), Sense::hover());
    let center = rect.center();
    let radius = (rect.width() - THICKNESS) / 2.0;
    let painter = ui.painter();
    painter.circle_stroke(center, radius, Stroke::new(THICKNESS, palette.track));

    if percent > 0 {
        let sweep = TAU * f32::from(percent.min(100)) / 100.0;
        let point = |angle: f32| center + radius * Vec2::angled(angle - FRAC_PI_2);
        // A vertex every 2 points of the arc keeps it round.
        let steps = (sweep * radius / 2.0).ceil() as usize;
        let arc = (0..=steps).map(|i| point(sweep * i as f32 / steps as f32)).collect();
        let stroke = if charging {
            // The highlight sweeps along the arc like the one on the bars.
            let half = 24.0 / radius;
            let head = sweep_phase(ui) * (sweep + 2.0 * half) - half;
            PathStroke::new_uv(THICKNESS, move |_, pos| {
                let angle = ((pos - center).angle() + FRAC_PI_2).rem_euclid(TAU);
                let light = (1.0 - (angle - head).abs() / half).max(0.0);
                color.lerp_to_gamma(Color32::WHITE, light * 0.47)
            })
        } else {
            PathStroke::new(THICKNESS, color)
        };
        painter.line(arc, stroke);
        // Rounds off the ends of the arc.
        for angle in [0.0, sweep] {
            painter.circle_filled(point(angle), THICKNESS / 2.0, color);
        }
    }

    let font = FontId::new(22.0, semibold());
    painter.text(center, Align2::CENTER_CENTER, format!("{percent}%"), font, palette.text);
    rect
}

/// Lays out `content` in the rest of a row, centered on the row's `height`. As egui only centers single widgets, the
/// content is measured first.
fn centered(ui: &mut egui::Ui, height: f32, content: impl Fn(&mut egui::Ui)) {
    let rect = ui.available_rect_before_wrap();
    let layout = Layout::top_down(Align::Min);
    let mut measure = ui.new_child(
        UiBuilder::new()
            .id_salt("measure")
            .max_rect(rect)
            .layout(layout)
            .invisible(),
    );
    content(&mut measure);
    let top = rect.top() + ((height - measure.min_rect().height()) / 2.0).max(0.0);
    ui.scope_builder(UiBuilder::new().max_rect(rect.with_min_y(top)).layout(layout), content);
}

/// A small card with a value below its label.
fn stat(ui: &mut egui::Ui, palette: &Palette, label: &str, value: &str) {
    card(ui, palette, None, |ui| {
        ui.spacing_mut().item_spacing.y = 2.0;
        ui.label(RichText::new(label).size(12.0).color(palette.weak));
        let value = RichText::new(value).family(semibold()).size(18.0);
        ui.label(value.color(palette.text));
    });
}

/// How much the battery holds compared to when it was new.
fn health(ui: &mut egui::Ui, palette: &Palette, pack: &Pack) {
    let percent = (pack.full as f32 / pack.design as f32 * 100.0).round().min(255.0) as u8;
    let [r, g, b] = level_color(percent);
    ui.horizontal(|ui| {
        let label = RichText::new("Battery health").family(semibold()).size(14.0);
        ui.label(label.color(palette.text));
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            let value = RichText::new(format!("{percent}%")).family(semibold()).size(14.0);
            ui.label(value.color(palette.text));
        });
    });
    // The track stands for the capacity when new, of which the hatched part can no longer be charged.
    let color = Color32::from_rgb(r, g, b);
    let (track, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 8.0), Sense::hover());
    hatch(ui, palette, track);
    let mut fill = track;
    fill.set_width(track.width() * f32::from(percent.min(100)) / 100.0);
    ui.painter().rect_filled(fill, 4.0, color);

    ui.spacing_mut().item_spacing.y = 6.0;
    legend(ui, palette, "Full charge", &watt_hours(pack.full), |ui, swatch| {
        ui.painter()
            .circle_filled(swatch.center(), swatch.height() / 2.0, color);
    });
    let lost = pack.design.saturating_sub(pack.full);
    if lost > 0 {
        legend(ui, palette, "Lost to wear", &watt_hours(lost), |ui, swatch| {
            hatch(ui, palette, swatch);
        });
    }
}

/// A capsule of diagonal stripes on the track color, which marks capacity that is lost.
fn hatch(ui: &egui::Ui, palette: &Palette, rect: Rect) {
    const SAMPLES: usize = 16;
    let radius = rect.height() / 2.0;
    ui.painter().rect_filled(rect, radius, palette.track);
    let stroke = Stroke::new(1.5, palette.weak.gamma_multiply(0.6));
    let core = rect.shrink(radius);
    let inside = |point: Pos2| point.distance(core.clamp(point)) <= radius;
    let mut x = rect.left() - rect.height();
    while x < rect.right() {
        // Each stripe is cut to the samples along it that lie within the rounded ends.
        let along = |i| pos2(x, rect.bottom()) + rect.height() * Vec2::new(1.0, -1.0) * i as f32 / SAMPLES as f32;
        let points: Vec<Pos2> = (0..=SAMPLES).map(along).filter(|&point| inside(point)).collect();
        if let [first, .., last] = points[..] {
            ui.painter().line_segment([first, last], stroke);
        }
        x += 4.0;
    }
}

/// A line of a legend: a swatch painted by `swatch` and a label on the left, the value on the right.
fn legend(ui: &mut egui::Ui, palette: &Palette, label: &str, value: &str, swatch: impl FnOnce(&egui::Ui, Rect)) {
    ui.horizontal(|ui| {
        let (rect, _) = ui.allocate_exact_size(Vec2::splat(10.0), Sense::hover());
        swatch(ui, rect);
        ui.label(RichText::new(label).size(12.0).color(palette.weak));
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            ui.label(RichText::new(value).size(12.0).color(palette.text));
        });
    });
}

/// A glyph as a borderless button, which gets a rounded background under the pointer. Returns whether it was clicked.
fn icon_button(ui: &mut egui::Ui, palette: &Palette, glyph: &str, color: Color32, tooltip: &str) -> bool {
    let (rect, response) = ui.allocate_exact_size(Vec2::splat(28.0), Sense::click());
    if response.hovered() {
        ui.painter().rect_filled(rect, 6.0, palette.track);
    }
    let font = FontId::new(14.0, icons());
    ui.painter()
        .text(rect.center(), Align2::CENTER_CENTER, glyph, font, color);
    response.on_hover_text(tooltip).clicked()
}

/// Only the computer's battery reports its capacity, which gives it more to show than its card.
fn has_details(battery: &Battery) -> bool {
    battery.pack.is_some()
}

/// A rounded bar filled up to `percent`, with a moving highlight while `charging`.
fn bar(ui: &mut egui::Ui, palette: &Palette, percent: u8, color: Color32, charging: bool) {
    let (bar, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 6.0), Sense::hover());
    ui.painter().rect_filled(bar, 3.0, palette.track);
    let mut fill = bar;
    fill.set_width(bar.width() * f32::from(percent.min(100)) / 100.0);
    ui.painter().rect_filled(fill, 3.0, color);
    if charging {
        shimmer(ui, fill);
    }
}

/// A paired device that is out of reach, muted and without a bar, as its level may be outdated.
fn absent(ui: &mut egui::Ui, palette: &Palette, battery: &Battery) {
    card(ui, palette, None, |ui| {
        ui.horizontal(|ui| {
            badge(ui, battery.kind, palette.weak, palette.track);
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                // Aligns the level with the name, and the time of its reading with the connection.
                ui.with_layout(Layout::top_down(Align::Max), |ui| {
                    ui.spacing_mut().item_spacing.y = 0.0;
                    let percent = RichText::new(format!("{}%", battery.percent))
                        .family(semibold())
                        .size(16.0);
                    ui.label(percent.color(palette.weak));
                    if let Some(time) = battery.reported {
                        // A time in the future, which a changed clock can cause, counts as just now.
                        let age = ago(time.elapsed().unwrap_or_default());
                        ui.label(RichText::new(age).size(11.0).color(palette.weak));
                    }
                });
                ui.with_layout(Layout::top_down(Align::Min), |ui| {
                    ui.spacing_mut().item_spacing = Vec2::new(4.0, 2.0);
                    let name = RichText::new(&battery.name).family(semibold()).size(14.0);
                    ui.add(egui::Label::new(name.color(palette.text)).wrap());
                    ui.add(egui::Label::new(source(battery, palette.weak)).wrap());
                });
            });
        });
    });
}

/// A round badge with the glyph of the device's kind.
fn badge(ui: &mut egui::Ui, kind: Kind, color: Color32, fill: Color32) {
    let (badge, _) = ui.allocate_exact_size(Vec2::splat(36.0), Sense::hover());
    ui.painter().circle_filled(badge.center(), 18.0, fill);
    let glyph = match kind {
        Kind::Computer => LAPTOP,
        Kind::Keyboard => KEYBOARD,
        Kind::Mouse => MOUSE,
        Kind::Headset => HEADSET,
        Kind::Other => BATTERY,
    };
    let font = FontId::new(16.0, icons());
    ui.painter()
        .text(badge.center(), Align2::CENTER_CENTER, glyph, font, color);
}

/// The manufacturer, followed by the connection behind a glyph of its kind, e.g. "Logitech [USB] USB Receiver".
fn source(battery: &Battery, color: Color32) -> LayoutJob {
    let text = TextFormat::simple(FontId::proportional(12.0), color);
    let glyph = TextFormat {
        valign: Align::Center,
        ..TextFormat::simple(FontId::new(12.0, icons()), color)
    };
    let link = match battery.link {
        Link::Builtin => BATTERY,
        Link::Usb => USB,
        Link::Bluetooth => BLUETOOTH,
    };
    let mut job = LayoutJob::default();
    job.append(&battery.manufacturer, 0.0, text.clone());
    let gap = if battery.manufacturer.is_empty() { 0.0 } else { 10.0 };
    job.append(link, gap, glyph);
    job.append(&battery.connection, 4.0, text);
    job
}

/// The power state as a chip, followed by the time estimate if there is one. Devices that report neither get none.
fn status_row(ui: &mut egui::Ui, palette: &Palette, battery: &Battery, color: Color32) -> Option<egui::Response> {
    let [r, g, b] = level_color(0);
    // Tinted in the level color while the battery fills or is full, and in red for a problem.
    let (glyph, state, tint) = match battery.power {
        Power::Charging => (Some(CHARGING), "Charging", Some(color)),
        Power::Full => (Some(CHECK), "Fully charged", Some(color)),
        // The pause glyph looks like two missing glyphs at this size, so the text stands alone.
        Power::PluggedIn => (None, "Charging paused", None),
        Power::Fault => (Some(WARNING), "Charging problem", Some(Color32::from_rgb(r, g, b))),
        Power::Discharging if !battery.rechargeable => (None, "Replaceable battery", None),
        Power::Discharging => (None, "On battery", None),
        Power::Unknown => return None,
    };
    let estimate = battery.remaining.map(duration).and_then(|time| match battery.power {
        Power::Charging => Some(format!("{time} until full")),
        Power::Discharging => Some(format!("{time} left")),
        _ => None,
    });

    ui.add_space(4.0);
    // The estimate moves below the chip when both don't fit next to the percentage.
    let row = ui.horizontal_wrapped(|ui| {
        let (fill, text) = match tint {
            Some(tint) => (tint.gamma_multiply(0.15), tint),
            None => (palette.track, palette.weak),
        };
        Frame::new()
            .fill(fill)
            .corner_radius(8)
            .inner_margin(Margin::symmetric(8, 2))
            .show(ui, |ui| {
                ui.spacing_mut().item_spacing.x = 4.0;
                ui.horizontal(|ui| {
                    if let Some(glyph) = glyph {
                        ui.label(icon(glyph, 10.0).color(text));
                    }
                    ui.label(RichText::new(state).family(semibold()).size(11.0).color(text));
                });
            });
        if let Some(estimate) = estimate {
            ui.label(RichText::new(estimate).size(12.0).color(palette.weak));
        }
    });
    Some(row.response)
}

/// The amount of a power flow in milliwatts, in either direction, e.g. "7.9 W".
fn watts(rate: i32) -> String {
    format!("{:.1} W", rate.unsigned_abs() as f32 / 1000.0)
}

/// An amount of energy in milliwatt-hours, e.g. "52.3 Wh".
fn watt_hours(energy: u32) -> String {
    format!("{:.1} Wh", energy as f32 / 1000.0)
}

/// A duration in hours and minutes, e.g. "2 h 5 min".
fn duration(time: Duration) -> String {
    let minutes = (time.as_secs() + 30) / 60;
    match (minutes / 60, minutes % 60) {
        (0, minutes) => format!("{} min", minutes.max(1)),
        (hours, 0) => format!("{hours} h"),
        (hours, minutes) => format!("{hours} h {minutes} min"),
    }
}

/// How long ago something happened, e.g. "5 min ago".
fn ago(elapsed: Duration) -> String {
    match elapsed.as_secs() / 60 {
        0 => "just now".into(),
        minutes @ 1..60 => format!("{minutes} min ago"),
        minutes => format!("{} h ago", minutes / 60),
    }
}

/// How far the highlight of a charging battery has swept along its track, from 0 to 1. Keeps repainting to move it.
fn sweep_phase(ui: &egui::Ui) -> f32 {
    const PERIOD: f64 = 1.8;
    // About 30 frames per second is smooth enough for this and saves power.
    ui.ctx().request_repaint_after(Duration::from_millis(33));
    (ui.input(|input| input.time) % PERIOD / PERIOD) as f32
}

/// Sweeps a soft highlight along the filled part of a charging battery's bar.
fn shimmer(ui: &egui::Ui, fill: egui::Rect) {
    let width = fill.height() * 8.0;
    let center = fill.left() - width / 2.0 + sweep_phase(ui) * (fill.width() + width);
    let light = Color32::from_white_alpha(120);

    let mut mesh = egui::Mesh::default();
    for (x, color) in [
        (center - width / 2.0, Color32::TRANSPARENT),
        (center, light),
        (center + width / 2.0, Color32::TRANSPARENT),
    ] {
        mesh.colored_vertex(egui::pos2(x, fill.top()), color);
        mesh.colored_vertex(egui::pos2(x, fill.bottom()), color);
    }
    for i in [0, 2] {
        mesh.add_triangle(i, i + 1, i + 2);
        mesh.add_triangle(i + 1, i + 2, i + 3);
    }
    // Keep the highlight off the bar's rounded ends, and out of view while its page slides.
    let clip = fill
        .shrink2(Vec2::new(fill.height() / 2.0, 0.0))
        .intersect(ui.clip_rect());
    ui.painter().with_clip_rect(clip).add(mesh);
}

/// A rounded panel. Given an `id`, it lights up under the pointer and its response tells about clicks.
fn card<R>(
    ui: &mut egui::Ui,
    palette: &Palette,
    id: Option<Id>,
    content: impl FnOnce(&mut egui::Ui) -> R,
) -> InnerResponse<R> {
    let mut prepared = Frame::new()
        .fill(palette.card)
        .corner_radius(10)
        .inner_margin(14)
        .begin(ui);
    let width = prepared.content_ui.available_width();
    prepared.content_ui.set_width(width);
    let inner = content(&mut prepared.content_ui);
    let mut response = prepared.allocate_space(ui);
    // Cards on a page that is only measured mustn't react, not even with the pointer's shape.
    if let Some(id) = id.filter(|_| ui.is_enabled()) {
        // Registered after the content, so the card takes the clicks on top of its texts.
        response = ui
            .interact(response.rect, id.with("card"), Sense::click())
            .on_hover_cursor(CursorIcon::PointingHand);
        if response.hovered() {
            prepared.frame.fill = palette.hover;
        }
    }
    // The fill is decided only now, as it depends on the pointer.
    prepared.paint(ui);
    InnerResponse::new(inner, response)
}

/// Neutral colors in the style of Windows 11 flyouts.
struct Palette {
    background: Color32,
    card: Color32,
    /// A card under the pointer.
    hover: Color32,
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
                hover: Color32::from_rgb(0x32, 0x32, 0x32),
                text: Color32::from_rgb(0xff, 0xff, 0xff),
                weak: Color32::from_rgb(0xa0, 0xa0, 0xa0),
                track: Color32::from_rgb(0x3d, 0x3d, 0x3d),
            }
        } else {
            Self {
                background: Color32::from_rgb(0xf3, 0xf3, 0xf3),
                card: Color32::from_rgb(0xff, 0xff, 0xff),
                hover: Color32::from_rgb(0xf9, 0xf9, 0xf9),
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

/// Uses the system's UI fonts, falling back to egui's bundled fonts where they are missing.
fn fonts() -> FontDefinitions {
    let mut fonts = FontDefinitions::default();
    let fallbacks = fonts.families[&FontFamily::Proportional].clone();
    fonts.families.insert(semibold(), fallbacks.clone());
    fonts.families.insert(icons(), fallbacks);

    let sources = [
        (Font::Text, FontFamily::Proportional),
        (Font::Semibold, semibold()),
        (Font::Icons, icons()),
    ];
    for (font, family) in sources {
        if let Some(data) = platform::popup::font(font) {
            let name = family.to_string();
            fonts
                .font_data
                .insert(name.clone(), Arc::new(FontData::from_owned(data)));
            fonts.families.entry(family).or_default().insert(0, name);
        }
    }
    fonts
}
