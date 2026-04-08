use freya::{prelude::*, radio::*};

use crate::components::Section;

mod data;

pub use data::*;

const LIGHT_SIZE: f32 = 1024.;
const LIGHT_SIZE_HALF: f32 = LIGHT_SIZE / 2.;
const COLORS: &[&str] = &["#081F22", "#1B1F15", "#000000AA", "#081F22"];

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

        // let on_press = move |_| {
        //     radio.write().lists.push(Vec::default());
        // };

        rect()
            .vertical()
            .expanded()
            .theme_color()
            .background(Color::BLACK)
            .child(rect().horizontal().children((0..=5).map(|i| {
                let i = i as usize % COLORS.len();
                rect()
                    .width(Size::px(LIGHT_SIZE))
                    .height(Size::px(LIGHT_SIZE))
                    .position(
                        Position::new_global()
                            .left(-(LIGHT_SIZE_HALF) + (i as f32 * (LIGHT_SIZE_HALF)))
                            .top(-(LIGHT_SIZE_HALF)),
                    )
                    .background_radial_gradient(RadialGradient::new().stops([
                        GradientStop::new(Color::from_hex(COLORS[i]).unwrap(), 0.),
                        GradientStop::new(Color::from_hex(COLORS[i]).unwrap().with_a(0), 100.),
                    ]))
                    .into_element()
            })))
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
