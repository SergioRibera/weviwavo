use std::str::FromStr;

use bytes::Bytes;
use freya::icons::lucide::{
    audio_lines, chevron_up, ellipsis_vertical, play, repeat, shuffle, skip_back, skip_forward,
    thumbs_down, thumbs_up, volume_2,
};
use freya::prelude::*;
use freya::radio::use_radio;

use crate::app::{Data, DataChannel};
use crate::audio::AudioCommand;

fn fmt_secs(secs: f32) -> String {
    let s = secs as u32;
    format!("{}:{:02}", s / 60, s % 60)
}

fn icon_btn(icon: Bytes, size: f32) -> impl IntoElement {
    rect()
        .center()
        .padding(Gaps::new_all(6.))
        .rounded_full()
        .on_pointer_enter(|_| Cursor::set(CursorIcon::Pointer))
        .on_pointer_leave(|_| Cursor::set(CursorIcon::Default))
        .child(
            SvgViewer::new(icon)
                .color(Color::WHITE)
                .fill(Color::WHITE)
                .width(Size::px(size))
                .height(Size::px(size)),
        )
}

#[derive(Clone, PartialEq, Default)]
pub struct PlayerBar {
    layout: LayoutData,
}

impl LayoutExt for PlayerBar {
    fn get_layout(&mut self) -> &mut LayoutData {
        &mut self.layout
    }
}

impl ContainerExt for PlayerBar {}

impl Component for PlayerBar {
    fn render(&self) -> impl IntoElement {
        let radio = use_radio::<Data, DataChannel>(DataChannel::Player);
        let state = radio.read();
        let p = &state.player;

        let is_playing = p.is_playing;
        let current_secs = p.current_secs;
        let total_secs = p.total_secs;
        let title = p.title.clone();
        let artist = p.artist.clone();
        let album = p.album.clone();
        let year = p.year.clone();
        let thumbnail_url = p.thumbnail_url.clone();
        let audio_cmd = state.audio_cmd.clone();

        let progress_pct = if total_secs > 0. {
            (current_secs / total_secs * 100.).clamp(0., 100.)
        } else {
            0.
        };

        let subtitle = {
            let mut parts = vec![artist.as_str(), album.as_str()];
            let year_str = year.as_deref().unwrap_or("");
            if !year_str.is_empty() {
                parts.push(year_str);
            }
            parts.retain(|s: &&str| !s.is_empty());
            parts.join(" • ")
        };

        rect()
            .vertical()
            .width(Size::Fill)
            .background(Color::from_hex("#0F0F0F").unwrap())
            // seek bar
            .child(
                rect()
                    .width(Size::Fill)
                    .height(Size::px(3.))
                    .background(Color::from_hex("#2D2D2D").unwrap())
                    .child(
                        rect()
                            .width(Size::percent(progress_pct))
                            .height(Size::Fill)
                            .background(Color::from_hex("#FF0000").unwrap()),
                    ),
            )
            // main row
            .child(
                rect()
                    .horizontal()
                    .width(Size::Fill)
                    .height(Size::px(64.))
                    .padding(Gaps::new_symmetric(0., 16.))
                    .cross_align(Alignment::Center)
                    // left: transport controls + time
                    .child(
                        rect()
                            .horizontal()
                            .spacing(4.)
                            .cross_align(Alignment::Center)
                            .child(icon_btn(skip_back(), 20.))
                            .child(
                                rect()
                                    .center()
                                    .width(Size::px(36.))
                                    .height(Size::px(36.))
                                    .rounded_full()
                                    .border(Some(Border::new().width(1.5).fill(Color::WHITE)))
                                    .on_pointer_enter(|_| Cursor::set(CursorIcon::Pointer))
                                    .on_pointer_leave(|_| Cursor::set(CursorIcon::Default))
                                    .on_press(move |_| {
                                        let Some(tx) = audio_cmd.clone() else { return };
                                        let cmd = if is_playing {
                                            AudioCommand::Pause
                                        } else {
                                            AudioCommand::Resume
                                        };
                                        tx.try_send(cmd).ok();
                                    })
                                    .child(
                                        SvgViewer::new(if is_playing {
                                            audio_lines()
                                        } else {
                                            play()
                                        })
                                        .color(Color::WHITE)
                                        .fill(Color::WHITE)
                                        .width(Size::px(18.))
                                        .height(Size::px(18.)),
                                    ),
                            )
                            .child(icon_btn(skip_forward(), 20.))
                            .child(
                                label()
                                    .text(format!(
                                        "{} / {}",
                                        fmt_secs(current_secs),
                                        fmt_secs(total_secs)
                                    ))
                                    .font_size(12.)
                                    .color(Color::from_hex("#AAAAAA").unwrap()),
                            ),
                    )
                    // center: thumbnail + song info + reactions
                    .child(
                        rect()
                            .horizontal()
                            .expanded()
                            .spacing(12.)
                            .cross_align(Alignment::Center)
                            .main_align(Alignment::Center)
                            .maybe_child((!thumbnail_url.is_empty()).then(|| {
                                rect()
                                    .width(Size::px(48.))
                                    .height(Size::px(48.))
                                    .overflow(Overflow::Clip)
                                    .corner_radius(4.)
                                    .child(
                                        ImageViewer::new(
                                            Url::from_str(&thumbnail_url).unwrap(),
                                        )
                                        .expanded()
                                        .image_cover(ImageCover::Center),
                                    )
                                    .into_element()
                            }))
                            .child(
                                rect()
                                    .vertical()
                                    .spacing(2.)
                                    .child(
                                        label()
                                            .text(title)
                                            .font_size(14.)
                                            .font_weight(FontWeight::BOLD)
                                            .max_lines(1)
                                            .text_overflow(TextOverflow::Ellipsis),
                                    )
                                    .child(
                                        label()
                                            .text(subtitle)
                                            .font_size(12.)
                                            .color(Color::from_hex("#AAAAAA").unwrap())
                                            .max_lines(1)
                                            .text_overflow(TextOverflow::Ellipsis),
                                    ),
                            )
                            .child(icon_btn(thumbs_up(), 18.))
                            .child(icon_btn(thumbs_down(), 18.))
                            .child(icon_btn(ellipsis_vertical(), 18.)),
                    )
                    // right: volume, repeat, shuffle, expand
                    .child(
                        rect()
                            .horizontal()
                            .spacing(4.)
                            .cross_align(Alignment::Center)
                            .child(icon_btn(volume_2(), 18.))
                            .child(icon_btn(repeat(), 18.))
                            .child(icon_btn(shuffle(), 18.))
                            .child(icon_btn(chevron_up(), 18.)),
                    ),
            )
    }
}
