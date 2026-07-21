use std::collections::HashMap;

use ytdroid::client::Locale;
use ytdroid::{AudioStream, ContentHints, YouTube};

use crate::audio::AudioQuality;

#[derive(Debug, thiserror::Error)]
pub enum AudioError {
    #[error("YouTube: {0}")]
    Yt(#[from] ytdroid::error::Error),
    #[error("HTTP: {0}")]
    Http(#[from] reqwest::Error),
    #[error("HTTP {status} fetching stream")]
    HttpStatus { status: u16 },
    #[error("no audio format for {quality:?}")]
    NoFormat { quality: AudioQuality },
    #[error("cipher sig decryption failed")]
    CipherDecryptFailed,
}

pub async fn fetch_audio_bytes(
    video_id: &str,
    quality: AudioQuality,
    cookies: Option<String>,
) -> Result<Vec<u8>, AudioError> {
    tracing::debug!(video_id, has_cookies = cookies.is_some(), "fetching audio");

    let yt = YouTube::new(cookies.as_deref(), Locale::default())?;

    // Generate PoToken for WEB clients — non-fatal on failure.
    // VISIONOS/VR clients succeed without it; WEB_REMIX uses it if available.
    let (po_player, po_streaming) = match servo_webview::potoken::generate(
        yt.visitor_id().unwrap_or(video_id),
        video_id,
    )
    .await
    {
        Ok(tok) => {
            tracing::debug!("PoToken generated");
            (Some(tok.player), Some(tok.streaming))
        }
        Err(e) => {
            tracing::warn!("PoToken generation failed ({e}), WEB clients will lack token");
            (None, None)
        }
    };

    let stream = yt
        .audio_stream(
            video_id,
            &ContentHints::default(),
            po_player.as_deref(),
            po_streaming.as_deref(),
        )
        .await?;

    let url = resolve_stream(stream).await?;
    tracing::debug!(%url, "fetching stream bytes");

    let http = reqwest::Client::builder()
        .gzip(true)
        .build()
        .map_err(AudioError::Http)?;

    let mut req = http
        .get(&url)
        .header("User-Agent", "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36");

    if let Some(cookie_str) = cookies.as_deref() {
        req = req.header("cookie", cookie_str);
    }

    let resp = req.send().await.map_err(AudioError::Http)?;
    let status = resp.status();
    if !status.is_success() {
        tracing::warn!(%status, "stream fetch bad status");
        return Err(AudioError::HttpStatus { status: status.as_u16() });
    }

    let bytes = resp.bytes().await.map_err(AudioError::Http)?;
    tracing::debug!(bytes = bytes.len(), "audio fetched");
    let _ = quality;
    Ok(bytes.to_vec())
}

/// Resolve an [`AudioStream`] to a final CDN URL.
///
/// - Direct URL: apply nsig transform, then append `pot=` if present.
/// - Cipher URL: decrypt sig → apply nsig → append `pot=` if present.
async fn resolve_stream(stream: AudioStream) -> Result<String, AudioError> {
    let url = if stream.is_cipher {
        let fake = format!("x:?{}", stream.data);
        let parsed = reqwest::Url::parse(&fake).ok();
        let params: HashMap<String, String> = parsed
            .as_ref()
            .map(|u| {
                u.query_pairs()
                    .map(|(k, v)| (k.into_owned(), v.into_owned()))
                    .collect()
            })
            .unwrap_or_default();

        let enc_sig = params.get("s").cloned().unwrap_or_default();
        let sig_param = params
            .get("sp")
            .cloned()
            .unwrap_or_else(|| "sig".to_string());
        let base_url = params.get("url").cloned().unwrap_or_default();

        tracing::debug!(sig_param, base_url_len = base_url.len(), "decrypting cipher sig");

        let decrypted_sig = super::nsig::decrypt_sig(&enc_sig)
            .await
            .ok_or(AudioError::CipherDecryptFailed)?;

        let url_with_sig = if base_url.contains('?') {
            format!("{base_url}&{sig_param}={decrypted_sig}")
        } else {
            format!("{base_url}?{sig_param}={decrypted_sig}")
        };

        super::nsig::decrypt_url(&url_with_sig).await
    } else {
        super::nsig::decrypt_url(&stream.data).await
    };

    Ok(match stream.streaming_pot {
        Some(pot) => append_pot(&url, &pot),
        None => url,
    })
}

fn append_pot(url: &str, pot: &str) -> String {
    if url.contains('?') {
        format!("{url}&pot={pot}")
    } else {
        format!("{url}?pot={pot}")
    }
}
