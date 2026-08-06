use freya::prelude::*;
use freya::radio::use_radio;

use crate::app::{Data, DataChannel};
use crate::components::{Section, use_responsive_width};

#[derive(PartialEq)]
struct ChipsRow {
    chips: Vec<String>,
}

impl Component for ChipsRow {
    fn render(&self) -> impl IntoElement {
        let max_width = use_responsive_width();

        rect()
            .padding(10.)
            .width(Size::Fill)
            .height(Size::Inner)
            .center()
            .child(
                rect()
                    .horizontal()
                    .content(Content::wrap())
                    .width(Size::percent(max_width))
                    .spacing(8.)
                    .children(self.chips.iter().map(|title| {
                        rect()
                            .padding((8., 16.))
                            .corner_radius(8.)
                            .background(Color::from_hex("#282828").unwrap())
                            .color(Color::WHITE)
                            .child(label().font_size(14.).text(title.clone()))
                            .into_element()
                    })),
            )
    }
}

#[derive(PartialEq)]
pub struct Home;

impl Component for Home {
    fn render(&self) -> impl IntoElement {
        let feed_radio = use_radio::<Data, DataChannel>(DataChannel::Feed);
        let radio = feed_radio.read();

        let chips: Vec<String> = radio.feed.chips.iter().map(|c| c.title.clone()).collect();

        rect()
            .expanded()
            .child(
                ScrollView::new()
                    .expanded()
                    .direction(Direction::Vertical)
                    .spacing(18.)
                    .child(ChipsRow { chips })
                    .children(
                        radio
                            .feed
                            .sections
                            .clone()
                            .into_iter()
                            .map(Section::new)
                            .map(IntoElement::into_element),
                    ),
            )
    }
}
