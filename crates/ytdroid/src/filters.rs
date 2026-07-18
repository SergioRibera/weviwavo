/// Typed wrapper for a search-filter `params` value (base64-encoded protobuf).
///
/// Pass to [`crate::YouTube::search`] to restrict results to a single content type.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SearchFilter(pub &'static str);

impl SearchFilter {
    pub const SONGS: Self = Self("EgWKAQIIAWoKEAkQBRAKEAMQBA%3D%3D");
    pub const VIDEOS: Self = Self("EgWKAQIQAWoKEAkQChAFEAMQBA%3D%3D");
    pub const ALBUMS: Self = Self("EgWKAQIYAWoKEAkQChAFEAMQBA%3D%3D");
    pub const ARTISTS: Self = Self("EgWKAQIgAWoKEAkQChAFEAMQBA%3D%3D");
    pub const FEATURED_PLAYLISTS: Self = Self("EgeKAQQoADgBagwQDhAKEAMQBRAJEAQ%3D");
    pub const COMMUNITY_PLAYLISTS: Self = Self("EgeKAQQoAEABagoQAxAEEAoQCRAF");
    pub const PODCASTS: Self = Self("EgWKAQJQAWoKEAkQChAFEAMQBA%3D%3D");
    pub const EPISODES: Self = Self("EgWKAQJYAWoKEAkQChAFEAMQBA%3D%3D");
    pub const PROFILES: Self = Self("EgWKAQJYAWoSEAUQCRADEAQQEBAVEAoQDhAR");
}

/// Typed wrapper for a library-filter continuation token.
///
/// These are used as `continuation` values in browse requests to filter the
/// library landing page by a specific sort/filter criterion.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LibraryFilter(pub &'static str);

impl LibraryFilter {
    pub const RECENT_ACTIVITY: Self =
        Self("4qmFsgIrEhdGRW11c2ljX2xpYnJhcnlfbGFuZGluZxoQZ2dNR0tnUUlCaEFCb0FZQg%3D%3D");
    pub const RECENTLY_PLAYED: Self =
        Self("4qmFsgIrEhdGRW11c2ljX2xpYnJhcnlfbGFuZGluZxoQZ2dNR0tnUUlCUkFCb0FZQg%3D%3D");
    pub const PLAYLISTS_ALPHABETICAL: Self =
        Self("4qmFsgIrEhdGRW11c2ljX2xpa2VkX3BsYXlsaXN0cxoQZ2dNR0tnUUlBUkFBb0FZQg%3D%3D");
    pub const PLAYLISTS_RECENTLY_SAVED: Self =
        Self("4qmFsgIrEhdGRW11c2ljX2xpa2VkX3BsYXlsaXN0cxoQZ2dNR0tnUUlBQkFCb0FZQg%3D%3D");
}

/// Well-known browse IDs for `YouTube` Music's special pages.
pub mod browse_id {
    pub const HOME: &str = "FEmusic_home";
    pub const EXPLORE: &str = "FEmusic_explore";
    pub const NEW_RELEASES: &str = "FEmusic_new_releases_albums";
    pub const MOODS_AND_GENRES: &str = "FEmusic_moods_and_genres";
    pub const CHARTS: &str = "FEmusic_charts";
    pub const CHARTS_PARAMS: &str = "ggMGCgQIgAQ%3D";
    pub const HISTORY: &str = "FEmusic_history";
    pub const PODCAST_DISCOVER: &str = "FEmusic_non_music_audio";

    pub const LIBRARY_SONGS: &str = "FEmusic_liked_videos";
    pub const LIBRARY_PLAYLISTS: &str = "FEmusic_liked_playlists";
    pub const LIBRARY_ALBUMS: &str = "FEmusic_liked_albums";
    pub const LIBRARY_ARTISTS: &str = "FEmusic_library_artist_subscription_channel";
    pub const LIBRARY_PODCAST_CHANNELS: &str = "FEmusic_library_non_music_audio_channels_list";
    pub const LIBRARY_PODCAST_EPISODES: &str = "FEmusic_library_non_music_audio_list";

    /// "Episodes for Later" auto-playlist.
    pub const SAVED_EPISODES: &str = "VLSE";
    /// New episodes from subscribed/saved podcast shows.
    pub const NEW_PODCAST_EPISODES: &str = "VLRDPN";

    /// Prefix a playlist ID to browse its contents (e.g. `"VL" + playlist_id`).
    #[must_use]
    pub fn playlist(id: &str) -> String {
        format!("VL{id}")
    }
}
