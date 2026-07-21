use chacha20poly1305::aead::{Aead, KeyInit};
use chacha20poly1305::{ChaCha20Poly1305, Nonce};
use ytdroid::{YouTube, client::Locale};

fn load_saved_cookies() -> Option<String> {
    let base = dirs::data_dir()?.join("weviwavo");
    let key_bytes = std::fs::read(base.join(".keyfile")).ok()?;
    if key_bytes.len() != 32 { return None; }
    let ct = std::fs::read(base.join("cookies.enc")).ok()?;
    if ct.len() < 12 { return None; }
    let (nonce_bytes, ciphertext) = ct.split_at(12);
    let cipher = ChaCha20Poly1305::new(key_bytes.as_slice().into());
    let nonce = Nonce::from_slice(nonce_bytes);
    let plain = cipher.decrypt(nonce, ciphertext).ok()?;
    String::from_utf8(plain).ok()
}

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(
            std::env::var("RUST_LOG")
                .unwrap_or_else(|_| "ytdroid=warn,weviwavo=debug".to_string())
                .as_str(),
        )
        .init();

    let video_id = std::env::args().nth(1).unwrap_or_else(|| "KcmwR2j_Zns".to_string());
    let cookies = std::env::args().nth(2).or_else(load_saved_cookies);

    eprintln!("video_id={video_id}  logged_in={}", if cookies.is_some() { "YES" } else { "NO" });

    let yt = YouTube::new(cookies.as_deref(), Locale::default()).expect("client build failed");

    match yt.audio_stream(&video_id).await {
        Ok((data, is_cipher)) => {
            eprintln!("audio_stream: is_cipher={is_cipher}  data_len={}", data.len());

            let cookie_clone = cookies.clone();
            let result = fetch_audio_preview(&data, is_cipher, cookie_clone.as_deref()).await;
            match result {
                Ok(bytes) => eprintln!("SUCCESS: fetched {} bytes", bytes),
                Err(e) => {
                    eprintln!("FAIL: {e}");
                    std::process::exit(1);
                }
            }
        }
        Err(e) => {
            eprintln!("FAIL audio_stream: {e}");
            std::process::exit(1);
        }
    }
}

async fn fetch_audio_preview(data: &str, is_cipher: bool, cookies: Option<&str>) -> Result<usize, String> {
    use std::collections::HashMap;

    let stream_url = if is_cipher {
        let fake = format!("x:?{data}");
        let parsed = reqwest::Url::parse(&fake).map_err(|e| format!("url parse: {e}"))?;
        let params: HashMap<String, String> = parsed.query_pairs()
            .map(|(k, v)| (k.into_owned(), v.into_owned()))
            .collect();

        let enc_sig = params.get("s").cloned().unwrap_or_default();
        let sig_param = params.get("sp").cloned().unwrap_or_else(|| "sig".to_string());
        let base_url = params.get("url").cloned().unwrap_or_default();

        if base_url.is_empty() {
            return Err("cipher: missing url param".to_string());
        }

        eprintln!("Decrypting sig (len={})...", enc_sig.len());
        let decrypted_sig = weviwavo::decrypt_sig(&enc_sig).await
            .ok_or_else(|| "decrypt_sig returned None — check /tmp/yt-player-debug.js".to_string())?;
        eprintln!("Sig decrypted (len={}).", decrypted_sig.len());

        let url_with_sig = if base_url.contains('?') {
            format!("{base_url}&{sig_param}={decrypted_sig}")
        } else {
            format!("{base_url}?{sig_param}={decrypted_sig}")
        };

        weviwavo::decrypt_url(&url_with_sig).await
    } else {
        weviwavo::decrypt_url(data).await
    };

    eprintln!("Fetching stream URL:\n{}", &stream_url);

    let http = reqwest::Client::new();
    let mut req = http.get(&stream_url)
        .header("User-Agent", "Mozilla/5.0 (ChromiumStylePlatform) Cobalt/25.lts.30.1034943-gold (unlike Gecko), Unknown_TV_Unknown_0/Unknown (Unknown, Unknown)")
        .header("Origin", "https://www.youtube.com")
        .header("Referer", "https://www.youtube.com/")
        .header("Range", "bytes=0-65535");
    if let Some(c) = cookies {
        req = req.header("cookie", c);
    }
    let resp = req.send().await.map_err(|e| format!("fetch: {e}"))?;
    let status = resp.status();
    eprintln!("HTTP status: {status}");
    if !status.is_success() && status.as_u16() != 206 {
        let body = resp.text().await.unwrap_or_default();
        eprintln!("Response body (first 500): {}", &body[..body.len().min(500)]);
        return Err(format!("HTTP {status}"));
    }
    let bytes = resp.bytes().await.map_err(|e| format!("read: {e}"))?;
    Ok(bytes.len())
}
