use crate::util::exec::exec_capture;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LinuxKeyringBackend {
    Gnome,
    Kwallet,
    Basic,
}

pub async fn get_linux_chromium_safe_storage_password(
    app: &str, // "chrome" or "edge"
    backend_override: Option<LinuxKeyringBackend>,
) -> (String, Vec<String>) {
    let mut warnings = Vec::new();

    // Check env override
    let override_key = if app == "edge" {
        "SWEET_COOKIE_EDGE_SAFE_STORAGE_PASSWORD"
    } else {
        "SWEET_COOKIE_CHROME_SAFE_STORAGE_PASSWORD"
    };

    if let Ok(val) = std::env::var(override_key) {
        let trimmed = val.trim().to_string();
        if !trimmed.is_empty() {
            return (trimmed, warnings);
        }
    }

    let backend = backend_override
        .or_else(parse_linux_keyring_backend)
        .unwrap_or_else(choose_linux_keyring_backend);

    if backend == LinuxKeyringBackend::Basic {
        return (String::new(), warnings);
    }

    let (service, account, folder) = if app == "edge" {
        (
            "Microsoft Edge Safe Storage",
            "Microsoft Edge",
            "Microsoft Edge Keys",
        )
    } else {
        ("Chrome Safe Storage", "Chrome", "Chrome Keys")
    };

    if backend == LinuxKeyringBackend::Gnome {
        // Try the new v2 schema first (application attribute), then fall back to old schema.
        // Modern Chrome versions store Safe Storage under `application=chrome`.
        let application_attr = if app == "edge" { "msedge" } else { "chrome" };
        let res = exec_capture(
            "secret-tool",
            &["lookup", "application", application_attr],
            Some(3_000),
        )
        .await;
        if res.code == 0 && !res.stdout.trim().is_empty() {
            return (res.stdout.trim().to_string(), warnings);
        }
        // Fall back to old schema (service/account)
        let res = exec_capture(
            "secret-tool",
            &["lookup", "service", service, "account", account],
            Some(3_000),
        )
        .await;
        if res.code == 0 {
            return (res.stdout.trim().to_string(), warnings);
        }
        warnings.push(
            "Failed to read Linux keyring via secret-tool; v11 cookies may be unavailable."
                .to_string(),
        );
        return (String::new(), warnings);
    }

    // KDE/KWallet
    let kde_version = std::env::var("KDE_SESSION_VERSION")
        .unwrap_or_default()
        .trim()
        .to_string();

    let (service_name, wallet_path) = match kde_version.as_str() {
        "6" => ("org.kde.kwalletd6", "/modules/kwalletd6"),
        "5" => ("org.kde.kwalletd5", "/modules/kwalletd5"),
        _ => ("org.kde.kwalletd", "/modules/kwalletd"),
    };

    let wallet = get_kwallet_network_wallet(service_name, wallet_path).await;
    let password_res = exec_capture(
        "kwallet-query",
        &["--read-password", service, "--folder", folder, &wallet],
        Some(3_000),
    )
    .await;

    if password_res.code != 0 {
        warnings.push(
            "Failed to read Linux keyring via kwallet-query; v11 cookies may be unavailable."
                .to_string(),
        );
        return (String::new(), warnings);
    }

    if password_res
        .stdout
        .to_lowercase()
        .starts_with("failed to read")
    {
        return (String::new(), warnings);
    }

    (password_res.stdout.trim().to_string(), warnings)
}

fn parse_linux_keyring_backend() -> Option<LinuxKeyringBackend> {
    let raw = std::env::var("SWEET_COOKIE_LINUX_KEYRING").ok()?;
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return None;
    }
    match trimmed.to_lowercase().as_str() {
        "gnome" => Some(LinuxKeyringBackend::Gnome),
        "kwallet" => Some(LinuxKeyringBackend::Kwallet),
        "basic" => Some(LinuxKeyringBackend::Basic),
        _ => None,
    }
}

fn choose_linux_keyring_backend() -> LinuxKeyringBackend {
    let xdg = std::env::var("XDG_CURRENT_DESKTOP").unwrap_or_default();
    let is_kde = xdg.split(':').any(|p| p.trim().eq_ignore_ascii_case("kde"))
        || std::env::var("KDE_FULL_SESSION").is_ok();

    if is_kde {
        LinuxKeyringBackend::Kwallet
    } else {
        LinuxKeyringBackend::Gnome
    }
}

async fn get_kwallet_network_wallet(service_name: &str, wallet_path: &str) -> String {
    let dest = format!("--dest={service_name}");
    let res = exec_capture(
        "dbus-send",
        &[
            "--session",
            "--print-reply=literal",
            &dest,
            wallet_path,
            "org.kde.KWallet.networkWallet",
        ],
        Some(3_000),
    )
    .await;

    let fallback = "kdewallet".to_string();
    if res.code != 0 {
        return fallback;
    }
    let raw = res.stdout.trim().replace('"', "");
    if raw.is_empty() {
        fallback
    } else {
        raw
    }
}
