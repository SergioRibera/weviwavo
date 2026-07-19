use cookie_scrapy::{GetCookiesOptions, get_cookies, to_cookie_header};
use freya::radio::RadioStation;
use ytdroid::client::Locale;
use ytdroid::YouTube;

use crate::app::{Data, DataChannel};
use crate::audio::{AudioEngine, run_audio_engine};
use crate::auth::COOKIE_NAMES;

pub async fn run_startup(mut radio: RadioStation<Data, DataChannel>) {
    let (engine, audio_rx) = AudioEngine::new();
    radio
        .write_channel(DataChannel::Player)
        .audio_cmd = Some(engine.sender());

    tokio::join!(
        startup_inner(radio),
        run_audio_engine(audio_rx, radio),
    );
}

async fn startup_inner(mut radio: RadioStation<Data, DataChannel>) {
    // Use pre-populated cookie header (from login window or cookie-scrapy in main),
    // falling back to a fresh cookie-scrapy scrape if absent.
    let cookie_header = match radio.read().cookie_header.clone() {
        Some(h) => h,
        None => {
            let result = get_cookies(GetCookiesOptions::new("https://youtube.com")).await;
            let cookies = result
                .cookies
                .into_iter()
                .filter(|c| COOKIE_NAMES.iter().any(|n| c.name.starts_with(n)))
                .collect::<Vec<_>>();
            to_cookie_header(&cookies)
        }
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
