use std::collections::HashMap;

use crate::providers::chrome::{ChromeOptions, get_cookies_from_chrome};
use crate::providers::edge::{EdgeOptions, get_cookies_from_edge};
use crate::providers::firefox::{FirefoxOptions, get_cookies_from_firefox};
use crate::providers::inline::{InlineSource, get_cookies_from_inline};
use crate::providers::safari::{SafariOptions, get_cookies_from_safari};
use crate::types::{Cookie, GetCookiesOptions, GetCookiesResult, normalize_names};
use crate::util::origins::normalize_origins;

pub async fn get_cookies(options: GetCookiesOptions) -> GetCookiesResult {
    let mut warnings: Vec<String> = Vec::new();
    let origins = normalize_origins(&options.url, options.origins.as_deref());
    let names = normalize_names(&options.names);

    let browser_futures: Vec<
        std::pin::Pin<Box<dyn std::future::Future<Output = GetCookiesResult>>>,
    > = vec![
        Box::pin(get_cookies_from_chrome(
            ChromeOptions {
                profile: options
                    .chrome_profile
                    .clone()
                    .or_else(|| options.profile.clone()),
                timeout_ms: options.timeout_ms,
                include_expired: options.include_expired,
                debug: options.debug,
            },
            &origins,
            names.as_ref(),
        )),
        Box::pin(get_cookies_from_edge(
            EdgeOptions {
                profile: options
                    .edge_profile
                    .clone()
                    .or_else(|| options.profile.clone()),
                timeout_ms: options.timeout_ms,
                include_expired: options.include_expired,
                debug: options.debug,
            },
            &origins,
            names.as_ref(),
        )),
        Box::pin(get_cookies_from_firefox(
            FirefoxOptions {
                profile: options.firefox_profile.clone(),
                include_expired: options.include_expired,
            },
            &origins,
            names.as_ref(),
        )),
        Box::pin(get_cookies_from_safari(
            SafariOptions {
                include_expired: options.include_expired,
                file: options.safari_cookies_file.clone(),
            },
            &origins,
            names.as_ref(),
        )),
    ];

    // Inline sources first
    let inline_sources = resolve_inline_sources(&options);
    for source in &inline_sources {
        let inline_result = get_cookies_from_inline(source, &origins, names.as_ref()).await;
        warnings.extend(inline_result.warnings);
        if !inline_result.cookies.is_empty() {
            return GetCookiesResult {
                cookies: inline_result.cookies,
                warnings,
            };
        }
    }

    let mut merged: HashMap<String, Cookie> = HashMap::new();

    for fut in browser_futures {
        let result = fut.await;
        warnings.extend(result.warnings);

        for cookie in result.cookies {
            let domain = cookie.domain.as_deref().unwrap_or("");
            let path = cookie.path.as_deref().unwrap_or("");
            let key = format!("{}|{}|{}", cookie.name, domain, path);

            merged
                .entry(key)
                .and_modify(|existing: &mut Cookie| {
                    let existing_exp = existing.expires.unwrap_or(i64::MIN);
                    let new_exp = cookie.expires.unwrap_or(i64::MIN);
                    if new_exp > existing_exp {
                        *existing = cookie.clone();
                    }
                })
                .or_insert(cookie);
        }
    }

    GetCookiesResult {
        cookies: merged.into_values().collect(),
        warnings,
    }
}

pub fn to_cookie_header(cookies: &[Cookie]) -> String {
    let mut items: Vec<(&str, &str)> = cookies
        .iter()
        .filter(|c| !c.name.is_empty())
        .map(|c| (c.name.as_str(), c.value.as_str()))
        .collect();

    items.sort_by(|a, b| a.0.cmp(b.0));

    items
        .iter()
        .map(|(n, v)| format!("{n}={v}"))
        .collect::<Vec<_>>()
        .join("; ")
}

fn resolve_inline_sources(options: &GetCookiesOptions) -> Vec<InlineSource> {
    let mut sources = Vec::new();
    if let Some(ref json) = options.inline_cookies_json {
        sources.push(InlineSource {
            source: "inline-json".to_string(),
            payload: json.clone(),
        });
    }
    if let Some(ref b64) = options.inline_cookies_base64 {
        sources.push(InlineSource {
            source: "inline-base64".to_string(),
            payload: b64.clone(),
        });
    }
    if let Some(ref file) = options.inline_cookies_file {
        sources.push(InlineSource {
            source: "inline-file".to_string(),
            payload: file.clone(),
        });
    }
    sources
}
