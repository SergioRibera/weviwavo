use freya::icons::lucide::{clock, search, trash_2, x};
use freya::prelude::*;

const INITIAL_HISTORY: &[&str] = &[
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
        let history = use_state(|| {
            INITIAL_HISTORY
                .iter()
                .map(|s| s.to_string())
                .collect::<Vec<_>>()
        });

        let focus = use_a11y();
        let focus_status = use_focus(focus);
        let is_focused = focus_status().is_focused();

        let is_empty = query.read().is_empty();
        let show = *show_dropdown.read();
        let query_val = query.read().clone();

        let border_color = if is_focused {
            Color::WHITE.with_a(160)
        } else {
            Color::WHITE.with_a(38)
        };
        let border_width = if is_focused { 1.5 } else { 1. };

        let history_items = history.read().clone();
        let filtered: Vec<String> = if query_val.is_empty() {
            history_items
        } else {
            history_items
                .into_iter()
                .filter(|h| h.to_lowercase().contains(&query_val.to_lowercase()))
                .collect()
        };
        let show_history = show && !filtered.is_empty();

        rect()
            .vertical()
            .width(Size::Fill)
            // Fullscreen dismiss overlay
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
                    .corner_radius(8.)
                    .content(Content::Flex)
                    .background(Color::WHITE.with_a(38))
                    .cross_align(Alignment::Center)
                    .padding(Gaps::new_symmetric(0., 14.))
                    .spacing(8.)
                    .border(Border::new().width(border_width).fill(border_color))
                    .on_press(move |_| show_dropdown.set(true))
                    .child(
                        SvgViewer::new(search())
                            .color(Color::from_hex("#888888").unwrap())
                            .width(Size::px(16.))
                            .height(Size::px(16.)),
                    )
                    .child(
                        Input::new(query)
                            .a11y_id(focus)
                            .flat()
                            .expanded()
                            .width(Size::flex(1.))
                            .theme_colors(InputColorsThemePartial {
                                border_fill: Some(Color::TRANSPARENT.into()),
                                focus_border_fill: Some(Color::TRANSPARENT.into()),
                                background: Some(Color::TRANSPARENT.into()),
                                focus_background: Some(Color::TRANSPARENT.into()),
                                ..Default::default()
                            })
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
                                    .width(Size::px(14.))
                                    .height(Size::px(14.)),
                            ),
                    )),
            )
            // Dropdown
            .maybe_child(show_history.then_some(
                rect()
                    .position(Position::new_absolute().top(50.))
                    .width(Size::Fill)
                    .corner_radius(8.)
                    .layer(Layer::Overlay)
                    .background(Color::from_hex("#0d0d0d").unwrap())
                    .border(
                        Border::new()
                            .fill(Color::WHITE.with_a(28))
                            .width(1.)
                            .alignment(BorderAlignment::Outer),
                    )
                    .vertical()
                    .padding((6., 6.))
                    .children(filtered.into_iter().map(|item| {
                        HistoryItem {
                            text: item,
                            query,
                            show_dropdown,
                            history,
                        }
                        .into_element()
                    })),
            ))
    }
}

#[derive(Clone, PartialEq)]
struct HistoryItem {
    text: String,
    query: State<String>,
    show_dropdown: State<bool>,
    history: State<Vec<String>>,
}

impl Component for HistoryItem {
    fn render(&self) -> impl IntoElement {
        let mut is_hover = use_state(|| false);
        let text = self.text.clone();
        let text_for_delete = text.clone();
        let mut query = self.query;
        let mut show_dropdown = self.show_dropdown;
        let mut history = self.history;

        rect()
            .horizontal()
            .cross_align(Alignment::Center)
            .padding(Gaps::new_symmetric(10., 10.))
            .spacing(10.)
            .width(Size::Fill)
            .content(Content::Flex)
            .corner_radius(6.)
            .background(if is_hover() {
                Color::WHITE.with_a(12)
            } else {
                Color::TRANSPARENT
            })
            .on_pointer_enter(move |_| {
                is_hover.set(true);
                Cursor::set(CursorIcon::Pointer);
            })
            .on_pointer_leave(move |_| {
                is_hover.set(false);
                Cursor::set(CursorIcon::Default);
            })
            .on_press(move |_| {
                query.set(text.clone());
                show_dropdown.set(false);
            })
            .child(
                SvgViewer::new(clock())
                    .color(Color::from_hex("#555555").unwrap())
                    .width(Size::px(14.))
                    .height(Size::px(14.)),
            )
            .child(
                label()
                    .max_lines(1)
                    .width(Size::flex(1.))
                    .text_overflow(TextOverflow::Ellipsis)
                    .font_size(14.)
                    .color(Color::WHITE)
                    .text(self.text.clone()),
            )
            .child(
                rect()
                    .center()
                    .width(Size::px(24.))
                    .height(Size::px(24.))
                    .corner_radius(4.)
                    .background(if is_hover() {
                        Color::WHITE.with_a(18)
                    } else {
                        Color::TRANSPARENT
                    })
                    .on_press(move |e: Event<PressEventData>| {
                        e.stop_propagation();
                        let new: Vec<String> = history
                            .read()
                            .iter()
                            .filter(|h| h.as_str() != text_for_delete.as_str())
                            .cloned()
                            .collect();
                        history.set(new);
                    })
                    .on_pointer_enter(|_| Cursor::set(CursorIcon::Pointer))
                    .on_pointer_leave(|_| Cursor::set(CursorIcon::Default))
                    .child(
                        SvgViewer::new(trash_2())
                            .color(if is_hover() {
                                Color::from_hex("#999999").unwrap()
                            } else {
                                Color::from_hex("#444444").unwrap()
                            })
                            .width(Size::px(13.))
                            .height(Size::px(13.)),
                    ),
            )
    }
}
