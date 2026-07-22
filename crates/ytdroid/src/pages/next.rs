//! Up-next / related tracks page returned by the `next` endpoint.

use crate::error::{Error, Result};
use crate::models::SongItem;
use crate::response::common::get_continuation;
use crate::response::{NextResponse, PlaylistPanelVideoRenderer};
use super::parse_duration;

#[derive(Debug, Clone)]
pub struct NextPage {
    pub items: Vec<NextItem>,
    pub playlist_id: Option<String>,
    pub continuation: Option<String>,
    /// Browse ID for the "Related" tab (tab index 2 in the next response).
    pub related_browse_id: Option<String>,
    /// Radio/mood chips shown above the up-next queue.
    pub chips: Vec<NextChip>,
}

#[derive(Debug, Clone)]
pub struct NextItem {
    pub song: SongItem,
    /// Used to uniquely identify the item within the playlist panel.
    pub playlist_set_video_id: Option<String>,
}

#[derive(Debug, Clone)]
pub struct NextChip {
    pub title: String,
    pub params: Option<String>,
}

#[derive(Debug, Clone)]
pub struct NextContinuationPage {
    pub items: Vec<NextItem>,
    pub continuation: Option<String>,
}

impl NextPage {
    /// Parse a `next` endpoint response.
    ///
    /// # Errors
    ///
    /// Returns [`crate::error::Error::MissingField`] when the queue renderer is absent.
    pub fn from_next_response(response: &NextResponse) -> Result<Self> {
        let queue = queue_renderer(response).ok_or(Error::MissingField {
            field: "playlistPanelRenderer",
        })?;

        let chips = queue
            .sub_header_chip_cloud
            .as_ref()
            .and_then(|sc| sc.chip_cloud_renderer.as_ref())
            .map(|cr| {
                cr.chips
                    .iter()
                    .filter_map(|chip| {
                        let r = &chip.chip_cloud_chip_renderer;
                        let title = r.text.as_ref()?.runs.first()?.text.clone();
                        let params = r
                            .navigation_endpoint
                            .as_ref()
                            .and_then(|e| e.watch_endpoint.as_ref())
                            .and_then(|e| e.params.clone());
                        Some(NextChip { title, params })
                    })
                    .collect()
            })
            .unwrap_or_default();

        let panel = queue
            .content
            .as_ref()
            .and_then(|c| c.playlist_panel_renderer.as_ref())
            .ok_or(Error::MissingField {
                field: "playlistPanelRenderer",
            })?;

        let playlist_id = panel.playlist_id.clone();
        let continuation = get_continuation(&panel.continuations);

        let items = panel
            .contents
            .iter()
            .filter_map(|c| {
                // Unwrap wrapper renderer if present.
                let video = c
                    .playlist_panel_video_renderer
                    .as_ref()
                    .or_else(|| {
                        c.playlist_panel_video_wrapper_renderer
                            .as_ref()
                            .and_then(|w| w.primary_renderer.as_ref())
                            .and_then(|p| p.playlist_panel_video_renderer.as_ref())
                    })?;
                video_to_next_item(video)
            })
            .collect();

        // Tab 2 of the next response holds the "Related" browse endpoint.
        let related_browse_id = response
            .contents
            .as_ref()
            .and_then(|c| c.tabbed_renderer.as_ref())
            .and_then(|t| t.watch_next_tabbed_results_renderer.as_ref())
            .and_then(|r| r.tabs.get(2))
            .and_then(|t| t.tab_renderer.endpoint.as_ref())
            .and_then(|e| e.browse_endpoint.as_ref())
            .and_then(|b| b.browse_id.clone());

        Ok(Self {
            items,
            playlist_id,
            continuation,
            related_browse_id,
            chips,
        })
    }

    /// Parse a continuation of the `next` response.
    #[must_use]
    pub fn from_continuation(response: &NextResponse) -> NextContinuationPage {
        let panel = response
            .continuation_contents
            .as_ref()
            .and_then(|cc| cc.playlist_panel_continuation.as_ref());

        let items = panel
            .map(|p| {
                p.contents
                    .iter()
                    .filter_map(|c| {
                        let video = c
                            .playlist_panel_video_renderer
                            .as_ref()
                            .or_else(|| {
                                c.playlist_panel_video_wrapper_renderer
                                    .as_ref()
                                    .and_then(|w| w.primary_renderer.as_ref())
                                    .and_then(|p| p.playlist_panel_video_renderer.as_ref())
                            })?;
                        video_to_next_item(video)
                    })
                    .collect()
            })
            .unwrap_or_default();

        let continuation = panel.map(|p| get_continuation(&p.continuations)).unwrap_or_default();

        NextContinuationPage { items, continuation }
    }
}

fn queue_renderer(
    response: &NextResponse,
) -> Option<&crate::response::MusicQueueRenderer> {
    response
        .contents
        .as_ref()?
        .tabbed_renderer
        .as_ref()?
        .watch_next_tabbed_results_renderer
        .as_ref()?
        .tabs
        .first()?
        .tab_renderer
        .content
        .as_ref()?
        .music_queue_renderer
        .as_ref()
}

fn video_to_next_item(video: &PlaylistPanelVideoRenderer) -> Option<NextItem> {
    let id = video.video_id.clone()?;
    let title = video.title.as_ref()?.runs.first()?.text.clone();
    let thumbnail = video
        .thumbnail
        .as_ref()
        .and_then(|t| t.thumbnails.last())
        .map(|t| t.url.clone());
    let artists = video
        .long_by_line_text
        .as_ref()
        .map(|r| {
            r.odd_elements()
                .filter(|run| {
                    run.navigation_endpoint
                        .as_ref()
                        .and_then(|e| e.browse_endpoint.as_ref())
                        .is_some_and(super::super::response::common::BrowseEndpoint::is_artist_endpoint)
                })
                .map(|run| crate::models::Artist {
                    name: run.text.clone(),
                    id: run
                        .navigation_endpoint
                        .as_ref()
                        .and_then(|e| e.browse_endpoint.as_ref())
                        .and_then(|b| b.browse_id.clone()),
                })
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    let duration = video
        .length_text
        .as_ref()
        .and_then(|r| r.runs.first())
        .and_then(|r| parse_duration(&r.text));
    let playlist_set_video_id = video.playlist_set_video_id.clone();
    Some(NextItem {
        song: SongItem {
            id,
            title,
            artists,
            album: None,
            duration,
            thumbnail,
            explicit: false,
            set_video_id: playlist_set_video_id.clone(),
            like_status: None,
            library_add_token: None,
            library_remove_token: None,
            music_video_type: None,
        },
        playlist_set_video_id,
    })
}
