use crate::battery::{self, Battery};
use crate::platform::tray::{Event, Events, Notifier};
use crate::{platform, popup};
use std::collections::HashSet;
use std::error::Error;
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::{Duration, Instant};
use tray_icon::menu::{Menu, MenuEvent, MenuItem, PredefinedMenuItem};
use tray_icon::{Icon, MouseButton, MouseButtonState, Rect, TrayIcon, TrayIconBuilder, TrayIconEvent};

const POLL_INTERVAL: Duration = Duration::from_secs(5 * 60);

/// Devices to warn about when their battery drops to a low level, once per device while the tray runs.
#[derive(Default)]
struct Warnings {
    /// Devices whose last reading was above `popup::LOW_PERCENT`.
    above: HashSet<String>,
    warned: HashSet<String>,
}

#[derive(Clone, Default)]
pub struct State {
    pub batteries: Vec<Battery>,
    pub updated: Option<Instant>,
    pub updating: bool,
    /// Context of the open popup, so new readings can wake it up.
    pub popup: Option<eframe::egui::Context>,
}

/// State shared between the tray, the popup and the scanning threads.
#[derive(Clone)]
pub struct Shared {
    state: Arc<Mutex<State>>,
    /// Tells the tray thread when new readings are available.
    tray: Notifier,
}

impl Shared {
    pub fn lock(&self) -> MutexGuard<'_, State> {
        self.state.lock().unwrap()
    }

    pub fn refresh(&self) {
        {
            let mut state = self.lock();
            if state.updating {
                return;
            }
            state.updating = true;
        }
        let shared = self.clone();
        // Scanning takes up to a second while a device wakes up, so it runs off the UI thread.
        std::thread::spawn(move || {
            let batteries = battery::scan();
            let mut state = shared.lock();
            state.batteries = batteries;
            state.updated = Some(Instant::now());
            state.updating = false;
            if let Some(popup) = &state.popup {
                popup.request_repaint();
            }
            shared.tray.notify();
        });
    }
}

pub fn run() -> Result<(), Box<dyn Error>> {
    // Keep a single tray icon if two trays start at the same time, e.g. by autostart and by hand.
    if !platform::tray::claim() {
        return Ok(());
    }
    let events = Events::new(POLL_INTERVAL);
    let shared = Shared {
        state: Default::default(),
        tray: events.notifier(),
    };
    let tray = TrayIconBuilder::new().with_menu_on_left_click(false).build()?;
    let mut warnings = Warnings::default();
    update(&tray, &shared, &mut warnings)?;
    shared.refresh();

    let mut popup_closed: Option<Instant> = None;
    for event in events {
        match event {
            Event::Poll => shared.refresh(),
            Event::Notified => update(&tray, &shared, &mut warnings)?,
            Event::Other => {}
        }

        while let Ok(event) = MenuEvent::receiver().try_recv() {
            match event.id.as_ref() {
                "refresh" => shared.refresh(),
                "quit" => return Ok(()),
                _ => {}
            }
        }
        while let Ok(event) = TrayIconEvent::receiver().try_recv() {
            let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                position,
                rect,
                ..
            } = event
            else {
                continue;
            };
            // Clicking the tray icon while the popup is open closes it by taking its focus, so don't reopen it.
            if popup_closed.is_some_and(|time| time.elapsed() < Duration::from_millis(300)) {
                continue;
            }
            // Tray hosts on Linux tell only where the click was, not where the icon is.
            let anchor = if rect.size.width == 0 {
                Rect { position, ..rect }
            } else {
                rect
            };
            popup::show(&shared, anchor)?;
            popup_closed = Some(Instant::now());
            // Readings that arrived while the popup was open skipped this loop.
            update(&tray, &shared, &mut warnings)?;
        }
    }
    Ok(())
}

/// Shows the latest readings of connected devices in the tray icon, its tooltip and its menu, and warns about
/// devices whose battery dropped to a low level.
fn update(tray: &TrayIcon, shared: &Shared, warnings: &mut Warnings) -> Result<(), Box<dyn Error>> {
    let state = shared.lock();
    let connected: Vec<&Battery> = state.batteries.iter().filter(|battery| battery.connected).collect();
    let lines: Vec<String> = connected.iter().map(|battery| battery.to_string()).collect();
    let menu = Menu::new();
    for line in &lines {
        menu.append(&MenuItem::new(line, false, None))?;
    }
    if !lines.is_empty() {
        menu.append(&PredefinedMenuItem::separator())?;
    }
    menu.append(&MenuItem::with_id("refresh", "Refresh", true, None))?;
    menu.append(&MenuItem::with_id("quit", "Quit", true, None))?;

    let tooltip = if lines.is_empty() {
        "No device with battery found".into()
    } else {
        lines.join("\n")
    };
    tray.set_menu(Some(Box::new(menu)));
    tray.set_tooltip(Some(tooltip))?;
    tray.set_icon(Some(icon(connected.iter().map(|battery| battery.percent).min())?))?;

    for battery in &state.batteries {
        if battery.percent > popup::LOW_PERCENT {
            if !warnings.warned.contains(&battery.name) {
                warnings.above.insert(battery.name.clone());
            }
        } else if warnings.above.remove(&battery.name) {
            warnings.warned.insert(battery.name.clone());
            let text = format!("{} is at {}%", battery.name, battery.percent);
            platform::tray::notify(tray, "Low battery", &text);
        }
    }
    Ok(())
}

/// Draws a battery filled up to `percent`, or an empty outline when no device is reachable.
fn icon(percent: Option<u8>) -> Result<Icon, tray_icon::BadIcon> {
    const OUTLINE: [u8; 4] = [0x90, 0x90, 0x90, 0xff];
    let size = platform::tray::icon_size();
    let [r, g, b] = popup::level_color(percent.unwrap_or(0));

    // Shapes are laid out on a 16x16 grid, scaled to the system icon size.
    let grid = |v: usize| (v as f32 * size as f32 / 16.0).round() as usize;
    let stroke = grid(1).max(1);
    let (left, top, right, bottom) = (0, grid(3), grid(14), grid(13));
    let (fill_left, fill_right) = (left + 2 * stroke, right - 2 * stroke);
    let fill_end = fill_left + ((fill_right - fill_left) * usize::from(percent.unwrap_or(0)) + 50) / 100;

    let mut rgba = vec![0u8; size * size * 4];
    for (i, pixel) in rgba.as_chunks_mut::<4>().0.iter_mut().enumerate() {
        let (x, y) = (i % size, i / size);
        let within = |x0, y0, x1, y1| (x0..x1).contains(&x) && (y0..y1).contains(&y);
        if within(fill_left, top + 2 * stroke, fill_end, bottom - 2 * stroke) {
            *pixel = [r, g, b, 0xff];
        } else if within(left, top, right, bottom)
            && !within(left + stroke, top + stroke, right - stroke, bottom - stroke)
            || within(right, grid(6), size, grid(10))
        {
            *pixel = OUTLINE;
        }
    }
    Icon::from_rgba(rgba, size as u32, size as u32)
}
