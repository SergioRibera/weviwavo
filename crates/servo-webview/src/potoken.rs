//! PoToken generation stub.
//!
//! BotGuard's minting functions (`webPoSignalOutput`) require V8 (Chromium).
//! On Linux desktop there is no embedded V8; QuickJS and SpiderMonkey both
//! produce an empty `webPoSignalOutput` and cannot mint tokens.
//!
//! Callers should fall through to non-WEB clients (TV_EMBEDDED, iOS,
//! ANDROID_TESTSUITE, etc.) which work without PoToken — same approach used
//! by yt-dlp and other extractors on desktop.

/// Token pair that would be returned by a working PoToken implementation.
pub struct PoTokenPair {
    pub player: String,
    pub streaming: String,
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("PoToken requires V8 (Chromium); use non-WEB fallback clients instead")]
    NotSupported,
}

/// Always fails immediately. Callers must handle the error by trying
/// non-WEB clients (TV_EMBEDDED, iOS, ANDROID_TESTSUITE, …).
pub async fn generate(_session_id: &str, _video_id: &str) -> Result<PoTokenPair, Error> {
    Err(Error::NotSupported)
}
