use std::sync::OnceLock;

use regex::Regex;
use tokio::sync::OnceCell;

static CLIENT: OnceLock<reqwest::Client> = OnceLock::new();

struct PlayerInfo {
    nfunc_js: Option<Box<str>>,
    sig_js: Option<Box<str>>,
    sig_timestamp: u32,
}

static PLAYER_INFO: OnceCell<PlayerInfo> = OnceCell::const_new();

fn shared_client() -> &'static reqwest::Client {
    CLIENT.get_or_init(reqwest::Client::new)
}

async fn player_js_url() -> Option<String> {
    let html = shared_client()
        .get("https://www.youtube.com")
        .header(
            "User-Agent",
            "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36",
        )
        .send()
        .await
        .ok()?
        .text()
        .await
        .ok()?;

    for pattern in [
        r#"PLAYER_JS_URL":"(/[^"]+base\.js)"#,
        r#""jsUrl":"(/[^"]+base\.js)"#,
        r#"src="(/s/player/[^"]+base\.js)"#,
    ] {
        if let Some(cap) = Regex::new(pattern).ok()?.captures(&html) {
            return Some(format!("https://www.youtube.com{}", &cap[1]));
        }
    }
    None
}

/// Returns the index of the matching closing bracket, scanning from `after_open`.
/// Skips string literal contents to avoid counting brackets inside strings.
fn find_closing(src: &str, after_open: usize, open: char, close: char) -> Option<usize> {
    let mut depth = 1usize;
    let mut in_str: Option<char> = None;
    let mut skip_next = false;

    for (off, c) in src[after_open..].char_indices() {
        if skip_next {
            skip_next = false;
            continue;
        }
        if let Some(delim) = in_str {
            if c == '\\' {
                skip_next = true;
            } else if c == delim {
                in_str = None;
            }
        } else {
            match c {
                '"' | '\'' | '`' => in_str = Some(c),
                c if c == open => depth += 1,
                c if c == close => {
                    depth -= 1;
                    if depth == 0 {
                        return Some(after_open + off);
                    }
                }
                _ => {}
            }
        }
    }
    None
}

fn extract_named_func(player_js: &str, sym: &str) -> Option<String> {
    let fn_re = Regex::new(&format!(
        r"(?:var\s+)?{}\s*=\s*(function\s*(?:[a-zA-Z0-9$_]+\s*)?\()",
        regex::escape(sym)
    ))
    .ok()?;
    let fn_match = fn_re.find(player_js)?;
    let fn_start = fn_match.start() + player_js[fn_match.start()..].find("function")?;
    let brace_start = fn_start + player_js[fn_start..].find('{')?;
    let brace_close = find_closing(player_js, brace_start + 1, '{', '}')?;
    Some(player_js[fn_start..=brace_close].to_owned())
}

fn extract_helper_obj(player_js: &str, sym: &str) -> Option<String> {
    let obj_re = Regex::new(&format!(
        r"(?:var\s+)?{}\s*=\s*\{{",
        regex::escape(sym)
    ))
    .ok()?;
    let m = obj_re.find(player_js)?;
    let brace_pos = m.start() + player_js[m.start()..].find('{')?;
    let close = find_closing(player_js, brace_pos + 1, '{', '}')?;
    Some(format!(
        "var {}={}",
        sym,
        &player_js[brace_pos..=close]
    ))
}

/// Newer player: QWO wraps `g.yk` which contains the actual nfunc.
///
/// Primary path: parse g.yk's delegation chain `return nN[H[idx]](...)` to locate the
/// actual transform function in the `nN` array — gives a tiny, browser-dep-free snippet.
///
/// Fallback: QWO fake-path URL trick with a large context window + browser stubs.
fn extract_nfunc_via_qwo(player_js: &str) -> Option<Box<str>> {
    let qwo_marker = "=function(N){try{var S=(new g.";
    let qwo_eq_pos = player_js.find(qwo_marker)?;

    // Walk back to get the QWO function name.
    let name_start = player_js[..qwo_eq_pos]
        .rfind(|c: char| !c.is_alphanumeric() && c != '$' && c != '_')
        .map(|i| i + 1)
        .unwrap_or(0);
    let qwo_name = player_js[name_start..qwo_eq_pos].to_owned();

    // Extract QWO function end position (needed for fallback chunk boundary).
    let fn_kw_off = player_js[qwo_eq_pos..].find("function")?;
    let fn_start = qwo_eq_pos + fn_kw_off;
    let brace_pos = fn_start + player_js[fn_start..].find('{')?;
    let qwo_close = find_closing(player_js, brace_pos + 1, '{', '}')?;

    // Find g.yk definition — search backwards from QWO.
    let before_qwo = &player_js[..qwo_eq_pos];
    let gyk_pos = before_qwo.rfind("g.yk=")?;

    {
        let hi = (gyk_pos + 300).min(player_js.len());
        tracing::debug!(snippet = &player_js[gyk_pos..hi], "g.yk definition");
    }

    // Try to trace the delegation chain: g.yk delegates to nN[H[idx]].
    let gyk_brace = gyk_pos + player_js[gyk_pos..].find('{')?;
    let gyk_close = find_closing(player_js, gyk_brace + 1, '{', '}')?;
    let gyk_body = &player_js[gyk_brace..=gyk_close];

    if let Some(func_src) = trace_delegation_nfunc(player_js, gyk_body, gyk_pos) {
        tracing::debug!(len = func_src.len(), "nfunc extracted via delegation trace");
        return Some(func_src);
    }

    // Fallback: QWO fake-path trick. Use a large window so nN is likely included.
    let chunk_start = gyk_pos.saturating_sub(60_000);
    let chunk = &player_js[chunk_start..=qwo_close];

    let js = format!(
        r#"var g={{}};
{chunk}
function __nfunc(n) {{
    var fakeUrl = "https://x.invalid/n/" + encodeURIComponent(n) + "/x";
    try {{
        var result = {qwo_name}(fakeUrl);
        if (!result) return null;
        var m = result.match(/\/n\/([^\/]+)\//);
        return m ? decodeURIComponent(m[1]) : null;
    }} catch(e) {{ return null; }}
}}"#
    );

    tracing::debug!(len = js.len(), qwo_name, "nfunc-via-QWO JS built (fallback)");
    Some(js.into_boxed_str())
}

/// Parse `g.yk = function(N,S){ return nN[H[idx]](this,...) }` and extract the
/// actual transform function at `nN[H[idx]]` as a self-contained `__nfunc` snippet.
///
/// `gyk_pos` anchors the search — H and nN are looked up in the 120 KB window before g.yk
/// to avoid matching unrelated short-named variables earlier in the player.
fn trace_delegation_nfunc(player_js: &str, gyk_body: &str, gyk_pos: usize) -> Option<Box<str>> {
    // Match: return OUTER[INNER[INDEX]]  (e.g. nN[H[18]])
    let re = Regex::new(
        r"return\s+([a-zA-Z0-9$_]+)\s*\[\s*([a-zA-Z0-9$_]+)\s*\[\s*(\d+)\s*\]\s*\]",
    )
    .ok()?;
    let caps = re.captures(gyk_body)?;
    let nn_name = caps[1].to_owned();
    let h_name = caps[2].to_owned();
    let h_idx: usize = caps[3].parse().ok()?;

    tracing::debug!(nn_name, h_name, h_idx, "delegation pattern found in g.yk");

    // Search for H and nN in a window around g.yk to avoid false matches.
    let window_start = gyk_pos.saturating_sub(120_000);
    let search_region = &player_js[window_start..];

    // Resolve H[h_idx] → integer index into nN.
    let h_arr_re = Regex::new(&format!(
        r"(?:var\s+|let\s+|const\s+)?{}\s*=\s*\[",
        regex::escape(&h_name)
    ))
    .ok()?;
    // Use the LAST match before gyk_pos (rfind semantics via find_iter + last).
    let h_match = h_arr_re
        .find_iter(search_region)
        .filter(|m| window_start + m.start() < gyk_pos)
        .last()?;
    let h_abs = window_start + h_match.start();
    let h_bracket = h_abs + player_js[h_abs..].find('[')?;
    let h_close = find_closing(player_js, h_bracket + 1, '[', ']')?;
    let h_contents = &player_js[h_bracket + 1..h_close];

    // Take the h_idx'th element directly (don't filter — that would shift indices).
    let nn_idx: usize = h_contents
        .split(',')
        .nth(h_idx)?
        .trim()
        .parse()
        .ok()?;

    tracing::debug!(nn_idx, "resolved H[{h_idx}] = {nn_idx}");

    // Locate nN array and pull out function at nn_idx.
    let nn_arr_re = Regex::new(&format!(
        r"(?:var\s+|let\s+|const\s+)?{}\s*=\s*\[",
        regex::escape(&nn_name)
    ))
    .ok()?;
    let nn_match = nn_arr_re
        .find_iter(search_region)
        .filter(|m| window_start + m.start() < gyk_pos)
        .last()?;
    let nn_abs = window_start + nn_match.start();
    let nn_bracket = nn_abs + player_js[nn_abs..].find('[')?;
    let nn_close = find_closing(player_js, nn_bracket + 1, '[', ']')?;
    let nn_contents = &player_js[nn_bracket + 1..nn_close];

    let func_re = Regex::new(r"function\s*(?:[a-zA-Z0-9$_]*)?\s*\(").ok()?;
    let nth = func_re.find_iter(nn_contents).nth(nn_idx)?;
    let brace_rel = nn_contents[nth.start()..].find('{')?;
    let brace_start = nth.start() + brace_rel;
    let brace_close = find_closing(nn_contents, brace_start + 1, '{', '}')?;
    let func_src = &nn_contents[nth.start()..=brace_close];

    tracing::debug!(len = func_src.len(), "actual transform function extracted from nN[{nn_idx}]");

    // The transform function often references a helper object for its operations.
    let js = if let Some(helper) = extract_nfunc_helper_obj(player_js, func_src) {
        format!("{helper};\nfunction __nfunc(n){{return ({func_src})(n);}}")
    } else {
        format!("function __nfunc(n){{return ({func_src})(n);}}")
    };

    Some(js.into_boxed_str())
}

/// If `func_src` calls methods on a short helper object (e.g. `Hb.splice(a,b)`),
/// find and extract that object so the function is self-contained.
fn extract_nfunc_helper_obj(player_js: &str, func_src: &str) -> Option<String> {
    let re = Regex::new(r"\b([a-zA-Z$_][a-zA-Z0-9$_]{0,3})\.[a-zA-Z$_]+\(").ok()?;
    let helper_name = re.captures(func_src).map(|c| c[1].to_owned())?;
    extract_helper_obj(player_js, &helper_name)
}

/// Extract the n-throttle decryption function.
fn extract_nfunc(player_js: &str) -> Option<Box<str>> {
    // Try QWO/g.yk approach first (newer players).
    if let Some(js) = extract_nfunc_via_qwo(player_js) {
        return Some(js);
    }

    let call_re = Regex::new(
        r#"\.get\("n"\)\)&&\(b=([a-zA-Z0-9$_]{1,4})(?:\[(\d+)\])?\([a-zA-Z0-9$_]+\)"#,
    )
    .ok()?;

    let caps = call_re.captures(player_js).or_else(|| {
        Regex::new(
            r#"\.get\("n"\)\)&&\([a-zA-Z0-9$_]+=([a-zA-Z0-9$_]{1,4})(?:\[(\d+)\])?\([a-zA-Z0-9$_]+\)"#,
        )
        .ok()
        .and_then(|re| re.captures(player_js))
    })?;

    let sym = &caps[1];
    let arr_idx = caps
        .get(2)
        .and_then(|m| m.as_str().parse::<usize>().ok());

    tracing::debug!(sym, ?arr_idx, "n-transform symbol located");

    let func_src = if let Some(idx) = arr_idx {
        let arr_re =
            Regex::new(&format!(r"(?:var\s+)?{}\s*=\s*\[", regex::escape(sym))).ok()?;
        let arr_match = arr_re.find(player_js)?;
        let bracket_rel = player_js[arr_match.start()..].find('[')?;
        let bracket_open = arr_match.start() + bracket_rel;
        let bracket_close = find_closing(player_js, bracket_open + 1, '[', ']')?;
        let arr_contents = &player_js[bracket_open + 1..bracket_close];

        let func_re = Regex::new(r"function\s*\(").ok()?;
        let nth = func_re.find_iter(arr_contents).nth(idx)?;
        let brace_rel = arr_contents[nth.start()..].find('{')?;
        let brace_start = nth.start() + brace_rel;
        let brace_close = find_closing(arr_contents, brace_start + 1, '{', '}')?;

        format!(
            "function{}",
            &arr_contents[nth.start() + "function".len()..=brace_close]
        )
    } else {
        extract_named_func(player_js, sym)?
    };

    tracing::debug!(len = func_src.len(), "nfunc extracted");
    Some(func_src.into_boxed_str())
}

/// Extract the signature decryption function (for `signatureCipher` formats).
///
/// Returns a self-contained JS snippet: `function(sig){...}` that decrypts one sig string.
fn extract_sig_func(player_js: &str) -> Option<Box<str>> {
    // Call-site patterns — find the sig-fn name.
    let sig_name = [
        r#"\.set\("sig"\s*,\s*([a-zA-Z0-9$_]{2,4})\([a-zA-Z0-9$_]+\.get\("s"\)\)"#,
        r#"\.sig\|\|([a-zA-Z0-9$_]{2,4})\(decodeURIComponent"#,
        r#"a\.sig=([a-zA-Z0-9$_]{2,4})\(a\."#,
        // Broader: assignment to sig from a 2-4 char fn
        r#""signature",([a-zA-Z0-9$_]{2,4})\("#,
    ]
    .iter()
    .find_map(|pat| {
        Regex::new(pat)
            .ok()
            .and_then(|re| re.captures(player_js))
            .map(|c| c[1].to_owned())
    })?;

    tracing::debug!(sig_name, "sig-transform symbol located");

    let fn_src = extract_named_func(player_js, &sig_name)?;

    // The sig function references a helper object; find and extract it too.
    let helper_name = Regex::new(r"([a-zA-Z0-9$_]{2,4})\.[a-zA-Z0-9$_]+\(a,")
        .ok()?
        .captures(&fn_src)?[1]
        .to_owned();

    let helper_src = extract_helper_obj(player_js, &helper_name)?;

    let full = format!("{helper_src};\nfunction __sig(a){{\n{fn_src}\nreturn {sig_name}(a);\n}}");
    tracing::debug!(len = full.len(), "sig func extracted");
    Some(full.into_boxed_str())
}

fn extract_sig_timestamp(player_js: &str) -> u32 {
    for pat in [
        r"signatureTimestamp:(\d+)",
        r"\.signatureTimestamp\s*=\s*(\d+)",
        r#""sts"\s*:\s*(\d+)"#,
    ] {
        if let Some(caps) = Regex::new(pat).ok().and_then(|re| re.captures(player_js)) {
            if let Ok(ts) = caps[1].parse::<u32>() {
                tracing::debug!(ts, "signatureTimestamp found");
                return ts;
            }
        }
    }
    tracing::warn!("signatureTimestamp not found in player JS, using fallback 19950");
    19950
}

async fn player_info() -> &'static PlayerInfo {
    PLAYER_INFO
        .get_or_init(|| async {
            let result = async {
                let url = player_js_url().await?;
                tracing::debug!(url, "fetching player JS");
                let js = shared_client()
                    .get(&url)
                    .send()
                    .await
                    .ok()?
                    .text()
                    .await
                    .ok()?;
                Some(js)
            }
            .await;

            match result {
                Some(js) => {
                    let nfunc_js = extract_nfunc(&js).or_else(|| {
                        tracing::warn!("nfunc not found in player JS");
                        None
                    });
                    let sig_js = extract_sig_func(&js).or_else(|| {
                        tracing::warn!("sig func not found in player JS");
                        None
                    });
                    let sig_timestamp = extract_sig_timestamp(&js);
                    PlayerInfo { nfunc_js, sig_js, sig_timestamp }
                }
                None => {
                    tracing::warn!("failed to fetch player JS");
                    PlayerInfo { nfunc_js: None, sig_js: None, sig_timestamp: 19950 }
                }
            }
        })
        .await
}

pub async fn signature_timestamp() -> u32 {
    player_info().await.sig_timestamp
}

// Browser API stubs injected into QuickJS before any player JS runs.
// QuickJS provides a minimal ES runtime with no DOM/BOM globals.
const BROWSER_STUBS: &str = r#"
var window = globalThis;
var self = globalThis;
var document = {
    cookie: '',
    location: { href: 'https://www.youtube.com/', hostname: 'www.youtube.com', protocol: 'https:' },
    createElement: function() { return {}; },
    getElementById: function() { return null; }
};
var navigator = {
    userAgent: 'Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36',
    platform: 'Linux x86_64',
    language: 'en-US'
};
var console = {
    log: function(){}, warn: function(){}, error: function(){},
    debug: function(){}, info: function(){}, trace: function(){}
};
var performance = { now: function() { return Date.now(); }, timeOrigin: 0 };
var location = {
    href: 'https://www.youtube.com/',
    hostname: 'www.youtube.com',
    protocol: 'https:',
    pathname: '/',
    search: ''
};
var history = { pushState: function(){}, replaceState: function(){} };
var setTimeout = function(fn, ms) { return 0; };
var clearTimeout = function(id) {};
var setInterval = function(fn, ms) { return 0; };
var clearInterval = function(id) {};
var requestAnimationFrame = function(fn) { return 0; };
var cancelAnimationFrame = function(id) {};
function XMLHttpRequest() {
    this.open = function(){};
    this.send = function(){};
    this.setRequestHeader = function(){};
    this.abort = function(){};
    this.readyState = 0;
    this.status = 0;
    this.responseText = '';
}
var fetch = function() {
    return Promise.resolve({ ok: false, json: function(){ return Promise.resolve({}); } });
};
var crypto = {
    getRandomValues: function(a) {
        for (var i = 0; i < a.length; i++) a[i] = (Math.random() * 256) | 0;
        return a;
    },
    subtle: {}
};
var localStorage = { getItem: function(){ return null; }, setItem: function(){}, removeItem: function(){} };
var sessionStorage = localStorage;
"#;

/// Decrypt the YouTube CDN `n` throttling parameter using the player JS nfunc.
async fn decrypt_nsig(encrypted: &str) -> Option<String> {
    let nfunc = player_info().await.nfunc_js.as_deref()?;
    let encrypted = encrypted.to_owned();
    let nfunc = nfunc.to_owned();

    let is_qwo_style = nfunc.contains("__nfunc");

    tokio::task::spawn_blocking(move || {
        let rt = rquickjs::Runtime::new().ok()?;
        let ctx = rquickjs::Context::full(&rt).ok()?;
        ctx.with(|ctx| {
            if let Err(e) = ctx.eval::<rquickjs::Value, _>(BROWSER_STUBS) {
                tracing::warn!(error = %e, "browser stubs injection failed");
            }

            let n_json = serde_json::to_string(&encrypted).ok()?;
            let code = if is_qwo_style {
                format!("{nfunc}\n__nfunc({n_json})")
            } else {
                format!("({nfunc})({n_json})")
            };
            let result = ctx.eval::<rquickjs::Value, _>(code.as_str());
            match result {
                Ok(v) => v.as_string().and_then(|s| s.to_string().ok()),
                Err(e) => {
                    tracing::warn!(error = %e, is_qwo_style, "QuickJS eval error in nfunc");
                    None
                }
            }
        })
    })
    .await
    .ok()
    .flatten()
}

/// Decrypt a YouTube `signatureCipher` value (base64 or raw encrypted sig).
pub async fn decrypt_sig(encrypted: &str) -> Option<String> {
    let sig_js = player_info().await.sig_js.as_deref()?;
    let encrypted = encrypted.to_owned();
    let sig_js = sig_js.to_owned();

    tokio::task::spawn_blocking(move || {
        let rt = rquickjs::Runtime::new().ok()?;
        let ctx = rquickjs::Context::full(&rt).ok()?;
        ctx.with(|ctx| {
            let _ = ctx.eval::<rquickjs::Value, _>(BROWSER_STUBS);
            let s_json = serde_json::to_string(&encrypted).ok()?;
            let code = format!("{sig_js}\n__sig({s_json})");
            ctx.eval::<String, _>(code.as_str()).ok()
        })
    })
    .await
    .ok()
    .flatten()
}

/// Return `url` with its `n` query param replaced by the decrypted value.
pub async fn decrypt_url(url: &str) -> String {
    let Ok(mut parsed) = reqwest::Url::parse(url) else {
        return url.to_owned();
    };

    let Some(encrypted) = parsed
        .query_pairs()
        .find(|(k, _)| k == "n")
        .map(|(_, v)| v.into_owned())
    else {
        return url.to_owned();
    };

    let Some(decrypted) = decrypt_nsig(&encrypted).await else {
        tracing::warn!("nsig decryption failed, using original URL");
        return url.to_owned();
    };

    let pairs: Vec<(String, String)> = parsed
        .query_pairs()
        .map(|(k, v)| {
            let v = if k == "n" {
                decrypted.clone()
            } else {
                v.into_owned()
            };
            (k.into_owned(), v)
        })
        .collect();
    parsed.query_pairs_mut().clear().extend_pairs(&pairs);

    tracing::debug!("nsig decrypted");
    parsed.to_string()
}
