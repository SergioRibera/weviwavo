use ytmapi_rs::YtMusic;
use ytmapi_rs::auth::AuthToken;

pub trait ExtYtAPI {}

impl<A: AuthToken> ExtYtAPI for YtMusic<A> {}
