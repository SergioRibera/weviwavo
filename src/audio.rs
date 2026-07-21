mod engine;
pub(crate) mod nsig;
mod youtube;

pub use engine::{AudioCommand, AudioEngine, run_audio_engine};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum AudioQuality {
    Low,
    #[default]
    Medium,
    High,
}
