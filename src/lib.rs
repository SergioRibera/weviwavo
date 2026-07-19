#![cfg_attr(
    all(not(debug_assertions), target_os = "windows"),
    windows_subsystem = "windows"
)]
use freya::prelude::*;

mod app;
mod audio;
mod auth;
mod components;
mod dialog;
mod startup;
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

#[allow(dead_code)]
#[cfg(not(target_os = "android"))]
fn main() {
    use freya::radio::*;
    use tracing::level_filters::LevelFilter;
    use tracing_subscriber::EnvFilter;
    use tracing_subscriber::fmt::writer::MakeWriterExt;

    use self::app::MainApp;
    use self::startup::run_startup;
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

    // Try to get cookies from installed browsers first.
    let scraped = rt.block_on(async {
        use cookie_scrapy::{GetCookiesOptions, get_cookies, to_cookie_header};
        let result = get_cookies(GetCookiesOptions::new("https://youtube.com")).await;
        let cookies = result
            .cookies
            .into_iter()
            .filter(|c| crate::auth::COOKIE_NAMES.iter().any(|n| c.name.starts_with(n)))
            .collect::<Vec<_>>();
        to_cookie_header(&cookies)
    });

    // If no browser cookies found, open the Servo login window (blocks main thread).
    let cookie_header: Option<String> = if scraped.is_empty() {
        tracing::info!("no browser cookies found — opening login window");
        match servo_webview::run_login() {
            Ok(h) => {
                tracing::info!("login complete");
                Some(h)
            }
            Err(servo_webview::Error::Cancelled) => {
                tracing::warn!("login cancelled by user");
                None
            }
            Err(e) => {
                tracing::error!(error = %e, "servo login window failed");
                None
            }
        }
    } else {
        Some(scraped)
    };

    let mut initial_data = app::Data::default();
    initial_data.cookie_header = cookie_header;

    let radio = RadioStation::create_global(initial_data);

    launch(
        LaunchConfig::new()
            .with_window(WindowConfig::new_app(MainApp { radio }).with_size(600., 450.))
            .with_future(move |_proxy| run_startup(radio)),
    )
}
