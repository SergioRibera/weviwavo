//! Explore, New Releases, Mood & Genres, and Charts pages.

use crate::models::YTItem;
use crate::response::BrowseResponse;
use super::{from_carousel_content, from_grid_item};

/// Explore page — typically a grid of new releases and featured playlists.
#[derive(Debug, Clone)]
pub struct ExplorePage {
    pub sections: Vec<ExploreSection>,
}

#[derive(Debug, Clone)]
pub struct ExploreSection {
    pub title: Option<String>,
    pub items: Vec<YTItem>,
}

/// Mood/genre button from the Moods & Genres page.
#[derive(Debug, Clone)]
pub struct MoodAndGenre {
    pub title: String,
    /// Hex color string derived from the stripe color.
    pub color: Option<String>,
    /// Params value to pass to a `browse` request to open this mood/genre.
    pub params: Option<String>,
    /// Browse ID for this mood (usually `"FEmusic_moods_and_genres_category"`).
    pub browse_id: Option<String>,
}

/// Moods & Genres explore page.
#[derive(Debug, Clone)]
pub struct MoodAndGenresPage {
    pub sections: Vec<MoodAndGenresSection>,
}

#[derive(Debug, Clone)]
pub struct MoodAndGenresSection {
    pub title: Option<String>,
    pub genres: Vec<MoodAndGenre>,
}

impl ExplorePage {
    #[must_use]
    pub fn from_browse_response(response: &BrowseResponse) -> Self {
        let section_list = response
            .single_col()
            .and_then(|c| c.first_section_list());

        let Some(sl) = section_list else {
            return Self { sections: Vec::new() };
        };

        let sections = sl
            .contents
            .iter()
            .filter_map(|content| {
                if let Some(carousel) = &content.music_carousel_shelf_renderer {
                    let items: Vec<YTItem> =
                        carousel.contents.iter().filter_map(from_carousel_content).collect();
                    if items.is_empty() {
                        return None;
                    }
                    return Some(ExploreSection {
                        title: carousel.title().map(str::to_owned),
                        items,
                    });
                }
                if let Some(grid) = &content.grid_renderer {
                    let items: Vec<YTItem> = grid.items.iter().filter_map(from_grid_item).collect();
                    if items.is_empty() {
                        return None;
                    }
                    let title = grid
                        .header
                        .as_ref()
                        .and_then(|h| h.grid_header_renderer.as_ref())
                        .and_then(|h| h.title.as_ref())
                        .map(super::super::response::common::Runs::text);
                    return Some(ExploreSection { title, items });
                }
                None
            })
            .collect();

        Self { sections }
    }
}

impl MoodAndGenresPage {
    #[must_use]
    pub fn from_browse_response(response: &BrowseResponse) -> Self {
        let section_list = response
            .single_col()
            .and_then(|c| c.first_section_list());

        let Some(sl) = section_list else {
            return Self { sections: Vec::new() };
        };

        let sections = sl
            .contents
            .iter()
            .filter_map(|content| {
                let carousel = content.music_carousel_shelf_renderer.as_ref()?;
                let genres: Vec<MoodAndGenre> = carousel
                    .contents
                    .iter()
                    .filter_map(|c| {
                        let nav_btn = c.music_navigation_button_renderer.as_ref()?;
                        let title =
                            nav_btn.button_text.as_ref()?.runs.first()?.text.clone();
                        let color = nav_btn
                            .solid
                            .as_ref()
                            .and_then(|s| s.left_stripe_color)
                            .map(|c| format!("#{c:06X}"));
                        let (browse_id, params) = nav_btn
                            .navigation_endpoint
                            .as_ref()
                            .and_then(|e| e.browse_endpoint.as_ref())
                            .map(|b| (b.browse_id.clone(), b.params.clone()))
                            .unwrap_or_default();
                        Some(MoodAndGenre { title, color, params, browse_id })
                    })
                    .collect();
                if genres.is_empty() {
                    return None;
                }
                Some(MoodAndGenresSection {
                    title: carousel.title().map(str::to_owned),
                    genres,
                })
            })
            .collect();

        Self { sections }
    }
}

/// Charts page — similar structure to Explore.
pub type ChartsPage = ExplorePage;

impl ChartsPage {
    /// Parse a charts browse response (alias for [`ExplorePage::from_browse_response`]).
    #[must_use]
    pub fn from_charts_response(response: &BrowseResponse) -> Self {
        ExplorePage::from_browse_response(response)
    }
}

/// New Releases albums page.
pub type NewReleasesPage = ExplorePage;
