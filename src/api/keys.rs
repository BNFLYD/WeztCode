use std::collections::HashMap;

use axum::extract::{Json, Query};
use axum::response::IntoResponse;

use crate::api::{err_json, ok_json};
use crate::config::keys::KeysStore;

pub async fn handle_keys_set(
    Json(body): Json<serde_json::Value>,
) -> impl IntoResponse {
    let name = body.get("name").and_then(|v| v.as_str()).unwrap_or("").to_string();
    let value = body.get("value").and_then(|v| v.as_str()).unwrap_or("").to_string();
    if name.is_empty() { return err_json("Missing 'name' field"); }

    let result = tokio::task::spawn_blocking(move || {
        KeysStore::set(&name, &value)
    }).await.unwrap();

    match result {
        Ok(_) => ok_json(serde_json::json!({"ok": true})),
        Err(e) => err_json(&e),
    }
}

pub async fn handle_keys_delete(
    Query(params): Query<HashMap<String, String>>,
) -> impl IntoResponse {
    let name = params.get("name").map(|s| s.as_str()).unwrap_or("");
    if name.is_empty() { return err_json("Missing 'name' query param"); }

    let name = name.to_string();
    let result = tokio::task::spawn_blocking(move || {
        KeysStore::delete(&name)
    }).await.unwrap();

    match result {
        Ok(_) => ok_json(serde_json::json!({"ok": true})),
        Err(e) => err_json(&e),
    }
}

pub async fn handle_keys_list() -> impl IntoResponse {
    let store = KeysStore::load();
    let names = store.list_names();
    ok_json(serde_json::json!({"ok": true, "keys": names}))
}
