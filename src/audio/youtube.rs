use serde_json::{json, Value};

use crate::audio::AudioQuality;

const ENDPOINT: &str = "https://music.youtube.com/youtubei/v1/player";
const ORIGIN: &str = "https://music.youtube.com";

#[derive(Debug, thiserror::Error)]
pub enum AudioError {
    #[error("YouTube: {0}")]
    Yt(String),
    #[error("HTTP: {0}")]
    Http(#[from] reqwest::Error),
    #[error("HTTP {status} fetching stream")]
    HttpStatus { status: u16 },
    #[error("no audio format for {quality:?}")]
    NoFormat { quality: AudioQuality },
}

struct Client {
    name: &'static str,
    version: &'static str,
    /// Numeric client ID for X-YouTube-Client-Name header
    id: &'static str,
    user_agent: &'static str,
    /// Whether to send cookie + SAPISIDHASH Authorization
    login: bool,
/// Extra fields merged into context.client
    context_extra: fn() -> Value,
}

const CLIENTS: &[Client] = &[
    // ANDROID (MOBILE) — loginSupported=true, useSignatureTimestamp=true, needsNTransform=false
    Client {
        name: "ANDROID",
        version: "21.03.38",
        id: "3",
        user_agent: "com.google.android.youtube/21.03.38 (Linux; U; Android 14) gzip",
        login: true,
        context_extra: || json!({}),
    },
    // VISIONOS — loginSupported=false, still pass cookie in case it helps
    Client {
        name: "VISIONOS",
        version: "0.1",
        id: "101",
        user_agent: "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/18.0 Safari/605.1.15",
        login: false,
        context_extra: || json!({}),
    },
    // ANDROID_VR 1.65.10
    Client {
        name: "ANDROID_VR",
        version: "1.65.10",
        id: "28",
        user_agent: "com.google.android.apps.youtube.vr.oculus/1.65.10 (Linux; U; Android 12L; eureka-user Build/SQ3A.220605.009.A1) gzip",
        login: false,
        context_extra: || json!({
            "userAgent": "com.google.android.apps.youtube.vr.oculus/1.65.10 (Linux; U; Android 12L; eureka-user Build/SQ3A.220605.009.A1) gzip",
            "osName": "Android",
            "osVersion": "12L",
            "deviceMake": "Oculus",
            "deviceModel": "Quest 3",
            "androidSdkVersion": "32"
        }),
    },
];

pub async fn fetch_audio_bytes(
    video_id: &str,
    _quality: AudioQuality,
    cookies: Option<String>,
) -> Result<Vec<u8>, AudioError> {
    tracing::debug!(video_id, has_cookies = cookies.is_some(), "fetching audio");

    let http = reqwest::Client::builder().build().map_err(AudioError::Http)?;

    for client in CLIENTS {
        // Build context.client
        let mut ctx_client = json!({
            "clientName": client.name,
            "clientVersion": client.version,
            "gl": "US",
            "hl": "en"
        });
        let extra = (client.context_extra)();
        if let (Some(base), Some(ext)) = (ctx_client.as_object_mut(), extra.as_object()) {
            for (k, v) in ext {
                base.insert(k.clone(), v.clone());
            }
        }

        // Nulls omitted intentionally — Metrolist uses explicitNulls=false
        let body = json!({
            "context": {
                "client": ctx_client,
                "user": {}
            },
            "videoId": video_id,
            "contentCheckOk": true,
            "racyCheckOk": true
        });

        let mut req = http
            .post(ENDPOINT)
            .query(&[("prettyPrint", "false")])
            .header("Content-Type", "application/json")
            .header("X-Goog-Api-Format-Version", "1")
            .header("X-YouTube-Client-Name", client.id)
            .header("X-YouTube-Client-Version", client.version)
            .header("X-Origin", ORIGIN)
            .header("Referer", format!("{ORIGIN}/"))
            .header("User-Agent", client.user_agent)
            .json(&body);

        // Auth: cookie + SAPISIDHASH for loginSupported clients
        if client.login {
            if let Some(cookie_str) = cookies.as_deref() {
                req = req.header("cookie", cookie_str);
                if let Some(auth) = sapisidhash(cookie_str) {
                    req = req.header("Authorization", auth);
                }
            }
        }

        let resp = match req.send().await {
            Ok(r) => r,
            Err(e) => {
                tracing::warn!(client = client.name, error = %e, "player request failed");
                continue;
            }
        };

        if !resp.status().is_success() {
            tracing::warn!(client = client.name, status = %resp.status(), "player bad status");
            continue;
        }

        let data: Value = match resp.json().await {
            Ok(v) => v,
            Err(e) => {
                tracing::warn!(client = client.name, error = %e, "player json parse failed");
                continue;
            }
        };

        let playability = data["playabilityStatus"]["status"].as_str().unwrap_or("");
        if playability != "OK" {
            let reason = data["playabilityStatus"]["reason"].as_str().unwrap_or("?");
            tracing::warn!(client = client.name, playability, reason, "not playable");
            continue;
        }

        let formats = data["streamingData"]["adaptiveFormats"]
            .as_array()
            .cloned()
            .unwrap_or_default();

        tracing::debug!(client = client.name, formats = formats.len(), "adaptive formats");

        // isAudio = width == null (Metrolist's Format.isAudio)
        // No n-transform needed for ANDROID/VISIONOS/ANDROID_VR
        let best = formats
            .iter()
            .filter(|f| f["width"].is_null())
            .filter(|f| f["url"].as_str().map(|u| !u.is_empty()).unwrap_or(false))
            .max_by_key(|f| f["bitrate"].as_u64().unwrap_or(0));

        let Some(fmt) = best else {
            tracing::warn!(client = client.name, "no usable audio format");
            continue;
        };

        let stream_url = fmt["url"].as_str().unwrap();
        let mime = fmt["mimeType"].as_str().unwrap_or("?");
        let bitrate = fmt["bitrate"].as_u64().unwrap_or(0);
        tracing::debug!(%mime, bitrate, "fetching stream");

        let mut stream_req = http
            .get(stream_url)
            .header("User-Agent", "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36");

        // Pass cookie for stream fetch too (needed for some content)
        if let Some(cookie_str) = cookies.as_deref() {
            stream_req = stream_req.header("cookie", cookie_str);
        }

        let stream_resp = match stream_req.send().await {
            Ok(r) => r,
            Err(e) => {
                tracing::warn!(error = %e, "stream fetch failed");
                continue;
            }
        };

        let status = stream_resp.status();
        if !status.is_success() {
            tracing::warn!(%status, "stream bad status");
            continue;
        }

        let bytes = stream_resp.bytes().await?;
        tracing::debug!(bytes = bytes.len(), "audio fetched");
        return Ok(bytes.to_vec());
    }

    Err(AudioError::NoFormat { quality: _quality })
}

/// Compute `SAPISIDHASH {ts}_{sha1}` from browser cookie string.
/// Mirrors Metrolist's InnerTube.kt auth header computation.
fn sapisidhash(cookies: &str) -> Option<String> {
    let sapisid = cookies
        .split(';')
        .map(|s| s.trim())
        .find(|s| s.starts_with("SAPISID=") || s.starts_with("__Secure-3PAPISID="))
        .and_then(|s| s.splitn(2, '=').nth(1))?;

    let ts = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .ok()?
        .as_secs();

    let data = format!("{ts} {sapisid} {ORIGIN}");
    let hash = sha1_smol::Sha1::from(data.as_bytes()).digest();
    Some(format!("SAPISIDHASH {ts}_{}", hash))
}
