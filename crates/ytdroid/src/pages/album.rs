//! Album browse page.

use crate::error::{Error, Result};
use crate::models::{AlbumItem, Artist, SongItem, YTItem};
use crate::response::common::get_continuation;
use crate::response::BrowseResponse;
use super::from_responsive;

#[derive(Debug, Clone)]
pub struct AlbumPage {
    pub album: AlbumItem,
    pub description: Option<String>,
    pub songs: Vec<SongItem>,
    pub songs_continuation: Option<String>,
}

impl AlbumPage {
    /// Parse an album browse response.
    ///
    /// Album pages use a `MusicDetailHeaderRenderer` or `MusicResponsiveHeaderRenderer`
    /// for metadata and a `MusicShelfRenderer` / `MusicPlaylistShelfRenderer` for tracks.
    ///
    /// # Errors
    ///
    /// Returns [`crate::error::Error::MissingField`] when the album header is absent.
    #[allow(clippy::too_many_lines)] // Complex nested JSON structure; decomposition would obscure the parsing logic.
    pub fn from_browse_response(response: &BrowseResponse) -> Result<Self> {
        let header = response.header.as_ref();

        // ── Header ──────────────────────────────────────────
        let (title, thumbnail, artists, year, explicit, description, playlist_id) =
            if let Some(dh) = header.and_then(|h| h.music_detail_header_renderer.as_ref()) {
                let title = dh.title.text();
                let thumbnail = dh.thumbnail.as_ref().and_then(|t| t.get_url()).map(str::to_owned);
                // subtitle format: "Type • Year • Artists"
                // or sometimes different orderings
                let subtitle_runs = &dh.subtitle.runs;
                let artists = subtitle_runs
                    .iter()
                    .filter(|r| {
                        r.navigation_endpoint
                            .as_ref()
                            .and_then(|e| e.browse_endpoint.as_ref())
                            .is_some_and(super::super::response::common::BrowseEndpoint::is_artist_endpoint)
                    })
                    .map(|r| Artist {
                        name: r.text.clone(),
                        id: r
                            .navigation_endpoint
                            .as_ref()
                            .and_then(|e| e.browse_endpoint.as_ref())
                            .and_then(|b| b.browse_id.clone()),
                    })
                    .collect();
                // Year = first run that's numeric and has no browse endpoint.
                let year = subtitle_runs
                    .iter()
                    .find(|r| r.navigation_endpoint.is_none() && r.text.parse::<u32>().is_ok())
                    .map(|r| r.text.clone());
                let description = dh.description.as_ref().map(super::super::response::common::Runs::text);
                let explicit = false; // MusicDetailHeaderRenderer doesn't have badges
                let playlist_id = dh
                    .menu
                    .as_ref()
                    .and_then(|m| m.menu_renderer.as_ref())
                    .and_then(|mr| mr.nav_item_by_icon("QUEUE_PLAY_NEXT"))
                    .and_then(|nav| nav.navigation_endpoint.as_ref())
                    .and_then(|e| e.watch_endpoint.as_ref())
                    .and_then(|e| e.playlist_id.clone());
                (title, thumbnail, artists, year, explicit, description, playlist_id)
            } else if let Some(rh) = response
                .single_col()
                .and_then(|sc| sc.first_section_list())
                .and_then(|sl| {
                    sl.contents
                        .iter()
                        .find_map(|c| c.music_responsive_header_renderer.as_ref())
                })
            {
                let title = rh.title.as_ref().map(super::super::response::common::Runs::text).unwrap_or_default();
                let thumbnail = rh.thumbnail.as_ref().and_then(|t| t.get_url()).map(str::to_owned);
                // strapline_text_one = "Album" / "Single" / "EP"
                // subtitle = "Year"
                let year = rh
                    .subtitle
                    .as_ref()
                    .and_then(|r| r.runs.first())
                    .map(|r| r.text.clone());
                let artists: Vec<Artist> = rh
                    .strapline_text_one
                    .as_ref()
                    .map(|r| {
                        r.odd_elements()
                            .filter(|run| {
                                run.navigation_endpoint
                                    .as_ref()
                                    .and_then(|e| e.browse_endpoint.as_ref())
                                    .is_some_and(super::super::response::common::BrowseEndpoint::is_artist_endpoint)
                            })
                            .map(|run| Artist {
                                name: run.text.clone(),
                                id: run
                                    .navigation_endpoint
                                    .as_ref()
                                    .and_then(|e| e.browse_endpoint.as_ref())
                                    .and_then(|b| b.browse_id.clone()),
                            })
                            .collect()
                    })
                    .unwrap_or_default();
                let description = rh
                    .description
                    .as_ref()
                    .and_then(|d| d.description.as_ref())
                    .map(super::super::response::common::Runs::text);
                let explicit = false;
                let playlist_id = None;
                (title, thumbnail, artists, year, explicit, description, playlist_id)
            } else {
                return Err(Error::MissingField { field: "album header" });
            };

        let id = String::new(); // caller fills from the browse ID used

        // ── Song list ────────────────────────────────────────
        let section_list = response
            .two_col()
            .and_then(|tc| tc.secondary_section_list())
            .or_else(|| response.single_col()?.first_section_list());

        let mut songs = Vec::new();
        let mut songs_continuation = None;

        if let Some(sl) = section_list {
            songs_continuation = sl.continuation();
            for content in &sl.contents {
                let shelf = content
                    .music_playlist_shelf_renderer
                    .as_ref()
                    .map(|s| (s.contents.as_slice(), get_continuation(&s.continuations)))
                    .or_else(|| {
                        content.music_shelf_renderer.as_ref().map(|s| {
                            (
                                s.contents.as_deref().unwrap_or_default(),
                                get_continuation(&s.continuations),
                            )
                        })
                    });
                if let Some((shelf_contents, shelf_cont)) = shelf {
                    for c in shelf_contents {
                        if let Some(YTItem::Song(song)) =
                            c.music_responsive_list_item_renderer.as_ref().and_then(from_responsive)
                        {
                            songs.push(*song);
                        }
                    }
                    if songs_continuation.is_none() {
                        songs_continuation = shelf_cont;
                    }
                }
            }
        }

        Ok(Self {
            album: AlbumItem {
                id,
                playlist_id,
                title,
                artists,
                year,
                thumbnail,
                explicit,
            },
            description,
            songs,
            songs_continuation,
        })
    }
}
