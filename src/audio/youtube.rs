use std::borrow::Cow;

use serde_json::Value;
use ytmapi_rs::auth::BrowserToken;
use ytmapi_rs::parse::ParseFrom;
use ytmapi_rs::query::{PostMethod, PostQuery, Query};
use ytmapi_rs::{YtMusic, parse::ProcessedResult};

use crate::audio::AudioQuality;

struct PlayerQuery {
    video_id: String,
}

#[derive(Debug)]
struct RawJson(String);

impl ParseFrom<PlayerQuery> for RawJson {
    fn parse_from(p: ProcessedResult<PlayerQuery>) -> ytmapi_rs::Result<Self> {
        Ok(RawJson(p.source))
    }
}

impl<A: ytmapi_rs::auth::AuthToken> Query<A> for PlayerQuery {
    type Output = RawJson;
    type Method = PostMethod;
}

impl PostQuery for PlayerQuery {
    fn header(&self) -> serde_json::Map<String, Value> {
        serde_json::Map::from_iter([
            ("videoId".to_string(), Value::String(self.video_id.clone())),
            (
                "playbackContext".to_string(),
                serde_json::json!({
                    "contentPlaybackContext": {
                        "signatureTimestamp": 0
                    }
                }),
            ),
        ])
    }

    fn params(&self) -> Vec<(&str, Cow<'_, str>)> {
        vec![]
    }

    fn path(&self) -> &str {
        "player"
    }
}

#[derive(Debug, thiserror::Error)]
pub enum AudioError {
    #[error("no YouTube session available")]
    NoSession,
    #[error("YouTube API: {0}")]
    Api(#[from] ytmapi_rs::Error),
    #[error("HTTP request: {0}")]
    Http(#[from] reqwest::Error),
    #[error("no audio format found for quality {quality:?}")]
    NoFormat { quality: AudioQuality },
    #[error("audio decode: {0}")]
    Decode(#[from] rodio::decoder::DecoderError),
    #[error("JSON parse: {0}")]
    Json(#[from] serde_json::Error),
}

pub async fn fetch_audio_bytes(
    yt: &YtMusic<BrowserToken>,
    video_id: &str,
    quality: AudioQuality,
) -> Result<Vec<u8>, AudioError> {
    let query = PlayerQuery {
        video_id: video_id.to_string(),
    };
    let raw = yt.raw_json_query::<PlayerQuery>(query).await?;
    let json: Value = serde_json::from_str(&raw)?;

    let formats = json
        .pointer("/streamingData/adaptiveFormats")
        .and_then(|v| v.as_array())
        .ok_or(AudioError::NoFormat { quality })?;

    // Keep audio-only, non-webm formats (AAC in MP4; Symphonia has no Opus decoder).
    let mut audio_formats: Vec<&Value> = formats
        .iter()
        .filter(|f| {
            let mime = f.get("mimeType").and_then(|m| m.as_str()).unwrap_or("");
            f.get("audioQuality").is_some()
                && f.get("width").is_none()
                && (mime.contains("audio/mp4") || mime.contains("audio/m4a"))
        })
        .collect();

    if audio_formats.is_empty() {
        return Err(AudioError::NoFormat { quality });
    }

    audio_formats.sort_by_key(|f| f.get("bitrate").and_then(|b| b.as_u64()).unwrap_or(0));

    let format = match quality {
        AudioQuality::Low => audio_formats.first(),
        AudioQuality::Medium => audio_formats.get(audio_formats.len() / 2),
        AudioQuality::High => audio_formats.last(),
    }
    .ok_or(AudioError::NoFormat { quality })?;

    let url = format
        .get("url")
        .and_then(|u| u.as_str())
        .ok_or(AudioError::NoFormat { quality })?;

    let client = reqwest::Client::new();
    let bytes = client.get(url).send().await?.bytes().await?;
    Ok(bytes.to_vec())
}
