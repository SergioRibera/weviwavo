use freya::{prelude::*, radio::*};

use crate::components::Section;

mod data;

pub use data::*;

pub struct MainApp {
    pub radio: RadioStation<Data, DataChannel>,
}

impl Into<AppComponent> for MainApp {
    fn into(self) -> AppComponent {
        AppComponent::new(self)
    }
}

impl Component for MainApp {
    fn render(&self) -> impl IntoElement {
        use_share_radio(move || self.radio);
        use_init_root_theme(|| PreferredTheme::Dark.to_theme());
        let radio = use_radio::<Data, DataChannel>(DataChannel::Feed);

        // let on_press = move |_| {
        //     radio.write().lists.push(Vec::default());
        // };

        // https://accounts.google.com/v3/signin/identifier?continue=https%3A%2F%2Fwww.youtube.com%2Fsignin%3Faction_handle_signin%3Dtrue%26app%3Ddesktop%26hl%3Den%26next%3Dhttps%253A%252F%252Fmusic.youtube.com%252F%26feature%3D__FEATURE__&dsh=S-332057247%3A1769407389042975&hl=en&ifkv=AXbMIuBcVMXNf7v5tojo4FavD_6iFXGCjqj3iUhQOJwznaQ75Q4GaUS5mvFxOobbGpOODtpGwZza&ltmpl=music&passive=true&service=youtube&uilel=3&flowName=GlifWebSignIn&flowEntry=ServiceLogin

        rect()
            .center()
            .vertical()
            .expanded()
            .theme_color()
            .theme_background()
            .child(
                ScrollView::new()
                    .expanded()
                    .direction(Direction::Vertical)
                    .spacing(10.)
                    .children(
                        radio
                            .read()
                            .feed
                            .clone()
                            .into_iter()
                            .map(Section::new)
                            .map(IntoElement::into_element),
                    ),
            )
    }
}
