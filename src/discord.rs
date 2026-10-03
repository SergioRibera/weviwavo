use std::sync::mpsc::{self, SyncSender};
use std::thread;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use discord_rich_presence::{DiscordIpc, DiscordIpcClient, activity};
use tracing::{info, warn};

/// Discord Developer Portal → New Application → copy the Application ID here.
/// https://discord.com/developers/applications
const APP_ID: &str = "1539009832512921690";

const GITHUB_URL: &str = "https://github.com/SergioRibera/weviwavo";
const YT_MUSIC_BASE: &str = "https://music.youtube.com/watch?v=";

/// Reconnect interval when Discord is not running or the connection drops.
const RECONNECT_DELAY: Duration = Duration::from_secs(15);

// ── internal event ────────────────────────────────────────────────────────────

enum Event {
    Update(Presence),
    Shutdown,
}

struct Presence {
    title: String,
    artist: String,
    album: String,
    cover_url: String,
    video_id: String,
    state: State,
}

enum State {
    /// Unix timestamp of when the song started (i.e. `now - elapsed`).
    Playing { started_at: i64 },
    Paused,
    Stopped,
}

// ── public handle ─────────────────────────────────────────────────────────────

/// Non-blocking handle to the Discord Rich Presence background thread.
///
/// All methods are fire-and-forget: they send an event and return immediately.
/// If Discord is not running, events are silently dropped.
pub(crate) struct DiscordPresence {
    tx: SyncSender<Event>,
}

impl DiscordPresence {
    pub(crate) fn new() -> Self {
        let (tx, rx) = mpsc::sync_channel::<Event>(8);
        thread::Builder::new()
            .name("discord-rpc".into())
            .spawn(move || presence_thread(rx))
            .expect("failed to spawn discord-rpc thread");
        Self { tx }
    }

    /// Song is now playing from `current_secs` into the track.
    pub(crate) fn playing(
        &self,
        title: &str,
        artist: &str,
        album: &str,
        cover_url: &str,
        video_id: &str,
        current_secs: f32,
    ) {
        let started_at = unix_now() - current_secs.max(0.) as i64;
        self.send(Presence {
            title: title.to_owned(),
            artist: artist.to_owned(),
            album: album.to_owned(),
            cover_url: cover_url.to_owned(),
            video_id: video_id.to_owned(),
            state: State::Playing { started_at },
        });
    }

    /// Song is paused.
    pub(crate) fn paused(
        &self,
        title: &str,
        artist: &str,
        album: &str,
        cover_url: &str,
        video_id: &str,
    ) {
        self.send(Presence {
            title: title.to_owned(),
            artist: artist.to_owned(),
            album: album.to_owned(),
            cover_url: cover_url.to_owned(),
            video_id: video_id.to_owned(),
            state: State::Paused,
        });
    }

    /// Playback stopped / no song loaded.
    pub(crate) fn stopped(&self) {
        // video_id unused when stopped, but Presence requires one field.
        self.send(Presence {
            title: String::new(),
            artist: String::new(),
            album: String::new(),
            cover_url: String::new(),
            video_id: String::new(),
            state: State::Stopped,
        });
    }

    fn send(&self, p: Presence) {
        // `try_send` on a full channel drops the oldest slot; that's fine
        // since only the latest state matters.
        self.tx.try_send(Event::Update(p)).ok();
    }
}

impl Drop for DiscordPresence {
    fn drop(&mut self) {
        self.tx.try_send(Event::Shutdown).ok();
    }
}

// ── background thread ─────────────────────────────────────────────────────────

fn presence_thread(rx: mpsc::Receiver<Event>) {
    // Skip everything if the app id is a placeholder.
    if APP_ID == "YOUR_DISCORD_APP_ID" {
        warn!("discord: APP_ID not set — rich presence disabled. \
               Create an app at https://discord.com/developers/applications \
               and set the ID in src/discord.rs");
        return;
    }

    'outer: loop {
        let mut client = match DiscordIpcClient::new(APP_ID) {
            Ok(c) => c,
            Err(e) => {
                warn!("discord: client create failed: {e}");
                if sleep_or_shutdown(&rx) { return; }
                continue;
            }
        };

        if let Err(e) = client.connect() {
            warn!("discord: connect failed: {e}");
            if sleep_or_shutdown(&rx) { return; }
            continue;
        }

        info!("discord: connected");

        loop {
            match rx.recv() {
                Err(_) | Ok(Event::Shutdown) => {
                    let _ = client.close();
                    return;
                }
                Ok(Event::Update(p)) => {
                    if let Err(e) = apply(&mut client, &p) {
                        warn!("discord: activity update failed ({e}), reconnecting");
                        let _ = client.close();
                        // Drain stale events, then reconnect.
                        drain_until_latest(&rx);
                        thread::sleep(RECONNECT_DELAY);
                        continue 'outer;
                    }
                }
            }
        }
    }
}

fn apply(client: &mut DiscordIpcClient, p: &Presence) -> Result<(), Box<dyn std::error::Error>> {
    if matches!(p.state, State::Stopped) {
        client.clear_activity()?;
        return Ok(());
    }

    let details = match p.state {
        State::Playing { .. } => p.title.clone(),
        State::Paused => format!("⏸ {}", p.title),
        State::Stopped => unreachable!(),
    };

    let state_text = if p.album.is_empty() {
        format!("by {}", p.artist)
    } else {
        format!("by {} · {}", p.artist, p.album)
    };

    let yt_url = format!("{YT_MUSIC_BASE}{}", p.video_id);

    let assets = activity::Assets::new()
        .large_image(p.cover_url.as_str())
        .large_text(p.album.as_str());

    let buttons = vec![
        activity::Button::new("Listen on YouTube Music", yt_url.as_str()),
        activity::Button::new("View on GitHub", GITHUB_URL),
    ];

    let mut act = activity::Activity::new()
        .details(details.as_str())
        .state(state_text.as_str())
        .assets(assets)
        .buttons(buttons);

    if let State::Playing { started_at } = p.state {
        act = act.timestamps(activity::Timestamps::new().start(started_at));
    }

    client.set_activity(act)?;
    tracing::info!(title = p.title.as_str(), "discord: activity set");
    Ok(())
}

/// Drain pending events, returning the last one (or `None` if shutdown).
fn drain_until_latest(rx: &mpsc::Receiver<Event>) {
    loop {
        match rx.try_recv() {
            Ok(Event::Update(_)) => continue,
            _ => return,
        }
    }
}

/// Sleep for `RECONNECT_DELAY`, but return early (true) if a Shutdown arrives.
fn sleep_or_shutdown(rx: &mpsc::Receiver<Event>) -> bool {
    let deadline = std::time::Instant::now() + RECONNECT_DELAY;
    loop {
        let remaining = deadline.saturating_duration_since(std::time::Instant::now());
        if remaining.is_zero() {
            return false;
        }
        match rx.recv_timeout(remaining.min(Duration::from_millis(200))) {
            Ok(Event::Shutdown) | Err(mpsc::RecvTimeoutError::Disconnected) => return true,
            _ => {}
        }
    }
}

fn unix_now() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}
