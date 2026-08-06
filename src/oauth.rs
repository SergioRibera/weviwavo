use std::path::PathBuf;

use crate::utils::data_dir;

fn token_path() -> PathBuf {
    data_dir(&["oauth_refresh_token"])
}

pub fn load_refresh_token() -> Option<String> {
    std::fs::read_to_string(token_path())
        .ok()
        .map(|s| s.trim().to_owned())
        .filter(|s| !s.is_empty())
}

pub fn save_refresh_token(token: &str) {
    let _ = std::fs::write(token_path(), token);
}

pub fn clear_refresh_token() {
    let _ = std::fs::remove_file(token_path());
}
