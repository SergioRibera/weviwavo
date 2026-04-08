use serde::{Deserialize, Serialize};
use std::collections::HashSet;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CookieSameSite {
    Strict,
    Lax,
    None,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CookieSource {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub profile: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub origin: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub store_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Cookie {
    pub name: String,
    pub value: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub domain: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expires: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub secure: Option<bool>,
    #[serde(rename = "httpOnly", skip_serializing_if = "Option::is_none")]
    pub http_only: Option<bool>,
    #[serde(rename = "sameSite", skip_serializing_if = "Option::is_none")]
    pub same_site: Option<CookieSameSite>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source: Option<CookieSource>,
}

#[derive(Debug, Clone)]
pub struct GetCookiesOptions {
    pub url: String,
    pub origins: Option<Vec<String>>,
    pub names: Option<Vec<String>>,
    pub profile: Option<String>,
    pub chrome_profile: Option<String>,
    pub edge_profile: Option<String>,
    pub firefox_profile: Option<String>,
    pub safari_cookies_file: Option<String>,
    pub include_expired: Option<bool>,
    pub timeout_ms: Option<u64>,
    pub debug: Option<bool>,
    pub inline_cookies_file: Option<String>,
    pub inline_cookies_json: Option<String>,
    pub inline_cookies_base64: Option<String>,
}

impl GetCookiesOptions {
    pub fn new(url: impl Into<String>) -> Self {
        Self {
            url: url.into(),
            origins: None,
            names: None,
            profile: None,
            chrome_profile: None,
            edge_profile: None,
            firefox_profile: None,
            safari_cookies_file: None,
            include_expired: None,
            timeout_ms: None,
            debug: None,
            inline_cookies_file: None,
            inline_cookies_json: None,
            inline_cookies_base64: None,
        }
    }

    pub fn origins(mut self, origins: Vec<String>) -> Self {
        self.origins = Some(origins);
        self
    }

    pub fn names(mut self, names: Vec<String>) -> Self {
        self.names = Some(names);
        self
    }

    pub fn chrome_profile(mut self, profile: impl Into<String>) -> Self {
        self.chrome_profile = Some(profile.into());
        self
    }

    pub fn edge_profile(mut self, profile: impl Into<String>) -> Self {
        self.edge_profile = Some(profile.into());
        self
    }

    pub fn firefox_profile(mut self, profile: impl Into<String>) -> Self {
        self.firefox_profile = Some(profile.into());
        self
    }

    pub fn safari_cookies_file(mut self, file: impl Into<String>) -> Self {
        self.safari_cookies_file = Some(file.into());
        self
    }

    pub fn include_expired(mut self, include: bool) -> Self {
        self.include_expired = Some(include);
        self
    }

    pub fn timeout_ms(mut self, ms: u64) -> Self {
        self.timeout_ms = Some(ms);
        self
    }

    pub fn debug(mut self, debug: bool) -> Self {
        self.debug = Some(debug);
        self
    }

    pub fn inline_cookies_file(mut self, file: impl Into<String>) -> Self {
        self.inline_cookies_file = Some(file.into());
        self
    }

    pub fn inline_cookies_json(mut self, json: impl Into<String>) -> Self {
        self.inline_cookies_json = Some(json.into());
        self
    }

    pub fn inline_cookies_base64(mut self, b64: impl Into<String>) -> Self {
        self.inline_cookies_base64 = Some(b64.into());
        self
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct GetCookiesResult {
    pub cookies: Vec<Cookie>,
    pub warnings: Vec<String>,
}

pub(crate) fn normalize_names(names: &Option<Vec<String>>) -> Option<HashSet<String>> {
    let names = names.as_ref()?;
    let cleaned: HashSet<String> = names
        .iter()
        .map(|n| n.trim().to_string())
        .filter(|n| !n.is_empty())
        .collect();
    if cleaned.is_empty() {
        return None;
    }
    Some(cleaned)
}
