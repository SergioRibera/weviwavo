//! Home feed page — carousels of recommended content.

use crate::error::{Error, Result};
use crate::models::YTItem;
use crate::response::BrowseResponse;
use super::from_carousel_content;

#[derive(Debug, Clone, Default, PartialEq)]
pub struct HomePage {
    pub chips: Vec<HomeChip>,
    pub sections: Vec<HomeSection>,
    /// Continuation token for fetching more sections (if the feed is paginated).
    pub continuation: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct HomeChip {
    pub title: String,
    /// `params` value to send in the next browse request (for chip activation).
    pub params: Option<String>,
    /// The `browseId` to use when activating the chip (usually `"FEmusic_home"`).
    pub browse_id: Option<String>,
    /// The `params` to deselect / clear the chip (back to default feed).
    pub deselect_params: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct HomeSection {
    pub title: Option<String>,
    pub items: Vec<YTItem>,
    /// `browseId` of the "Show more" button, if present.
    pub more_browse_id: Option<String>,
}

impl HomePage {
    /// Parse a home-feed `BrowseResponse` (`browseId = "FEmusic_home"`).
    ///
    /// # Errors
    ///
    /// Returns [`Error::MissingField`] when the expected renderer hierarchy is absent.
    pub fn from_browse_response(response: &BrowseResponse) -> Result<Self> {
        let section_list = response
            .single_col()
            .and_then(|c| c.first_section_list())
            .ok_or(Error::MissingField {
                field: "sectionListRenderer",
            })?;

        let chips = section_list
            .header
            .as_ref()
            .and_then(|h| h.chip_cloud_renderer.as_ref())
            .map(|cr| {
                cr.chips
                    .iter()
                    .map(|chip| {
                        let r = &chip.chip_cloud_chip_renderer;
                        let title = r.text.as_ref().map(super::super::response::common::Runs::text).unwrap_or_default();
                        let (browse_id, params) = r
                            .navigation_endpoint
                            .as_ref()
                            .and_then(|e| e.browse_endpoint.as_ref())
                            .map(|b| (b.browse_id.clone(), b.params.clone()))
                            .unwrap_or_default();
                        let deselect_params = r
                            .on_deselected_command
                            .as_ref()
                            .and_then(|e| e.browse_endpoint.as_ref())
                            .and_then(|b| b.params.clone());
                        HomeChip {
                            title,
                            params,
                            browse_id,
                            deselect_params,
                        }
                    })
                    .collect()
            })
            .unwrap_or_default();

        let continuation = section_list.continuation();

        let sections = section_list
            .contents
            .iter()
            .filter_map(|content| {
                let shelf = content.music_carousel_shelf_renderer.as_ref()?;
                let items: Vec<YTItem> = shelf
                    .contents
                    .iter()
                    .filter_map(from_carousel_content)
                    .collect();
                if items.is_empty() {
                    return None;
                }
                Some(HomeSection {
                    title: shelf.title().map(str::to_owned),
                    items,
                    more_browse_id: shelf.more_browse_id().map(str::to_owned),
                })
            })
            .collect();

        Ok(Self {
            chips,
            sections,
            continuation,
        })
    }
}
