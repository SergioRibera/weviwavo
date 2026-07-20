use thiserror::Error;

#[derive(Debug, Error)]
pub enum Error {
    #[error("HTTP request failed: {0}")]
    Http(#[from] reqwest::Error),

    #[error("JSON (de)serialization failed: {0}")]
    Json(#[from] serde_json::Error),

    #[error("API returned non-OK playability status: {status} — {reason}")]
    NotPlayable { status: String, reason: String },

    #[error("no usable audio format found for video {video_id}")]
    NoAudioFormat { video_id: String },

    #[error("all clients exhausted for video {video_id}")]
    AllClientsFailed { video_id: String },

    #[error("response missing expected field: {field}")]
    MissingField { field: &'static str },

    #[error("authentication required but no cookie provided")]
    Unauthenticated,

    #[error("HTTP {status} from Innertube API")]
    HttpStatus { status: u16 },
}

pub type Result<T> = std::result::Result<T, Error>;
