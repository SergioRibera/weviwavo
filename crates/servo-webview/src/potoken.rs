//! PoToken generation via configurable external HTTP provider.
//!
//! Set `POTOKEN_PROVIDER_URL` to the provider endpoint URL (e.g.
//! `http://localhost:4416/get_pot`). Compatible with bgutil-ytdlp-pot-provider.
//!
//! When the env var is not set, [`generate`] returns [`Error::NotConfigured`]
//! and callers must fall back to non-WEB clients (VisionOS, android_vr, etc.).

use std::collections::HashMap;
use std::sync::OnceLock;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use serde::Deserialize;
use tokio::sync::Mutex;
use tracing::{debug, warn};

/// Token pair for the two contexts YouTube requires.
pub struct PoTokenPair {
    /// Token for `serviceIntegrityDimensions.poToken` in the `/player` request body.
    /// Content-bound to `video_id`.
    pub player: String,
    /// Token appended as `?pot=` to the CDN stream URL.
    /// Content-bound to `visitor_data` (or `video_id` as fallback).
    pub streaming: String,
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error(
        "POTOKEN_PROVIDER_URL not set — set it to enable PoToken \
         (e.g. http://localhost:4416/get_pot)"
    )]
    NotConfigured,
    #[error("HTTP request to PoToken provider failed: {0}")]
    Http(#[from] reqwest::Error),
    #[error("PoToken provider returned empty or unparseable response")]
    EmptyResponse,
}

// In-memory cache: content_binding → (token, expires_at_unix_secs)
static CACHE: OnceLock<Mutex<HashMap<String, (String, u64)>>> = OnceLock::new();

fn cache() -> &'static Mutex<HashMap<String, (String, u64)>> {
    CACHE.get_or_init(|| Mutex::new(HashMap::new()))
}

fn now_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

async fn get_cached(binding: &str) -> Option<String> {
    let guard = cache().lock().await;
    guard.get(binding).and_then(|(tok, exp)| {
        if now_secs() < *exp {
            Some(tok.clone())
        } else {
            None
        }
    })
}

async fn set_cached(binding: &str, token: String) {
    let mut guard = cache().lock().await;
    // 6-hour TTL (matches yt-dlp WebPO default cache spec).
    guard.insert(binding.to_owned(), (token, now_secs() + 6 * 3600));
}

#[derive(Deserialize)]
struct ProviderResponse {
    po_token: Option<String>,
    #[serde(rename = "poToken")]
    po_token_camel: Option<String>,
    token: Option<String>,
}

impl ProviderResponse {
    fn into_token(self) -> Option<String> {
        self.po_token
            .or(self.po_token_camel)
            .or(self.token)
            .filter(|s| !s.is_empty())
    }
}

async fn fetch_token(provider_url: &str, content_binding: &str) -> Result<String, Error> {
    if let Some(cached) = get_cached(content_binding).await {
        debug!("PoToken cache hit for binding");
        return Ok(cached);
    }

    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(15))
        .build()
        .map_err(Error::Http)?;

    debug!(provider_url, "requesting PoToken from provider");

    let resp = client
        .post(provider_url)
        .json(&serde_json::json!({ "content_binding": content_binding }))
        .send()
        .await
        .map_err(Error::Http)?;

    let status = resp.status();
    if !status.is_success() {
        warn!(%status, "PoToken provider HTTP error");
        return Err(Error::EmptyResponse);
    }

    let parsed: ProviderResponse = resp.json().await.map_err(|_| Error::EmptyResponse)?;
    let token = parsed.into_token().ok_or(Error::EmptyResponse)?;

    set_cached(content_binding, token.clone()).await;
    debug!("PoToken fetched and cached");
    Ok(token)
}

/// Generate a [`PoTokenPair`] for a YouTube video.
///
/// - `visitor_data` — the `X-Goog-Visitor-Id` value from YouTube API responses.
///   Used as content binding for the streaming (GVS) token.
///   Pass `video_id` as fallback if not available.
/// - `video_id` — the YouTube video ID. Used as content binding for the player token.
///
/// Returns [`Error::NotConfigured`] when `POTOKEN_PROVIDER_URL` is not set.
///
/// # Errors
///
/// Returns [`Error::NotConfigured`], [`Error::Http`], or [`Error::EmptyResponse`].
pub async fn generate(visitor_data: &str, video_id: &str) -> Result<PoTokenPair, Error> {
    let provider_url = std::env::var("POTOKEN_PROVIDER_URL")
        .ok()
        .filter(|s| !s.is_empty())
        .ok_or(Error::NotConfigured)?;

    let (streaming, player) = tokio::join!(
        fetch_token(&provider_url, visitor_data),
        fetch_token(&provider_url, video_id),
    );

    Ok(PoTokenPair {
        player: player?,
        streaming: streaming?,
    })
}
