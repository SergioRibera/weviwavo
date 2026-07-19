use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::PathBuf;

use chacha20poly1305::aead::{Aead, KeyInit, OsRng};
use chacha20poly1305::{ChaCha20Poly1305, Nonce};
use rand::RngCore;

use crate::utils::data_dir;

const NONCE_LEN: usize = 12;

fn key_path() -> PathBuf {
    data_dir(&[".keyfile"])
}

fn cookies_path() -> PathBuf {
    data_dir(&["cookies.enc"])
}

fn load_or_create_key() -> [u8; 32] {
    let path = key_path();
    if let Ok(bytes) = fs::read(&path) {
        if bytes.len() == 32 {
            let mut key = [0u8; 32];
            key.copy_from_slice(&bytes);
            return key;
        }
    }
    let mut key = [0u8; 32];
    OsRng.fill_bytes(&mut key);
    if let Err(e) = fs::write(&path, &key) {
        tracing::warn!(error = %e, "failed to persist encryption key");
    } else {
        let _ = fs::set_permissions(&path, fs::Permissions::from_mode(0o600));
    }
    key
}

/// Load and decrypt saved cookies from disk. Returns `None` if absent or corrupt.
pub fn load_cookies() -> Option<String> {
    let ciphertext = fs::read(cookies_path()).ok()?;
    if ciphertext.len() < NONCE_LEN {
        return None;
    }
    let (nonce_bytes, ct) = ciphertext.split_at(NONCE_LEN);
    let key = load_or_create_key();
    let cipher = ChaCha20Poly1305::new(key.as_ref().into());
    let nonce = Nonce::from_slice(nonce_bytes);
    let plaintext = cipher.decrypt(nonce, ct).ok()?;
    String::from_utf8(plaintext).ok()
}

/// Encrypt and persist cookies to disk.
pub fn save_cookies(header: &str) {
    let key = load_or_create_key();
    let cipher = ChaCha20Poly1305::new(key.as_ref().into());
    let mut nonce_bytes = [0u8; NONCE_LEN];
    OsRng.fill_bytes(&mut nonce_bytes);
    let nonce = Nonce::from_slice(&nonce_bytes);
    let Ok(ct) = cipher.encrypt(nonce, header.as_bytes()) else {
        tracing::error!("cookie encryption failed");
        return;
    };
    let mut out = Vec::with_capacity(NONCE_LEN + ct.len());
    out.extend_from_slice(&nonce_bytes);
    out.extend_from_slice(&ct);
    let path = cookies_path();
    if let Err(e) = fs::write(&path, &out) {
        tracing::error!(error = %e, "failed to write cookies file");
    } else {
        let _ = fs::set_permissions(&path, fs::Permissions::from_mode(0o600));
        tracing::info!("cookies saved to disk");
    }
}
