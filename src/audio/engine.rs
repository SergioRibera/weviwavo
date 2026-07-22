use std::io::Cursor;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use freya::radio::RadioStation;
use rodio::Decoder;
use rodio::source::{SeekError, Source};
use tokio::sync::mpsc;
use tracing::{error, info};

use crate::app::{Data, DataChannel, QueueSong};
use crate::audio::{AudioQuality, youtube::fetch_audio_bytes};

const PROGRESS_INTERVAL: Duration = Duration::from_millis(250);

#[derive(Debug)]
pub enum AudioCommand {
    Play {
        video_id: String,
        quality: AudioQuality,
        title: String,
        artist: String,
        album: String,
        thumbnail_url: String,
    },
    PlayFromQueue {
        songs: Vec<QueueSong>,
        index: usize,
        quality: AudioQuality,
    },
    Next,
    Previous,
    Pause,
    Resume,
    Stop,
    Seek(f32),
    SetVolume(f32),
}

enum RodioCmd {
    Play(Vec<u8>, Arc<AtomicBool>),
    Pause,
    Resume,
    Stop,
    Seek(Duration),
    SetVolume(f32),
}

/// Shared progress state written by the rodio thread, read by the engine task.
struct ProgressState {
    current_secs: f32,
    total_secs: f32,
    /// `false` when the decoder could not determine total duration (e.g. some WebM/Opus files).
    /// When unknown, `total_secs` is grown dynamically in `periodic_access` so the seek bar
    /// remains functional even without a reliable total.
    total_known: bool,
}

impl Default for ProgressState {
    fn default() -> Self {
        Self { current_secs: 0.0, total_secs: 0.0, total_known: false }
    }
}

/// Wraps a [`Source`] and sets `flag` to `true` when the inner source is
/// exhausted, allowing the engine to detect song completion without polling.
struct CompletionNotifier<S: Source> {
    inner: S,
    flag: Arc<AtomicBool>,
    notified: bool,
}

impl<S: Source> Iterator for CompletionNotifier<S> {
    type Item = rodio::Sample;

    fn next(&mut self) -> Option<Self::Item> {
        let item = self.inner.next();
        if item.is_none() && !self.notified {
            self.notified = true;
            self.flag.store(true, Ordering::Relaxed);
        }
        item
    }
}

impl<S: Source> Source for CompletionNotifier<S> {
    fn current_span_len(&self) -> Option<usize> {
        self.inner.current_span_len()
    }

    fn channels(&self) -> rodio::ChannelCount {
        self.inner.channels()
    }

    fn sample_rate(&self) -> rodio::SampleRate {
        self.inner.sample_rate()
    }

    fn total_duration(&self) -> Option<Duration> {
        self.inner.total_duration()
    }

    fn try_seek(&mut self, pos: Duration) -> Result<(), SeekError> {
        let result = self.inner.try_seek(pos);
        if result.is_ok() {
            self.notified = false;
        }
        result
    }
}

/// Handle for sending commands to the audio engine.
pub struct AudioEngine {
    tx: mpsc::Sender<AudioCommand>,
}

impl AudioEngine {
    /// Creates a new `AudioEngine` and returns it together with the receiver
    /// that must be passed to [`run_audio_engine`].  Because
    /// [`RadioStation`] is `!Send`, the engine loop cannot be spawned with
    /// `tokio::spawn`; instead, await `run_audio_engine` from a task that
    /// already holds the station (e.g. the Freya `with_future` startup task).
    pub fn new() -> (Self, mpsc::Receiver<AudioCommand>) {
        let (tx, rx) = mpsc::channel(32);
        (Self { tx }, rx)
    }

    pub fn sender(&self) -> mpsc::Sender<AudioCommand> {
        self.tx.clone()
    }
}

/// Drive the audio engine loop.  Call this from within the `!Send` context
/// that owns the [`RadioStation`] (Freya's startup future).
pub async fn run_audio_engine(
    mut rx: mpsc::Receiver<AudioCommand>,
    mut radio: RadioStation<Data, DataChannel>,
) {
    let (rodio_tx, rodio_rx) = std::sync::mpsc::channel::<RodioCmd>();
    let (fetch_tx, mut fetch_rx) = mpsc::channel::<Result<Vec<u8>, String>>(4);

    let progress = Arc::new(Mutex::new(ProgressState::default()));
    let progress_for_rodio = Arc::clone(&progress);

    // Shared flag: the currently-playing source sets this to `true` when it
    // runs out of samples.  The engine detects it during the progress tick
    // and advances the queue.
    let song_done = Arc::new(AtomicBool::new(false));

    // The rodio I/O thread is blocking and does not touch Freya state — safe
    // to dispatch to the thread pool.
    tokio::task::spawn_blocking(move || {
        rodio_thread(rodio_rx, progress_for_rodio);
    });

    let mut progress_ticker = tokio::time::interval(PROGRESS_INTERVAL);

    loop {
        tokio::select! {
            maybe_cmd = rx.recv() => {
                let Some(cmd) = maybe_cmd else { break };
                handle_command(cmd, &rodio_tx, &mut radio, &fetch_tx, &song_done).await;
            }
            Some(result) = fetch_rx.recv() => {
                match result {
                    Ok(bytes) => {
                        song_done.store(false, Ordering::Relaxed);
                        rodio_tx.send(RodioCmd::Play(bytes, Arc::clone(&song_done))).ok();
                    }
                    Err(e) => {
                        error!(error = %e, "failed to fetch audio");
                        radio
                            .write_channel(DataChannel::Player)
                            .player
                            .is_playing = false;
                    }
                }
            }
            _ = progress_ticker.tick() => {
                let (current, total) = progress
                    .lock()
                    .map(|p| (p.current_secs, p.total_secs))
                    .unwrap_or((0., 0.));
                {
                    let mut state = radio.write_channel(DataChannel::Player);
                    state.player.current_secs = current;
                    state.player.total_secs = total;
                }

                // Auto-advance when the current source signals completion.
                if song_done.load(Ordering::Relaxed) {
                    song_done.store(false, Ordering::Relaxed);

                    let (queue, idx) = {
                        let p = &radio.read().player;
                        (p.queue.clone(), p.queue_index)
                    };

                    let next_idx = idx + 1;
                    if next_idx < queue.len() {
                        let next = queue[next_idx].clone();
                        let cookies = radio.read().cookie_header.clone();
                        let vol = radio.read().player.volume;

                        info!(
                            next_idx,
                            queue_len = queue.len(),
                            video_id = next.video_id,
                            "auto-advancing to next song"
                        );

                        {
                            let mut s = radio.write_channel(DataChannel::Player);
                            let p = &mut s.player;
                            p.queue_index = next_idx;
                            p.title = next.title.clone();
                            p.artist = next.artist.clone();
                            p.album = next.album.clone();
                            p.thumbnail_url = next.thumbnail_url.clone();
                            p.current_secs = 0.;
                            p.is_playing = true;
                        }

                        rodio_tx.send(RodioCmd::SetVolume(vol)).ok();

                        let tx = fetch_tx.clone();
                        let video_id = next.video_id.clone();
                        tokio::spawn(async move {
                            let result = fetch_audio_bytes(
                                &video_id,
                                AudioQuality::Medium,
                                cookies,
                            )
                            .await
                            .map_err(|e| e.to_string());
                            tx.send(result).await.ok();
                        });
                    } else {
                        // End of queue — mark as stopped.
                        info!(idx, "queue exhausted, stopping playback");
                        radio.write_channel(DataChannel::Player).player.is_playing = false;
                    }
                }
            }
        }
    }
}

async fn handle_command(
    cmd: AudioCommand,
    rodio_tx: &std::sync::mpsc::Sender<RodioCmd>,
    radio: &mut RadioStation<Data, DataChannel>,
    fetch_tx: &mpsc::Sender<Result<Vec<u8>, String>>,
    song_done: &Arc<AtomicBool>,
) {
    match cmd {
        AudioCommand::Play {
            video_id,
            quality,
            title,
            artist,
            album,
            thumbnail_url,
        } => {
            let cookies = radio.read().cookie_header.clone();
            song_done.store(false, Ordering::Relaxed);

            {
                let mut state = radio.write_channel(DataChannel::Player);
                let p = &mut state.player;
                p.title = title;
                p.artist = artist;
                p.album = album;
                p.thumbnail_url = thumbnail_url;
                p.current_secs = 0.;
                p.is_playing = true;
                // Single-song play clears the queue so auto-advance is a no-op.
                p.queue.clear();
                p.queue_index = 0;
            }

            let tx = fetch_tx.clone();
            tokio::spawn(async move {
                let result = fetch_audio_bytes(&video_id, quality, cookies)
                    .await
                    .map_err(|e| e.to_string());
                tx.send(result).await.ok();
            });
        }

        AudioCommand::PlayFromQueue { songs, index, quality } => {
            if songs.is_empty() || index >= songs.len() {
                return;
            }

            let song = songs[index].clone();
            let cookies = radio.read().cookie_header.clone();
            song_done.store(false, Ordering::Relaxed);

            {
                let mut state = radio.write_channel(DataChannel::Player);
                let p = &mut state.player;
                p.queue = songs;
                p.queue_index = index;
                p.title = song.title.clone();
                p.artist = song.artist.clone();
                p.album = song.album.clone();
                p.thumbnail_url = song.thumbnail_url.clone();
                p.current_secs = 0.;
                p.is_playing = true;
            }

            let tx = fetch_tx.clone();
            let video_id = song.video_id.clone();
            tokio::spawn(async move {
                let result = fetch_audio_bytes(&video_id, quality, cookies)
                    .await
                    .map_err(|e| e.to_string());
                tx.send(result).await.ok();
            });
        }

        AudioCommand::Next => {
            let (queue, idx) = {
                let p = &radio.read().player;
                (p.queue.clone(), p.queue_index)
            };
            let next_idx = idx + 1;
            if next_idx < queue.len() {
                let song = queue[next_idx].clone();
                let cookies = radio.read().cookie_header.clone();
                song_done.store(false, Ordering::Relaxed);

                {
                    let mut state = radio.write_channel(DataChannel::Player);
                    let p = &mut state.player;
                    p.queue_index = next_idx;
                    p.title = song.title.clone();
                    p.artist = song.artist.clone();
                    p.album = song.album.clone();
                    p.thumbnail_url = song.thumbnail_url.clone();
                    p.current_secs = 0.;
                    p.is_playing = true;
                }

                let tx = fetch_tx.clone();
                let video_id = song.video_id.clone();
                tokio::spawn(async move {
                    let result =
                        fetch_audio_bytes(&video_id, AudioQuality::Medium, cookies)
                            .await
                            .map_err(|e| e.to_string());
                    tx.send(result).await.ok();
                });
            }
        }

        AudioCommand::Previous => {
            let (queue, idx) = {
                let p = &radio.read().player;
                (p.queue.clone(), p.queue_index)
            };
            if idx > 0 {
                let prev_idx = idx - 1;
                let song = queue[prev_idx].clone();
                let cookies = radio.read().cookie_header.clone();
                song_done.store(false, Ordering::Relaxed);

                {
                    let mut state = radio.write_channel(DataChannel::Player);
                    let p = &mut state.player;
                    p.queue_index = prev_idx;
                    p.title = song.title.clone();
                    p.artist = song.artist.clone();
                    p.album = song.album.clone();
                    p.thumbnail_url = song.thumbnail_url.clone();
                    p.current_secs = 0.;
                    p.is_playing = true;
                }

                let tx = fetch_tx.clone();
                let video_id = song.video_id.clone();
                tokio::spawn(async move {
                    let result =
                        fetch_audio_bytes(&video_id, AudioQuality::Medium, cookies)
                            .await
                            .map_err(|e| e.to_string());
                    tx.send(result).await.ok();
                });
            }
        }

        AudioCommand::Pause => {
            rodio_tx.send(RodioCmd::Pause).ok();
            radio.write_channel(DataChannel::Player).player.is_playing = false;
        }
        AudioCommand::Resume => {
            rodio_tx.send(RodioCmd::Resume).ok();
            radio.write_channel(DataChannel::Player).player.is_playing = true;
        }
        AudioCommand::Stop => {
            rodio_tx.send(RodioCmd::Stop).ok();
            let mut state = radio.write_channel(DataChannel::Player);
            state.player.is_playing = false;
            state.player.current_secs = 0.;
        }
        AudioCommand::Seek(secs) => {
            rodio_tx
                .send(RodioCmd::Seek(Duration::from_secs_f32(secs)))
                .ok();
        }
        AudioCommand::SetVolume(vol) => {
            rodio_tx.send(RodioCmd::SetVolume(vol)).ok();
            if vol > 0.0 {
                radio.write_channel(DataChannel::Player).player.volume = vol;
                tokio::task::spawn_blocking(move || crate::prefs::save_volume(vol));
            }
        }
    }
}

fn rodio_thread(rx: std::sync::mpsc::Receiver<RodioCmd>, progress: Arc<Mutex<ProgressState>>) {
    let sink_handle: rodio::MixerDeviceSink = match rodio::DeviceSinkBuilder::open_default_sink() {
        Ok(mut h) => {
            h.log_on_drop(false);
            h
        }
        Err(e) => {
            error!(error = %e, "failed to open audio output stream");
            return;
        }
    };
    let player = rodio::Player::connect_new(sink_handle.mixer());

    while let Ok(cmd) = rx.recv() {
        match cmd {
            RodioCmd::Play(bytes, done_flag) => {
                player.clear();
                let cursor = Cursor::new(bytes);
                match Decoder::new(cursor) {
                    Ok(source) => {
                        let maybe_total = source.total_duration();
                        let total = maybe_total.map(|d| d.as_secs_f32()).unwrap_or(0.);
                        if let Ok(mut p) = progress.lock() {
                            p.total_secs = total;
                            p.current_secs = 0.;
                            p.total_known = maybe_total.is_some();
                        }
                        let progress_clone = Arc::clone(&progress);
                        let source = CompletionNotifier {
                            inner: source,
                            flag: done_flag,
                            notified: false,
                        }
                        .track_position()
                        .periodic_access(PROGRESS_INTERVAL, move |s| {
                            let pos = s.get_pos().as_secs_f32();
                            if let Ok(mut p) = progress_clone.lock() {
                                p.current_secs = pos;
                                // When total duration is unknown, maintain a rolling estimate
                                // so the seek bar stays functional (always shows ~30s ahead).
                                if !p.total_known && pos + 30.0 > p.total_secs {
                                    p.total_secs = pos + 30.0;
                                }
                            }
                        });
                        player.append(source);
                        player.play();
                        info!("playback started, total={total:.1}s, known={}", maybe_total.is_some());
                    }
                    Err(e) => error!(error = %e, "audio decode failed"),
                }
            }
            RodioCmd::Pause => player.pause(),
            RodioCmd::Resume => player.play(),
            RodioCmd::Stop => player.clear(),
            RodioCmd::Seek(dur) => {
                if let Err(e) = player.try_seek(dur) {
                    error!(error = %e, "seek failed");
                }
            }
            RodioCmd::SetVolume(vol) => player.set_volume(vol.powi(2).clamp(0., 1.)),
        }
    }
}
