use freya::radio::RadioStation;
use tokio::sync::mpsc;
use ytdroid::client::Locale;
use ytdroid::YouTube;

use ytdroid::filters::browse_id;
use ytdroid::models::YTItem;

use crate::app::{AlbumViewData, Data, DataChannel, NavCommand, PlaylistViewData, QueueSong};
use crate::audio::{AudioCommand, AudioEngine, AudioQuality, run_audio_engine};

pub async fn run_startup(mut radio: RadioStation<Data, DataChannel>) {
    let (engine, audio_rx) = AudioEngine::new();
    let (nav_tx, nav_rx) = mpsc::channel::<NavCommand>(16);

    let sender = engine.sender();
    {
        radio.write_channel(DataChannel::Player).player.volume = crate::prefs::load_volume();
    }
    {
        let mut state = radio.write_channel(DataChannel::Navigation);
        state.audio_cmd = Some(sender.clone());
        state.nav_cmd = Some(nav_tx);
    }

    sender.try_send(AudioCommand::SetVolume(radio.read().player.volume)).ok();

    tokio::join!(
        startup_inner(radio),
        run_audio_engine(audio_rx, radio),
        run_nav_engine(nav_rx, radio),
    );
}

async fn startup_inner(mut radio: RadioStation<Data, DataChannel>) {
    let cookie_header = radio.read().cookie_header.clone();

    let yt_base = match &cookie_header {
        Some(h) => YouTube::new(Some(h), Locale::default()),
        None => YouTube::new(None, Locale::default()),
    };

    let Ok(yt_base) = yt_base.inspect_err(|e| tracing::error!(error = %e, "failed to create YT client")) else {
        return;
    };

    // Seed a persisted visitor_id so YouTube doesn't bot-detect our first request.
    // yt-dlp always sends X-Goog-Visitor-Id on every request (from ytcfg page fetch).
    // We skip the page fetch but persist and reload the token across sessions.
    let yt_base = match crate::visitor_id::load() {
        Some(vid) => {
            tracing::debug!("seeding visitor_id from disk");
            yt_base.with_visitor_id(vid)
        }
        None => yt_base,
    };

    let yt = yt_base;

    {
        let mut data = radio.write_channel(DataChannel::YtApi);
        data.yt_session = Some(yt.clone());
        if let Some(h) = cookie_header {
            data.cookie_header = Some(h);
        }
    }

    // Pre-warm player JS cache + QuickJS worker in the background so the
    // first song play doesn't pay the cold-start cost (base.js fetch + parse).
    tokio::spawn(crate::audio::nsig::warm());

    match yt.home(None).await {
        Ok(mut feed) => {
            // Fetch continuation pages until exhausted or capped (YT paginates the home feed).
            let mut cont = feed.continuation.take();
            let mut page_count = 0u8;
            while let Some(token) = cont.take() {
                if page_count >= 3 {
                    break;
                }
                page_count += 1;
                match yt.home_continuation(&token).await {
                    Ok(mut more) => {
                        feed.sections.append(&mut more.sections);
                        cont = more.continuation.take();
                    }
                    Err(e) => {
                        tracing::warn!(error = %e, "home continuation failed");
                        break;
                    }
                }
            }
            tracing::debug!(sections = feed.sections.len(), "home feed loaded");
            radio.write_channel(DataChannel::Feed).feed = feed;
        }
        Err(e) => {
            tracing::error!(error = %e, "failed to fetch home feed");
        }
    }

    // Persist visitor_id after the first successful API call (home feed captures it).
    if let Some(vid) = yt.visitor_id() {
        crate::visitor_id::save(&vid);
        tracing::debug!("visitor_id persisted to disk");
    }

    match yt.library(browse_id::LIBRARY_PLAYLISTS, None).await {
        Ok(page) => {
            let playlists = page
                .items
                .into_iter()
                .filter_map(|item| match item {
                    YTItem::Playlist(p) => Some(p),
                    _ => None,
                })
                .collect();
            radio.write_channel(DataChannel::YtApi).library_playlists = playlists;
        }
        Err(e) => {
            tracing::warn!(error = %e, "library playlists unavailable");
        }
    }
}

async fn run_nav_engine(
    mut rx: mpsc::Receiver<NavCommand>,
    mut radio: RadioStation<Data, DataChannel>,
) {
    while let Some(cmd) = rx.recv().await {
        match cmd {
            NavCommand::BeginNavigation { playlist_id } => {
                let mut state = radio.write_channel(DataChannel::Navigation);
                state.is_loading = true;
                state.pending_playlist_id = Some(playlist_id);
            }

            NavCommand::ClearPending => {
                radio.write_channel(DataChannel::Navigation).pending_playlist_id = None;
            }

            NavCommand::LoadPlaylist(browse_id) => {
                let yt = radio.read().yt_session.clone();
                let Some(yt) = yt else {
                    tracing::warn!(browse_id, "no YT session for playlist load");
                    continue;
                };

                match yt.playlist(&browse_id).await {
                    Ok(page) => {
                        let playlist_id = page.playlist.playlist_id().to_owned();

                        // Follow continuations to get all songs (cap at 10 pages).
                        let mut songs = page.songs;
                        let mut cont = page.songs_continuation;
                        let mut page_count = 0u8;
                        while let Some(token) = cont.take() {
                            if page_count >= 10 {
                                break;
                            }
                            page_count += 1;
                            match yt.playlist_continuation(&token).await {
                                Ok(cp) => {
                                    songs.extend(cp.songs);
                                    cont = cp.continuation;
                                }
                                Err(e) => {
                                    tracing::warn!(error = %e, "playlist continuation failed");
                                    break;
                                }
                            }
                        }

                        // Suggestions: up-next queue seeded with the first song.
                        let first_id = songs.first().map(|s| s.id.clone());
                        let next_page = yt
                            .next(
                                first_id.as_deref(),
                                Some(&playlist_id),
                                None,
                                None,
                                None,
                            )
                            .await;

                        let suggestions = match &next_page {
                            Ok(p) => {
                                let items: Vec<_> = p.items.iter().map(|i| i.song.clone()).collect();
                                tracing::debug!(count = items.len(), "suggestions loaded");
                                items
                            }
                            Err(e) => {
                                tracing::warn!(error = %e, "next endpoint failed — no suggestions");
                                Vec::new()
                            }
                        };

                        // Related playlists from the "Related" tab browse endpoint.
                        let related_playlists = next_page
                            .ok()
                            .and_then(|p| p.related_browse_id)
                            .map(|rbid| {
                                let yt = yt.clone();
                                async move {
                                    yt.related(&rbid)
                                        .await
                                        .map(|rp| rp.playlists)
                                        .unwrap_or_default()
                                }
                            });
                        let related_playlists = match related_playlists {
                            Some(fut) => fut.await,
                            None => Vec::new(),
                        };

                        {
                            let mut state = radio.write_channel(DataChannel::Navigation);
                            state.playlist_view = Some(PlaylistViewData {
                                playlist: page.playlist,
                                songs,
                                suggestions,
                                related_playlists,
                            });
                            state.is_loading = false;
                        }
                    }
                    Err(e) => {
                        tracing::error!(error = %e, browse_id, "failed to fetch playlist");
                        radio.write_channel(DataChannel::Navigation).is_loading = false;
                    }
                }
            }

            NavCommand::PlayPlaylist(browse_id) => {
                let yt = radio.read().yt_session.clone();
                let audio_cmd = radio.read().audio_cmd.clone();

                let (Some(yt), Some(tx)) = (yt, audio_cmd) else {
                    tracing::warn!(browse_id, "no YT session or audio_cmd for playlist play");
                    continue;
                };

                match yt.playlist(&browse_id).await {
                    Ok(page) => {
                        if !page.songs.is_empty() {
                            let songs: Vec<QueueSong> = page.songs.iter().map(|s| QueueSong {
                                video_id: s.id.clone(),
                                title: s.title.clone(),
                                artist: s.artists.first().map(|a| a.name.clone()).unwrap_or_default(),
                                album: s.album.as_ref().map(|a| a.name.clone()).unwrap_or_default(),
                                thumbnail_url: s.thumbnail.clone().unwrap_or_default(),
                                like_status: s.like_status.clone(),
                            }).collect();
                            tx.try_send(AudioCommand::PlayFromQueue {
                                songs,
                                index: 0,
                                quality: AudioQuality::Medium,
                            })
                            .ok();
                        }
                    }
                    Err(e) => {
                        tracing::error!(error = %e, browse_id, "failed to fetch playlist for playback");
                    }
                }
            }

            NavCommand::LoadAlbum(browse_id) => {
                let yt = radio.read().yt_session.clone();
                let Some(yt) = yt else {
                    tracing::warn!(browse_id, "no YT session for album load");
                    continue;
                };

                match yt.album(&browse_id).await {
                    Ok(mut page) => {
                        page.album.id = browse_id.clone();
                        radio.write_channel(DataChannel::Navigation).album_view =
                            Some(AlbumViewData {
                                album: page.album,
                                songs: page.songs,
                            });
                    }
                    Err(e) => {
                        tracing::error!(error = %e, browse_id, "failed to fetch album");
                    }
                }
            }
        }
    }
}
