use std::collections::HashSet;

use crate::types::GetCookiesResult;

#[cfg(target_os = "windows")]
use super::chromium::crypto::decrypt_chromium_aes256_gcm;
#[cfg(any(target_os = "macos", target_os = "linux"))]
use super::chromium::crypto::{decrypt_chromium_aes128_cbc, derive_aes128_cbc_key};
#[cfg(any(target_os = "macos", target_os = "linux", target_os = "windows"))]
use super::chromium::paths;
#[cfg(any(target_os = "macos", target_os = "linux", target_os = "windows"))]
use super::chromium::shared::{DecryptFn, get_cookies_from_chrome_sqlite_db};

#[derive(Debug, Default)]
pub struct EdgeOptions {
    pub profile: Option<String>,
    pub timeout_ms: Option<u64>,
    pub include_expired: Option<bool>,
    pub debug: Option<bool>,
}

pub async fn get_cookies_from_edge(
    options: EdgeOptions,
    origins: &[String],
    allowlist_names: Option<&HashSet<String>>,
) -> GetCookiesResult {
    #[cfg(target_os = "macos")]
    {
        get_cookies_from_edge_macos(&options, origins, allowlist_names).await
    }
    #[cfg(target_os = "linux")]
    {
        get_cookies_from_edge_linux(&options, origins, allowlist_names).await
    }
    #[cfg(target_os = "windows")]
    {
        get_cookies_from_edge_windows(&options, origins, allowlist_names).await
    }
    #[cfg(not(any(target_os = "macos", target_os = "linux", target_os = "windows")))]
    {
        let _ = (&options, origins, allowlist_names);
        GetCookiesResult {
            cookies: vec![],
            warnings: vec![],
        }
    }
}

#[cfg(target_os = "macos")]
async fn get_cookies_from_edge_macos(
    options: &EdgeOptions,
    origins: &[String],
    allowlist_names: Option<&HashSet<String>>,
) -> GetCookiesResult {
    use super::chromium::keychain::read_keychain_generic_password_first;

    let roots = paths::edge_roots();
    let db_path =
        paths::resolve_cookies_db_from_profile_or_roots(options.profile.as_deref(), &roots);
    let db_path = match db_path {
        Some(p) => p,
        None => {
            return GetCookiesResult {
                cookies: vec![],
                warnings: vec!["Edge cookies database not found.".to_string()],
            };
        }
    };

    let mut warnings = Vec::new();
    let password_result = read_keychain_generic_password_first(
        "Microsoft Edge",
        &["Microsoft Edge Safe Storage", "Microsoft Edge"],
        options.timeout_ms.unwrap_or(3_000),
        "Microsoft Edge Safe Storage",
    )
    .await;

    let edge_password = match password_result {
        Ok(p) => p,
        Err(e) => {
            warnings.push(e);
            return GetCookiesResult {
                cookies: vec![],
                warnings,
            };
        }
    };

    if edge_password.trim().is_empty() {
        warnings.push(
            "macOS Keychain returned an empty Microsoft Edge Safe Storage password.".to_string(),
        );
        return GetCookiesResult {
            cookies: vec![],
            warnings,
        };
    }

    let key = derive_aes128_cbc_key(edge_password.trim(), 1003);
    let decrypt: DecryptFn = Box::new(move |encrypted_value: &[u8], strip_hash_prefix: bool| {
        decrypt_chromium_aes128_cbc(
            encrypted_value,
            std::slice::from_ref(&key),
            strip_hash_prefix,
            true,
        )
    });

    let mut result = get_cookies_from_chrome_sqlite_db(
        &db_path.to_string_lossy(),
        options.profile.as_deref(),
        options.include_expired.unwrap_or(false),
        origins,
        allowlist_names,
        decrypt,
    )
    .await;
    let mut combined_warnings = warnings;
    combined_warnings.append(&mut result.warnings);
    result.warnings = combined_warnings;
    result
}

#[cfg(target_os = "linux")]
async fn get_cookies_from_edge_linux(
    options: &EdgeOptions,
    origins: &[String],
    allowlist_names: Option<&HashSet<String>>,
) -> GetCookiesResult {
    use super::chromium::linux_keyring::get_linux_chromium_safe_storage_password;

    let roots = paths::edge_roots();
    let db_path =
        paths::resolve_cookies_db_from_profile_or_roots(options.profile.as_deref(), &roots);
    let db_path = match db_path {
        Some(p) => p,
        None => {
            return GetCookiesResult {
                cookies: vec![],
                warnings: vec!["Edge cookies database not found.".to_string()],
            };
        }
    };

    let (password, mut keyring_warnings) =
        get_linux_chromium_safe_storage_password("edge", None).await;

    let v10_key = derive_aes128_cbc_key("peanuts", 1);
    let empty_key = derive_aes128_cbc_key("", 1);
    let v11_key = derive_aes128_cbc_key(&password, 1);

    let decrypt: DecryptFn = Box::new(move |encrypted_value: &[u8], strip_hash_prefix: bool| {
        if encrypted_value.len() >= 3 {
            let prefix = std::str::from_utf8(&encrypted_value[..3]).unwrap_or("");
            if prefix == "v10" {
                return decrypt_chromium_aes128_cbc(
                    encrypted_value,
                    &[v10_key.clone(), empty_key.clone()],
                    strip_hash_prefix,
                    false,
                );
            }
            if prefix == "v11" {
                return decrypt_chromium_aes128_cbc(
                    encrypted_value,
                    &[v11_key.clone(), empty_key.clone()],
                    strip_hash_prefix,
                    false,
                );
            }
        }
        None
    });

    let mut result = get_cookies_from_chrome_sqlite_db(
        &db_path.to_string_lossy(),
        options.profile.as_deref(),
        options.include_expired.unwrap_or(false),
        origins,
        allowlist_names,
        decrypt,
    )
    .await;
    keyring_warnings.append(&mut result.warnings);
    result.warnings = keyring_warnings;
    result
}

#[cfg(target_os = "windows")]
async fn get_cookies_from_edge_windows(
    options: &EdgeOptions,
    origins: &[String],
    allowlist_names: Option<&HashSet<String>>,
) -> GetCookiesResult {
    use super::chromium::windows_master_key::get_windows_chromium_master_key;

    let (db_path, user_data_dir) = paths::resolve_chromium_paths_windows(
        "Microsoft\\Edge\\User Data",
        options.profile.as_deref(),
    );
    let db_path = match db_path {
        Some(p) => p,
        None => {
            return GetCookiesResult {
                cookies: vec![],
                warnings: vec!["Edge cookies database not found.".to_string()],
            };
        }
    };
    let user_data_dir = match user_data_dir {
        Some(d) => d,
        None => {
            return GetCookiesResult {
                cookies: vec![],
                warnings: vec!["Edge user data directory not found.".to_string()],
            };
        }
    };

    let master_key = match get_windows_chromium_master_key(&user_data_dir, "Edge").await {
        Ok(k) => k,
        Err(e) => {
            return GetCookiesResult {
                cookies: vec![],
                warnings: vec![e],
            };
        }
    };

    let decrypt: DecryptFn = Box::new(move |encrypted_value: &[u8], strip_hash_prefix: bool| {
        decrypt_chromium_aes256_gcm(encrypted_value, &master_key, strip_hash_prefix)
    });

    get_cookies_from_chrome_sqlite_db(
        &db_path.to_string_lossy(),
        options.profile.as_deref(),
        options.include_expired.unwrap_or(false),
        origins,
        allowlist_names,
        decrypt,
    )
    .await
}
