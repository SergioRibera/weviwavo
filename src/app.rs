use freya::{prelude::*, router::*};
use freya::radio::*;

use crate::components::{LoadingBar, PlayerBar};

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
    pub radio: RadioStation<Data, DataChannel>,
}

impl App for MainApp {
    fn render(&self) -> impl IntoElement {
        use_share_radio(move || self.radio);
        use_init_theme(|| PreferredTheme::Dark.to_theme());
        Router::<Route>::new(|| RouterConfig::default().with_initial_path(Route::Home))
    }
}

/// Shared layout rendered around every page: background gradient + loading bar + player bar.
#[derive(PartialEq)]
struct AppLayout;

impl Component for AppLayout {
    fn render(&self) -> impl IntoElement {
        // ── hooks ────────────────────────────────────────────────────────────
        let player_radio = use_radio::<Data, DataChannel>(DataChannel::Player);
        let nav_radio = use_radio::<Data, DataChannel>(DataChannel::Navigation);
        let router = RouterContext::get();

        // Guard against pushing the same route more than once per navigation.
        let mut route_pushed = use_state(|| false);

        let has_player = !player_radio.read().player.title.is_empty();
        let bg_image = ("bg_image", include_bytes!("../resources/bg_default.webp"));
        let platform = Platform::get().root_size;

        let nav_state = nav_radio.read();
        let is_loading = nav_state.is_loading;
        let pending_id = nav_state.pending_playlist_id.clone();
        let loaded_id = nav_state
            .playlist_view
            .as_ref()
            .map(|pv| pv.playlist.id.clone());
        let nav_cmd = nav_state.nav_cmd.clone();
        drop(nav_state);

        // ── delayed navigation ────────────────────────────────────────────────
        // Push the route only once data has arrived, so the old page stays
        // visible while the bar sweeps.
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
        // Reset guard when pending_id has been cleared by the nav engine.
        if pending_id.is_none() && *route_pushed.read() {
            route_pushed.set(false);
        }

        // ── layout ───────────────────────────────────────────────────────────
        rect()
            .vertical()
            .expanded()
            .content(Content::Flex)
            .theme_color()
            .background(Color::BLACK)
            .child(ContextMenuViewer::new())
            .child(LoadingBar { active: is_loading })
            .child(
                rect()
                    .width(Size::Fill)
                    .height(Size::flex(1.0))
                    .child(Outlet::<Route>::new()),
            )
            .maybe_child(has_player.then(PlayerBar::default))
    }
}
