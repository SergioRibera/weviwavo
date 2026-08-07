use std::cell::RefCell;
use std::rc::Rc;

use freya::animation::*;
use freya::prelude::*;
use freya::radio::use_radio;

use crate::app::{Data, DataChannel};
use crate::components::{HomeBar, Section, Sidebar, use_responsive_width};

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

        let chips: Vec<String> = feed_radio
            .read()
            .feed
            .chips
            .iter()
            .map(|c| c.title.clone())
            .collect();

        // ── sidebar animation ─────────────────────────────────────────────
        let mut sidebar_open = use_state(|| false);
        let mut prev_open = use_state(|| false);
        let mut sidebar_anim = use_animation(|_| {
            AnimNum::new(0., 240.)
                .function(Function::Sine)
                .ease(Ease::InOut)
                .time(250)
        });

        let open = *sidebar_open.read();
        if open != *prev_open.read() {
            if open { sidebar_anim.start() } else { sidebar_anim.reverse() }
            prev_open.set(open);
        }

        let sidebar_width = sidebar_anim.read().value();

        // Single stable callback — reads current state at press time, so
        // HomeBar never needs to re-render when the sidebar toggles.
        let on_toggle: Rc<RefCell<dyn FnMut()>> =
            Rc::new(RefCell::new(move || sidebar_open.set(!*sidebar_open.read())));

        // ── layout ────────────────────────────────────────────────────────
        // HomeBar is full-width and sits above the sidebar+content split so
        // the search bar never shifts when the sidebar opens.
        rect()
            .vertical()
            .expanded()
            .child(HomeBar {
                on_menu: Some(on_toggle),
                show_back_btn: false,
            })
            .child(
                rect()
                    .horizontal()
                    .width(Size::Fill)
                    .height(Size::flex(1.0))
                    .child(Sidebar {
                        width: sidebar_width,
                    })
                    .child(
                        ScrollView::new()
                            .width(Size::flex(1.0))
                            .height(Size::Fill)
                            .direction(Direction::Vertical)
                            .spacing(18.)
                            .child(ChipsRow { chips })
                            .children(
                                feed_radio
                                    .read()
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
