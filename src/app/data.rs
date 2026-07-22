use freya::radio::RadioChannel;
use tokio::sync::mpsc;
use ytdroid::models::{PlaylistItem, SongItem};
use ytdroid::pages::home::HomePage;
use ytdroid::YouTube;

use crate::audio::AudioCommand;

/// A song entry in the playback queue.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct QueueSong {
    pub video_id: String,
    pub title: String,
    pub artist: String,
    pub album: String,
    pub thumbnail_url: String,
}

/// Commands for the background data-loading task.
#[derive(Debug)]
pub enum NavCommand {
    /// Set `is_loading = true` and `pending_playlist_id` before the fetch begins.
    BeginNavigation { playlist_id: String },
    /// Fetch playlist metadata + songs + suggestions and write them to `Data::playlist_view`.
    LoadPlaylist(String),
    /// Fetch the first song of a playlist and start playback immediately.
    PlayPlaylist(String),
    /// Clear `pending_playlist_id` after the route push has been dispatched.
    ClearPending,
}

#[derive(Clone)]
pub struct PlaylistViewData {
    pub playlist: PlaylistItem,
    pub songs: Vec<SongItem>,
    pub suggestions: Vec<SongItem>,
    pub related_playlists: Vec<PlaylistItem>,
}

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
    pub queue: Vec<QueueSong>,
    pub queue_index: usize,
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
            queue: Vec::new(),
            queue_index: 0,
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
    pub nav_cmd: Option<mpsc::Sender<NavCommand>>,
    pub playlist_view: Option<PlaylistViewData>,
    /// True while a playlist fetch is in progress.
    pub is_loading: bool,
    /// Browse ID of the playlist navigation that's pending data.
    pub pending_playlist_id: Option<String>,
}

#[derive(Default, PartialEq, Eq, Clone, Debug, Copy, Hash, PartialOrd, Ord)]
pub enum DataChannel {
    #[default]
    YtApi,
    Feed,
    Player,
    /// Owns `nav_cmd`, `playlist_view`, `is_loading`, `pending_playlist_id`.
    Navigation,
}

impl RadioChannel<Data> for DataChannel {}
