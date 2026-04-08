use std::collections::HashSet;

use crate::types::{Cookie, GetCookiesResult};
use crate::util::base64::try_decode_base64_json;
use crate::util::host_match::host_matches_cookie_domain;
use url::Url;

pub struct InlineSource {
    pub source: String,
    pub payload: String,
}

pub async fn get_cookies_from_inline(
    inline: &InlineSource,
    origins: &[String],
    allowlist_names: Option<&HashSet<String>>,
) -> GetCookiesResult {
    let warnings = Vec::new();

    let raw_payload = if inline.source.ends_with("file")
        || inline.payload.ends_with(".json")
        || inline.payload.ends_with(".base64")
    {
        match tokio::fs::read_to_string(&inline.payload).await {
            Ok(content) => content,
            Err(_) => inline.payload.clone(),
        }
    } else {
        inline.payload.clone()
    };

    let decoded = try_decode_base64_json(&raw_payload).unwrap_or_else(|| raw_payload.clone());
    let parsed = match try_parse_cookie_payload(&decoded) {
        Some(cookies) => cookies,
        None => {
            return GetCookiesResult {
                cookies: vec![],
                warnings,
            }
        }
    };

    let host_allow: HashSet<String> = origins
        .iter()
        .filter_map(|o| {
            Url::parse(o)
                .ok()
                .and_then(|u| u.host_str().map(|h| h.to_string()))
        })
        .collect();

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
        let domain = cookie.domain.as_deref().map(|d| d.to_string()).or_else(|| {
            cookie
                .url
                .as_deref()
                .and_then(|u| Url::parse(u).ok())
                .and_then(|u| u.host_str().map(|h| h.to_string()))
        });
        if let Some(ref domain) = domain {
            if !host_allow.is_empty() && !matches_any_host(&host_allow, domain) {
                continue;
            }
        }
        cookies.push(cookie);
    }

    GetCookiesResult { cookies, warnings }
}

fn try_parse_cookie_payload(input: &str) -> Option<Vec<Cookie>> {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return None;
    }
    // Try as array
    if let Ok(cookies) = serde_json::from_str::<Vec<Cookie>>(trimmed) {
        return Some(cookies);
    }
    // Try as { cookies: [...] }
    #[derive(serde::Deserialize)]
    struct Wrapped {
        cookies: Vec<Cookie>,
    }
    if let Ok(wrapped) = serde_json::from_str::<Wrapped>(trimmed) {
        return Some(wrapped.cookies);
    }
    None
}

fn matches_any_host(hosts: &HashSet<String>, cookie_domain: &str) -> bool {
    hosts
        .iter()
        .any(|host| host_matches_cookie_domain(host, cookie_domain))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn parses_json_array() {
        let source = InlineSource {
            source: "inline-json".to_string(),
            payload: r#"[{"name":"foo","value":"bar","domain":"example.com"}]"#.to_string(),
        };
        let origins = vec!["https://example.com/".to_string()];
        let result = get_cookies_from_inline(&source, &origins, None).await;
        assert_eq!(result.cookies.len(), 1);
        assert_eq!(result.cookies[0].name, "foo");
        assert_eq!(result.cookies[0].value, "bar");
    }

    #[tokio::test]
    async fn parses_wrapped_object() {
        let source = InlineSource {
            source: "inline-json".to_string(),
            payload: r#"{"cookies":[{"name":"foo","value":"bar","domain":"example.com"}]}"#
                .to_string(),
        };
        let origins = vec!["https://example.com/".to_string()];
        let result = get_cookies_from_inline(&source, &origins, None).await;
        assert_eq!(result.cookies.len(), 1);
    }

    #[tokio::test]
    async fn filters_by_domain() {
        let source = InlineSource {
            source: "inline-json".to_string(),
            payload: r#"[{"name":"foo","value":"bar","domain":"other.com"}]"#.to_string(),
        };
        let origins = vec!["https://example.com/".to_string()];
        let result = get_cookies_from_inline(&source, &origins, None).await;
        assert_eq!(result.cookies.len(), 0);
    }

    #[tokio::test]
    async fn filters_by_name() {
        let source = InlineSource {
            source: "inline-json".to_string(),
            payload: r#"[{"name":"foo","value":"bar","domain":"example.com"},{"name":"baz","value":"qux","domain":"example.com"}]"#.to_string(),
        };
        let origins = vec!["https://example.com/".to_string()];
        let mut names = HashSet::new();
        names.insert("foo".to_string());
        let result = get_cookies_from_inline(&source, &origins, Some(&names)).await;
        assert_eq!(result.cookies.len(), 1);
        assert_eq!(result.cookies[0].name, "foo");
    }

    #[tokio::test]
    async fn base64_encoded_json() {
        use base64::Engine;
        let json = r#"[{"name":"foo","value":"bar","domain":"example.com"}]"#;
        let encoded = base64::engine::general_purpose::STANDARD.encode(json);
        let source = InlineSource {
            source: "inline-base64".to_string(),
            payload: encoded,
        };
        let origins = vec!["https://example.com/".to_string()];
        let result = get_cookies_from_inline(&source, &origins, None).await;
        assert_eq!(result.cookies.len(), 1);
    }
}
