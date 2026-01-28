use freya::prelude::*;

#[derive(PartialEq)]
pub struct ItemButton {
    icon: Element,
    title: String,
    layout: LayoutData,
}

impl ItemButton {
    pub fn new(icon: impl IntoElement, title: String, layout: LayoutData) -> Self {
        Self {
            icon: icon.into_element(),
            title,
            layout,
        }
    }

    pub fn icon(mut self, icon: impl Into<Element>) -> Self {
        self.icon = icon.into();
        self
    }

    pub fn title(mut self, title: impl Into<String>) -> Self {
        self.title = title.into();
        self
    }

    pub fn layout(mut self, layout: impl Into<LayoutData>) -> Self {
        self.layout = layout.into();
        self
    }
}

impl Component for ItemButton {
    fn render(&self) -> impl IntoElement {
        Button::new().expanded().padding((25., 25.)).child(
            rect()
                .expanded()
                .horizontal()
                .spacing(25.)
                .child(self.icon.clone())
                .child(
                    label()
                        .text(self.title.clone())
                        .text_align(TextAlign::Center),
                ),
        )
    }
}

impl LayoutExt for ItemButton {
    fn get_layout(&mut self) -> &mut LayoutData {
        &mut self.layout
    }
}
