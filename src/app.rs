use std::cell::RefCell;
use std::rc::Rc;

use freya::animation::*;
use freya::{prelude::*, router::*};

use crate::components::{HomeBar, LoadingBar, PlayerBar, PlayerExpanded, Sidebar};

mod album;
mod artist;
mod data;
mod home;
mod playlist;

pub use data::*;
use album::Album;
use artist::Artist;
use home::Home;
pub use playlist::Playlist;

#[derive(Routable, Clone, PartialEq)]
#[rustfmt::skip]
pub enum Route {
    #[layout(AppLayout)]
        #[route("/")]
        Home,
        #[route("/playlist/:id")]
        Playlist { id: String },
        #[route("/artist/:id")]
        Artist { id: String },
        #[route("/album/:id")]
        Album { id: String },
}

#[derive(Clone)]
pub struct MainApp {
    pub radio: freya::radio::RadioStation<Data, DataChannel>,
}

impl App for MainApp {
    fn render(&self) -> impl IntoElement {
        use freya::radio::use_share_radio;
        use_share_radio(move || self.radio);
        use_init_theme(|| PreferredTheme::Dark.to_theme());
        Router::<Route>::new(|| RouterConfig::default().with_initial_path(Route::Home))
    }
}

/// Shared layout: loading bar + HomeBar + [Sidebar | page content] + player bar.
#[derive(PartialEq)]
struct AppLayout;

const SIDEBAR_COLLAPSED: f32 = 72.;
const SIDEBAR_EXPANDED: f32 = 240.;
const SIDEBAR_THRESHOLD: f32 = 140.; // below this → icon-only mode

impl Component for AppLayout {
    fn render(&self) -> impl IntoElement {
        use freya::radio::use_radio;

        // ── radio ─────────────────────────────────────────────────────────
        let player_radio = use_radio::<Data, DataChannel>(DataChannel::Player);
        let nav_radio = use_radio::<Data, DataChannel>(DataChannel::Navigation);
        let router = RouterContext::get();

        // ── route ─────────────────────────────────────────────────────────
        let current_route = use_route::<Route>();
        let show_back = !matches!(current_route, Route::Home);

        // ── delayed navigation guard ──────────────────────────────────────
        let mut route_pushed = use_state(|| false);

        let has_player = !player_radio.read().player.title.is_empty();
        let is_expanded = player_radio.read().player_expanded;
        let bg_image = ("bg_image", include_bytes!("../resources/bg_default.webp"));

        let root_size = Platform::get().root_size.read();
        let window_w = root_size.width;
        let window_h = root_size.height;
        drop(root_size);
        const HOMEBAR_H: f32 = 64.;
        const PLAYERBAR_H: f32 = 67.;
        let content_h = (window_h - HOMEBAR_H - PLAYERBAR_H).max(0.);

        let nav_state = nav_radio.read();
        let is_loading = nav_state.is_loading;
        let pending_id = nav_state.pending_playlist_id.clone();
        let loaded_id = nav_state
            .playlist_view
            .as_ref()
            .map(|pv| pv.playlist.id.clone());
        let nav_cmd = nav_state.nav_cmd.clone();
        drop(nav_state);

        if !is_loading && !*route_pushed.read() {
            if let Some(ref pid) = pending_id {
                if loaded_id.as_deref() == Some(pid.as_str()) {
                    route_pushed.set(true);
                    _ = router.push(Route::Playlist { id: pid.clone() });
                    if let Some(tx) = &nav_cmd {
                        tx.try_send(NavCommand::ClearPending).ok();
                    }
                }
            }
        }
        if pending_id.is_none() && *route_pushed.read() {
            route_pushed.set(false);
        }

        // ── sidebar animation ─────────────────────────────────────────────
        let mut sidebar_open = use_state(|| false);
        let mut prev_open = use_state(|| false);
        let mut sidebar_anim = use_animation(|_| {
            AnimNum::new(SIDEBAR_COLLAPSED, SIDEBAR_EXPANDED)
                .function(Function::Sine)
                .ease(Ease::InOut)
                .time(250)
        });

        let open = *sidebar_open.read();
        if open != *prev_open.read() {
            if open { sidebar_anim.start() } else { sidebar_anim.reverse() }
            prev_open.set(open);
        }
        let sidebar_width = sidebar_anim.read().value();

        let on_toggle: Rc<RefCell<dyn FnMut()>> =
            Rc::new(RefCell::new(move || sidebar_open.toggle()));

        // ── expand animation ──────────────────────────────────────────────
        let expand_anim = use_animation_with_dependencies(&is_expanded, move |conf, &open| {
            conf.on_change(OnChange::Rerun);
            AnimNum::new(
                if open { content_h } else { 0. },
                if open { 0. } else { content_h },
            )
            .function(Function::Sine)
            .ease(Ease::Out)
            .time(350)
        });
        let expand_slide = expand_anim.get().value();
        let show_expanded = is_expanded || *expand_anim.is_running().read();

        // ── layout ────────────────────────────────────────────────────────
        rect()
            .vertical()
            .expanded()
            .content(Content::Flex)
            .theme_color()
            .background(Color::BLACK)
            .child(ContextMenuViewer::new())
            .child(LoadingBar { active: is_loading })
            .child(HomeBar {
                on_menu: Some(on_toggle),
                show_back_btn: show_back,
            })
            .child(
                rect()
                    .horizontal()
                    .width(Size::Fill)
                    .height(Size::flex(1.0))
                    .child(Sidebar { width: sidebar_width })
                    .child(
                        rect()
                            .vertical()
                            .width(Size::flex(1.0))
                            .height(Size::Fill)
                            .child(Outlet::<Route>::new()),
                    ),
            )
            .maybe_child((show_expanded && has_player).then(|| {
                rect()
                    .position(Position::new_global())
                    .offset_x(0.)
                    .offset_y(HOMEBAR_H + expand_slide)
                    .width(Size::px(window_w))
                    .height(Size::px(content_h))
                    .child(PlayerExpanded::default())
                    .into_element()
            }))
            .maybe_child(has_player.then(PlayerBar::default))
    }
}

