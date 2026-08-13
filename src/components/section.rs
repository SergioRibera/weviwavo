use std::ops::Deref;

use freya::prelude::*;
use freya::radio::use_radio;
use ytdroid::models::YTItem;
use ytdroid::pages::home::{HomeSection, HomeSectionLayout};

use super::{SongInfo, scroll_button, use_cached_thumbnail, use_responsive_width, ScrollDir};
use crate::app::{Data, DataChannel};
use crate::audio::{AudioCommand, AudioQuality};

// ── List-layout row item ──────────────────────────────────────────────────────

#[derive(Clone, PartialEq)]
struct FeedListRow {
    item: YTItem,
    col_width: f32,
}

impl Component for FeedListRow {
    fn render(&self) -> impl IntoElement {
        let nav_state = use_radio::<Data, DataChannel>(DataChannel::Navigation);
        let audio_cmd = nav_state.read().audio_cmd.clone();
        let mut hover = use_state(|| false);

        let (id, title, artist, album, thumbnail) = match &self.item {
            YTItem::Song(s) => (
                s.id.clone(),
                s.title.clone(),
                s.artists.first().map(|a| a.name.clone()).unwrap_or_default(),
                s.album.as_ref().map(|a| a.name.clone()).unwrap_or_default(),
                s.thumbnail.clone().unwrap_or_default(),
            ),
            YTItem::Album(v) => (
                v.id.clone(),
                v.title.clone(),
                v.artists.first().map(|a| a.name.clone()).unwrap_or_default(),
                String::new(),
                v.thumbnail.clone().unwrap_or_default(),
            ),
            YTItem::Playlist(v) => (
                v.id.clone(),
                v.title.clone(),
                v.author.clone().unwrap_or_default(),
                String::new(),
                v.thumbnail.clone().unwrap_or_default(),
            ),
            YTItem::Artist(v) => (
                v.id.clone(),
                v.title.clone(),
                String::new(),
                String::new(),
                v.thumbnail.clone().unwrap_or_default(),
            ),
            YTItem::Podcast(v) => (
                v.id.clone(),
                v.title.clone(),
                v.author.clone().unwrap_or_default(),
                String::new(),
                v.thumbnail.clone().unwrap_or_default(),
            ),
            YTItem::Episode(v) => (
                v.id.clone(),
                v.title.clone(),
                v.podcast.as_ref().map(|p| p.name.clone()).unwrap_or_default(),
                String::new(),
                v.thumbnail.clone().unwrap_or_default(),
            ),
        };

        let thumb_src = use_cached_thumbnail(&thumbnail);

        let col_width = self.col_width;

        // Clone for on_press capture so the values remain usable for labels below.
        let press_id = id.clone();
        let press_title = title.clone();
        let press_artist = artist.clone();
        let press_album = album.clone();
        let press_thumbnail = thumbnail.clone();

        rect()
            .horizontal()
            .spacing(10.)
            .padding(Gaps::new_symmetric(5., 4.))
            .width(Size::px(col_width))
            .cross_align(Alignment::Center)
            .corner_radius(6.)
            .background(if *hover.read() {
                Color::WHITE.with_a(15)
            } else {
                Color::TRANSPARENT
            })
            .on_pointer_enter(move |_| {
                Cursor::set(CursorIcon::Pointer);
                hover.set(true);
            })
            .on_pointer_leave(move |_| {
                Cursor::set(CursorIcon::Default);
                hover.set(false);
            })
            .on_press(move |_| {
                let Some(tx) = audio_cmd.clone() else { return };
                tx.try_send(AudioCommand::Play {
                    video_id: press_id.clone(),
                    quality: AudioQuality::Medium,
                    title: press_title.clone(),
                    artist: press_artist.clone(),
                    album: press_album.clone(),
                    thumbnail_url: press_thumbnail.clone(),
                })
                .ok();
            })
            // Thumbnail
            .child(
                rect()
                    .width(Size::px(50.))
                    .height(Size::px(50.))
                    .corner_radius(4.)
                    .overflow(Overflow::Clip)
                    .maybe_child(thumb_src.map(|src| {
                        ImageViewer::new(src)
                            .expanded()
                            .image_cover(ImageCover::Center)
                    })),
            )
            // Text
            .child(
                rect()
                    .vertical()
                    .spacing(3.)
                    .width(Size::flex(1.))
                    .child(
                        label()
                            .text(title.clone())
                            .max_lines(1)
                            .text_overflow(TextOverflow::Ellipsis)
                            .font_weight(FontWeight::MEDIUM)
                            .color(Color::WHITE),
                    )
                    .child(
                        label()
                            .text({
                                let mut sub = artist.clone();
                                if !album.is_empty() {
                                    sub.push_str(" • ");
                                    sub.push_str(&album);
                                }
                                sub
                            })
                            .max_lines(1)
                            .text_overflow(TextOverflow::Ellipsis)
                            .font_size(12.)
                            .color(Color::from_hex("#B3B3B3").unwrap()),
                    ),
            )
    }
}

// ── Section ───────────────────────────────────────────────────────────────────

#[derive(PartialEq)]
pub struct Section {
    layout: LayoutData,
    home: HomeSection,
}

impl Section {
    pub fn new(home: HomeSection) -> Self {
        Self {
            home,
            layout: LayoutData::default(),
        }
    }
}

impl Deref for Section {
    type Target = HomeSection;

    fn deref(&self) -> &Self::Target {
        &self.home
    }
}

impl LayoutExt for Section {
    fn get_layout(&mut self) -> &mut LayoutData {
        &mut self.layout
    }
}

impl ContainerExt for Section {}

impl Component for Section {
    fn render_key(&self) -> DiffKey {
        DiffKey::from(&self.home.title.clone().unwrap_or_default())
    }

    fn render(&self) -> impl IntoElement {
        let max_width = use_responsive_width();
        let mut scroll_controller = use_scroll_controller(ScrollConfig::default);

        // Freya stores scroll as negative offsets: 0 = start, -N = scrolled N px right.
        let (current_x, _): (i32, i32) = scroll_controller.into();
        let can_scroll_left = current_x < 0;

        let is_list = matches!(
            self.home.layout,
            HomeSectionLayout::List | HomeSectionLayout::Grid
        );

        // col_width for List: each column is wide enough for a song row.
        let col_width: f32 = 420.;
        let list_scroll_amount = (col_width as i32) + 20;
        let list_col_count = self.items.len().div_ceil(4) as i32;

        let (scroll_amount, can_scroll_right) = if is_list {
            let overflow = (list_col_count * list_scroll_amount)
                .saturating_sub(list_scroll_amount * 3);
            (list_scroll_amount, current_x > -overflow && list_col_count > 3)
        } else {
            let amount = 270i32; // 250 (card) + 20 (gap)
            let overflow =
                (self.items.len() as i32 * amount).saturating_sub(amount * 4);
            (amount, current_x > -overflow && self.items.len() > 4)
        };

        tracing::trace!(
            section = %self.home.title.as_deref().unwrap_or(""),
            content_len = self.items.len(),
            current_x,
            can_scroll_left,
            can_scroll_right,
            "section render"
        );

        // ── Header row (title + scroll buttons) ──────────────────────────────
        let header = rect()
            .horizontal()
            .width(Size::Fill)
            .cross_align(Alignment::End)
            .main_align(Alignment::SpaceBetween)
            .padding(Gaps::new_symmetric(0., 8.))
            .max_width(Size::percent(max_width))
            .child(
                rect()
                    .spacing(15.)
                    .horizontal()
                    .cross_align(Alignment::Center)
                    .maybe_child(self.home.thumbnail.as_deref().map(|url| {
                        let thumb = use_cached_thumbnail(url);
                        rect()
                            .width(Size::px(44.))
                            .height(Size::px(44.))
                            .rounded_full()
                            .overflow(Overflow::Clip)
                            .maybe_child(thumb.map(|src| {
                                ImageViewer::new(src)
                                    .expanded()
                                    .image_cover(ImageCover::Center)
                            }))
                    }))
                    .child(
                        rect()
                            .spacing(2.)
                            .vertical()
                            .maybe_child(self.home.strapline.clone().map(|s| {
                                label()
                                    .font_size(11.)
                                    .font_weight(FontWeight::MEDIUM)
                                    .color(Color::from_hex("#B3B3B3").unwrap())
                                    .text(s.to_uppercase())
                            }))
                            .child(
                                label()
                                    .font_weight(FontWeight::BOLD)
                                    .font_size(54.)
                                    .text(self.home.title.clone().unwrap_or_default()),
                            ),
                    ),
            )
            .child(
                rect().horizontal().spacing(12.).children([
                    scroll_button(ScrollDir::Left, can_scroll_left, move || {
                        let target = (current_x + scroll_amount).min(0);
                        scroll_controller.scroll_to_x(target);
                    })
                    .into_element(),
                    scroll_button(ScrollDir::Right, can_scroll_right, move || {
                        let target = current_x - scroll_amount;
                        scroll_controller.scroll_to_x(target);
                    })
                    .into_element(),
                ]),
            );

        // ── Scroll content ────────────────────────────────────────────────────
        let scroll_content = if is_list {
            // Group items into columns of 4, scroll horizontally.
            ScrollView::new_controlled(scroll_controller)
                .spacing(20.)
                .direction(Direction::Horizontal)
                .height(Size::Inner)
                .show_scrollbar(false)
                .width(Size::percent(max_width))
                .children(self.items.chunks(4).map(|chunk| {
                    rect()
                        .vertical()
                        .spacing(4.)
                        .width(Size::px(col_width))
                        .height(Size::Inner)
                        .children(chunk.iter().map(|item| {
                            FeedListRow { item: item.clone(), col_width }
                                .into_element()
                        }))
                        .into_element()
                }))
                .into_element()
        } else {
            ScrollView::new_controlled(scroll_controller)
                .spacing(20.)
                .direction(Direction::Horizontal)
                .height(Size::Inner)
                .show_scrollbar(false)
                .width(Size::percent(max_width))
                .children(
                    self.items
                        .iter()
                        .map(SongInfo::from)
                        .map(IntoElement::into_element),
                )
                .into_element()
        };

        rect()
            .vertical()
            .spacing(16.)
            .padding(10.)
            .width(Size::Fill)
            .center()
            .child(header)
            .child(scroll_content)
    }
}
