use std::ops::Not;

use freya::prelude::*;

pub fn song() -> impl IntoElement {
    let title = "🪉 Alabanzas";
    let ty = "Sergio Ribera";
    let details = "119 pistas";
    let is_playing = false;
    let is_album = true;
    let is_artist = false;
    let thumbnail = "https://yt3.googleusercontent.com/THlqr9zXY7ZdMYYo2PxoPvlZeSo-ySb-oyiZvQnj5k6kYfqtFytbYh3RxB8u7nio72AS8qGnCfA=s576";

    let mut size = use_state(|| 223.);
    let mut play_btn_size = use_state(|| 48.);
    let mut hover = use_state(|| false);
    let mut hover_options = use_state(|| false);

    rect()
        .vertical()
        .spacing(12.)
        .child(
            rect()
                .expanded()
                .center()
                .width(Size::px(size()))
                .height(Size::px(size()))
                .rounded_lg()
                .overflow(Overflow::Clip)
                .on_pointer_enter(move |_| hover.set(true))
                .on_pointer_leave(move |_| hover.set(false))
                .child(
                    ImageViewer::new(thumbnail)
                        .expanded()
                        .center()
                        .image_cover(ImageCover::Center)
                        // center play button
                        .maybe_child(
                            (!is_playing && !is_artist).then_some(
                                rect()
                                    .rounded_full()
                                    .width(Size::px(play_btn_size()))
                                    .height(Size::px(play_btn_size()))
                                    .background(Color::BLUE)
                                    .position(
                                        Position::new_absolute()
                                            .top((size() / 2.) - (play_btn_size() / 2.))
                                            .left((size() / 2.) - (play_btn_size() / 2.)),
                                    ),
                            ),
                        )
                        // context right top
                        .maybe_child(
                            (*hover.read() && !is_artist).then_some(
                                rect()
                                    .width(Size::px(36.))
                                    .height(Size::px(36.))
                                    .rounded_full()
                                    .on_pointer_enter(move |_| hover_options.set(true))
                                    .on_pointer_leave(move |_| hover_options.set(false))
                                    .position(Position::new_absolute().top(8.).right(8.))
                                    .background(Color::WHITE.with_a(if *hover_options.read() {
                                        180
                                    } else {
                                        0
                                    })),
                            ),
                        )
                        .child(rect().expanded().background(
                            Color::BLACK.with_a(if *hover.read() && !is_artist { 60 } else { 0 }),
                        )),
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
                        .text(title),
                )
                .child(
                    rect()
                        .horizontal()
                        .spacing(2.)
                        .text_align(TextAlign::Left)
                        .font_weight(FontWeight::NORMAL)
                        .color(Color::from_hex("#B3B3B3").unwrap())
                        .child(label().text(ty))
                        .maybe_child(
                            details
                                .is_empty()
                                .not()
                                .then_some(label().text("•").max_lines(1)),
                        )
                        .maybe_child(
                            details
                                .is_empty()
                                .not()
                                .then_some(label().text(details).max_lines(1)),
                        ),
                ),
        )
}
