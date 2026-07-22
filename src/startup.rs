use freya::radio::RadioStation;
use tokio::sync::mpsc;
use ytdroid::client::Locale;
use ytdroid::YouTube;

use crate::app::{Data, DataChannel, NavCommand, PlaylistViewData};
use crate::audio::{AudioCommand, AudioEngine, AudioQuality, run_audio_engine};

pub async fn run_startup(mut radio: RadioStation<Data, DataChannel>) {
    let (engine, audio_rx) = AudioEngine::new();
    let (nav_tx, nav_rx) = mpsc::channel::<NavCommand>(16);

    let sender = engine.sender();
    {
        let mut state = radio.write_channel(DataChannel::Player);
        state.audio_cmd = Some(sender.clone());
        state.player.volume = crate::prefs::load_volume();
    }
    {
        radio.write_channel(DataChannel::Navigation).nav_cmd = Some(nav_tx);
    }

    sender.try_send(AudioCommand::SetVolume(radio.read().player.volume)).ok();

    tokio::join!(
        startup_inner(radio),
        run_audio_engine(audio_rx, radio),
        run_nav_engine(nav_rx, radio),
    );
}

async fn startup_inner(mut radio: RadioStation<Data, DataChannel>) {
    let Some(cookie_header) = radio.read().cookie_header.clone() else {
        tracing::warn!("no cookies available — skipping YT client init");
        return;
    };

    let yt = YouTube::new(Some(&cookie_header), Locale::default())
        .inspect_err(|e| tracing::error!(error = %e, "failed to create YT client"));

    tracing::debug!(success = yt.is_ok(), "YT client creation finished");

    if let Ok(yt) = yt {
        {
            let mut data = radio.write_channel(DataChannel::YtApi);
            data.yt_session = Some(yt.clone());
            data.cookie_header = Some(cookie_header);
        }
        match yt.home(None).await {
            Ok(feed) => {
                tracing::debug!(sections = feed.sections.len(), "home feed loaded");
                radio.write_channel(DataChannel::Feed).feed = feed;
            }
            Err(e) => {
                tracing::error!(error = %e, "failed to fetch home feed");
            }
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

                        let suggestions = next_page
                            .as_ref()
                            .map(|p| p.items.iter().map(|i| i.song.clone()).collect::<Vec<_>>())
                            .unwrap_or_default();

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
                        if let Some(first) = page.songs.first() {
                            tx.try_send(AudioCommand::Play {
                                video_id: first.id.clone(),
                                quality: AudioQuality::Medium,
                                title: first.title.clone(),
                                artist: first
                                    .artists
                                    .first()
                                    .map(|a| a.name.clone())
                                    .unwrap_or_default(),
                                album: first
                                    .album
                                    .as_ref()
                                    .map(|a| a.name.clone())
                                    .unwrap_or_default(),
                                thumbnail_url: first.thumbnail.clone().unwrap_or_default(),
                            })
                            .ok();
                        }
                    }
                    Err(e) => {
                        tracing::error!(error = %e, browse_id, "failed to fetch playlist for playback");
                    }
                }
            }
        }
    }
}
