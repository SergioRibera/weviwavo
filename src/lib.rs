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

#[allow(dead_code)]
#[cfg(not(target_os = "android"))]
fn main() {
    use freya::radio::*;
    use tracing::level_filters::LevelFilter;
    use tracing_subscriber::EnvFilter;

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
                .with_default_directive(LevelFilter::TRACE.into())
                .try_from_env()
                .unwrap_or_default(),
        )
        .with_writer(non_blocking)
        .init();

    let rt = tokio::runtime::Builder::new_multi_thread().enable_all().build().unwrap();
    let _rt = rt.enter();

    let mut radio = RadioStation::create_global(app::Data::default());

    launch(
        LaunchConfig::new()
            .with_window(WindowConfig::new(MainApp { radio }).with_size(600., 450.))
            .with_future(move |_proxy| async move {
                let yt = ytmapi_rs::YtMusic::from_cookie(env!("TEST_COOKIE"))
                    .await
                    .inspect_err(|e| println!("Fail to get no auth client: {e}"));
                if let Ok(yt) = yt {
                    radio.write_channel(app::DataChannel::YtApi).yt_session = Some(yt.clone());
                    if let Ok(feed) = yt
                        .get_home(Some(4))
                        .await
                        .inspect_err(|e| println!("Cannot get home feed: {e}"))
                    {
                        radio.write_channel(app::DataChannel::Feed).feed = feed;
                    }
                }
            }),
    )
}
