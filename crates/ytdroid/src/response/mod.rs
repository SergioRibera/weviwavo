//! Top-level response structs for every Innertube endpoint used by [`crate::YouTube`].

pub mod common;

use serde::Deserialize;

use common::{
    ChipCloudRenderer, ContinuationItem, GridRenderer,
    MusicCarouselShelfRenderer, MusicDetailHeaderRenderer, MusicEditablePlaylistDetailHeaderRenderer,
    MusicHeaderRenderer, MusicImmersiveHeaderRenderer,
    MusicPlaylistShelfRenderer, MusicResponsiveHeaderRenderer, MusicResponsiveListItemRenderer,
    MusicShelfRenderer, MusicVisualHeaderRenderer,
    get_continuation,
};

// ─────────────────────────────────────────────
// SectionListRenderer — the core content container
// ─────────────────────────────────────────────

/// One slot inside `sectionListRenderer.contents`.
/// Only one renderer inside is populated per slot.
#[derive(Debug, Clone, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct SectionContent {
    pub music_carousel_shelf_renderer: Option<MusicCarouselShelfRenderer>,
    pub music_shelf_renderer: Option<MusicShelfRenderer>,
    pub music_playlist_shelf_renderer: Option<MusicPlaylistShelfRenderer>,
    pub music_responsive_header_renderer: Option<MusicResponsiveHeaderRenderer>,
    pub music_editable_playlist_detail_header_renderer:
        Option<MusicEditablePlaylistDetailHeaderRenderer>,
    pub music_card_shelf_renderer: Option<MusicCardShelfRenderer>,
    pub music_description_shelf_renderer: Option<MusicDescriptionShelfRenderer>,
    pub grid_renderer: Option<GridRenderer>,
    pub item_section_renderer: Option<ItemSectionRenderer>,
}

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct SectionListRendererHeader {
    pub chip_cloud_renderer: Option<ChipCloudRenderer>,
}

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct SectionListRenderer {
    #[serde(default)]
    pub contents: Vec<SectionContent>,
    pub header: Option<SectionListRendererHeader>,
    #[serde(default)]
    pub continuations: Vec<ContinuationItem>,
}

impl SectionListRenderer {
    #[must_use]
    pub fn continuation(&self) -> Option<String> {
        get_continuation(&self.continuations)
    }
}

// ─────────────────────────────────────────────
// Card shelf (top result in search)
// ─────────────────────────────────────────────

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct MusicCardShelfHeaderBasicRenderer {
    pub title: Option<common::Runs>,
}

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct MusicCardShelfHeader {
    pub music_card_shelf_header_basic_renderer: Option<MusicCardShelfHeaderBasicRenderer>,
}

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct MusicCardShelfContent {
    pub music_responsive_list_item_renderer: Option<MusicResponsiveListItemRenderer>,
}

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct MusicCardShelfRenderer {
    pub title: common::Runs,
    pub subtitle: common::Runs,
    pub header: Option<MusicCardShelfHeader>,
    pub on_tap: common::NavigationEndpoint,
    pub contents: Option<Vec<MusicCardShelfContent>>,
}

// ─────────────────────────────────────────────
// Description shelf (artist bio)
// ─────────────────────────────────────────────

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct MusicDescriptionShelfRenderer {
    pub description: Option<common::Runs>,
    pub more_button: Option<MoreButton>,
}

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct MoreButton {
    pub button_renderer: Option<MoreButtonRenderer>,
}

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct MoreButtonRenderer {
    pub text: Option<common::Runs>,
}

// ─────────────────────────────────────────────
// ItemSectionRenderer (wraps other renderers in library)
// ─────────────────────────────────────────────

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ItemSectionContent {
    pub grid_renderer: Option<GridRenderer>,
    pub music_shelf_renderer: Option<MusicShelfRenderer>,
    pub music_playlist_shelf_renderer: Option<MusicPlaylistShelfRenderer>,
}

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ItemSectionRenderer {
    #[serde(default)]
    pub contents: Vec<ItemSectionContent>,
}

// ─────────────────────────────────────────────
// Tab hierarchy
// ─────────────────────────────────────────────

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct TabRendererContent {
    pub section_list_renderer: Option<SectionListRenderer>,
}

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct TabRenderer {
    pub title: Option<String>,
    pub content: Option<TabRendererContent>,
    pub selected: Option<bool>,
}

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct Tab {
    pub tab_renderer: TabRenderer,
}

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct SingleColumnBrowseResultsRenderer {
    #[serde(default)]
    pub tabs: Vec<Tab>,
}

impl SingleColumnBrowseResultsRenderer {
    #[must_use]
    pub fn first_section_list(&self) -> Option<&SectionListRenderer> {
        self.tabs
            .first()?
            .tab_renderer
            .content
            .as_ref()?
            .section_list_renderer
            .as_ref()
    }

    #[must_use]
    pub fn section_list_at(&self, idx: usize) -> Option<&SectionListRenderer> {
        self.tabs
            .get(idx)?
            .tab_renderer
            .content
            .as_ref()?
            .section_list_renderer
            .as_ref()
    }
}

// ─────────────────────────────────────────────
// Two-column layout (playlist / album / podcast)
// ─────────────────────────────────────────────

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct TwoColumnSecondaryContents {
    pub section_list_renderer: Option<SectionListRenderer>,
}

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct TwoColumnBrowseResultsRenderer {
    #[serde(default)]
    pub tabs: Vec<Tab>,
    pub secondary_contents: Option<TwoColumnSecondaryContents>,
}

impl TwoColumnBrowseResultsRenderer {
    #[must_use]
    pub fn first_tab_section_list(&self) -> Option<&SectionListRenderer> {
        self.tabs
            .first()?
            .tab_renderer
            .content
            .as_ref()?
            .section_list_renderer
            .as_ref()
    }

    #[must_use]
    pub fn secondary_section_list(&self) -> Option<&SectionListRenderer> {
        self.secondary_contents
            .as_ref()?
            .section_list_renderer
            .as_ref()
    }
}

// ─────────────────────────────────────────────
// Tabbed search results
// ─────────────────────────────────────────────

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct TabbedSearchResultsRenderer {
    #[serde(default)]
    pub tabs: Vec<Tab>,
}

// ─────────────────────────────────────────────
// Browse response top-level
// ─────────────────────────────────────────────

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct BrowseResponseContents {
    pub single_column_browse_results_renderer: Option<SingleColumnBrowseResultsRenderer>,
    pub two_column_browse_results_renderer: Option<TwoColumnBrowseResultsRenderer>,
}

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct BrowseResponseHeader {
    pub music_immersive_header_renderer: Option<MusicImmersiveHeaderRenderer>,
    pub music_detail_header_renderer: Option<MusicDetailHeaderRenderer>,
    pub music_header_renderer: Option<MusicHeaderRenderer>,
    pub music_visual_header_renderer: Option<MusicVisualHeaderRenderer>,
}

/// Returned by `browse/edit_playlist` continuation responses.
#[derive(Debug, Clone, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct SectionListContinuation {
    #[serde(default)]
    pub contents: Vec<SectionContent>,
    #[serde(default)]
    pub continuations: Vec<ContinuationItem>,
}

impl SectionListContinuation {
    #[must_use]
    pub fn continuation(&self) -> Option<String> {
        get_continuation(&self.continuations)
    }
}

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct MusicPlaylistShelfContinuation {
    #[serde(default)]
    pub contents: Vec<common::PlaylistShelfContent>,
    #[serde(default)]
    pub continuations: Vec<ContinuationItem>,
}

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct MusicShelfContinuation {
    #[serde(default)]
    pub contents: Vec<common::ShelfContent>,
    #[serde(default)]
    pub continuations: Vec<ContinuationItem>,
}

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct GridContinuation {
    #[serde(default)]
    pub items: Vec<common::GridRendererItem>,
    #[serde(default)]
    pub continuations: Vec<ContinuationItem>,
}

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct BrowseContinuationContents {
    pub section_list_continuation: Option<SectionListContinuation>,
    pub music_playlist_shelf_continuation: Option<MusicPlaylistShelfContinuation>,
    pub music_shelf_continuation: Option<MusicShelfContinuation>,
    pub grid_continuation: Option<GridContinuation>,
}

/// Items appended via `appendContinuationItemsAction`.
#[derive(Debug, Clone, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct AppendContinuationItemsAction {
    /// Each item may be a shelf content slot or a raw continuation item.
    #[serde(default)]
    pub continuation_items: Vec<AppendedItem>,
}

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct AppendedItem {
    pub music_responsive_list_item_renderer: Option<MusicResponsiveListItemRenderer>,
    pub continuation_item_renderer: Option<common::ContinuationItemRenderer>,
}

impl AppendedItem {
    #[must_use]
    pub fn continuation(&self) -> Option<String> {
        self.continuation_item_renderer
            .as_ref()?
            .continuation_endpoint
            .as_ref()?
            .continuation_command
            .as_ref()?
            .token
            .clone()
    }
}

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct OnResponseReceivedAction {
    pub append_continuation_items_action: Option<AppendContinuationItemsAction>,
}

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct BrowseResponse {
    pub contents: Option<BrowseResponseContents>,
    pub header: Option<BrowseResponseHeader>,
    pub continuation_contents: Option<BrowseContinuationContents>,
    #[serde(default)]
    pub on_response_received_actions: Vec<OnResponseReceivedAction>,
    pub microformat: Option<Microformat>,
}

impl BrowseResponse {
    #[must_use]
    pub fn single_col(&self) -> Option<&SingleColumnBrowseResultsRenderer> {
        self.contents
            .as_ref()?
            .single_column_browse_results_renderer
            .as_ref()
    }

    #[must_use]
    pub fn two_col(&self) -> Option<&TwoColumnBrowseResultsRenderer> {
        self.contents
            .as_ref()?
            .two_column_browse_results_renderer
            .as_ref()
    }

    pub fn appended_items(&self) -> impl Iterator<Item = &AppendedItem> {
        self.on_response_received_actions
            .iter()
            .filter_map(|a| a.append_continuation_items_action.as_ref())
            .flat_map(|a| a.continuation_items.iter())
    }
}

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct Microformat {
    pub microformat_data_renderer: Option<MicroformatDataRenderer>,
}

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct MicroformatDataRenderer {
    pub url_canonical: Option<String>,
}

// ─────────────────────────────────────────────
// Search response
// ─────────────────────────────────────────────

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct SearchResponseContents {
    pub tabbed_search_results_renderer: Option<TabbedSearchResultsRenderer>,
}

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct MusicShelfContinuationWrapper {
    pub music_shelf_continuation: Option<MusicShelfContinuation>,
}

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct SearchResponse {
    pub contents: Option<SearchResponseContents>,
    pub continuation_contents: Option<MusicShelfContinuationWrapper>,
}

impl SearchResponse {
    #[must_use]
    pub fn first_section_list(&self) -> Option<&SectionListRenderer> {
        self.contents
            .as_ref()?
            .tabbed_search_results_renderer
            .as_ref()?
            .tabs
            .first()?
            .tab_renderer
            .content
            .as_ref()?
            .section_list_renderer
            .as_ref()
    }
}

// ─────────────────────────────────────────────
// Player response
// ─────────────────────────────────────────────

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct PlayabilityStatus {
    pub status: String,
    pub reason: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct StreamingFormat {
    pub url: Option<String>,
    pub bitrate: Option<u64>,
    /// Present only for video streams; absent for audio-only.
    pub width: Option<u32>,
    pub height: Option<u32>,
    #[serde(rename = "mimeType")]
    pub mime_type: Option<String>,
    #[serde(rename = "contentLength")]
    pub content_length: Option<String>,
    #[serde(rename = "audioQuality")]
    pub audio_quality: Option<String>,
    #[serde(rename = "signatureCipher")]
    pub signature_cipher: Option<String>,
}

impl StreamingFormat {
    /// True when this format carries only audio (no width/height).
    #[must_use]
    pub fn is_audio_only(&self) -> bool {
        self.width.is_none() && self.height.is_none()
    }

    #[must_use]
    pub fn has_direct_url(&self) -> bool {
        self.url.as_deref().is_some_and(|u| !u.is_empty())
    }
}

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct StreamingData {
    #[serde(default)]
    pub formats: Vec<StreamingFormat>,
    #[serde(default)]
    pub adaptive_formats: Vec<StreamingFormat>,
    pub expires_in_seconds: Option<String>,
}

impl StreamingData {
    /// Best audio-only format with a direct URL.
    ///
    /// Priority: `audio/mp4` (AAC) → `audio/mpeg` (MP3) → `audio/ogg` (Vorbis) → non-WebM fallback.
    /// `audio/webm` (Opus) excluded: symphonia 0.5.x has no Opus codec.
    #[must_use]
    pub fn best_audio_format(&self) -> Option<&StreamingFormat> {
        self.best_supported_audio(|f| f.is_audio_only() && f.has_direct_url())
    }

    /// Best audio-only format that uses `signatureCipher` (WEB clients).
    ///
    /// Same MIME priority as [`best_audio_format`]: mp4 → mpeg → ogg → non-WebM fallback.
    #[must_use]
    pub fn best_cipher_audio_format(&self) -> Option<&StreamingFormat> {
        self.best_supported_audio(|f| f.is_audio_only() && f.signature_cipher.is_some())
    }

    fn best_supported_audio(&self, base: impl Fn(&StreamingFormat) -> bool) -> Option<&StreamingFormat> {
        let audio = || self.adaptive_formats.iter().filter(|f| base(f));
        for prefix in &["audio/mp4", "audio/mpeg", "audio/ogg"] {
            let best = audio()
                .filter(|f| f.mime_type.as_deref().map_or(false, |m| m.starts_with(prefix)))
                .max_by_key(|f| f.bitrate.unwrap_or(0));
            if best.is_some() {
                return best;
            }
        }
        // Last resort: anything except WebM (Opus not supported by symphonia 0.5.x).
        audio()
            .filter(|f| !f.mime_type.as_deref().map_or(false, |m| m.starts_with("audio/webm")))
            .max_by_key(|f| f.bitrate.unwrap_or(0))
    }
}

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct PlayerResponse {
    #[serde(default)]
    pub playability_status: PlayabilityStatus,
    pub streaming_data: Option<StreamingData>,
    pub video_details: Option<VideoDetails>,
}

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct VideoDetails {
    pub video_id: Option<String>,
    pub title: Option<String>,
    pub author: Option<String>,
    pub length_seconds: Option<String>,
    pub thumbnail: Option<common::Thumbnails>,
}

// ─────────────────────────────────────────────
// Next response (related / up-next)
// ─────────────────────────────────────────────

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct NextTabContent {
    pub music_queue_renderer: Option<MusicQueueRenderer>,
}

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct NextTab {
    pub tab_renderer: NextTabRenderer,
}

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct NextTabRenderer {
    pub content: Option<NextTabContent>,
}

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct MusicQueueRenderer {
    pub content: Option<MusicQueueContent>,
    pub sub_header_chip_cloud: Option<SubHeaderChipCloud>,
}

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct SubHeaderChipCloud {
    pub chip_cloud_renderer: Option<common::ChipCloudRenderer>,
}

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct MusicQueueContent {
    pub playlist_panel_renderer: Option<PlaylistPanelRenderer>,
}

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct PlaylistPanelRenderer {
    #[serde(default)]
    pub contents: Vec<PlaylistPanelContent>,
    pub playlist_id: Option<String>,
    #[serde(default)]
    pub continuations: Vec<ContinuationItem>,
}

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct PlaylistPanelContent {
    pub playlist_panel_video_renderer: Option<PlaylistPanelVideoRenderer>,
    pub playlist_panel_video_wrapper_renderer: Option<Box<PlaylistPanelVideoWrapperRenderer>>,
    pub automix_preview_video_renderer: Option<AutomixPreviewVideoRenderer>,
}

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct PlaylistPanelVideoWrapperRenderer {
    pub primary_renderer: Option<PlaylistPanelContent>,
}

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct PlaylistPanelVideoRenderer {
    pub video_id: Option<String>,
    pub title: Option<common::Runs>,
    pub long_by_line_text: Option<common::Runs>,
    pub thumbnail: Option<common::Thumbnails>,
    pub length_text: Option<common::Runs>,
    pub navigation_endpoint: Option<common::NavigationEndpoint>,
    pub playlist_set_video_id: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct AutomixPreviewVideoRenderer {
    pub content: Option<AutomixContent>,
}

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct AutomixContent {
    pub automix_play_button_renderer: Option<AutomixPlayButtonRenderer>,
}

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct AutomixPlayButtonRenderer {
    pub play_navigation_endpoint: Option<common::NavigationEndpoint>,
}

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct NextContents {
    pub tabbed_renderer: Option<NextTabbedRenderer>,
    pub two_column_watch_next_results: Option<TwoColumnWatchNextResults>,
}

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct NextTabbedRenderer {
    pub watch_next_tabbed_results_renderer: Option<WatchNextTabbedResultsRenderer>,
}

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct WatchNextTabbedResultsRenderer {
    #[serde(default)]
    pub tabs: Vec<NextTab>,
}

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct TwoColumnWatchNextResults {
    pub results: Option<TwoColumnWatchNextResultsContent>,
}

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct TwoColumnWatchNextResultsContent {
    pub results: Option<WatchNextResultsContent>,
}

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct WatchNextResultsContent {
    #[serde(default)]
    pub content: Vec<WatchNextResultSlot>,
}

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct WatchNextResultSlot {
    pub video_primary_info_renderer: Option<VideoPrimaryInfoRenderer>,
    pub video_secondary_info_renderer: Option<VideoSecondaryInfoRenderer>,
}

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct VideoPrimaryInfoRenderer {
    pub title: Option<common::Runs>,
    pub date_text: Option<SimpleText>,
}

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct VideoSecondaryInfoRenderer {
    pub owner: Option<VideoOwnerWrapper>,
    pub attributed_description: Option<AttributedDescription>,
}

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct VideoOwnerWrapper {
    pub video_owner_renderer: Option<VideoOwnerRenderer>,
}

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct VideoOwnerRenderer {
    pub title: Option<common::Runs>,
    pub thumbnail: Option<common::Thumbnails>,
    pub navigation_endpoint: Option<common::NavigationEndpoint>,
    pub subscriber_count_text: Option<SimpleText>,
}

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct AttributedDescription {
    pub content: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct NextContinuationContents {
    pub playlist_panel_continuation: Option<PlaylistPanelContinuation>,
}

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct PlaylistPanelContinuation {
    #[serde(default)]
    pub contents: Vec<PlaylistPanelContent>,
    #[serde(default)]
    pub continuations: Vec<ContinuationItem>,
}

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct NextResponse {
    pub contents: Option<NextContents>,
    pub continuation_contents: Option<NextContinuationContents>,
}

// ─────────────────────────────────────────────
// Queue response
// ─────────────────────────────────────────────

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct GetQueueResponse {
    pub queue_datas: Option<Vec<QueueData>>,
}

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct QueueData {
    pub content: Option<QueueDataContent>,
}

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct QueueDataContent {
    pub playlist_panel_video_renderer: Option<PlaylistPanelVideoRenderer>,
}

// ─────────────────────────────────────────────
// Search suggestions response
// ─────────────────────────────────────────────

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct SearchSuggestionRenderer {
    pub suggestion: Option<common::Runs>,
}

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct SearchSuggestionsSectionContent {
    pub search_suggestion_renderer: Option<SearchSuggestionRenderer>,
    pub music_responsive_list_item_renderer: Option<MusicResponsiveListItemRenderer>,
}

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct SearchSuggestionsSectionRenderer {
    #[serde(default)]
    pub contents: Vec<SearchSuggestionsSectionContent>,
}

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct SearchSuggestionsSection {
    pub search_suggestions_section_renderer: Option<SearchSuggestionsSectionRenderer>,
}

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct GetSearchSuggestionsResponse {
    #[serde(default)]
    pub contents: Vec<SearchSuggestionsSection>,
}

// ─────────────────────────────────────────────
// Account menu response
// ─────────────────────────────────────────────

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct AccountMenuResponse {
    pub actions: Option<Vec<AccountMenuAction>>,
}

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct AccountMenuAction {
    pub open_popup_action: Option<OpenPopupAction>,
}

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct OpenPopupAction {
    pub popup: Option<AccountMenuPopup>,
}

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct AccountMenuPopup {
    pub multi_page_menu_renderer: Option<MultiPageMenuRenderer>,
}

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct MultiPageMenuRenderer {
    pub header: Option<AccountMenuHeader>,
}

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct AccountMenuHeader {
    pub active_account_header_renderer: Option<ActiveAccountHeaderRenderer>,
}

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ActiveAccountHeaderRenderer {
    pub account_name: Option<common::Runs>,
    pub account_photo: Option<common::Thumbnails>,
    pub email: Option<SimpleText>,
}

// ─────────────────────────────────────────────
// Transcript response
// ─────────────────────────────────────────────

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct GetTranscriptResponse {
    pub actions: Option<Vec<TranscriptAction>>,
}

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct TranscriptAction {
    pub update_engagement_panel_action: Option<UpdateEngagementPanelAction>,
}

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct UpdateEngagementPanelAction {
    pub content: Option<TranscriptPanelContent>,
}

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct TranscriptPanelContent {
    pub transcript_renderer: Option<TranscriptRenderer>,
}

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct TranscriptRenderer {
    pub content: Option<TranscriptContent>,
}

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct TranscriptContent {
    pub transcript_search_panel_renderer: Option<TranscriptSearchPanelRenderer>,
}

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct TranscriptSearchPanelRenderer {
    pub body: Option<TranscriptBody>,
}

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct TranscriptBody {
    pub transcript_segment_list_renderer: Option<TranscriptSegmentListRenderer>,
}

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct TranscriptSegmentListRenderer {
    #[serde(default)]
    pub initial_segments: Vec<TranscriptSegment>,
}

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct TranscriptSegment {
    pub transcript_segment_renderer: Option<TranscriptSegmentRenderer>,
}

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct TranscriptSegmentRenderer {
    pub start_ms: Option<String>,
    pub end_ms: Option<String>,
    pub snippet: Option<common::Runs>,
}

// ─────────────────────────────────────────────
// Shared simple helpers
// ─────────────────────────────────────────────

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct SimpleText {
    pub simple_text: Option<String>,
}
