use crate::utils::data_dir;

pub fn load_volume() -> f32 {
    let path = data_dir(&["volume"]);
    std::fs::read_to_string(&path)
        .ok()
        .and_then(|s| s.trim().parse().ok())
        .unwrap_or(1.0)
}

pub fn save_volume(vol: f32) {
    let path = data_dir(&["volume"]);
    let _ = std::fs::write(path, vol.to_string());
}
