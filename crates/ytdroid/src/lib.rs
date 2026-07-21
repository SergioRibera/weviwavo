//! `YouTube` Music Innertube client — faithful port of Metrolist's `innertube` module.
//!
//! # Quick start
//!
//! ```no_run
//! use ytdroid::{YouTube, SearchFilter};
//!
//! # async fn example() -> ytdroid::error::Result<()> {
//! let yt = YouTube::new(None, Default::default())?;
//!
//! // Home feed
//! let home = yt.home(None).await?;
//! println!("{} sections", home.sections.len());
//!
//! // Search (filtered — songs only)
//! use ytdroid::youtube::SearchResult;
//! let result = yt.search("Radiohead", Some(&SearchFilter::SONGS)).await?;
//! if let SearchResult::Filtered(page) = result {
//!     println!("{} songs found", page.items.len());
//! }
//! # Ok(())
//! # }
//! ```

pub mod auth;
pub mod client;
pub mod error;
pub mod filters;
pub mod http;
pub mod models;
pub mod pages;
pub mod response;
pub mod youtube;

// ── Flat re-exports for ergonomic use ────────────────────────────────────────

pub use client::Locale;
pub use error::{Error, Result};
pub use filters::{LibraryFilter, SearchFilter};
pub use models::{
    AlbumItem, AlbumRef, Artist, ArtistItem, EpisodeItem, LikeStatus, PlaylistItem, PodcastItem,
    PodcastRef, SongItem, WatchEndpoint, WatchPlaylistEndpoint, YTItem,
};
pub use http::PlaylistPrivacy;
pub use youtube::{AudioStream, ContentHints, SearchResult, YouTube};
