use freya::prelude::*;

/// Dialog base component that compone el `Popup` (freya-components) y ofrece
/// una API simple para título, contenido y botones de acción.
///
/// Ejemplo de uso:
/// ```rust
/// Dialog::new("Mi diálogo")
///     .width(Size::px(640.))
///     .show_close_button(true)
///     .on_close_request(|| println!("Cerrado"))
///     .child(label().text("Contenido del diálogo"))
///     .action(
///         button()
///             .compact()
///             .on_press(|_| println!("Aceptar"))
///             .child(label().text("Aceptar")),
///     )
///     .into_element()
/// ```
#[derive(Clone, PartialEq)]
pub struct Dialog {
    title: ReadState<String>,
    children: Vec<Element>,
    actions: Vec<Element>,
    on_close_request: Option<EventHandler<()>>,
    width: Size,
    key: DiffKey,
}

impl KeyExt for Dialog {
    fn write_key(&mut self) -> &mut DiffKey {
        &mut self.key
    }
}

impl Dialog {
    pub fn new(title: impl Into<ReadState<String>>) -> Self {
        Self {
            title: title.into(),
            children: vec![],
            actions: vec![],
            on_close_request: None,
            width: Size::px(500.),
            key: DiffKey::None,
        }
    }

    pub fn width(mut self, w: impl Into<Size>) -> Self {
        self.width = w.into();
        self
    }

    pub fn on_close_request(mut self, handler: impl Into<EventHandler<()>>) -> Self {
        self.on_close_request = Some(handler.into());
        self
    }

    pub fn child(mut self, e: impl Into<Element>) -> Self {
        self.children.push(e.into());
        self
    }

    pub fn action(mut self, e: impl Into<Element>) -> Self {
        self.actions.push(e.into());
        self
    }
}

impl ChildrenExt for Dialog {
    fn get_children(&mut self) -> &mut Vec<Element> {
        &mut self.children
    }
}

impl Component for Dialog {
    fn render(&self) -> impl IntoElement {
        let close_handler = self.on_close_request.clone();

        let invoke_close = move || {
            if let Some(h) = &close_handler {
                h.call(());
            }
        };

        Popup::new()
            .on_close_request({
                let invoke_close = invoke_close.clone();
                move |()| {
                    invoke_close();
                }
            })
            .child(
                rect()
                    .spacing(8.)
                    .padding(8.)
                    .width(Size::Fill)
                    .horizontal()
                    .main_align(Alignment::SpaceBetween)
                    .cross_align(Alignment::Center)
                    .child(
                        rect().font_size(18.).child(
                            label()
                                .a11y_role(AccessibilityRole::TitleBar)
                                .width(Size::fill())
                                .text(self.title.read().to_string()),
                        ),
                    )
                    .child(
                        Button::new()
                            .compact()
                            .on_press(move |_| {
                                invoke_close();
                            })
                            .child(label().text("✕")),
                    )
                    .into_element(),
            )
            .child(
                PopupContent::new().child(
                    rect()
                        .vertical()
                        .spacing(8.)
                        .padding(8.)
                        .children(self.children.clone()),
                ),
            )
            .child(PopupButtons::new().children(self.actions.clone()))
    }

    fn render_key(&self) -> DiffKey {
        self.key.clone().or(self.default_key())
    }
}
