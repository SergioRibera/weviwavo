use cookie_scrapy::{GetCookiesOptions, get_cookies, to_cookie_header};
use freya::radio::RadioStation;
use ytmapi_rs::{YtMusic, auth::BrowserToken};

use crate::app::{Data, DataChannel};
use crate::auth::COOKIE_NAMES;

pub async fn run_startup(mut radio: RadioStation<Data, DataChannel>) {
    let result = get_cookies(GetCookiesOptions::new("https://youtube.com")).await;
    let cookies = result
        .cookies
        .into_iter()
        .filter(|c| COOKIE_NAMES.iter().any(|n| c.name.starts_with(n)))
        .collect::<Vec<_>>();
    let cookies = to_cookie_header(&cookies);

    let yt: Result<YtMusic<BrowserToken>, _> = YtMusic::from_cookie(cookies)
        .await
        .inspect_err(|e| tracing::error!(error = %e, "failed to create YT client"));

    tracing::debug!(success = yt.is_ok(), "YT client creation finished");

    if let Ok(yt) = yt {
        radio.write_channel(DataChannel::YtApi).yt_session = Some(yt.clone());
        if let Ok(feed) = yt
            .get_home(Some(4))
            .await
            .inspect_err(|e| tracing::error!(error = %e, "failed to fetch home feed"))
        {
            tracing::debug!(sections = feed.len(), "home feed loaded");
            radio.write_channel(DataChannel::Feed).feed = feed;
        }
    }
}
