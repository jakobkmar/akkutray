//! What egui doesn't cover for the popup: docking it to the panel through the layer shell protocol of Wayland, and the
//! fonts of the desktop, through fontconfig.

use crate::popup::{Bounds, Font, SCREEN_MARGIN};
use eframe::egui::{self, Event, Key, Modifiers, PointerButton, Pos2, ViewportCommand, ViewportId};
use glutin::config::ConfigTemplateBuilder;
use glutin::context::{ContextAttributesBuilder, PossiblyCurrentContext};
use glutin::display::{Display, DisplayApiPreference};
use glutin::prelude::*;
use glutin::surface::{Surface, SurfaceAttributesBuilder, WindowSurface};
use raw_window_handle::{
    HasWindowHandle, RawDisplayHandle, RawWindowHandle, WaylandDisplayHandle, WaylandWindowHandle,
};
use smithay_client_toolkit::compositor::{CompositorHandler, CompositorState};
use smithay_client_toolkit::output::{OutputHandler, OutputState};
use smithay_client_toolkit::reexports::calloop::EventLoop;
use smithay_client_toolkit::reexports::calloop::ping::make_ping;
use smithay_client_toolkit::reexports::calloop_wayland_source::WaylandSource;
use smithay_client_toolkit::reexports::client::globals::registry_queue_init;
use smithay_client_toolkit::reexports::client::protocol::{wl_keyboard, wl_output, wl_pointer, wl_seat, wl_surface};
use smithay_client_toolkit::reexports::client::{Connection, Dispatch, Proxy, QueueHandle, WEnum};
use smithay_client_toolkit::registry::{ProvidesRegistryState, RegistryState};
use smithay_client_toolkit::seat::pointer::{
    CursorIcon, PointerEvent, PointerEventKind, PointerHandler, ThemeSpec, ThemedPointer,
};
use smithay_client_toolkit::seat::{Capability, SeatHandler, SeatState};
use smithay_client_toolkit::shell::WaylandSurface;
use smithay_client_toolkit::shell::wlr_layer::{
    Anchor, KeyboardInteractivity, Layer, LayerShell, LayerShellHandler, LayerSurface, LayerSurfaceConfigure,
};
use smithay_client_toolkit::shm::{Shm, ShmHandler};
use smithay_client_toolkit::{
    delegate_compositor, delegate_layer, delegate_output, delegate_pointer, delegate_registry, delegate_seat,
    delegate_shm, registry_handlers,
};
use std::ffi::c_void;
use std::num::NonZeroU32;
use std::process::Command;
use std::ptr::NonNull;
use std::sync::Arc;
use std::time::Instant;

/// Codes of the Linux input events, which Wayland passes on as they are.
const KEY_ESC: u32 = 1;
const BTN_LEFT: u32 = 0x110;
const BTN_RIGHT: u32 = 0x111;
/// Gap between the popup and the top of the screen where no panel reserves space there, in points. Such panels, e.g.
/// floating or hiding ones, rarely grow taller, so the popup stays clear of them.
const UNRESERVED_GAP: i32 = 48;

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

/// Linux has no common way to tell the work area, and Wayland doesn't let windows place themselves at all. The popup
/// docks to the panel instead where it can, and the system places it where it can't.
pub fn work_area(_x: i32, _y: i32) -> Option<Bounds> {
    None
}

/// Linux desktops share no setting for animations.
pub fn animations_enabled() -> bool {
    true
}

/// The desktop's light or dark mode, from the XDG desktop portal.
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

/// The popup as a layer surface, which Wayland desktops other than GNOME offer for panels and their popups. The
/// compositor keeps it clear of the panel.
pub struct Dock {
    // EGL cleans up through the Wayland connection, so it has to go before the event loop and the state, which hold
    // the connection.
    gl: Gl,
    event_loop: EventLoop<'static, State>,
    state: State,
}

impl Dock {
    /// Returns `None` where the desktop doesn't support the layer shell.
    pub fn new() -> Option<Self> {
        let connection = Connection::connect_to_env().ok()?;
        let (globals, mut queue) = registry_queue_init(&connection).ok()?;
        let qh = queue.handle();
        let layer_shell = LayerShell::bind(&globals, &qh).ok()?;
        let compositor = CompositorState::bind(&globals, &qh).ok()?;
        let layer = layer_shell.create_layer_surface(
            &qh,
            compositor.create_surface(&qh),
            Layer::Overlay,
            Some("akkutray"),
            None,
        );
        let mut state = State {
            registry: RegistryState::new(&globals),
            seat: SeatState::new(&globals, &qh),
            output: OutputState::new(&globals, &qh),
            shm: Shm::bind(&globals, &qh).ok()?,
            compositor,
            connection: connection.clone(),
            layer,
            pointer: None,
            keyboard: None,
            size: None,
            scale: 1,
            focused: None,
            events: Vec::new(),
            repaint: Some(Instant::now()),
            closed: false,
        };
        // Panels don't tell where they are, but those that reserve space shrink the area left to other surfaces. So
        // the popup asks for the whole screen, once with that space and once without, before it shows.
        state.layer.set_anchor(Anchor::all());
        state.layer.set_exclusive_zone(-1);
        state.layer.commit();
        queue.roundtrip(&mut state).ok()?;
        let screen = state.size.take();
        state.layer.set_exclusive_zone(0);
        state.layer.commit();
        queue.roundtrip(&mut state).ok()?;
        // The compositor only answers if the area changed.
        let reserved = state.size.take().is_some_and(|free| Some(free) != screen);
        // ponytail: hangs from the middle of the top edge, as tray hosts don't tell where their icons are
        state.layer.set_anchor(Anchor::TOP);
        let margin = SCREEN_MARGIN as i32;
        let top = if reserved { 0 } else { UNRESERVED_GAP };
        state.layer.set_margin(top, margin, margin, margin);
        // Takes the keyboard to get the focus when it opens, then lets it go to other windows, which closes the popup.
        state.layer.set_keyboard_interactivity(KeyboardInteractivity::Exclusive);
        // Without both side anchors, the size can't be left to the compositor. The popup sets its own on its first frame.
        state.layer.set_size(1, 1);
        state.layer.commit();
        let gl = Gl::new(&connection, state.layer.wl_surface())?;
        let event_loop = EventLoop::try_new().ok()?;
        WaylandSource::new(connection.clone(), queue)
            .insert(event_loop.handle())
            .ok()?;
        Some(Self { event_loop, state, gl })
    }

    /// Shows the popup until it closes.
    pub fn run(
        mut self,
        ctx: &egui::Context,
        mut ui: impl FnMut(&mut egui::Ui),
        background: fn(&egui::Visuals) -> [f32; 4],
    ) {
        let (ping, wake) = make_ping().expect("failed to create a ping");
        let _ = self.event_loop.handle().insert_source(wake, |_, _, state: &mut State| {
            state.repaint = Some(Instant::now());
        });
        // Readings from the scanning thread repaint the popup.
        ctx.set_request_repaint_callback(move |_| ping.ping());
        let start = Instant::now();
        while !self.state.closed {
            let timeout = self
                .state
                .repaint
                .map(|at| at.saturating_duration_since(Instant::now()));
            if self.event_loop.dispatch(timeout, &mut self.state).is_err() {
                break;
            }
            let Some((width, height)) = self.state.size else {
                continue;
            };
            if self.state.repaint.is_none_or(|at| at > Instant::now()) {
                continue;
            }
            let mut input = egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    Pos2::ZERO,
                    egui::vec2(width as f32, height as f32),
                )),
                time: Some(start.elapsed().as_secs_f64()),
                events: std::mem::take(&mut self.state.events),
                focused: self.state.focused == Some(true),
                ..Default::default()
            };
            let viewport = input.viewports.entry(ViewportId::ROOT).or_default();
            viewport.native_pixels_per_point = Some(self.state.scale as f32);
            viewport.focused = self.state.focused;
            let mut output = ctx.run_ui(input, &mut ui);
            let Some(viewport) = output.viewport_output.remove(&ViewportId::ROOT) else {
                break;
            };
            for command in viewport.commands {
                match command {
                    ViewportCommand::Close => self.state.closed = true,
                    ViewportCommand::InnerSize(size) => {
                        self.state.layer.set_size(size.x.round() as u32, size.y.round() as u32);
                        self.state.layer.commit();
                    }
                    _ => {}
                }
            }
            self.state.set_cursor(output.platform_output.cursor_icon);
            let pixels = [width * self.state.scale as u32, height * self.state.scale as u32];
            let primitives = ctx.tessellate(output.shapes, output.pixels_per_point);
            self.state.layer.wl_surface().set_buffer_scale(self.state.scale);
            self.gl
                .paint(pixels, background(&ctx.global_style().visuals), |painter| {
                    painter.paint_and_update_textures(
                        pixels,
                        output.pixels_per_point,
                        &primitives,
                        &mut output.textures_delta,
                    )
                });
            self.state.repaint = Instant::now().checked_add(viewport.repaint_delay);
        }
        self.gl.painter.destroy();
    }
}

/// OpenGL on the layer surface, through EGL.
struct Gl {
    surface: Surface<WindowSurface>,
    context: PossiblyCurrentContext,
    painter: egui_glow::Painter,
}

impl Gl {
    fn new(connection: &Connection, surface: &wl_surface::WlSurface) -> Option<Self> {
        let display = NonNull::new(connection.backend().display_ptr().cast::<c_void>())?;
        let window = NonNull::new(surface.id().as_ptr().cast::<c_void>())?;
        let window = RawWindowHandle::Wayland(WaylandWindowHandle::new(window));
        // Safe as the connection and the surface outlive the display, which `Dock` drops before them.
        let display = unsafe {
            Display::new(
                RawDisplayHandle::Wayland(WaylandDisplayHandle::new(display)),
                DisplayApiPreference::Egl,
            )
        }
        .ok()?;
        let config = unsafe { display.find_configs(ConfigTemplateBuilder::new().with_alpha_size(8).build()) }
            .ok()?
            .next()?;
        let context =
            unsafe { display.create_context(&config, &ContextAttributesBuilder::new().build(Some(window))) }.ok()?;
        let attributes =
            SurfaceAttributesBuilder::<WindowSurface>::new().build(window, NonZeroU32::MIN, NonZeroU32::MIN);
        let surface = unsafe { display.create_window_surface(&config, &attributes) }.ok()?;
        let context = context.make_current(&surface).ok()?;
        let gl = unsafe { egui_glow::glow::Context::from_loader_function_cstr(|name| display.get_proc_address(name)) };
        let painter = egui_glow::Painter::new(Arc::new(gl), "", None, false).ok()?;
        Some(Self {
            surface,
            context,
            painter,
        })
    }

    fn paint(&mut self, [width, height]: [u32; 2], background: [f32; 4], paint: impl FnOnce(&mut egui_glow::Painter)) {
        let size = |pixels| NonZeroU32::new(pixels).unwrap_or(NonZeroU32::MIN);
        self.surface.resize(&self.context, size(width), size(height));
        self.painter.clear([width, height], background);
        paint(&mut self.painter);
        let _ = self.surface.swap_buffers(&self.context);
    }
}

struct State {
    registry: RegistryState,
    seat: SeatState,
    output: OutputState,
    shm: Shm,
    compositor: CompositorState,
    connection: Connection,
    layer: LayerSurface,
    pointer: Option<ThemedPointer>,
    keyboard: Option<wl_keyboard::WlKeyboard>,
    /// The size the compositor gave the popup, in points, once it did.
    size: Option<(u32, u32)>,
    scale: i32,
    focused: Option<bool>,
    events: Vec<Event>,
    /// When to paint the next frame.
    repaint: Option<Instant>,
    closed: bool,
}

impl State {
    fn set_cursor(&self, icon: egui::CursorIcon) {
        let icon = match icon {
            egui::CursorIcon::PointingHand => CursorIcon::Pointer,
            _ => CursorIcon::Default,
        };
        if let Some(pointer) = &self.pointer {
            let _ = pointer.set_cursor(&self.connection, icon);
        }
    }

    fn input(&mut self, event: Event) {
        self.events.push(event);
        self.repaint = Some(Instant::now());
    }
}

impl LayerShellHandler for State {
    fn closed(&mut self, _: &Connection, _: &QueueHandle<Self>, _: &LayerSurface) {
        self.closed = true;
    }

    fn configure(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        _: &LayerSurface,
        configure: LayerSurfaceConfigure,
        _: u32,
    ) {
        // Before the popup asks for a size, the compositor leaves it to the popup.
        let (width, height) = configure.new_size;
        self.size = Some((width.max(1), height.max(1)));
        self.repaint = Some(Instant::now());
    }
}

impl CompositorHandler for State {
    fn scale_factor_changed(&mut self, _: &Connection, _: &QueueHandle<Self>, _: &wl_surface::WlSurface, scale: i32) {
        self.scale = scale;
        self.repaint = Some(Instant::now());
    }

    fn transform_changed(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        _: &wl_surface::WlSurface,
        _: wl_output::Transform,
    ) {
    }

    fn frame(&mut self, _: &Connection, _: &QueueHandle<Self>, _: &wl_surface::WlSurface, _: u32) {}

    fn surface_enter(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        _: &wl_surface::WlSurface,
        _: &wl_output::WlOutput,
    ) {
    }

    fn surface_leave(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        _: &wl_surface::WlSurface,
        _: &wl_output::WlOutput,
    ) {
    }
}

impl SeatHandler for State {
    fn seat_state(&mut self) -> &mut SeatState {
        &mut self.seat
    }

    fn new_seat(&mut self, _: &Connection, _: &QueueHandle<Self>, _: wl_seat::WlSeat) {}

    fn new_capability(
        &mut self,
        _: &Connection,
        qh: &QueueHandle<Self>,
        seat: wl_seat::WlSeat,
        capability: Capability,
    ) {
        if capability == Capability::Pointer && self.pointer.is_none() {
            let surface = self.compositor.create_surface(qh);
            self.pointer = self
                .seat
                .get_pointer_with_theme(qh, &seat, self.shm.wl_shm(), surface, ThemeSpec::System)
                .ok();
        }
        if capability == Capability::Keyboard && self.keyboard.is_none() {
            self.keyboard = Some(seat.get_keyboard(qh, ()));
        }
    }

    fn remove_capability(&mut self, _: &Connection, _: &QueueHandle<Self>, _: wl_seat::WlSeat, _: Capability) {}

    fn remove_seat(&mut self, _: &Connection, _: &QueueHandle<Self>, _: wl_seat::WlSeat) {}
}

impl PointerHandler for State {
    fn pointer_frame(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        _: &wl_pointer::WlPointer,
        events: &[PointerEvent],
    ) {
        for event in events {
            let pos = Pos2::new(event.position.0 as f32, event.position.1 as f32);
            let button = |pressed, button| {
                let button = match button {
                    BTN_LEFT => PointerButton::Primary,
                    BTN_RIGHT => PointerButton::Secondary,
                    _ => PointerButton::Middle,
                };
                Event::PointerButton {
                    pos,
                    button,
                    pressed,
                    modifiers: Modifiers::NONE,
                }
            };
            match event.kind {
                PointerEventKind::Enter { .. } => {
                    self.set_cursor(egui::CursorIcon::Default);
                    self.input(Event::PointerMoved(pos));
                }
                PointerEventKind::Motion { .. } => self.input(Event::PointerMoved(pos)),
                PointerEventKind::Leave { .. } => self.input(Event::PointerGone),
                PointerEventKind::Press { button: code, .. } => self.input(button(true, code)),
                PointerEventKind::Release { button: code, .. } => self.input(button(false, code)),
                PointerEventKind::Axis { .. } => {}
            }
        }
    }
}

impl Dispatch<wl_keyboard::WlKeyboard, ()> for State {
    fn event(
        state: &mut Self,
        _: &wl_keyboard::WlKeyboard,
        event: wl_keyboard::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        match event {
            wl_keyboard::Event::Enter { .. } => {
                if state.focused.is_none() {
                    state.layer.set_keyboard_interactivity(KeyboardInteractivity::OnDemand);
                    state.layer.commit();
                }
                state.focused = Some(true);
                state.repaint = Some(Instant::now());
            }
            wl_keyboard::Event::Leave { .. } => {
                state.focused = Some(false);
                state.repaint = Some(Instant::now());
            }
            wl_keyboard::Event::Key {
                key: KEY_ESC,
                state: WEnum::Value(key_state),
                ..
            } => state.input(Event::Key {
                key: Key::Escape,
                physical_key: Some(Key::Escape),
                pressed: key_state == wl_keyboard::KeyState::Pressed,
                repeat: false,
                modifiers: Modifiers::NONE,
            }),
            _ => {}
        }
    }
}

impl OutputHandler for State {
    fn output_state(&mut self) -> &mut OutputState {
        &mut self.output
    }

    fn new_output(&mut self, _: &Connection, _: &QueueHandle<Self>, _: wl_output::WlOutput) {}

    fn update_output(&mut self, _: &Connection, _: &QueueHandle<Self>, _: wl_output::WlOutput) {}

    fn output_destroyed(&mut self, _: &Connection, _: &QueueHandle<Self>, _: wl_output::WlOutput) {}
}

impl ShmHandler for State {
    fn shm_state(&mut self) -> &mut Shm {
        &mut self.shm
    }
}

impl ProvidesRegistryState for State {
    fn registry(&mut self) -> &mut RegistryState {
        &mut self.registry
    }

    registry_handlers![OutputState, SeatState];
}

delegate_compositor!(State);
delegate_output!(State);
delegate_seat!(State);
delegate_pointer!(State);
delegate_shm!(State);
delegate_layer!(State);
delegate_registry!(State);
