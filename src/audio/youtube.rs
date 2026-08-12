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
) -> Result<crate::audio::AudioRaw, AudioError> {
    tracing::debug!(video_id, has_cookies = cookies.is_some(), "fetching audio");

    let yt_base = YouTube::new(cookies.as_deref(), Locale::default())?;
    let yt = match crate::visitor_id::load() {
        Some(vid) => yt_base.with_visitor_id(vid),
        None => yt_base,
    };

    // Generate the player PoToken upfront (content-bound to video_id).
    // Non-fatal: VisionOS/VR clients succeed without it.
    let po_player = servo_webview::potoken::generate(video_id, video_id)
        .await
        .ok()
        .map(|p| p.player);

    if po_player.is_some() {
        tracing::debug!("player PoToken generated");
    }

    let sig_ts = super::nsig::signature_timestamp().await;

    // First pass: get the stream. streaming_pot is None here — we fill it
    // below after the player request has captured visitor_data from response headers.
    let mut stream = yt
        .audio_stream(
            video_id,
            &ContentHints::default(),
            po_player.as_deref(),
            None,
            Some(sig_ts),
        )
        .await?;

    // If the selected client requires a GVS PoToken (WEB clients), generate it
    // now. visitor_data is available because the /player request above captured
    // it from the X-Goog-Visitor-Id response header.
    if stream.needs_gvs_pot {
        let visitor_data = yt.visitor_data().unwrap_or_else(|| video_id.to_owned());
        match servo_webview::potoken::generate(&visitor_data, video_id).await {
            Ok(pair) => {
                tracing::debug!("streaming PoToken generated");
                stream.streaming_pot = Some(pair.streaming);
            }
            Err(e) => {
                tracing::warn!("streaming PoToken failed ({e}), stream may 403");
            }
        }
    }

    let url = resolve_stream(stream).await?;
    tracing::debug!(%url, "fetching stream bytes");

    let yt_report = yt.clone();
    let vid_report = video_id.to_owned();
    tokio::spawn(async move {
        if let Err(e) = yt_report.report_history(&vid_report).await {
            tracing::warn!(error = %e, "history report failed");
        }
    });

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
    let content_type = resp
        .headers()
        .get("content-type")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("unknown")
        .to_owned();
    if !status.is_success() {
        tracing::warn!(%status, content_type, "stream fetch bad status");
        return Err(AudioError::HttpStatus { status: status.as_u16() });
    }
    tracing::debug!(%status, content_type, "stream response headers");

    let content_length = resp.content_length();
    let (reader, writer) = crate::audio::streaming::progressive_channel(content_length);

    tokio::spawn(async move {
        let mut resp = resp;
        loop {
            match resp.chunk().await {
                Ok(Some(chunk)) => writer.push(&chunk),
                Ok(None) => break,
                Err(e) => {
                    tracing::warn!(error = %e, "stream chunk error");
                    break;
                }
            }
        }
        writer.finish();
    });

    let _ = quality;
    Ok(Box::new(reader))
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
