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

/// Ventana viewer de puertos (única): window + WebView para reemplazar contenido.
struct ViewerWindow {
    window: ApplicationWindow,
    webview: WebView,
}

pub struct Gtk4Platform {
    app: Application,
    window: Rc<RefCell<Option<ApplicationWindow>>>,
    webview: Rc<RefCell<Option<WebView>>>,
    monitor_geo: Rc<RefCell<Option<WindowGeometry>>>,
    /// Ventana viewer de puertos (sección 表示): hoisted al platform para que
    /// handle_wm_events la alcance — es hermana del menú y re-ancla en CADA
    /// GeometryChanged, no solo con el snapshot de cuando se abrió.
    viewer_state: Rc<RefCell<Option<ViewerWindow>>>,
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
            viewer_state: Rc::new(RefCell::new(None)),
        }
    }

    /// Gdk::Monitor donde vive la ventana (None si aún no tiene surface).
    /// La ventana viewer se pinea al monitor del menú: los márgenes de
    /// layer-shell son relativos al monitor donde cae cada ventana.
    fn monitor_at_window(window: &ApplicationWindow) -> Option<gdk::Monitor> {
        let display = gdk::Display::default()?;
        let surface = window.surface()?;
        display.monitor_at_surface(&surface)
    }

    /// Detect monitor geometry using GTK4/GDK and return WindowGeometry
    fn detect_monitor_geometry(window: &ApplicationWindow) -> Option<WindowGeometry> {
        let monitor = Self::monitor_at_window(window)?;
        let geo = monitor.geometry();
        Some(WindowGeometry::new(geo.x(), geo.y(), geo.width(), geo.height()))
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

        // Phase 3 (viewer de puertos): margen izquierdo de la terminal. La
        // ventana de visualización ancla Left con este margen y llena el
        // rectángulo libre al lado del menú (sus 4 anclajes + márgenes
        // heredados del canvas del menú).
        let margin_left = (terminal_geo.x - monitor_geo.x).max(0);

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
        let viewer_state_ref = self.viewer_state.clone();
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
                let (margin_top, margin_bottom, margin_left, margin_right) =
                    Self::calculate_canvas_margins(&monitor, term_geo, overlay_width);

//                 println!("[GTK] Using monitor geometry for margins: {}x{} (top={}, bottom={}, right={})",
//                          monitor.width, monitor.height, margin_top, margin_bottom, margin_right);
                window.set_margin(Edge::Top, margin_top);
                window.set_margin(Edge::Bottom, margin_bottom);
                window.set_margin(Edge::Right, margin_right);

                // Layout del menú → la ventana viewer se ancla al lado (viewer.rs)
                crate::gui::viewer::set_menu_layout(crate::gui::viewer::MenuLayout {
                    top: margin_top,
                    bottom: margin_bottom,
                    right: margin_right,
                    width: overlay_width,
                    left: margin_left,
                });

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
                crate::gui::viewer::set_menu_layout(crate::gui::viewer::MenuLayout {
                    top: geo.y,
                    bottom: 0,
                    right: 0,
                    width: initial_width,
                    left: geo.x.max(0),
                });
            } else {
                // Default values when no geometry available
//                 println!("[GTK] Using default margins (no geometry available)");
                window.set_margin(Edge::Top, 1);
                window.set_margin(Edge::Bottom, 1);
                crate::gui::viewer::set_menu_layout(crate::gui::viewer::MenuLayout {
                    top: 1,
                    bottom: 1,
                    right: 0,
                    width: initial_width,
                    left: 0,
                });
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

            // ---- Viewer de puertos (sección 表示) ----
            // Canal API → main loop: la ventana se crea/reemplaza/destruye acá,
            // en el thread de GTK. Ventana única: otro puerto reemplaza contenido.
            // El recv().await es un Future puro — glib lo maneja con spawn_local
            // (event-driven, sin polling). El estado vive en el PLATFORM
            // (viewer_state), así handle_wm_events puede re-anclarla como
            // hermana del menú en cada GeometryChanged.
            let viewer_app = app.clone();
            let viewer_state = viewer_state_ref.clone();
            let menu_window_ref = window_ref.clone();
            let mut viewer_rx = crate::gui::viewer::setup_channel();
            gtk4::glib::MainContext::default().spawn_local(async move {
                while let Some(cmd) = viewer_rx.recv().await {
                    match cmd {
                        crate::gui::viewer::ViewerCmd::Open { port } => {
                            let layout = crate::gui::viewer::menu_layout();
                            // Ventana ya abierta: mismo WebView, contenido nuevo
                            let existing = viewer_state
                                .borrow()
                                .as_ref()
                                .map(|st| (st.window.clone(), st.webview.clone()));
                            if let Some((win, webview)) = existing {
                                webview.load_uri(&format!("http://127.0.0.1:{}/", port));
                                // No resucitar la ventana si la app está oculta
                                // (otro workspace / sin foco): el menú manda.
                                if win.is_visible() {
                                    win.present();
                                }
                            } else {
                                // Mismo monitor que el menú: los márgenes de
                                // layer-shell son relativos al monitor donde
                                // cae la ventana.
                                let menu_monitor = menu_window_ref
                                    .borrow()
                                    .as_ref()
                                    .and_then(Self::monitor_at_window);
                                // Nace con la visibilidad del menú: si la app está
                                // en otro workspace, el viewer no debe aparecer.
                                let menu_visible = menu_window_ref
                                    .borrow()
                                    .as_ref()
                                    .map(|w| w.is_visible())
                                    .unwrap_or(true);
                                let on_close = {
                                    let st = viewer_state.clone();
                                    Rc::new(move || {
                                        *st.borrow_mut() = None;
                                        crate::gui::viewer::set_status(None);
                                    }) as Rc<dyn Fn()>
                                };
                                let win = Self::create_viewer_window(
                                    &viewer_app,
                                    port,
                                    layout,
                                    menu_monitor.as_ref(),
                                    menu_visible,
                                    on_close,
                                );
                                *viewer_state.borrow_mut() = Some(win);
                            }
                            crate::gui::viewer::set_status(Some(port));
                        }
                        crate::gui::viewer::ViewerCmd::Close => {
                            // take() FUERA del borrow: window.close() emite
                            // close-request de forma sincrónica → el cleanup
                            // re-entra y hace borrow_mut() sobre un RefCell ya
                            // prestado → panic en el main loop de GTK.
                            let taken = viewer_state.borrow_mut().take();
                            if let Some(st) = taken {
                                st.window.close();
                            }
                            crate::gui::viewer::set_status(None);
                        }
                    }
                }
            });
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

    /// Ventana viewer de puertos (sección 表示): hermana del menú. Reutiliza
    /// los cálculos de canvas del menú (MenuLayout) para llenar el espacio
    /// libre de la terminal al lado de la sidebar: los 4 anclajes + márgenes
    /// hacen que el COMPOSITOR controle ambas dimensiones — right aterriza en
    /// el borde IZQUIERDO del menú (right del menú + su ancho), left en el
    /// borde izquierdo de la terminal. WebKit directo al puerto elegido y
    /// botón de cierre en la esquina externa.
    fn create_viewer_window(
        app: &gtk4::Application,
        port: u16,
        layout: crate::gui::viewer::MenuLayout,
        monitor: Option<&gdk::Monitor>,
        visible: bool,
        on_close: Rc<dyn Fn()>,
    ) -> ViewerWindow {
        let window = gtk4::ApplicationWindow::builder()
            .application(app)
            .title("WeztCode Viewer")
            .default_width(600)
            .default_height(400)
            .build();
        window.set_css_classes(&[&"transparent-window"]);

        window.init_layer_shell();
        window.set_layer(Layer::Overlay);
        window.set_keyboard_mode(KeyboardMode::OnDemand);
        // Mismo monitor que el menú: los márgenes solo significan lo mismo
        // si ambas hermanas caen en el mismo monitor.
        window.set_monitor(monitor);
        // Hermana del menú: llena el rectángulo libre al lado de la sidebar.
        window.set_anchor(Edge::Right, true);
        window.set_anchor(Edge::Top, true);
        window.set_anchor(Edge::Bottom, true);
        window.set_anchor(Edge::Left, true);
        window.set_margin(Edge::Top, layout.top);
        window.set_margin(Edge::Bottom, layout.bottom);
        window.set_margin(Edge::Left, layout.left);
        window.set_margin(Edge::Right, layout.right + layout.width);
        window.set_exclusive_zone(-1);

        // Botón de cierre flotante (esquina superior externa)
        let css = gtk4::CssProvider::new();
        css.load_from_data(
            "button.viewer-close {\
                border-radius: 9999px;\
                background: rgba(13, 13, 13, 0.85);\
                color: #ffffff;\
                min-width: 30px; min-height: 30px;\
                padding: 0; font-size: 14px;\
                border: 1px solid rgba(255, 255, 255, 0.15);\
            }"
        );
        gtk4::style_context_add_provider_for_display(
            &gdk::Display::default().expect("No display"),
            &css,
            gtk4::STYLE_PROVIDER_PRIORITY_APPLICATION,
        );

        let webview = WebView::new();
        // Fondo sólido oscuro: es un browser, no un overlay de vidrio
        webview.set_background_color(&gdk::RGBA::new(0.05, 0.05, 0.05, 1.0));
        webview.set_can_focus(true);
        webview.set_focus_on_click(true);
        let wv = webview.clone();
        let gesture = gtk4::GestureClick::new();
        gesture.connect_pressed(move |_, _, _, _| {
            wv.grab_focus();
        });
        webview.add_controller(gesture);
        webview.load_uri(&format!("http://127.0.0.1:{}/", port));

        let overlay = gtk4::Overlay::new();
        overlay.set_child(Some(&webview));

        let close_btn = gtk4::Button::with_label("✕");
        close_btn.set_halign(gtk4::Align::Start);
        close_btn.set_valign(gtk4::Align::Start);
        close_btn.set_margin_top(6);
        close_btn.set_margin_start(6);
        close_btn.add_css_class("viewer-close");
        // ✕ = MISMA vía que cualquier otro cierre: window.close() dispara
        // close-request → on_close() (limpia estado). Una sola fuente de
        // limpieza; antes el botón solo limpiaba el estado y dejaba la
        // ventana viva en pantalla.
        let btn_win = window.clone();
        close_btn.connect_clicked(move |_| {
            btn_win.close();
        });
        overlay.add_overlay(&close_btn);

        // Cualquier otra vía de cierre (WM, etc.) también limpia el estado
        let win_onclose = on_close;
        window.connect_close_request(move |_| {
            win_onclose();
            gtk4::glib::Propagation::Proceed
        });

        window.set_child(Some(&overlay));
        window.set_visible(visible);
        if visible {
            window.present();
        }

        ViewerWindow { window, webview }
    }

    /// Handle WM events and update window visibility accordingly
    /// Handle WM events: 100% event-driven. El recv().await es un Future que
    /// glib maneja con spawn_local — el main loop DUEERME hasta que llega el
    /// evento (cero idle-polling). El debounce del padding usa un timer one-shot
    /// agendado como REACCIÓN al evento (trailing): ráfagas de eventos colapsan
    /// en una sola actualización.
    pub fn handle_wm_events(&self, receiver: tokio::sync::mpsc::UnboundedReceiver<crate::gui::protocol::wayland::wm::WmEvent>) {
        use gtk4::glib;
        use crate::gui::protocol::wayland::wm::WmEvent;

        let window_weak = self.window.clone();
        let monitor_geo_weak = self.monitor_geo.clone();
        let viewer_state = self.viewer_state.clone();

        // Debounce trailing del padding reload: width pendiente + timer one-shot
        let pending_width: Rc<RefCell<Option<u32>>> = Rc::new(RefCell::new(None));
        let debounce_src: Rc<RefCell<Option<glib::SourceId>>> = Rc::new(RefCell::new(None));

        glib::MainContext::default().spawn_local(async move {
            let mut receiver = receiver;
            while let Some(event) = receiver.recv().await {
                match event {
                    WmEvent::WindowFocused { app_id: _ } => {
                        if let Ok(window_ref) = window_weak.try_borrow() {
                            if let Some(ref window) = *window_ref {
                                window.set_visible(true);
                                window.present();
                            }
                        }
                        // Hermana: el viewer vuelve con el menú (mismo workspace).
                        Self::set_viewer_visible(&viewer_state, true);
                    }
                    WmEvent::WindowUnfocused { app_id: _ } => {
                        if let Ok(window_ref) = window_weak.try_borrow() {
                            if let Some(ref window) = *window_ref {
                                window.set_visible(false);
                            }
                        }
                        // Hermana: el viewer se oculta con el menú — si no, queda
                        // flotando sobre todo cuando la app está en otro workspace.
                        Self::set_viewer_visible(&viewer_state, false);
                    }
                    WmEvent::GeometryChanged { app_id: _, geometry } => {
                        if let Ok(window_ref) = window_weak.try_borrow() {
                            if let Some(ref window) = *window_ref {
                                Self::apply_geometry(window, &monitor_geo_weak, geometry, &pending_width, &debounce_src);
                                // Hermana: el viewer re-ancla con los números frescos
                                Self::apply_viewer_geometry(&viewer_state);
                            }
                        }
                    }
                    WmEvent::FullscreenChanged { app_id: _, geometry, is_fullscreen: _ } => {
                        if let Ok(window_ref) = window_weak.try_borrow() {
                            if let Some(ref window) = *window_ref {
                                Self::apply_geometry(window, &monitor_geo_weak, geometry, &pending_width, &debounce_src);
                                // Hermana: el viewer re-ancla con los números frescos
                                Self::apply_viewer_geometry(&viewer_state);
                            }
                        }
                    }
                    WmEvent::WindowDestroyed { app_id: _ } => {
                        // Kill switch: la ventana de wezterm ES el ciclo de vida de
                        // la app — si el usuario la cierra, WeztCode se apaga
                        // completo (el pi/lc hijo muere solo por EOF de stdin).
                        eprintln!("[weztcode] WezTerm cerrado — apagando WeztCode");
                        std::process::exit(0);
                    }
                    _ => {} // WindowCreated - ignore
                }
            }
        });
    }

    /// Aplica la geometría al overlay + margen del menú (viewer) + agenda el
    /// debounce trailing del padding (50ms one-shot).
    fn apply_geometry(
        window: &ApplicationWindow,
        monitor_geo_weak: &Rc<RefCell<Option<WindowGeometry>>>,
        geometry: WindowGeometry,
        pending_width: &Rc<RefCell<Option<u32>>>,
        debounce_src: &Rc<RefCell<Option<gtk4::glib::SourceId>>>,
    ) {
        use gtk4::glib;

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
            let (margin_top, margin_bottom, margin_left, margin_right) =
                Self::calculate_canvas_margins(&monitor, &geometry, overlay_width);

            window.set_margin(Edge::Top, margin_top);
            window.set_margin(Edge::Bottom, margin_bottom);
            window.set_margin(Edge::Right, margin_right);

            // Mantener el layout del menú sincronizado para el viewer
            crate::gui::viewer::set_menu_layout(crate::gui::viewer::MenuLayout {
                top: margin_top,
                bottom: margin_bottom,
                right: margin_right,
                width: overlay_width,
                left: margin_left,
            });
        }

        // Trailing debounce: cancelar el timer previo y re-agendar +50ms.
        // El timer es one-shot y se agenda como reacción al evento — no hay
        // ningún reloj vigilado en cada idle del loop.
        if let Some(sid) = debounce_src.borrow_mut().take() {
            sid.remove();
        }
        *pending_width.borrow_mut() = Some(overlay_width as u32);
        let pw = pending_width.clone();
        let src = debounce_src.clone();
        let sid = glib::timeout_add_local(std::time::Duration::from_millis(50), move || {
            *src.borrow_mut() = None;
            if let Some(w) = pw.borrow_mut().take() {
                Self::set_padding_and_reload(w);
            }
            glib::ControlFlow::Break
        });
        *debounce_src.borrow_mut() = Some(sid);
    }

    /// El viewer es hermana del menú: comparte su visibilidad. Cuando la app
    /// sale del workspace activo (o pierde el foco), sway avisa con
    /// WindowUnfocused y el menú se oculta — el viewer se oculta con él y
    /// reaparece con él en WindowFocused.
    fn set_viewer_visible(state: &Rc<RefCell<Option<ViewerWindow>>>, visible: bool) {
        if let Ok(guard) = state.try_borrow() {
            if let Some(ref vw) = *guard {
                vw.window.set_visible(visible);
                if visible {
                    vw.window.present();
                }
            }
        }
    }

    /// Re-ancla la ventana viewer con el layout fresco del menú (su hermana):
    /// mismos top/bottom, left = borde izquierdo de la terminal, right =
    /// borde izquierdo del menú. Se llama en cada GeometryChanged — el viewer
    /// no es el snapshot de cuando se abrió, sigue viva como el menú.
    fn apply_viewer_geometry(state: &Rc<RefCell<Option<ViewerWindow>>>) {
        if let Ok(guard) = state.try_borrow() {
            if let Some(ref vw) = *guard {
                let layout = crate::gui::viewer::menu_layout();
                vw.window.set_margin(Edge::Top, layout.top);
                vw.window.set_margin(Edge::Bottom, layout.bottom);
                vw.window.set_margin(Edge::Left, layout.left);
                vw.window.set_margin(Edge::Right, layout.right + layout.width);
            }
        }
    }


}
