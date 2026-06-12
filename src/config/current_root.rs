use std::path::PathBuf;
use std::sync::{OnceLock, RwLock};

static CURRENT_ROOT: OnceLock<RwLock<PathBuf>> = OnceLock::new();

pub fn init() {
    let root = crate::config::props::UserProps::load()
        .get("current_dir")
        .map(PathBuf::from)
        .unwrap_or_else(|| std::env::current_dir().unwrap_or_default());
    CURRENT_ROOT.set(RwLock::new(root)).ok();
}

pub fn get() -> PathBuf {
    CURRENT_ROOT
        .get()
        .map(|lock| lock.read().unwrap().clone())
        .unwrap_or_else(|| std::env::current_dir().unwrap_or_default())
}

pub fn set(path: &str) -> Result<(), String> {
    let root = PathBuf::from(path);
    if let Some(lock) = CURRENT_ROOT.get() {
        *lock.write().unwrap() = root.clone();
    }
    crate::config::props::UserProps::set("current_dir", path)?;
    std::env::set_current_dir(&root)
        .map_err(|e| format!("Failed to set cwd: {}", e))?;
    Ok(())
}
