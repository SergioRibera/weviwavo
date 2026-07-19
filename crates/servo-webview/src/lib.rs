//! Servo-backed WebView for the weviwavo login flow.
//!
//! Call [`run_login`] before launching the main Freya app. It opens a Servo
//! window, navigates to the YouTube Music login page, and returns the raw
//! `Cookie:` header string once the user successfully signs in.
//!
//! The function blocks the calling thread (runs its own winit event loop) and
//! is safe to call on the main thread on all supported platforms.

use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::sync::{Arc, Mutex};

use euclid::Scale;
use servo::{
    CookieSource, DevicePoint, EventLoopWaker, InputEvent, Key, KeyState, KeyboardEvent,
    LoadStatus, MouseButton, MouseButtonAction, MouseButtonEvent, MouseMoveEvent, NamedKey,
    RenderingContext, Servo, ServoBuilder, WebView, WebViewBuilder, WebViewDelegate,
    WheelDelta, WheelEvent, WheelMode, WindowRenderingContext,
};
use url::Url;
use winit::application::ApplicationHandler;
use winit::dpi::PhysicalPosition;
use winit::event::{ElementState, MouseScrollDelta, WindowEvent};
use winit::event_loop::{ActiveEventLoop, EventLoop, EventLoopProxy};
use winit::raw_window_handle::{HasDisplayHandle, HasWindowHandle};
use winit::window::{Window, WindowAttributes};

/// Error returned by [`run_login`].
#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("winit event loop: {0}")]
    EventLoop(#[from] winit::error::EventLoopError),
    #[error("rendering context: {0}")]
    RenderingContext(String),
    #[error("login cancelled — window was closed before signing in")]
    Cancelled,
}

/// Open a YouTube Music login window backed by Servo.
///
/// Blocks the calling thread until the user completes login or closes the
/// window. On success returns the raw `Cookie:` header string (e.g.
/// `"SAPISID=…; __Secure-3PAPISID=…; …"`) suitable for passing to
/// `ytdroid::YouTube::new(Some(&cookies), …)`.
///
/// # Errors
///
/// Returns [`Error::Cancelled`] when the user closes the window before
/// signing in, or an OS error if the event loop or rendering context fails.
pub fn run_login() -> Result<String, Error> {
    rustls::crypto::aws_lc_rs::default_provider()
        .install_default()
        .ok(); // ok to fail if already installed

    let result: Arc<Mutex<Option<String>>> = Arc::new(Mutex::new(None));
    let result_clone = result.clone();

    let event_loop = EventLoop::<WakerEvent>::with_user_event().build()?;
    let mut app = App::new(&event_loop, result_clone);
    event_loop.run_app(&mut app)?;

    Arc::try_unwrap(result)
        .unwrap_or_else(|arc| Mutex::new(arc.lock().unwrap().clone()))
        .into_inner()
        .unwrap()
        .ok_or(Error::Cancelled)
}

// ─── EventLoopWaker ──────────────────────────────────────────────────────────

#[derive(Clone)]
struct Waker(EventLoopProxy<WakerEvent>);

#[derive(Debug)]
struct WakerEvent;

impl EventLoopWaker for Waker {
    fn clone_box(&self) -> Box<dyn EventLoopWaker> {
        Box::new(Self(self.0.clone()))
    }

    fn wake(&self) {
        if let Err(e) = self.0.send_event(WakerEvent) {
            tracing::warn!(error = ?e, "failed to wake servo event loop");
        }
    }
}

// ─── Login delegate ───────────────────────────────────────────────────────────

struct LoginState {
    window: Window,
    servo: Servo,
    rendering_context: Rc<WindowRenderingContext>,
    webviews: RefCell<Vec<WebView>>,
    cursor_pos: Cell<PhysicalPosition<f64>>,
    /// Set to the cookie header string when login completes.
    result: Arc<Mutex<Option<String>>>,
    should_exit: Cell<bool>,
}

impl WebViewDelegate for LoginState {
    fn notify_new_frame_ready(&self, _: WebView) {
        self.window.request_redraw();
    }

    fn notify_load_status_changed(&self, webview: WebView, status: LoadStatus) {
        if status != LoadStatus::Complete {
            return;
        }
        let Some(url) = webview.url() else { return };
        if url.host_str() != Some("music.youtube.com") {
            return;
        }

        tracing::debug!(%url, "login complete — extracting cookies");

        let cookies = self
            .servo
            .site_data_manager()
            .cookies_for_url(url, CookieSource::HTTP);

        let header = cookies
            .iter()
            .map(|c| format!("{}={}", c.name(), c.value()))
            .collect::<Vec<_>>()
            .join("; ");

        tracing::debug!(cookies = cookies.len(), "cookies extracted");

        *self.result.lock().unwrap() = Some(header);
        self.should_exit.set(true);
        self.window.request_redraw();
    }

    fn notify_url_changed(&self, _: WebView, url: Url) {
        tracing::debug!(%url, "url changed");
        self.window.set_title(&format!("Sign in — {}", url.host_str().unwrap_or("")));
    }
}

// ─── ApplicationHandler ───────────────────────────────────────────────────────

enum App {
    Initial {
        waker: Waker,
        result: Arc<Mutex<Option<String>>>,
    },
    Running(Rc<LoginState>),
}

impl App {
    fn new(event_loop: &EventLoop<WakerEvent>, result: Arc<Mutex<Option<String>>>) -> Self {
        Self::Initial {
            waker: Waker(event_loop.create_proxy()),
            result,
        }
    }
}

impl ApplicationHandler<WakerEvent> for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        let Self::Initial { waker, result } = self else { return };

        let attrs = WindowAttributes::default()
            .with_title("Sign in to YouTube Music")
            .with_inner_size(winit::dpi::LogicalSize::new(700u32, 850u32));
        let window = event_loop
            .create_window(attrs)
            .expect("failed to create login window");

        let display_handle = event_loop.display_handle().expect("no display handle");
        let window_handle = window.window_handle().expect("no window handle");
        let size = window.inner_size();

        let rendering_context = Rc::new(
            WindowRenderingContext::new(display_handle, window_handle, size)
                .expect("failed to create WindowRenderingContext"),
        );
        rendering_context.make_current().expect("make_current failed");

        let servo = ServoBuilder::default()
            .event_loop_waker(Box::new(waker.clone()))
            .build();
        servo.setup_logging();

        let state = Rc::new(LoginState {
            window,
            servo,
            rendering_context,
            webviews: RefCell::new(Vec::new()),
            cursor_pos: Cell::new(PhysicalPosition::new(0.0, 0.0)),
            result: result.clone(),
            should_exit: Cell::new(false),
        });

        let login_url =
            Url::parse("https://accounts.google.com/ServiceLogin?service=youtube&continue=https%3A%2F%2Fmusic.youtube.com%2F")
                .expect("hardcoded URL is valid");

        let scale = Scale::new(state.window.scale_factor() as f32);
        let webview = WebViewBuilder::new(&state.servo, state.rendering_context.clone())
            .url(login_url)
            .hidpi_scale_factor(scale)
            .delegate(state.clone())
            .build();

        state.webviews.borrow_mut().push(webview);
        *self = Self::Running(state);
    }

    fn user_event(&mut self, _: &ActiveEventLoop, _: WakerEvent) {
        if let Self::Running(state) = self {
            state.servo.spin_event_loop();
        }
    }

    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        _window_id: winit::window::WindowId,
        event: WindowEvent,
    ) {
        let Self::Running(state) = self else { return };

        state.servo.spin_event_loop();

        let webview = state.webviews.borrow();
        let Some(webview) = webview.last() else { return };

        match event {
            WindowEvent::CloseRequested => {
                event_loop.exit();
            }
            WindowEvent::RedrawRequested => {
                if state.should_exit.get() {
                    event_loop.exit();
                    return;
                }
                webview.paint();
                state.rendering_context.present();
            }
            WindowEvent::Resized(new_size) => {
                webview.resize(new_size);
            }
            WindowEvent::CursorMoved { position, .. } => {
                state.cursor_pos.set(position);
                webview.notify_input_event(InputEvent::MouseMove(MouseMoveEvent::new(
                    DevicePoint::new(position.x as f32, position.y as f32).into(),
                )));
            }
            WindowEvent::MouseInput { state: btn_state, button, .. } => {
                let pos = state.cursor_pos.get();
                let action = match btn_state {
                    ElementState::Pressed => MouseButtonAction::Down,
                    ElementState::Released => MouseButtonAction::Up,
                };
                let btn = match button {
                    winit::event::MouseButton::Left => MouseButton::Left,
                    winit::event::MouseButton::Right => MouseButton::Right,
                    winit::event::MouseButton::Middle => MouseButton::Middle,
                    _ => return,
                };
                webview.notify_input_event(InputEvent::MouseButton(MouseButtonEvent::new(
                    action,
                    btn,
                    DevicePoint::new(pos.x as f32, pos.y as f32).into(),
                )));
            }
            WindowEvent::MouseWheel { delta, .. } => {
                let pos = state.cursor_pos.get();
                let (dx, dy, mode) = match delta {
                    MouseScrollDelta::LineDelta(x, y) => {
                        (x as f64 * 76.0, y as f64 * 76.0, WheelMode::DeltaLine)
                    }
                    MouseScrollDelta::PixelDelta(d) => (d.x, d.y, WheelMode::DeltaPixel),
                };
                webview.notify_input_event(InputEvent::Wheel(WheelEvent::new(
                    WheelDelta { x: dx, y: dy, z: 0.0, mode },
                    DevicePoint::new(pos.x as f32, pos.y as f32).into(),
                )));
            }
            WindowEvent::KeyboardInput { event: key_event, .. } => {
                let state_k = match key_event.state {
                    ElementState::Pressed => KeyState::Down,
                    ElementState::Released => KeyState::Up,
                };
                let key = match key_event.logical_key {
                    winit::keyboard::Key::Character(ref s) => Key::Character(s.to_string()),
                    winit::keyboard::Key::Named(winit::keyboard::NamedKey::Backspace) => {
                        Key::Named(NamedKey::Backspace)
                    }
                    winit::keyboard::Key::Named(winit::keyboard::NamedKey::Enter) => {
                        Key::Named(NamedKey::Enter)
                    }
                    winit::keyboard::Key::Named(winit::keyboard::NamedKey::Tab) => {
                        Key::Named(NamedKey::Tab)
                    }
                    winit::keyboard::Key::Named(winit::keyboard::NamedKey::Escape) => {
                        Key::Named(NamedKey::Escape)
                    }
                    winit::keyboard::Key::Named(winit::keyboard::NamedKey::ArrowLeft) => {
                        Key::Named(NamedKey::ArrowLeft)
                    }
                    winit::keyboard::Key::Named(winit::keyboard::NamedKey::ArrowRight) => {
                        Key::Named(NamedKey::ArrowRight)
                    }
                    winit::keyboard::Key::Named(winit::keyboard::NamedKey::Delete) => {
                        Key::Named(NamedKey::Delete)
                    }
                    _ => return,
                };
                webview.notify_input_event(InputEvent::Keyboard(
                    KeyboardEvent::from_state_and_key(state_k, key),
                ));
            }
            WindowEvent::ScaleFactorChanged { scale_factor, .. } => {
                webview.set_hidpi_scale_factor(Scale::new(scale_factor as f32));
            }
            _ => {}
        }
    }

    fn about_to_wait(&mut self, _: &ActiveEventLoop) {
        if let Self::Running(state) = self {
            state.servo.spin_event_loop();
        }
    }
}
