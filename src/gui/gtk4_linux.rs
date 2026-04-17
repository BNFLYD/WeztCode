use crate::gui::GuiPlatform;
use crate::terminal::{TerminalProtocol, WeztermProtocol};
use gtk4::prelude::*;
use gtk4::{Application, ApplicationWindow};
use gtk4_layer_shell::{Edge, Layer, LayerShell};
use webkit6::prelude::*;
use webkit6::{UserContentManager, WebView};
use std::cell::RefCell;
use std::rc::Rc;
use std::process::Child;

#[derive(serde::Deserialize)]
struct JsCommand {
    id: u32,
    command: String,
    #[serde(default)]
    args: serde_json::Value,
}

pub struct Gtk4Platform {
    app: Application,
    window: Rc<RefCell<Option<ApplicationWindow>>>,
    webview: Rc<RefCell<Option<WebView>>>,
    terminal: Rc<RefCell<Option<Child>>>,
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
            terminal: Rc::new(RefCell::new(None)),
        }
    }

    fn handle_command(&self, cmd: JsCommand) -> Result<String, String> {
        let term = WeztermProtocol::new();

        match cmd.command.as_str() {
            "spawn_term" => {
                match term.spawn("weztcode-terminal") {
                    Ok(child) => {
                        let pid = child.id().to_string();
                        *self.terminal.borrow_mut() = Some(child);
                        Ok(format!("Terminal spawned: PID {}", pid))
                    }
                    Err(e) => Err(e),
                }
            }
            "list_panes" => {
                term.list_panes()
            }
            "send_text" => {
                let text = cmd.args.get("text")
                    .and_then(|v| v.as_str())
                    .unwrap_or("");
                term.send_text(text, None)
                    .map(|_| "Text sent".to_string())
            }
            "set_size" => {
                let width = cmd.args.get("width")
                    .and_then(|v| v.as_i64())
                    .unwrap_or(350) as i32;
                let height = cmd.args.get("height")
                    .and_then(|v| v.as_i64())
                    .unwrap_or(600) as i32;

                if let Some(ref window) = *self.window.borrow() {
                    window.set_default_size(width, height);
                }
                Ok(format!("Size set to {}x{}", width, height))
            }
            "set_position" => {
                let x = cmd.args.get("x")
                    .and_then(|v| v.as_i64())
                    .unwrap_or(0) as i32;
                let y = cmd.args.get("y")
                    .and_then(|v| v.as_i64())
                    .unwrap_or(0) as i32;

                self.set_geometry(x, y, 350, 600);
                Ok(format!("Position set to {}, {}", x, y))
            }
            _ => Err(format!("Unknown command: {}", cmd.command)),
        }
    }

    fn send_response(&self, webview: &WebView, id: u32, result: Result<String, String>) {
        let (result_str, error_str) = match result {
            Ok(r) => (r, "".to_string()),
            Err(e) => ("".to_string(), e),
        };

        let js = format!(
            "window.weztcodeResponse({}, {}, {})",
            id,
            serde_json::to_string(&result_str).unwrap_or_default(),
            serde_json::to_string(&error_str).unwrap_or_default()
        );

        webview.evaluate_javascript(&js, None::<&str>, None::<&str>, None::<&gtk4::gio::Cancellable>, |_result| {});
    }
}

impl GuiPlatform for Gtk4Platform {
    fn create_overlay(&self, url: &str) -> Result<(), String> {
        let window_ref = self.window.clone();
        let webview_ref = self.webview.clone();
        let terminal_ref = self.terminal.clone();
        let url = url.to_string();
        let self_ref = Rc::new(RefCell::new(self));

        self.app.connect_activate(move |app| {
            let window = ApplicationWindow::builder()
                .application(app)
                .title("WeztCode")
                .default_width(350)
                .default_height(600)
                .build();

            window.init_layer_shell();
            window.set_layer(Layer::Overlay);
            window.set_anchor(Edge::Right, true);
            window.set_anchor(Edge::Top, true);
            window.set_anchor(Edge::Bottom, true);
            window.set_exclusive_zone(-1);

            // Setup message handler for JS bridge
            let content_manager = UserContentManager::new();
            let webview = WebView::builder()
                .user_content_manager(&content_manager)
                .build();

            webview.load_uri(&url);
            window.set_child(Some(&webview));

            // Register script message handler
            content_manager.register_script_message_handler("weztcode", None);

            // Connect to script message received signal
            let webview_msg_ref = webview.clone();
            content_manager.connect_script_message_received(Some("weztcode"), move |_, value| {
                // Convert JavaScript value to string
                let msg = value.to_string();
                let webview = webview_msg_ref.clone();

                // Parse command and handle
                if let Ok(cmd) = serde_json::from_str::<JsCommand>(&msg) {
                    let response = match cmd.command.as_str() {
                        "spawn_term" => Ok("Term spawned".to_string()),
                        "list_panes" => Ok("Pane list placeholder".to_string()),
                        "send_text" => Ok("Text sent".to_string()),
                        "set_size" => {
                            let w = cmd.args.get("width").and_then(|v| v.as_i64()).unwrap_or(350) as i32;
                            let h = cmd.args.get("height").and_then(|v| v.as_i64()).unwrap_or(600) as i32;
                            Ok(format!("Size: {}x{}", w, h))
                        }
                        _ => Err("Unknown command".to_string()),
                    };

                    // Send response back to JS with proper error handling
                    let (res, err) = match response {
                        Ok(r) => (r, "".to_string()),
                        Err(e) => ("".to_string(), e),
                    };

                    let js = format!("window.weztcodeResponse({}, {}, {})",
                        cmd.id,
                        serde_json::to_string(&res).unwrap_or_default(),
                        serde_json::to_string(&err).unwrap_or_default()
                    );

                    webview.evaluate_javascript(&js, None::<&str>, None::<&str>, None::<&gtk4::gio::Cancellable>, |_result| {});
                }
            });

            // Inject bridge script after page loads
            webview.connect_load_changed(move |webview, event| {
                if event == webkit6::LoadEvent::Finished {
                    let bridge_script = r#"
                        if (!window.weztcode) {
                            window.weztcode = {
                                pending: new Map(),
                                requestId: 0,
                                call: function(command, args = {}) {
                                    return new Promise((resolve, reject) => {
                                        const id = ++this.requestId;
                                        this.pending.set(id, { resolve, reject });
                                        const msg = JSON.stringify({ id, command, args });
                                        if (window.webkit && window.webkit.messageHandlers.weztcode) {
                                            window.webkit.messageHandlers.weztcode.postMessage(msg);
                                        }
                                    });
                                },
                                spawnTerm: function() { return this.call('spawn_term'); },
                                listPanes: function() { return this.call('list_panes'); },
                                sendText: function(text) { return this.call('send_text', { text }); },
                                setSize: function(w, h) { return this.call('set_size', { width: w, height: h }); },
                                setPosition: function(x, y) { return this.call('set_position', { x, y }); },
                                response: function(id, result, error) {
                                    const p = this.pending.get(id);
                                    if (p) {
                                        if (error) p.reject(error);
                                        else p.resolve(result);
                                        this.pending.delete(id);
                                    }
                                }
                            };
                            window.weztcodeResponse = window.weztcode.response.bind(window.weztcode);
                        }
                    "#;
                    webview.evaluate_javascript(bridge_script, None::<&str>, None::<&str>, None::<&gtk4::gio::Cancellable>, |_result| {});
                }
            });

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
