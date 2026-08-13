//! Home feed page — carousels of recommended content.

use crate::error::{Error, Result};
use crate::models::YTItem;
use crate::response::{BrowseResponse, SectionContent};
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
pub enum HomeSectionLayout {
    /// Horizontal carousel of cards (music_carousel_shelf_renderer).
    #[default]
    Carousel,
    /// Horizontal scroll of 4-row song columns (music_shelf_renderer).
    List,
    /// Grid of tiles (grid_renderer).
    Grid,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct HomeSection {
    pub title: Option<String>,
    /// Small descriptor above the title (e.g. "SERGIO RIBERA", "ÉXITOS DE CADA DÉCADA").
    pub strapline: Option<String>,
    /// Optional thumbnail shown beside the strapline/title (e.g. user avatar for "Listen again").
    pub thumbnail: Option<String>,
    pub items: Vec<YTItem>,
    pub layout: HomeSectionLayout,
    /// `browseId` of the "Show more" button, if present.
    pub more_browse_id: Option<String>,
}

fn parse_section_contents(contents: &[SectionContent]) -> Vec<HomeSection> {
    use super::super::response::common::Runs;

    contents
        .iter()
        .filter_map(|content| {
            if let Some(shelf) = &content.music_carousel_shelf_renderer {
                let items: Vec<YTItem> = shelf
                    .contents
                    .iter()
                    .filter_map(from_carousel_content)
                    .collect();
                if items.is_empty() {
                    return None;
                }
                let basic_header = shelf
                    .header
                    .as_ref()
                    .and_then(|h| h.music_carousel_shelf_basic_header_renderer.as_ref());
                let strapline = basic_header
                    .and_then(|h| h.strapline.as_ref())
                    .map(Runs::text);
                let thumbnail = basic_header
                    .and_then(|h| h.thumbnail.as_ref())
                    .and_then(|t| t.get_url())
                    .map(str::to_owned);
                return Some(HomeSection {
                    title: shelf.title().map(str::to_owned),
                    strapline,
                    thumbnail,
                    items,
                    layout: HomeSectionLayout::Carousel,
                    more_browse_id: shelf.more_browse_id().map(str::to_owned),
                });
            }
            if let Some(grid) = &content.grid_renderer {
                let items: Vec<YTItem> =
                    grid.items.iter().filter_map(super::from_grid_item).collect();
                if items.is_empty() {
                    return None;
                }
                let title = grid
                    .header
                    .as_ref()
                    .and_then(|h| h.grid_header_renderer.as_ref())
                    .and_then(|h| h.title.as_ref())
                    .map(Runs::text);
                return Some(HomeSection {
                    title,
                    strapline: None,
                    thumbnail: None,
                    items,
                    layout: HomeSectionLayout::Grid,
                    more_browse_id: None,
                });
            }
            if let Some(shelf) = &content.music_shelf_renderer {
                let items: Vec<YTItem> = shelf
                    .contents
                    .iter()
                    .flatten()
                    .filter_map(super::from_shelf_content)
                    .collect();
                if items.is_empty() {
                    return None;
                }
                let title = shelf.title.as_ref().map(Runs::text);
                return Some(HomeSection {
                    title,
                    strapline: None,
                    thumbnail: None,
                    items,
                    layout: HomeSectionLayout::List,
                    more_browse_id: None,
                });
            }
            None
        })
        .collect()
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
        let sections = parse_section_contents(&section_list.contents);

        Ok(Self { chips, sections, continuation })
    }

    /// Parse a home-feed continuation response (`continuationContents.sectionListContinuation`).
    ///
    /// # Errors
    ///
    /// Returns [`Error::MissingField`] when `sectionListContinuation` is absent.
    pub fn from_continuation_response(response: &BrowseResponse) -> Result<Self> {
        let cont = response
            .continuation_contents
            .as_ref()
            .and_then(|cc| cc.section_list_continuation.as_ref())
            .ok_or(Error::MissingField {
                field: "sectionListContinuation",
            })?;

        let continuation = cont.continuation();
        let sections = parse_section_contents(&cont.contents);

        Ok(Self { chips: Vec::new(), sections, continuation })
    }
}
