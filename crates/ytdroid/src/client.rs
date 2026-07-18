/// All known `YouTube` / `YouTube` Music Innertube clients.
///
/// Faithfully ported from Metrolist's `YouTubeClient.kt`.
#[derive(Debug, Clone)]
#[allow(clippy::struct_excessive_bools)] // Each bool is a distinct feature flag; no enum makes sense here.
pub struct YouTubeClient {
    pub client_name: &'static str,
    pub client_version: &'static str,
    /// Numeric ID sent as `X-YouTube-Client-Name` header (NOT the name string).
    pub client_id: &'static str,
    pub user_agent: &'static str,
    /// Extra context.client fields (os, device, sdk, …).
    pub context_extra: ContextExtra,
    /// Whether this client supports cookie + SAPISIDHASH auth.
    pub login_supported: bool,
    /// Whether to include `signatureTimestamp` in player requests.
    pub use_signature_timestamp: bool,
    /// Whether to include `PoToken` in player requests (WEB clients).
    pub use_web_po_tokens: bool,
    /// Whether to send `userAgent` inside `context.client`.
    pub include_user_agent_in_context: bool,
    /// Whether this is an embedded player that can bypass age-gating.
    pub is_embedded: bool,
}

/// Extra fields merged into `context.client` for clients that need them.
#[derive(Debug, Clone, Default)]
pub struct ContextExtra {
    pub os_name: Option<&'static str>,
    pub os_version: Option<&'static str>,
    pub device_make: Option<&'static str>,
    pub device_model: Option<&'static str>,
    pub android_sdk_version: Option<&'static str>,
    pub build_id: Option<&'static str>,
    pub cronet_version: Option<&'static str>,
    pub package_name: Option<&'static str>,
}

const UA_WEB: &str =
    "Mozilla/5.0 (Windows NT 10.0; Win64; x64; rv:140.0) Gecko/20100101 Firefox/140.0";

impl YouTubeClient {
    /// Primary client for all browse / search / library calls.
    pub const WEB_REMIX: Self = Self {
        client_name: "WEB_REMIX",
        client_version: "1.20260114.03.00",
        client_id: "67",
        user_agent: UA_WEB,
        context_extra: ContextExtra {
            os_name: None,
            os_version: None,
            device_make: None,
            device_model: None,
            android_sdk_version: None,
            build_id: None,
            cronet_version: None,
            package_name: None,
        },
        login_supported: true,
        use_signature_timestamp: true,
        use_web_po_tokens: true,
        include_user_agent_in_context: false,
        is_embedded: false,
    };

    /// Generic web client (no auth, no n-transform in this context).
    pub const WEB: Self = Self {
        client_name: "WEB",
        client_version: "2.20260114.08.00",
        client_id: "1",
        user_agent: UA_WEB,
        context_extra: ContextExtra {
            os_name: None,
            os_version: None,
            device_make: None,
            device_model: None,
            android_sdk_version: None,
            build_id: None,
            cronet_version: None,
            package_name: None,
        },
        login_supported: false,
        use_signature_timestamp: false,
        use_web_po_tokens: false,
        include_user_agent_in_context: false,
        is_embedded: false,
    };

    /// Primary player client — no n-transform, SAPISIDHASH auth supported.
    pub const ANDROID: Self = Self {
        client_name: "ANDROID",
        client_version: "21.03.38",
        client_id: "3",
        user_agent: "com.google.android.youtube/21.03.38 (Linux; U; Android 14) gzip",
        context_extra: ContextExtra {
            os_name: None,
            os_version: None,
            device_make: None,
            device_model: None,
            android_sdk_version: None,
            build_id: None,
            cronet_version: None,
            package_name: None,
        },
        login_supported: true,
        use_signature_timestamp: true,
        use_web_po_tokens: false,
        include_user_agent_in_context: false,
        is_embedded: false,
    };

    /// `VisionOS` — no auth, no n-transform, may stop working any time (internal client).
    pub const VISIONOS: Self = Self {
        client_name: "VISIONOS",
        client_version: "0.1",
        client_id: "101",
        user_agent: "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/18.0 Safari/605.1.15",
        context_extra: ContextExtra {
            os_name: Some("visionOS"),
            os_version: Some("1.3.21O771"),
            device_make: Some("Apple"),
            device_model: Some("RealityDevice14,1"),
            android_sdk_version: None,
            build_id: None,
            cronet_version: None,
            package_name: None,
        },
        login_supported: false,
        use_signature_timestamp: false,
        use_web_po_tokens: false,
        include_user_agent_in_context: false,
        is_embedded: false,
    };

    /// Android VR 1.65.10 — no auth, no n-transform, non-adaptive bitrate variant available.
    pub const ANDROID_VR_1_65: Self = Self {
        client_name: "ANDROID_VR",
        client_version: "1.65.10",
        client_id: "28",
        user_agent: "com.google.android.apps.youtube.vr.oculus/1.65.10 (Linux; U; Android 12L; eureka-user Build/SQ3A.220605.009.A1) gzip",
        context_extra: ContextExtra {
            os_name: Some("Android"),
            os_version: Some("12L"),
            device_make: Some("Oculus"),
            device_model: Some("Quest 3"),
            android_sdk_version: Some("32"),
            build_id: None,
            cronet_version: None,
            package_name: None,
        },
        login_supported: false,
        use_signature_timestamp: false,
        use_web_po_tokens: false,
        include_user_agent_in_context: true,
        is_embedded: false,
    };

    /// Android VR 1.43.32 — uses non-adaptive bitrate (fixes audio stuttering on YTM).
    pub const ANDROID_VR_1_43: Self = Self {
        client_name: "ANDROID_VR",
        client_version: "1.43.32",
        client_id: "28",
        user_agent: "com.google.android.apps.youtube.vr.oculus/1.43.32 (Linux; U; Android 12; en_US; Quest 3; Build/SQ3A.220605.009.A1; Cronet/107.0.5284.2)",
        context_extra: ContextExtra {
            os_name: Some("Android"),
            os_version: Some("12"),
            device_make: Some("Oculus"),
            device_model: Some("Quest 3"),
            android_sdk_version: Some("32"),
            build_id: Some("SQ3A.220605.009.A1"),
            cronet_version: Some("107.0.5284.2"),
            package_name: Some("com.google.android.apps.youtube.vr.oculus"),
        },
        login_supported: false,
        use_signature_timestamp: false,
        use_web_po_tokens: false,
        include_user_agent_in_context: true,
        is_embedded: false,
    };

    /// TVHTML5 — login-capable WEB client, needs n-transform + `PoToken` for player.
    pub const TVHTML5: Self = Self {
        client_name: "TVHTML5",
        client_version: "7.20260114.12.00",
        client_id: "7",
        user_agent: "Mozilla/5.0 (ChromiumStylePlatform) Cobalt/25.lts.30.1034943-gold (unlike Gecko), Unknown_TV_Unknown_0/Unknown (Unknown, Unknown)",
        context_extra: ContextExtra {
            os_name: None,
            os_version: None,
            device_make: None,
            device_model: None,
            android_sdk_version: None,
            build_id: None,
            cronet_version: None,
            package_name: None,
        },
        login_supported: true,
        use_signature_timestamp: true,
        use_web_po_tokens: true,
        include_user_agent_in_context: true,
        is_embedded: false,
    };
}

/// Locale sent in `context.client.gl` / `context.client.hl`.
#[derive(Debug, Clone)]
pub struct Locale {
    /// Country code, e.g. `"US"`.
    pub gl: String,
    /// Language tag, e.g. `"en"`.
    pub hl: String,
}

impl Default for Locale {
    fn default() -> Self {
        Self {
            gl: "US".to_owned(),
            hl: "en".to_owned(),
        }
    }
}

pub(crate) const MUSIC_ORIGIN: &str = "https://music.youtube.com";
pub(crate) const MUSIC_REFERER: &str = "https://music.youtube.com/";
pub(crate) const MUSIC_API_BASE: &str = "https://music.youtube.com/youtubei/v1/";
