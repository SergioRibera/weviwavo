//! Raw Innertube HTTP client — every method corresponds to one API endpoint.
//!
//! This is the lowest level of the ytdroid stack. Callers should prefer the
//! higher-level [`crate::YouTube`] API unless they need raw JSON access.

use std::collections::HashMap;
use std::sync::{Arc, OnceLock};

use reqwest::{Client, StatusCode};
use serde_json::{Value, json};
use tracing::{debug, instrument, warn};

use crate::auth::{parse_cookies, sapisidhash};
use crate::client::{Locale, YouTubeClient, MUSIC_API_BASE, MUSIC_ORIGIN, MUSIC_REFERER};
use crate::error::{Error, Result};
use crate::response::{
    AccountMenuResponse, BrowseResponse, GetQueueResponse, GetSearchSuggestionsResponse,
    GetTranscriptResponse, NextResponse, PlayerResponse, SearchResponse,
};

/// Low-level Innertube HTTP client.
///
/// Construct via [`InnerTube::new`]. Handles auth, retry on 429, and JSON parsing.
#[derive(Clone)]
pub struct InnerTube {
    http: Client,
    locale: Locale,
    cookies: HashMap<String, String>,
    /// Visitor ID sent/received as `X-Goog-Visitor-Id`.
    ///
    /// Shared across clones so that a response-captured value is immediately
    /// visible to all holders (first-write-wins via [`OnceLock`]).
    visitor_id: Arc<OnceLock<String>>,
    /// OAuth2 Bearer token for Android/iOS clients (set via [`YouTube::with_bearer_token`]).
    pub bearer_token: Option<String>,
}

impl InnerTube {
    /// Create a new client.
    ///
    /// `cookie_header` — the raw `Cookie:` header value from a logged-in browser session.
    /// Pass `""` / `None` for unauthenticated use.
    ///
    /// # Errors
    ///
    /// Returns an error if the underlying HTTP client cannot be constructed.
    pub fn new(cookie_header: Option<&str>, locale: Locale) -> Result<Self> {
        let http = Client::builder()
            .gzip(true)
            .deflate(true)
            .user_agent("Mozilla/5.0")
            .build()?;
        let cookies = cookie_header
            .map(parse_cookies)
            .unwrap_or_default();
        Ok(Self {
            http,
            locale,
            cookies,
            visitor_id: Arc::new(OnceLock::new()),
            bearer_token: None,
        })
    }

    /// True when cookie-based or bearer-token auth is present.
    #[must_use]
    pub fn is_logged_in(&self) -> bool {
        self.bearer_token.is_some()
            || self.cookies.contains_key("SAPISID")
            || self.cookies.contains_key("__Secure-3PAPISID")
    }

    /// Returns the visitor ID captured from response headers, if any.
    #[must_use]
    pub fn visitor_data(&self) -> Option<String> {
        self.visitor_id.get().cloned()
    }

    /// Seed the visitor ID (e.g. from a previously stored value).
    ///
    /// Silently ignored if a value was already set (first-write-wins).
    pub(crate) fn set_visitor_id(&self, vid: String) {
        self.visitor_id.set(vid).ok();
    }

    // ── Internal helpers ─────────────────────────────────────────────────────

    fn build_context(&self, client: &YouTubeClient) -> Value {
        let mut ctx = json!({
            "clientName": client.client_name,
            "clientVersion": client.client_version,
            "gl": self.locale.gl,
            "hl": self.locale.hl,
        });
        let obj = ctx.as_object_mut().unwrap();
        if let Some(vid) = self.visitor_id.get() {
            obj.insert("visitorData".into(), json!(vid));
        }
        if client.include_user_agent_in_context {
            obj.insert("userAgent".into(), json!(client.user_agent));
        }
        if let Some(v) = client.context_extra.os_name {
            obj.insert("osName".into(), json!(v));
        }
        if let Some(v) = client.context_extra.os_version {
            obj.insert("osVersion".into(), json!(v));
        }
        if let Some(v) = client.context_extra.device_make {
            obj.insert("deviceMake".into(), json!(v));
        }
        if let Some(v) = client.context_extra.device_model {
            obj.insert("deviceModel".into(), json!(v));
        }
        if let Some(v) = client.context_extra.android_sdk_version {
            // Must be an integer — "31" (string) causes INVALID_ARGUMENT 400.
            if let Ok(n) = v.parse::<u32>() {
                obj.insert("androidSdkVersion".into(), json!(n));
            }
        }
        // WEB-family clients (use_web_po_tokens=true): full context with request/user.
        // App/mobile clients (bearer, VR, iOS, VisionOS, ANDROID): minimal — just client.
        // Metrolist confirms ANDROID player body has no request/user sections.
        let mut out = if client.use_web_po_tokens {
            json!({
                "client": ctx,
                "request": {
                    "internalExperimentFlags": [],
                    "useSsl": true
                },
                "user": {
                    "lockedSafetyMode": false
                }
            })
        } else {
            json!({ "client": ctx })
        };
        if client.is_embedded {
            out.as_object_mut().unwrap().insert(
                "thirdParty".into(),
                json!({ "embedUrl": "https://www.reddit.com/" }),
            );
        }
        out
    }

    fn cookie_header(&self) -> String {
        self.cookies
            .iter()
            .map(|(k, v)| format!("{k}={v}"))
            .collect::<Vec<_>>()
            .join("; ")
    }

    async fn post_raw(
        &self,
        endpoint: &str,
        client: &YouTubeClient,
        body: Value,
    ) -> Result<Value> {
        let base = client.api_base.unwrap_or(MUSIC_API_BASE);
        // When using Bearer auth, omit the API key — Google rejects key+Bearer combinations.
        let has_active_bearer = client.use_bearer_auth && self.bearer_token.is_some();
        let url = if has_active_bearer {
            format!("{base}{endpoint}?prettyPrint=false")
        } else {
            match client.api_key {
                Some(key) => format!("{base}{endpoint}?key={key}&prettyPrint=false"),
                None => format!("{base}{endpoint}?prettyPrint=false"),
            }
        };
        if client.use_bearer_auth {
            debug!(
                url,
                body = %serde_json::to_string(&body).unwrap_or_default(),
                has_bearer = self.bearer_token.is_some(),
                "bearer request"
            );
        }
        let cookie_str = self.cookie_header();

        // Common headers for every client.
        let mut req = self
            .http
            .post(&url)
            .header("Content-Type", "application/json")
            .header("Accept", "application/json")
            .header("X-YouTube-Client-Name", client.client_id)
            .header("X-YouTube-Client-Version", client.client_version)
            .header("User-Agent", client.user_agent);

        // yt-dlp always sends Origin matching the API host. Required to avoid bot detection.
        let api_origin = if client.api_base.is_none() {
            MUSIC_ORIGIN
        } else {
            "https://www.youtube.com"
        };
        req = req.header("Origin", api_origin);

        if client.use_bearer_auth {
            // OAuth2 Bearer path — Authorization + X-Goog-AuthUser (yt-dlp includes this for all auth).
            if let Some(token) = &self.bearer_token {
                req = req
                    .header("Authorization", format!("Bearer {token}"))
                    .header("X-Goog-AuthUser", "0");
            }
        } else {
            // Music API clients (api_base=None → music.youtube.com): extra music-specific headers.
            let is_music_api = client.api_base.is_none();
            if is_music_api {
                req = req
                    .header("X-Goog-Api-Format-Version", "1")
                    .header("X-Origin", MUSIC_ORIGIN)
                    .header("Referer", MUSIC_REFERER);
            }
            if client.login_supported && !cookie_str.is_empty() {
                req = req.header("Cookie", &cookie_str);
                if let Some(auth) = sapisidhash(&self.cookies) {
                    req = req.header("Authorization", auth);
                }
            }
        }
        if let Some(vid) = self.visitor_id.get() {
            req = req.header("X-Goog-Visitor-Id", vid);
        }

        req = req.json(&body);

        let resp = self.with_retry(req).await?;
        let status = resp.status();
        if !status.is_success() {
            let body = resp.text().await.unwrap_or_default();
            warn!(endpoint, client = client.client_name, %status, body = %body, "API non-success");
            return Err(Error::HttpStatus { status: status.as_u16() });
        }
        // Capture visitor ID from response header (set before consuming body).
        if let Some(v) = resp
            .headers()
            .get("X-Goog-Visitor-Id")
            .and_then(|h| h.to_str().ok())
            .filter(|s| !s.is_empty())
        {
            self.visitor_id.set(v.to_owned()).ok();
        }
        let value = resp.json::<Value>().await.map_err(Error::from)?;
        tracing::trace!(endpoint, raw_response = %value, "innertube raw response");
        // Also capture from responseContext.visitorData in body (yt-dlp does this too).
        if let Some(vd) = value
            .pointer("/responseContext/visitorData")
            .and_then(|v| v.as_str())
            .filter(|s| !s.is_empty())
        {
            self.visitor_id.set(vd.to_owned()).ok();
        }
        Ok(value)
    }

    async fn with_retry(
        &self,
        req: reqwest::RequestBuilder,
    ) -> Result<reqwest::Response> {
        let mut attempts = 0u32;
        loop {
            // `try_clone` is available because we only use json bodies (not streams).
            let r = req.try_clone().ok_or(Error::MissingField {
                field: "request clone",
            })?;
            let resp = r.send().await?;
            if resp.status() == StatusCode::TOO_MANY_REQUESTS && attempts < 3 {
                attempts += 1;
                warn!("429 Too Many Requests — waiting before retry {attempts}");
                tokio::time::sleep(std::time::Duration::from_secs(u64::from(attempts) * 2)).await;
                continue;
            }
            return Ok(resp);
        }
    }

    fn body(&self, client: &YouTubeClient, extra: &Value) -> Value {
        let mut obj = json!({ "context": self.build_context(client) });
        if let (Some(base), Some(ext)) = (obj.as_object_mut(), extra.as_object()) {
            for (k, v) in ext {
                base.insert(k.clone(), v.clone());
            }
        }
        obj
    }

    // ── Endpoints ────────────────────────────────────────────────────────────

    /// `browse` — the universal page-fetch endpoint.
    ///
    /// # Errors
    ///
    /// Propagates HTTP, auth, or JSON parsing errors.
    #[instrument(skip(self), fields(browse_id, params, continuation))]
    pub async fn browse(
        &self,
        client: &YouTubeClient,
        browse_id: &str,
        params: Option<&str>,
        continuation: Option<&str>,
    ) -> Result<BrowseResponse> {
        debug!("browse {browse_id}");
        let mut extra = json!({ "browseId": browse_id });
        if let Some(p) = params {
            extra["params"] = json!(p);
        }
        if let Some(c) = continuation {
            extra["continuation"] = json!(c);
        }
        let raw = self.post_raw("browse", client, self.body(client, &extra)).await?;
        Ok(serde_json::from_value(raw)?)
    }

    /// `search` — search with optional `params` (filter) and `continuation`.
    ///
    /// # Errors
    ///
    /// Propagates HTTP, auth, or JSON parsing errors.
    #[instrument(skip(self), fields(query))]
    pub async fn search(
        &self,
        client: &YouTubeClient,
        query: &str,
        params: Option<&str>,
        continuation: Option<&str>,
    ) -> Result<SearchResponse> {
        debug!("search {query:?}");
        let mut extra = json!({ "query": query });
        if let Some(p) = params {
            extra["params"] = json!(p);
        }
        if let Some(c) = continuation {
            extra["continuation"] = json!(c);
        }
        let raw = self.post_raw("search", client, self.body(client, &extra)).await?;
        Ok(serde_json::from_value(raw)?)
    }

    /// `player` — fetch playback / streaming data for a video ID.
    ///
    /// `po_token` is only included for clients with `use_web_po_tokens = true`; pass `None` for
    /// all other clients.
    ///
    /// # Errors
    ///
    /// Propagates HTTP, auth, or JSON parsing errors.
    #[instrument(skip(self), fields(video_id))]
    pub async fn player(
        &self,
        client: &YouTubeClient,
        video_id: &str,
        playlist_id: Option<&str>,
        signature_timestamp: Option<u32>,
        po_token: Option<&str>,
    ) -> Result<PlayerResponse> {
        debug!("player {video_id}");
        let mut extra = json!({
            "videoId": video_id,
            "racyCheckOk": true,
            "contentCheckOk": true,
        });
        if let Some(pid) = playlist_id {
            extra["playlistId"] = json!(pid);
        }
        if let Some(ts) = signature_timestamp.filter(|_| client.use_signature_timestamp) {
            extra["playbackContext"] = json!({
                "contentPlaybackContext": {
                    "signatureTimestamp": ts
                }
            });
        }
        // Player PoToken goes to ALL clients that support it (not just WEB).
        // android, android_vr, ios all have PLAYER_PO_TOKEN_POLICY in yt-dlp.
        // With a player token, GVS streaming token is not required (not_required_with_player_token=true).
        if let Some(token) = po_token {
            extra["serviceIntegrityDimensions"] = json!({ "poToken": token });
        }
        let raw = self.post_raw("player", client, self.body(client, &extra)).await?;
        debug!(client = client.client_name, raw = %raw, "player raw response");
        Ok(serde_json::from_value(raw)?)
    }

    /// `next` — fetch up-next / related tracks.
    ///
    /// # Errors
    ///
    /// Propagates HTTP, auth, or JSON parsing errors.
    #[allow(clippy::too_many_arguments)] // Each Option is a distinct query field with no natural grouping.
    #[instrument(skip(self), fields(video_id, playlist_id))]
    pub async fn next(
        &self,
        client: &YouTubeClient,
        video_id: Option<&str>,
        playlist_id: Option<&str>,
        params: Option<&str>,
        index: Option<u32>,
        playlist_set_video_id: Option<&str>,
        continuation: Option<&str>,
    ) -> Result<NextResponse> {
        let mut extra = json!({});
        if let Some(vid) = video_id {
            extra["videoId"] = json!(vid);
        }
        if let Some(pid) = playlist_id {
            extra["playlistId"] = json!(pid);
        }
        if let Some(p) = params {
            extra["params"] = json!(p);
        }
        if let Some(i) = index {
            extra["index"] = json!(i);
        }
        if let Some(psid) = playlist_set_video_id {
            extra["playlistSetVideoId"] = json!(psid);
        }
        if let Some(c) = continuation {
            extra["continuation"] = json!(c);
        }
        let raw = self.post_raw("next", client, self.body(client, &extra)).await?;
        Ok(serde_json::from_value(raw)?)
    }

    /// `get_queue` — resolve a set of video / playlist IDs into queue items.
    ///
    /// # Errors
    ///
    /// Propagates HTTP, auth, or JSON parsing errors.
    pub async fn get_queue(
        &self,
        client: &YouTubeClient,
        video_ids: &[&str],
        playlist_id: Option<&str>,
    ) -> Result<GetQueueResponse> {
        let mut extra = json!({});
        if !video_ids.is_empty() {
            extra["videoIds"] = json!(video_ids);
        }
        if let Some(pid) = playlist_id {
            extra["playlistId"] = json!(pid);
        }
        let raw = self.post_raw("music/get_queue", client, self.body(client, &extra)).await?;
        Ok(serde_json::from_value(raw)?)
    }

    /// `get_search_suggestions` — autocomplete suggestions.
    ///
    /// # Errors
    ///
    /// Propagates HTTP, auth, or JSON parsing errors.
    pub async fn get_search_suggestions(
        &self,
        client: &YouTubeClient,
        query: &str,
    ) -> Result<GetSearchSuggestionsResponse> {
        let extra = json!({ "input": query });
        let raw = self
            .post_raw("music/get_search_suggestions", client, self.body(client, &extra))
            .await?;
        Ok(serde_json::from_value(raw)?)
    }

    /// `get_transcript` — fetch episode/video transcript.
    ///
    /// # Errors
    ///
    /// Propagates HTTP, auth, or JSON parsing errors.
    pub async fn get_transcript(
        &self,
        client: &YouTubeClient,
        params: &str,
    ) -> Result<GetTranscriptResponse> {
        let extra = json!({ "params": params });
        let raw = self.post_raw("get_transcript", client, self.body(client, &extra)).await?;
        Ok(serde_json::from_value(raw)?)
    }

    /// `feedback` — library add/remove, thumbs feedback.
    ///
    /// # Errors
    ///
    /// Propagates HTTP, auth, or JSON parsing errors.
    pub async fn feedback(
        &self,
        client: &YouTubeClient,
        feedback_tokens: &[&str],
    ) -> Result<()> {
        let extra = json!({ "feedbackTokens": feedback_tokens });
        let _ = self.post_raw("feedback", client, self.body(client, &extra)).await?;
        Ok(())
    }

    /// Like / Dislike / Remove-like via the `like/status` endpoint.
    ///
    /// # Errors
    ///
    /// Propagates HTTP, auth, or JSON parsing errors.
    pub async fn like(
        &self,
        client: &YouTubeClient,
        video_id: &str,
        action: LikeAction,
    ) -> Result<()> {
        let endpoint = match action {
            LikeAction::Like => "like/like",
            LikeAction::Dislike => "like/dislike",
            LikeAction::RemoveLike => "like/removelike",
        };
        let extra = json!({
            "target": { "videoId": video_id }
        });
        let _ = self.post_raw(endpoint, client, self.body(client, &extra)).await?;
        Ok(())
    }

    /// Subscribe to an artist channel.
    ///
    /// # Errors
    ///
    /// Propagates HTTP, auth, or JSON parsing errors.
    pub async fn subscribe(
        &self,
        client: &YouTubeClient,
        channel_ids: &[&str],
    ) -> Result<()> {
        let extra = json!({
            "channelIds": channel_ids,
            "params": "EgIIAhgA"
        });
        let _ = self
            .post_raw("subscription/subscribe", client, self.body(client, &extra))
            .await?;
        Ok(())
    }

    /// Unsubscribe from an artist channel.
    ///
    /// # Errors
    ///
    /// Propagates HTTP, auth, or JSON parsing errors.
    pub async fn unsubscribe(
        &self,
        client: &YouTubeClient,
        channel_ids: &[&str],
    ) -> Result<()> {
        let extra = json!({
            "channelIds": channel_ids,
            "params": "EgIIAhgA"
        });
        let _ = self
            .post_raw("subscription/unsubscribe", client, self.body(client, &extra))
            .await?;
        Ok(())
    }

    /// Create a new user playlist.
    ///
    /// Returns the new playlist's ID on success.
    ///
    /// # Errors
    ///
    /// Propagates HTTP, auth, or JSON parsing errors.
    pub async fn create_playlist(
        &self,
        client: &YouTubeClient,
        title: &str,
        description: Option<&str>,
        privacy: PlaylistPrivacy,
        video_ids: &[&str],
    ) -> Result<String> {
        let mut extra = json!({
            "title": title,
            "status": privacy.as_str(),
        });
        if let Some(d) = description {
            extra["description"] = json!(d);
        }
        if !video_ids.is_empty() {
            extra["videoIds"] = json!(video_ids);
        }
        let raw = self
            .post_raw("playlist/create", client, self.body(client, &extra))
            .await?;
        raw["playlistId"]
            .as_str()
            .map(str::to_owned)
            .ok_or(Error::MissingField { field: "playlistId" })
    }

    /// Add videos to an existing playlist.
    ///
    /// # Errors
    ///
    /// Propagates HTTP, auth, or JSON parsing errors.
    pub async fn add_to_playlist(
        &self,
        client: &YouTubeClient,
        playlist_id: &str,
        video_ids: &[&str],
        dedup_option: AddDedupOption,
    ) -> Result<()> {
        let extra = json!({
            "playlistId": playlist_id,
            "actions": video_ids.iter().map(|id| json!({
                "addedVideoId": id,
                "action": "ACTION_ADD_VIDEO"
            })).collect::<Vec<_>>(),
            "params": dedup_option.as_str(),
        });
        let _ = self
            .post_raw("browse/edit_playlist", client, self.body(client, &extra))
            .await?;
        Ok(())
    }

    /// Remove a video from a playlist using its `setVideoId`.
    ///
    /// # Errors
    ///
    /// Propagates HTTP, auth, or JSON parsing errors.
    pub async fn remove_from_playlist(
        &self,
        client: &YouTubeClient,
        playlist_id: &str,
        video_id: &str,
        set_video_id: &str,
    ) -> Result<()> {
        let extra = json!({
            "playlistId": playlist_id,
            "actions": [{
                "setVideoId": set_video_id,
                "removedVideoId": video_id,
                "action": "ACTION_REMOVE_VIDEO"
            }]
        });
        let _ = self
            .post_raw("browse/edit_playlist", client, self.body(client, &extra))
            .await?;
        Ok(())
    }

    /// Edit playlist metadata (title, description, privacy).
    ///
    /// # Errors
    ///
    /// Propagates HTTP, auth, or JSON parsing errors.
    pub async fn edit_playlist(
        &self,
        client: &YouTubeClient,
        playlist_id: &str,
        title: Option<&str>,
        description: Option<&str>,
        privacy: Option<PlaylistPrivacy>,
    ) -> Result<()> {
        let mut actions: Vec<Value> = Vec::new();
        if let Some(t) = title {
            actions.push(json!({ "action": "ACTION_SET_PLAYLIST_NAME", "playlistName": t }));
        }
        if let Some(d) = description {
            actions.push(json!({ "action": "ACTION_SET_PLAYLIST_DESCRIPTION", "playlistDescription": d }));
        }
        if let Some(p) = privacy {
            actions.push(json!({ "action": "ACTION_SET_PLAYLIST_PRIVACY_STATUS", "playlistPrivacyStatus": p.as_str() }));
        }
        if actions.is_empty() {
            return Ok(());
        }
        let extra = json!({ "playlistId": playlist_id, "actions": actions });
        let _ = self
            .post_raw("browse/edit_playlist", client, self.body(client, &extra))
            .await?;
        Ok(())
    }

    /// Delete a user-owned playlist.
    ///
    /// # Errors
    ///
    /// Propagates HTTP, auth, or JSON parsing errors.
    pub async fn delete_playlist(
        &self,
        client: &YouTubeClient,
        playlist_id: &str,
    ) -> Result<()> {
        let extra = json!({ "playlistId": playlist_id });
        let _ = self
            .post_raw("playlist/delete", client, self.body(client, &extra))
            .await?;
        Ok(())
    }

    /// `account/get_setting` — retrieve the current account menu (includes email/name/photo).
    ///
    /// # Errors
    ///
    /// Propagates HTTP, auth, or JSON parsing errors.
    pub async fn account_menu(
        &self,
        client: &YouTubeClient,
    ) -> Result<AccountMenuResponse> {
        let extra = json!({});
        let raw = self
            .post_raw("account/get_setting", client, self.body(client, &extra))
            .await?;
        Ok(serde_json::from_value(raw)?)
    }

    /// Report playback to `s.youtube.com/api/stats/playback` so the track
    /// appears in the YouTube Music listening history on all devices.
    ///
    /// Fire-and-forget — errors are non-fatal. Requires a logged-in session
    /// (cookies) to be recorded in history; unauthenticated calls are silently
    /// ignored by YouTube.
    ///
    /// # Errors
    ///
    /// Propagates HTTP errors.
    pub async fn report_playback(&self, video_id: &str, duration_secs: u32) -> Result<()> {
        use base64::Engine as _;
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        let hash = sha1_smol::Sha1::from(format!("{nanos}{video_id}").as_bytes())
            .digest()
            .bytes();
        let cpn = base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(&hash[..12]);
        let len = duration_secs.to_string();
        let et = (duration_secs.min(30)).to_string();

        let cookie_header = self
            .cookies
            .iter()
            .map(|(k, v)| format!("{k}={v}"))
            .collect::<Vec<_>>()
            .join("; ");

        let mut req = self
            .http
            .post("https://music.youtube.com/api/stats/watchtime")
            .query(&[
                ("ns", "yt"),
                ("el", "detailpage"),
                ("cpn", cpn.as_str()),
                ("docid", video_id),
                ("ver", "2"),
                ("c", YouTubeClient::WEB_REMIX.client_name),
                ("cver", YouTubeClient::WEB_REMIX.client_version),
                ("cplayer", "UNIPLAYER"),
                ("referrer", MUSIC_REFERER),
                ("len", len.as_str()),
                ("st", "0"),
                ("et", et.as_str()),
                ("cmt", et.as_str()),
                ("final", "1"),
                ("state", "paused"),
                ("volume", "100"),
                ("muted", "0"),
                ("vis", "1"),
            ]);

        if !cookie_header.is_empty() {
            req = req.header("cookie", &cookie_header);
        }

        let resp = req.send().await?;
        let status = resp.status();
        let body = resp.text().await.unwrap_or_default();
        debug!(video_id, %status, cpn, body = %body, "watchtime report response");
        Ok(())
    }
}

// ─────────────────────────────────────────────
// Action enums
// ─────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LikeAction {
    Like,
    Dislike,
    RemoveLike,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlaylistPrivacy {
    Public,
    Private,
    Unlisted,
}

impl PlaylistPrivacy {
    fn as_str(self) -> &'static str {
        match self {
            Self::Public => "PUBLIC",
            Self::Private => "PRIVATE",
            Self::Unlisted => "UNLISTED",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AddDedupOption {
    /// Allow duplicate videos (default).
    None,
    /// Skip already-present videos.
    SkipDuplicates,
}

impl AddDedupOption {
    fn as_str(self) -> &'static str {
        match self {
            Self::None => "PLAYLIST_EDIT_PARAMS_ACTIONS_ADD",
            Self::SkipDuplicates => "PLAYLIST_EDIT_PARAMS_ACTIONS_ADD_SKIP_DUPLICATES",
        }
    }
}
