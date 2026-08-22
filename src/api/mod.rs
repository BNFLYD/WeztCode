pub mod analyze;
pub mod chat;
pub mod editor;
pub mod fs;
pub mod git;
pub mod keys;
pub mod models;
pub mod projects;
pub mod sub_agents;
pub mod terminal;

use std::path::PathBuf;

use axum::{
    http::StatusCode,
    response::Json,
    routing::{get, post},
    Router,
};

pub fn router() -> Router {
    Router::new()
        .route("/chat/send", post(chat::handle_chat_send))
        .route("/chat/new-session", post(chat::handle_chat_new_session))
        .route("/chat/switch-model", post(chat::handle_chat_switch_model))
        .route("/chat/backend", get(chat::handle_chat_backend_status))
        .route("/chat/switch-backend", post(chat::handle_chat_switch_backend))
        .route("/keys/set", post(keys::handle_keys_set))
        .route("/keys/delete", get(keys::handle_keys_delete))
        .route("/keys/list", get(keys::handle_keys_list))
        .route("/terminal/list", get(terminal::handle_terminal_list))
        .route("/terminal/spawn", post(terminal::handle_terminal_spawn))
        .route("/terminal/kill", post(terminal::handle_terminal_kill))
        .route("/terminal/activate", post(terminal::handle_terminal_activate))
        .route("/terminal/active-pane", get(terminal::handle_active_pane))
        .route("/terminal/ensure-main", post(terminal::handle_terminal_ensure_main))
        .route("/terminal/metadata", get(terminal::handle_terminal_metadata_get))
        .route("/terminal/metadata/set", post(terminal::handle_terminal_metadata_set))
        .route("/terminal/metadata/delete", get(terminal::handle_terminal_metadata_delete))
        .route("/terminal/edit-defaults", get(terminal::handle_terminal_edit_defaults))
        .route("/terminal/default-terms", get(terminal::handle_terminal_default_terms))
        .route("/projects/list", get(projects::handle_projects_list))
        .route("/projects/add", post(projects::handle_projects_add))
        .route("/projects/delete", get(projects::handle_projects_delete))
        .route("/projects/switch", get(projects::handle_projects_switch))
        .route("/models/list", get(models::handle_models_list))
        .route("/models/edit-defaults", get(models::handle_models_edit_defaults))
        .route("/sub-agents/list", get(sub_agents::handle_list))
        .route("/sub-agents/switch", post(sub_agents::handle_switch))
        .route("/sub-agents/install", post(sub_agents::handle_install))
        .route("/sub-agents/edit", get(sub_agents::handle_edit))
        .route("/sub-agents/dirs", get(sub_agents::handle_dirs))
        .route("/sub-agents/builtins", get(sub_agents::handle_builtins))
        .route("/analyze/tools", get(analyze::handle_tools))
        .route("/analyze/run", post(analyze::handle_run))
        .route("/git/log", get(git::handle_git_log))
        .route("/editor/open", get(editor::handle_editor_open))
        .route("/fs/ls", get(fs::handle_ls))
        .route("/fs/read", get(fs::handle_read))
        .route("/fs/create", get(fs::handle_create))
        .route("/fs/delete", get(fs::handle_delete))
        .route("/fs/rename", get(fs::handle_rename))
        .route("/fs/move", get(fs::handle_move))
        .route("/fs/image", get(fs::handle_image))
}

pub type ApiResponse = (StatusCode, Json<serde_json::Value>);

pub fn ok_json(data: serde_json::Value) -> ApiResponse {
    (StatusCode::OK, Json(data))
}

pub fn err_json(msg: &str) -> ApiResponse {
    (StatusCode::OK, Json(serde_json::json!({"ok": false, "error": msg})))
}

fn get_current_root() -> PathBuf {
    crate::config::current_root::get()
}
