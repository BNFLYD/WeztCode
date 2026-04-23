use crate::gui::GuiPlatform;
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
    fn create_overlay(&self, url: &str) -> Result<(), String> {
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
            // No anclar a top/bottom para no estirarse
            window.set_anchor(Edge::Top, true);
            window.set_anchor(Edge::Bottom, true);
            // Margen superior para respetar la waybar (~30px)
            window.set_margin(Edge::Top, 1);
            window.set_margin(Edge::Bottom, 1);
            // Exclusive zone 0 = se comporta bien con otras ventanas
            window.set_exclusive_zone(0);

            println!("GTK: Creando WebView...");
            let webview = WebView::new();

            println!("GTK: Cargando URL: {}", &url);
            webview.load_uri(&url);

            println!("GTK: Añadiendo WebView a ventana...");
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
}
