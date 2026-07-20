#![cfg_attr(
    all(not(debug_assertions), target_os = "windows"),
    windows_subsystem = "windows"
)]
use freya::prelude::*;

mod app;
mod audio;
mod auth;
mod components;
mod cookies;
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

    // Subprocess mode: run only the Servo login window, save cookies, exit.
    // The parent process spawns us with this flag to avoid the winit EventLoop
    // recreation error (only one EventLoop allowed per process lifetime).
    if std::env::args().any(|a| a == "--login") {
        match servo_webview::run_login() {
            Ok(h) => {
                cookies::save_cookies(&h);
                std::process::exit(0);
            }
            Err(servo_webview::Error::Cancelled) => std::process::exit(1),
            Err(_) => std::process::exit(2),
        }
    }

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

    // Load persisted cookies; if absent spawn self as a login subprocess so
    // that Servo and Freya each get their own winit EventLoop (one per process).
    let cookie_header: Option<String> = match cookies::load_cookies() {
        Some(h) => {
            tracing::info!("loaded cookies from disk");
            Some(h)
        }
        None => {
            tracing::info!("no saved cookies — spawning login subprocess");
            let exe = std::env::current_exe().expect("cannot resolve own executable path");
            match std::process::Command::new(&exe).arg("--login").status() {
                Ok(s) if s.success() => {
                    tracing::info!("login subprocess completed — loading cookies");
                    cookies::load_cookies()
                }
                Ok(s) if s.code() == Some(1) => {
                    tracing::warn!("login cancelled by user");
                    None
                }
                Ok(_) => {
                    tracing::error!("login subprocess failed");
                    None
                }
                Err(e) => {
                    tracing::error!(error = %e, "failed to spawn login subprocess");
                    None
                }
            }
        }
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
