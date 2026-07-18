//! Playlist browse page (user playlists and auto-generated playlists).

use crate::error::{Error, Result};
use crate::models::{PlaylistItem, SongItem, YTItem};
use crate::response::common::get_continuation;
use crate::response::BrowseResponse;
use super::from_responsive;

#[derive(Debug, Clone)]
pub struct PlaylistPage {
    pub playlist: PlaylistItem,
    pub songs: Vec<SongItem>,
    /// Continuation token for the first song batch (when the playlist is large).
    pub songs_continuation: Option<String>,
}

#[derive(Debug, Clone)]
pub struct PlaylistContinuationPage {
    pub songs: Vec<SongItem>,
    pub continuation: Option<String>,
}

impl PlaylistPage {
    /// Parse a playlist / album browse response.
    ///
    /// Reads the header for playlist metadata and the shelf for song items.
    ///
    /// # Errors
    ///
    /// Returns [`crate::error::Error::MissingField`] on structural mismatch.
    pub fn from_browse_response(response: &BrowseResponse) -> Result<Self> {
        // ── Header ──────────────────────────────────────────
        // Responsive/editable headers live in section list contents; detail header is page-level.
        let section_header = response
            .single_col()
            .and_then(|sc| sc.first_section_list())
            .and_then(|sl| {
                sl.contents.iter().find(|c| {
                    c.music_responsive_header_renderer.is_some()
                        || c.music_editable_playlist_detail_header_renderer.is_some()
                })
            });

        let page_header = response.header.as_ref();

        let (title, thumbnail, author, song_count_text) = if let Some(rh) =
            section_header.and_then(|c| c.music_responsive_header_renderer.as_ref())
        {
            let title = rh.title.as_ref().map(super::super::response::common::Runs::text).unwrap_or_default();
            let thumbnail = rh.thumbnail.as_ref().and_then(|t| t.get_url()).map(str::to_owned);
            let author = rh
                .strapline_text_one
                .as_ref()
                .and_then(|r| r.runs.first())
                .map(|r| r.text.clone());
            let song_count_text = rh.second_subtitle.as_ref().map(super::super::response::common::Runs::text);
            (title, thumbnail, author, song_count_text)
        } else if let Some(eh) = section_header
            .and_then(|c| c.music_editable_playlist_detail_header_renderer.as_ref())
            .and_then(|eh| eh.header.as_ref())
        {
            // editable playlist (liked songs, etc.)
            let (title, thumbnail, author, song_count_text) = if let Some(rh) =
                &eh.music_responsive_header_renderer
            {
                let title = rh.title.as_ref().map(super::super::response::common::Runs::text).unwrap_or_default();
                let thumbnail = rh.thumbnail.as_ref().and_then(|t| t.get_url()).map(str::to_owned);
                let author = rh
                    .strapline_text_one
                    .as_ref()
                    .and_then(|r| r.runs.first())
                    .map(|r| r.text.clone());
                let song_count_text = rh.second_subtitle.as_ref().map(super::super::response::common::Runs::text);
                (title, thumbnail, author, song_count_text)
            } else if let Some(dh) = &eh.music_detail_header_renderer {
                let title = dh.title.text();
                let thumbnail = dh.thumbnail.as_ref().and_then(|t| t.get_url()).map(str::to_owned);
                let author = dh.subtitle.runs.first().map(|r| r.text.clone());
                (title, thumbnail, author, None)
            } else {
                return Err(Error::MissingField { field: "playlist header" });
            };
            (title, thumbnail, author, song_count_text)
        } else if let Some(dh) = page_header.and_then(|h| h.music_detail_header_renderer.as_ref())
        {
            let title = dh.title.text();
            let thumbnail = dh.thumbnail.as_ref().and_then(|t| t.get_url()).map(str::to_owned);
            let author = dh.subtitle.runs.first().map(|r| r.text.clone());
            (title, thumbnail, author, None)
        } else {
            return Err(Error::MissingField { field: "playlist header" });
        };

        // ── Song list ────────────────────────────────────────
        // The song list is in two_col secondary OR single_col section list.
        let (songs, songs_continuation) =
            extract_songs_from_response(response).unwrap_or_default();

        // Build a browse ID from the URL canonical (microformat) or fall back to empty.
        let browse_id = response
            .microformat
            .as_ref()
            .and_then(|m| m.microformat_data_renderer.as_ref())
            .and_then(|m| m.url_canonical.as_ref())
            .and_then(|url| {
                // e.g. "https://music.youtube.com/playlist?list=PLxx..."
                url.split("list=").nth(1).map(|id| format!("VL{id}"))
            })
            .unwrap_or_default();

        Ok(Self {
            playlist: PlaylistItem {
                id: browse_id,
                title,
                author,
                song_count_text,
                thumbnail,
                library_add_token: None,
                library_remove_token: None,
            },
            songs,
            songs_continuation,
        })
    }

    /// Parse a playlist songs continuation.
    #[must_use]
    pub fn from_continuation(response: &BrowseResponse) -> PlaylistContinuationPage {
        let (songs, continuation) = extract_songs_from_response(response).unwrap_or_default();
        PlaylistContinuationPage { songs, continuation }
    }
}

fn extract_songs_from_response(
    response: &BrowseResponse,
) -> Option<(Vec<SongItem>, Option<String>)> {
    // Try two-column layout (playlist/album page).
    let section_list = response
        .two_col()
        .and_then(|tc| tc.secondary_section_list())
        .or_else(|| response.single_col()?.first_section_list())
        .or_else(|| response.two_col()?.first_tab_section_list())?;

    let mut songs = Vec::new();
    let mut continuation = section_list.continuation();

    for content in &section_list.contents {
        let shelf = content
            .music_playlist_shelf_renderer
            .as_ref()
            .map(|s| s.contents.as_slice())
            .or_else(|| {
                content
                    .music_shelf_renderer
                    .as_ref()
                    .and_then(|s| s.contents.as_deref())
            });
        if let Some(shelf_contents) = shelf {
            for c in shelf_contents {
                if let Some(YTItem::Song(song)) =
                    c.music_responsive_list_item_renderer.as_ref().and_then(from_responsive)
                {
                    songs.push(*song);
                }
            }
            // The continuation may also be embedded inside the shelf.
            if continuation.is_none() {
                let shelf_cont = content
                    .music_playlist_shelf_renderer
                    .as_ref()
                    .map(|s| get_continuation(&s.continuations))
                    .or_else(|| {
                        content
                            .music_shelf_renderer
                            .as_ref()
                            .map(|s| get_continuation(&s.continuations))
                    });
                continuation = shelf_cont.flatten();
            }
        }
    }

    // Also check on_response_received_actions (used by some continuation forms).
    for item in response.appended_items() {
        if let Some(r) = &item.music_responsive_list_item_renderer
            && let Some(YTItem::Song(song)) = from_responsive(r) {
                songs.push(*song);
            }
        if let Some(token) = item.continuation()
            && continuation.is_none() {
                continuation = Some(token);
            }
    }

    Some((songs, continuation))
}
