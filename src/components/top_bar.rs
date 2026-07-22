use freya::icons::lucide::chevron_left;
use freya::prelude::*;
use freya::router::RouterContext;

/// Standalone top navigation bar. Contains the back button on the left;
/// the right side is reserved for future page-level actions.
#[derive(Clone, PartialEq)]
pub struct TopBar;

impl Component for TopBar {
    fn render(&self) -> impl IntoElement {
        let router = RouterContext::get();

        rect()
            .horizontal()
            .width(Size::window_percent(100.))
            .height(Size::px(52.))
            .padding(Gaps::new_symmetric(0., 16.))
            .cross_align(Alignment::Center)
            .background(Color::from_hex("#0D0D0D").unwrap())
            .child(
                rect()
                    .horizontal()
                    .cross_align(Alignment::Center)
                    .spacing(4.)
                    .padding(Gaps::new_symmetric(6., 10.))
                    .corner_radius(8.)
                    .on_pointer_enter(|_| Cursor::set(CursorIcon::Pointer))
                    .on_pointer_leave(|_| Cursor::set(CursorIcon::Default))
                    .on_press(move |_| router.go_back())
                    .child(
                        SvgViewer::new(chevron_left())
                            .color(Color::WHITE)
                            .width(Size::px(18.))
                            .height(Size::px(18.)),
                    )
                    .child(
                        label()
                            .text("Volver")
                            .color(Color::WHITE)
                            .font_size(14.),
                    ),
            )
    }
}
