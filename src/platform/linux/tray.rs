//! The event loop of the tray thread, a single tray at a time, notifications and starting the tray apart from the
//! terminal.

use crossbeam_channel::{Receiver, Select, Sender, unbounded};
use std::collections::HashMap;
use std::io::IsTerminal;
use std::os::linux::net::SocketAddrExt;
use std::os::unix::fs::MetadataExt;
use std::os::unix::net::{SocketAddr, UnixListener};
use std::os::unix::process::CommandExt;
use std::process::Command;
use std::time::{Duration, Instant};
use tray_icon::menu::MenuEvent;
use tray_icon::{TrayIcon, TrayIconEvent};
use zbus::blocking::Connection;
use zbus::zvariant::Value;

/// What woke up the tray thread.
pub enum Event {
    /// The poll interval passed.
    Poll,
    /// Another thread called `Notifier::notify`.
    Notified,
    /// Input for the tray icon or its menu, which waits in their receivers.
    Other,
}

/// The events of the tray thread. tray-icon runs the tray icon on a thread of its own and queues its input.
pub struct Events {
    interval: Duration,
    next_poll: Instant,
    notifier: Sender<()>,
    notified: Receiver<()>,
}

impl Events {
    /// Starts waking up the current thread every `interval`.
    pub fn new(interval: Duration) -> Self {
        let (notifier, notified) = unbounded();
        Self {
            interval,
            next_poll: Instant::now() + interval,
            notifier,
            notified,
        }
    }

    pub fn notifier(&self) -> Notifier {
        Notifier(self.notifier.clone())
    }
}

impl Iterator for Events {
    type Item = Event;

    fn next(&mut self) -> Option<Event> {
        let mut select = Select::new();
        let notified = select.recv(&self.notified);
        select.recv(MenuEvent::receiver());
        select.recv(TrayIconEvent::receiver());
        // Waits without taking the input, which the caller reads from the receivers.
        Some(match select.ready_deadline(self.next_poll) {
            Err(_) => {
                self.next_poll = Instant::now() + self.interval;
                Event::Poll
            }
            Ok(ready) if ready == notified => {
                self.notified.try_iter().count();
                Event::Notified
            }
            Ok(_) => Event::Other,
        })
    }
}

/// Wakes up the tray thread from any thread.
#[derive(Clone)]
pub struct Notifier(Sender<()>);

impl Notifier {
    pub fn notify(&self) {
        let _ = self.0.send(());
    }
}

pub fn is_running() -> bool {
    // Binding fails while the running tray holds the address, and the listener is closed again right away.
    socket().is_none_or(|address| UnixListener::bind_addr(&address).is_err())
}

/// Marks this process as the running tray. Returns false if another tray did so first.
pub fn claim() -> bool {
    // The listener is never closed, so the address stays taken as long as the process lives.
    let listener = socket().and_then(|address| UnixListener::bind_addr(&address).ok());
    listener.map(std::mem::forget).is_some()
}

/// An abstract socket address, which the system frees when its process ends, one for each user.
fn socket() -> Option<SocketAddr> {
    let user = std::fs::metadata("/proc/self").ok()?.uid();
    SocketAddr::from_abstract_name(format!("akkutray-tray-{user}")).ok()
}

/// Shows a warning as a desktop notification.
pub fn notify(_tray: &TrayIcon, title: &str, text: &str) {
    let Ok(connection) = Connection::session() else {
        return;
    };
    let hints: HashMap<&str, Value> = HashMap::from([("urgency", Value::U8(1))]);
    let _ = connection.call_method(
        Some("org.freedesktop.Notifications"),
        "/org/freedesktop/Notifications",
        Some("org.freedesktop.Notifications"),
        "Notify",
        &(
            "akkutray",
            0u32,
            "battery-low",
            title,
            text,
            Vec::<&str>::new(),
            hints,
            -1i32,
        ),
    );
}

/// Size of the tray icon in pixels, which is square. The panel scales the icon to its own size, so this only needs
/// to be large enough to stay sharp.
pub fn icon_size() -> usize {
    32
}

/// Whether the process runs in a terminal, which would end a tray running in it when it closes.
pub fn has_console() -> bool {
    std::io::stdin().is_terminal()
}

/// Makes a command start its process apart from the terminal, so closing it or pressing Ctrl+C leaves the tray running.
pub fn detach(command: &mut Command) -> &mut Command {
    command.process_group(0)
}
