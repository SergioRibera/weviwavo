use freya::radio::RadioChannel;
use tokio::sync::mpsc;
use ytdroid::pages::home::HomePage;
use ytdroid::YouTube;

use crate::audio::AudioCommand;

#[derive(Clone)]
pub struct PlayerState {
    pub is_playing: bool,
    pub current_secs: f32,
    pub total_secs: f32,
    pub title: String,
    pub artist: String,
    pub album: String,
    pub year: Option<String>,
    pub thumbnail_url: String,
    pub volume: f32,
}

impl Default for PlayerState {
    fn default() -> Self {
        Self {
            is_playing: false,
            current_secs: 0.0,
            total_secs: 0.0,
            title: String::new(),
            artist: String::new(),
            album: String::new(),
            year: None,
            thumbnail_url: String::new(),
            volume: 1.0,
        }
    }
}

#[derive(Default, Clone)]
pub struct Data {
    pub yt_session: Option<YouTube>,
    /// Raw `Cookie:` header preserved for the audio engine's authenticated requests.
    pub cookie_header: Option<String>,
    pub feed: HomePage,
    pub player: PlayerState,
    pub audio_cmd: Option<mpsc::Sender<AudioCommand>>,
}

#[derive(Default, PartialEq, Eq, Clone, Debug, Copy, Hash, PartialOrd, Ord)]
pub enum DataChannel {
    #[default]
    YtApi,
    Feed,
    Player,
}

impl RadioChannel<Data> for DataChannel {}
