use std::ops::Deref;

use freya::{animation::*, prelude::*};
use ytdroid::pages::home::HomeSection;

use super::{SongInfo, scroll_button, ScrollDir};

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

#[derive(PartialEq, Clone, Copy)]
enum WindowSize {
    Small, // <= 1024
    Large, // > 1024
}

impl Component for Section {
    fn render_key(&self) -> DiffKey {
        DiffKey::from(&self.home.title.clone().unwrap_or_default())
    }

    fn render(&self) -> impl IntoElement {
        let platform = Platform::get();

        let mut max_width_anim = use_animation(|_| {
            AnimNum::new(70., 100.)
                .function(Function::Sine)
                .ease(Ease::InOut)
                .time(100)
        });

        let mut prev_window_size = use_state(|| {
            let width = platform.root_size.read().width;
            if width <= 1024. {
                WindowSize::Small
            } else {
                WindowSize::Large
            }
        });

        let mut scroll_controller = use_scroll_controller(ScrollConfig::default);

        let current_width = platform.root_size.read().width;
        let current_window_size = if current_width <= 1024. {
            WindowSize::Small
        } else {
            WindowSize::Large
        };

        if current_window_size != *prev_window_size.read() {
            match current_window_size {
                WindowSize::Small => {
                    max_width_anim.start();
                }
                WindowSize::Large => {
                    max_width_anim.reverse();
                }
            }
            prev_window_size.set(current_window_size);
        }

        let scroll_amount = 270; // 250 (item size) + 20 (spacing)
        let content_len = self.items.len();

        // Freya stores scroll as negative offsets: 0 = start, -N = scrolled N px right.
        let (current_x, _): (i32, i32) = scroll_controller.into();
        let can_scroll_left = current_x < 0;

        let max_overflow = (content_len as i32 * scroll_amount).saturating_sub(scroll_amount * 4);
        let can_scroll_right = current_x > -max_overflow && content_len > 4;

        tracing::trace!(
            section = %self.home.title.as_deref().unwrap_or(""),
            content_len,
            current_x,
            can_scroll_left,
            can_scroll_right,
            max_overflow,
            "section render"
        );

        let max_width = max_width_anim.read().value();

        rect()
            .vertical()
            .spacing(16.)
            .padding(10.)
            .width(Size::Fill)
            .center()
            .child(
                rect()
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
                            .child(
                                rect()
                                    .spacing(0.)
                                    .vertical()
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
                    ),
            )
            .child(
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
                    ),
            )
    }
}
