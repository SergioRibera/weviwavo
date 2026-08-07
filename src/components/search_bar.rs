use freya::icons::lucide::{clock, search, trash_2, x};
use freya::prelude::*;

const HISTORY: &[&str] = &[
    "me salvaste estoy tan enamorado",
    "tão apaixonado que já não me lembro do meu passa...",
    "tan enamorado upperroom",
    "ahora estoy tan enamorado que ya no recuerdo mi p...",
    "me salvaste",
    "gilberto daza",
    "gracias musiko",
];

#[derive(PartialEq)]
pub struct SearchBar;

impl Component for SearchBar {
    fn render(&self) -> impl IntoElement {
        let mut query = use_state(String::new);
        let mut show_dropdown = use_state(|| false);

        let is_empty = query.read().is_empty();
        let show = *show_dropdown.read();

        rect()
            .vertical()
            .width(Size::Fill)
            // Fullscreen dismiss overlay (rendered before children → behind them)
            .maybe_child(show.then_some(
                rect()
                    .position(Position::new_absolute().top(0.).left(0.))
                    .width(Size::window_percent(100.))
                    .height(Size::window_percent(100.))
                    .on_press(move |_| show_dropdown.set(false)),
            ))
            // Input row
            .child(
                rect()
                    .horizontal()
                    .width(Size::Fill)
                    .height(Size::px(44.))
                    .corner_radius(22.)
                    .background(Color::from_hex("#1E1E1E").unwrap())
                    .cross_align(Alignment::Center)
                    .padding(Gaps::new_symmetric(0., 14.))
                    .spacing(8.)
                    .on_press(move |_| show_dropdown.set(true))
                    .child(
                        SvgViewer::new(search())
                            .color(Color::from_hex("#888888").unwrap())
                            .fill(Color::from_hex("#888888").unwrap())
                            .width(Size::px(16.))
                            .height(Size::px(16.)),
                    )
                    .child(
                        Input::new(query)
                            .flat()
                            .expanded()
                            .placeholder("Buscar canciones, álbumes, artistas o pódcasts")
                            .on_submit(move |_| show_dropdown.set(false))
                            .on_pre_key_down(Callback::new(move |e: Event<KeyboardEventData>| {
                                if e.key == Key::Named(NamedKey::Escape) {
                                    show_dropdown.set(false);
                                }
                                match &e.key {
                                    Key::Named(NamedKey::Enter)
                                    | Key::Named(NamedKey::Escape)
                                    | Key::Named(NamedKey::Shift) => true,
                                    Key::Named(NamedKey::Tab) => false,
                                    _ => {
                                        e.stop_propagation();
                                        e.prevent_default();
                                        true
                                    }
                                }
                            })),
                    )
                    .maybe_child((!is_empty).then_some(
                        rect()
                            .center()
                            .width(Size::px(28.))
                            .height(Size::px(28.))
                            .rounded_full()
                            .on_press(move |e: Event<PressEventData>| {
                                e.stop_propagation();
                                query.set(String::new());
                            })
                            .on_pointer_enter(|_| Cursor::set(CursorIcon::Pointer))
                            .on_pointer_leave(|_| Cursor::set(CursorIcon::Default))
                            .child(
                                SvgViewer::new(x())
                                    .color(Color::WHITE)
                                    .fill(Color::WHITE)
                                    .width(Size::px(14.))
                                    .height(Size::px(14.)),
                            ),
                    )),
            )
            // Dropdown
            .maybe_child(show.then_some(
                rect()
                    .position(Position::new_absolute().top(50.))
                    .width(Size::Fill)
                    .corner_radius(8.)
                    .background(Color::from_hex("#1A1A1A").unwrap())
                    .border(
                        Border::new()
                            .fill(Color::from_hex("#333333").unwrap())
                            .width(1.)
                            .alignment(BorderAlignment::Outer),
                    )
                    .vertical()
                    .padding((8., 0.))
                    .children(HISTORY.iter().map(|&item| {
                        rect()
                            .horizontal()
                            .cross_align(Alignment::Center)
                            .padding(Gaps::new_symmetric(12., 16.))
                            .spacing(12.)
                            .width(Size::Fill)
                            .on_press(move |e: Event<PressEventData>| e.stop_propagation())
                            .on_pointer_enter(|_| Cursor::set(CursorIcon::Pointer))
                            .on_pointer_leave(|_| Cursor::set(CursorIcon::Default))
                            .child(
                                SvgViewer::new(clock())
                                    .color(Color::from_hex("#888888").unwrap())
                                    .fill(Color::from_hex("#888888").unwrap())
                                    .width(Size::px(16.))
                                    .height(Size::px(16.)),
                            )
                            .child(
                                label()
                                    .max_lines(1)
                                    .text_overflow(TextOverflow::Ellipsis)
                                    .font_size(14.)
                                    .color(Color::WHITE)
                                    .text(item),
                            )
                            .child(rect().width(Size::flex(1.0)))
                            .child(
                                SvgViewer::new(trash_2())
                                    .color(Color::from_hex("#555555").unwrap())
                                    .fill(Color::from_hex("#555555").unwrap())
                                    .width(Size::px(16.))
                                    .height(Size::px(16.)),
                            )
                            .into_element()
                    })),
            ))
    }
}
