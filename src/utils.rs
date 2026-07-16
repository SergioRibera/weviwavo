use std::path::{Path, PathBuf};

use crate::APP_NAME;

pub fn data_dir(sub_dir: &[impl AsRef<Path>]) -> PathBuf {
    let base = dirs::data_dir()
        .unwrap_or_else(|| std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")))
        .join(APP_NAME);
    if !base.exists() {
        std::fs::create_dir_all(&base).unwrap();
    }
    sub_dir.iter().fold(base, |path, b| path.join(b))
}
