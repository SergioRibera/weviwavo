//! Public domain types returned by all page parsers and the high-level [`crate::YouTube`] API.

/// The primary item type returned by search / browse / library pages.
#[derive(Debug, Clone, PartialEq)]
pub enum YTItem {
    Song(Box<SongItem>),
    Album(AlbumItem),
    Playlist(PlaylistItem),
    Artist(ArtistItem),
    Podcast(PodcastItem),
    Episode(EpisodeItem),
}

impl YTItem {
    #[must_use]
    pub fn as_song(&self) -> Option<&SongItem> {
        if let Self::Song(s) = self { Some(s) } else { None }
    }

    #[must_use]
    pub fn as_album(&self) -> Option<&AlbumItem> {
        if let Self::Album(a) = self { Some(a) } else { None }
    }

    #[must_use]
    pub fn as_playlist(&self) -> Option<&PlaylistItem> {
        if let Self::Playlist(p) = self { Some(p) } else { None }
    }

    #[must_use]
    pub fn as_artist(&self) -> Option<&ArtistItem> {
        if let Self::Artist(a) = self { Some(a) } else { None }
    }

    #[must_use]
    pub fn as_podcast(&self) -> Option<&PodcastItem> {
        if let Self::Podcast(p) = self { Some(p) } else { None }
    }

    #[must_use]
    pub fn as_episode(&self) -> Option<&EpisodeItem> {
        if let Self::Episode(e) = self { Some(e) } else { None }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct SongItem {
    /// `YouTube` video ID.
    pub id: String,
    pub title: String,
    pub artists: Vec<Artist>,
    pub album: Option<AlbumRef>,
    /// Duration in seconds.
    pub duration: Option<u32>,
    pub thumbnail: Option<String>,
    pub explicit: bool,
    /// `setVideoId` for playlist context — needed to remove songs from playlists.
    pub set_video_id: Option<String>,
    pub like_status: Option<LikeStatus>,
    pub library_add_token: Option<String>,
    pub library_remove_token: Option<String>,
    pub music_video_type: Option<String>,
}

impl SongItem {
    /// True when this song has a real music video rather than an ATV-only track.
    #[must_use]
    pub fn is_video_song(&self) -> bool {
        self.music_video_type
            .as_deref()
            .is_some_and(|t| t != "MUSIC_VIDEO_TYPE_ATV")
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct AlbumItem {
    /// Browse ID (e.g. `"MPREb_..."`).
    pub id: String,
    /// Playlist ID for the album's auto-playlist (if available).
    pub playlist_id: Option<String>,
    pub title: String,
    pub artists: Vec<Artist>,
    /// Release year as a string (e.g. `"2024"`).
    pub year: Option<String>,
    pub thumbnail: Option<String>,
    pub explicit: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct PlaylistItem {
    /// Browse ID — always starts with `"VL"`.
    pub id: String,
    pub title: String,
    pub author: Option<String>,
    pub song_count_text: Option<String>,
    pub thumbnail: Option<String>,
    pub library_add_token: Option<String>,
    pub library_remove_token: Option<String>,
}

impl PlaylistItem {
    /// Raw playlist ID with the leading `"VL"` stripped.
    #[must_use]
    pub fn playlist_id(&self) -> &str {
        self.id.strip_prefix("VL").unwrap_or(&self.id)
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct ArtistItem {
    /// Browse ID for the artist's channel.
    pub id: String,
    pub title: String,
    pub subscribers: Option<String>,
    pub thumbnail: Option<String>,
    /// Channel IDs required for the subscribe endpoint (may be empty).
    pub channel_ids: Vec<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct PodcastItem {
    /// Browse ID for the podcast show page.
    pub id: String,
    pub title: String,
    pub author: Option<String>,
    pub thumbnail: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct EpisodeItem {
    /// `YouTube` video ID for the episode.
    pub id: String,
    pub title: String,
    pub podcast: Option<PodcastRef>,
    pub date: Option<String>,
    /// Duration in seconds.
    pub duration: Option<u32>,
    pub thumbnail: Option<String>,
    pub explicit: bool,
    pub library_add_token: Option<String>,
    pub library_remove_token: Option<String>,
}

/// Minimal artist reference used inside [`SongItem`] / [`AlbumItem`].
#[derive(Debug, Clone, PartialEq)]
pub struct Artist {
    pub name: String,
    /// Browse ID — absent when the artist doesn't have a channel page.
    pub id: Option<String>,
}

/// Minimal album reference used inside [`SongItem`].
#[derive(Debug, Clone, PartialEq)]
pub struct AlbumRef {
    pub name: String,
    pub id: String,
}

/// Minimal podcast reference used inside [`EpisodeItem`].
#[derive(Debug, Clone, PartialEq)]
pub struct PodcastRef {
    pub name: String,
    pub id: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LikeStatus {
    Like,
    Indifferent,
    Dislike,
}

/// Watch endpoint for playing a single video.
#[derive(Debug, Clone, PartialEq)]
pub struct WatchEndpoint {
    pub video_id: String,
    pub playlist_id: Option<String>,
    pub params: Option<String>,
    pub playlist_set_video_id: Option<String>,
}

/// Watch endpoint for playing a playlist / radio.
#[derive(Debug, Clone, PartialEq)]
pub struct WatchPlaylistEndpoint {
    pub playlist_id: String,
    pub params: Option<String>,
}
