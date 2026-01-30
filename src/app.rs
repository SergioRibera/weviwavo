use freya::prelude::*;
use freya_radio::prelude::*;

use crate::components::song;

pub fn init() -> impl IntoElement {
    use_init_root_theme(|| PreferredTheme::Dark.to_theme());
    // use_init_radio_station::<Data, DataChannel>(Data::default);
    // let mut radio = use_radio::<Data, DataChannel>(DataChannel::ListCreation);

    // let on_press = move |_| {
    //     radio.write().lists.push(Vec::default());
    // };

    rect()
        .center()
        .vertical()
        .expanded()
        .theme_color()
        .theme_background()
        .child(song(Default::default()))
}
