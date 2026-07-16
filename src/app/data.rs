use freya::radio::RadioChannel;
use tokio::sync::mpsc;
use ytmapi_rs::YtMusic;
use ytmapi_rs::auth::BrowserToken;
use ytmapi_rs::parse::HomeSections;

use crate::audio::AudioCommand;

#[derive(Default, Clone)]
pub struct PlayerState {
    pub is_playing: bool,
    pub current_secs: f32,
    pub total_secs: f32,
    pub title: String,
    pub artist: String,
    pub album: String,
    pub year: Option<String>,
    pub thumbnail_url: String,
}

#[derive(Default, Clone)]
pub struct Data {
    pub yt_session: Option<YtMusic<BrowserToken>>,
    pub feed: HomeSections,
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
