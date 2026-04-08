#[cfg(target_os = "windows")]
use std::path::Path;
use std::path::PathBuf;

pub fn looks_like_path(value: &str) -> bool {
    value.contains('/') || value.contains('\\')
}

pub fn expand_path(input: &str) -> PathBuf {
    if let Some(rest) = input.strip_prefix("~/") {
        if let Some(home) = dirs::home_dir() {
            return home.join(rest);
        }
    }
    let p = PathBuf::from(input);
    if p.is_absolute() {
        p
    } else {
        std::env::current_dir()
            .unwrap_or_else(|_| PathBuf::from("."))
            .join(p)
    }
}

pub fn resolve_cookies_db_from_profile_or_roots(
    profile: Option<&str>,
    roots: &[PathBuf],
) -> Option<PathBuf> {
    let mut candidates = Vec::new();

    if let Some(profile) = profile {
        if looks_like_path(profile) {
            let expanded = expand_path(profile);
            if expanded.is_file() {
                return Some(expanded);
            }
            candidates.push(expanded.join("Cookies"));
            candidates.push(expanded.join("Network/Cookies"));
        } else {
            let profile_dir = if profile.trim().is_empty() {
                "Default"
            } else {
                profile.trim()
            };
            for root in roots {
                candidates.push(root.join(profile_dir).join("Cookies"));
                candidates.push(root.join(profile_dir).join("Network/Cookies"));
            }
        }
    } else {
        for root in roots {
            candidates.push(root.join("Default/Cookies"));
            candidates.push(root.join("Default/Network/Cookies"));
        }
    }

    for candidate in &candidates {
        if candidate.exists() {
            return Some(candidate.clone());
        }
    }

    None
}

#[cfg(target_os = "macos")]
pub fn chrome_roots() -> Vec<PathBuf> {
    dirs::home_dir()
        .map(|h| vec![h.join("Library/Application Support/Google/Chrome")])
        .unwrap_or_default()
}

#[cfg(target_os = "macos")]
pub fn edge_roots() -> Vec<PathBuf> {
    dirs::home_dir()
        .map(|h| vec![h.join("Library/Application Support/Microsoft Edge")])
        .unwrap_or_default()
}

#[cfg(target_os = "linux")]
pub fn chrome_roots() -> Vec<PathBuf> {
    let config_home = std::env::var("XDG_CONFIG_HOME")
        .ok()
        .filter(|s| !s.trim().is_empty())
        .map(PathBuf::from)
        .or_else(|| dirs::home_dir().map(|h| h.join(".config")));

    config_home
        .map(|c| vec![c.join("google-chrome")])
        .unwrap_or_default()
}

#[cfg(target_os = "linux")]
pub fn edge_roots() -> Vec<PathBuf> {
    let config_home = std::env::var("XDG_CONFIG_HOME")
        .ok()
        .filter(|s| !s.trim().is_empty())
        .map(PathBuf::from)
        .or_else(|| dirs::home_dir().map(|h| h.join(".config")));

    config_home
        .map(|c| vec![c.join("microsoft-edge")])
        .unwrap_or_default()
}

#[cfg(target_os = "windows")]
pub fn chrome_roots() -> Vec<PathBuf> {
    std::env::var("LOCALAPPDATA")
        .ok()
        .map(|la| vec![PathBuf::from(la).join("Google/Chrome/User Data")])
        .unwrap_or_default()
}

#[cfg(target_os = "windows")]
pub fn edge_roots() -> Vec<PathBuf> {
    std::env::var("LOCALAPPDATA")
        .ok()
        .map(|la| vec![PathBuf::from(la).join("Microsoft/Edge/User Data")])
        .unwrap_or_default()
}

#[cfg(not(any(target_os = "macos", target_os = "linux", target_os = "windows")))]
pub fn chrome_roots() -> Vec<PathBuf> {
    vec![]
}

#[cfg(not(any(target_os = "macos", target_os = "linux", target_os = "windows")))]
pub fn edge_roots() -> Vec<PathBuf> {
    vec![]
}

#[cfg(target_os = "windows")]
pub fn resolve_chromium_paths_windows(
    local_app_data_vendor_path: &str,
    profile: Option<&str>,
) -> (Option<PathBuf>, Option<PathBuf>) {
    let local_app_data = match std::env::var("LOCALAPPDATA") {
        Ok(la) => la,
        Err(_) => return (None, None),
    };
    let root = PathBuf::from(&local_app_data).join(local_app_data_vendor_path);

    if let Some(profile) = profile {
        if looks_like_path(profile) {
            let expanded = expand_path(profile);
            let candidates = if expanded.to_string_lossy().ends_with("Cookies") {
                vec![expanded.clone()]
            } else {
                vec![
                    expanded.join("Network/Cookies"),
                    expanded.join("Cookies"),
                    expanded.join("Default/Network/Cookies"),
                ]
            };
            for candidate in &candidates {
                if candidate.exists() {
                    let user_data_dir = find_user_data_dir(candidate);
                    return (Some(candidate.clone()), user_data_dir);
                }
            }
            if expanded.join("Local State").exists() {
                return (None, Some(expanded));
            }
        }
    }

    let profile_dir = profile
        .filter(|p| !p.trim().is_empty())
        .unwrap_or("Default");

    let candidates = vec![
        root.join(profile_dir).join("Network/Cookies"),
        root.join(profile_dir).join("Cookies"),
    ];
    for candidate in &candidates {
        if candidate.exists() {
            return (Some(candidate.clone()), Some(root));
        }
    }
    (None, Some(root))
}

#[cfg(target_os = "windows")]
fn find_user_data_dir(cookies_db_path: &Path) -> Option<PathBuf> {
    let mut current = cookies_db_path.parent()?;
    for _ in 0..6 {
        if current.join("Local State").exists() {
            return Some(current.to_path_buf());
        }
        current = current.parent()?;
    }
    None
}
