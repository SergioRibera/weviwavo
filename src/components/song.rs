use std::ops::Not;
use std::str::FromStr;

use freya::animation::*;
use freya::icons::lucide::{audio_lines, play};
use freya::prelude::*;

pub struct SongInfo {
    title: String,
    ty: String,
    details: String,
    is_album: bool,
    is_artist: bool,
    thumbnail: String,
}

impl Default for SongInfo {
    fn default() -> Self {
        Self {
            title: "Amor Como Fuego".into(),
            ty: "Canción".into(),
            details: "Hillsong En Español".into(),
            is_album: true,
            is_artist: false,
            thumbnail: "https://lh3.googleusercontent.com/8daRI8WzbLKSrodjKXQy-50Yegbuxmh16mF7BQ8aTxFtku67M7Wh4oaWNzy38uNL_blAd8FW18drn4OmxQ=w226-h226-l90-rj".into(),
        }
    }
}

pub fn song(info: SongInfo) -> impl IntoElement {
    let mut is_playing = use_state(|| false);
    let mut size = use_state(|| 223.);
    let mut play_btn_size = use_state(|| 48.);
    let mut hover = use_state(|| false);

    let mut anim_album_play = use_animation(|_| {
        (
            AnimColor::new(Color::BLACK.with_a(180), Color::BLACK)
                .function(Function::Sine)
                .ease(Ease::InOut)
                .time(100),
            AnimNum::new(40., 60.)
                .function(Function::Sine)
                .ease(Ease::InOut)
                .time(100),
        )
    });

    let (album_play_color, album_play_size) = anim_album_play.read().value();
    let album_play_size = Size::px(album_play_size);

    rect()
        .vertical()
        .spacing(12.)
        .child(
            rect()
                .expanded()
                .center()
                .width(Size::px(size()))
                .height(Size::px(size()))
                .corner_radius(if info.is_artist { size() } else { 8. })
                .overflow(Overflow::Clip)
                .on_pointer_enter(move |_| {
                    Cursor::set(CursorIcon::Pointer);
                    hover.set(true);
                })
                .on_pointer_leave(move |_| {
                    Cursor::set(CursorIcon::Default);
                    hover.set(false);
                })
                .on_secondary_press(move |_| ContextMenu::open(Menu::new()))
                .child(
                    ImageViewer::new(Uri::from_str(info.thumbnail.as_str()).unwrap())
                        .expanded()
                        .center()
                        .image_cover(ImageCover::Center)
                        .center()
                        .child(
                            rect()
                                .expanded()
                                .center()
                                .on_press(move |_| {
                                    if !info.is_album && !info.is_artist {
                                        is_playing.toggle()
                                    }
                                })
                                .background(Color::BLACK.with_a(
                                    if *hover.read() && !info.is_artist {
                                        60
                                    } else {
                                        0
                                    },
                                ))
                                // center play button
                                .maybe_child(
                                    (!info.is_album && !info.is_artist).then_some(
                                        svg(if *is_playing.read() {
                                            audio_lines()
                                        } else {
                                            play()
                                        })
                                        .fill(Color::WHITE)
                                        .color(Color::WHITE)
                                        .width(Size::px(play_btn_size()))
                                        .height(Size::px(play_btn_size())),
                                    ),
                                ),
                        )
                        // album play/pause right bottom
                        .maybe_child(
                            (*hover.read() && info.is_album).then_some(
                                rect()
                                    .width(album_play_size.clone())
                                    .height(album_play_size)
                                    .rounded_full()
                                    .padding(12.)
                                    .on_press(move |_| is_playing.toggle())
                                    .on_pointer_enter(move |_| {
                                        anim_album_play.start();
                                    })
                                    .on_pointer_leave(move |_| {
                                        anim_album_play.reverse();
                                    })
                                    .position(Position::new_absolute().bottom(8.).right(8.))
                                    .background(album_play_color)
                                    .child(
                                        svg(if *is_playing.read() {
                                            audio_lines()
                                        } else {
                                            play()
                                        })
                                        .fill(Color::WHITE)
                                        .color(Color::WHITE)
                                        .expanded(),
                                    ),
                            ),
                        ),
                ),
        )
        .child(
            rect()
                .vertical()
                .spacing(2.)
                .child(
                    label()
                        .max_lines(1)
                        .text_align(TextAlign::Left)
                        .font_weight(FontWeight::BOLD)
                        .text(info.title),
                )
                .child(
                    rect()
                        .horizontal()
                        .spacing(2.)
                        .text_align(TextAlign::Left)
                        .font_weight(FontWeight::NORMAL)
                        .color(Color::from_hex("#B3B3B3").unwrap())
                        .child(
                            CursorArea::new()
                                .icon(CursorIcon::Pointer)
                                .child(label().text(info.ty)),
                        )
                        .maybe_child(
                            info.details
                                .is_empty()
                                .not()
                                .then_some(label().text("•").max_lines(1)),
                        )
                        .maybe_child(
                            info.details.is_empty().not().then_some(
                                CursorArea::new()
                                    .icon(CursorIcon::Pointer)
                                    .child(label().text(info.details).max_lines(1)),
                            ),
                        ),
                ),
        )
}
