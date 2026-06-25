use crate::gui::GuiPlatform;
use crate::gui::protocol::wayland::wm::WindowGeometry;
use gtk4::prelude::*;
use gtk4::{Application, ApplicationWindow, gdk};
use gtk4_layer_shell::{Edge, KeyboardMode, Layer, LayerShell};
use webkit6::prelude::*;
use crate::terminal::TerminalProtocol;
use webkit6::WebView;
use webkit6::{UserContentInjectedFrames, UserStyleLevel, UserStyleSheet};
use std::cell::RefCell;
use std::rc::Rc;
use std::thread;
use std::time::{Duration, Instant};

pub struct Gtk4Platform {
    app: Application,
    window: Rc<RefCell<Option<ApplicationWindow>>>,
    webview: Rc<RefCell<Option<WebView>>>,
    monitor_geo: Rc<RefCell<Option<WindowGeometry>>>,
    pending_width: Rc<RefCell<Option<u32>>>,
    debounce_until: Rc<RefCell<Option<Instant>>>,
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
            monitor_geo: Rc::new(RefCell::new(None)),
            pending_width: Rc::new(RefCell::new(None)),
            debounce_until: Rc::new(RefCell::new(None)),
        }
    }

    /// Detect monitor geometry using GTK4/GDK and return WindowGeometry
    fn detect_monitor_geometry(window: &ApplicationWindow) -> Option<WindowGeometry> {
        if let Some(display) = gdk::Display::default() {
            if let Some(surface) = window.surface() {
                if let Some(monitor) = display.monitor_at_surface(&surface) {
                    let geo = monitor.geometry();
//                     println!("[GTK] Monitor detected: {}x{} at x={}, y={}",
//                              geo.width(), geo.height(), geo.x(), geo.y());

                    // Log additional monitor info
//                     println!("[GTK] Monitor scale factor: {}", monitor.scale_factor());
                    if let Some(manufacturer) = monitor.manufacturer() {
//                         println!("[GTK] Monitor manufacturer: {}", manufacturer);
                    }
                    if let Some(model) = monitor.model() {
//                         println!("[GTK] Monitor model: {}", model);
                    }

                    return Some(WindowGeometry::new(
                        geo.x(),
                        geo.y(),
                        geo.width(),
                        geo.height(),
                    ));
                } else {
//                     println!("[GTK] WARNING: Could not detect monitor at surface");
                }
            } else {
//                 println!("[GTK] WARNING: Window has no surface yet");
            }
        } else {
//             println!("[GTK] WARNING: No default display available");
        }
        None
    }

    /// Calculate canvas margins based on monitor and terminal geometry
    /// Phase 2: Calculates top, bottom, and right margins for right-side positioning
    fn calculate_canvas_margins(
        monitor_geo: &WindowGeometry,
        terminal_geo: &WindowGeometry,
        overlay_width: i32,
    ) -> (i32, i32, i32, i32) {
        // Calculate top margin: space between terminal top and monitor top
        let top_gap = terminal_geo.y - monitor_geo.y;
        let margin_top = top_gap + 1;

        // Calculate bottom margin: space between terminal bottom and monitor bottom
        let terminal_bottom = terminal_geo.y + terminal_geo.height;
        let bottom_gap = monitor_geo.height - terminal_bottom;
        let margin_bottom = bottom_gap + 1;

        // Phase 2: Calculate right margin for right-side positioning
        // margin_right = monitor.width - terminal.x - terminal.width
        // (NO overlay_width subtraction - we want overlay right edge aligned with terminal right edge)
        let terminal_x_relative = terminal_geo.x - monitor_geo.x;
        let margin_right = monitor_geo.width
            - terminal_x_relative
            - terminal_geo.width;
        let margin_right = margin_right.max(0); // Ensure non-negative

        let margin_left = 0; // Phase 3: Will calculate for left-side positioning

//         println!("[GTK] Canvas margins calculated: top={}, bottom={}, left={}, right={}",
//                  margin_top, margin_bottom, margin_left, margin_right);
//         println!("[GTK] Terminal position: x={}, y={}, w={}, h={}",
//                  terminal_geo.x, terminal_geo.y, terminal_geo.width, terminal_geo.height);
//         println!("[GTK] Monitor position: x={}, y={}, w={}, h={}",
//                  monitor_geo.x, monitor_geo.y, monitor_geo.width, monitor_geo.height);

        (margin_top, margin_bottom, margin_left, margin_right)
    }

    fn set_padding_and_reload(width: u32) {
        thread::spawn(move || {
            let term = crate::terminal::wezterm::WeztermProtocol::new();
            let _ = term.set_right_padding(width);
            let mut cmd = std::process::Command::new("wezterm");
            cmd.args(["cli", "reload-config"]);
            let _ = crate::terminal::wezterm::run_cmd_with_timeout(&mut cmd, Duration::from_secs(3));
        });
    }
}

impl GuiPlatform for Gtk4Platform {
    fn create_overlay(&self, url: &str, term_geometry: Option<WindowGeometry>) -> Result<(), String> {
        let window_ref = self.window.clone();
        let webview_ref = self.webview.clone();
        let monitor_geo_ref = self.monitor_geo.clone();
        let url = url.to_string();
        let term_geometry_clone = term_geometry.clone();

        self.app.connect_activate(move |app| {
            // Calculate initial size based on terminal geometry if available
            let (initial_width, initial_height) = if let Some(ref geo) = term_geometry_clone {
                let width = ((geo.width as f32) * 0.20).max(350.0) as i32;
//                 println!("[GTK] Calculating initial size: terminal {}x{} -> overlay {}x{}",
//                          geo.width, geo.height, width, geo.height);
                (width, geo.height)
            } else {
//                 println!("[GTK] Using default size 350x600 (no terminal geometry available)");
                (350, 600)
            };

            let window = ApplicationWindow::builder()
                .application(app)
                .title("WeztCode")
                .default_width(initial_width)
                .default_height(initial_height)
                .build();

            // Make window transparent for rounded corners
            window.set_css_classes(&[&"transparent-window"]);
            let css_provider = gtk4::CssProvider::new();
            css_provider.load_from_data(
                "window.transparent-window { background: transparent; }"
            );
            gtk4::style_context_add_provider_for_display(
                &gdk::Display::default().expect("No display"),
                &css_provider,
                gtk4::STYLE_PROVIDER_PRIORITY_APPLICATION
            );

            window.init_layer_shell();
            window.set_layer(Layer::Overlay);
            window.set_keyboard_mode(KeyboardMode::OnDemand);
            window.set_anchor(Edge::Right, true);
            // Anchor to top and bottom to adapt to margins
            window.set_anchor(Edge::Top, true);
            window.set_anchor(Edge::Bottom, true);

            window.present();

            // Use GTK monitor geometry for margin calculations
            let monitor_geo = Self::detect_monitor_geometry(&window);

            if let (Some(monitor), Some(term_geo)) = (monitor_geo, &term_geometry) {
                // Calculate canvas margins using monitor geometry + terminal geometry + overlay width
                let overlay_width = initial_width as i32;
                let (margin_top, margin_bottom, _margin_left, margin_right) =
                    Self::calculate_canvas_margins(&monitor, term_geo, overlay_width);

//                 println!("[GTK] Using monitor geometry for margins: {}x{} (top={}, bottom={}, right={})",
//                          monitor.width, monitor.height, margin_top, margin_bottom, margin_right);
                window.set_margin(Edge::Top, margin_top);
                window.set_margin(Edge::Bottom, margin_bottom);
                window.set_margin(Edge::Right, margin_right);

                // Sync terminal padding with overlay width (non-blocking)
                Self::set_padding_and_reload(overlay_width as u32);

                // Store monitor geometry for dynamic recalculation in GeometryChanged
                *monitor_geo_ref.borrow_mut() = Some(monitor.clone());
            } else if let Some(geo) = &term_geometry {
                // Fallback: Use terminal geometry only (existing behavior)
//                 println!("[GTK] Fallback to terminal-only margins for x={}, y={}, w={}, h={}",
//                          geo.x, geo.y, geo.width, geo.height);
                window.set_margin(Edge::Top, geo.y);
                window.set_margin(Edge::Bottom, 0);
            } else {
                // Default values when no geometry available
//                 println!("[GTK] Using default margins (no geometry available)");
                window.set_margin(Edge::Top, 1);
                window.set_margin(Edge::Bottom, 1);
            }

            // Exclusive zone -1 = margins measured from monitor edges, not from available area
            window.set_exclusive_zone(-1);

//             println!("GTK: Creating WebView...");
            let webview = WebView::new();

            // Make WebView background transparent to allow GTK window transparency
            webview.set_background_color(&gdk::RGBA::new(0.0, 0.0, 0.0, 0.0));

            // Enable focus on click so WebView receives keyboard/mouse input
            webview.set_can_focus(true);
            webview.set_focus_on_click(true);

            // Gesture controller to capture clicks and prevent pass-through to terminal
            let webview_for_gesture = webview.clone();
            let gesture = gtk4::GestureClick::new();
            gesture.connect_pressed(move |_, _, _, _| {
                webview_for_gesture.grab_focus();
            });
            webview.add_controller(gesture);

            // Force appearance: none on all elements via WebKit UserStyleSheet
            // to override WebKitGTK native styling for selects, inputs, etc.
            if let Some(manager) = webview.user_content_manager() {
                let css = "\
                    * {\
                        -webkit-appearance: none !important;\
                        appearance: none !important;\
                    }\
                ";
                let sheet = webkit6::UserStyleSheet::new(
                    css,
                    webkit6::UserContentInjectedFrames::AllFrames,
                    webkit6::UserStyleLevel::User,
                    &[], // allow_list — all URLs
                    &[], // block_list — none excluded
                );
                manager.add_style_sheet(&sheet);
            }

//             println!("GTK: Loading URL: {}", &url);
            webview.load_uri(&url);

//             println!("GTK: Adding WebView to window...");
            window.set_child(Some(&webview));

            *window_ref.borrow_mut() = Some(window.clone());
            *webview_ref.borrow_mut() = Some(webview);
        });

        Ok(())
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
        let monitor_geo_weak = self.monitor_geo.clone();
        let pending_width = self.pending_width.clone();
        let debounce_until = self.debounce_until.clone();

        glib::idle_add_local(move || {
            // Drain all pending events in one cycle for better reactivity
            loop {
                match receiver.try_recv() {
                    Ok(WmEvent::WindowFocused { app_id }) => {
                        if let Ok(window_ref) = window_weak.try_borrow() {
                            if let Some(ref window) = *window_ref {
                                window.set_visible(true);
                                window.present();
                            }
                        }
                    }
                    Ok(WmEvent::WindowUnfocused { app_id }) => {
                        if let Ok(window_ref) = window_weak.try_borrow() {
                            if let Some(ref window) = *window_ref {
                                window.set_visible(false);
                            }
                        }
                    }
                    Ok(WmEvent::GeometryChanged { app_id, geometry }) => {
                        if let Ok(window_ref) = window_weak.try_borrow() {
                            if let Some(ref window) = *window_ref {
                                let overlay_width = ((geometry.width as f32) * 0.20).max(350.0) as i32;

                                window.set_default_size(overlay_width, geometry.height);

                                // Re-detect monitor geometry (handles hotplug)
                                let fresh_monitor = Self::detect_monitor_geometry(window);
                                if fresh_monitor.is_some() {
                                    *monitor_geo_weak.borrow_mut() = fresh_monitor.clone();
                                }
                                let monitor = fresh_monitor
                                    .or_else(|| monitor_geo_weak.borrow().clone());

                                if let Some(monitor) = monitor {
                                    let (margin_top, margin_bottom, _margin_left, margin_right) =
                                        Self::calculate_canvas_margins(&monitor, &geometry, overlay_width);

                                    window.set_margin(Edge::Top, margin_top);
                                    window.set_margin(Edge::Bottom, margin_bottom);
                                    window.set_margin(Edge::Right, margin_right);
                                }

                                // Schedule debounced padding + reload
                                *pending_width.borrow_mut() = Some(overlay_width as u32);
                                *debounce_until.borrow_mut() = Some(Instant::now() + Duration::from_millis(50));
                            }
                        }
                    }
                    Ok(WmEvent::FullscreenChanged { app_id, geometry, is_fullscreen }) => {
                        if let Ok(window_ref) = window_weak.try_borrow() {
                            if let Some(ref window) = *window_ref {
                                let overlay_width = ((geometry.width as f32) * 0.20).max(350.0) as i32;

                                window.set_default_size(overlay_width, geometry.height);

                                // Re-detect monitor geometry (handles hotplug)
                                let fresh_monitor = Self::detect_monitor_geometry(window);
                                if fresh_monitor.is_some() {
                                    *monitor_geo_weak.borrow_mut() = fresh_monitor.clone();
                                }
                                let monitor = fresh_monitor
                                    .or_else(|| monitor_geo_weak.borrow().clone());

                                if let Some(monitor) = monitor {
                                    let (margin_top, margin_bottom, _margin_left, margin_right) =
                                        Self::calculate_canvas_margins(&monitor, &geometry, overlay_width);

                                    window.set_margin(Edge::Top, margin_top);
                                    window.set_margin(Edge::Bottom, margin_bottom);
                                    window.set_margin(Edge::Right, margin_right);
                                }

                                // Schedule debounced padding + reload
                                *pending_width.borrow_mut() = Some(overlay_width as u32);
                                *debounce_until.borrow_mut() = Some(Instant::now() + Duration::from_millis(50));
                            }
                        }
                    }
                    Ok(_) => {} // WindowCreated, WindowDestroyed - ignore
                    Err(std::sync::mpsc::TryRecvError::Empty) => break,
                    Err(std::sync::mpsc::TryRecvError::Disconnected) => return glib::ControlFlow::Break,
                }
            }

            // Apply debounced padding + reload if deadline passed
            let should_apply = debounce_until.borrow()
                .map(|d| Instant::now() >= d)
                .unwrap_or(false);
            if should_apply {
                let width = pending_width.borrow_mut().take();
                if let Some(w) = width {
                    Self::set_padding_and_reload(w);
                }
                *debounce_until.borrow_mut() = None;
            }

            glib::ControlFlow::Continue
        });
    }
}
