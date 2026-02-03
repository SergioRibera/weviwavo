use std::ops::Deref;

use freya::icons::lucide::{chevron_left, chevron_right};
use freya::prelude::*;
use ytmapi_rs::parse::HomeSection;

use super::SongInfo;

#[derive(PartialEq)]
pub struct Section(pub HomeSection);

impl Deref for Section {
    type Target = HomeSection;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl Component for Section {
    fn render(&self) -> impl IntoElement {
        let notifier = use_state(|| ());
        let requests = use_state(|| Vec::new());
        let mut scroll_position = use_state(|| (0i32, 0i32));
        let on_scroll = use_state(|| {
            Callback::new(move |ev: ScrollEvent| {
                let current = *scroll_position.read();
                match ev {
                    ScrollEvent::X(x) => {
                        scroll_position.write().0 = x;
                    }
                    ScrollEvent::Y(y) => {
                        scroll_position.write().1 = y;
                    }
                }
                current != *scroll_position.read()
            })
        });
        let get_scroll = use_state(|| Callback::new(move |_| *scroll_position.read()));

        let mut scroll_controller = use_hook(|| {
            ScrollController::managed(
                notifier.clone(),
                requests.clone(),
                on_scroll.clone(),
                get_scroll.clone(),
            )
        });

        let scroll_amount = 270; // 250 (item size) + 16 (spacing)
        let content = self.contents.clone();
        let content_len = content.len();

        let (current_x, _) = *scroll_position.read();
        let can_scroll_left = current_x > 0;

        // Estimate max scroll based on content
        // Assuming viewport shows ~4-5 items at once
        let max_scroll = (content_len as i32 * scroll_amount).saturating_sub(scroll_amount * 4);
        let can_scroll_right = current_x < max_scroll && content_len > 4;

        rect()
            .vertical()
            .spacing(16.)
            .padding(10.)
            .child(
                rect()
                    .horizontal()
                    .width(Size::Fill)
                    .cross_align(Alignment::End)
                    .main_align(Alignment::SpaceBetween)
                    .padding(Gaps::new_symmetric(0., 8.))
                    .child(
                        rect()
                            .spacing(15.)
                            .horizontal()
                            // TODO: get image if available
                            .maybe_child(None::<Element>)
                            .child(
                                rect()
                                    .spacing(10.)
                                    .vertical()
                                    // TODO: get extra text if available
                                    .maybe_child(None::<Element>)
                                    .child(
                                        label()
                                            .font_weight(FontWeight::BOLD)
                                            .font_size(54.)
                                            .text(self.title.clone()),
                                    ),
                            ),
                    )
                    .child(
                        rect().horizontal().spacing(12.).children([
                            // Left scroll button
                            rect()
                                .rounded_full()
                                .padding(Gaps::new_all(8.))
                                .cross_align(Alignment::Center)
                                .main_align(Alignment::Center)
                                .width(Size::px(40.))
                                .height(Size::px(40.))
                                .border(Some(Border::new().width(2.).fill(if can_scroll_left {
                                    Color::WHITE
                                } else {
                                    Color::from_hex("#404040").unwrap()
                                })))
                                .on_press(move |_| {
                                    if can_scroll_left {
                                        let new_x = (current_x - scroll_amount).max(0);
                                        scroll_controller.scroll_to_x(new_x);
                                    }
                                })
                                .child(
                                    svg(chevron_left())
                                        .color(if can_scroll_left {
                                            Color::WHITE
                                        } else {
                                            Color::from_hex("#404040").unwrap()
                                        })
                                        .width(Size::px(24.))
                                        .height(Size::px(24.)),
                                )
                                .into_element(),
                            // Right scroll button
                            rect()
                                .rounded_full()
                                .padding(Gaps::new_all(8.))
                                .cross_align(Alignment::Center)
                                .main_align(Alignment::Center)
                                .width(Size::px(40.))
                                .height(Size::px(40.))
                                .border(Some(Border::new().width(2.).fill(if can_scroll_right {
                                    Color::WHITE
                                } else {
                                    Color::from_hex("#404040").unwrap()
                                })))
                                .on_press(move |_| {
                                    if can_scroll_right {
                                        let new_x = current_x + scroll_amount;
                                        scroll_controller.scroll_to_x(new_x);
                                    }
                                })
                                .child(
                                    svg(chevron_right())
                                        .color(if can_scroll_right {
                                            Color::WHITE
                                        } else {
                                            Color::from_hex("#404040").unwrap()
                                        })
                                        .width(Size::px(24.))
                                        .height(Size::px(24.)),
                                )
                                .into_element(),
                        ]),
                    ),
            )
            .child(
                ScrollView::new_controlled(scroll_controller)
                    .spacing(20.)
                    .direction(Direction::Horizontal)
                    .width(Size::Fill)
                    .height(Size::Inner)
                    .show_scrollbar(false)
                    .children(
                        self.contents
                            .iter()
                            .map(SongInfo::from)
                            .map(IntoElement::into_element),
                    ),
            )
    }
}
