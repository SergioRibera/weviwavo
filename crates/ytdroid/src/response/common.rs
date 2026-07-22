//! All shared renderer types that appear across browse / search / next / player responses.
//!
//! Field naming mirrors the actual Innertube JSON (`camelCase` → `snake_case` via serde).
//! Every field that may be absent is `Option<T>`; `Vec<T>` fields use `#[serde(default)]`
//! so a missing array deserialises as empty rather than erroring.

use serde::Deserialize;

// ─────────────────────────────────────────────
// Primitive helpers
// ─────────────────────────────────────────────

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct Run {
    pub text: String,
    pub navigation_endpoint: Option<NavigationEndpoint>,
}

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct Runs {
    #[serde(default)]
    pub runs: Vec<Run>,
}

impl Runs {
    /// Join all run texts into a single string.
    #[must_use]
    pub fn text(&self) -> String {
        self.runs.iter().map(|r| r.text.as_str()).collect()
    }

    /// Split runs on `·` separator runs, mirroring Metrolist's `splitBySeparator()`.
    /// Returns a `Vec` of groups, where each group is the slice of runs between separators.
    #[must_use]
    pub fn split_by_separator(&self) -> Vec<&[Run]> {
        let runs = self.runs.as_slice();
        let mut groups: Vec<&[Run]> = Vec::new();
        let mut start = 0;
        for (i, run) in runs.iter().enumerate() {
            if run.text == "\u{00B7}" || run.text == " \u{00B7} " {
                groups.push(&runs[start..i]);
                start = i + 1;
            }
        }
        groups.push(&runs[start..]);
        groups
    }

    /// Every other run starting from index 0 (skips separator runs).
    /// Mirrors Metrolist's `oddElements()`.
    pub fn odd_elements(&self) -> impl Iterator<Item = &Run> {
        self.runs.iter().step_by(2)
    }
}

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct Icon {
    pub icon_type: Option<String>,
}

// ─────────────────────────────────────────────
// Thumbnails
// ─────────────────────────────────────────────

#[derive(Debug, Clone, Deserialize)]
pub struct Thumbnail {
    pub url: String,
    pub width: Option<u32>,
    pub height: Option<u32>,
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct Thumbnails {
    #[serde(default)]
    pub thumbnails: Vec<Thumbnail>,
}

impl Thumbnails {
    /// Highest-resolution thumbnail URL (last entry = largest).
    #[must_use]
    pub fn best_url(&self) -> Option<&str> {
        self.thumbnails.last().map(|t| t.url.as_str())
    }
}

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct MusicThumbnailRenderer {
    pub thumbnail: Option<Thumbnails>,
}

impl MusicThumbnailRenderer {
    #[must_use]
    pub fn best_url(&self) -> Option<&str> {
        self.thumbnail.as_ref()?.best_url()
    }
}

/// Wraps `musicThumbnailRenderer` — used as `thumbnailRenderer` in `TwoRowItem`
/// and as `thumbnail` in `ResponsiveListItem` / `MultiRowItem`.
#[derive(Debug, Clone, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ThumbnailWrapper {
    pub music_thumbnail_renderer: Option<MusicThumbnailRenderer>,
}

impl ThumbnailWrapper {
    #[must_use]
    pub fn get_url(&self) -> Option<&str> {
        self.music_thumbnail_renderer.as_ref()?.best_url()
    }
}

// ─────────────────────────────────────────────
// Navigation endpoints
// ─────────────────────────────────────────────

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct BrowseEndpointContextMusicConfig {
    pub page_type: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct BrowseEndpointContextSupportedConfigs {
    pub browse_endpoint_context_music_config: Option<BrowseEndpointContextMusicConfig>,
}

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct BrowseEndpoint {
    pub browse_id: Option<String>,
    pub params: Option<String>,
    pub browse_endpoint_context_supported_configs: Option<BrowseEndpointContextSupportedConfigs>,
}

impl BrowseEndpoint {
    #[must_use]
    pub fn page_type(&self) -> Option<&str> {
        self.browse_endpoint_context_supported_configs
            .as_ref()?
            .browse_endpoint_context_music_config
            .as_ref()?
            .page_type
            .as_deref()
    }

    #[must_use]
    pub fn is_artist_endpoint(&self) -> bool {
        matches!(self.page_type(), Some(PAGE_TYPE_ARTIST))
    }

    #[must_use]
    pub fn is_podcast_endpoint(&self) -> bool {
        matches!(self.page_type(), Some(PAGE_TYPE_PODCAST_SHOW))
    }
}

/// Page type constants as found in `browseEndpointContextMusicConfig.pageType`.
pub const PAGE_TYPE_ALBUM: &str = "MUSIC_PAGE_TYPE_ALBUM";
pub const PAGE_TYPE_ARTIST: &str = "MUSIC_PAGE_TYPE_ARTIST";
pub const PAGE_TYPE_PLAYLIST: &str = "MUSIC_PAGE_TYPE_PLAYLIST";
pub const PAGE_TYPE_PODCAST_SHOW: &str = "MUSIC_PAGE_TYPE_PODCAST_SHOW_DETAIL_PAGE";
pub const PAGE_TYPE_EPISODE: &str = "MUSIC_PAGE_TYPE_NON_MUSIC_AUDIO_PAGE_TYPE";
pub const PAGE_TYPE_USER_CHANNEL: &str = "MUSIC_PAGE_TYPE_USER_CHANNEL";

/// `musicVideoType` values for `MusicResponsiveListItemRenderer`.
pub const VIDEO_TYPE_ATV: &str = "MUSIC_VIDEO_TYPE_ATV";
pub const VIDEO_TYPE_EPISODE: &str = "MUSIC_VIDEO_TYPE_EPISODE";

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct WatchEndpoint {
    pub video_id: Option<String>,
    pub playlist_id: Option<String>,
    pub params: Option<String>,
    pub playlist_set_video_id: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct WatchPlaylistEndpoint {
    pub playlist_id: Option<String>,
    pub params: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct FeedbackEndpoint {
    pub feedback_token: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct SubscribeEndpoint {
    #[serde(default)]
    pub channel_ids: Vec<String>,
}

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ServiceEndpoint {
    pub feedback_endpoint: Option<FeedbackEndpoint>,
    pub subscribe_endpoint: Option<SubscribeEndpoint>,
}

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct MusicDeleteEntityCommand {
    pub entity_id: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ButtonRendererCommand {
    pub music_delete_privately_owned_entity_command: Option<MusicDeleteEntityCommand>,
}

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ConfirmButtonRenderer {
    pub command: Option<ButtonRendererCommand>,
}

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ConfirmDialogRenderer {
    pub confirm_button: Option<ConfirmButtonRenderer>,
}

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ConfirmDialogEndpointContent {
    pub confirm_dialog_renderer: Option<ConfirmDialogRenderer>,
}

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ConfirmDialogEndpoint {
    pub content: Option<ConfirmDialogEndpointContent>,
}

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct NavigationEndpoint {
    pub browse_endpoint: Option<BrowseEndpoint>,
    pub watch_endpoint: Option<WatchEndpoint>,
    pub watch_playlist_endpoint: Option<WatchPlaylistEndpoint>,
    pub confirm_dialog_endpoint: Option<ConfirmDialogEndpoint>,
}

impl NavigationEndpoint {
    /// Returns the `videoId` from any available watch-type endpoint.
    #[must_use]
    pub fn any_video_id(&self) -> Option<&str> {
        self.watch_endpoint
            .as_ref()
            .and_then(|e| e.video_id.as_deref())
    }

    /// Returns the `playlistId` from `watchPlaylistEndpoint` or `watchEndpoint`.
    #[must_use]
    pub fn any_playlist_id(&self) -> Option<&str> {
        self.watch_playlist_endpoint
            .as_ref()
            .and_then(|e| e.playlist_id.as_deref())
            .or_else(|| {
                self.watch_endpoint
                    .as_ref()
                    .and_then(|e| e.playlist_id.as_deref())
            })
    }
}

// ─────────────────────────────────────────────
// Menu types
// ─────────────────────────────────────────────

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct MenuNavigationItemRenderer {
    pub text: Option<Runs>,
    pub icon: Option<Icon>,
    pub navigation_endpoint: Option<NavigationEndpoint>,
}

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct MenuServiceItemRenderer {
    pub text: Option<Runs>,
    pub icon: Option<Icon>,
    pub service_endpoint: Option<ServiceEndpoint>,
}

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ToggleMenuServiceItemRenderer {
    pub default_icon: Option<Icon>,
    pub toggled_icon: Option<Icon>,
    pub default_service_endpoint: Option<ServiceEndpoint>,
    pub toggled_service_endpoint: Option<ServiceEndpoint>,
    /// `true` when the user has already toggled this item (e.g. subscribed / saved).
    pub is_selected: Option<bool>,
}

/// Represents one item inside a `menuRenderer.items` list.
/// Exactly one inner renderer is non-null per item.
#[derive(Debug, Clone, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct MenuItem {
    pub menu_navigation_item_renderer: Option<MenuNavigationItemRenderer>,
    pub menu_service_item_renderer: Option<MenuServiceItemRenderer>,
    pub toggle_menu_service_item_renderer: Option<ToggleMenuServiceItemRenderer>,
}

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct MenuRenderer {
    #[serde(default)]
    pub items: Vec<MenuItem>,
}

impl MenuRenderer {
    /// Find a navigation item by icon type (e.g. `"MUSIC_SHUFFLE"`, `"MIX"`).
    #[must_use]
    pub fn nav_item_by_icon(&self, icon_type: &str) -> Option<&MenuNavigationItemRenderer> {
        self.items.iter().find_map(|i| {
            let nav = i.menu_navigation_item_renderer.as_ref()?;
            if nav.icon.as_ref()?.icon_type.as_deref() == Some(icon_type) {
                Some(nav)
            } else {
                None
            }
        })
    }

    /// Find a toggle item by its *default* icon type (e.g. `"BOOKMARK_BORDER"`, `"SUBSCRIBE"`).
    #[must_use]
    pub fn toggle_item_by_default_icon(
        &self,
        icon_type: &str,
    ) -> Option<&ToggleMenuServiceItemRenderer> {
        self.items.iter().find_map(|i| {
            let toggle = i.toggle_menu_service_item_renderer.as_ref()?;
            if toggle.default_icon.as_ref()?.icon_type.as_deref() == Some(icon_type) {
                Some(toggle)
            } else {
                None
            }
        })
    }

    /// Extract `(add_token, remove_token)` library feedback tokens.
    ///
    /// Icon conventions (from Metrolist):
    /// - `BOOKMARK_BORDER` → default=add, toggled=remove
    /// - `BOOKMARK`        → default=remove, toggled=add
    /// - `HEART`           → same as `BOOKMARK_BORDER`
    /// - `HEART_ACTIVE`    → same as `BOOKMARK`
    #[must_use]
    pub fn library_tokens(&self) -> (Option<String>, Option<String>) {
        const ADD_ICONS: &[&str] = &["BOOKMARK_BORDER", "HEART", "ADD_TO_LIBRARY"];
        const REMOVE_ICONS: &[&str] = &["BOOKMARK", "HEART_ACTIVE", "ADDED_TO_LIBRARY"];

        for item in &self.items {
            let Some(toggle) = &item.toggle_menu_service_item_renderer else {
                continue;
            };
            let default_icon = toggle
                .default_icon
                .as_ref()
                .and_then(|i| i.icon_type.as_deref())
                .unwrap_or("");

            let default_token = toggle
                .default_service_endpoint
                .as_ref()
                .and_then(|e| e.feedback_endpoint.as_ref())
                .and_then(|e| e.feedback_token.clone());
            let toggled_token = toggle
                .toggled_service_endpoint
                .as_ref()
                .and_then(|e| e.feedback_endpoint.as_ref())
                .and_then(|e| e.feedback_token.clone());

            if ADD_ICONS.contains(&default_icon) {
                return (default_token, toggled_token);
            }
            if REMOVE_ICONS.contains(&default_icon) {
                return (toggled_token, default_token);
            }
        }
        (None, None)
    }
}

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct Menu {
    pub menu_renderer: Option<MenuRenderer>,
}

// ─────────────────────────────────────────────
// Badge types
// ─────────────────────────────────────────────

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct MusicInlineBadgeRenderer {
    pub icon: Option<Icon>,
}

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct Badge {
    pub music_inline_badge_renderer: Option<MusicInlineBadgeRenderer>,
}

impl Badge {
    #[must_use]
    pub fn is_explicit(&self) -> bool {
        self.music_inline_badge_renderer
            .as_ref()
            .and_then(|b| b.icon.as_ref())
            .and_then(|i| i.icon_type.as_deref())
            == Some("MUSIC_EXPLICIT_BADGE")
    }
}

#[must_use]
pub fn badges_explicit(badges: &[Badge]) -> bool {
    badges.iter().any(Badge::is_explicit)
}

// ─────────────────────────────────────────────
// Play button overlay
// ─────────────────────────────────────────────

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct PlayNavigationEndpoint {
    pub watch_endpoint: Option<WatchEndpoint>,
    pub watch_playlist_endpoint: Option<WatchPlaylistEndpoint>,
}

impl PlayNavigationEndpoint {
    #[must_use]
    pub fn video_id(&self) -> Option<&str> {
        self.watch_endpoint
            .as_ref()
            .and_then(|e| e.video_id.as_deref())
    }

    #[must_use]
    pub fn playlist_id(&self) -> Option<&str> {
        self.watch_playlist_endpoint
            .as_ref()
            .and_then(|e| e.playlist_id.as_deref())
            .or_else(|| {
                self.watch_endpoint
                    .as_ref()
                    .and_then(|e| e.playlist_id.as_deref())
            })
    }
}

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct MusicPlayButtonRenderer {
    pub play_navigation_endpoint: Option<PlayNavigationEndpoint>,
}

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct OverlayContent {
    pub music_play_button_renderer: Option<MusicPlayButtonRenderer>,
}

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct MusicItemThumbnailOverlayRenderer {
    pub content: Option<OverlayContent>,
}

impl MusicItemThumbnailOverlayRenderer {
    #[must_use]
    pub fn play_nav(&self) -> Option<&PlayNavigationEndpoint> {
        self.content
            .as_ref()?
            .music_play_button_renderer
            .as_ref()?
            .play_navigation_endpoint
            .as_ref()
    }
}

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct MusicItemThumbnailOverlay {
    pub music_item_thumbnail_overlay_renderer: Option<MusicItemThumbnailOverlayRenderer>,
}

impl MusicItemThumbnailOverlay {
    #[must_use]
    pub fn renderer(&self) -> Option<&MusicItemThumbnailOverlayRenderer> {
        self.music_item_thumbnail_overlay_renderer.as_ref()
    }
}

// ─────────────────────────────────────────────
// Continuation token
// ─────────────────────────────────────────────

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NextContinuationData {
    pub continuation: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ContinuationItem {
    pub next_continuation_data: Option<NextContinuationData>,
    /// `continuationItemRenderer` contains a `trigger` and `continuationEndpoint`.
    pub continuation_item_renderer: Option<ContinuationItemRenderer>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ContinuationItemRenderer {
    pub continuation_endpoint: Option<ContinuationEndpoint>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ContinuationEndpoint {
    pub continuation_command: Option<ContinuationCommand>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ContinuationCommand {
    pub token: Option<String>,
}

/// Extract the continuation token from a slice of `ContinuationItem`.
#[must_use]
pub fn get_continuation(items: &[ContinuationItem]) -> Option<String> {
    items.iter().find_map(|item| {
        item.next_continuation_data
            .as_ref()
            .map(|d| d.continuation.clone())
            .or_else(|| {
                item.continuation_item_renderer
                    .as_ref()?
                    .continuation_endpoint
                    .as_ref()?
                    .continuation_command
                    .as_ref()?
                    .token
                    .clone()
            })
    })
}

// ─────────────────────────────────────────────
// MusicTwoRowItemRenderer
// ─────────────────────────────────────────────

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MusicTwoRowItemRenderer {
    pub title: Runs,
    pub subtitle: Option<Runs>,
    /// Named `thumbnailRenderer` in the JSON.
    pub thumbnail_renderer: ThumbnailWrapper,
    pub navigation_endpoint: NavigationEndpoint,
    pub thumbnail_overlay: Option<MusicItemThumbnailOverlay>,
    pub menu: Option<Menu>,
    pub subtitle_badges: Option<Vec<Badge>>,
}

impl MusicTwoRowItemRenderer {
    #[must_use]
    pub fn is_song(&self) -> bool {
        self.navigation_endpoint.watch_endpoint.is_some()
    }

    #[must_use]
    pub fn is_album(&self) -> bool {
        self.navigation_endpoint
            .browse_endpoint
            .as_ref()
            .and_then(|e| e.page_type())
            == Some(PAGE_TYPE_ALBUM)
    }

    #[must_use]
    pub fn is_playlist(&self) -> bool {
        self.navigation_endpoint
            .browse_endpoint
            .as_ref()
            .and_then(|e| e.page_type())
            == Some(PAGE_TYPE_PLAYLIST)
    }

    #[must_use]
    pub fn is_artist(&self) -> bool {
        self.navigation_endpoint
            .browse_endpoint
            .as_ref()
            .and_then(|e| e.page_type())
            == Some(PAGE_TYPE_ARTIST)
    }

    #[must_use]
    pub fn is_podcast(&self) -> bool {
        self.navigation_endpoint
            .browse_endpoint
            .as_ref()
            .and_then(|e| e.page_type())
            == Some(PAGE_TYPE_PODCAST_SHOW)
    }

    /// Note: check this BEFORE `is_song()` — episodes can match `is_song()` too.
    #[must_use]
    pub fn is_episode(&self) -> bool {
        self.navigation_endpoint
            .browse_endpoint
            .as_ref()
            .and_then(|e| e.page_type())
            == Some(PAGE_TYPE_EPISODE)
    }

    #[must_use]
    pub fn is_user_channel(&self) -> bool {
        self.navigation_endpoint
            .browse_endpoint
            .as_ref()
            .and_then(|e| e.page_type())
            == Some(PAGE_TYPE_USER_CHANNEL)
    }

    #[must_use]
    pub fn thumbnail_url(&self) -> Option<&str> {
        self.thumbnail_renderer.get_url()
    }

    #[must_use]
    pub fn is_explicit(&self) -> bool {
        self.subtitle_badges
            .as_deref()
            .is_some_and(badges_explicit)
    }

    /// Shuffle `watchPlaylistEndpoint` from menu item with icon `MUSIC_SHUFFLE`.
    #[must_use]
    pub fn shuffle_endpoint(&self) -> Option<&WatchPlaylistEndpoint> {
        self.menu
            .as_ref()?
            .menu_renderer
            .as_ref()?
            .nav_item_by_icon("MUSIC_SHUFFLE")?
            .navigation_endpoint
            .as_ref()?
            .watch_playlist_endpoint
            .as_ref()
    }

    /// Radio `watchPlaylistEndpoint` from menu item with icon `MIX`.
    #[must_use]
    pub fn radio_endpoint(&self) -> Option<&WatchPlaylistEndpoint> {
        self.menu
            .as_ref()?
            .menu_renderer
            .as_ref()?
            .nav_item_by_icon("MIX")?
            .navigation_endpoint
            .as_ref()?
            .watch_playlist_endpoint
            .as_ref()
    }

    /// Overlay play `watchPlaylistEndpoint` (for albums/playlists/podcasts).
    #[must_use]
    pub fn play_endpoint(&self) -> Option<&WatchPlaylistEndpoint> {
        self.thumbnail_overlay
            .as_ref()?
            .renderer()?
            .play_nav()?
            .watch_playlist_endpoint
            .as_ref()
    }

    /// Overlay play `watchEndpoint` (for songs/episodes).
    #[must_use]
    pub fn play_watch_endpoint(&self) -> Option<&WatchEndpoint> {
        self.thumbnail_overlay
            .as_ref()?
            .renderer()?
            .play_nav()?
            .watch_endpoint
            .as_ref()
    }
}

// ─────────────────────────────────────────────
// MusicResponsiveListItemRenderer
// ─────────────────────────────────────────────

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct FlexColumnRenderer {
    pub text: Option<Runs>,
}

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct FlexColumn {
    #[serde(default)]
    pub music_responsive_list_item_flex_column_renderer: FlexColumnRenderer,
}

impl FlexColumn {
    #[must_use]
    pub fn runs(&self) -> Option<&Runs> {
        self.music_responsive_list_item_flex_column_renderer
            .text
            .as_ref()
    }
}

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct PlaylistItemData {
    pub video_id: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct MusicResponsiveListItemRenderer {
    #[serde(default)]
    pub flex_columns: Vec<FlexColumn>,
    pub fixed_columns: Option<Vec<FlexColumn>>,
    /// Named `thumbnail` in JSON (same structure as `ThumbnailWrapper`).
    pub thumbnail: Option<ThumbnailWrapper>,
    pub badges: Option<Vec<Badge>>,
    pub menu: Option<Menu>,
    pub overlay: Option<MusicItemThumbnailOverlay>,
    pub navigation_endpoint: Option<NavigationEndpoint>,
    pub playlist_item_data: Option<PlaylistItemData>,
    pub playlist_set_video_id: Option<String>,
    pub music_video_type: Option<String>,
}

impl MusicResponsiveListItemRenderer {
    /// Extract the video ID from the best available source.
    #[must_use]
    pub fn video_id(&self) -> Option<&str> {
        self.playlist_item_data
            .as_ref()
            .and_then(|d| d.video_id.as_deref())
            .or_else(|| {
                self.navigation_endpoint
                    .as_ref()
                    .and_then(|e| e.watch_endpoint.as_ref())
                    .and_then(|e| e.video_id.as_deref())
            })
            .or_else(|| {
                self.overlay
                    .as_ref()?
                    .renderer()?
                    .play_nav()?
                    .video_id()
            })
            .or_else(|| {
                self.flex_columns
                    .first()?
                    .runs()?
                    .runs
                    .first()?
                    .navigation_endpoint
                    .as_ref()?
                    .watch_endpoint
                    .as_ref()?
                    .video_id
                    .as_deref()
            })
    }

    /// Note: always check `is_episode()` BEFORE `is_song()`.
    #[must_use]
    pub fn is_song(&self) -> bool {
        self.flex_columns
            .first()
            .and_then(|c| c.runs())
            .and_then(|r| r.runs.first())
            .and_then(|r| r.navigation_endpoint.as_ref())
            .and_then(|e| e.watch_endpoint.as_ref())
            .is_some()
            || self
                .overlay
                .as_ref()
                .and_then(|o| o.renderer())
                .and_then(|r| r.play_nav())
                .and_then(|n| n.watch_endpoint.as_ref())
                .is_some()
    }

    #[must_use]
    pub fn is_episode(&self) -> bool {
        self.music_video_type.as_deref() == Some(VIDEO_TYPE_EPISODE)
            || self
                .navigation_endpoint
                .as_ref()
                .and_then(|e| e.watch_endpoint.as_ref())
                .and_then(|e| e.params.as_deref())
                == Some("wAEB8gECKAE%3D")
    }

    #[must_use]
    pub fn is_album(&self) -> bool {
        self.page_type() == Some(PAGE_TYPE_ALBUM)
    }

    #[must_use]
    pub fn is_playlist(&self) -> bool {
        self.page_type() == Some(PAGE_TYPE_PLAYLIST)
    }

    #[must_use]
    pub fn is_artist(&self) -> bool {
        self.page_type() == Some(PAGE_TYPE_ARTIST)
    }

    #[must_use]
    pub fn is_podcast(&self) -> bool {
        self.page_type() == Some(PAGE_TYPE_PODCAST_SHOW)
    }

    #[must_use]
    pub fn is_user_channel(&self) -> bool {
        self.page_type() == Some(PAGE_TYPE_USER_CHANNEL)
    }

    fn page_type(&self) -> Option<&str> {
        self.navigation_endpoint
            .as_ref()?
            .browse_endpoint
            .as_ref()?
            .page_type()
    }

    #[must_use]
    pub fn is_explicit(&self) -> bool {
        self.badges
            .as_deref()
            .is_some_and(badges_explicit)
    }

    #[must_use]
    pub fn thumbnail_url(&self) -> Option<&str> {
        self.thumbnail.as_ref()?.get_url()
    }

    /// Duration string from `fixedColumns[0].runs[0].text`.
    #[must_use]
    pub fn duration_text(&self) -> Option<&str> {
        self.fixed_columns
            .as_ref()?
            .first()?
            .runs()?
            .runs
            .first()
            .map(|r| r.text.as_str())
    }

    /// Library feedback tokens from the item's menu.
    #[must_use]
    pub fn library_tokens(&self) -> (Option<String>, Option<String>) {
        self.menu
            .as_ref()
            .and_then(|m| m.menu_renderer.as_ref())
            .map_or((None, None), MenuRenderer::library_tokens)
    }

    /// Overlay play `watchEndpoint` for songs.
    #[must_use]
    pub fn play_watch_endpoint(&self) -> Option<&WatchEndpoint> {
        self.overlay.as_ref()?.renderer()?.play_nav()?.watch_endpoint.as_ref()
    }

    /// Overlay play `watchPlaylistEndpoint` for playlists/podcasts.
    #[must_use]
    pub fn play_playlist_endpoint(&self) -> Option<&WatchPlaylistEndpoint> {
        self.overlay
            .as_ref()?
            .renderer()?
            .play_nav()?
            .watch_playlist_endpoint
            .as_ref()
    }
}

// ─────────────────────────────────────────────
// MusicMultiRowListItemRenderer (podcast episodes)
// ─────────────────────────────────────────────

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct MusicMultiRowListItemRenderer {
    pub title: Option<Runs>,
    pub subtitle: Option<Runs>,
    pub second_subtitle: Option<Runs>,
    pub thumbnail: Option<ThumbnailWrapper>,
    pub on_tap: Option<NavigationEndpoint>,
    pub menu: Option<Menu>,
}

impl MusicMultiRowListItemRenderer {
    #[must_use]
    pub fn video_id(&self) -> Option<&str> {
        self.on_tap
            .as_ref()?
            .watch_endpoint
            .as_ref()?
            .video_id
            .as_deref()
    }

    #[must_use]
    pub fn thumbnail_url(&self) -> Option<&str> {
        self.thumbnail.as_ref()?.get_url()
    }

    #[must_use]
    pub fn library_tokens(&self) -> (Option<String>, Option<String>) {
        self.menu
            .as_ref()
            .and_then(|m| m.menu_renderer.as_ref())
            .map_or((None, None), MenuRenderer::library_tokens)
    }
}

// ─────────────────────────────────────────────
// MusicNavigationButtonRenderer (mood/genre chips)
// ─────────────────────────────────────────────

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MusicNavigationButtonRenderer {
    pub button_text: Option<Runs>,
    pub solid: Option<MusicNavButtonSolid>,
    pub navigation_endpoint: Option<NavigationEndpoint>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MusicNavButtonSolid {
    pub left_stripe_color: Option<u64>,
}

// ─────────────────────────────────────────────
// ChipCloud (home page filter chips)
// ─────────────────────────────────────────────

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChipCloudChipRenderer {
    pub text: Option<Runs>,
    pub navigation_endpoint: Option<NavigationEndpoint>,
    pub on_deselected_command: Option<NavigationEndpoint>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChipCloudChip {
    pub chip_cloud_chip_renderer: ChipCloudChipRenderer,
}

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ChipCloudRenderer {
    #[serde(default)]
    pub chips: Vec<ChipCloudChip>,
}

// ─────────────────────────────────────────────
// Section / shelf renderers
// ─────────────────────────────────────────────

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct MusicCarouselShelfBasicHeaderRenderer {
    pub title: Option<Runs>,
    pub strapline: Option<Runs>,
    pub thumbnail: Option<ThumbnailWrapper>,
    pub more_content_button: Option<MoreContentButton>,
}

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct MoreContentButton {
    pub button_renderer: Option<MoreContentButtonRenderer>,
}

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct MoreContentButtonRenderer {
    pub navigation_endpoint: Option<NavigationEndpoint>,
}

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct MusicCarouselShelfHeader {
    pub music_carousel_shelf_basic_header_renderer: Option<MusicCarouselShelfBasicHeaderRenderer>,
}

/// One slot inside a `musicCarouselShelfRenderer.contents` list.
/// Exactly one inner renderer is populated.
#[derive(Debug, Clone, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct CarouselShelfContent {
    pub music_two_row_item_renderer: Option<MusicTwoRowItemRenderer>,
    pub music_responsive_list_item_renderer: Option<MusicResponsiveListItemRenderer>,
    pub music_multi_row_list_item_renderer: Option<MusicMultiRowListItemRenderer>,
    pub music_navigation_button_renderer: Option<MusicNavigationButtonRenderer>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MusicCarouselShelfRenderer {
    pub header: Option<MusicCarouselShelfHeader>,
    #[serde(default)]
    pub contents: Vec<CarouselShelfContent>,
    #[serde(default)]
    pub continuations: Vec<ContinuationItem>,
}

impl MusicCarouselShelfRenderer {
    #[must_use]
    pub fn title(&self) -> Option<&str> {
        self.header
            .as_ref()?
            .music_carousel_shelf_basic_header_renderer
            .as_ref()?
            .title
            .as_ref()
            .map(|r| r.runs.first().map(|r| r.text.as_str()))?
    }

    #[must_use]
    pub fn more_browse_id(&self) -> Option<&str> {
        self.header
            .as_ref()?
            .music_carousel_shelf_basic_header_renderer
            .as_ref()?
            .more_content_button
            .as_ref()?
            .button_renderer
            .as_ref()?
            .navigation_endpoint
            .as_ref()?
            .browse_endpoint
            .as_ref()?
            .browse_id
            .as_deref()
    }
}

/// One slot inside `musicShelfRenderer.contents`.
#[derive(Debug, Clone, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ShelfContent {
    pub music_responsive_list_item_renderer: Option<MusicResponsiveListItemRenderer>,
    pub music_multi_row_list_item_renderer: Option<MusicMultiRowListItemRenderer>,
    /// Continuation item embedded inside a shelf contents list.
    pub continuation_item_renderer: Option<ContinuationItemRenderer>,
}

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct MusicShelfRenderer {
    pub title: Option<Runs>,
    pub contents: Option<Vec<ShelfContent>>,
    #[serde(default)]
    pub continuations: Vec<ContinuationItem>,
}

impl MusicShelfRenderer {
    /// All non-continuation responsive-list items.
    pub fn list_items(&self) -> impl Iterator<Item = &MusicResponsiveListItemRenderer> {
        self.contents
            .as_deref()
            .unwrap_or_default()
            .iter()
            .filter_map(|c| c.music_responsive_list_item_renderer.as_ref())
    }
}

/// One slot inside `musicPlaylistShelfRenderer.contents`.
/// Same shape as `ShelfContent`.
pub type PlaylistShelfContent = ShelfContent;

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct MusicPlaylistShelfRenderer {
    pub playlist_id: Option<String>,
    #[serde(default)]
    pub contents: Vec<PlaylistShelfContent>,
    #[serde(default)]
    pub continuations: Vec<ContinuationItem>,
}

/// `gridRenderer` item slot.
#[derive(Debug, Clone, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct GridRendererItem {
    pub music_two_row_item_renderer: Option<MusicTwoRowItemRenderer>,
}

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct GridHeaderRenderer {
    pub title: Option<Runs>,
}

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct GridRendererHeader {
    pub grid_header_renderer: Option<GridHeaderRenderer>,
}

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct GridRenderer {
    #[serde(default)]
    pub items: Vec<GridRendererItem>,
    pub header: Option<GridRendererHeader>,
    #[serde(default)]
    pub continuations: Vec<ContinuationItem>,
}

// ─────────────────────────────────────────────
// MusicResponsiveHeaderRenderer (playlist/album/podcast page header)
// ─────────────────────────────────────────────

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct AvatarViewModel {
    pub image: Option<AvatarImage>,
}

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct AvatarSource {
    pub url: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct AvatarImage {
    #[serde(default)]
    pub sources: Vec<AvatarSource>,
}

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct AvatarStackViewModel {
    pub text: Option<AvatarStackText>,
    pub renderer_context: Option<RendererContext>,
    #[serde(default)]
    pub avatars: Vec<AvatarItem>,
}

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct AvatarItem {
    pub avatar_view_model: Option<AvatarViewModel>,
}

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct AvatarStackText {
    pub content: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct RendererContext {
    pub command_context: Option<CommandContext>,
}

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct CommandContext {
    pub on_tap: Option<OnTapCommand>,
}

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct OnTapCommand {
    pub innertube_command: Option<InnertubeCommand>,
}

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct InnertubeCommand {
    pub browse_endpoint: Option<BrowseEndpoint>,
}

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct Facepile {
    pub avatar_stack_view_model: Option<AvatarStackViewModel>,
}

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct MusicDescriptionShelf {
    pub description: Option<Runs>,
}

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct HeaderButton {
    pub menu_renderer: Option<MenuRenderer>,
    pub toggle_button_renderer: Option<ToggleButtonRenderer>,
    pub music_play_button_renderer: Option<MusicPlayButtonRenderer>,
}

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ToggleButtonRenderer {
    pub default_icon: Option<Icon>,
    pub default_service_endpoint: Option<ServiceEndpoint>,
    pub toggled_service_endpoint: Option<ServiceEndpoint>,
}

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct MusicResponsiveHeaderRenderer {
    pub title: Option<Runs>,
    pub subtitle: Option<Runs>,
    pub strapline_text_one: Option<Runs>,
    pub second_subtitle: Option<Runs>,
    pub thumbnail: Option<ThumbnailWrapper>,
    pub description: Option<MusicDescriptionShelf>,
    pub facepile: Option<Facepile>,
    #[serde(default)]
    pub buttons: Vec<HeaderButton>,
}

// ─────────────────────────────────────────────
// Subscribe button (artist page header)
// ─────────────────────────────────────────────

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct SubscribeButtonRenderer {
    pub channel_id: Option<String>,
    pub subscribed: Option<bool>,
    pub long_subscriber_count_text: Option<Runs>,
    pub short_subscriber_count_text: Option<Runs>,
    pub subscriber_count_with_subscribe_text: Option<Runs>,
}

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct SubscriptionButton {
    pub subscribe_button_renderer: Option<SubscribeButtonRenderer>,
}

// ─────────────────────────────────────────────
// Artist page immersive header
// ─────────────────────────────────────────────

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct MusicImmersiveHeaderRenderer {
    pub title: Option<Runs>,
    pub thumbnail: Option<ThumbnailWrapper>,
    pub description: Option<Runs>,
    pub play_button: Option<PlayButtonWrapper>,
    pub start_radio_button: Option<PlayButtonWrapper>,
    pub subscription_button: Option<SubscriptionButton>,
    pub subscription_button2: Option<SubscriptionButton>,
    pub monthly_listener_count: Option<Runs>,
}

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct PlayButtonWrapper {
    pub button_renderer: Option<PlayButtonRenderer>,
}

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct PlayButtonRenderer {
    pub navigation_endpoint: Option<NavigationEndpoint>,
}

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct MusicVisualHeaderRenderer {
    pub title: Option<Runs>,
    pub foreground_thumbnail: Option<ThumbnailWrapper>,
    pub subscription_button: Option<SubscriptionButton>,
}

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct MusicDetailHeaderRenderer {
    pub title: Runs,
    pub subtitle: Runs,
    pub thumbnail: Option<ThumbnailWrapper>,
    pub description: Option<Runs>,
    pub menu: Option<Menu>,
}

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct MusicHeaderRenderer {
    pub title: Option<Runs>,
    pub strapline_text_one: Option<Runs>,
    pub thumbnail: Option<ThumbnailWrapper>,
    pub second_subtitle: Option<Runs>,
    #[serde(default)]
    pub buttons: Vec<HeaderButton>,
}

/// `musicEditablePlaylistDetailHeaderRenderer` — wraps either a responsive or detail header.
#[derive(Debug, Clone, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct MusicEditablePlaylistDetailHeaderRenderer {
    pub header: Option<EditablePlaylistHeader>,
}

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct EditablePlaylistHeader {
    pub music_responsive_header_renderer: Option<MusicResponsiveHeaderRenderer>,
    pub music_detail_header_renderer: Option<MusicDetailHeaderRenderer>,
}
