//! Library browse pages (songs, albums, playlists, artists, podcasts).

use crate::models::YTItem;
use crate::response::common::get_continuation;
use crate::response::BrowseResponse;
use super::{from_grid_item, from_responsive, from_shelf_content};

#[derive(Debug, Clone)]
pub struct LibraryPage {
    pub items: Vec<YTItem>,
    pub continuation: Option<String>,
}

#[derive(Debug, Clone)]
pub struct LibraryContinuationPage {
    pub items: Vec<YTItem>,
    pub continuation: Option<String>,
}

impl LibraryPage {
    /// Parse a library browse response.
    ///
    /// Handles songs (`FEmusic_liked_videos`), albums, playlists, artists, podcasts, and
    /// podcast episodes pages.
    #[must_use]
    pub fn from_browse_response(response: &BrowseResponse) -> Self {
        let mut items = Vec::new();
        let mut continuation = None;

        let section_list = response
            .single_col()
            .and_then(|c| c.first_section_list());

        let Some(sl) = section_list else {
            return Self { items, continuation };
        };

        if continuation.is_none() {
            continuation = sl.continuation();
        }

        for content in &sl.contents {
            // Grid (albums, playlists)
            if let Some(grid) = &content.grid_renderer {
                for item in grid.items.iter().filter_map(from_grid_item) {
                    items.push(item);
                }
                if continuation.is_none() {
                    continuation = get_continuation(&grid.continuations);
                }
            }

            // Shelf (songs, episodes)
            if let Some(shelf) = &content.music_shelf_renderer {
                for c in shelf.contents.iter().flatten().filter_map(from_shelf_content) {
                    items.push(c);
                }
                if continuation.is_none() {
                    continuation = get_continuation(&shelf.continuations);
                }
            }

            // Playlist shelf
            if let Some(shelf) = &content.music_playlist_shelf_renderer {
                for c in shelf.contents.iter().filter_map(from_shelf_content) {
                    items.push(c);
                }
                if continuation.is_none() {
                    continuation = get_continuation(&shelf.continuations);
                }
            }

            // Carousel (used in some library sections)
            if let Some(carousel) = &content.music_carousel_shelf_renderer {
                for c in carousel.contents.iter().filter_map(super::from_carousel_content) {
                    items.push(c);
                }
            }

            // ItemSectionRenderer wrapper (some library pages wrap content here)
            if let Some(item_sec) = &content.item_section_renderer {
                for inner_content in &item_sec.contents {
                    if let Some(grid) = &inner_content.grid_renderer {
                        for item in grid.items.iter().filter_map(from_grid_item) {
                            items.push(item);
                        }
                        if continuation.is_none() {
                            continuation = get_continuation(&grid.continuations);
                        }
                    }
                    if let Some(shelf) = &inner_content.music_shelf_renderer {
                        for c in shelf.contents.iter().flatten().filter_map(from_shelf_content) {
                            items.push(c);
                        }
                    }
                    if let Some(shelf) = &inner_content.music_playlist_shelf_renderer {
                        for c in shelf.contents.iter().filter_map(from_shelf_content) {
                            items.push(c);
                        }
                    }
                }
            }
        }

        // on_response_received_actions (used for some continuation appends)
        for item in response.appended_items() {
            if let Some(r) = &item.music_responsive_list_item_renderer
                && let Some(ytitem) = from_responsive(r) {
                    items.push(ytitem);
                }
        }

        Self { items, continuation }
    }

    /// Parse a library continuation response.
    #[must_use]
    pub fn from_continuation(response: &BrowseResponse) -> LibraryContinuationPage {
        let mut items = Vec::new();
        let mut continuation = None;

        // Grid continuation
        if let Some(gc) = response
            .continuation_contents
            .as_ref()
            .and_then(|cc| cc.grid_continuation.as_ref())
        {
            for item in gc.items.iter().filter_map(from_grid_item) {
                items.push(item);
            }
            continuation = get_continuation(&gc.continuations);
        }

        // Shelf continuation
        if let Some(sc) = response
            .continuation_contents
            .as_ref()
            .and_then(|cc| cc.music_shelf_continuation.as_ref())
        {
            for c in sc.contents.iter().filter_map(from_shelf_content) {
                items.push(c);
            }
            continuation = get_continuation(&sc.continuations);
        }

        // Playlist shelf continuation
        if let Some(pc) = response
            .continuation_contents
            .as_ref()
            .and_then(|cc| cc.music_playlist_shelf_continuation.as_ref())
        {
            for c in pc.contents.iter().filter_map(from_shelf_content) {
                items.push(c);
            }
            continuation = get_continuation(&pc.continuations);
        }

        // Section list continuation (browse continuation)
        if let Some(slc) = response
            .continuation_contents
            .as_ref()
            .and_then(|cc| cc.section_list_continuation.as_ref())
        {
            for content in &slc.contents {
                if let Some(grid) = &content.grid_renderer {
                    for item in grid.items.iter().filter_map(from_grid_item) {
                        items.push(item);
                    }
                }
                if let Some(shelf) = &content.music_shelf_renderer {
                    for c in shelf.contents.iter().flatten().filter_map(from_shelf_content) {
                        items.push(c);
                    }
                }
            }
            if continuation.is_none() {
                continuation = slc.continuation();
            }
        }

        for item in response.appended_items() {
            if let Some(r) = &item.music_responsive_list_item_renderer
                && let Some(ytitem) = from_responsive(r) {
                    items.push(ytitem);
                }
            if continuation.is_none() {
                continuation = item.continuation();
            }
        }

        LibraryContinuationPage { items, continuation }
    }
}
