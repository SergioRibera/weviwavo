use freya::prelude::*;
use freya::radio::use_radio;

use crate::app::{Data, DataChannel};
use crate::components::Section;

#[derive(PartialEq)]
pub struct Home;

impl Component for Home {
    fn render(&self) -> impl IntoElement {
        let feed_radio = use_radio::<Data, DataChannel>(DataChannel::Feed);
        let radio = feed_radio.read();

        rect()
            .vertical()
            .expanded()
            .content(Content::Flex)
            .child(
                rect()
                    .horizontal()
                    .spacing(10.)
                    .color(Color::WHITE)
                    .children(radio.feed.chips.iter().map(|c| {
                        rect()
                            .padding((10., 15.))
                            .corner_radius(8.)
                            .background(Color::GRAY)
                            .child(label().text(c.title.clone()))
                            .into_element()
                    })),
            )
            .child(
                rect()
                    .width(Size::Fill)
                    .height(Size::flex(1.0))
                    .child(
                        ScrollView::new()
                            .expanded()
                            .direction(Direction::Vertical)
                            .spacing(18.)
                            .children(
                                radio
                                    .feed
                                    .sections
                                    .clone()
                                    .into_iter()
                                    .map(Section::new)
                                    .map(IntoElement::into_element),
                            ),
                    ),
            )
    }
}
