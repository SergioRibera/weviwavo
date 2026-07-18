//! Page parsers: convert raw Innertube response types into domain [`crate::models`].

pub mod album;
pub mod artist;
pub mod explore;
pub mod home;
pub mod library;
pub mod next;
pub mod playlist;
pub mod search;

use crate::models::{
    AlbumItem, AlbumRef, Artist, ArtistItem, EpisodeItem, PlaylistItem, PodcastItem, PodcastRef,
    SongItem, YTItem,
};
use crate::response::common::{
    MusicMultiRowListItemRenderer, MusicResponsiveListItemRenderer, MusicTwoRowItemRenderer,
    PAGE_TYPE_ALBUM, PAGE_TYPE_PODCAST_SHOW,
};

// ─────────────────────────────────────────────
// Shared helpers
// ─────────────────────────────────────────────

/// Parse `"mm:ss"` or `"h:mm:ss"` duration text into total seconds.
pub(crate) fn parse_duration(text: &str) -> Option<u32> {
    let parts: Vec<&str> = text.trim().split(':').collect();
    match parts.as_slice() {
        [m, s] => Some(m.parse::<u32>().ok()? * 60 + s.parse::<u32>().ok()?),
        [h, m, s] => Some(
            h.parse::<u32>().ok()? * 3600
                + m.parse::<u32>().ok()? * 60
                + s.parse::<u32>().ok()?,
        ),
        _ => None,
    }
}

// ─────────────────────────────────────────────
// MusicTwoRowItemRenderer → YTItem
// ─────────────────────────────────────────────

/// Convert a `MusicTwoRowItemRenderer` (used in carousels, grids, search) to a [`YTItem`].
///
/// Returns `None` when the item type cannot be determined or required fields are missing.
#[allow(clippy::too_many_lines)] // Complex dispatch over all item types; splitting would obscure the logic.
pub(crate) fn from_two_row(r: &MusicTwoRowItemRenderer) -> Option<YTItem> {
    let title = r.title.runs.first()?.text.clone();
    let thumbnail = r.thumbnail_url().map(str::to_owned);
    let explicit = r.is_explicit();

    // Episode must be checked before Song because episodes also satisfy is_song().
    if r.is_episode() {
        let id = r
            .navigation_endpoint
            .browse_endpoint
            .as_ref()?
            .browse_id
            .clone()?;
        let subtitle = r.subtitle.as_ref();
        let groups = subtitle
            .map(|s| s.split_by_separator())
            .unwrap_or_default();
        let podcast = groups.first().and_then(|runs| runs.first()).and_then(|run| {
            let b = run
                .navigation_endpoint
                .as_ref()?
                .browse_endpoint
                .as_ref()?;
            if b.page_type() == Some(PAGE_TYPE_PODCAST_SHOW) {
                Some(PodcastRef {
                    name: run.text.clone(),
                    id: b.browse_id.clone()?,
                })
            } else {
                None
            }
        });
        let date = groups
            .get(1)
            .and_then(|runs| runs.first())
            .map(|r| r.text.clone());
        return Some(YTItem::Episode(EpisodeItem {
            id,
            title,
            podcast,
            date,
            duration: None,
            thumbnail,
            explicit,
            library_add_token: None,
            library_remove_token: None,
        }));
    }

    if r.is_album() {
        let id = r
            .navigation_endpoint
            .browse_endpoint
            .as_ref()?
            .browse_id
            .clone()?;
        let subtitle = r.subtitle.as_ref();
        let artists: Vec<Artist> = subtitle
            .map(|s| {
                s.odd_elements()
                    .filter(|run| {
                        run.navigation_endpoint
                            .as_ref()
                            .and_then(|e| e.browse_endpoint.as_ref())
                            .is_some_and(super::response::common::BrowseEndpoint::is_artist_endpoint)
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
        // Year = the last odd-element run that has no browse endpoint.
        let year: Option<String> = subtitle.and_then(|s| {
            s.odd_elements()
                .filter(|run| {
                    run.navigation_endpoint
                        .as_ref()
                        .and_then(|e| e.browse_endpoint.as_ref())
                        .is_none()
                })
                .last()
                .map(|run| run.text.clone())
        });
        let playlist_id = r.play_endpoint().and_then(|e| e.playlist_id.clone());
        return Some(YTItem::Album(AlbumItem {
            id,
            playlist_id,
            title,
            artists,
            year,
            thumbnail,
            explicit,
        }));
    }

    if r.is_playlist() {
        let id = r
            .navigation_endpoint
            .browse_endpoint
            .as_ref()
            .and_then(|b| b.browse_id.clone())
            .or_else(|| {
                r.play_endpoint()
                    .and_then(|e| e.playlist_id.as_ref())
                    .map(|pid| format!("VL{pid}"))
            })?;
        let subtitle = r.subtitle.as_ref();
        let author = subtitle.and_then(|s| s.runs.first()).map(|r| r.text.clone());
        let song_count_text = subtitle.and_then(|s| s.runs.get(2)).map(|r| r.text.clone());
        let (library_add_token, library_remove_token) = r
            .menu
            .as_ref()
            .and_then(|m| m.menu_renderer.as_ref())
            .map(super::response::common::MenuRenderer::library_tokens)
            .unwrap_or_default();
        return Some(YTItem::Playlist(PlaylistItem {
            id,
            title,
            author,
            song_count_text,
            thumbnail,
            library_add_token,
            library_remove_token,
        }));
    }

    if r.is_artist() || r.is_user_channel() {
        let id = r
            .navigation_endpoint
            .browse_endpoint
            .as_ref()?
            .browse_id
            .clone()?;
        let subscribers = r
            .subtitle
            .as_ref()
            .and_then(|s| s.runs.first())
            .map(|r| r.text.clone());
        return Some(YTItem::Artist(ArtistItem {
            id,
            title,
            subscribers,
            thumbnail,
            channel_ids: Vec::new(),
        }));
    }

    if r.is_podcast() {
        let id = r
            .navigation_endpoint
            .browse_endpoint
            .as_ref()?
            .browse_id
            .clone()?;
        let author = r
            .subtitle
            .as_ref()
            .and_then(|s| s.runs.first())
            .map(|r| r.text.clone());
        return Some(YTItem::Podcast(PodcastItem {
            id,
            title,
            author,
            thumbnail,
        }));
    }

    // Default: Song
    if r.is_song() {
        let id = r
            .navigation_endpoint
            .any_video_id()
            .or_else(|| r.play_watch_endpoint().and_then(|e| e.video_id.as_deref()))?
            .to_owned();
        let artists: Vec<Artist> = r
            .subtitle
            .as_ref()
            .map(|s| {
                s.odd_elements()
                    .filter(|run| {
                        run.navigation_endpoint
                            .as_ref()
                            .and_then(|e| e.browse_endpoint.as_ref())
                            .is_some_and(super::response::common::BrowseEndpoint::is_artist_endpoint)
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
        return Some(YTItem::Song(Box::new(SongItem {
            id,
            title,
            artists,
            album: None,
            duration: None,
            thumbnail,
            explicit,
            set_video_id: None,
            like_status: None,
            library_add_token: None,
            library_remove_token: None,
            music_video_type: None,
        })));
    }

    None
}

// ─────────────────────────────────────────────
// MusicResponsiveListItemRenderer → YTItem
// ─────────────────────────────────────────────

/// Convert a `MusicResponsiveListItemRenderer` (search, library, playlist songs) to a [`YTItem`].
///
/// **Critical:** episode check comes before song check — episodes satisfy `is_song()` too.
#[allow(clippy::too_many_lines)] // Complex dispatch over all item types; splitting would obscure the logic.
pub(crate) fn from_responsive(r: &MusicResponsiveListItemRenderer) -> Option<YTItem> {
    let title = r
        .flex_columns
        .first()?
        .runs()?
        .runs
        .first()?
        .text
        .clone();
    let thumbnail = r.thumbnail_url().map(str::to_owned);
    let explicit = r.is_explicit();

    // Check isEpisode BEFORE isSong (critical ordering from Metrolist SearchPage.kt).
    if r.is_episode() {
        let id = r.video_id()?.to_owned();
        let col1 = r.flex_columns.get(1).and_then(|c| c.runs());
        let groups = col1
            .map(|runs| runs.split_by_separator())
            .unwrap_or_default();
        let podcast = groups.first().and_then(|runs| runs.first()).and_then(|run| {
            let b = run
                .navigation_endpoint
                .as_ref()?
                .browse_endpoint
                .as_ref()?;
            if b.page_type() == Some(PAGE_TYPE_PODCAST_SHOW) {
                Some(PodcastRef {
                    name: run.text.clone(),
                    id: b.browse_id.clone()?,
                })
            } else {
                None
            }
        });
        let date = groups
            .get(1)
            .and_then(|runs| runs.first())
            .map(|r| r.text.clone());
        let (library_add_token, library_remove_token) = r.library_tokens();
        return Some(YTItem::Episode(EpisodeItem {
            id,
            title,
            podcast,
            date,
            duration: None,
            thumbnail,
            explicit,
            library_add_token,
            library_remove_token,
        }));
    }

    if r.is_song() {
        let id = r.video_id()?.to_owned();
        let col1 = r.flex_columns.get(1).and_then(|c| c.runs());
        let artists: Vec<Artist> = col1
            .map(|runs| {
                runs.odd_elements()
                    .filter(|run| {
                        run.navigation_endpoint
                            .as_ref()
                            .and_then(|e| e.browse_endpoint.as_ref())
                            .is_some_and(super::response::common::BrowseEndpoint::is_artist_endpoint)
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
        let album = r.flex_columns.get(2).and_then(|c| c.runs()).and_then(|runs| {
            let run = runs.runs.first()?;
            let b = run
                .navigation_endpoint
                .as_ref()?
                .browse_endpoint
                .as_ref()?;
            if b.page_type() == Some(PAGE_TYPE_ALBUM) {
                Some(AlbumRef {
                    name: run.text.clone(),
                    id: b.browse_id.clone()?,
                })
            } else {
                None
            }
        });
        let duration = r.duration_text().and_then(parse_duration);
        let set_video_id = r.playlist_set_video_id.clone();
        let music_video_type = r.music_video_type.clone();
        let (library_add_token, library_remove_token) = r.library_tokens();
        return Some(YTItem::Song(Box::new(SongItem {
            id,
            title,
            artists,
            album,
            duration,
            thumbnail,
            explicit,
            set_video_id,
            like_status: None,
            library_add_token,
            library_remove_token,
            music_video_type,
        })));
    }

    if r.is_album() {
        let id = r
            .navigation_endpoint
            .as_ref()?
            .browse_endpoint
            .as_ref()?
            .browse_id
            .clone()?;
        let col1 = r.flex_columns.get(1).and_then(|c| c.runs());
        let artists: Vec<Artist> = col1
            .map(|runs| {
                runs.odd_elements()
                    .filter(|run| {
                        run.navigation_endpoint
                            .as_ref()
                            .and_then(|e| e.browse_endpoint.as_ref())
                            .is_some_and(super::response::common::BrowseEndpoint::is_artist_endpoint)
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
        let year = r
            .flex_columns
            .get(2)
            .and_then(|c| c.runs())
            .and_then(|runs| runs.runs.first())
            .map(|r| r.text.clone());
        let playlist_id = r.play_playlist_endpoint().and_then(|e| e.playlist_id.clone());
        return Some(YTItem::Album(AlbumItem {
            id,
            playlist_id,
            title,
            artists,
            year,
            thumbnail,
            explicit,
        }));
    }

    if r.is_playlist() {
        let id = r
            .navigation_endpoint
            .as_ref()?
            .browse_endpoint
            .as_ref()?
            .browse_id
            .clone()?;
        let author = r
            .flex_columns
            .get(1)
            .and_then(|c| c.runs())
            .and_then(|runs| runs.runs.first())
            .map(|r| r.text.clone());
        let song_count_text = r
            .flex_columns
            .get(2)
            .and_then(|c| c.runs())
            .and_then(|runs| runs.runs.first())
            .map(|r| r.text.clone());
        let (library_add_token, library_remove_token) = r.library_tokens();
        return Some(YTItem::Playlist(PlaylistItem {
            id,
            title,
            author,
            song_count_text,
            thumbnail,
            library_add_token,
            library_remove_token,
        }));
    }

    if r.is_artist() || r.is_user_channel() {
        let id = r
            .navigation_endpoint
            .as_ref()?
            .browse_endpoint
            .as_ref()?
            .browse_id
            .clone()?;
        let subscribers = r
            .flex_columns
            .get(1)
            .and_then(|c| c.runs())
            .and_then(|runs| runs.runs.first())
            .map(|r| r.text.clone());
        return Some(YTItem::Artist(ArtistItem {
            id,
            title,
            subscribers,
            thumbnail,
            channel_ids: Vec::new(),
        }));
    }

    if r.is_podcast() {
        let id = r
            .navigation_endpoint
            .as_ref()?
            .browse_endpoint
            .as_ref()?
            .browse_id
            .clone()?;
        let author = r
            .flex_columns
            .get(1)
            .and_then(|c| c.runs())
            .and_then(|runs| runs.runs.first())
            .map(|r| r.text.clone());
        return Some(YTItem::Podcast(PodcastItem {
            id,
            title,
            author,
            thumbnail,
        }));
    }

    None
}

// ─────────────────────────────────────────────
// MusicMultiRowListItemRenderer → YTItem (podcast episodes in shelf)
// ─────────────────────────────────────────────

/// Convert a `MusicMultiRowListItemRenderer` (podcast episodes in carousel/shelf) to a [`YTItem`].
pub(crate) fn from_multi_row(r: &MusicMultiRowListItemRenderer) -> Option<YTItem> {
    let id = r.video_id()?.to_owned();
    let title = r.title.as_ref()?.runs.first()?.text.clone();
    let thumbnail = r.thumbnail_url().map(str::to_owned);
    let podcast = r.subtitle.as_ref().and_then(|s| {
        let run = s.runs.first()?;
        let b = run
            .navigation_endpoint
            .as_ref()?
            .browse_endpoint
            .as_ref()?;
        if b.page_type() == Some(PAGE_TYPE_PODCAST_SHOW) {
            Some(PodcastRef {
                name: run.text.clone(),
                id: b.browse_id.clone()?,
            })
        } else {
            None
        }
    });
    // second_subtitle format: "Duration · Date"
    let second = r.second_subtitle.as_ref();
    let groups = second
        .map(|s| s.split_by_separator())
        .unwrap_or_default();
    let duration = groups
        .first()
        .and_then(|runs| runs.first())
        .and_then(|r| parse_duration(&r.text));
    let date = groups
        .get(1)
        .and_then(|runs| runs.first())
        .map(|r| r.text.clone());
    let (library_add_token, library_remove_token) = r.library_tokens();
    Some(YTItem::Episode(EpisodeItem {
        id,
        title,
        podcast,
        date,
        duration,
        thumbnail,
        explicit: false,
        library_add_token,
        library_remove_token,
    }))
}

// ─────────────────────────────────────────────
// Carousel / shelf item dispatch
// ─────────────────────────────────────────────

/// Convert any carousel shelf content slot into a [`YTItem`].
pub(crate) fn from_carousel_content(
    c: &crate::response::common::CarouselShelfContent,
) -> Option<YTItem> {
    if let Some(r) = &c.music_two_row_item_renderer {
        return from_two_row(r);
    }
    if let Some(r) = &c.music_responsive_list_item_renderer {
        return from_responsive(r);
    }
    if let Some(r) = &c.music_multi_row_list_item_renderer {
        return from_multi_row(r);
    }
    None
}

/// Convert any shelf content slot into a [`YTItem`].
pub(crate) fn from_shelf_content(c: &crate::response::common::ShelfContent) -> Option<YTItem> {
    if let Some(r) = &c.music_responsive_list_item_renderer {
        return from_responsive(r);
    }
    if let Some(r) = &c.music_multi_row_list_item_renderer {
        return from_multi_row(r);
    }
    None
}

/// Convert a grid renderer item into a [`YTItem`].
pub(crate) fn from_grid_item(item: &crate::response::common::GridRendererItem) -> Option<YTItem> {
    item.music_two_row_item_renderer
        .as_ref()
        .and_then(from_two_row)
}

