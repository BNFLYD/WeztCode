use crate::gui::GuiPlatform;
use crate::gui::protocol::wayland::wm::WindowGeometry;
use gtk4::prelude::*;
use gtk4::{Application, ApplicationWindow};
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
}

impl GuiPlatform for Gtk4Platform {
    fn create_overlay(&self, url: &str, term_geometry: Option<WindowGeometry>) -> Result<(), String> {
        let window_ref = self.window.clone();
        let webview_ref = self.webview.clone();
        let url = url.to_string();

        self.app.connect_activate(move |app| {
            let window = ApplicationWindow::builder()
                .application(app)
                .title("WeztCode")
                .default_width(350)
                .default_height(600)
                .build();

            window.init_layer_shell();
            window.set_layer(Layer::Top);
            window.set_anchor(Edge::Right, true);
            // Anchor to top and bottom to adapt to margins
            window.set_anchor(Edge::Top, true);
            window.set_anchor(Edge::Bottom, true);

            // Calculate margins based on terminal geometry
            if let Some(geo) = &term_geometry {
                println!("GTK: Using terminal geometry: x={}, y={}, w={}, h={}",
                         geo.x, geo.y, geo.width, geo.height);

                // Top margin = terminal Y position (for alignment)
                window.set_margin(Edge::Top, geo.y);
                // Bottom margin = remaining screen space minus terminal height
                let screen_height = 1080; // TODO: detectar dinámicamente
                let bottom_margin = screen_height - geo.y - geo.height;
                window.set_margin(Edge::Bottom, bottom_margin.max(0));
            } else {
                // Default values
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

            window.present();
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
                            let screen_height = 1080; // TODO: detect dynamically

                            let screen_width = 1920; // TODO: detect dynamically
                            let right_margin = (screen_width - geometry.x - overlay_width).max(0);

                            println!("[GTK] Updating position and size: x={}, y={}, w={}, h={}",
                                     geometry.x, geometry.y, overlay_width, overlay_height);
                            println!("[GTK] Calculated margins: right={}, top={}", right_margin, geometry.y);

                            // Resize the window
                            window.set_default_size(overlay_width, overlay_height);

                            // Reposition using layer shell margins
                            window.set_margin(Edge::Right, right_margin);
                            window.set_margin(Edge::Top, geometry.y);

                            println!("[GTK] Repositioned to x={}, y={} with size {}x{}",
                                     geometry.x, geometry.y, overlay_width, overlay_height);
                        }
                    }
                }
                Ok(WmEvent::FullscreenChanged { app_id, geometry, is_fullscreen }) => {
                    println!("[GTK] FullscreenChanged for {}: fullscreen={}, geometry={:?}", app_id, is_fullscreen, geometry);
                    if let Ok(window_ref) = window_weak.try_borrow() {
                        if let Some(ref window) = *window_ref {
                            let overlay_width = ((geometry.width as f32) * 0.20).max(350.0) as i32;
                            let overlay_height = geometry.height;
                            let screen_height = 1080; // TODO: detect dynamically

                            if is_fullscreen {
                                // In fullscreen mode: change to OVERLAY layer to stay on top
                                window.set_layer(Layer::Overlay);
                                println!("[GTK] Terminal fullscreen - layer changed to OVERLAY");
                            } else {
                                // Exited fullscreen: restore TOP layer
                                window.set_layer(Layer::Top);
                                println!("[GTK] Terminal exited fullscreen - layer restored to TOP");
                            }

                            let screen_width = 1920; // TODO: detect dynamically
                            let right_margin = (screen_width - geometry.x - overlay_width).max(0);

                            // Resize and reposition
                            window.set_default_size(overlay_width, overlay_height);
                            window.set_margin(Edge::Right, right_margin);
                            window.set_margin(Edge::Top, geometry.y);

                            println!("[GTK] Fullscreen resize: {}x{} at x={}, y={}",
                                     overlay_width, overlay_height, geometry.x, geometry.y);
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
