use std::fs::{self, File};
use std::path::{Path, PathBuf};

#[derive(serde::Serialize)]
pub struct FsEntry {
    pub name: String,
    pub path: String,
    pub entry_type: String,
    pub size: Option<u64>,
    pub modified: Option<String>,
}

pub fn list_dir(rel_path: &str, root: &Path) -> Result<Vec<FsEntry>, String> {
    let dir_path = sanitize_path(rel_path, root)?;

    let read_dir = fs::read_dir(&dir_path)
        .map_err(|e| format!("Cannot read directory: {}", e))?;

    let mut entries = Vec::new();
    for entry in read_dir {
        let entry = entry.map_err(|e| format!("Cannot read entry: {}", e))?;
        let name = entry.file_name().to_string_lossy().to_string();
        let metadata = entry.metadata().ok();

        let entry_type = if entry.file_type().ok().map(|t| t.is_dir()).unwrap_or(false) {
            "dir".to_string()
        } else {
            "file".to_string()
        };

        let rel = PathBuf::from(rel_path).join(&name);
        let rel_str = rel.to_string_lossy().to_string().replace('\\', "/");

        let size = metadata.as_ref().and_then(|m| {
            if m.is_file() { Some(m.len()) } else { None }
        });

        let modified = metadata.and_then(|m| {
            m.modified().ok().map(|t| {
                let duration = t.duration_since(std::time::UNIX_EPOCH).unwrap_or_default();
                let secs = duration.as_secs();
                let millis = duration.subsec_millis();
                format!("{}", secs * 1000 + millis as u64)
            })
        });

        entries.push(FsEntry {
            name,
            path: rel_str,
            entry_type,
            size,
            modified,
        });
    }

    entries.sort_by(|a, b| {
        if a.entry_type != b.entry_type {
            if a.entry_type == "dir" { std::cmp::Ordering::Less }
            else { std::cmp::Ordering::Greater }
        } else {
            a.name.to_lowercase().cmp(&b.name.to_lowercase())
        }
    });

    Ok(entries)
}

pub fn read_file(rel_path: &str, root: &Path) -> Result<String, String> {
    let file_path = sanitize_path(rel_path, root)?;

    if !file_path.is_file() {
        return Err("Not a file".to_string());
    }

    fs::read_to_string(&file_path)
        .map_err(|e| format!("Cannot read file: {}", e))
}

pub fn create_entry(rel_path: &str, root: &Path) -> Result<(), String> {
    let requested = rel_path.trim_start_matches('/');
    let root_canonical = root.canonicalize()
        .map_err(|_| "Root path not found".to_string())?;

    // Build full path from root + requested, without requiring the path to exist
    let full_path = root_canonical.join(requested);

    // Verify the parent directory exists and is within the root
    let parent = full_path.parent().ok_or("Invalid path".to_string())?;
    let parent_canonical = parent.canonicalize()
        .map_err(|_| "Parent directory does not exist".to_string())?;
    if !parent_canonical.starts_with(&root_canonical) {
        return Err("Path traversal detected".to_string());
    }

    if rel_path.ends_with('/') {
        fs::create_dir(&full_path).map_err(|e| format!("Cannot create directory: {}", e))?;
    } else {
        File::create(&full_path).map_err(|e| format!("Cannot create file: {}", e))?;
    }
    Ok(())
}

pub fn delete_entry(rel_path: &str, root: &Path) -> Result<(), String> {
    let full_path = sanitize_path(rel_path, root)?;
    if full_path.is_dir() {
        fs::remove_dir_all(&full_path)
            .map_err(|e| format!("Cannot delete directory: {}", e))?;
    } else {
        fs::remove_file(&full_path)
            .map_err(|e| format!("Cannot delete file: {}", e))?;
    }
    Ok(())
}

pub fn rename_entry(rel_path: &str, new_name: &str, root: &Path) -> Result<(), String> {
    let full_path = sanitize_path(rel_path, root)?;
    let root_canonical = root.canonicalize()
        .map_err(|_| "Root path not found".to_string())?;
    let parent = full_path.parent().ok_or("Invalid path".to_string())?;
    let new_path = parent.join(new_name);

    let parent_canonical = parent.canonicalize()
        .map_err(|_| "Parent directory does not exist".to_string())?;
    if !parent_canonical.starts_with(&root_canonical) {
        return Err("Path traversal detected".to_string());
    }

    fs::rename(&full_path, &new_path)
        .map_err(|e| format!("Cannot rename: {}", e))?;
    Ok(())
}

pub fn move_entry(rel_path: &str, dest: &str, root: &Path) -> Result<(), String> {
    let full_path = sanitize_path(rel_path, root)?;

    let root_canonical = root.canonicalize()
        .map_err(|_| "Root path not found".to_string())?;

    let dest_requested = dest.trim_start_matches('/');
    let dest_path = root_canonical.join(dest_requested);

    let dest_parent = dest_path.parent().ok_or("Invalid destination")?;
    let dest_parent_canonical = if dest_parent.exists() {
        dest_parent.canonicalize().map_err(|_| "Cannot resolve destination parent")?
    } else {
        fs::create_dir_all(dest_parent)
            .map_err(|e| format!("Cannot create destination directory: {}", e))?;
        dest_parent.canonicalize().map_err(|_| "Cannot resolve destination parent")?
    };

    if !dest_parent_canonical.starts_with(&root_canonical) {
        return Err("Path traversal detected".to_string());
    }

    let safe_dest = dest_parent_canonical.join(
        dest_path.file_name().ok_or("Invalid destination filename")?
    );

    fs::rename(&full_path, &safe_dest)
        .map_err(|e| format!("Cannot move: {}", e))?;
    Ok(())
}

pub fn sanitize_path(requested: &str, root: &Path) -> Result<PathBuf, String> {
    let requested = requested.trim_start_matches('/');
    let joined = root.join(requested);

    let canonical = joined.canonicalize()
        .map_err(|_| "Path not found".to_string())?;

    let root_canonical = root.canonicalize()
        .map_err(|_| "Root path not found".to_string())?;

    if !canonical.starts_with(&root_canonical) {
        return Err("Path traversal detected".to_string());
    }

    Ok(canonical)
}
