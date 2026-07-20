//! PoToken generation via Servo WebView.
//!
//! Mirrors Metrolist's `PoTokenWebView` pattern: all YouTube API calls are made
//! from Rust; the Servo browser only provides the JS runtime for the BotGuard VM.
//!
//! Call [`generate`] from a tokio async context.

use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use base64::{Engine, engine::general_purpose::STANDARD as B64};
use euclid::Scale;
use http::HeaderMap;
use serde_json::{Value, json};
use servo::{
    ConsoleLogLevel, DevicePoint, EventLoopWaker, InputEvent, JSValue, LoadStatus, MouseButton,
    MouseButtonAction, RenderingContext, Servo, ServoBuilder, WebResourceLoad, WebResourceResponse,
    WebView, WebViewBuilder, WebViewDelegate, WindowRenderingContext,
};
use winit::application::ApplicationHandler;
use winit::dpi::PhysicalPosition;
use winit::event::WindowEvent;
use winit::event_loop::{ActiveEventLoop, EventLoop, EventLoopProxy};
use winit::raw_window_handle::{HasDisplayHandle, HasWindowHandle};
use winit::window::{Window, WindowAttributes};

/// Token pair returned by [`generate`].
pub struct PoTokenPair {
    /// Token bound to the session/visitor ID — send in `/player` request body as
    /// `serviceIntegrityDimensions.poToken`.
    pub player: String,
    /// Token bound to the video ID — append to CDN URL as `pot=<value>`.
    pub streaming: String,
}

/// Error variants for PoToken generation.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("winit event loop: {0}")]
    EventLoop(#[from] winit::error::EventLoopError),
    #[error("HTTP: {0}")]
    Http(#[from] reqwest::Error),
    #[error("JSON: {0}")]
    Json(#[from] serde_json::Error),
    #[error("PoToken timed out")]
    Timeout,
    #[error("PoToken cancelled")]
    Cancelled,
    #[error("BotGuard JS error: {0}")]
    JsError(String),
    #[error("parse: {0}")]
    Parse(String),
}

const BOTGUARD_CREATE_URL: &str = "https://www.youtube.com/api/jnn/v1/Create";
const BOTGUARD_GENERATE_IT_URL: &str = "https://www.youtube.com/api/jnn/v1/GenerateIT";
const REQUEST_KEY: &str = "O43z0dpjhgX20SCx4KAo";
const GOOGLE_API_KEY: &str = "AIzaSyDyT5W0Jh49F30Pqqtyfdf7pDLFKLJoAnw";
const BG_USER_AGENT: &str =
    "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/131.0.0.0 Safari/537.3";

const PO_TOKEN_HTML: &str = include_str!("po_token.html");

// ── Public entry point ────────────────────────────────────────────────────────

/// Generate a PoToken pair for the given session and video IDs.
///
/// Starts an invisible Servo browser. The browser runs the BotGuard JS; all
/// YouTube API round-trips are handled by Rust via `load_web_resource`.
///
/// # Errors
///
/// Returns [`Error::Timeout`] after 90 s if the BotGuard flow does not complete.
pub async fn generate(session_id: &str, video_id: &str) -> Result<PoTokenPair, Error> {
    rustls::crypto::aws_lc_rs::default_provider()
        .install_default()
        .ok();

    let http = reqwest::Client::builder().user_agent(BG_USER_AGENT).build()?;

    // Step 1: make the BotGuard Create call before starting Servo.
    let create_body = serde_json::to_string(&json!([REQUEST_KEY]))?;
    let raw = call_botguard(&http, BOTGUARD_CREATE_URL, &create_body).await?;
    let challenge_json = parse_challenge(&raw)?.to_string();

    let (done_tx, done_rx) = tokio::sync::oneshot::channel::<Result<PoTokenPair, Error>>();
    let done_tx = Arc::new(Mutex::new(Some(done_tx)));
    let exit_flag = Arc::new(AtomicBool::new(false));

    let session_id = session_id.to_string();
    let video_id = video_id.to_string();
    let done_tx_clone = done_tx.clone();
    let exit_flag_clone = exit_flag.clone();
    let tokio_handle = tokio::runtime::Handle::current();

    tokio::task::spawn_blocking(move || {
        run_servo(
            challenge_json,
            session_id,
            video_id,
            http,
            exit_flag_clone,
            done_tx_clone,
            tokio_handle,
        );
    });

    tokio::time::timeout(std::time::Duration::from_secs(90), done_rx)
        .await
        .map_err(|_| Error::Timeout)?
        .map_err(|_| Error::Cancelled)?
}

// ── Servo event loop ──────────────────────────────────────────────────────────

#[derive(Debug)]
struct PoTokenWake;

#[derive(Clone)]
struct PoTokenWaker(EventLoopProxy<PoTokenWake>);

impl EventLoopWaker for PoTokenWaker {
    fn clone_box(&self) -> Box<dyn EventLoopWaker> {
        Box::new(Self(self.0.clone()))
    }

    fn wake(&self) {
        let _ = self.0.send_event(PoTokenWake);
    }
}

fn run_servo(
    challenge_json: String,
    session_id: String,
    video_id: String,
    http: reqwest::Client,
    exit_flag: Arc<AtomicBool>,
    done_tx: Arc<Mutex<Option<tokio::sync::oneshot::Sender<Result<PoTokenPair, Error>>>>>,
    tokio_handle: tokio::runtime::Handle,
) {
    let el = match EventLoop::<PoTokenWake>::with_user_event().build() {
        Ok(el) => el,
        Err(e) => {
            tracing::error!("PoToken: event loop failed: {e}");
            signal_done(Err(Error::EventLoop(e)), &done_tx, &exit_flag, None);
            return;
        }
    };

    let proxy = el.create_proxy();
    let waker = PoTokenWaker(proxy.clone());

    let mut app = PoTokenApp::Initial {
        waker,
        challenge_json,
        session_id,
        video_id,
        http,
        exit_flag,
        done_tx,
        proxy,
        tokio_handle,
    };

    if let Err(e) = el.run_app(&mut app) {
        tracing::error!("PoToken: servo event loop error: {e}");
    }
}

// ── Orchestration phase ───────────────────────────────────────────────────────

#[derive(Clone, Copy, PartialEq)]
enum Phase {
    WaitingForLoad,
    RunningBotGuard,
    FetchingGenerateIT,
    CreatingMinter,
    ObtainingTokens,
    Done,
}

// ── WebViewDelegate state ─────────────────────────────────────────────────────

struct PoTokenState {
    window: Window,
    servo: Servo,
    rendering_context: Rc<WindowRenderingContext>,
    webviews: RefCell<Vec<WebView>>,
    cursor_pos: Cell<PhysicalPosition<f64>>,
    exit_flag: Arc<AtomicBool>,
    should_exit: Cell<bool>,
    phase: RefCell<Phase>,
    challenge_json: String,
    session_id: String,
    video_id: String,
    http: reqwest::Client,
    pending_integrity: Arc<Mutex<Option<Result<Vec<u8>, Error>>>>,
    done_tx: Arc<Mutex<Option<tokio::sync::oneshot::Sender<Result<PoTokenPair, Error>>>>>,
    proxy: EventLoopProxy<PoTokenWake>,
    tokio_handle: tokio::runtime::Handle,
}

impl PoTokenState {
    fn active_webview(&self) -> Option<WebView> {
        self.webviews.borrow().last().cloned()
    }

    fn inject_script(&self, script: &str) {
        let Some(wv) = self.active_webview() else { return };
        wv.evaluate_javascript(script, |_| {});
    }

    fn signal_done_result(&self, result: Result<PoTokenPair, Error>) {
        *self.phase.borrow_mut() = Phase::Done;
        signal_done(result, &self.done_tx, &self.exit_flag, Some(&self.proxy));
        self.should_exit.set(true);
        self.window.request_redraw();
    }

    /// Called when /s/0 fires: read `window._bgR`, then spawn GenerateIT HTTP task.
    fn handle_signal_0(&self) {
        let Some(wv) = self.active_webview() else { return };
        let pending = self.pending_integrity.clone();
        let proxy = self.proxy.clone();
        let http = self.http.clone();
        // Capture the stored handle so the callback can spawn on the tokio runtime
        // from the Servo blocking thread (which has no ambient async context).
        let tokio_handle = self.tokio_handle.clone();

        wv.evaluate_javascript("window._bgR", move |result| {
            let bg_response = match result {
                Ok(JSValue::String(s)) => s,
                Ok(other) => {
                    tracing::error!("PoToken: window._bgR unexpected type: {other:?}");
                    *pending.lock().unwrap() =
                        Some(Err(Error::Parse("window._bgR not a string".into())));
                    let _ = proxy.send_event(PoTokenWake);
                    return;
                }
                Err(e) => {
                    tracing::error!("PoToken: evaluate_javascript(_bgR) error: {e:?}");
                    *pending.lock().unwrap() =
                        Some(Err(Error::Parse(format!("JS eval: {e:?}"))));
                    let _ = proxy.send_event(PoTokenWake);
                    return;
                }
            };

            let pending2 = pending.clone();
            let proxy2 = proxy.clone();
            tokio_handle.spawn(async move {
                let result = fetch_generate_it(&http, &bg_response).await;
                *pending2.lock().unwrap() = Some(result);
                let _ = proxy2.send_event(PoTokenWake);
            });
        });
    }

    /// Inject SCRIPT_2: createPoTokenMinter with integrity token bytes.
    fn inject_script_2(&self, integrity_bytes: &[u8]) {
        let bytes_js = bytes_to_js_array(integrity_bytes);
        let script = format!(
            "(function(){{\
              createPoTokenMinter(window._wps,new Uint8Array([{bytes_js}])).then(function(){{\
                fetch('http://potoken.internal/s/1').catch(function(){{}});\
              }}).catch(function(e){{\
                window._err=String(e);\
                fetch('http://potoken.internal/s/err').catch(function(){{}});\
              }});\
            }})();"
        );
        self.inject_script(&script);
    }

    /// Inject SCRIPT_3: obtainPoToken for both session and video IDs.
    fn inject_script_3(&self) {
        let session_bytes = bytes_to_js_array(self.session_id.as_bytes());
        let video_bytes = bytes_to_js_array(self.video_id.as_bytes());
        let script = format!(
            "(function(){{\
              Promise.all([\
                obtainPoToken(new Uint8Array([{session_bytes}])),\
                obtainPoToken(new Uint8Array([{video_bytes}]))\
              ]).then(function(rs){{\
                window._pR=Array.from(rs[0]).join(',');\
                window._sR=Array.from(rs[1]).join(',');\
                fetch('http://potoken.internal/s/2').catch(function(){{}});\
              }}).catch(function(e){{\
                window._err=String(e);\
                fetch('http://potoken.internal/s/err').catch(function(){{}});\
              }});\
            }})();"
        );
        self.inject_script(&script);
    }

    /// After /s/2: read both token globals and signal done.
    fn read_tokens_and_finish(&self) {
        let Some(wv) = self.active_webview() else { return };
        let wv2 = wv.clone();
        let done_tx = self.done_tx.clone();
        let exit_flag = self.exit_flag.clone();
        let proxy = self.proxy.clone();

        wv.evaluate_javascript("window._pR", move |r1| {
            let player_csv = match r1 {
                Ok(JSValue::String(s)) => s,
                Ok(other) => {
                    tracing::error!("PoToken: window._pR unexpected: {other:?}");
                    signal_done(
                        Err(Error::Parse("_pR not a string".into())),
                        &done_tx,
                        &exit_flag,
                        Some(&proxy),
                    );
                    return;
                }
                Err(e) => {
                    tracing::error!("PoToken: eval _pR: {e:?}");
                    signal_done(
                        Err(Error::Parse(format!("JS eval _pR: {e:?}"))),
                        &done_tx,
                        &exit_flag,
                        Some(&proxy),
                    );
                    return;
                }
            };

            wv2.evaluate_javascript("window._sR", move |r2| {
                let streaming_csv = match r2 {
                    Ok(JSValue::String(s)) => s,
                    Ok(other) => {
                        tracing::error!("PoToken: window._sR unexpected: {other:?}");
                        signal_done(
                            Err(Error::Parse("_sR not a string".into())),
                            &done_tx,
                            &exit_flag,
                            Some(&proxy),
                        );
                        return;
                    }
                    Err(e) => {
                        tracing::error!("PoToken: eval _sR: {e:?}");
                        signal_done(
                            Err(Error::Parse(format!("JS eval _sR: {e:?}"))),
                            &done_tx,
                            &exit_flag,
                            Some(&proxy),
                        );
                        return;
                    }
                };

                let player = u8_csv_to_base64url(&player_csv);
                let streaming = u8_csv_to_base64url(&streaming_csv);
                signal_done(
                    Ok(PoTokenPair { player, streaming }),
                    &done_tx,
                    &exit_flag,
                    Some(&proxy),
                );
            });
        });
    }

    /// After /s/err: read `window._err` and signal failure.
    fn read_error_and_finish(&self) {
        let Some(wv) = self.active_webview() else { return };
        let done_tx = self.done_tx.clone();
        let exit_flag = self.exit_flag.clone();
        let proxy = self.proxy.clone();

        wv.evaluate_javascript("window._err", move |result| {
            let msg = match result {
                Ok(JSValue::String(s)) => s,
                _ => "unknown JS error".into(),
            };
            tracing::error!("PoToken: JS error: {msg}");
            signal_done(
                Err(Error::JsError(msg)),
                &done_tx,
                &exit_flag,
                Some(&proxy),
            );
        });
    }
}

impl WebViewDelegate for PoTokenState {
    fn notify_new_frame_ready(&self, _: WebView) {
        self.window.request_redraw();
    }

    fn notify_load_status_changed(&self, _: WebView, status: LoadStatus) {
        if status != LoadStatus::Complete {
            return;
        }
        if *self.phase.borrow() != Phase::WaitingForLoad {
            return;
        }
        *self.phase.borrow_mut() = Phase::RunningBotGuard;

        // SCRIPT_1: run BotGuard VM, store results in JS globals, signal via fetch.
        let challenge = &self.challenge_json;
        let script = format!(
            "(function(){{\
              var c={challenge};\
              window._wps=[];\
              runBotGuard(c).then(function(r){{\
                window._wps=r.webPoSignalOutput;\
                window._bgR=r.botguardResponse;\
                fetch('http://potoken.internal/s/0').catch(function(){{}});\
              }}).catch(function(e){{\
                window._err=String(e);\
                fetch('http://potoken.internal/s/err').catch(function(){{}});\
              }});\
            }})();"
        );
        self.inject_script(&script);
    }

    fn load_web_resource(&self, _webview: WebView, load: WebResourceLoad) {
        let url = load.request.url.clone();
        let path = url.path().to_string();

        if url.host_str() != Some("potoken.internal") {
            // Let all other resources pass through unintercepted.
            return;
        }

        match path.as_str() {
            "/" => {
                // Serve the BotGuard helper HTML.
                let mut headers = HeaderMap::new();
                headers.insert(
                    http::header::CONTENT_TYPE,
                    "text/html; charset=utf-8".parse().unwrap(),
                );
                headers.insert(
                    http::header::ACCESS_CONTROL_ALLOW_ORIGIN,
                    "*".parse().unwrap(),
                );
                let response = WebResourceResponse::new(url).headers(headers);
                let mut intercepted = load.intercept(response);
                intercepted.send_body_data(PO_TOKEN_HTML.as_bytes().to_vec());
                intercepted.finish();
            }
            "/s/0" => {
                serve_ok(load, url);
                if *self.phase.borrow() == Phase::RunningBotGuard {
                    *self.phase.borrow_mut() = Phase::FetchingGenerateIT;
                    self.handle_signal_0();
                }
            }
            "/s/1" => {
                serve_ok(load, url);
                if *self.phase.borrow() == Phase::CreatingMinter {
                    *self.phase.borrow_mut() = Phase::ObtainingTokens;
                    self.inject_script_3();
                }
            }
            "/s/2" => {
                serve_ok(load, url);
                if *self.phase.borrow() == Phase::ObtainingTokens {
                    *self.phase.borrow_mut() = Phase::Done;
                    self.read_tokens_and_finish();
                }
            }
            "/s/err" => {
                serve_ok(load, url);
                self.read_error_and_finish();
            }
            _ => {
                serve_ok(load, url);
            }
        }
    }

    fn show_console_message(&self, _: WebView, level: ConsoleLogLevel, message: String) {
        match level {
            ConsoleLogLevel::Error => tracing::error!(target: "potoken_js", "{message}"),
            ConsoleLogLevel::Warn => tracing::warn!(target: "potoken_js", "{message}"),
            ConsoleLogLevel::Debug => tracing::debug!(target: "potoken_js", "{message}"),
            _ => tracing::info!(target: "potoken_js", "{message}"),
        }
    }
}

// ── ApplicationHandler ────────────────────────────────────────────────────────

enum PoTokenApp {
    Initial {
        waker: PoTokenWaker,
        challenge_json: String,
        session_id: String,
        video_id: String,
        http: reqwest::Client,
        exit_flag: Arc<AtomicBool>,
        done_tx: Arc<Mutex<Option<tokio::sync::oneshot::Sender<Result<PoTokenPair, Error>>>>>,
        proxy: EventLoopProxy<PoTokenWake>,
        tokio_handle: tokio::runtime::Handle,
    },
    Running(Rc<PoTokenState>),
}

impl ApplicationHandler<PoTokenWake> for PoTokenApp {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        let Self::Initial {
            waker,
            challenge_json,
            session_id,
            video_id,
            http,
            exit_flag,
            done_tx,
            proxy,
            tokio_handle,
        } = self
        else {
            return;
        };

        let attrs = WindowAttributes::default()
            .with_title("YouTube Security Check")
            .with_inner_size(winit::dpi::LogicalSize::new(300u32, 200u32));
        let window = match event_loop.create_window(attrs) {
            Ok(w) => w,
            Err(e) => {
                tracing::error!("PoToken: window creation failed: {e}");
                event_loop.exit();
                return;
            }
        };

        let display_handle = event_loop.display_handle().expect("no display handle");
        let window_handle = window.window_handle().expect("no window handle");
        let size = window.inner_size();

        let rendering_context = Rc::new(
            WindowRenderingContext::new(display_handle, window_handle, size)
                .expect("failed to create rendering context"),
        );
        rendering_context.make_current().expect("make_current failed");

        let servo = ServoBuilder::default()
            .event_loop_waker(Box::new(waker.clone()))
            .build();

        let state = Rc::new(PoTokenState {
            window,
            servo,
            rendering_context,
            webviews: RefCell::new(Vec::new()),
            cursor_pos: Cell::new(PhysicalPosition::new(0.0, 0.0)),
            exit_flag: exit_flag.clone(),
            should_exit: Cell::new(false),
            phase: RefCell::new(Phase::WaitingForLoad),
            challenge_json: challenge_json.clone(),
            session_id: session_id.clone(),
            video_id: video_id.clone(),
            http: http.clone(),
            pending_integrity: Arc::new(Mutex::new(None)),
            done_tx: done_tx.clone(),
            proxy: proxy.clone(),
            tokio_handle: tokio_handle.clone(),
        });

        let url = url::Url::parse("http://potoken.internal/").expect("hardcoded URL is valid");
        let scale = Scale::new(state.window.scale_factor() as f32);
        let webview = WebViewBuilder::new(&state.servo, state.rendering_context.clone())
            .url(url)
            .hidpi_scale_factor(scale)
            .delegate(state.clone())
            .build();

        state.webviews.borrow_mut().push(webview);
        *self = Self::Running(state);
    }

    fn user_event(&mut self, event_loop: &ActiveEventLoop, _: PoTokenWake) {
        let Self::Running(state) = self else { return };

        state.servo.spin_event_loop();

        if state.exit_flag.load(Ordering::Relaxed) {
            event_loop.exit();
            return;
        }

        // If a GenerateIT response has arrived, advance to the minter phase.
        if *state.phase.borrow() == Phase::FetchingGenerateIT {
            let result = state.pending_integrity.lock().unwrap().take();
            if let Some(outcome) = result {
                match outcome {
                    Ok(bytes) => {
                        *state.phase.borrow_mut() = Phase::CreatingMinter;
                        state.inject_script_2(&bytes);
                    }
                    Err(e) => {
                        state.signal_done_result(Err(e));
                        event_loop.exit();
                    }
                }
            }
        }
    }

    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        _: winit::window::WindowId,
        event: WindowEvent,
    ) {
        let Self::Running(state) = self else { return };
        state.servo.spin_event_loop();

        match event {
            WindowEvent::CloseRequested => {
                event_loop.exit();
            }
            WindowEvent::RedrawRequested => {
                if state.should_exit.get() || state.exit_flag.load(Ordering::Relaxed) {
                    event_loop.exit();
                    return;
                }
                let webviews = state.webviews.borrow();
                if let Some(wv) = webviews.last() {
                    wv.paint();
                    state.rendering_context.present();
                }
            }
            WindowEvent::Resized(size) => {
                let webviews = state.webviews.borrow();
                if let Some(wv) = webviews.last() {
                    wv.resize(size);
                }
            }
            WindowEvent::CursorMoved { position, .. } => {
                state.cursor_pos.set(position);
                let webviews = state.webviews.borrow();
                if let Some(wv) = webviews.last() {
                    wv.notify_input_event(InputEvent::MouseMove(
                        servo::MouseMoveEvent::new(
                            DevicePoint::new(position.x as f32, position.y as f32).into(),
                        ),
                    ));
                }
            }
            WindowEvent::MouseInput { state: btn_state, button, .. } => {
                let pos = state.cursor_pos.get();
                let action = match btn_state {
                    winit::event::ElementState::Pressed => MouseButtonAction::Down,
                    winit::event::ElementState::Released => MouseButtonAction::Up,
                };
                let btn = match button {
                    winit::event::MouseButton::Left => MouseButton::Left,
                    winit::event::MouseButton::Right => MouseButton::Right,
                    winit::event::MouseButton::Middle => MouseButton::Middle,
                    _ => return,
                };
                let webviews = state.webviews.borrow();
                if let Some(wv) = webviews.last() {
                    wv.notify_input_event(InputEvent::MouseButton(
                        servo::MouseButtonEvent::new(
                            action,
                            btn,
                            DevicePoint::new(pos.x as f32, pos.y as f32).into(),
                        ),
                    ));
                }
            }
            WindowEvent::ScaleFactorChanged { scale_factor, .. } => {
                let webviews = state.webviews.borrow();
                if let Some(wv) = webviews.last() {
                    wv.set_hidpi_scale_factor(Scale::new(scale_factor as f32));
                }
            }
            _ => {}
        }
    }

    fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
        let Self::Running(state) = self else { return };
        state.servo.spin_event_loop();

        if state.exit_flag.load(Ordering::Relaxed) {
            event_loop.exit();
            return;
        }

        // Mirror user_event's GenerateIT check in case the proxy wake fires before
        // about_to_wait runs.
        if *state.phase.borrow() == Phase::FetchingGenerateIT {
            let result = state.pending_integrity.lock().unwrap().take();
            if let Some(outcome) = result {
                match outcome {
                    Ok(bytes) => {
                        *state.phase.borrow_mut() = Phase::CreatingMinter;
                        state.inject_script_2(&bytes);
                    }
                    Err(e) => {
                        state.signal_done_result(Err(e));
                        event_loop.exit();
                    }
                }
            }
        }
    }
}

// ── YouTube BotGuard API calls ────────────────────────────────────────────────

async fn call_botguard(http: &reqwest::Client, url: &str, body: &str) -> Result<String, Error> {
    let resp = http
        .post(url)
        .header("Content-Type", "application/json+protobuf")
        .header("x-goog-api-key", GOOGLE_API_KEY)
        .header("x-user-agent", "grpc-web-javascript/0.1")
        .body(body.to_string())
        .send()
        .await?;
    let status = resp.status();
    let text = resp.text().await?;
    if !status.is_success() || text.is_empty() {
        return Err(Error::Parse(format!(
            "BotGuard {url} returned {status}: {text}"
        )));
    }
    Ok(text)
}

async fn fetch_generate_it(
    http: &reqwest::Client,
    bg_response: &str,
) -> Result<Vec<u8>, Error> {
    let req_body = serde_json::to_string(&json!([REQUEST_KEY, bg_response]))?;
    let raw = call_botguard(http, BOTGUARD_GENERATE_IT_URL, &req_body).await?;
    let (bytes, _expires) = parse_integrity_token(&raw)?;
    Ok(bytes)
}

// ── Challenge / token parsing (port of Metrolist's JavaScriptUtil.kt) ─────────

fn parse_challenge(raw: &str) -> Result<Value, Error> {
    let outer: Value = serde_json::from_str(raw)?;
    let outer = outer
        .as_array()
        .ok_or_else(|| Error::Parse("outer not array".into()))?;

    let challenge = if outer.len() > 1 && outer[1].is_string() {
        let descrambled = descramble(outer[1].as_str().unwrap())?;
        serde_json::from_str::<Value>(&descrambled)?
            .as_array()
            .ok_or_else(|| Error::Parse("descrambled not array".into()))?
            .clone()
    } else {
        outer[0]
            .as_array()
            .ok_or_else(|| Error::Parse("outer[0] not array".into()))?
            .clone()
    };

    let get_str = |i: usize| challenge.get(i).and_then(|v| v.as_str()).unwrap_or("");

    let safe_script = challenge
        .get(1)
        .and_then(|v| v.as_array())
        .and_then(|a| a.iter().find(|v| v.is_string()))
        .cloned()
        .unwrap_or(Value::Null);

    let trusted_url = challenge
        .get(2)
        .and_then(|v| v.as_array())
        .and_then(|a| a.iter().find(|v| v.is_string()))
        .cloned()
        .unwrap_or(Value::Null);

    Ok(json!({
        "messageId": get_str(0),
        "interpreterJavascript": {
            "privateDoNotAccessOrElseSafeScriptWrappedValue": safe_script,
            "privateDoNotAccessOrElseTrustedResourceUrlWrappedValue": trusted_url
        },
        "interpreterHash": get_str(3),
        "program": get_str(4),
        "globalName": get_str(5),
        "clientExperimentsStateBlob": get_str(7)
    }))
}

fn parse_integrity_token(raw: &str) -> Result<(Vec<u8>, u64), Error> {
    let arr: Value = serde_json::from_str(raw)?;
    let arr = arr
        .as_array()
        .ok_or_else(|| Error::Parse("integrity token not array".into()))?;

    let b64 = arr
        .first()
        .and_then(|v| v.as_str())
        .ok_or_else(|| Error::Parse("arr[0] not string".into()))?;
    let expires = arr
        .get(1)
        .and_then(|v| v.as_u64())
        .ok_or_else(|| Error::Parse("arr[1] not u64".into()))?;

    let bytes = base64_decode_yt(b64)?;
    Ok((bytes, expires))
}

fn descramble(b64: &str) -> Result<String, Error> {
    let normalized = b64.replace('-', "+").replace('_', "/").replace('.', "=");
    let bytes = B64
        .decode(&normalized)
        .map_err(|e| Error::Parse(format!("base64 descramble: {e}")))?;
    let decoded: Vec<u8> = bytes.iter().map(|&b| b.wrapping_add(97)).collect();
    String::from_utf8(decoded).map_err(|e| Error::Parse(format!("utf8 descramble: {e}")))
}

fn base64_decode_yt(b64: &str) -> Result<Vec<u8>, Error> {
    let normalized = b64.replace('-', "+").replace('_', "/").replace('.', "=");
    B64.decode(&normalized)
        .map_err(|e| Error::Parse(format!("base64: {e}")))
}

// ── Helpers ───────────────────────────────────────────────────────────────────

/// Build an inline `"0,1,2,…"` JS number literal suitable for `new Uint8Array([…])`.
fn bytes_to_js_array(bytes: &[u8]) -> String {
    bytes.iter().map(|b| b.to_string()).collect::<Vec<_>>().join(",")
}

/// Convert a comma-separated byte string (e.g. `"72,101,108,108,111"`) to base64url.
fn u8_csv_to_base64url(csv: &str) -> String {
    let bytes: Vec<u8> = csv
        .split(',')
        .filter_map(|s| s.trim().parse::<u8>().ok())
        .collect();
    B64.encode(&bytes)
        .replace('+', "-")
        .replace('/', "_")
        .trim_end_matches('=')
        .to_string()
}

/// Send a minimal 200 OK response for a signal URL.
fn serve_ok(load: WebResourceLoad, url: url::Url) {
    let mut headers = HeaderMap::new();
    headers.insert(
        http::header::CONTENT_TYPE,
        "text/plain".parse().unwrap(),
    );
    headers.insert(
        http::header::ACCESS_CONTROL_ALLOW_ORIGIN,
        "*".parse().unwrap(),
    );
    let response = WebResourceResponse::new(url).headers(headers);
    let mut intercepted = load.intercept(response);
    intercepted.send_body_data(b"ok".to_vec());
    intercepted.finish();
}

/// Signal the async caller with a result, mark exit, and optionally wake the proxy.
fn signal_done(
    result: Result<PoTokenPair, Error>,
    done_tx: &Arc<Mutex<Option<tokio::sync::oneshot::Sender<Result<PoTokenPair, Error>>>>>,
    exit_flag: &Arc<AtomicBool>,
    proxy: Option<&EventLoopProxy<PoTokenWake>>,
) {
    if let Some(tx) = done_tx.lock().unwrap().take() {
        let _ = tx.send(result);
    }
    exit_flag.store(true, Ordering::Relaxed);
    if let Some(p) = proxy {
        let _ = p.send_event(PoTokenWake);
    }
}
