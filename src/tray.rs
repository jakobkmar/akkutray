use crate::hidpp::{self, Battery};
use std::cell::RefCell;
use std::ptr::{null, null_mut};
use std::sync::OnceLock;
use windows_sys::Win32::Foundation::{ERROR_ALREADY_EXISTS, GetLastError, HWND, LPARAM, LRESULT, POINT, WPARAM};
use windows_sys::Win32::System::LibraryLoader::GetModuleHandleW;
use windows_sys::Win32::System::Threading::CreateMutexW;
use windows_sys::Win32::UI::HiDpi::{DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2, SetProcessDpiAwarenessContext};
use windows_sys::Win32::UI::Shell::*;
use windows_sys::Win32::UI::WindowsAndMessaging::*;
use windows_sys::w;

const POLL_INTERVAL_MS: u32 = 5 * 60 * 1000;
const LOW_PERCENT: u8 = 20;
const WM_TRAY: u32 = WM_APP;
const ID_REFRESH: i32 = 1;
const ID_QUIT: i32 = 2;

thread_local! {
    static BATTERIES: RefCell<Vec<Battery>> = const { RefCell::new(Vec::new()) };
}
static TASKBAR_CREATED: OnceLock<u32> = OnceLock::new();

pub fn run() {
    unsafe {
        // Keep a single tray icon if the tray gets started twice, e.g. by autostart and by hand.
        CreateMutexW(null(), 0, w!("lgbattery-tray"));
        if GetLastError() == ERROR_ALREADY_EXISTS {
            return;
        }
        SetProcessDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2);
        TASKBAR_CREATED.get_or_init(|| RegisterWindowMessageW(w!("TaskbarCreated")));

        let instance = GetModuleHandleW(null());
        let class = w!("lgbattery");
        RegisterClassW(&WNDCLASSW {
            lpfnWndProc: Some(window_proc),
            hInstance: instance,
            lpszClassName: class,
            ..Default::default()
        });
        // A hidden top-level window, as message-only windows miss the TaskbarCreated broadcast.
        let hwnd = CreateWindowExW(0, class, class, 0, 0, 0, 0, 0, null_mut(), null_mut(), instance, null());

        notify(hwnd, NIM_ADD);
        refresh(hwnd);
        // The tolerance lets Windows batch our wake-ups with other timers to save power.
        SetCoalescableTimer(hwnd, 1, POLL_INTERVAL_MS, None, POLL_INTERVAL_MS / 10);

        let mut msg = std::mem::zeroed();
        while GetMessageW(&mut msg, null_mut(), 0, 0) > 0 {
            DispatchMessageW(&msg);
        }
        notify(hwnd, NIM_DELETE);
    }
}

unsafe extern "system" fn window_proc(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    unsafe {
        match msg {
            WM_TIMER => refresh(hwnd),
            WM_TRAY if matches!(lparam as u32, WM_LBUTTONUP | WM_RBUTTONUP) => show_menu(hwnd),
            // Explorer restarted and lost all tray icons.
            _ if Some(&msg) == TASKBAR_CREATED.get() => notify(hwnd, NIM_ADD),
            _ => return DefWindowProcW(hwnd, msg, wparam, lparam),
        }
    }
    0
}

unsafe fn refresh(hwnd: HWND) {
    BATTERIES.set(hidpp::scan());
    unsafe { notify(hwnd, NIM_MODIFY) };
}

unsafe fn notify(hwnd: HWND, action: NOTIFY_ICON_MESSAGE) {
    let mut data = NOTIFYICONDATAW {
        cbSize: size_of::<NOTIFYICONDATAW>() as u32,
        hWnd: hwnd,
        uFlags: NIF_MESSAGE | NIF_ICON | NIF_TIP,
        uCallbackMessage: WM_TRAY,
        ..Default::default()
    };
    BATTERIES.with_borrow(|batteries| {
        let tip = lines(batteries)
            .join("\n")
            .encode_utf16()
            .take(data.szTip.len() - 1)
            .collect::<Vec<_>>();
        data.szTip[..tip.len()].copy_from_slice(&tip);
        data.hIcon = unsafe { icon(batteries.iter().map(|b| b.percent).min()) };
    });
    unsafe {
        Shell_NotifyIconW(action, &data);
        // The shell keeps its own copy of the icon.
        DestroyIcon(data.hIcon);
    }
}

unsafe fn show_menu(hwnd: HWND) {
    unsafe {
        let menu = CreatePopupMenu();
        BATTERIES.with_borrow(|batteries| {
            for line in lines(batteries) {
                AppendMenuW(menu, MF_DISABLED, 0, wide(&line).as_ptr());
            }
        });
        AppendMenuW(menu, MF_SEPARATOR, 0, null());
        AppendMenuW(menu, MF_STRING, ID_REFRESH as usize, w!("Refresh"));
        AppendMenuW(menu, MF_STRING, ID_QUIT as usize, w!("Quit"));

        let mut cursor = POINT::default();
        GetCursorPos(&mut cursor);
        // Without this the menu does not close when clicking elsewhere.
        SetForegroundWindow(hwnd);
        let command = TrackPopupMenu(menu, TPM_RETURNCMD | TPM_NONOTIFY, cursor.x, cursor.y, 0, hwnd, null());
        DestroyMenu(menu);
        match command {
            ID_REFRESH => refresh(hwnd),
            ID_QUIT => PostQuitMessage(0),
            _ => {}
        }
    }
}

fn lines(batteries: &[Battery]) -> Vec<String> {
    if batteries.is_empty() {
        return vec!["No Logitech device with battery found".into()];
    }
    batteries.iter().map(Battery::to_string).collect()
}

fn wide(text: &str) -> Vec<u16> {
    text.encode_utf16().chain([0]).collect()
}

/// Draws a battery filled up to `percent`, or an empty outline when no device is reachable.
unsafe fn icon(percent: Option<u8>) -> HICON {
    const OUTLINE: u32 = 0xff909090;
    let size = unsafe { GetSystemMetrics(SM_CXSMICON) } as usize;
    let fill_color = if percent.is_some_and(|p| p <= LOW_PERCENT) {
        0xffe04040
    } else {
        0xff40c040
    };

    // Shapes are laid out on a 16x16 grid, scaled to the system icon size. Pixels are 0xAARRGGBB.
    let grid = |v: usize| (v as f32 * size as f32 / 16.0).round() as usize;
    let stroke = grid(1).max(1);
    let (left, top, right, bottom) = (0, grid(3), grid(14), grid(13));
    let (fill_left, fill_right) = (left + 2 * stroke, right - 2 * stroke);
    let fill_end = fill_left + ((fill_right - fill_left) * usize::from(percent.unwrap_or(0)) + 50) / 100;

    let mut pixels = vec![0u32; size * size];
    for (i, pixel) in pixels.iter_mut().enumerate() {
        let (x, y) = (i % size, i / size);
        let within = |x0, y0, x1, y1| (x0..x1).contains(&x) && (y0..y1).contains(&y);
        *pixel = if within(fill_left, top + 2 * stroke, fill_end, bottom - 2 * stroke) {
            fill_color
        } else if within(left, top, right, bottom)
            && !within(left + stroke, top + stroke, right - stroke, bottom - stroke)
            || within(right, grid(6), size, grid(10))
        {
            OUTLINE
        } else {
            0
        };
    }
    // With 32-bit color the alpha channel defines transparency, so the AND mask stays empty.
    let mask = vec![0u8; size * size];
    unsafe {
        CreateIcon(
            null_mut(),
            size as i32,
            size as i32,
            1,
            32,
            mask.as_ptr(),
            pixels.as_ptr().cast(),
        )
    }
}
