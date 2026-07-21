/// All known `YouTube` / `YouTube` Music Innertube clients.
///
/// Faithfully ported from Metrolist's `YouTubeClient.kt`.
/// All clients send requests to `music.youtube.com`; Metrolist hardcodes this
/// base URL regardless of the client type.
#[derive(Debug, Clone)]
#[allow(clippy::struct_excessive_bools)]
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
    /// Whether this client won't work at all without login (e.g. WEB_CREATOR).
    pub login_required: bool,
    /// Whether to include `signatureTimestamp` in player requests.
    pub use_signature_timestamp: bool,
    /// Whether to include `PoToken` in player requests (WEB clients).
    pub use_web_po_tokens: bool,
    /// Whether a PoToken is mandatory — skip this client if none is available.
    pub require_po_token: bool,
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

const UA_TVHTML5: &str =
    "Mozilla/5.0 (ChromiumStylePlatform) Cobalt/25.lts.30.1034943-gold (unlike Gecko), Unknown_TV_Unknown_0/Unknown (Unknown, Unknown)";

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
        login_required: false,
        use_signature_timestamp: true,
        use_web_po_tokens: true,
        require_po_token: false,
        include_user_agent_in_context: false,
        is_embedded: false,
    };

    /// Generic web client.
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
        login_required: false,
        use_signature_timestamp: false,
        use_web_po_tokens: false,
        require_po_token: false,
        include_user_agent_in_context: false,
        is_embedded: false,
    };

    /// YouTube Studio web client — login-required, used for uploaded tracks.
    pub const WEB_CREATOR: Self = Self {
        client_name: "WEB_CREATOR",
        client_version: "1.20260114.05.00",
        client_id: "62",
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
        login_required: true,
        use_signature_timestamp: true,
        use_web_po_tokens: true,
        require_po_token: false,
        include_user_agent_in_context: false,
        is_embedded: false,
    };

    /// Android YouTube client — login-capable (SAPISIDHASH + cookies), matching Metrolist's MOBILE.
    pub const MOBILE: Self = Self {
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
        login_required: false,
        use_signature_timestamp: true,
        use_web_po_tokens: false,
        require_po_token: false,
        include_user_agent_in_context: false,
        is_embedded: false,
    };

    /// Android VR — no SAPISIDHASH, cookie-only auth.
    pub const ANDROID_VR_NO_AUTH: Self = Self {
        client_name: "ANDROID_VR",
        client_version: "1.61.48",
        client_id: "28",
        user_agent: "com.google.android.apps.youtube.vr.oculus/1.61.48 (Linux; U; Android 12; en_US; Oculus Quest 3; Build/SQ3A.220605.009.A1; Cronet/132.0.6808.3)",
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
        login_required: false,
        use_signature_timestamp: false,
        use_web_po_tokens: false,
        require_po_token: false,
        include_user_agent_in_context: true,
        is_embedded: false,
    };

    /// Android VR 1.65.10 — cookie-only auth, non-adaptive bitrate available.
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
        login_required: false,
        use_signature_timestamp: false,
        use_web_po_tokens: false,
        require_po_token: false,
        include_user_agent_in_context: true,
        is_embedded: false,
    };

    /// Android VR 1.43.32 — cookie-only auth, non-adaptive bitrate (fixes audio stuttering on YTM).
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
        login_required: false,
        use_signature_timestamp: false,
        use_web_po_tokens: false,
        require_po_token: false,
        include_user_agent_in_context: true,
        is_embedded: false,
    };

    /// VisionOS — cookie-only auth. May stop working at any time.
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
        login_required: false,
        use_signature_timestamp: false,
        use_web_po_tokens: false,
        require_po_token: false,
        include_user_agent_in_context: false,
        is_embedded: false,
    };

    /// TVHTML5 — login-capable, needs sig + nsig + PoToken for full reliability.
    pub const TVHTML5: Self = Self {
        client_name: "TVHTML5",
        client_version: "7.20260114.12.00",
        client_id: "7",
        user_agent: UA_TVHTML5,
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
        login_required: false,
        use_signature_timestamp: true,
        use_web_po_tokens: true,
        require_po_token: false,
        include_user_agent_in_context: true,
        is_embedded: false,
    };

    /// TVHTML5_SIMPLY — requires PoToken, skipped when none is available.
    pub const TVHTML5_SIMPLY: Self = Self {
        client_name: "TVHTML5_SIMPLY",
        client_version: "1.0",
        client_id: "75",
        user_agent: UA_TVHTML5,
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
        login_required: false,
        use_signature_timestamp: true,
        use_web_po_tokens: true,
        require_po_token: true,
        include_user_agent_in_context: false,
        is_embedded: false,
    };

    /// Embedded player client — Metrolist explicitly marks "PoToken not required".
    /// Uses PS4 UA, login-capable, embedded context (reddit embedUrl).
    pub const TV_EMBEDDED: Self = Self {
        client_name: "TVHTML5_SIMPLY_EMBEDDED_PLAYER",
        client_version: "2.0",
        client_id: "85",
        user_agent: "Mozilla/5.0 (PlayStation; PlayStation 4/12.02) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/15.4 Safari/605.1.15",
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
        login_required: false,
        use_signature_timestamp: true,
        use_web_po_tokens: false,
        require_po_token: false,
        include_user_agent_in_context: false,
        is_embedded: true,
    };

    /// Android YouTube Music client — used by yt-dlp on Linux for YTM without PoToken.
    /// clientId=21, cookie-only auth, returns direct stream URLs.
    pub const ANDROID_MUSIC: Self = Self {
        client_name: "ANDROID_MUSIC",
        client_version: "7.27.0",
        client_id: "21",
        user_agent: "com.google.android.apps.youtube.music/7.27.0 (Linux; U; Android 12; en_US; Google Pixel 6; Build/SP2A.220305.013.A3; Cronet/102.0.5005.61) gzip",
        context_extra: ContextExtra {
            os_name: Some("Android"),
            os_version: Some("12"),
            device_make: Some("Google"),
            device_model: Some("Pixel 6"),
            android_sdk_version: Some("31"),
            build_id: Some("SP2A.220305.013.A3"),
            cronet_version: Some("102.0.5005.61"),
            package_name: Some("com.google.android.apps.youtube.music"),
        },
        login_supported: false,
        login_required: false,
        use_signature_timestamp: false,
        use_web_po_tokens: false,
        require_po_token: false,
        include_user_agent_in_context: true,
        is_embedded: false,
    };

    /// Android test suite client — widely used (yt-dlp, NewPipe) to bypass PoToken requirements.
    /// Returns direct stream URLs. YouTube doesn't enforce PoToken for this testing client.
    pub const ANDROID_TESTSUITE: Self = Self {
        client_name: "ANDROID_TESTSUITE",
        client_version: "1.9",
        client_id: "30",
        user_agent: "com.google.android.youtube/1.9 (Linux; U; Android 4.0.3;)",
        context_extra: ContextExtra {
            os_name: Some("Android"),
            os_version: Some("4.0.3"),
            device_make: None,
            device_model: None,
            android_sdk_version: Some("15"),
            build_id: None,
            cronet_version: None,
            package_name: None,
        },
        login_supported: false,
        login_required: false,
        use_signature_timestamp: false,
        use_web_po_tokens: false,
        require_po_token: false,
        include_user_agent_in_context: true,
        is_embedded: false,
    };

    /// iOS YouTube app — cookie-only auth, direct stream URLs, no PoToken required.
    pub const IOS: Self = Self {
        client_name: "IOS",
        client_version: "19.45.4",
        client_id: "5",
        user_agent: "com.google.ios.youtube/19.45.4 (iPhone16,2; U; CPU iOS 18_1_0 like Mac OS X;)",
        context_extra: ContextExtra {
            os_name: Some("iOS"),
            os_version: Some("18.1.0.22B83"),
            device_make: Some("Apple"),
            device_model: Some("iPhone16,2"),
            android_sdk_version: None,
            build_id: None,
            cronet_version: None,
            package_name: None,
        },
        login_supported: false,
        login_required: false,
        use_signature_timestamp: false,
        use_web_po_tokens: false,
        require_po_token: false,
        include_user_agent_in_context: true,
        is_embedded: false,
    };

    /// iOS YouTube Music app — cookie-only auth, direct stream URLs, no PoToken required.
    pub const IOS_MUSIC: Self = Self {
        client_name: "IOS_MUSIC",
        client_version: "7.27.0",
        client_id: "26",
        user_agent: "com.google.ios.youtubemusic/7.27.0 (iPhone16,2; U; CPU iOS 18_1_0 like Mac OS X;)",
        context_extra: ContextExtra {
            os_name: Some("iOS"),
            os_version: Some("18.1.0.22B83"),
            device_make: Some("Apple"),
            device_model: Some("iPhone16,2"),
            android_sdk_version: None,
            build_id: None,
            cronet_version: None,
            package_name: None,
        },
        login_supported: false,
        login_required: false,
        use_signature_timestamp: false,
        use_web_po_tokens: false,
        require_po_token: false,
        include_user_agent_in_context: true,
        is_embedded: false,
    };

    /// Android Creator — login-capable, plays most content including kids/uploaded.
    pub const ANDROID_CREATOR: Self = Self {
        client_name: "ANDROID_CREATOR",
        client_version: "25.03.101",
        client_id: "14",
        user_agent: "com.google.android.apps.youtube.creator/25.03.101 (Linux; U; Android 15; en_US; Pixel 9 Pro Fold; Build/AP3A.241005.015.A2; Cronet/132.0.6779.0)",
        context_extra: ContextExtra {
            os_name: Some("Android"),
            os_version: Some("15"),
            device_make: Some("Google"),
            device_model: Some("Pixel 9 Pro Fold"),
            android_sdk_version: Some("35"),
            build_id: Some("AP3A.241005.015.A2"),
            cronet_version: Some("132.0.6779.0"),
            package_name: Some("com.google.android.apps.youtube.creator"),
        },
        login_supported: true,
        login_required: false,
        use_signature_timestamp: true,
        use_web_po_tokens: false,
        require_po_token: false,
        include_user_agent_in_context: false,
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
