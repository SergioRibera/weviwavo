use freya::radio::RadioChannel;
use ytmapi_rs::YtMusic;
use ytmapi_rs::auth::BrowserToken;
use ytmapi_rs::parse::HomeSections;

#[derive(Default, Clone)]
pub struct Data {
    pub yt_session: Option<YtMusic<BrowserToken>>,
    pub feed: HomeSections,
}

#[derive(Default, PartialEq, Eq, Clone, Debug, Copy, Hash, PartialOrd, Ord)]
pub enum DataChannel {
    #[default]
    YtApi,
    Feed,
}

impl RadioChannel<Data> for DataChannel {}
