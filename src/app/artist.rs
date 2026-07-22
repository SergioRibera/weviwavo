use freya::prelude::*;
use freya::router::RouterContext;

/// Stub page for `/artist/:id`.
#[derive(Clone, PartialEq)]
pub struct Artist {
    pub id: String,
}

impl Component for Artist {
    fn render(&self) -> impl IntoElement {
        let router = RouterContext::get();

        rect()
            .vertical()
            .expanded()
            .spacing(16.)
            .padding(Gaps::new_all(24.))
            .child(
                rect()
                    .horizontal()
                    .width(Size::Fill)
                    .child(
                        rect()
                            .padding(Gaps::new_symmetric(8., 12.))
                            .corner_radius(8.)
                            .on_press(move |_| {
                                router.go_back();
                            })
                            .on_pointer_enter(|_| Cursor::set(CursorIcon::Pointer))
                            .on_pointer_leave(|_| Cursor::set(CursorIcon::Default))
                            .child(
                                label()
                                    .text("← Volver")
                                    .color(Color::WHITE)
                                    .font_size(14.),
                            ),
                    ),
            )
            .child(
                label()
                    .text(format!("Artist: {}", self.id))
                    .color(Color::WHITE)
                    .font_size(20.),
            )
    }
}
