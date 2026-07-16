use freya::icons::lucide::{chevron_left, chevron_right};
use freya::prelude::*;

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum ScrollDir {
    Left,
    Right,
}

pub fn scroll_button(
    dir: ScrollDir,
    enabled: bool,
    mut on_press: impl FnMut() + 'static,
) -> impl IntoElement {
    let color = if enabled {
        Color::WHITE
    } else {
        Color::from_hex("#404040").unwrap()
    };
    let icon = match dir {
        ScrollDir::Left => chevron_left(),
        ScrollDir::Right => chevron_right(),
    };

    rect()
        .rounded_full()
        .padding(Gaps::new_all(8.))
        .cross_align(Alignment::Center)
        .main_align(Alignment::Center)
        .width(Size::px(40.))
        .height(Size::px(40.))
        .border(Some(Border::new().width(2.).fill(color)))
        .on_press(move |_| {
            if enabled {
                on_press();
            }
        })
        .child(
            SvgViewer::new(icon)
                .color(color)
                .width(Size::px(24.))
                .height(Size::px(24.)),
        )
}
