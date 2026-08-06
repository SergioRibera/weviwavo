//! YouTube / Google OAuth2 device-flow — mirrors Metrolist's `OAuthManager`.
//!
//! Uses the publicly-known YouTube Android app OAuth2 credentials (same as
//! yt-dlp, NewPipe, Metrolist). The device flow lets desktop / TV apps
//! authorize without embedding a browser: the user visits a short URL, enters
//! a code, and the polling loop picks up the resulting token pair.

use std::time::Duration;

const CLIENT_ID: &str =
    "861556708454-d6dlm3lh05idd8npek18k6be8ba3oc68.apps.googleusercontent.com";
const CLIENT_SECRET: &str = "SboVhoG9s0rNafixCSGGKXAT";
const SCOPE: &str = "https://www.googleapis.com/auth/youtube";
const DEVICE_CODE_URL: &str = "https://oauth2.googleapis.com/device/code";
const TOKEN_URL: &str = "https://oauth2.googleapis.com/token";

/// Response from the device-code initiation endpoint.
#[derive(Debug, Clone, serde::Deserialize)]
pub struct DeviceCodeResponse {
    pub device_code: String,
    /// Short code the user types at `verification_url`.
    pub user_code: String,
    /// URL the user visits (usually <https://www.google.com/device>).
    pub verification_url: String,
    pub expires_in: u64,
    /// Minimum polling interval in seconds.
    pub interval: u64,
}

/// Full token pair returned on successful device-flow authorization.
#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
pub struct OAuthToken {
    pub access_token: String,
    /// Present only on the initial grant — absent on refresh responses.
    #[serde(default)]
    pub refresh_token: String,
    pub expires_in: u64,
}

#[derive(Debug, thiserror::Error)]
pub enum OAuthError {
    #[error("HTTP: {0}")]
    Http(#[from] reqwest::Error),
    #[error("device code expired — restart flow")]
    Expired,
    #[error("access denied by user")]
    Denied,
    #[error("OAuth API error: {0}")]
    Api(String),
}

/// Step 1 — request a device + user code pair.
pub async fn request_device_code(client: &reqwest::Client) -> Result<DeviceCodeResponse, OAuthError> {
    let resp = client
        .post(DEVICE_CODE_URL)
        .form(&[("client_id", CLIENT_ID), ("scope", SCOPE)])
        .send()
        .await?
        .json::<DeviceCodeResponse>()
        .await?;
    Ok(resp)
}

/// Step 2 — poll until the user approves or the device code expires.
///
/// Sleeps `interval_secs` between attempts as required by the spec.
/// `slow_down` errors increase the delay by 5 s each time, also per spec.
pub async fn poll_token(
    client: &reqwest::Client,
    device_code: &str,
    interval_secs: u64,
) -> Result<OAuthToken, OAuthError> {
    let mut delay = Duration::from_secs(interval_secs.max(5));
    loop {
        tokio::time::sleep(delay).await;
        let val: serde_json::Value = client
            .post(TOKEN_URL)
            .form(&[
                ("client_id", CLIENT_ID),
                ("client_secret", CLIENT_SECRET),
                ("device_code", device_code),
                ("grant_type", "urn:ietf:params:oauth:grant-type:device_code"),
            ])
            .send()
            .await?
            .json()
            .await?;

        if let Some(err) = val.get("error").and_then(|v| v.as_str()) {
            match err {
                "authorization_pending" => continue,
                "slow_down" => {
                    delay += Duration::from_secs(5);
                    continue;
                }
                "expired_token" => return Err(OAuthError::Expired),
                "access_denied" => return Err(OAuthError::Denied),
                other => return Err(OAuthError::Api(other.to_owned())),
            }
        }
        return serde_json::from_value(val).map_err(|e| OAuthError::Api(e.to_string()));
    }
}

/// Refresh an expired `access_token` using a stored `refresh_token`.
///
/// Returns the new access token. The refresh token itself is unchanged.
pub async fn refresh_access_token(
    client: &reqwest::Client,
    refresh_token: &str,
) -> Result<String, OAuthError> {
    #[derive(serde::Deserialize)]
    struct Resp {
        access_token: String,
    }
    let r: Resp = client
        .post(TOKEN_URL)
        .form(&[
            ("client_id", CLIENT_ID),
            ("client_secret", CLIENT_SECRET),
            ("refresh_token", refresh_token),
            ("grant_type", "refresh_token"),
        ])
        .send()
        .await?
        .json()
        .await?;
    Ok(r.access_token)
}
