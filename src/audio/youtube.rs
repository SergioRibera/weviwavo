use std::collections::HashMap;

use ytdroid::client::{Locale, YouTubeClient};
use ytdroid::YouTube;

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

    // ── Try WEB_REMIX with PoToken first (primary, like Metrolist) ───────────
    let stream_url = try_web_remix(&yt, video_id).await.or_else(|e| {
        tracing::warn!("WEB_REMIX+PoToken failed ({e}), trying fallback clients");
        Err(e)
    });

    let stream_url = match stream_url {
        Ok(url) => url,
        Err(_) => {
            // Fallback: Android/VR/iOS clients (no PoToken needed)
            let (raw_data, is_cipher) = yt.audio_stream(video_id).await?;
            build_stream_url(raw_data, is_cipher, None).await?
        }
    };

    tracing::debug!(%stream_url, "fetching stream bytes");

    let http = reqwest::Client::builder()
        .gzip(true)
        .build()
        .map_err(AudioError::Http)?;

    let mut req = http
        .get(&stream_url)
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

/// Try WEB_REMIX with PoToken. Returns the final CDN URL (with pot= appended).
async fn try_web_remix(yt: &YouTube, video_id: &str) -> Result<String, AudioError> {
    let session_id = yt.visitor_id().unwrap_or(video_id);

    let tokens =
        servo_webview::potoken::generate(session_id, video_id)
            .await
            .map_err(|e| AudioError::Yt(ytdroid::error::Error::AllClientsFailed {
                video_id: format!("PoToken: {e}"),
            }))?;

    let resp = yt
        .player_raw(&YouTubeClient::WEB_REMIX, video_id, None, Some(&tokens.player))
        .await?;

    if resp.playability_status.status != "OK" {
        return Err(AudioError::Yt(ytdroid::error::Error::NotPlayable {
            status: resp.playability_status.status,
            reason: resp.playability_status.reason.unwrap_or_default(),
        }));
    }

    let sd = resp.streaming_data.as_ref();

    if let Some(fmt) = sd.and_then(|s| s.best_audio_format()) {
        let url = fmt.url.clone().ok_or(AudioError::CipherDecryptFailed)?;
        let pot_url = append_pot(&url, &tokens.streaming);
        return Ok(pot_url);
    }

    if let Some(fmt) = sd.and_then(|s| s.best_cipher_audio_format()) {
        let cipher = fmt.signature_cipher.clone().ok_or(AudioError::CipherDecryptFailed)?;
        let url = build_stream_url(cipher, true, Some(&tokens.streaming)).await?;
        return Ok(url);
    }

    Err(AudioError::Yt(ytdroid::error::Error::NoAudioFormat {
        video_id: video_id.to_owned(),
    }))
}

fn append_pot(url: &str, pot: &str) -> String {
    if url.contains('?') {
        format!("{url}&pot={pot}")
    } else {
        format!("{url}?pot={pot}")
    }
}

/// Resolve raw stream data to a final CDN URL, applying sig decryption + nsig + optional pot=.
async fn build_stream_url(
    raw_data: String,
    is_cipher: bool,
    pot: Option<&str>,
) -> Result<String, AudioError> {
    let url = if is_cipher {
        let fake = format!("x:?{raw_data}");
        let parsed = reqwest::Url::parse(&fake).ok();
        let params: HashMap<String, String> = parsed
            .as_ref()
            .map(|u| u.query_pairs().map(|(k, v)| (k.into_owned(), v.into_owned())).collect())
            .unwrap_or_default();

        let enc_sig = params.get("s").cloned().unwrap_or_default();
        let sig_param = params.get("sp").cloned().unwrap_or_else(|| "sig".to_string());
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
        super::nsig::decrypt_url(&raw_data).await
    };

    Ok(pot.map_or_else(|| url.clone(), |p| append_pot(&url, p)))
}
