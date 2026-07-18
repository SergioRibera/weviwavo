//! Artist browse page and artist items sub-pages.

use crate::error::{Error, Result};
use crate::models::{ArtistItem, YTItem};
use crate::response::common::get_continuation;
use crate::response::BrowseResponse;
use super::{from_carousel_content, from_shelf_content};

/// Full artist page — header + content sections.
#[derive(Debug, Clone)]
pub struct ArtistPage {
    pub artist: ArtistItem,
    pub description: Option<String>,
    pub sections: Vec<ArtistSection>,
    /// Monthly listener count text (e.g. `"1,234,567 monthly listeners"`).
    pub monthly_listeners: Option<String>,
    /// Subscribe button channel IDs (for the subscribe / unsubscribe call).
    pub channel_ids: Vec<String>,
    pub subscribed: bool,
}

#[derive(Debug, Clone)]
pub struct ArtistSection {
    pub title: Option<String>,
    pub items: Vec<YTItem>,
    /// `browseId` for the "Show all" / "See discography" endpoint.
    pub more_browse_id: Option<String>,
    /// `params` for the more endpoint.
    pub more_browse_params: Option<String>,
}

/// Sub-page of artist items (songs, albums, singles, etc.) from the "Show all" button.
#[derive(Debug, Clone)]
pub struct ArtistItemsPage {
    pub title: Option<String>,
    pub items: Vec<YTItem>,
    pub continuation: Option<String>,
}

/// Continuation of an artist items page.
#[derive(Debug, Clone)]
pub struct ArtistItemsContinuationPage {
    pub items: Vec<YTItem>,
    pub continuation: Option<String>,
}

impl ArtistPage {
    /// Parse an artist browse response.
    ///
    /// # Errors
    ///
    /// Returns [`crate::error::Error::MissingField`] when the artist header is absent.
    pub fn from_browse_response(response: &BrowseResponse) -> Result<Self> {
        // ── Header ──────────────────────────────────────────
        let header = response.header.as_ref();

        let (title, thumbnail, description, monthly_listeners, channel_ids, subscribed) =
            if let Some(imm) =
                header.and_then(|h| h.music_immersive_header_renderer.as_ref())
            {
                let title = imm.title.as_ref().map(super::super::response::common::Runs::text).unwrap_or_default();
                let thumbnail = imm.thumbnail.as_ref().and_then(|t| t.get_url()).map(str::to_owned);
                let description = imm.description.as_ref().map(super::super::response::common::Runs::text);
                let monthly_listeners = imm.monthly_listener_count.as_ref().map(super::super::response::common::Runs::text);
                let (channel_ids, subscribed) = extract_subscribe_info(
                    imm.subscription_button
                        .as_ref()
                        .or(imm.subscription_button2.as_ref()),
                );
                (title, thumbnail, description, monthly_listeners, channel_ids, subscribed)
            } else if let Some(vis) =
                header.and_then(|h| h.music_visual_header_renderer.as_ref())
            {
                let title = vis.title.as_ref().map(super::super::response::common::Runs::text).unwrap_or_default();
                let thumbnail = vis
                    .foreground_thumbnail
                    .as_ref()
                    .and_then(|t| t.get_url())
                    .map(str::to_owned);
                let (channel_ids, subscribed) =
                    extract_subscribe_info(vis.subscription_button.as_ref());
                (title, thumbnail, None, None, channel_ids, subscribed)
            } else {
                return Err(Error::MissingField {
                    field: "artist header",
                });
            };

        let artist = ArtistItem {
            id: String::new(), // caller fills this from the browse ID they used
            title,
            subscribers: None,
            thumbnail,
            channel_ids: channel_ids.clone(),
        };

        // ── Content sections ────────────────────────────────
        let section_list = response
            .single_col()
            .and_then(|c| c.first_section_list())
            .or_else(|| response.two_col()?.first_tab_section_list());

        let sections = section_list
            .map(|sl| {
                sl.contents
                    .iter()
                    .filter_map(|content| {
                        let carousel = content.music_carousel_shelf_renderer.as_ref()?;
                        let items: Vec<YTItem> = carousel
                            .contents
                            .iter()
                            .filter_map(from_carousel_content)
                            .collect();
                        if items.is_empty() {
                            return None;
                        }
                        let more_endpoint = carousel.more_browse_id().and_then(|_| {
                            carousel
                                .header
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
                                .as_ref()
                                .map(|b| (b.browse_id.clone(), b.params.clone()))
                        });
                        Some(ArtistSection {
                            title: carousel.title().map(str::to_owned),
                            items,
                            more_browse_id: more_endpoint.as_ref().and_then(|e| e.0.clone()),
                            more_browse_params: more_endpoint.and_then(|e| e.1),
                        })
                    })
                    .collect()
            })
            .unwrap_or_default();

        Ok(Self {
            artist,
            description,
            sections,
            monthly_listeners,
            channel_ids,
            subscribed,
        })
    }
}

impl ArtistItemsPage {
    /// Parse an artist items sub-page (songs, albums, singles, videos).
    #[must_use]
    pub fn from_browse_response(response: &BrowseResponse) -> Self {
        let section_list = response
            .single_col()
            .and_then(|c| c.first_section_list());

        let Some(sl) = section_list else {
            return Self { title: None, items: Vec::new(), continuation: None };
        };

        let mut title = None;
        let mut items = Vec::new();
        let mut continuation = sl.continuation();

        for content in &sl.contents {
            if let Some(grid) = &content.grid_renderer {
                if title.is_none() {
                    title = grid
                        .header
                        .as_ref()
                        .and_then(|h| h.grid_header_renderer.as_ref())
                        .and_then(|h| h.title.as_ref())
                        .map(super::super::response::common::Runs::text);
                }
                for item in grid.items.iter().filter_map(super::from_grid_item) {
                    items.push(item);
                }
                if continuation.is_none() {
                    continuation = get_continuation(&grid.continuations);
                }
            }
            if let Some(carousel) = &content.music_carousel_shelf_renderer {
                if title.is_none() {
                    title = carousel.title().map(str::to_owned);
                }
                for c in carousel.contents.iter().filter_map(from_carousel_content) {
                    items.push(c);
                }
            }
            if let Some(shelf) = &content.music_shelf_renderer {
                if title.is_none() {
                    title = shelf.title.as_ref().map(super::super::response::common::Runs::text);
                }
                for c in shelf.contents.iter().flatten().filter_map(from_shelf_content) {
                    items.push(c);
                }
                if continuation.is_none() {
                    continuation = get_continuation(&shelf.continuations);
                }
            }
        }

        Self { title, items, continuation }
    }

    /// Parse a continuation of an artist items page.
    #[must_use]
    pub fn from_continuation(response: &BrowseResponse) -> ArtistItemsContinuationPage {
        let mut items = Vec::new();
        let mut continuation = None;

        if let Some(gc) = response
            .continuation_contents
            .as_ref()
            .and_then(|cc| cc.grid_continuation.as_ref())
        {
            for item in gc.items.iter().filter_map(super::from_grid_item) {
                items.push(item);
            }
            continuation = get_continuation(&gc.continuations);
        }

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

        ArtistItemsContinuationPage { items, continuation }
    }
}

fn extract_subscribe_info(
    button: Option<&crate::response::common::SubscriptionButton>,
) -> (Vec<String>, bool) {
    let Some(renderer) = button.and_then(|b| b.subscribe_button_renderer.as_ref()) else {
        return (Vec::new(), false);
    };
    let channel_ids = renderer
        .channel_id
        .as_ref()
        .map(|id| vec![id.clone()])
        .unwrap_or_default();
    let subscribed = renderer.subscribed.unwrap_or(false);
    (channel_ids, subscribed)
}
