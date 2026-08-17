use std::time::Duration;

use souvlaki::{MediaControlEvent, MediaControls, MediaMetadata, MediaPlayback, MediaPosition, PlatformConfig, SeekDirection};
use tokio::sync::mpsc;
use tracing::warn;

use crate::audio::AudioCommand;

pub(crate) struct MediaBridge {
    controls: Option<MediaControls>,
}

impl MediaBridge {
    pub(crate) fn new(tx: mpsc::Sender<AudioCommand>) -> Self {
        let config = PlatformConfig {
            dbus_name: "rs.weviwavo",
            display_name: "WeviWavo",
            // HWND required on Windows to associate with the app window;
            // None disables thumbnail toolbar but media keys still work.
            hwnd: None,
        };

        let mut controls = match MediaControls::new(config) {
            Ok(c) => c,
            Err(e) => {
                warn!("media controls init failed: {e:?}");
                return Self { controls: None };
            }
        };

        if let Err(e) = controls.attach(move |evt| forward_event(evt, &tx)) {
            warn!("media controls attach failed: {e:?}");
            return Self { controls: None };
        }

        Self { controls: Some(controls) }
    }

    /// Called when a new song starts. Sets metadata and marks playback as Playing at position 0.
    pub(crate) fn song_started(&mut self, title: &str, artist: &str, album: &str, cover_url: &str) {
        self.update_metadata(title, artist, album, cover_url, None);
        self.set_playback(MediaPlayback::Playing {
            progress: Some(MediaPosition(Duration::ZERO)),
        });
    }

    /// Update duration once the decoder has determined total length for the current song.
    pub(crate) fn update_duration(
        &mut self,
        title: &str,
        artist: &str,
        album: &str,
        cover_url: &str,
        duration: Duration,
    ) {
        self.update_metadata(title, artist, album, cover_url, Some(duration));
    }

    pub(crate) fn set_paused(&mut self, pos: f32) {
        self.set_playback(MediaPlayback::Paused {
            progress: Some(MediaPosition(Duration::from_secs_f32(pos.max(0.)))),
        });
    }

    pub(crate) fn set_playing(&mut self, pos: f32) {
        self.set_playback(MediaPlayback::Playing {
            progress: Some(MediaPosition(Duration::from_secs_f32(pos.max(0.)))),
        });
    }

    pub(crate) fn set_stopped(&mut self) {
        self.set_playback(MediaPlayback::Stopped);
    }

    fn update_metadata(
        &mut self,
        title: &str,
        artist: &str,
        album: &str,
        cover_url: &str,
        duration: Option<Duration>,
    ) {
        let Some(c) = self.controls.as_mut() else { return };
        if let Err(e) = c.set_metadata(MediaMetadata {
            title: (!title.is_empty()).then_some(title),
            artist: (!artist.is_empty()).then_some(artist),
            album: (!album.is_empty()).then_some(album),
            cover_url: (!cover_url.is_empty()).then_some(cover_url),
            duration,
        }) {
            warn!("media controls set_metadata failed: {e:?}");
        }
    }

    fn set_playback(&mut self, playback: MediaPlayback) {
        let Some(c) = self.controls.as_mut() else { return };
        if let Err(e) = c.set_playback(playback) {
            warn!("media controls set_playback failed: {e:?}");
        }
    }
}

fn forward_event(evt: MediaControlEvent, tx: &mpsc::Sender<AudioCommand>) {
    let cmd = match evt {
        MediaControlEvent::Play => AudioCommand::Resume,
        MediaControlEvent::Pause => AudioCommand::Pause,
        MediaControlEvent::Toggle => AudioCommand::Toggle,
        MediaControlEvent::Next => AudioCommand::Next,
        MediaControlEvent::Previous => AudioCommand::Previous,
        MediaControlEvent::Stop => AudioCommand::Stop,
        MediaControlEvent::Seek(SeekDirection::Forward) => AudioCommand::SeekRelative(10.),
        MediaControlEvent::Seek(SeekDirection::Backward) => AudioCommand::SeekRelative(-10.),
        MediaControlEvent::SeekBy(SeekDirection::Forward, dur) => {
            AudioCommand::SeekRelative(dur.as_secs_f32())
        }
        MediaControlEvent::SeekBy(SeekDirection::Backward, dur) => {
            AudioCommand::SeekRelative(-dur.as_secs_f32())
        }
        MediaControlEvent::SetPosition(pos) => AudioCommand::Seek(pos.0.as_secs_f32()),
        MediaControlEvent::OpenUri(_)
        | MediaControlEvent::Raise
        | MediaControlEvent::Quit
        | MediaControlEvent::SetVolume(_) => {
            return;
        }
    };
    tx.try_send(cmd).ok();
}
