use freya::prelude::*;
use freya_radio::prelude::*;

use crate::components::song;

#[derive(Default)]
struct Data {
    pub lists: Vec<Vec<String>>,
}

#[derive(PartialEq, Eq, Clone, Debug, Copy, Hash, PartialOrd, Ord)]
pub enum DataChannel {
    ListCreation,
    SpecificListItemUpdate(usize),
}

impl RadioChannel<Data> for DataChannel {}

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
        .child(song())
}

#[derive(PartialEq)]
struct ListComp(usize);
impl Component for ListComp {
    fn render(&self) -> impl IntoElement {
        let list_n = self.0;
        let mut radio = use_radio::<Data, DataChannel>(DataChannel::SpecificListItemUpdate(list_n));

        println!("Running DataChannel::SpecificListItemUpdate({list_n})");

        rect()
            .child(
                Button::new()
                    .on_press(move |_| radio.write().lists[list_n].push("Hello, World".to_string()))
                    .child("New Item"),
            )
            .children(
                radio.read().lists[list_n]
                    .iter()
                    .enumerate()
                    .map(move |(i, item)| label().key(i).text(item.clone()).into()),
            )
    }
}
