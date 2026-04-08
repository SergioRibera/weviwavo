use freya::{prelude::*, radio::*};

use crate::components::Section;

mod data;

pub use data::*;

#[derive(Clone)]
pub struct MainApp {
    pub radio: RadioStation<Data, DataChannel>,
}

impl App for MainApp {
    fn render(&self) -> impl IntoElement {
        use_share_radio(move || self.radio);
        use_init_root_theme(|| PreferredTheme::Dark.to_theme());
        let radio = use_radio::<Data, DataChannel>(DataChannel::Feed);
        let radio = radio.read();
        let bg_image = ("bg_image", include_bytes!("../resources/bg_default.webp"));
        let platform = Platform::get().root_size;
        //
        // let on_press = move |_| {
        //     radio.write().lists.push(Vec::default());
        // };

        rect()
            .vertical()
            .expanded()
            .theme_color()
            .background(Color::BLACK)
            .child(
                rect()
                    .child(
                        ImageViewer::new(bg_image).child(
                            rect().expanded().background_linear_gradient(
                                LinearGradient::new().stops([
                                    GradientStop::new(Color::TRANSPARENT, 0.0),
                                    GradientStop::new(Color::BLACK, 80.0),
                                ]),
                            ),
                        ),
                    )
                    .position(
                        Position::new_global()
                            .left(0.)
                            .top(-(platform.read().height * 0.20)),
                    ),
            )
            .child(
                rect().horizontal().children(
                    radio
                        .feed
                        .chips
                        .iter()
                        .map(|c| label().text(c.title.clone()).into_element()),
                ),
            )
            .child(
                rect().expanded().child(
                    ScrollView::new()
                        .expanded()
                        .direction(Direction::Vertical)
                        .spacing(18.)
                        .children(
                            radio
                                .feed
                                .clone()
                                .into_iter()
                                .map(Section::new)
                                .map(IntoElement::into_element),
                        ),
                ),
            )
    }
}
