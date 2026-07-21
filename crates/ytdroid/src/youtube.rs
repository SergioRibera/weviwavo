//! High-level `YouTube` Music API — the primary entry point for consumers.

use tracing::instrument;

use crate::client::{Locale, YouTubeClient};
use crate::error::{Error, Result};
use crate::filters::{LibraryFilter, SearchFilter};
use crate::http::{AddDedupOption, InnerTube, LikeAction};
pub use crate::http::PlaylistPrivacy;

// ─────────────────────────────────────────────
// Content-aware stream client selection
// (mirrors Metrolist's ContentAwareFallbackStrategy)
// ─────────────────────────────────────────────

/// Hints about the content being played used to select the optimal client chain.
#[derive(Debug, Default, Clone)]
pub struct ContentHints {
    /// True for user-uploaded tracks (playlistId contains "MLPT").
    pub is_uploaded: bool,
    /// True for age-restricted / explicit content.
    pub is_explicit: bool,
    /// True for YouTube Kids content.
    pub is_kids_content: bool,
    /// True for live streams.
    pub is_live: bool,
}

/// Resolved audio stream from [`YouTube::audio_stream`].
pub struct AudioStream {
    /// Direct CDN URL or raw `signatureCipher` query string.
    pub data: String,
    /// When `true`, `data` is a `signatureCipher` that requires sig decryption.
    pub is_cipher: bool,
    /// Streaming PoToken to append as `pot=` to the final CDN URL (WEB clients only).
    /// The caller must append this AFTER sig decryption and nsig transform.
    pub streaming_pot: Option<String>,
}

fn clients_for_hints(hints: &ContentHints) -> Vec<YouTubeClient> {
    if hints.is_uploaded {
        // Uploaded tracks: WEB clients with auth; WEB_CREATOR requires login.
        vec![
            YouTubeClient::TVHTML5,
            YouTubeClient::WEB_REMIX,
            YouTubeClient::WEB_CREATOR,
        ]
    } else if hints.is_live {
        vec![
            YouTubeClient::TVHTML5,
            YouTubeClient::WEB_REMIX,
            YouTubeClient::WEB_CREATOR,
            YouTubeClient::TVHTML5_SIMPLY,
        ]
    } else if hints.is_kids_content {
        vec![
            YouTubeClient::TVHTML5,
            YouTubeClient::WEB_REMIX,
            YouTubeClient::TVHTML5_SIMPLY,
            YouTubeClient::WEB_CREATOR,
        ]
    } else if hints.is_explicit {
        vec![
            YouTubeClient::VISIONOS,
            YouTubeClient::TVHTML5,
            YouTubeClient::WEB_REMIX,
        ]
    } else {
        // Default: direct-URL clients first (no sig/nsig/PoToken), then WEB fallbacks.
        vec![
            YouTubeClient::VISIONOS,
            YouTubeClient::ANDROID_VR_1_65,
            YouTubeClient::ANDROID_VR_1_43,
            YouTubeClient::WEB_REMIX,
            YouTubeClient::TVHTML5,
            YouTubeClient::TVHTML5_SIMPLY,
        ]
    }
}
use crate::pages::{
    album::AlbumPage,
    artist::{ArtistItemsContinuationPage, ArtistItemsPage, ArtistPage},
    explore::{ChartsPage, ExplorePage, MoodAndGenresPage, NewReleasesPage},
    home::HomePage,
    library::{LibraryContinuationPage, LibraryPage},
    next::{NextContinuationPage, NextPage},
    playlist::{PlaylistContinuationPage, PlaylistPage},
    search::{SearchContinuationPage, SearchPage, SearchSummaryPage},
};
use crate::response::{
    AccountMenuResponse, GetQueueResponse, GetTranscriptResponse, PlayerResponse,
};

/// The primary `YouTube` Music client.
///
/// Wraps an [`InnerTube`] HTTP client and exposes ergonomic methods for every
/// page / action supported by the Innertube API.
///
/// # Authentication
///
/// Pass the raw `Cookie:` header from a logged-in browser session to
/// [`YouTube::new`] to enable library / playlist write operations.
/// Unauthenticated instances can still access all public pages.
#[derive(Clone)]
pub struct YouTube {
    inner: InnerTube,
}

impl YouTube {
    /// Create a new client with optional authentication.
    ///
    /// `cookie_header` — the raw `Cookie:` header value. Pass `None` for unauthenticated use.
    /// `locale` — `gl` / `hl` pair; use [`Locale::default`] for `US`/`en`.
    ///
    /// # Errors
    ///
    /// Returns an error if the underlying HTTP client cannot be constructed.
    pub fn new(cookie_header: Option<&str>, locale: Locale) -> Result<Self> {
        Ok(Self {
            inner: InnerTube::new(cookie_header, locale)?,
        })
    }

    /// Set an optional visitor ID (sent as `X-Goog-Visitor-Id`).
    #[must_use]
    pub fn with_visitor_id(mut self, visitor_id: impl Into<String>) -> Self {
        self.inner.visitor_id = Some(visitor_id.into());
        self
    }

    /// The visitor/session ID set via [`with_visitor_id`], if any.
    #[must_use]
    pub fn visitor_id(&self) -> Option<&str> {
        self.inner.visitor_id.as_deref()
    }

    /// True when a `SAPISID` / `__Secure-3PAPISID` cookie was provided.
    #[must_use]
    pub fn is_logged_in(&self) -> bool {
        self.inner.is_logged_in()
    }

    // ── Browse / page methods ────────────────────────────────────────────────

    /// Fetch the home feed.
    ///
    /// Pass `chip_params` to activate a chip filter (e.g. "Music videos").
    ///
    /// # Errors
    ///
    /// Propagates HTTP and parsing errors.
    #[instrument(skip(self))]
    pub async fn home(&self, chip_params: Option<&str>) -> Result<HomePage> {
        use crate::filters::browse_id::HOME;
        let raw = self
            .inner
            .browse(&YouTubeClient::WEB_REMIX, HOME, chip_params, None)
            .await?;
        HomePage::from_browse_response(&raw)
    }

    /// Fetch the next home-feed page (continuation).
    ///
    /// # Errors
    ///
    /// Propagates HTTP and parsing errors.
    pub async fn home_continuation(&self, continuation: &str) -> Result<HomePage> {
        use crate::filters::browse_id::HOME;
        let raw = self
            .inner
            .browse(&YouTubeClient::WEB_REMIX, HOME, None, Some(continuation))
            .await?;
        HomePage::from_browse_response(&raw)
    }

    /// Search `YouTube` Music.
    ///
    /// Without a `filter`, returns a [`SearchSummaryPage`] with one section per content type.
    /// With a `filter`, returns a [`SearchPage`] containing only items of that type.
    ///
    /// # Errors
    ///
    /// Propagates HTTP and parsing errors.
    #[instrument(skip(self))]
    pub async fn search(
        &self,
        query: &str,
        filter: Option<&SearchFilter>,
    ) -> Result<SearchResult> {
        let raw = self
            .inner
            .search(
                &YouTubeClient::WEB_REMIX,
                query,
                filter.map(|f| f.0),
                None,
            )
            .await?;
        if filter.is_some() {
            Ok(SearchResult::Filtered(SearchPage::from_search_response(&raw)?))
        } else {
            Ok(SearchResult::Summary(SearchSummaryPage::from_search_response(&raw)))
        }
    }

    /// Continue a filtered search result page.
    ///
    /// # Errors
    ///
    /// Propagates HTTP and parsing errors.
    pub async fn search_continuation(
        &self,
        continuation: &str,
    ) -> Result<SearchContinuationPage> {
        let raw = self
            .inner
            .search(&YouTubeClient::WEB_REMIX, "", None, Some(continuation))
            .await?;
        Ok(SearchPage::from_search_continuation(&raw))
    }

    /// Fetch an artist page by its browse ID.
    ///
    /// # Errors
    ///
    /// Propagates HTTP and parsing errors.
    #[instrument(skip(self))]
    pub async fn artist(&self, browse_id: &str) -> Result<ArtistPage> {
        let raw = self
            .inner
            .browse(&YouTubeClient::WEB_REMIX, browse_id, None, None)
            .await?;
        let mut page = ArtistPage::from_browse_response(&raw)?;
        browse_id.clone_into(&mut page.artist.id);
        Ok(page)
    }

    /// Fetch all items of one type from an artist page (songs, albums, singles, videos).
    ///
    /// # Errors
    ///
    /// Propagates HTTP and parsing errors.
    pub async fn artist_items(&self, browse_id: &str, params: &str) -> Result<ArtistItemsPage> {
        let raw = self
            .inner
            .browse(&YouTubeClient::WEB_REMIX, browse_id, Some(params), None)
            .await?;
        Ok(ArtistItemsPage::from_browse_response(&raw))
    }

    /// Continue an artist items page.
    ///
    /// # Errors
    ///
    /// Propagates HTTP and parsing errors.
    pub async fn artist_items_continuation(
        &self,
        continuation: &str,
    ) -> Result<ArtistItemsContinuationPage> {
        let raw = self
            .inner
            .browse(&YouTubeClient::WEB_REMIX, "", None, Some(continuation))
            .await?;
        Ok(ArtistItemsPage::from_continuation(&raw))
    }

    /// Fetch an album page by its browse ID.
    ///
    /// # Errors
    ///
    /// Propagates HTTP and parsing errors.
    #[instrument(skip(self))]
    pub async fn album(&self, browse_id: &str) -> Result<AlbumPage> {
        let raw = self
            .inner
            .browse(&YouTubeClient::WEB_REMIX, browse_id, None, None)
            .await?;
        let mut page = AlbumPage::from_browse_response(&raw)?;
        browse_id.clone_into(&mut page.album.id);
        Ok(page)
    }

    /// Fetch a playlist page by its browse ID (e.g. `"VL" + playlist_id`).
    ///
    /// # Errors
    ///
    /// Propagates HTTP and parsing errors.
    #[instrument(skip(self))]
    pub async fn playlist(&self, browse_id: &str) -> Result<PlaylistPage> {
        let raw = self
            .inner
            .browse(&YouTubeClient::WEB_REMIX, browse_id, None, None)
            .await?;
        let mut page = PlaylistPage::from_browse_response(&raw)?;
        if page.playlist.id.is_empty() {
            browse_id.clone_into(&mut page.playlist.id);
        }
        Ok(page)
    }

    /// Continue a playlist / album song list.
    ///
    /// # Errors
    ///
    /// Propagates HTTP and parsing errors.
    pub async fn playlist_continuation(
        &self,
        continuation: &str,
    ) -> Result<PlaylistContinuationPage> {
        let raw = self
            .inner
            .browse(&YouTubeClient::WEB_REMIX, "", None, Some(continuation))
            .await?;
        Ok(PlaylistPage::from_continuation(&raw))
    }

    // ── Library ──────────────────────────────────────────────────────────────

    /// Fetch a library page (songs, albums, playlists, artists, or podcasts).
    ///
    /// Use [`crate::filters::browse_id`] constants for `browse_id` and optionally
    /// a [`LibraryFilter`] to control sort order.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Unauthenticated`] when not logged in. Propagates HTTP and parsing errors.
    #[instrument(skip(self))]
    pub async fn library(
        &self,
        browse_id: &str,
        filter: Option<&LibraryFilter>,
    ) -> Result<LibraryPage> {
        if !self.inner.is_logged_in() {
            return Err(Error::Unauthenticated);
        }
        let raw = self
            .inner
            .browse(
                &YouTubeClient::WEB_REMIX,
                browse_id,
                None,
                filter.map(|f| f.0),
            )
            .await?;
        Ok(LibraryPage::from_browse_response(&raw))
    }

    /// Continue a library page.
    ///
    /// # Errors
    ///
    /// Propagates HTTP and parsing errors.
    pub async fn library_continuation(
        &self,
        continuation: &str,
    ) -> Result<LibraryContinuationPage> {
        let raw = self
            .inner
            .browse(&YouTubeClient::WEB_REMIX, "", None, Some(continuation))
            .await?;
        Ok(LibraryPage::from_continuation(&raw))
    }

    // ── History ──────────────────────────────────────────────────────────────

    /// Fetch the user's listening history.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Unauthenticated`] when not logged in. Propagates HTTP and parsing errors.
    pub async fn history(&self) -> Result<LibraryPage> {
        use crate::filters::browse_id::HISTORY;
        if !self.inner.is_logged_in() {
            return Err(Error::Unauthenticated);
        }
        let raw = self
            .inner
            .browse(&YouTubeClient::WEB_REMIX, HISTORY, None, None)
            .await?;
        Ok(LibraryPage::from_browse_response(&raw))
    }

    // ── Explore ──────────────────────────────────────────────────────────────

    /// Fetch the Explore page.
    ///
    /// # Errors
    ///
    /// Propagates HTTP and parsing errors.
    pub async fn explore(&self) -> Result<ExplorePage> {
        use crate::filters::browse_id::EXPLORE;
        let raw = self
            .inner
            .browse(&YouTubeClient::WEB_REMIX, EXPLORE, None, None)
            .await?;
        Ok(ExplorePage::from_browse_response(&raw))
    }

    /// Fetch the New Releases page.
    ///
    /// # Errors
    ///
    /// Propagates HTTP and parsing errors.
    pub async fn new_releases(&self) -> Result<NewReleasesPage> {
        use crate::filters::browse_id::NEW_RELEASES;
        let raw = self
            .inner
            .browse(&YouTubeClient::WEB_REMIX, NEW_RELEASES, None, None)
            .await?;
        Ok(ExplorePage::from_browse_response(&raw))
    }

    /// Fetch the Moods & Genres page.
    ///
    /// # Errors
    ///
    /// Propagates HTTP and parsing errors.
    pub async fn moods_and_genres(&self) -> Result<MoodAndGenresPage> {
        use crate::filters::browse_id::MOODS_AND_GENRES;
        let raw = self
            .inner
            .browse(&YouTubeClient::WEB_REMIX, MOODS_AND_GENRES, None, None)
            .await?;
        Ok(MoodAndGenresPage::from_browse_response(&raw))
    }

    /// Fetch items in a specific mood/genre category.
    ///
    /// # Errors
    ///
    /// Propagates HTTP and parsing errors.
    pub async fn mood_genre_items(
        &self,
        browse_id: &str,
        params: &str,
    ) -> Result<ExplorePage> {
        let raw = self
            .inner
            .browse(&YouTubeClient::WEB_REMIX, browse_id, Some(params), None)
            .await?;
        Ok(ExplorePage::from_browse_response(&raw))
    }

    /// Fetch the Charts page.
    ///
    /// # Errors
    ///
    /// Propagates HTTP and parsing errors.
    pub async fn charts(&self, params: Option<&str>) -> Result<ChartsPage> {
        use crate::filters::browse_id::{CHARTS, CHARTS_PARAMS};
        let raw = self
            .inner
            .browse(
                &YouTubeClient::WEB_REMIX,
                CHARTS,
                Some(params.unwrap_or(CHARTS_PARAMS)),
                None,
            )
            .await?;
        Ok(ExplorePage::from_browse_response(&raw))
    }

    // ── Podcasts ─────────────────────────────────────────────────────────────

    /// Fetch the podcast discovery page.
    ///
    /// # Errors
    ///
    /// Propagates HTTP and parsing errors.
    pub async fn podcast_discover(&self) -> Result<ExplorePage> {
        use crate::filters::browse_id::PODCAST_DISCOVER;
        let raw = self
            .inner
            .browse(&YouTubeClient::WEB_REMIX, PODCAST_DISCOVER, None, None)
            .await?;
        Ok(ExplorePage::from_browse_response(&raw))
    }

    // ── Player ───────────────────────────────────────────────────────────────

    /// Fetch player / streaming data for a video.
    ///
    /// Uses the ANDROID client for direct URLs (no cipher / n-transform required).
    ///
    /// # Errors
    ///
    /// Propagates HTTP and parsing errors.
    #[instrument(skip(self))]
    pub async fn player(
        &self,
        video_id: &str,
        playlist_id: Option<&str>,
    ) -> Result<PlayerResponse> {
        let raw = self
            .inner
            .player(&YouTubeClient::MOBILE, video_id, playlist_id, None, None)
            .await?;
        Ok(raw)
    }

    /// Low-level player call — choose any client, pass an optional PoToken.
    ///
    /// For WEB clients (`use_web_po_tokens = true`) pass the `po_token` from
    /// `servo_webview::potoken::generate` as the player-request token.
    ///
    /// # Errors
    ///
    /// Propagates HTTP and parsing errors.
    pub async fn player_raw(
        &self,
        client: &YouTubeClient,
        video_id: &str,
        signature_timestamp: Option<u32>,
        po_token: Option<&str>,
    ) -> Result<PlayerResponse> {
        self.inner
            .player(client, video_id, None, signature_timestamp, po_token)
            .await
    }

    /// Fetch the best audio-only stream for a video.
    ///
    /// Mirrors Metrolist's `ContentAwareFallbackStrategy` + `YTPlayerUtils.playerResponseForPlayback`.
    ///
    /// - `hints` — content type hints that select the optimal client chain.
    /// - `po_token` — player-request PoToken (sent in the `/player` body for WEB clients).
    /// - `streaming_pot` — streaming PoToken to append as `pot=` to the CDN URL after resolution.
    /// - `sig_ts` — `signatureTimestamp` from the player JS, required by WEB/TVHTML5 clients.
    ///
    /// Clients that `require_po_token` are skipped when `po_token` is `None`.
    /// Clients that `login_required` are skipped when not authenticated.
    ///
    /// # Errors
    ///
    /// Returns [`Error::AllClientsFailed`], [`Error::NotPlayable`], or [`Error::NoAudioFormat`]
    /// when no client yields a playable audio stream.
    pub async fn audio_stream(
        &self,
        video_id: &str,
        hints: &ContentHints,
        po_token: Option<&str>,
        streaming_pot: Option<&str>,
        sig_ts: Option<u32>,
    ) -> Result<AudioStream> {
        let clients = clients_for_hints(hints);
        let logged_in = self.inner.is_logged_in();

        let mut last_err = Error::AllClientsFailed {
            video_id: video_id.to_owned(),
        };

        for client in &clients {
            if client.require_po_token && po_token.is_none() {
                tracing::debug!(client = client.client_name, "skipping: requires PoToken");
                continue;
            }
            if client.login_required && !logged_in {
                tracing::debug!(client = client.client_name, "skipping: requires login");
                continue;
            }

            let client_pot = if client.use_web_po_tokens { po_token } else { None };
            let client_sig_ts = if client.use_signature_timestamp { sig_ts } else { None };

            match self.inner.player(client, video_id, None, client_sig_ts, client_pot).await {
                Ok(resp) => {
                    let status = &resp.playability_status.status;
                    if status != "OK" {
                        tracing::warn!(
                            client = client.client_name,
                            playability = %status,
                            reason = resp.playability_status.reason.as_deref().unwrap_or(""),
                            "not playable"
                        );
                        last_err = Error::NotPlayable {
                            status: status.clone(),
                            reason: resp.playability_status.reason.unwrap_or_default(),
                        };
                        continue;
                    }

                    let sd = resp.streaming_data.as_ref();
                    let audio_only = sd.map(|s| s.adaptive_formats.iter().filter(|f| f.is_audio_only()).count()).unwrap_or(0);
                    let direct = sd.map(|s| s.adaptive_formats.iter().filter(|f| f.is_audio_only() && f.has_direct_url()).count()).unwrap_or(0);
                    let cipher = sd.map(|s| s.adaptive_formats.iter().filter(|f| f.is_audio_only() && f.signature_cipher.is_some()).count()).unwrap_or(0);
                    tracing::debug!(client = client.client_name, audio_only, direct, cipher, "player OK");

                    // Prefer direct URL (no sig/nsig needed — VISIONOS/VR path).
                    if let Some(fmt) = sd.and_then(|s| s.best_audio_format()) {
                        tracing::debug!(client = client.client_name, bitrate = fmt.bitrate, mime = fmt.mime_type.as_deref().unwrap_or("?"), "direct audio format");
                        let url = fmt.url.clone().ok_or(Error::NoAudioFormat {
                            video_id: video_id.to_owned(),
                        })?;
                        let spot = client.use_web_po_tokens.then(|| streaming_pot).flatten().map(str::to_owned);
                        return Ok(AudioStream { data: url, is_cipher: false, streaming_pot: spot });
                    }

                    // Cipher URL (WEB clients — sig decryption + nsig needed by caller).
                    if let Some(fmt) = sd.and_then(|s| s.best_cipher_audio_format()) {
                        tracing::debug!(client = client.client_name, bitrate = fmt.bitrate, "cipher audio format");
                        let cipher_str = fmt.signature_cipher.clone().ok_or(Error::NoAudioFormat {
                            video_id: video_id.to_owned(),
                        })?;
                        let spot = client.use_web_po_tokens.then(|| streaming_pot).flatten().map(str::to_owned);
                        return Ok(AudioStream { data: cipher_str, is_cipher: true, streaming_pot: spot });
                    }

                    last_err = Error::NoAudioFormat { video_id: video_id.to_owned() };
                }
                Err(e) => {
                    tracing::warn!(client = client.client_name, error = %e, "player request failed");
                    last_err = e;
                }
            }
        }

        Err(last_err)
    }

    /// Convenience wrapper — resolves a direct CDN URL using default hints and no PoToken.
    ///
    /// # Errors
    ///
    /// Returns [`Error::AllClientsFailed`], [`Error::NotPlayable`], or [`Error::NoAudioFormat`].
    pub async fn audio_url(&self, video_id: &str) -> Result<String> {
        let stream = self.audio_stream(video_id, &ContentHints::default(), None, None, None).await?;
        if stream.is_cipher {
            Err(Error::NoAudioFormat { video_id: video_id.to_owned() })
        } else {
            Ok(stream.data)
        }
    }

    // ── Up-next / queue ──────────────────────────────────────────────────────

    /// Fetch the up-next queue for a video / playlist.
    ///
    /// # Errors
    ///
    /// Propagates HTTP and parsing errors.
    #[instrument(skip(self))]
    pub async fn next(
        &self,
        video_id: Option<&str>,
        playlist_id: Option<&str>,
        params: Option<&str>,
        index: Option<u32>,
        playlist_set_video_id: Option<&str>,
    ) -> Result<NextPage> {
        let raw = self
            .inner
            .next(
                &YouTubeClient::WEB_REMIX,
                video_id,
                playlist_id,
                params,
                index,
                playlist_set_video_id,
                None,
            )
            .await?;
        NextPage::from_next_response(&raw)
    }

    /// Continue an up-next queue.
    ///
    /// # Errors
    ///
    /// Propagates HTTP and parsing errors.
    pub async fn next_continuation(&self, continuation: &str) -> Result<NextContinuationPage> {
        let raw = self
            .inner
            .next(
                &YouTubeClient::WEB_REMIX,
                None,
                None,
                None,
                None,
                None,
                Some(continuation),
            )
            .await?;
        Ok(NextPage::from_continuation(&raw))
    }

    /// Resolve video IDs or a playlist ID into queue items.
    ///
    /// # Errors
    ///
    /// Propagates HTTP and parsing errors.
    pub async fn get_queue(
        &self,
        video_ids: &[&str],
        playlist_id: Option<&str>,
    ) -> Result<GetQueueResponse> {
        self.inner
            .get_queue(&YouTubeClient::WEB_REMIX, video_ids, playlist_id)
            .await
    }

    // ── Suggestions / transcript ─────────────────────────────────────────────

    /// Get search autocomplete suggestions.
    ///
    /// # Errors
    ///
    /// Propagates HTTP and parsing errors.
    pub async fn search_suggestions(&self, query: &str) -> Result<Vec<String>> {
        let resp = self
            .inner
            .get_search_suggestions(&YouTubeClient::WEB_REMIX, query)
            .await?;
        let suggestions = resp
            .contents
            .iter()
            .filter_map(|sec| sec.search_suggestions_section_renderer.as_ref())
            .flat_map(|sec| sec.contents.iter())
            .filter_map(|c| c.search_suggestion_renderer.as_ref())
            .filter_map(|r| r.suggestion.as_ref())
            .map(super::response::common::Runs::text)
            .collect();
        Ok(suggestions)
    }

    /// Get a video / episode transcript.
    ///
    /// # Errors
    ///
    /// Propagates HTTP and parsing errors.
    pub async fn transcript(
        &self,
        params: &str,
    ) -> Result<GetTranscriptResponse> {
        self.inner
            .get_transcript(&YouTubeClient::WEB_REMIX, params)
            .await
    }

    // ── Library actions ──────────────────────────────────────────────────────

    /// Add a song / album / playlist to the library.
    ///
    /// # Errors
    ///
    /// Propagates HTTP errors.
    pub async fn add_to_library(&self, feedback_token: &str) -> Result<()> {
        self.inner
            .feedback(&YouTubeClient::WEB_REMIX, &[feedback_token])
            .await
    }

    /// Remove a song / album / playlist from the library.
    ///
    /// # Errors
    ///
    /// Propagates HTTP errors.
    pub async fn remove_from_library(&self, feedback_token: &str) -> Result<()> {
        self.inner
            .feedback(&YouTubeClient::WEB_REMIX, &[feedback_token])
            .await
    }

    /// Like a song.
    ///
    /// # Errors
    ///
    /// Propagates HTTP errors.
    pub async fn like_song(&self, video_id: &str) -> Result<()> {
        self.inner
            .like(&YouTubeClient::WEB_REMIX, video_id, LikeAction::Like)
            .await
    }

    /// Dislike a song.
    ///
    /// # Errors
    ///
    /// Propagates HTTP errors.
    pub async fn dislike_song(&self, video_id: &str) -> Result<()> {
        self.inner
            .like(&YouTubeClient::WEB_REMIX, video_id, LikeAction::Dislike)
            .await
    }

    /// Remove like / dislike from a song.
    ///
    /// # Errors
    ///
    /// Propagates HTTP errors.
    pub async fn remove_like(&self, video_id: &str) -> Result<()> {
        self.inner
            .like(&YouTubeClient::WEB_REMIX, video_id, LikeAction::RemoveLike)
            .await
    }

    /// Subscribe to an artist.
    ///
    /// # Errors
    ///
    /// Propagates HTTP errors.
    pub async fn subscribe_artist(&self, channel_ids: &[&str]) -> Result<()> {
        self.inner
            .subscribe(&YouTubeClient::WEB_REMIX, channel_ids)
            .await
    }

    /// Unsubscribe from an artist.
    ///
    /// # Errors
    ///
    /// Propagates HTTP errors.
    pub async fn unsubscribe_artist(&self, channel_ids: &[&str]) -> Result<()> {
        self.inner
            .unsubscribe(&YouTubeClient::WEB_REMIX, channel_ids)
            .await
    }

    // ── Playlist management ──────────────────────────────────────────────────

    /// Create a new playlist and return its ID.
    ///
    /// # Errors
    ///
    /// Propagates HTTP errors.
    pub async fn create_playlist(
        &self,
        title: &str,
        description: Option<&str>,
        privacy: PlaylistPrivacy,
        video_ids: &[&str],
    ) -> Result<String> {
        self.inner
            .create_playlist(
                &YouTubeClient::WEB_REMIX,
                title,
                description,
                privacy,
                video_ids,
            )
            .await
    }

    /// Add videos to an existing playlist.
    ///
    /// # Errors
    ///
    /// Propagates HTTP errors.
    pub async fn add_to_playlist(
        &self,
        playlist_id: &str,
        video_ids: &[&str],
    ) -> Result<()> {
        self.inner
            .add_to_playlist(
                &YouTubeClient::WEB_REMIX,
                playlist_id,
                video_ids,
                AddDedupOption::None,
            )
            .await
    }

    /// Remove a video from a playlist (requires the `set_video_id` from the playlist song).
    ///
    /// # Errors
    ///
    /// Propagates HTTP errors.
    pub async fn remove_from_playlist(
        &self,
        playlist_id: &str,
        video_id: &str,
        set_video_id: &str,
    ) -> Result<()> {
        self.inner
            .remove_from_playlist(
                &YouTubeClient::WEB_REMIX,
                playlist_id,
                video_id,
                set_video_id,
            )
            .await
    }

    /// Edit playlist metadata.
    ///
    /// # Errors
    ///
    /// Propagates HTTP errors.
    pub async fn edit_playlist(
        &self,
        playlist_id: &str,
        title: Option<&str>,
        description: Option<&str>,
        privacy: Option<PlaylistPrivacy>,
    ) -> Result<()> {
        self.inner
            .edit_playlist(
                &YouTubeClient::WEB_REMIX,
                playlist_id,
                title,
                description,
                privacy,
            )
            .await
    }

    /// Delete a user-owned playlist.
    ///
    /// # Errors
    ///
    /// Propagates HTTP errors.
    pub async fn delete_playlist(&self, playlist_id: &str) -> Result<()> {
        self.inner
            .delete_playlist(&YouTubeClient::WEB_REMIX, playlist_id)
            .await
    }

    // ── Account ──────────────────────────────────────────────────────────────

    /// Retrieve account info (name, email, avatar) for the logged-in user.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Unauthenticated`] when not logged in. Propagates HTTP errors.
    pub async fn account(&self) -> Result<AccountMenuResponse> {
        if !self.inner.is_logged_in() {
            return Err(Error::Unauthenticated);
        }
        self.inner.account_menu(&YouTubeClient::WEB_REMIX).await
    }
}

// ─────────────────────────────────────────────
// Return type for search
// ─────────────────────────────────────────────

/// Return value of [`YouTube::search`].
#[derive(Debug, Clone)]
pub enum SearchResult {
    /// No filter was applied — one section per content type.
    Summary(SearchSummaryPage),
    /// A [`SearchFilter`] was applied — flat list of one content type.
    Filtered(SearchPage),
}

