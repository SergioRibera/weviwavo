use std::sync::Arc;

use freya::prelude::*;
use ytmapi_rs::common::YoutubeID;
use ytmapi_rs::parse::ParsedSongArtist;

#[derive(Clone)]
pub struct TextInfo {
    layout: LayoutData,
    align: TextAlign,
    on_click: Arc<dyn Fn(String)>,
    ty: TextInfoType,
}

impl PartialEq for TextInfo {
    fn eq(&self, other: &Self) -> bool {
        self.layout == other.layout && self.ty == other.ty
    }
}

#[derive(Clone, PartialEq)]
pub enum TextInfoType {
    None,
    Plain(String),
    Clickable { text: String, id: String },
    Authors(Vec<ParsedSongArtist>),
}

impl Default for TextInfo {
    fn default() -> Self {
        Self {
            align: TextAlign::Left,
            layout: Default::default(),
            on_click: Arc::new(|_| {}),
            ty: TextInfoType::Plain("".into()),
        }
    }
}

impl TextInfo {
    pub fn none() -> Self {
        Self {
            ty: TextInfoType::None,
            ..Default::default()
        }
    }
    pub fn plain(content: impl Into<String>, align: impl Into<Option<TextAlign>>) -> Self {
        Self {
            align: align.into().unwrap_or(TextAlign::Start),
            ty: TextInfoType::Plain(content.into()),
            ..Default::default()
        }
    }
    pub fn clickable(id: impl Into<String>, content: impl Into<String>) -> Self {
        Self {
            ty: TextInfoType::Clickable {
                id: id.into(),
                text: content.into(),
            },
            ..Default::default()
        }
    }
    pub fn authors(content: impl Into<Vec<ParsedSongArtist>>) -> Self {
        Self {
            ty: TextInfoType::Authors(content.into()),
            ..Default::default()
        }
    }
    pub fn with_on_click(mut self, on_click: Arc<dyn Fn(String)>) -> Self {
        self.on_click = on_click;
        self
    }

    /// Get inline elements for use in a paragraph
    pub fn get_inline_elements(
        &self,
        on_click: impl Fn(String) + 'static + Clone,
    ) -> Vec<Span<'static>> {
        match &self.ty {
            TextInfoType::None => vec![],
            TextInfoType::Plain(txt) => {
                vec![Span::new(txt.clone())]
            }
            TextInfoType::Clickable { text: txt, id } => {
                let id = id.clone();
                vec![
                    Span::new(txt.clone()),
                    // TODO: implement on press
                    // .on_press(move |_| on_click(id.clone()))
                ]
            }
            TextInfoType::Authors(authors) => {
                if authors.is_empty() {
                    return vec![];
                }

                let mut elements = Vec::new();

                for (i, author) in authors.iter().enumerate() {
                    // Add separator
                    if i == authors.len() - 1 && i > 0 {
                        elements.push(Span::new(" y "));
                    } else if i > 0 {
                        elements.push(Span::new(", "));
                    }

                    // Add author name
                    if let Some(id) = &author.id {
                        let id_str = id.get_raw().to_string();
                        let on_click = on_click.clone();
                        elements.push(
                            Span::new(author.name.clone()),
                            // TODO: implement on press
                            // .on_press(move |_| on_click(id_str.clone())),
                        );
                    } else {
                        elements.push(Span::new(author.name.clone()));
                    }
                }

                elements
            }
        }
    }
}

impl From<Vec<ParsedSongArtist>> for TextInfo {
    fn from(value: Vec<ParsedSongArtist>) -> Self {
        Self::authors(value)
    }
}

impl LayoutExt for TextInfo {
    fn get_layout(&mut self) -> &mut LayoutData {
        &mut self.layout
    }
}

impl ContainerExt for TextInfo {}

impl Component for TextInfo {
    fn render(&self) -> impl IntoElement {
        let on_click = self.on_click.clone();
        match &self.ty {
            TextInfoType::None => rect().into_element(),
            TextInfoType::Plain(txt) => label()
                .text(txt.clone())
                .width(Size::Fill)
                .max_lines(2)
                .text_align(self.align)
                .into_element(),
            TextInfoType::Clickable { text: txt, id } => {
                let id = id.clone();
                CursorArea::new()
                    .icon(CursorIcon::Pointer)
                    .child(
                        label()
                            .text(txt.clone())
                            .max_lines(2)
                            .on_press(move |_| on_click(id.clone())),
                    )
                    .into_element()
            }
            TextInfoType::Authors(authors) => {
                if authors.is_empty() {
                    return rect().into_element();
                }

                paragraph()
                    .max_lines(2)
                    .text_align(self.align)
                    .width(Size::Fill)
                    .spans_iter(self.get_inline_elements(move |id| on_click(id)).into_iter())
                    .into_element()
            }
        }
    }
}
