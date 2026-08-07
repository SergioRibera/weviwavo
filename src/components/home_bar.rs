use std::cell::RefCell;
use std::rc::Rc;

use freya::icons::lucide::{chevron_left, menu};
use freya::prelude::*;
use freya::router::RouterContext;

use super::SearchBar;

pub struct HomeBar {
    /// When Some, show the hamburger button that calls this callback.
    pub on_menu: Option<Rc<RefCell<dyn FnMut()>>>,
    /// When true, show a back chevron that calls router.go_back().
    pub show_back_btn: bool,
}

impl PartialEq for HomeBar {
    fn eq(&self, other: &Self) -> bool {
        self.show_back_btn == other.show_back_btn
            && self.on_menu.is_some() == other.on_menu.is_some()
    }
}

impl Component for HomeBar {
    fn render(&self) -> impl IntoElement {
        let router = RouterContext::get();
        let on_menu = self.on_menu.clone();
        let show_back = self.show_back_btn;

        rect()
            .horizontal()
            .width(Size::Fill)
            .height(Size::px(64.))
            .padding(Gaps::new_symmetric(0., 20.))
            .cross_align(Alignment::Center)
            .spacing(16.)
            .background(Color::TRANSPARENT)
            .blur(25.)
            .border(
                Border::new()
                    .alignment(BorderAlignment::Outer)
                    .width(1.)
                    .fill(Color::from_hex("#292929").unwrap()),
            )
            .maybe_child(on_menu.map(|cb| {
                rect()
                    .center()
                    .width(Size::px(36.))
                    .height(Size::px(36.))
                    .rounded_full()
                    .on_press(move |_| (cb.borrow_mut())())
                    .on_pointer_enter(|_| Cursor::set(CursorIcon::Pointer))
                    .on_pointer_leave(|_| Cursor::set(CursorIcon::Default))
                    .child(
                        SvgViewer::new(menu())
                            .color(Color::WHITE)
                            .width(Size::px(20.))
                            .height(Size::px(20.)),
                    )
            }))
            .maybe_child(
                show_back.then_some(
                    rect()
                        .center()
                        .width(Size::px(36.))
                        .height(Size::px(36.))
                        .rounded_full()
                        .on_press(move |_| router.go_back())
                        .on_pointer_enter(|_| Cursor::set(CursorIcon::Pointer))
                        .on_pointer_leave(|_| Cursor::set(CursorIcon::Default))
                        .child(
                            SvgViewer::new(chevron_left())
                                .color(Color::WHITE)
                                .width(Size::px(20.))
                                .height(Size::px(20.)),
                        ),
                ),
            )
            .child(SearchBar)
    }
}
