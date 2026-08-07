use std::str::FromStr;

use freya::icons::lucide::{bookmark, house, plus};
use freya::prelude::*;
use freya::radio::use_radio;

use crate::app::{Data, DataChannel, NavCommand};

const SELECTED_BG: &str = "#282828";
/// Below this width the sidebar shows icons/thumbnails only.
const COLLAPSED_THRESHOLD: f32 = 140.;

pub struct Sidebar {
    pub width: f32,
}

impl PartialEq for Sidebar {
    fn eq(&self, other: &Self) -> bool {
        self.width == other.width
    }
}

fn nav_btn(
    icon: bytes::Bytes,
    text: &'static str,
    active: bool,
    collapsed: bool,
) -> impl IntoElement {
    let bg = if active {
        Color::from_hex(SELECTED_BG).unwrap()
    } else {
        Color::TRANSPARENT
    };

    if collapsed {
        rect()
            .center()
            .width(Size::Fill)
            .height(Size::px(44.))
            .corner_radius(8.)
            .background(bg)
            .on_pointer_enter(|_| Cursor::set(CursorIcon::Pointer))
            .on_pointer_leave(|_| Cursor::set(CursorIcon::Default))
            .on_press(|_| {})
            .child(
                SvgViewer::new(icon)
                    .color(Color::WHITE)
                    .fill(Color::WHITE)
                    .width(Size::px(20.))
                    .height(Size::px(20.)),
            )
    } else {
        rect()
            .horizontal()
            .width(Size::Fill)
            .height(Size::px(44.))
            .corner_radius(8.)
            .padding(Gaps::new_symmetric(0., 12.))
            .spacing(12.)
            .cross_align(Alignment::Center)
            .background(bg)
            .on_pointer_enter(|_| Cursor::set(CursorIcon::Pointer))
            .on_pointer_leave(|_| Cursor::set(CursorIcon::Default))
            .on_press(|_| {})
            .child(
                SvgViewer::new(icon)
                    .color(Color::WHITE)
                    .fill(Color::WHITE)
                    .width(Size::px(20.))
                    .height(Size::px(20.)),
            )
            .child(label().font_size(15.).color(Color::WHITE).text(text))
    }
}

impl Component for Sidebar {
    fn render(&self) -> impl IntoElement {
        let yt_radio = use_radio::<Data, DataChannel>(DataChannel::YtApi);
        let nav_radio = use_radio::<Data, DataChannel>(DataChannel::Navigation);

        let yt_state = yt_radio.read();
        let is_logged_in = yt_state.yt_session.is_some();
        let playlists = yt_state.library_playlists.clone();
        drop(yt_state);

        let nav_cmd = nav_radio.read().nav_cmd.clone();
        let collapsed = self.width < COLLAPSED_THRESHOLD;

        rect()
            .vertical()
            .width(Size::px(self.width))
            .height(Size::Fill)
            .overflow(Overflow::Clip)
            .padding(Gaps::new(8., 12., 8., 12.))
            .spacing(4.)
            .border(
                Border::new()
                    .alignment(BorderAlignment::Outer)
                    .width(1.)
                    .fill(Color::from_hex("#292929").unwrap()),
            )
            // ── nav buttons ───────────────────────────────────────────────
            .child(nav_btn(house(), "Inicio", true, collapsed))
            .child(nav_btn(bookmark(), "Mi biblioteca", false, collapsed))
            // ── separator ─────────────────────────────────────────────────
            .child(
                rect()
                    .width(Size::Fill)
                    .height(Size::px(1.))
                    .background(Color::from_hex("#2A2A2A").unwrap()),
            )
            // ── library (only when logged in) ─────────────────────────────
            .maybe_child(is_logged_in.then_some({
                let playlists_inner = playlists.clone();
                let nav_cmd_inner = nav_cmd.clone();
                rect()
                    .vertical()
                    .width(Size::Fill)
                    .height(Size::flex(1.0))
                    .spacing(2.)
                    // "Nueva lista" button
                    .child(if collapsed {
                        rect()
                            .center()
                            .width(Size::Fill)
                            .height(Size::px(40.))
                            .corner_radius(8.)
                            .background(Color::from_hex("#282828").unwrap())
                            .on_press(|_| {})
                            .on_pointer_enter(|_| Cursor::set(CursorIcon::Pointer))
                            .on_pointer_leave(|_| Cursor::set(CursorIcon::Default))
                            .child(
                                SvgViewer::new(plus())
                                    .color(Color::WHITE)
                                    .fill(Color::WHITE)
                                    .width(Size::px(16.))
                                    .height(Size::px(16.)),
                            )
                    } else {
                        rect()
                            .horizontal()
                            .width(Size::Fill)
                            .height(Size::px(40.))
                            .corner_radius(8.)
                            .padding(Gaps::new_symmetric(0., 12.))
                            .spacing(8.)
                            .cross_align(Alignment::Center)
                            .background(Color::from_hex("#282828").unwrap())
                            .on_press(|_| {})
                            .on_pointer_enter(|_| Cursor::set(CursorIcon::Pointer))
                            .on_pointer_leave(|_| Cursor::set(CursorIcon::Default))
                            .child(
                                SvgViewer::new(plus())
                                    .color(Color::WHITE)
                                    .fill(Color::WHITE)
                                    .width(Size::px(16.))
                                    .height(Size::px(16.)),
                            )
                            .child(
                                label()
                                    .font_size(14.)
                                    .color(Color::WHITE)
                                    .text("Nueva lista"),
                            )
                    })
                    .child(
                        ScrollView::new()
                            .direction(Direction::Vertical)
                            .show_scrollbar(false)
                            .width(Size::Fill)
                            .height(Size::flex(1.0))
                            .children(playlists_inner.into_iter().map(|p| {
                                let id = p.id.clone();
                                let name = p.title.clone();
                                let sub = p.author.clone().unwrap_or_default();
                                let thumb =
                                    p.thumbnail.as_deref().and_then(|u| Url::from_str(u).ok());
                                let nav = nav_cmd_inner.clone();
                                let press_id = id.clone();
                                let press_nav = nav.clone();

                                let thumb_rect = rect()
                                    .width(Size::px(44.))
                                    .height(Size::px(44.))
                                    .corner_radius(6.)
                                    .overflow(Overflow::Clip)
                                    .maybe_child(thumb.map(|url| {
                                        ImageViewer::new(url)
                                            .expanded()
                                            .image_cover(ImageCover::Center)
                                    }));

                                rect()
                                    .horizontal()
                                    .cross_align(Alignment::Center)
                                    .width(Size::Fill)
                                    .height(Size::px(56.))
                                    .corner_radius(6.)
                                    .overflow(Overflow::Clip)
                                    .padding(Gaps::new_symmetric(0., 4.))
                                    .spacing(10.)
                                    .on_press(move |_| {
                                        if let Some(ref tx) = nav {
                                            tx.try_send(NavCommand::BeginNavigation {
                                                playlist_id: id.clone(),
                                            })
                                            .ok();
                                            tx.try_send(NavCommand::LoadPlaylist(id.clone())).ok();
                                        }
                                    })
                                    .on_pointer_enter(|_| Cursor::set(CursorIcon::Pointer))
                                    .on_pointer_leave(|_| Cursor::set(CursorIcon::Default))
                                    .child(thumb_rect)
                                    .child(
                                        rect()
                                            .vertical()
                                            .width(Size::flex(1.0))
                                            .spacing(2.)
                                            .child(
                                                label()
                                                    .max_lines(1)
                                                    .text_overflow(TextOverflow::Ellipsis)
                                                    .font_size(14.)
                                                    .color(Color::WHITE)
                                                    .text(name),
                                            )
                                            .child(
                                                label()
                                                    .max_lines(1)
                                                    .font_size(12.)
                                                    .color(Color::from_hex("#888888").unwrap())
                                                    .text(sub),
                                            ),
                                    )
                                    .into_element()
                            })),
                    )
            }))
    }
}
