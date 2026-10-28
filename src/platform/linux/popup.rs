//! What egui doesn't cover for the popup: docking it to the panel through the layer shell of Wayland, the work area
//! of X11, the desktop's light or dark mode and its fonts.

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
use x11rb::connection::Connection as _;
use x11rb::protocol::xproto::{AtomEnum, ConnectionExt};

/// Codes of the Linux input events, which Wayland passes on as they are.
const KEY_ESC: u32 = 1;
const BTN_LEFT: u32 = 0x110;

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

/// The popup as a layer surface, the kind panels use for their own popups. Unlike a window, it stays above the other
/// windows, gets the keyboard when it opens and sits where it asks to.
pub struct Dock {
    // EGL cleans up through the Wayland surface and connection, so it has to go before the layer, the event loop and
    // the state, which hold them.
    gl: Gl,
    layer: LayerSurface,
    place: Place,
    connection: Connection,
    event_loop: EventLoop<'static, State>,
    state: State,
}

impl Dock {
    /// Docks the popup to the panel at `icon`, the click on the tray icon in screen coordinates. Returns `None` where
    /// the desktop lacks the layer shell, such as GNOME, or has a version before 4, the first that lets the popup give
    /// the keyboard back.
    pub fn new(icon: (i32, i32)) -> Option<Self> {
        let connection = Connection::connect_to_env().ok()?;
        let (globals, mut queue) = registry_queue_init(&connection).ok()?;
        let current = globals.contents().with_list(|list| {
            list.iter()
                .any(|global| global.interface == "zwlr_layer_shell_v1" && global.version >= 4)
        });
        if !current {
            return None;
        }
        let qh = queue.handle();
        let layer_shell = LayerShell::bind(&globals, &qh).ok()?;
        let mut state = State {
            registry: RegistryState::new(&globals),
            seat: SeatState::new(&globals, &qh),
            output: OutputState::new(&globals, &qh),
            shm: Shm::bind(&globals, &qh).ok()?,
            compositor: CompositorState::bind(&globals, &qh).ok()?,
            pointer: None,
            keyboard: None,
            size: None,
            scale: 1,
            focused: None,
            events: Vec::new(),
            repaint: Some(Instant::now()),
            closed: false,
        };
        // Learns where the screens are.
        queue.roundtrip(&mut state).ok()?;
        let (output, place) = Place::new(&state.output, icon);
        let surface = state.compositor.create_surface(&qh);
        let layer = layer_shell.create_layer_surface(&qh, surface, Layer::Overlay, Some("akkutray"), output.as_ref());
        layer.set_keyboard_interactivity(KeyboardInteractivity::Exclusive);
        // The popup takes its real size on its first frame.
        place.apply(&layer, 1, 1);
        layer.commit();
        let gl = Gl::new(&connection, layer.wl_surface())?;
        let event_loop = EventLoop::try_new().ok()?;
        WaylandSource::new(connection.clone(), queue)
            .insert(event_loop.handle())
            .ok()?;
        Some(Self {
            gl,
            layer,
            place,
            connection,
            event_loop,
            state,
        })
    }

    /// Shows the popup until it closes.
    pub fn run(mut self, ctx: &egui::Context, background: [f32; 4], mut ui: impl FnMut(&mut egui::Ui)) {
        let Ok((ping, wake)) = make_ping() else {
            return;
        };
        let woken = self.event_loop.handle().insert_source(wake, |_, _, state: &mut State| {
            state.repaint = Some(Instant::now());
        });
        if woken.is_err() {
            return;
        }
        // Wakes up the popup for readings from the scanning thread. Its own requests come back with each frame.
        ctx.set_request_repaint_callback(move |info| {
            if info.delay.is_zero() {
                ping.ping();
            }
        });
        let start = Instant::now();
        let mut keyboard_released = false;
        while !self.state.closed {
            let timeout = self
                .state
                .repaint
                .map(|at| at.saturating_duration_since(Instant::now()));
            if self.event_loop.dispatch(timeout, &mut self.state).is_err() {
                break;
            }
            // Once the popup has the keyboard, other surfaces may take it, which closes the popup.
            if self.state.focused == Some(true) && !keyboard_released {
                self.layer.set_keyboard_interactivity(KeyboardInteractivity::OnDemand);
                self.layer.commit();
                keyboard_released = true;
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
                        self.place
                            .apply(&self.layer, size.x.round() as u32, size.y.round() as u32);
                        self.layer.commit();
                    }
                    _ => {}
                }
            }
            if let Some(pointer) = &self.state.pointer {
                let icon = match output.platform_output.cursor_icon {
                    egui::CursorIcon::PointingHand => CursorIcon::Pointer,
                    _ => CursorIcon::Default,
                };
                let _ = pointer.set_cursor(&self.connection, icon);
            }
            let scale = self.state.scale;
            let pixels = [width * scale as u32, height * scale as u32];
            let primitives = ctx.tessellate(output.shapes, output.pixels_per_point);
            self.layer.wl_surface().set_buffer_scale(scale);
            self.gl.paint(pixels, background, |painter| {
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

/// Where the popup docks: at the screen edge nearest to the tray icon, centered on it.
struct Place {
    edge: Anchor,
    /// Distance from the edge, in points.
    gap: i32,
    /// The icon's position along the edge and the edge's length, in points. Without them, the popup docks to the
    /// middle of the edge.
    along: Option<(i32, i32)>,
}

impl Place {
    fn new(outputs: &OutputState, (x, y): (i32, i32)) -> (Option<wl_output::WlOutput>, Self) {
        let margin = SCREEN_MARGIN as i32;
        // Tray hosts tell where their icon was clicked, except COSMIC's, whose panel sits at the top by default.
        let screen = outputs.outputs().filter(|_| (x, y) != (0, 0)).find_map(|output| {
            let info = outputs.info(&output)?;
            let ((left, top), (width, height)) = (info.logical_position?, info.logical_size?);
            let (x, y) = (x - left, y - top);
            ((0..width).contains(&x) && (0..height).contains(&y)).then_some((output, x, y, width, height))
        });
        let Some((output, x, y, width, height)) = screen else {
            let place = Self {
                edge: Anchor::TOP,
                gap: margin,
                along: None,
            };
            return (None, place);
        };
        let edges = [
            (Anchor::TOP, y, x, width),
            (Anchor::BOTTOM, height - y, x, width),
            (Anchor::LEFT, x, y, height),
            (Anchor::RIGHT, width - x, y, height),
        ];
        let (edge, distance, along, length) = edges.into_iter().min_by_key(|&(_, distance, ..)| distance).unwrap();
        // The icon sits in the middle of its panel, so the panel ends twice as far from the edge.
        let place = Self {
            edge,
            gap: 2 * distance + margin,
            along: Some((along, length)),
        };
        (Some(output), place)
    }

    /// Sizes the popup and moves it to its place, which takes effect on the next commit.
    fn apply(&self, layer: &LayerSurface, width: u32, height: u32) {
        layer.set_size(width, height);
        let horizontal = self.edge == Anchor::TOP || self.edge == Anchor::BOTTOM;
        let Some((along, length)) = self.along else {
            // Panels that reserve their space keep the popup clear of them.
            layer.set_exclusive_zone(0);
            layer.set_anchor(self.edge);
            layer.set_margin(self.gap, 0, 0, 0);
            return;
        };
        let size = if horizontal { width } else { height } as i32;
        let margin = SCREEN_MARGIN as i32;
        let offset = (along - size / 2).min(length - size - margin).max(margin);
        // The gap already clears the panel, so it counts from the screen edge.
        layer.set_exclusive_zone(-1);
        layer.set_anchor(self.edge | if horizontal { Anchor::LEFT } else { Anchor::TOP });
        match self.edge {
            Anchor::TOP => layer.set_margin(self.gap, 0, 0, offset),
            Anchor::BOTTOM => layer.set_margin(0, 0, self.gap, offset),
            Anchor::LEFT => layer.set_margin(offset, 0, 0, self.gap),
            _ => layer.set_margin(offset, self.gap, 0, 0),
        }
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
        // Safe as the connection and the surface outlive the EGL objects, which `Dock` drops first.
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
            let click = |pressed| Event::PointerButton {
                pos,
                button: PointerButton::Primary,
                pressed,
                modifiers: Modifiers::NONE,
            };
            match event.kind {
                PointerEventKind::Enter { .. } | PointerEventKind::Motion { .. } => {
                    self.input(Event::PointerMoved(pos))
                }
                PointerEventKind::Leave { .. } => self.input(Event::PointerGone),
                PointerEventKind::Press { button: BTN_LEFT, .. } => self.input(click(true)),
                PointerEventKind::Release { button: BTN_LEFT, .. } => self.input(click(false)),
                _ => {}
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
