//! Related content page returned by the "Related" browse endpoint from the `next` response.

use crate::models::PlaylistItem;
use crate::response::BrowseResponse;
use super::{from_carousel_content, from_two_row};

#[derive(Debug, Clone, Default)]
pub struct RelatedPage {
    pub playlists: Vec<PlaylistItem>,
}

impl RelatedPage {
    /// Parse a related browse response.
    #[must_use]
    pub fn from_browse_response(response: &BrowseResponse) -> Self {
        let mut playlists = Vec::new();

        let contents = response
            .contents
            .as_ref()
            .and_then(|c| c.section_list_renderer.as_ref())
            .map(|sl| sl.contents.as_slice())
            .unwrap_or_default();

        for section in contents {
            let shelf = match &section.music_carousel_shelf_renderer {
                Some(s) => &s.contents,
                None => continue,
            };
            for content in shelf {
                let item = content
                    .music_two_row_item_renderer
                    .as_ref()
                    .and_then(from_two_row)
                    .or_else(|| from_carousel_content(content));

                if let Some(crate::models::YTItem::Playlist(p)) = item {
                    playlists.push(p);
                }
            }
        }

        Self { playlists }
    }
}
