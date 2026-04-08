use std::collections::HashSet;
use std::path::{Path, PathBuf};

use crate::types::{Cookie, CookieSameSite, CookieSource, GetCookiesResult};
use crate::util::host_match::host_matches_cookie_domain;
use url::Url;

pub async fn get_cookies_from_firefox(
    options: FirefoxOptions,
    origins: &[String],
    allowlist_names: Option<&HashSet<String>>,
) -> GetCookiesResult {
    let mut warnings = Vec::new();
    let db_path = resolve_firefox_cookies_db(options.profile.as_deref());
    let db_path = match db_path {
        Some(p) => p,
        None => {
            warnings.push("Firefox cookies database not found.".to_string());
            return GetCookiesResult {
                cookies: vec![],
                warnings,
            };
        }
    };

    let temp_dir = match tempfile::Builder::new()
        .prefix("cookie-scoop-firefox-")
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

    let temp_db_path = temp_dir.path().join("cookies.sqlite");
    if let Err(e) = std::fs::copy(&db_path, &temp_db_path) {
        warnings.push(format!("Failed to copy Firefox cookie DB: {e}"));
        return GetCookiesResult {
            cookies: vec![],
            warnings,
        };
    }
    copy_sidecar(&db_path, &temp_db_path, "-wal");
    copy_sidecar(&db_path, &temp_db_path, "-shm");

    let hosts: Vec<String> = origins
        .iter()
        .filter_map(|o| {
            Url::parse(o)
                .ok()
                .and_then(|u| u.host_str().map(|h| h.to_string()))
        })
        .collect();
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64;
    let include_expired = options.include_expired.unwrap_or(false);

    let where_clause = build_host_where_clause(&hosts);
    let expiry_clause = if include_expired {
        String::new()
    } else {
        format!(" AND (expiry = 0 OR expiry > {now})")
    };
    let sql = format!(
        "SELECT name, value, host, path, expiry, isSecure, isHttpOnly, sameSite \
         FROM moz_cookies WHERE ({where_clause}){expiry_clause} ORDER BY expiry DESC;"
    );

    let db_path_str = temp_db_path.to_string_lossy().to_string();
    let profile = options.profile.clone();
    let names_owned = allowlist_names.cloned();
    let result = tokio::task::spawn_blocking(move || {
        query_firefox_cookies(
            &db_path_str,
            &sql,
            &hosts,
            include_expired,
            names_owned.as_ref(),
            profile.as_deref(),
        )
    })
    .await;

    match result {
        Ok(Ok(cookies)) => GetCookiesResult {
            cookies,
            warnings,
        },
        Ok(Err(e)) => {
            warnings.push(format!("Failed reading Firefox cookies: {e}"));
            GetCookiesResult {
                cookies: vec![],
                warnings,
            }
        }
        Err(e) => {
            warnings.push(format!("Firefox cookie task failed: {e}"));
            GetCookiesResult {
                cookies: vec![],
                warnings,
            }
        }
    }
}

#[derive(Debug, Default)]
pub struct FirefoxOptions {
    pub profile: Option<String>,
    pub include_expired: Option<bool>,
}

fn query_firefox_cookies(
    db_path: &str,
    sql: &str,
    hosts: &[String],
    include_expired: bool,
    allowlist_names: Option<&HashSet<String>>,
    profile: Option<&str>,
) -> Result<Vec<Cookie>, String> {
    let conn = rusqlite::Connection::open_with_flags(
        db_path,
        rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY | rusqlite::OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )
    .map_err(|e| e.to_string())?;

    let mut stmt = conn.prepare(sql).map_err(|e| e.to_string())?;

    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64;

    let rows = stmt
        .query_map([], |row| {
            let name: String = row.get(0)?;
            let value: String = row.get(1)?;
            let host: String = row.get(2)?;
            let path: String = row.get(3)?;
            let expiry: i64 = row.get(4)?;
            let is_secure: i32 = row.get(5)?;
            let is_http_only: i32 = row.get(6)?;
            let same_site: i32 = row.get(7)?;
            Ok((
                name,
                value,
                host,
                path,
                expiry,
                is_secure,
                is_http_only,
                same_site,
            ))
        })
        .map_err(|e| e.to_string())?;

    let mut cookies = Vec::new();
    for row in rows {
        let (name, value, host, path, expiry, is_secure, is_http_only, same_site) =
            row.map_err(|e| e.to_string())?;

        if name.is_empty() {
            continue;
        }
        if let Some(names) = allowlist_names {
            if !names.is_empty() && !names.contains(&name) {
                continue;
            }
        }

        let cookie_domain = host.strip_prefix('.').unwrap_or(&host);
        if !hosts
            .iter()
            .any(|h| host_matches_cookie_domain(h, cookie_domain))
        {
            continue;
        }

        let expires = if expiry > 0 { Some(expiry) } else { None };
        if !include_expired {
            if let Some(exp) = expires {
                if exp < now {
                    continue;
                }
            }
        }

        let domain = host.strip_prefix('.').unwrap_or(&host).to_string();
        let same_site_val = match same_site {
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
            value,
            domain: Some(domain),
            path: Some(if path.is_empty() {
                "/".to_string()
            } else {
                path
            }),
            url: None,
            expires,
            secure: Some(is_secure != 0),
            http_only: Some(is_http_only != 0),
            same_site: same_site_val,
            source: Some(source),
        });
    }

    Ok(cookies)
}

fn resolve_firefox_cookies_db(profile: Option<&str>) -> Option<PathBuf> {
    let home = dirs::home_dir()?;

    let roots: Vec<PathBuf> = if cfg!(target_os = "macos") {
        vec![home.join("Library/Application Support/Firefox/Profiles")]
    } else if cfg!(target_os = "linux") {
        vec![home.join(".mozilla/firefox")]
    } else if cfg!(target_os = "windows") {
        if let Some(appdata) = std::env::var_os("APPDATA") {
            vec![PathBuf::from(appdata).join("Mozilla/Firefox/Profiles")]
        } else {
            vec![]
        }
    } else {
        vec![]
    };

    if let Some(profile) = profile {
        if looks_like_path(profile) {
            let p = PathBuf::from(profile);
            let candidate = if profile.ends_with("cookies.sqlite") {
                p
            } else {
                p.join("cookies.sqlite")
            };
            return if candidate.exists() {
                Some(candidate)
            } else {
                None
            };
        }
    }

    for root in &roots {
        if !root.exists() {
            continue;
        }
        if let Some(profile) = profile {
            let candidate = root.join(profile).join("cookies.sqlite");
            if candidate.exists() {
                return Some(candidate);
            }
            continue;
        }

        let entries = safe_readdir(root);
        let default_release = entries.iter().find(|e| e.contains("default-release"));
        let picked = default_release.or(entries.first());
        if let Some(picked) = picked {
            let candidate = root.join(picked).join("cookies.sqlite");
            if candidate.exists() {
                return Some(candidate);
            }
        }
    }

    None
}

fn safe_readdir(dir: &Path) -> Vec<String> {
    match std::fs::read_dir(dir) {
        Ok(entries) => entries
            .filter_map(|e| e.ok())
            .filter(|e| e.file_type().map(|t| t.is_dir()).unwrap_or(false))
            .filter_map(|e| e.file_name().to_str().map(|s| s.to_string()))
            .collect(),
        Err(_) => vec![],
    }
}

fn looks_like_path(value: &str) -> bool {
    value.contains('/') || value.contains('\\')
}

fn copy_sidecar(source_db_path: &Path, temp_db_path: &Path, suffix: &str) {
    let sidecar = PathBuf::from(format!("{}{}", source_db_path.to_string_lossy(), suffix));
    let target = PathBuf::from(format!("{}{}", temp_db_path.to_string_lossy(), suffix));
    if sidecar.exists() {
        let _ = std::fs::copy(&sidecar, &target);
    }
}

fn build_host_where_clause(hosts: &[String]) -> String {
    let mut clauses = Vec::new();
    for host in hosts {
        let escaped = sql_literal(host);
        let escaped_dot = sql_literal(&format!(".{host}"));
        let escaped_like = sql_literal(&format!("%.{host}"));
        clauses.push(format!("host = {escaped}"));
        clauses.push(format!("host = {escaped_dot}"));
        clauses.push(format!("host LIKE {escaped_like}"));
    }
    if clauses.is_empty() {
        "1=0".to_string()
    } else {
        clauses.join(" OR ")
    }
}

fn sql_literal(value: &str) -> String {
    let escaped = value.replace('\'', "''");
    format!("'{escaped}'")
}
