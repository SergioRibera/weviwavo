mod engine;
pub(crate) mod nsig;
mod streaming;
mod youtube;

pub use engine::{AudioCommand, AudioEngine, run_audio_engine};
pub(crate) use streaming::AudioRaw;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum AudioQuality {
    Low,
    #[default]
    Medium,
    High,
}
