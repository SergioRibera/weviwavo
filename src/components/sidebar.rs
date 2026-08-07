use std::time::Duration;

use freya::icons::lucide::{bookmark, house, plus};
use freya::prelude::*;
use freya::radio::use_radio;

use super::cached_image_source;
use crate::app::{Data, DataChannel, NavCommand};

const SELECTED_BG: &str = "#282828";
const SIDEBAR_COLLAPSED_W: f32 = 72.;

pub struct Sidebar {
    pub width: f32,
}

impl PartialEq for Sidebar {
    fn eq(&self, other: &Self) -> bool {
        self.width == other.width
    }
}

fn nav_btn(icon: bytes::Bytes, text: &'static str, active: bool, collapsed: bool) -> Element {
    let bg = if active {
        Color::from_hex(SELECTED_BG).unwrap()
    } else {
        Color::TRANSPARENT
    };

    let btn = rect()
        .horizontal()
        .width(Size::Fill)
        .height(Size::px(44.))
        .corner_radius(8.)
        .overflow(Overflow::Clip)
        .content(Content::Flex)
        .spacing(8.)
        .cross_align(Alignment::Center)
        .main_align(Alignment::Start)
        .background(bg)
        .on_pointer_enter(|_| Cursor::set(CursorIcon::Pointer))
        .on_pointer_leave(|_| Cursor::set(CursorIcon::Default))
        .on_press(|_| {})
        .child(
            rect()
                .center()
                .width(Size::px(48.))
                .height(Size::px(44.))
                .child(
                    SvgViewer::new(icon)
                        .color(Color::WHITE)
                        .width(Size::px(20.))
                        .height(Size::px(20.)),
                ),
        )
        .child(
            rect()
                .width(Size::flex(1.0))
                .overflow(Overflow::Clip)
                .cross_align(Alignment::Start)
                .child(
                    label()
                        .font_size(15.)
                        .color(Color::WHITE)
                        .text(text)
                        .max_lines(1)
                        .text_overflow(TextOverflow::Ellipsis),
                ),
        );

    if collapsed {
        TooltipContainer::new(Tooltip::new(text))
            .position(AttachedPosition::Right)
            .delay(Duration::from_millis(300))
            .child(btn)
            .into_element()
    } else {
        btn.into_element()
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
        let collapsed = self.width <= SIDEBAR_COLLAPSED_W;

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
                    .child({
                        let nueva_btn = rect()
                            .horizontal()
                            .width(Size::Fill)
                            .height(Size::px(40.))
                            .corner_radius(8.)
                            .overflow(Overflow::Clip)
                            .content(Content::Flex)
                            .spacing(8.)
                            .cross_align(Alignment::Center)
                            .main_align(Alignment::Start)
                            .background(Color::from_hex("#282828").unwrap())
                            .on_press(|_| {})
                            .on_pointer_enter(|_| Cursor::set(CursorIcon::Pointer))
                            .on_pointer_leave(|_| Cursor::set(CursorIcon::Default))
                            .child(
                                rect()
                                    .center()
                                    .width(Size::px(48.))
                                    .height(Size::px(48.))
                                    .child(
                                        SvgViewer::new(plus())
                                            .color(Color::WHITE)
                                            .width(Size::px(16.))
                                            .height(Size::px(16.)),
                                    ),
                            )
                            .child(
                                rect()
                                    .width(Size::flex(1.0))
                                    .overflow(Overflow::Clip)
                                    .cross_align(Alignment::Start)
                                    .child(
                                        label()
                                            .font_size(14.)
                                            .color(Color::WHITE)
                                            .text("Nueva lista")
                                            .max_lines(1)
                                            .text_overflow(TextOverflow::Ellipsis),
                                    ),
                            );
                        if collapsed {
                            TooltipContainer::new(Tooltip::new("Nueva lista"))
                                .position(AttachedPosition::Right)
                                .delay(Duration::from_millis(300))
                                .child(nueva_btn)
                                .into_element()
                        } else {
                            nueva_btn.into_element()
                        }
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
                                let tooltip_name = name.clone();
                                let sub = p.author.clone().unwrap_or_default();
                                let thumb_src =
                                    p.thumbnail.as_deref().and_then(cached_image_source);
                                let nav = nav_cmd_inner.clone();

                                let thumb_rect = rect()
                                    .width(Size::px(44.))
                                    .height(Size::px(44.))
                                    .corner_radius(6.)
                                    .overflow(Overflow::Clip)
                                    .maybe_child(thumb_src.map(|src| {
                                        ImageViewer::new(src)
                                            .expanded()
                                            .image_cover(ImageCover::Center)
                                    }));

                                let row = rect()
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
                                                    .text_overflow(TextOverflow::Ellipsis)
                                                    .color(Color::from_hex("#888888").unwrap())
                                                    .text(sub),
                                            ),
                                    );

                                if collapsed {
                                    TooltipContainer::new(Tooltip::new(tooltip_name))
                                        .position(AttachedPosition::Right)
                                        .delay(Duration::from_millis(300))
                                        .child(row)
                                        .into_element()
                                } else {
                                    row.into_element()
                                }
                            })),
                    )
            }))
    }
}
