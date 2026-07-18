use std::collections::HashMap;
use std::time::{SystemTime, UNIX_EPOCH};

const ORIGIN: &str = "https://music.youtube.com";

/// Parse a browser cookie header string into a name→value map.
pub(crate) fn parse_cookies(cookie_str: &str) -> HashMap<String, String> {
    cookie_str
        .split(';')
        .filter_map(|s| {
            let s = s.trim();
            let (k, v) = s.split_once('=')?;
            Some((k.trim().to_owned(), v.trim().to_owned()))
        })
        .collect()
}

/// Compute `SAPISIDHASH {ts}_{sha1}`.
///
/// Mirrors `InnerTube.kt`'s auth header computation exactly.
/// Returns `None` when neither SAPISID nor __Secure-3PAPISID is in the cookie map.
pub(crate) fn sapisidhash(cookies: &HashMap<String, String>) -> Option<String> {
    let sapisid = cookies
        .get("SAPISID")
        .or_else(|| cookies.get("__Secure-3PAPISID"))?;

    let ts = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .ok()?
        .as_secs();

    let payload = format!("{ts} {sapisid} {ORIGIN}");
    let hash = sha1_smol::Sha1::from(payload.as_bytes()).digest();
    Some(format!("SAPISIDHASH {ts}_{hash}"))
}
