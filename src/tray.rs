use crate::battery::{self, Battery};
use crate::popup;
use std::error::Error;
use std::ptr::{null, null_mut};
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::{Duration, Instant};
use tray_icon::menu::{Menu, MenuEvent, MenuItem, PredefinedMenuItem};
use tray_icon::{Icon, MouseButton, MouseButtonState, TrayIcon, TrayIconBuilder, TrayIconEvent};
use windows_sys::Win32::Foundation::{CloseHandle, ERROR_ALREADY_EXISTS, GetLastError};
use windows_sys::Win32::System::Threading::{
    CreateMutexW, GetCurrentThreadId, OpenMutexW, SYNCHRONIZATION_SYNCHRONIZE,
};
use windows_sys::Win32::UI::WindowsAndMessaging::{
    DispatchMessageW, GetMessageW, GetSystemMetrics, MSG, PostThreadMessageW, SM_CXSMICON, SetCoalescableTimer,
    TranslateMessage, WM_APP, WM_TIMER,
};
use windows_sys::core::PCWSTR;
use windows_sys::w;

const MUTEX_NAME: PCWSTR = w!("akkutray-tray");
const POLL_INTERVAL_MS: u32 = 5 * 60 * 1000;
/// Posted to the tray thread when new readings are available.
const WM_READINGS: u32 = WM_APP;

#[derive(Default)]
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
    tray_thread: u32,
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
            unsafe { PostThreadMessageW(shared.tray_thread, WM_READINGS, 0, 0) };
        });
    }
}

pub fn is_running() -> bool {
    unsafe {
        let mutex = OpenMutexW(SYNCHRONIZATION_SYNCHRONIZE, 0, MUTEX_NAME);
        if mutex.is_null() {
            return false;
        }
        CloseHandle(mutex);
        true
    }
}

pub fn run() -> Result<(), Box<dyn Error>> {
    // Keep a single tray icon if two trays start at the same time, e.g. by autostart and by hand.
    unsafe {
        CreateMutexW(null(), 0, MUTEX_NAME);
        if GetLastError() == ERROR_ALREADY_EXISTS {
            return Ok(());
        }
    }
    let shared = Shared {
        state: Default::default(),
        tray_thread: unsafe { GetCurrentThreadId() },
    };
    let tray = TrayIconBuilder::new().with_menu_on_left_click(false).build()?;
    update(&tray, &shared)?;
    shared.refresh();
    // The tolerance lets Windows batch our wake-ups with other timers to save power.
    unsafe { SetCoalescableTimer(null_mut(), 0, POLL_INTERVAL_MS, None, POLL_INTERVAL_MS / 10) };

    let mut popup_closed: Option<Instant> = None;
    loop {
        let mut msg = MSG::default();
        if unsafe { GetMessageW(&mut msg, null_mut(), 0, 0) } <= 0 {
            return Ok(());
        }
        match msg.message {
            WM_TIMER if msg.hwnd.is_null() => shared.refresh(),
            WM_READINGS if msg.hwnd.is_null() => update(&tray, &shared)?,
            _ => unsafe {
                TranslateMessage(&msg);
                DispatchMessageW(&msg);
            },
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
            popup::show(&shared, rect)?;
            popup_closed = Some(Instant::now());
            // Readings that arrived while the popup was open skipped this loop.
            update(&tray, &shared)?;
        }
    }
}

/// Shows the latest readings in the tray icon, its tooltip and its menu.
fn update(tray: &TrayIcon, shared: &Shared) -> Result<(), Box<dyn Error>> {
    let state = shared.lock();
    let lines: Vec<String> = state.batteries.iter().map(Battery::to_string).collect();
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
    tray.set_icon(Some(icon(state.batteries.iter().map(|battery| battery.percent).min())?))?;
    Ok(())
}

/// Draws a battery filled up to `percent`, or an empty outline when no device is reachable.
fn icon(percent: Option<u8>) -> Result<Icon, tray_icon::BadIcon> {
    const OUTLINE: [u8; 4] = [0x90, 0x90, 0x90, 0xff];
    let size = unsafe { GetSystemMetrics(SM_CXSMICON) } as usize;
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
