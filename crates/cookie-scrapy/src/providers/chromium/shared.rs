use std::collections::HashSet;
use std::path::{Path, PathBuf};

use crate::types::{Cookie, CookieSameSite, CookieSource, GetCookiesResult};
use crate::util::expire::normalize_expiration;
use crate::util::host_match::host_matches_cookie_domain;
use url::Url;

pub type DecryptFn = Box<dyn Fn(&[u8], bool) -> Option<String> + Send + Sync>;

pub async fn get_cookies_from_chrome_sqlite_db(
    db_path: &str,
    profile: Option<&str>,
    include_expired: bool,
    origins: &[String],
    allowlist_names: Option<&HashSet<String>>,
    decrypt: DecryptFn,
) -> GetCookiesResult {
    let mut warnings = Vec::new();

    let temp_dir = match tempfile::Builder::new()
        .prefix("cookie-scoop-chrome-")
        .tempdir()
    {
        Ok(d) => d,
        Err(e) => {
            warnings.push(format!("Failed to create temp dir: {e}"));
            return GetCookiesResult {
                cookies: vec![],
                warnings,
            };
        }
    };

    let temp_db_path = temp_dir.path().join("Cookies");
    let source_path = Path::new(db_path);
    if let Err(e) = std::fs::copy(source_path, &temp_db_path) {
        warnings.push(format!("Failed to copy Chrome cookie DB: {e}"));
        return GetCookiesResult {
            cookies: vec![],
            warnings,
        };
    }
    copy_sidecar(source_path, &temp_db_path, "-wal");
    copy_sidecar(source_path, &temp_db_path, "-shm");

    let hosts: Vec<String> = origins
        .iter()
        .filter_map(|o| {
            Url::parse(o)
                .ok()
                .and_then(|u| u.host_str().map(|h| h.to_string()))
        })
        .collect();
    let where_clause = build_host_where_clause(&hosts);

    let temp_db_str = temp_db_path.to_string_lossy().to_string();
    let profile_owned = profile.map(|s| s.to_string());
    let names_owned = allowlist_names.cloned();
    let hosts_clone = hosts.clone();

    let result = tokio::task::spawn_blocking(move || {
        query_chrome_cookies(
            &temp_db_str,
            &where_clause,
            &hosts_clone,
            include_expired,
            names_owned.as_ref(),
            profile_owned.as_deref(),
            &decrypt,
        )
    })
    .await;

    match result {
        Ok(Ok((cookies, mut db_warnings))) => {
            warnings.append(&mut db_warnings);
            GetCookiesResult {
                cookies,
                warnings,
            }
        }
        Ok(Err(e)) => {
            warnings.push(e);
            GetCookiesResult {
                cookies: vec![],
                warnings,
            }
        }
        Err(e) => {
            warnings.push(format!("Chrome cookie task failed: {e}"));
            GetCookiesResult {
                cookies: vec![],
                warnings,
            }
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn query_chrome_cookies(
    db_path: &str,
    where_clause: &str,
    hosts: &[String],
    include_expired: bool,
    allowlist_names: Option<&HashSet<String>>,
    profile: Option<&str>,
    decrypt: &DecryptFn,
) -> Result<(Vec<Cookie>, Vec<String>), String> {
    let mut warnings = Vec::new();
    let conn = rusqlite::Connection::open_with_flags(
        db_path,
        rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY | rusqlite::OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )
    .map_err(|e| format!("Failed to open Chrome cookie DB: {e}"))?;

    let meta_version = read_meta_version(&conn);
    let strip_hash_prefix = meta_version >= 24;

    let sql = format!(
        "SELECT name, value, host_key, path, expires_utc, samesite, encrypted_value, \
         is_secure, is_httponly \
         FROM cookies WHERE ({where_clause}) ORDER BY expires_utc DESC;"
    );

    let mut stmt = conn.prepare(&sql).map_err(|e| {
        format!("Failed reading Chrome cookies (requires modern Chromium, e.g. Chrome >= 100): {e}")
    })?;

    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64;

    let mut cookies = Vec::new();
    let mut warned_encrypted_type = false;

    let rows = stmt
        .query_map([], |row| {
            let name: String = row.get(0)?;
            let value: String = row.get(1)?;
            let host_key: String = row.get(2)?;
            let path: String = row.get(3)?;
            let expires_utc: i64 = row.get(4)?;
            let samesite: i32 = row.get(5)?;
            let encrypted_value: Option<Vec<u8>> = row.get(6)?;
            let is_secure: i32 = row.get(7)?;
            let is_httponly: i32 = row.get(8)?;
            Ok((
                name,
                value,
                host_key,
                path,
                expires_utc,
                samesite,
                encrypted_value,
                is_secure,
                is_httponly,
            ))
        })
        .map_err(|e| e.to_string())?;

    for row in rows {
        let (
            name,
            value,
            host_key,
            path,
            expires_utc,
            samesite,
            encrypted_value,
            is_secure,
            is_httponly,
        ) = row.map_err(|e| e.to_string())?;

        if name.is_empty() {
            continue;
        }
        if let Some(names) = allowlist_names {
            if !names.is_empty() && !names.contains(&name) {
                continue;
            }
        }

        let cookie_domain = host_key.strip_prefix('.').unwrap_or(&host_key);
        if !hosts
            .iter()
            .any(|h| host_matches_cookie_domain(h, cookie_domain))
        {
            continue;
        }

        let mut cookie_value: Option<String> = if !value.is_empty() { Some(value) } else { None };

        if cookie_value.is_none() {
            if let Some(ref enc_bytes) = encrypted_value {
                if !enc_bytes.is_empty() {
                    cookie_value = decrypt(enc_bytes, strip_hash_prefix);
                }
            } else if encrypted_value.is_some() && !warned_encrypted_type {
                warnings
                    .push("Chrome cookie encrypted_value is in an unsupported type.".to_string());
                warned_encrypted_type = true;
            }
        }

        let cookie_value = match cookie_value {
            Some(v) => v,
            None => continue,
        };

        let expires = if expires_utc != 0 {
            normalize_expiration(expires_utc)
        } else {
            None
        };

        if !include_expired {
            if let Some(exp) = expires {
                if exp < now {
                    continue;
                }
            }
        }

        let domain = host_key.strip_prefix('.').unwrap_or(&host_key).to_string();

        let same_site = match samesite {
            2 => Some(CookieSameSite::Strict),
            1 => Some(CookieSameSite::Lax),
            0 => Some(CookieSameSite::None),
            _ => None,
        };

        let mut source = CookieSource {
            profile: None,
            origin: None,
            store_id: None,
        };
        if let Some(p) = profile {
            source.profile = Some(p.to_string());
        }

        cookies.push(Cookie {
            name,
            value: cookie_value,
            domain: Some(domain),
            path: Some(if path.is_empty() {
                "/".to_string()
            } else {
                path
            }),
            url: None,
            expires,
            secure: Some(is_secure != 0),
            http_only: Some(is_httponly != 0),
            same_site,
            source: Some(source),
        });
    }

    Ok((cookies, warnings))
}

fn read_meta_version(conn: &rusqlite::Connection) -> i64 {
    // The meta table stores version as text, so try String first, then i64.
    let result: Result<String, _> =
        conn.query_row("SELECT value FROM meta WHERE key = 'version'", [], |row| {
            row.get(0)
        });
    match result {
        Ok(s) => s.trim().parse::<i64>().unwrap_or(0),
        Err(_) => conn
            .query_row("SELECT value FROM meta WHERE key = 'version'", [], |row| {
                row.get::<_, i64>(0)
            })
            .unwrap_or(0),
    }
}

fn copy_sidecar(source_path: &Path, temp_path: &Path, suffix: &str) {
    let sidecar = PathBuf::from(format!("{}{suffix}", source_path.to_string_lossy()));
    let target = PathBuf::from(format!("{}{suffix}", temp_path.to_string_lossy()));
    if sidecar.exists() {
        let _ = std::fs::copy(&sidecar, &target);
    }
}

fn build_host_where_clause(hosts: &[String]) -> String {
    let mut clauses = Vec::new();
    for host in hosts {
        for candidate in expand_host_candidates(host) {
            let escaped = sql_literal(&candidate);
            let escaped_dot = sql_literal(&format!(".{candidate}"));
            let escaped_like = sql_literal(&format!("%.{candidate}"));
            clauses.push(format!("host_key = {escaped}"));
            clauses.push(format!("host_key = {escaped_dot}"));
            clauses.push(format!("host_key LIKE {escaped_like}"));
        }
    }
    if clauses.is_empty() {
        "1=0".to_string()
    } else {
        clauses.join(" OR ")
    }
}

fn expand_host_candidates(host: &str) -> Vec<String> {
    let parts: Vec<&str> = host.split('.').filter(|p| !p.is_empty()).collect();
    if parts.len() <= 1 {
        return vec![host.to_string()];
    }
    let mut candidates = Vec::new();
    candidates.push(host.to_string());
    // Include parent domains down to two labels (avoid TLD-only)
    for i in 1..=(parts.len().saturating_sub(2)) {
        let candidate = parts[i..].join(".");
        if !candidate.is_empty() {
            candidates.push(candidate);
        }
    }
    candidates
}

fn sql_literal(value: &str) -> String {
    let escaped = value.replace('\'', "''");
    format!("'{escaped}'")
}
