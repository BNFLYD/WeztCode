use crate::gui::GuiPlatform;
use crate::gui::protocol::wayland::wm::WindowGeometry;
use gtk4::prelude::*;
use gtk4::{Application, ApplicationWindow, gdk};
use gtk4_layer_shell::{Edge, Layer, LayerShell};
use webkit6::prelude::*;
use webkit6::WebView;
use std::cell::RefCell;
use std::rc::Rc;

pub struct Gtk4Platform {
    app: Application,
    window: Rc<RefCell<Option<ApplicationWindow>>>,
    webview: Rc<RefCell<Option<WebView>>>,
}

impl Gtk4Platform {
    pub fn new() -> Self {
        let app = Application::builder()
            .application_id("com.weztcode.app")
            .build();

        Self {
            app,
            window: Rc::new(RefCell::new(None)),
            webview: Rc::new(RefCell::new(None)),
        }
    }

    /// Detect monitor geometry using GTK4/GDK and return WindowGeometry
    fn detect_monitor_geometry(window: &ApplicationWindow) -> Option<WindowGeometry> {
        if let Some(display) = gdk::Display::default() {
            if let Some(surface) = window.surface() {
                if let Some(monitor) = display.monitor_at_surface(&surface) {
                    let geo = monitor.geometry();
                    println!("[GTK] Monitor detected: {}x{} at x={}, y={}",
                             geo.width(), geo.height(), geo.x(), geo.y());

                    // Log additional monitor info
                    println!("[GTK] Monitor scale factor: {}", monitor.scale_factor());
                    if let Some(manufacturer) = monitor.manufacturer() {
                        println!("[GTK] Monitor manufacturer: {}", manufacturer);
                    }
                    if let Some(model) = monitor.model() {
                        println!("[GTK] Monitor model: {}", model);
                    }

                    return Some(WindowGeometry::new(
                        geo.x(),
                        geo.y(),
                        geo.width(),
                        geo.height(),
                    ));
                } else {
                    println!("[GTK] WARNING: Could not detect monitor at surface");
                }
            } else {
                println!("[GTK] WARNING: Window has no surface yet");
            }
        } else {
            println!("[GTK] WARNING: No default display available");
        }
        None
    }

    /// Calculate canvas margins based on monitor and terminal geometry
    /// Phase 1: Only calculates top and bottom margins
    /// Side margins (left/right) are kept unchanged from existing behavior
    fn calculate_canvas_margins(
        monitor_geo: &WindowGeometry,
        terminal_geo: &WindowGeometry,
    ) -> (i32, i32, i32, i32) {
        // Calculate top margin: space between terminal top and monitor top
        let margin_top = terminal_geo.y - monitor_geo.y;

        // Calculate bottom margin: space between terminal bottom and monitor bottom
        let terminal_bottom = terminal_geo.y + terminal_geo.height;
        let monitor_bottom = monitor_geo.y + monitor_geo.height;
        let margin_bottom = monitor_bottom - terminal_bottom;

        // Phase 1: Side margins use existing behavior (no dynamic calculation yet)
        // These values will be applied separately, not returned here
        let margin_left = 0;  // Keep existing behavior
        let margin_right = 0; // Keep existing behavior

        println!("[GTK] Canvas margins calculated: top={}, bottom={}, left={}, right={}",
                 margin_top, margin_bottom, margin_left, margin_right);
        println!("[GTK] Terminal position: x={}, y={}, w={}, h={}",
                 terminal_geo.x, terminal_geo.y, terminal_geo.width, terminal_geo.height);
        println!("[GTK] Monitor position: x={}, y={}, w={}, h={}",
                 monitor_geo.x, monitor_geo.y, monitor_geo.width, monitor_geo.height);

        (margin_top, margin_bottom, margin_left, margin_right)
    }
}

impl GuiPlatform for Gtk4Platform {
    fn create_overlay(&self, url: &str, term_geometry: Option<WindowGeometry>) -> Result<(), String> {
        let window_ref = self.window.clone();
        let webview_ref = self.webview.clone();
        let url = url.to_string();
        let term_geometry_clone = term_geometry.clone();

        self.app.connect_activate(move |app| {
            // Calculate initial size based on terminal geometry if available
            let (initial_width, initial_height) = if let Some(ref geo) = term_geometry_clone {
                let width = ((geo.width as f32) * 0.20).max(350.0) as i32;
                println!("[GTK] Calculating initial size: terminal {}x{} -> overlay {}x{}",
                         geo.width, geo.height, width, geo.height);
                (width, geo.height)
            } else {
                println!("[GTK] Using default size 350x600 (no terminal geometry available)");
                (350, 600)
            };

            let window = ApplicationWindow::builder()
                .application(app)
                .title("WeztCode")
                .default_width(initial_width)
                .default_height(initial_height)
                .build();

            window.init_layer_shell();
            window.set_layer(Layer::Top);
            window.set_anchor(Edge::Right, true);
            // Anchor to top and bottom to adapt to margins
            window.set_anchor(Edge::Top, true);
            window.set_anchor(Edge::Bottom, true);

            window.present();

            // Get monitor geometry after window is shown
            if let (Some(monitor_geo), Some(term_geo)) = (Self::detect_monitor_geometry(&window), &term_geometry) {
                // Calculate canvas margins using monitor + terminal geometry
                let (margin_top, margin_bottom, _margin_left, _margin_right) =
                    Self::calculate_canvas_margins(&monitor_geo, term_geo);

                println!("[GTK] Applying canvas margins: top={}, bottom={}", margin_top, margin_bottom);
                window.set_margin(Edge::Top, margin_top);
                window.set_margin(Edge::Bottom, margin_bottom);
            } else if let Some(geo) = &term_geometry {
                // Fallback: Use terminal geometry only (existing behavior)
                println!("[GTK] Fallback to terminal-only margins for x={}, y={}, w={}, h={}",
                         geo.x, geo.y, geo.width, geo.height);
                window.set_margin(Edge::Top, geo.y);
                window.set_margin(Edge::Bottom, 0);
            } else {
                // Default values when no geometry available
                println!("[GTK] Using default margins (no geometry available)");
                window.set_margin(Edge::Top, 1);
                window.set_margin(Edge::Bottom, 1);
            }

            // Exclusive zone 0 = behaves well with other windows
            window.set_exclusive_zone(0);

            println!("GTK: Creating WebView...");
            let webview = WebView::new();

            println!("GTK: Loading URL: {}", &url);
            webview.load_uri(&url);

            println!("GTK: Adding WebView to window...");
            window.set_child(Some(&webview));

            *window_ref.borrow_mut() = Some(window.clone());
            *webview_ref.borrow_mut() = Some(webview);
        });

        Ok(())
    }

    fn set_geometry(&self, x: i32, y: i32, width: u32, height: u32) {
        if let Some(ref window) = *self.window.borrow() {
            window.set_default_size(width as i32, height as i32);
            window.set_margin(Edge::Top, y);
            window.set_margin(Edge::Right, x);
        }
    }

    fn show(&self) {
        if let Some(ref window) = *self.window.borrow() {
            window.present();
        }
    }

    fn hide(&self) {
        if let Some(ref window) = *self.window.borrow() {
            window.set_visible(false);
        }
    }

    fn is_available() -> bool {
        true
    }
}

impl Gtk4Platform {
    pub fn run(&self) {
        self.app.run();
    }

    /// Handle WM events and update window visibility accordingly
    pub fn handle_wm_events(&self, receiver: std::sync::mpsc::Receiver<crate::gui::protocol::wayland::wm::WmEvent>) {
        use gtk4::glib;
        use crate::gui::protocol::wayland::wm::WmEvent;

        let window_weak = self.window.clone();

        glib::idle_add_local(move || {
            match receiver.try_recv() {
                Ok(WmEvent::WindowFocused { app_id }) => {
                    // Terminal gained focus - SHOW overlay
                    println!("[GTK] WindowFocused event received for {}", app_id);
                    if let Ok(window_ref) = window_weak.try_borrow() {
                        if let Some(ref window) = *window_ref {
                            println!("[GTK] Setting visible=true and presenting");
                            window.set_visible(true);
                            window.present();
                            println!("[GTK] Overlay should be visible now");
                        } else {
                            println!("[GTK] ERROR: Window is None");
                        }
                    } else {
                        println!("[GTK] ERROR: Failed to borrow window");
                    }
                }
                Ok(WmEvent::WindowUnfocused { app_id }) => {
                    // Terminal lost focus - HIDE overlay
                    println!("[GTK] WindowUnfocused event received for {}", app_id);
                    if let Ok(window_ref) = window_weak.try_borrow() {
                        if let Some(ref window) = *window_ref {
                            println!("[GTK] Setting visible=false");
                            window.set_visible(false);
                            println!("[GTK] Overlay should be hidden now");
                        } else {
                            println!("[GTK] ERROR: Window is None");
                        }
                    } else {
                        println!("[GTK] ERROR: Failed to borrow window");
                    }
                }
                Ok(WmEvent::GeometryChanged { app_id, geometry }) => {
                    println!("[GTK] GeometryChanged for {}: {:?}", app_id, geometry);
                    if let Ok(window_ref) = window_weak.try_borrow() {
                        if let Some(ref window) = *window_ref {
                            // Calculate proportional width: 20% of terminal width, min 350px
                            let overlay_width = ((geometry.width as f32) * 0.20).max(350.0) as i32;
                            let overlay_height = geometry.height;

                            println!("[GTK] Resizing overlay to {}x{} (terminal: {}x{} at x={}, y={})",
                                     overlay_width, overlay_height, geometry.width, geometry.height, geometry.x, geometry.y);

                            window.set_default_size(overlay_width, overlay_height);
                            // TODO: Positioning to be implemented
                        }
                    }
                }
                Ok(WmEvent::FullscreenChanged { app_id, geometry, is_fullscreen }) => {
                    println!("[GTK] FullscreenChanged for {}: fullscreen={}", app_id, is_fullscreen);
                    if let Ok(window_ref) = window_weak.try_borrow() {
                        if let Some(ref window) = *window_ref {
                            let overlay_width = ((geometry.width as f32) * 0.20).max(350.0) as i32;
                            let overlay_height = geometry.height;

                            println!("[GTK] Fullscreen mode: {}, resizing to {}x{}",
                                     is_fullscreen, overlay_width, overlay_height);

                            window.set_default_size(overlay_width, overlay_height);
                            // TODO: Layer shell switching to be implemented later
                            // TODO: Positioning to be implemented later
                        }
                    }
                }
                Err(std::sync::mpsc::TryRecvError::Empty) => {
                    // No events, this is normal
                }
                Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                    println!("[GTK] ERROR: Channel disconnected!");
                    return glib::ControlFlow::Break;
                }
                _ => {
                    // Ignore other events (WindowCreated, WindowDestroyed, etc.)
                }
            }
            glib::ControlFlow::Continue
        });
    }
}
