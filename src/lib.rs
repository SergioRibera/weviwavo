#![cfg_attr(
    all(not(debug_assertions), target_os = "windows"),
    windows_subsystem = "windows"
)]
use freya::prelude::*;

mod api;
mod app;
mod components;
mod dialog;
mod utils;

pub const APP_NAME: &str = env!("CARGO_CRATE_NAME");

#[cfg(target_os = "android")]
use winit::platform::android::activity::AndroidApp;

#[cfg(target_os = "android")]
#[no_mangle]
fn android_main(droid_app: AndroidApp) {
    use winit::platform::android::EventLoopBuilderExtAndroid;

    launch(
        LaunchConfig::new().with_window(
            WindowConfig::new(init)
                .with_size(500., 450.)
                .with_window_attributes(|_attr, event_loop_builder| {
                    event_loop_builder.with_android_app(droid_app)
                }),
        ),
    )
}

#[rustfmt::skip]
const COOKIE_NAMES: &[&str] = &[
    "VISITOR_INFO1_LIVE", "VISITOR_PRIVACY_METADATA", "_gcl_au", "PREF", "__Secure-BUCKET", "YSC",
    "__Secure-ROLLOUT_TOKEN", "__Secure-1PSIDTS", "__Secure-3PSIDTS", "HSID", "SSID", "APISID",
    "SAPISID", "__Secure-1PAPISID", "__Secure-3PAPISID", "SID", "__Secure-1PSID", "__Secure-3PSID",
    "LOGIN_INFO", "SIDCC", "__Secure-1PSIDCC", "__Secure-3PSIDCC",
];

#[allow(dead_code)]
#[cfg(not(target_os = "android"))]
fn main() {
    use freya::radio::*;
    use tracing::level_filters::LevelFilter;
    use tracing_subscriber::EnvFilter;
    use tracing_subscriber::fmt::writer::MakeWriterExt;
    use ytmapi_rs::{YtMusic, auth::BrowserToken};

    use self::app::MainApp;
    use self::utils::data_dir;

    let builder = tracing_appender::rolling::Builder::new()
        .rotation(tracing_appender::rolling::Rotation::DAILY)
        .filename_suffix("log")
        .build(data_dir(&["logs"]))
        .unwrap();
    let (non_blocking, _guard) = tracing_appender::non_blocking(builder);
    tracing_subscriber::fmt()
        .with_ansi(false)
        .with_env_filter(
            EnvFilter::builder()
                .with_default_directive(LevelFilter::OFF.into())
                .try_from_env()
                .unwrap_or_default(),
        )
        .with_writer(non_blocking.and(std::io::stdout))
        .init();

    tracing::debug!("weviwavo starting");

    let rt = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .unwrap();
    let _rt = rt.enter();

    let mut radio = RadioStation::create_global(app::Data::default());

    launch(
        LaunchConfig::new()
            .with_window(WindowConfig::new_app(MainApp { radio }).with_size(600., 450.))
            .with_future(move |_proxy| async move {
                use cookie_scrapy::*;
                let result = get_cookies(GetCookiesOptions::new("https://youtube.com")).await;

                let cookies = result
                    .cookies
                    .into_iter()
                    .filter(|c| COOKIE_NAMES.iter().any(|n| c.name.starts_with(n)))
                    .collect::<Vec<_>>();
                let cookies = cookie_scrapy::to_cookie_header(&cookies);

                let yt: Result<YtMusic<BrowserToken>, _> = YtMusic::from_cookie(cookies)
                    .await
                    .inspect_err(|e| tracing::error!(error = %e, "failed to create YT client"));
                tracing::debug!(success = yt.is_ok(), "YT client creation finished");
                if let Ok(yt) = yt {
                    radio.write_channel(app::DataChannel::YtApi).yt_session = Some(yt.clone());
                    if let Ok(feed) = yt
                        .get_home(Some(4))
                        .await
                        .inspect_err(|e| tracing::error!(error = %e, "failed to fetch home feed"))
                    {
                        tracing::debug!(sections = feed.len(), "home feed loaded");
                        radio.write_channel(app::DataChannel::Feed).feed = feed;
                    }
                }
            }),
    )
}
