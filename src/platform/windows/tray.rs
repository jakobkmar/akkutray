//! The message loop of the tray thread, a single tray at a time, notifications and starting the tray without a
//! console.

use std::os::windows::process::CommandExt;
use std::process::Command;
use std::ptr::{null, null_mut};
use std::time::Duration;
use tray_icon::TrayIcon;
use windows_sys::Win32::Foundation::{CloseHandle, ERROR_ALREADY_EXISTS, GetLastError};
use windows_sys::Win32::System::Console::GetConsoleWindow;
use windows_sys::Win32::System::Threading::{
    CreateMutexW, DETACHED_PROCESS, GetCurrentThreadId, OpenMutexW, SYNCHRONIZATION_SYNCHRONIZE,
};
use windows_sys::Win32::UI::Shell::{
    NIF_INFO, NIIF_RESPECT_QUIET_TIME, NIIF_WARNING, NIM_MODIFY, NOTIFYICONDATAW, Shell_NotifyIconW,
};
use windows_sys::Win32::UI::WindowsAndMessaging::{
    DispatchMessageW, GetMessageW, GetSystemMetrics, MSG, PostThreadMessageW, SM_CXSMICON, SetCoalescableTimer,
    TranslateMessage, WM_APP, WM_TIMER,
};
use windows_sys::core::PCWSTR;
use windows_sys::w;

const MUTEX_NAME: PCWSTR = w!("akkutray-tray");
/// Posted to the tray thread by a `Notifier`.
const WM_NOTIFY_TRAY: u32 = WM_APP;

/// What woke up the tray thread.
pub enum Event {
    /// The poll interval passed.
    Poll,
    /// Another thread called `Notifier::notify`.
    Notified,
    /// Input for the tray icon or its menu, which has been passed on to them.
    Other,
}

/// The events of the tray thread, which is the thread that creates the tray icon. Ends when the thread is asked to
/// quit.
pub struct Events {
    thread: u32,
}

impl Events {
    /// Starts waking up the current thread every `interval`.
    pub fn new(interval: Duration) -> Self {
        let ms = interval.as_millis() as u32;
        // The tolerance lets Windows batch our wake-ups with other timers to save power.
        unsafe { SetCoalescableTimer(null_mut(), 0, ms, None, ms / 10) };
        Self {
            thread: unsafe { GetCurrentThreadId() },
        }
    }

    pub fn notifier(&self) -> Notifier {
        Notifier(self.thread)
    }
}

impl Iterator for Events {
    type Item = Event;

    fn next(&mut self) -> Option<Event> {
        let mut msg = MSG::default();
        if unsafe { GetMessageW(&mut msg, null_mut(), 0, 0) } <= 0 {
            return None;
        }
        Some(match msg.message {
            WM_TIMER if msg.hwnd.is_null() => Event::Poll,
            WM_NOTIFY_TRAY if msg.hwnd.is_null() => Event::Notified,
            _ => {
                unsafe {
                    TranslateMessage(&msg);
                    DispatchMessageW(&msg);
                }
                Event::Other
            }
        })
    }
}

/// Wakes up the tray thread from any thread.
#[derive(Clone)]
pub struct Notifier(u32);

impl Notifier {
    pub fn notify(&self) {
        unsafe { PostThreadMessageW(self.0, WM_NOTIFY_TRAY, 0, 0) };
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

/// Marks this process as the running tray. Returns false if another tray did so first.
pub fn claim() -> bool {
    // The mutex is never closed, so it lasts as long as the process.
    unsafe {
        CreateMutexW(null(), 0, MUTEX_NAME);
        GetLastError() != ERROR_ALREADY_EXISTS
    }
}

/// Shows a warning from the tray icon, which Windows shows as a notification.
pub fn notify(tray: &TrayIcon, title: &str, text: &str) {
    let mut data = NOTIFYICONDATAW {
        cbSize: size_of::<NOTIFYICONDATAW>() as u32,
        hWnd: tray.window_handle(),
        uFlags: NIF_INFO,
        dwInfoFlags: NIIF_WARNING | NIIF_RESPECT_QUIET_TIME,
        ..Default::default()
    };
    copy_wide(&mut data.szInfoTitle, title);
    copy_wide(&mut data.szInfo, text);
    // tray-icon doesn't expose the id it registers the icon with. It numbers its icons and ids from 1, and the window
    // holds only this icon, so the first id Windows accepts is the icon's.
    for id in 1..=16 {
        data.uID = id;
        if unsafe { Shell_NotifyIconW(NIM_MODIFY, &data) } != 0 {
            return;
        }
    }
}

/// Copies `from` into a fixed-size, null-terminated string, cutting it off if it doesn't fit.
fn copy_wide(to: &mut [u16], from: &str) {
    let len = to.len() - 1;
    for (to, from) in to[..len].iter_mut().zip(from.encode_utf16()) {
        *to = from;
    }
}

/// Size of the tray icon in pixels, which is square.
pub fn icon_size() -> usize {
    unsafe { GetSystemMetrics(SM_CXSMICON) as usize }
}

/// Whether the process has a console window, which a tray running in it would keep open.
pub fn has_console() -> bool {
    !unsafe { GetConsoleWindow() }.is_null()
}

/// Makes a command start its process without a console.
pub fn detach(command: &mut Command) -> &mut Command {
    command.creation_flags(DETACHED_PROCESS)
}
