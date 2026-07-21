use std::io::Cursor;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use freya::radio::RadioStation;
use rodio::Decoder;
use rodio::source::Source;
use tokio::sync::mpsc;
use tracing::{error, info};

use crate::app::{Data, DataChannel};
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
    Pause,
    Resume,
    Stop,
    Seek(f32),
    SetVolume(f32),
}

enum RodioCmd {
    Play(Vec<u8>),
    Pause,
    Resume,
    Stop,
    Seek(Duration),
    SetVolume(f32),
}

/// Shared progress state written by the rodio thread, read by the engine task.
#[derive(Default)]
struct ProgressState {
    current_secs: f32,
    total_secs: f32,
}

/// Handle for sending commands to the audio engine.
pub struct AudioEngine {
    tx: mpsc::Sender<AudioCommand>,
}

impl AudioEngine {
    /// Creates a new `AudioEngine` and returns it together with a future that
    /// must be driven to completion to run the engine.  Because
    /// [`RadioStation`] is `!Send`, the engine loop cannot be spawned with
    /// `tokio::spawn`; instead, await `run()` from a task that already holds
    /// the station (e.g. the Freya `with_future` startup task).
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
                handle_command(cmd, &rodio_tx, &mut radio, &fetch_tx).await;
            }
            Some(result) = fetch_rx.recv() => {
                match result {
                    Ok(bytes) => {
                        rodio_tx.send(RodioCmd::Play(bytes)).ok();
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
                let mut state = radio.write_channel(DataChannel::Player);
                state.player.current_secs = current;
                state.player.total_secs = total;
            }
        }
    }
}

async fn handle_command(
    cmd: AudioCommand,
    rodio_tx: &std::sync::mpsc::Sender<RodioCmd>,
    radio: &mut RadioStation<Data, DataChannel>,
    fetch_tx: &mpsc::Sender<Result<Vec<u8>, String>>,
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

            {
                let mut state = radio.write_channel(DataChannel::Player);
                let p = &mut state.player;
                p.title = title;
                p.artist = artist;
                p.album = album;
                p.thumbnail_url = thumbnail_url;
                p.current_secs = 0.;
                p.is_playing = true;
            }

            let tx = fetch_tx.clone();
            tokio::spawn(async move {
                let result = fetch_audio_bytes(&video_id, quality, cookies)
                    .await
                    .map_err(|e| e.to_string());
                tx.send(result).await.ok();
            });
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
            RodioCmd::Play(bytes) => {
                player.clear();
                let _ = std::fs::write("/tmp/weviwavo-audio-debug.webm", &bytes);
                let cursor = Cursor::new(bytes);
                match Decoder::new(cursor) {
                    Ok(source) => {
                        let total = source
                            .total_duration()
                            .map(|d| d.as_secs_f32())
                            .unwrap_or(0.);
                        if let Ok(mut p) = progress.lock() {
                            p.total_secs = total;
                            p.current_secs = 0.;
                        }
                        let progress_clone = Arc::clone(&progress);
                        let source =
                            source
                                .track_position()
                                .periodic_access(PROGRESS_INTERVAL, move |s| {
                                    let pos = s.get_pos().as_secs_f32();
                                    if let Ok(mut p) = progress_clone.lock() {
                                        p.current_secs = pos;
                                    }
                                });
                        player.append(source);
                        player.play();
                        info!("playback started, total={total:.1}s");
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
            RodioCmd::SetVolume(vol) => player.set_volume(vol.clamp(0., 1.)),
        }
    }
}
