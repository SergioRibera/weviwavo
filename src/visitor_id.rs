use std::path::PathBuf;

use crate::utils::cache_dir;

fn path() -> PathBuf {
    cache_dir(&["visitor_id"])
}

pub fn load() -> Option<String> {
    std::fs::read_to_string(path())
        .ok()
        .map(|s| s.trim().to_owned())
        .filter(|s| !s.is_empty())
}

pub fn save(visitor_id: &str) {
    let _ = std::fs::write(path(), visitor_id);
}
