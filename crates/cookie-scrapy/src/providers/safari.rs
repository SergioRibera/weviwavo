use std::collections::HashSet;

use crate::types::GetCookiesResult;
#[cfg(any(target_os = "macos", test))]
use crate::types::{Cookie, CookieSource};
#[cfg(target_os = "macos")]
use crate::util::host_match::host_matches_cookie_domain;
#[cfg(any(target_os = "macos", test))]
use url::Url;

#[cfg(any(target_os = "macos", test))]
const MAC_EPOCH_DELTA_SECONDS: i64 = 978_307_200;

pub async fn get_cookies_from_safari(
    options: SafariOptions,
    origins: &[String],
    allowlist_names: Option<&HashSet<String>>,
) -> GetCookiesResult {
    #[cfg(not(target_os = "macos"))]
    {
        let _ = (&options, origins, allowlist_names);
        GetCookiesResult {
            cookies: vec![],
            warnings: vec![],
        }
    }

    #[cfg(target_os = "macos")]
    {
        let mut warnings = Vec::new();
        let cookie_file = options.file.or_else(resolve_safari_binary_cookies_path);
        let cookie_file = match cookie_file {
            Some(f) => f,
            None => {
                warnings.push("Safari Cookies.binarycookies not found.".to_string());
                return GetCookiesResult {
                    cookies: vec![],
                    warnings,
                };
            }
        };

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

        let data = match std::fs::read(&cookie_file) {
            Ok(d) => d,
            Err(e) => {
                warnings.push(format!("Failed to read Safari cookies: {e}"));
                return GetCookiesResult {
                    cookies: vec![],
                    warnings,
                };
            }
        };

        let parsed = decode_binary_cookies(&data);
        let mut cookies = Vec::new();
        for cookie in parsed {
            if cookie.name.is_empty() {
                continue;
            }
            if let Some(names) = allowlist_names {
                if !names.is_empty() && !names.contains(&cookie.name) {
                    continue;
                }
            }
            let domain = match &cookie.domain {
                Some(d) => d,
                None => continue,
            };
            if !hosts.iter().any(|h| host_matches_cookie_domain(h, domain)) {
                continue;
            }
            if !options.include_expired.unwrap_or(false) {
                if let Some(expires) = cookie.expires {
                    if expires < now {
                        continue;
                    }
                }
            }
            cookies.push(cookie);
        }

        GetCookiesResult {
            cookies,
            warnings,
        }
    }
}

#[derive(Debug, Default)]
pub struct SafariOptions {
    pub include_expired: Option<bool>,
    pub file: Option<String>,
}

#[cfg(target_os = "macos")]
fn resolve_safari_binary_cookies_path() -> Option<String> {
    let home = dirs::home_dir()?;
    let candidates = [
        home.join("Library/Cookies/Cookies.binarycookies"),
        home.join("Library/Containers/com.apple.Safari/Data/Library/Cookies/Cookies.binarycookies"),
    ];
    for candidate in &candidates {
        if candidate.exists() {
            return Some(candidate.to_string_lossy().to_string());
        }
    }
    None
}

#[cfg(any(target_os = "macos", test))]
fn decode_binary_cookies(buffer: &[u8]) -> Vec<Cookie> {
    if buffer.len() < 8 {
        return vec![];
    }
    if &buffer[0..4] != b"cook" {
        return vec![];
    }
    let page_count = u32::from_be_bytes([buffer[4], buffer[5], buffer[6], buffer[7]]) as usize;
    let mut cursor = 8;
    let mut page_sizes = Vec::new();
    for _ in 0..page_count {
        if cursor + 4 > buffer.len() {
            return vec![];
        }
        let size = u32::from_be_bytes([
            buffer[cursor],
            buffer[cursor + 1],
            buffer[cursor + 2],
            buffer[cursor + 3],
        ]) as usize;
        page_sizes.push(size);
        cursor += 4;
    }

    let mut cookies = Vec::new();
    for page_size in page_sizes {
        if cursor + page_size > buffer.len() {
            break;
        }
        let page = &buffer[cursor..cursor + page_size];
        cookies.extend(decode_page(page));
        cursor += page_size;
    }
    cookies
}

#[cfg(any(target_os = "macos", test))]
fn decode_page(page: &[u8]) -> Vec<Cookie> {
    if page.len() < 16 {
        return vec![];
    }
    let header = u32::from_be_bytes([page[0], page[1], page[2], page[3]]);
    if header != 0x00000100 {
        return vec![];
    }
    let cookie_count = u32::from_le_bytes([page[4], page[5], page[6], page[7]]) as usize;
    let mut offsets = Vec::new();
    let mut cursor = 8;
    for _ in 0..cookie_count {
        if cursor + 4 > page.len() {
            return vec![];
        }
        let offset = u32::from_le_bytes([
            page[cursor],
            page[cursor + 1],
            page[cursor + 2],
            page[cursor + 3],
        ]) as usize;
        offsets.push(offset);
        cursor += 4;
    }

    let mut cookies = Vec::new();
    for offset in offsets {
        if offset < page.len() {
            if let Some(cookie) = decode_cookie(&page[offset..]) {
                cookies.push(cookie);
            }
        }
    }
    cookies
}

#[cfg(any(target_os = "macos", test))]
fn decode_cookie(buf: &[u8]) -> Option<Cookie> {
    if buf.len() < 48 {
        return None;
    }

    let size = u32::from_le_bytes([buf[0], buf[1], buf[2], buf[3]]) as usize;
    if size < 48 || size > buf.len() {
        return None;
    }

    let flags_value = u32::from_le_bytes([buf[8], buf[9], buf[10], buf[11]]);
    let is_secure = (flags_value & 1) != 0;
    let is_http_only = (flags_value & 4) != 0;

    let url_offset = u32::from_le_bytes([buf[16], buf[17], buf[18], buf[19]]) as usize;
    let name_offset = u32::from_le_bytes([buf[20], buf[21], buf[22], buf[23]]) as usize;
    let path_offset = u32::from_le_bytes([buf[24], buf[25], buf[26], buf[27]]) as usize;
    let value_offset = u32::from_le_bytes([buf[28], buf[29], buf[30], buf[31]]) as usize;

    let expiration = read_double_le(buf, 40);

    let raw_url = read_c_string(buf, url_offset, size);
    let name = read_c_string(buf, name_offset, size)?;
    let cookie_path = read_c_string(buf, path_offset, size).unwrap_or_else(|| "/".to_string());
    let value = read_c_string(buf, value_offset, size).unwrap_or_default();

    if name.is_empty() {
        return None;
    }

    let domain = raw_url.as_deref().and_then(safe_hostname_from_url);

    let expires = if expiration > 0.0 {
        Some(expiration as i64 + MAC_EPOCH_DELTA_SECONDS)
    } else {
        None
    };

    let mut cookie = Cookie {
        name,
        value,
        domain: domain.map(|d| d.to_string()),
        path: Some(cookie_path),
        url: None,
        expires,
        secure: Some(is_secure),
        http_only: Some(is_http_only),
        same_site: None,
        source: Some(CookieSource {
            profile: None,
            origin: None,
            store_id: None,
        }),
    };

    // Safari doesn't have the domain field if we couldn't parse URL
    if cookie.domain.is_none() {
        cookie.domain = None;
    }

    Some(cookie)
}

#[cfg(any(target_os = "macos", test))]
fn read_double_le(buf: &[u8], offset: usize) -> f64 {
    if offset + 8 > buf.len() {
        return 0.0;
    }
    let bytes: [u8; 8] = buf[offset..offset + 8].try_into().unwrap();
    f64::from_le_bytes(bytes)
}

#[cfg(any(target_os = "macos", test))]
fn read_c_string(buf: &[u8], offset: usize, end: usize) -> Option<String> {
    if offset == 0 || offset >= end || offset >= buf.len() {
        return None;
    }
    let mut cursor = offset;
    while cursor < end && cursor < buf.len() && buf[cursor] != 0 {
        cursor += 1;
    }
    if cursor >= buf.len() {
        return None;
    }
    String::from_utf8(buf[offset..cursor].to_vec()).ok()
}

#[cfg(any(target_os = "macos", test))]
fn safe_hostname_from_url(raw: &str) -> Option<String> {
    let url_str = if raw.contains("://") {
        raw.to_string()
    } else {
        format!("https://{raw}")
    };
    match Url::parse(&url_str) {
        Ok(parsed) => {
            let host = parsed.host_str()?;
            Some(host.strip_prefix('.').unwrap_or(host).to_string())
        }
        Err(_) => {
            let cleaned = raw.trim();
            if cleaned.is_empty() {
                None
            } else {
                Some(cleaned.strip_prefix('.').unwrap_or(cleaned).to_string())
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decode_empty_buffer() {
        assert!(decode_binary_cookies(&[]).is_empty());
    }

    #[test]
    fn decode_wrong_magic() {
        assert!(decode_binary_cookies(b"nope1234").is_empty());
    }

    #[test]
    fn decode_synthetic_binary_cookies() {
        // Build a minimal valid binary cookies buffer
        let mut buf = Vec::new();

        // Magic: "cook"
        buf.extend_from_slice(b"cook");
        // Page count: 1 (BE)
        buf.extend_from_slice(&1u32.to_be_bytes());
        // Page size (will be calculated below)
        let page_size_offset = buf.len();
        buf.extend_from_slice(&0u32.to_be_bytes()); // placeholder

        // Build a page
        let mut page = Vec::new();
        // Page header: 0x00000100 (BE)
        page.extend_from_slice(&0x00000100u32.to_be_bytes());
        // Cookie count: 1 (LE)
        page.extend_from_slice(&1u32.to_le_bytes());
        // Cookie offset: 12 (LE) - after header(4) + count(4) + 1 offset(4)
        page.extend_from_slice(&12u32.to_le_bytes());

        // Build a cookie record at offset 12
        let mut cookie_buf = vec![0u8; 48]; // minimum size, will extend

        // Strings to embed after the 48-byte header
        let domain_str = b".example.com\0";
        let name_str = b"testcookie\0";
        let path_str = b"/\0";
        let value_str = b"testvalue\0";

        let strings_start = 48;
        let domain_offset = strings_start;
        let name_offset = domain_offset + domain_str.len();
        let path_offset = name_offset + name_str.len();
        let value_offset = path_offset + path_str.len();
        let total_size = value_offset + value_str.len();

        // Size (LE)
        cookie_buf[0..4].copy_from_slice(&(total_size as u32).to_le_bytes());
        // Flags: secure (1) | httpOnly (4) = 5
        cookie_buf[8..12].copy_from_slice(&5u32.to_le_bytes());
        // URL offset
        cookie_buf[16..20].copy_from_slice(&(domain_offset as u32).to_le_bytes());
        // Name offset
        cookie_buf[20..24].copy_from_slice(&(name_offset as u32).to_le_bytes());
        // Path offset
        cookie_buf[24..28].copy_from_slice(&(path_offset as u32).to_le_bytes());
        // Value offset
        cookie_buf[28..32].copy_from_slice(&(value_offset as u32).to_le_bytes());
        // Expiration (f64 LE at offset 40): Mac epoch for ~2030
        let expiry: f64 = 946_684_800.0; // well after 2001
        cookie_buf[40..48].copy_from_slice(&expiry.to_le_bytes());

        // Append strings
        cookie_buf.extend_from_slice(domain_str);
        cookie_buf.extend_from_slice(name_str);
        cookie_buf.extend_from_slice(path_str);
        cookie_buf.extend_from_slice(value_str);

        page.extend_from_slice(&cookie_buf);

        // Patch page size
        let page_size = page.len() as u32;
        buf[page_size_offset..page_size_offset + 4].copy_from_slice(&page_size.to_be_bytes());
        buf.extend_from_slice(&page);

        let cookies = decode_binary_cookies(&buf);
        assert_eq!(cookies.len(), 1);
        let c = &cookies[0];
        assert_eq!(c.name, "testcookie");
        assert_eq!(c.value, "testvalue");
        assert_eq!(c.domain.as_deref(), Some("example.com"));
        assert_eq!(c.path.as_deref(), Some("/"));
        assert_eq!(c.secure, Some(true));
        assert_eq!(c.http_only, Some(true));
        assert!(c.expires.is_some());
    }
}
