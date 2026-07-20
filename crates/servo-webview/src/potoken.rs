//! PoToken generation via Servo WebView + local HTTP proxy.
//!
//! Mirrors Metrolist's `PoTokenWebView` pattern: all YouTube API calls are made
//! from Rust; the Servo browser only provides the JS runtime for the BotGuard VM.
//!
//! Call [`generate`] from a tokio async context.

use std::cell::Cell;
use std::rc::Rc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use base64::{Engine, engine::general_purpose::STANDARD as B64};
use euclid::Scale;
use serde_json::{Value, json};
use servo::{
    DevicePoint, EventLoopWaker, InputEvent, LoadStatus, MouseButton, MouseButtonAction,
    RenderingContext, Servo, ServoBuilder, WebView, WebViewBuilder,
    WebViewDelegate, WindowRenderingContext,
};
use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader};
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
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
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
/// Starts an invisible Servo browser and a local HTTP proxy. The browser runs
/// the BotGuard JS; all YouTube API round-trips are handled by Rust.
///
/// # Errors
///
/// Returns [`Error::Timeout`] after 45 s if the BotGuard flow does not complete.
pub async fn generate(session_id: &str, video_id: &str) -> Result<PoTokenPair, Error> {
    rustls::crypto::aws_lc_rs::default_provider()
        .install_default()
        .ok();

    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await?;
    let port = listener.local_addr()?.port();

    let (done_tx, done_rx) =
        tokio::sync::oneshot::channel::<Result<PoTokenPair, Error>>();
    let done_tx = Arc::new(Mutex::new(Some(done_tx)));

    // Servo sets this after creating its EventLoop so the HTTP task can wake it.
    let proxy_store: Arc<Mutex<Option<EventLoopProxy<PoTokenWake>>>> =
        Arc::new(Mutex::new(None));

    let exit_flag = Arc::new(AtomicBool::new(false));

    // ── HTTP server task (async) ─────────────────────────────────────────────
    {
        let session_id = session_id.to_string();
        let video_id = video_id.to_string();
        let done_tx = done_tx.clone();
        let exit_flag = exit_flag.clone();
        let proxy_store = proxy_store.clone();
        let http = reqwest::Client::builder().user_agent(BG_USER_AGENT).build()?;

        tokio::spawn(async move {
            run_http_server(
                listener, http, session_id, video_id, done_tx, exit_flag, proxy_store,
            )
            .await;
        });
    }

    // ── Servo task (blocking — winit event loop) ─────────────────────────────
    {
        let exit_flag = exit_flag.clone();
        let proxy_store = proxy_store.clone();

        tokio::task::spawn_blocking(move || {
            run_servo(port, exit_flag, proxy_store);
        });
    }

    tokio::time::timeout(std::time::Duration::from_secs(45), done_rx)
        .await
        .map_err(|_| Error::Timeout)?
        .map_err(|_| Error::Cancelled)?
}

// ── Local HTTP server ─────────────────────────────────────────────────────────

async fn run_http_server(
    listener: tokio::net::TcpListener,
    http: reqwest::Client,
    session_id: String,
    video_id: String,
    done_tx: Arc<Mutex<Option<tokio::sync::oneshot::Sender<Result<PoTokenPair, Error>>>>>,
    exit_flag: Arc<AtomicBool>,
    proxy_store: Arc<Mutex<Option<EventLoopProxy<PoTokenWake>>>>,
) {
    loop {
        let Ok((stream, _)) = listener.accept().await else { break };
        let is_done = handle_request(
            stream,
            &http,
            &session_id,
            &video_id,
            &done_tx,
            &exit_flag,
            &proxy_store,
        )
        .await;
        if is_done {
            break;
        }
    }
}

async fn handle_request(
    stream: tokio::net::TcpStream,
    http: &reqwest::Client,
    session_id: &str,
    video_id: &str,
    done_tx: &Arc<Mutex<Option<tokio::sync::oneshot::Sender<Result<PoTokenPair, Error>>>>>,
    exit_flag: &Arc<AtomicBool>,
    proxy_store: &Arc<Mutex<Option<EventLoopProxy<PoTokenWake>>>>,
) -> bool {
    let (read_half, mut writer) = stream.into_split();
    let mut reader = BufReader::new(read_half);

    let mut request_line = String::new();
    if reader.read_line(&mut request_line).await.unwrap_or(0) == 0 {
        return false;
    }

    let mut parts = request_line.split_ascii_whitespace();
    let method = parts.next().unwrap_or("").to_string();
    let path = parts.next().unwrap_or("").to_string();

    let mut content_length: usize = 0;
    loop {
        let mut h = String::new();
        if reader.read_line(&mut h).await.unwrap_or(0) == 0 {
            break;
        }
        if h == "\r\n" || h == "\n" {
            break;
        }
        let hl = h.to_ascii_lowercase();
        if hl.starts_with("content-length:") {
            content_length = hl[15..].trim().parse().unwrap_or(0);
        }
    }

    let mut body = vec![0u8; content_length];
    let _ = reader.read_exact(&mut body).await;
    let body_str = String::from_utf8_lossy(&body).into_owned();

    let is_done = match (method.as_str(), path.as_str()) {
        ("GET", "/") => {
            serve_html(&mut writer, session_id, video_id).await;
            false
        }
        ("POST", "/bg/create") => {
            handle_bg_create(&mut writer, http).await;
            false
        }
        ("POST", "/bg/generateit") => {
            handle_bg_generateit(&mut writer, http, &body_str).await;
            false
        }
        ("POST", "/bg/done") => {
            handle_bg_done(&mut writer, &body_str, done_tx, exit_flag, proxy_store).await;
            true
        }
        ("POST", "/bg/error") => {
            handle_bg_error(&mut writer, &body_str, done_tx, exit_flag, proxy_store).await;
            true
        }
        _ => {
            let _ = write_response(&mut writer, 404, "text/plain", b"Not Found").await;
            false
        }
    };

    is_done
}

// ── Route handlers ────────────────────────────────────────────────────────────

async fn serve_html<W: AsyncWriteExt + Unpin>(
    writer: &mut W,
    session_id: &str,
    video_id: &str,
) {
    let page_data =
        json!({ "sessionId": session_id, "videoId": video_id }).to_string();
    let html = PO_TOKEN_HTML.replace("\"__PAGE_DATA__\"", &page_data);
    let _ = write_response(writer, 200, "text/html; charset=utf-8", html.as_bytes()).await;
}

async fn handle_bg_create<W: AsyncWriteExt + Unpin>(writer: &mut W, http: &reqwest::Client) {
    let req_body = serde_json::to_string(&json!([REQUEST_KEY])).unwrap();
    match call_botguard(http, BOTGUARD_CREATE_URL, &req_body).await {
        Ok(raw) => match parse_challenge(&raw) {
            Ok(parsed) => {
                let body = parsed.to_string();
                let _ =
                    write_response(writer, 200, "application/json", body.as_bytes()).await;
            }
            Err(e) => {
                tracing::error!("PoToken: parse challenge: {e}");
                let _ = write_response(
                    writer,
                    500,
                    "text/plain",
                    format!("parse: {e}").as_bytes(),
                )
                .await;
            }
        },
        Err(e) => {
            tracing::error!("PoToken: BotGuard Create: {e}");
            let _ = write_response(
                writer,
                502,
                "text/plain",
                format!("upstream: {e}").as_bytes(),
            )
            .await;
        }
    }
}

async fn handle_bg_generateit<W: AsyncWriteExt + Unpin>(
    writer: &mut W,
    http: &reqwest::Client,
    body_str: &str,
) {
    let bg_response = serde_json::from_str::<Value>(body_str)
        .ok()
        .and_then(|v| v["botguardResponse"].as_str().map(String::from));
    let Some(bg_response) = bg_response else {
        let _ =
            write_response(writer, 400, "text/plain", b"missing botguardResponse").await;
        return;
    };

    let req_body =
        serde_json::to_string(&json!([REQUEST_KEY, bg_response])).unwrap();
    match call_botguard(http, BOTGUARD_GENERATE_IT_URL, &req_body).await {
        Ok(raw) => match parse_integrity_token(&raw) {
            Ok((bytes, expires)) => {
                let resp = json!({
                    "integrityToken": bytes,
                    "expiresInSeconds": expires
                });
                let body = resp.to_string();
                let _ =
                    write_response(writer, 200, "application/json", body.as_bytes())
                        .await;
            }
            Err(e) => {
                tracing::error!("PoToken: parse integrity token: {e}");
                let _ = write_response(
                    writer,
                    500,
                    "text/plain",
                    format!("parse: {e}").as_bytes(),
                )
                .await;
            }
        },
        Err(e) => {
            tracing::error!("PoToken: BotGuard GenerateIT: {e}");
            let _ = write_response(
                writer,
                502,
                "text/plain",
                format!("upstream: {e}").as_bytes(),
            )
            .await;
        }
    }
}

async fn handle_bg_done<W: AsyncWriteExt + Unpin>(
    writer: &mut W,
    body_str: &str,
    done_tx: &Arc<Mutex<Option<tokio::sync::oneshot::Sender<Result<PoTokenPair, Error>>>>>,
    exit_flag: &Arc<AtomicBool>,
    proxy_store: &Arc<Mutex<Option<EventLoopProxy<PoTokenWake>>>>,
) {
    let _ = write_response(writer, 200, "text/plain", b"ok").await;

    let parsed = serde_json::from_str::<Value>(body_str).ok();
    let player = parsed
        .as_ref()
        .and_then(|v| v["player_token"].as_str())
        .unwrap_or("")
        .to_string();
    let streaming = parsed
        .as_ref()
        .and_then(|v| v["streaming_token"].as_str())
        .unwrap_or("")
        .to_string();

    signal_done(Ok(PoTokenPair { player, streaming }), done_tx, exit_flag, proxy_store);
}

async fn handle_bg_error<W: AsyncWriteExt + Unpin>(
    writer: &mut W,
    body_str: &str,
    done_tx: &Arc<Mutex<Option<tokio::sync::oneshot::Sender<Result<PoTokenPair, Error>>>>>,
    exit_flag: &Arc<AtomicBool>,
    proxy_store: &Arc<Mutex<Option<EventLoopProxy<PoTokenWake>>>>,
) {
    let _ = write_response(writer, 200, "text/plain", b"ok").await;
    let err = Error::JsError(body_str.to_string());
    tracing::error!("PoToken: JS error: {body_str}");
    signal_done(Err(err), done_tx, exit_flag, proxy_store);
}

fn signal_done(
    result: Result<PoTokenPair, Error>,
    done_tx: &Arc<Mutex<Option<tokio::sync::oneshot::Sender<Result<PoTokenPair, Error>>>>>,
    exit_flag: &Arc<AtomicBool>,
    proxy_store: &Arc<Mutex<Option<EventLoopProxy<PoTokenWake>>>>,
) {
    if let Some(tx) = done_tx.lock().unwrap().take() {
        let _ = tx.send(result);
    }
    exit_flag.store(true, Ordering::Relaxed);
    if let Some(proxy) = proxy_store.lock().unwrap().as_ref() {
        let _ = proxy.send_event(PoTokenWake);
    }
}

// ── YouTube BotGuard API calls ────────────────────────────────────────────────

async fn call_botguard(
    http: &reqwest::Client,
    url: &str,
    body: &str,
) -> Result<String, Error> {
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

// ── HTTP response writer ───────────────────────────────────────────────────────

async fn write_response<W: AsyncWriteExt + Unpin>(
    writer: &mut W,
    status: u16,
    content_type: &str,
    body: &[u8],
) -> std::io::Result<()> {
    let status_text = match status {
        200 => "OK",
        400 => "Bad Request",
        404 => "Not Found",
        500 => "Internal Server Error",
        502 => "Bad Gateway",
        _ => "Unknown",
    };
    let header = format!(
        "HTTP/1.1 {status} {status_text}\r\n\
         Content-Type: {content_type}\r\n\
         Content-Length: {}\r\n\
         Access-Control-Allow-Origin: *\r\n\
         Connection: close\r\n\
         \r\n",
        body.len()
    );
    writer.write_all(header.as_bytes()).await?;
    writer.write_all(body).await?;
    writer.flush().await
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
    port: u16,
    exit_flag: Arc<AtomicBool>,
    proxy_store: Arc<Mutex<Option<EventLoopProxy<PoTokenWake>>>>,
) {
    let el = match EventLoop::<PoTokenWake>::with_user_event().build() {
        Ok(el) => el,
        Err(e) => {
            tracing::error!("PoToken: event loop failed: {e}");
            return;
        }
    };

    let proxy = el.create_proxy();
    *proxy_store.lock().unwrap() = Some(proxy.clone());

    let mut app = PoTokenApp::Initial {
        waker: PoTokenWaker(proxy),
        port,
        exit_flag,
    };

    if let Err(e) = el.run_app(&mut app) {
        tracing::error!("PoToken: servo event loop error: {e}");
    }
}

struct PoTokenState {
    window: Window,
    servo: Servo,
    rendering_context: Rc<WindowRenderingContext>,
    webviews: std::cell::RefCell<Vec<WebView>>,
    cursor_pos: Cell<PhysicalPosition<f64>>,
    exit_flag: Arc<AtomicBool>,
    should_exit: Cell<bool>,
}

impl WebViewDelegate for PoTokenState {
    fn notify_new_frame_ready(&self, _: WebView) {
        self.window.request_redraw();
    }

    fn notify_load_status_changed(&self, _: WebView, status: LoadStatus) {
        if status == LoadStatus::Complete && self.exit_flag.load(Ordering::Relaxed) {
            self.should_exit.set(true);
            self.window.request_redraw();
        }
    }
}

enum PoTokenApp {
    Initial {
        waker: PoTokenWaker,
        port: u16,
        exit_flag: Arc<AtomicBool>,
    },
    Running(Rc<PoTokenState>),
}

impl ApplicationHandler<PoTokenWake> for PoTokenApp {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        let Self::Initial { waker, port, exit_flag } = self else { return };

        let attrs = WindowAttributes::default()
            .with_title("PoToken")
            .with_visible(false)
            .with_inner_size(winit::dpi::LogicalSize::new(1u32, 1u32));
        let window = match event_loop.create_window(attrs) {
            Ok(w) => w,
            Err(e) => {
                tracing::error!("PoToken: window creation failed: {e}");
                event_loop.exit();
                return;
            }
        };

        let display_handle = event_loop
            .display_handle()
            .expect("no display handle");
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
            webviews: std::cell::RefCell::new(Vec::new()),
            cursor_pos: Cell::new(PhysicalPosition::new(0.0, 0.0)),
            exit_flag: exit_flag.clone(),
            should_exit: Cell::new(false),
        });

        let url = url::Url::parse(&format!("http://127.0.0.1:{port}/"))
            .expect("valid localhost url");
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
        if let Self::Running(state) = self {
            state.servo.spin_event_loop();
            if state.exit_flag.load(Ordering::Relaxed) {
                event_loop.exit();
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
        if let Self::Running(state) = self {
            state.servo.spin_event_loop();
            if state.exit_flag.load(Ordering::Relaxed) {
                event_loop.exit();
            }
        }
    }
}
