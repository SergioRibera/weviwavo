//! Search results pages.

use crate::error::{Error, Result};
use crate::models::YTItem;
use crate::response::common::get_continuation;
use crate::response::{MusicCardShelfRenderer, SearchResponse};
use super::{from_responsive, from_shelf_content};

/// Full search results (all item types mixed).
#[derive(Debug, Clone)]
pub struct SearchPage {
    /// All parsed items across all content sections.
    pub items: Vec<YTItem>,
    pub continuation: Option<String>,
}

/// A single typed section of search results (e.g. "Songs", "Albums", "Artists").
#[derive(Debug, Clone)]
pub struct SearchSummary {
    /// Section label, e.g. `"Songs"`, `"Albums"`.
    pub title: String,
    pub items: Vec<YTItem>,
}

/// Search result page when no `SearchFilter` is applied — returns one section per content type.
#[derive(Debug, Clone)]
pub struct SearchSummaryPage {
    pub sections: Vec<SearchSummary>,
}

/// Continuation of a typed search section.
#[derive(Debug, Clone)]
pub struct SearchContinuationPage {
    pub items: Vec<YTItem>,
    pub continuation: Option<String>,
}

impl SearchPage {
    /// Parse a filtered search response (single content type, `params` was set).
    ///
    /// # Errors
    ///
    /// Returns [`crate::error::Error::MissingField`] when the expected hierarchy is absent.
    pub fn from_search_response(response: &SearchResponse) -> Result<Self> {
        let section_list =
            response
                .first_section_list()
                .ok_or(Error::MissingField {
                    field: "tabbedSearchResultsRenderer",
                })?;

        let mut items = Vec::new();
        let mut continuation = None;

        for content in &section_list.contents {
            if let Some(shelf) = &content.music_shelf_renderer {
                for c in shelf.contents.iter().flatten() {
                    if let Some(item) = from_shelf_content(c) {
                        items.push(item);
                    }
                }
                if continuation.is_none() {
                    continuation = get_continuation(&shelf.continuations);
                }
            }
        }

        Ok(Self { items, continuation })
    }

    /// Parse a search continuation response.
    #[must_use]
    pub fn from_search_continuation(response: &SearchResponse) -> SearchContinuationPage {
        let items = response
            .continuation_contents
            .as_ref()
            .and_then(|c| c.music_shelf_continuation.as_ref())
            .map(|shelf| shelf.contents.iter().filter_map(from_shelf_content).collect())
            .unwrap_or_default();
        let continuation = response
            .continuation_contents
            .as_ref()
            .and_then(|c| c.music_shelf_continuation.as_ref())
            .map(|shelf| get_continuation(&shelf.continuations))
            .unwrap_or_default();
        SearchContinuationPage { items, continuation }
    }
}

impl SearchSummaryPage {
    /// Parse an un-filtered search response — multiple typed sections.
    #[must_use]
    pub fn from_search_response(response: &SearchResponse) -> Self {
        let Some(section_list) = response.first_section_list() else {
            return Self { sections: Vec::new() };
        };

        let sections = section_list
            .contents
            .iter()
            .filter_map(|content| {
                // Top-result card shelf
                if let Some(card) = &content.music_card_shelf_renderer {
                    return parse_card_shelf(card);
                }
                // Typed content shelf
                if let Some(shelf) = &content.music_shelf_renderer {
                    let title = shelf.title.as_ref().map(super::super::response::common::Runs::text).unwrap_or_default();
                    let items: Vec<YTItem> = shelf
                        .list_items()
                        .filter_map(from_responsive)
                        .collect();
                    if items.is_empty() {
                        return None;
                    }
                    return Some(SearchSummary { title, items });
                }
                None
            })
            .collect();

        Self { sections }
    }
}

fn parse_card_shelf(card: &MusicCardShelfRenderer) -> Option<SearchSummary> {
    let title = card
        .header
        .as_ref()
        .and_then(|h| h.music_card_shelf_header_basic_renderer.as_ref())
        .and_then(|h| h.title.as_ref()).map_or_else(|| "Top result".to_owned(), super::super::response::common::Runs::text);

    let mut items = Vec::new();

    // The main card itself (from on_tap endpoint)
    if let Some(item) = card.contents.as_ref().and_then(|contents| {
        contents.first()?.music_responsive_list_item_renderer.as_ref().and_then(from_responsive)
    }) {
        items.push(item);
    }

    if items.is_empty() {
        return None;
    }
    Some(SearchSummary { title, items })
}
