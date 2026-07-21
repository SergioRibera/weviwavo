use std::sync::OnceLock;

use regex::Regex;
use tokio::sync::OnceCell;

static CLIENT: OnceLock<reqwest::Client> = OnceLock::new();

struct PlayerInfo {
    nfunc_js: Option<Box<str>>,
    /// nClass name (e.g. "yk") for QWO players when delegation trace fails.
    nclass: Option<Box<str>>,
    /// Full player JS for nClass nsig and/or sig-injection paths.
    player_js: Option<Box<str>>,
    /// Classic self-contained sig snippet (old players).
    sig_js: Option<Box<str>>,
    /// Modern players: `g.__sig=function(__s){...};` injected before IIFE close.
    sig_injection: Option<Box<str>>,
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

/// Newer player: a QWO wrapper function calls `(new g.CLASS(url, arg)).get("n")`.
/// Variable names (`C`/`N`, `T`/`S`, etc.) change per player version — match flexibly.
///
/// Primary path: parse the nClass delegation chain to get a self-contained snippet.
/// Fallback: return None so the nClass eval path is used instead.
fn extract_nfunc_via_qwo(player_js: &str) -> Option<Box<str>> {
    // Match: =function(VAR){try{var INNER=(new g.CLASS(
    // Captures class name (group 1).
    let qwo_re = Regex::new(
        r"=function\([A-Za-z0-9$_]+\)\{try\{var [A-Za-z0-9$_]+=\(new g\.([a-zA-Z0-9$_]+)\(",
    )
    .ok()?;
    let qwo_caps = qwo_re.captures(player_js)?;
    let qwo_match = qwo_caps.get(0)?;
    let qwo_eq_pos = qwo_match.start();
    let gyk_name = qwo_caps[1].to_owned(); // e.g. "uS", "yk", …

    let name_start = player_js[..qwo_eq_pos]
        .rfind(|c: char| !c.is_alphanumeric() && c != '$' && c != '_')
        .map(|i| i + 1)
        .unwrap_or(0);
    let qwo_name = player_js[name_start..qwo_eq_pos].to_owned();

    tracing::debug!(qwo_name, nclass = gyk_name, "QWO function found");

    // Find g.CLASS definition before the QWO function.
    let before_qwo = &player_js[..qwo_eq_pos];
    let gyk_pattern = format!("g.{gyk_name}=");
    let gyk_pos = before_qwo.rfind(&gyk_pattern)?;

    {
        let hi = (gyk_pos + 300).min(player_js.len());
        tracing::debug!(snippet = &player_js[gyk_pos..hi], "g.{gyk_name} definition");
    }

    let gyk_brace = gyk_pos + player_js[gyk_pos..].find('{')?;
    let gyk_close = find_closing(player_js, gyk_brace + 1, '{', '}')?;
    let gyk_body = &player_js[gyk_brace..=gyk_close];

    if let Some(func_src) = trace_delegation_nfunc(player_js, gyk_body, gyk_pos) {
        tracing::debug!(len = func_src.len(), "nfunc extracted via delegation trace");
        return Some(func_src);
    }

    tracing::debug!(qwo_name, nclass = gyk_name, "delegation trace failed; nClass path will be used");
    None
}

/// Extract the nClass name from `(new g.CLASS(arg)).get("n")`.
/// Variable names change per player version — match flexibly.
fn extract_nclass_from_qwo(player_js: &str) -> Option<Box<str>> {
    let re = Regex::new(r#"\(new g\.([a-zA-Z0-9$_]+)\([^)]*\)\)\.get\("n"\)"#).ok()?;
    let caps = re.captures(player_js)?;
    let name = caps[1].to_owned();
    tracing::debug!(nclass = name, "extracted nClass via .get(\"n\") pattern");
    Some(name.into())
}

/// Parse `g.yk = function(N,S){ return nN[H[idx]](this,...) }` and extract the
/// actual transform function at `nN[H[idx]]` as a self-contained `__nfunc` snippet.
///
/// Searches the entire player JS before `g.yk` — the H/nN definitions can be hundreds of
/// kilobytes away in minified code.
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

    // Resolve H[h_idx] → integer index into nN.
    // Take the last definition before g.yk — handles reassignment and closures.
    let h_arr_re = Regex::new(&format!(
        r"(?:var\s+|let\s+|const\s+)?{}\s*=\s*\[",
        regex::escape(&h_name)
    ))
    .ok()?;
    let h_matches: Vec<_> = h_arr_re
        .find_iter(player_js)
        .filter(|m| m.start() < gyk_pos)
        .collect();
    tracing::debug!(
        count = h_matches.len(),
        h_name,
        gyk_kb = gyk_pos / 1024,
        "H array search results (full file before g.yk)"
    );
    let h_match = match h_matches.into_iter().last() {
        Some(m) => m,
        None => {
            // Extra diagnostic: show what the h_name looks like in context near g.yk.
            let ctx_lo = gyk_pos.saturating_sub(500);
            let ctx_hi = (gyk_pos + 100).min(player_js.len());
            tracing::warn!(
                h_name,
                gyk_ctx = &player_js[ctx_lo..ctx_hi],
                "H array literal not found before g.yk"
            );
            return None;
        }
    };
    let h_abs = h_match.start();
    let h_bracket = match player_js[h_abs..].find('[') {
        Some(off) => h_abs + off,
        None => {
            tracing::warn!("H array bracket not found");
            return None;
        }
    };
    let h_close = match find_closing(player_js, h_bracket + 1, '[', ']') {
        Some(c) => c,
        None => {
            tracing::warn!("H array closing bracket not found");
            return None;
        }
    };
    let h_contents = &player_js[h_bracket + 1..h_close];
    tracing::debug!(
        preview = &h_contents[..h_contents.len().min(300)],
        len = h_contents.len(),
        h_pos_kb = h_abs / 1024,
        "H array contents"
    );

    // Take the h_idx'th element directly (don't filter — that would shift indices).
    let h_element = match h_contents.split(',').nth(h_idx) {
        Some(e) => e.trim().to_owned(),
        None => {
            tracing::warn!(h_idx, elements = h_contents.split(',').count(), "H[h_idx] out of range");
            return None;
        }
    };
    let nn_idx: usize = match h_element.parse() {
        Ok(n) => n,
        Err(_) => {
            tracing::warn!(h_element, h_idx, "H[h_idx] is not a plain integer");
            return None;
        }
    };

    tracing::debug!(nn_idx, "resolved H[{h_idx}] = {nn_idx}");

    // Locate nN array and pull out function at nn_idx.
    let nn_arr_re = Regex::new(&format!(
        r"(?:var\s+|let\s+|const\s+)?{}\s*=\s*\[",
        regex::escape(&nn_name)
    ))
    .ok()?;
    let nn_matches: Vec<_> = nn_arr_re
        .find_iter(player_js)
        .filter(|m| m.start() < gyk_pos)
        .collect();
    tracing::debug!(count = nn_matches.len(), nn_name, "nN array search results");
    let nn_match = match nn_matches.into_iter().last() {
        Some(m) => m,
        None => {
            tracing::warn!(nn_name, "nN array not found in player JS before g.yk");
            return None;
        }
    };
    let nn_abs = nn_match.start();
    let nn_bracket = nn_abs + player_js[nn_abs..].find('[')?;
    let nn_close = find_closing(player_js, nn_bracket + 1, '[', ']')?;
    let nn_contents = &player_js[nn_bracket + 1..nn_close];

    let func_re = Regex::new(r"function\s*(?:[a-zA-Z0-9$_]*)?\s*\(").ok()?;
    let func_count = func_re.find_iter(nn_contents).count();
    tracing::debug!(func_count, nn_idx, "functions found in nN array");
    let nth = match func_re.find_iter(nn_contents).nth(nn_idx) {
        Some(m) => m,
        None => {
            tracing::warn!(nn_idx, func_count, "nn_idx out of range in nN array");
            return None;
        }
    };
    let brace_rel = nn_contents[nth.start()..].find('{')?;
    let brace_start = nth.start() + brace_rel;
    let brace_close = find_closing(nn_contents, brace_start + 1, '{', '}')?;
    let func_src = &nn_contents[nth.start()..=brace_close];

    tracing::debug!(len = func_src.len(), nn_idx, "actual transform function extracted from nN[{nn_idx}]");

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

    // Multiple call-site patterns for n-throttle, covering several player generations.
    let n_call_patterns = [
        r#"\.get\("n"\)\)&&\(b=([a-zA-Z0-9$_]{1,4})(?:\[(\d+)\])?\([a-zA-Z0-9$_]+\)"#,
        r#"\.get\("n"\)\)&&\([a-zA-Z0-9$_]+=([a-zA-Z0-9$_]{1,4})(?:\[(\d+)\])?\([a-zA-Z0-9$_]+\)"#,
        r#"n&&\(n=([a-zA-Z0-9$_]{1,4})(?:\[(\d+)\])?\(n\)"#,
        r#"\(b=([a-zA-Z0-9$_]{1,4})\[([a-zA-Z0-9$_]{1,4})\[(\d+)\]\]\)"#,
        r#"\.set\("n"\s*,\s*([a-zA-Z0-9$_]{1,4})(?:\[(\d+)\])?\([a-zA-Z0-9$_]+\)\)"#,
    ];
    let caps = n_call_patterns
        .iter()
        .find_map(|pat| Regex::new(pat).ok().and_then(|re| re.captures(player_js)))?;

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
/// Returns a self-contained JS snippet: `function __sig(a){...}` that decrypts one sig string.
fn extract_sig_func(player_js: &str) -> Option<Box<str>> {
    // 2025/2026 patterns: sig fn takes a numeric constant as first arg.
    // e.g. `&&(z=hJ(6,decodeURIComponent(h.s))` → name=hJ, const=6.
    let sig_with_const_patterns = [
        // April 2026: FUNC(NUM, decodeURIComponent(h.s))
        r"&&\s*\([a-zA-Z0-9$_]+=([a-zA-Z0-9$_]{2,4})\((\d+),\s*decodeURIComponent\([a-zA-Z0-9$_]+\.[a-z]\)",
        // 2025+: FUNC(NUM, decodeURIComponent(VAR))
        r"&&\s*\([a-zA-Z0-9$_]+=([a-zA-Z0-9$_]{2,4})\((\d+),\s*decodeURIComponent\([a-zA-Z0-9$_]+\)",
    ];
    for pat in &sig_with_const_patterns {
        if let Some(caps) = Regex::new(pat).ok().and_then(|re| re.captures(player_js)) {
            let sig_name = caps[1].to_owned();
            let const_arg = caps[2].to_owned();
            tracing::debug!(sig_name, const_arg, "sig-transform (const-arg pattern) located");
            let fn_src = extract_named_func(player_js, &sig_name)?;
            let helper_src = Regex::new(r"([a-zA-Z0-9$_]{2,4})\.[a-zA-Z0-9$_]+\(")
                .ok()
                .and_then(|re| re.captures(&fn_src))
                .and_then(|c| extract_helper_obj(player_js, &c[1].to_owned()));
            let prefix = helper_src.as_deref().unwrap_or("");
            let full = format!(
                "{prefix}function __sig(a){{\n{fn_src}\nreturn {sig_name}({const_arg},a);\n}}"
            );
            tracing::debug!(len = full.len(), "sig func (const-arg) extracted");
            return Some(full.into_boxed_str());
        }
    }

    // Classic patterns: sig fn takes just the sig string.
    let sig_name = [
        r#"\.set\("sig"\s*,\s*([a-zA-Z0-9$_]{2,4})\([a-zA-Z0-9$_]+\.get\("s"\)\)"#,
        r#"\.sig\|\|([a-zA-Z0-9$_]{2,4})\(decodeURIComponent"#,
        r#"a\.sig=([a-zA-Z0-9$_]{2,4})\(a\."#,
        r#"\.set\("signature"\s*,\s*([a-zA-Z0-9$_]{2,4})\(b\)"#,
        r#"a=([a-zA-Z0-9$_]{2,4})\(decodeURIComponent\(a\.get\("s"\)"#,
        r#"([a-zA-Z0-9$_]{2,4})\(decodeURIComponent\(h\.s\)\)"#,
        r#""signature",([a-zA-Z0-9$_]{2,4})\("#,
        r#"\.set\("signature",([a-zA-Z0-9$_]{2,4})\("#,
        // Broader: encodeURIComponent(FUNC(sig))
        r#"\b[cs]\s*&&\s*[a-z]+\.set\([^,]+,\s*encodeURIComponent\(([a-zA-Z0-9$_]{2,4})\("#,
        r#"\b[a-zA-Z0-9]+\s*&&\s*[a-zA-Z0-9]+\.set\([^,]+,\s*encodeURIComponent\(([a-zA-Z0-9$_]{2,4})\("#,
        r#"\bm=([a-zA-Z0-9$_]{2,})\(decodeURIComponent\(h\.s\)\)"#,
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

    let helper_src = Regex::new(r"([a-zA-Z0-9$_]{2,4})\.[a-zA-Z0-9$_]+\(a,")
        .ok()
        .and_then(|re| re.captures(&fn_src))
        .and_then(|c| extract_helper_obj(player_js, &c[1].to_owned()));
    let prefix = helper_src.as_deref().unwrap_or("");

    let full = format!("{prefix}\nfunction __sig(a){{\n{fn_src}\nreturn {sig_name}(a);\n}}");
    tracing::debug!(len = full.len(), "sig func extracted");
    Some(full.into_boxed_str())
}

/// Modern 2025+ players: find `R=OUTER(C1,C2,INNER(C3,C4,VAR.s))…(P,ENCODE(C5,C6,R))` and
/// produce a `g.__sig=function(__s){…};` snippet to inject before the IIFE close.
fn extract_sig_injection(player_js: &str) -> Option<Box<str>> {
    // Pattern A: 3-function chain — outer(c1,c2, inner(c3,c4, sig)) → encode(c5,c6, result)
    let pat_a = Regex::new(
        r"R=([A-Za-z0-9$_]{1,4})\((\d+),(\d+),([A-Za-z0-9$_]{1,4})\((\d+),(\d+),[^)]+\.s\)\).{0,60}\(P,([A-Za-z0-9$_]{1,4})\((\d+),(\d+),R\)\)"
    ).ok()?;
    if let Some(caps) = pat_a.captures(player_js) {
        let (f1, c1, c2) = (&caps[1], &caps[2], &caps[3]);
        let (f2, c3, c4) = (&caps[4], &caps[5], &caps[6]);
        let (f3, c5, c6) = (&caps[7], &caps[8], &caps[9]);
        tracing::debug!(f1, c1, c2, f2, c3, c4, f3, c5, c6, "sig injection (3-func) found");
        let inj = format!(
            "g.__sig=function(__s){{try{{return {f3}({c5},{c6},{f1}({c1},{c2},{f2}({c3},{c4},__s)));}}catch(e){{return null;}}}};"
        );
        return Some(inj.into_boxed_str());
    }

    // Pattern B: 2-function chain, no encode step — outer(c1,c2, inner(c3,c4, sig))
    let pat_b = Regex::new(
        r"R=([A-Za-z0-9$_]{1,4})\((\d+),(\d+),([A-Za-z0-9$_]{1,4})\((\d+),(\d+),[^)]+\.s\)\).{0,60}\(P,R\)"
    ).ok()?;
    if let Some(caps) = pat_b.captures(player_js) {
        let (f1, c1, c2) = (&caps[1], &caps[2], &caps[3]);
        let (f2, c3, c4) = (&caps[4], &caps[5], &caps[6]);
        tracing::debug!(f1, c1, c2, f2, c3, c4, "sig injection (2-func) found");
        let inj = format!(
            "g.__sig=function(__s){{try{{return {f1}({c1},{c2},{f2}({c3},{c4},__s));}}catch(e){{return null;}}}};"
        );
        return Some(inj.into_boxed_str());
    }

    // Pattern C: single instrumented call — f(c1, c2, sig) with encode
    let pat_c = Regex::new(
        r"R=([A-Za-z0-9$_]{1,4})\((\d+),(\d+),[^)]+\.s\).{0,60}\(P,([A-Za-z0-9$_]{1,4})\((\d+),(\d+),R\)\)"
    ).ok()?;
    if let Some(caps) = pat_c.captures(player_js) {
        let (f1, c1, c2) = (&caps[1], &caps[2], &caps[3]);
        let (f3, c5, c6) = (&caps[4], &caps[5], &caps[6]);
        tracing::debug!(f1, c1, c2, f3, c5, c6, "sig injection (1-func+encode) found");
        let inj = format!(
            "g.__sig=function(__s){{try{{return {f3}({c5},{c6},{f1}({c1},{c2},__s));}}catch(e){{return null;}}}};"
        );
        return Some(inj.into_boxed_str());
    }

    tracing::warn!("sig injection patterns all failed");
    None
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

/// Logs key pattern presence and writes player JS to /tmp for manual analysis.
fn dump_player_diagnostics(js: &str) {
    let has_qwo_marker = js.contains("=function(N){try{var S=(new g.");
    let has_gyk = js.contains("g.yk=");
    let has_nget = js.contains(r#".get("n")"#);
    let has_nset = js.contains(r#".set("n","#);
    let has_sig_set = js.contains(r#".set("signature","#) || js.contains(r#".set("sig","#);

    // Find the snippet around .get("n") for pattern analysis.
    let nget_ctx = js
        .find(r#".get("n")"#)
        .map(|pos| {
            let lo = pos.saturating_sub(80);
            let hi = (pos + 200).min(js.len());
            js[lo..hi].to_owned()
        })
        .unwrap_or_default();

    // Snippet around "signature" set-call.
    let sig_ctx = js
        .find(r#""signature""#)
        .map(|pos| {
            let lo = pos.saturating_sub(80);
            let hi = (pos + 200).min(js.len());
            js[lo..hi].to_owned()
        })
        .unwrap_or_default();

    tracing::warn!(
        has_qwo_marker,
        has_gyk,
        has_nget,
        has_nset,
        has_sig_set,
        js_len = js.len(),
        "player JS diagnostics"
    );
    tracing::warn!(nget_ctx, "context around .get(\"n\")");
    tracing::warn!(sig_ctx, "context around \"signature\"");

    // Write full player JS to /tmp so it can be inspected manually.
    if let Err(e) = std::fs::write("/tmp/yt-player-debug.js", js) {
        tracing::warn!(error = %e, "failed to write player JS to /tmp");
    } else {
        tracing::warn!("player JS written to /tmp/yt-player-debug.js for inspection");
    }
}

/// Pre-initialize IIFE-scoped vars that are assigned late (after a throw point).
///
/// `k4=window` is assigned at offset ~1.65 MB but `qN` reads `k4` at ~1.4 MB.
/// JS `var` hoisting means `k4` is `undefined` until the assignment runs —
/// that causes "Context has not been set and window is undefined." to throw and
/// stop execution before g.yk (~2.14 MB) and g.__sig (near end) are defined.
fn patch_player_js_for_eval(js: String) -> String {
    // `var window=this` makes window=_yt_player={} inside the IIFE — window.document etc
    // are undefined, causing throws mid-IIFE before g.yk / g.__sig are reached.
    // Replace with globalThis so our BROWSER_STUBS are visible as window.* inside the IIFE.
    // Also pre-assign k4 (read by qN at ~1.4 MB, normally set at ~1.65 MB).
    js.replacen(
        "(function(g){var window=this;",
        "(function(g){var window=globalThis;k4=globalThis;",
        1,
    )
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
                        dump_player_diagnostics(&js);
                        None
                    });
                    // When delegation trace fails, fall back to the nClass constructor approach.
                    let nclass = if nfunc_js.is_none() {
                        extract_nclass_from_qwo(&js)
                    } else {
                        None
                    };

                    let sig_js = extract_sig_func(&js).or_else(|| {
                        tracing::warn!("sig func not found in player JS");
                        None
                    });
                    // Modern players: inject g.__sig into the IIFE when classic extraction fails.
                    let sig_injection = if sig_js.is_none() {
                        extract_sig_injection(&js).or_else(|| {
                            tracing::warn!("sig injection extraction failed");
                            None
                        })
                    } else {
                        None
                    };

                    // Extract sig_timestamp before js is potentially moved.
                    let sig_timestamp = extract_sig_timestamp(&js);

                    // Keep full player JS when nClass or sig-injection paths are needed.
                    // Apply early-init patch so eval doesn't throw before g.yk / g.__sig are set.
                    let player_js = (nclass.is_some() || sig_injection.is_some())
                        .then(|| patch_player_js_for_eval(js).into_boxed_str());

                    PlayerInfo { nfunc_js, nclass, player_js, sig_js, sig_injection, sig_timestamp }
                }
                None => {
                    tracing::warn!("failed to fetch player JS");
                    PlayerInfo { nfunc_js: None, nclass: None, player_js: None, sig_js: None, sig_injection: None, sig_timestamp: 19950 }
                }
            }
        })
        .await
}

pub async fn signature_timestamp() -> u32 {
    player_info().await.sig_timestamp
}

fn format_js_exception(ctx: &rquickjs::Ctx<'_>) -> String {
    let exc = ctx.catch();
    if let Some(s) = exc.as_string().and_then(|s| s.to_string().ok()) {
        return s;
    }
    if let Some(obj) = exc.as_object() {
        let msg = obj
            .get::<_, rquickjs::Value>("message")
            .ok()
            .and_then(|v| v.as_string().and_then(|s| s.to_string().ok()))
            .unwrap_or_default();
        let stack = obj
            .get::<_, rquickjs::Value>("stack")
            .ok()
            .and_then(|v| v.as_string().and_then(|s| s.to_string().ok()))
            .unwrap_or_default();
        return format!("{msg}\n{stack}");
    }
    format!("{exc:?}")
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
// Event listeners
globalThis.addEventListener = function(){};
globalThis.removeEventListener = function(){};
globalThis.dispatchEvent = function(){ return true; };
document.addEventListener = function(){};
document.removeEventListener = function(){};
document.createEvent = function(){ return {initEvent:function(){},target:null}; };
// DOM helpers
var screen = { width: 1920, height: 1080, colorDepth: 24, pixelDepth: 24 };
var Image = function(){};
var Blob = globalThis.Blob || function(parts, opts){ this.size=0; this.type=(opts&&opts.type)||''; };
var URL = globalThis.URL || {
    createObjectURL: function(){ return ''; },
    revokeObjectURL: function(){},
    parse: function(){ return null; }
};
var Worker = function(){ this.postMessage=function(){}; this.terminate=function(){}; };
var MessageChannel = function(){
    var p = { onmessage: null, postMessage: function(){} };
    this.port1 = p; this.port2 = p;
};
var MutationObserver = function(cb){ this.observe=function(){}; this.disconnect=function(){}; };
var ResizeObserver = function(cb){ this.observe=function(){}; this.disconnect=function(){}; };
var IntersectionObserver = function(cb, opts){ this.observe=function(){}; this.disconnect=function(){}; };
var MediaSource = { isTypeSupported: function(){ return false; } };
var HTMLVideoElement = function(){};
var CustomEvent = function(type, opts){ this.type=type; this.detail=(opts&&opts.detail)||null; };
// _yt_player is the IIFE argument — g.* properties set inside survive even if the IIFE throws.
var _yt_player = {};
var URLSearchParams = globalThis.URLSearchParams || function(init) {
    this._p = {};
    if (typeof init === 'string') {
        var s = init.charAt(0) === '?' ? init.slice(1) : init;
        var pairs = s.split('&');
        for (var i = 0; i < pairs.length; i++) {
            var eq = pairs[i].indexOf('=');
            if (eq >= 0) {
                var k = decodeURIComponent(pairs[i].slice(0, eq).replace(/\+/g, ' '));
                var v = decodeURIComponent(pairs[i].slice(eq + 1).replace(/\+/g, ' '));
                if (!this._p[k]) { this._p[k] = []; }
                this._p[k].push(v);
            }
        }
    }
    this.get = function(k) { return (this._p[k] && this._p[k].length) ? this._p[k][0] : null; };
    this.has = function(k) { return Object.prototype.hasOwnProperty.call(this._p, k); };
    this.set = function(k, v) { this._p[k] = [String(v)]; };
    this.append = function(k, v) { if (!this._p[k]) { this._p[k] = []; } this._p[k].push(String(v)); };
    this.delete = function(k) { delete this._p[k]; };
    this.toString = function() {
        var out = [];
        for (var k in this._p) {
            if (Object.prototype.hasOwnProperty.call(this._p, k)) {
                for (var i = 0; i < this._p[k].length; i++) {
                    out.push(encodeURIComponent(k) + '=' + encodeURIComponent(this._p[k][i]));
                }
            }
        }
        return out.join('&');
    };
};
"#;

/// Decrypt the YouTube CDN `n` throttling parameter using the player JS nfunc.
async fn decrypt_nsig(encrypted: &str) -> Option<String> {
    let info = player_info().await;
    let encrypted = encrypted.to_owned();

    if let Some(nfunc) = info.nfunc_js.as_deref() {
        // Self-contained snippet from delegation trace or old-style extraction.
        // Delegation trace produces `function __nfunc(n){...}`; old-style is a raw function.
        let nfunc = nfunc.to_owned();
        return tokio::task::spawn_blocking(move || {
            let rt = rquickjs::Runtime::new().ok()?;
            let ctx = rquickjs::Context::full(&rt).ok()?;
            ctx.with(|ctx| {
                let _ = ctx.eval::<rquickjs::Value, _>(BROWSER_STUBS);
                let n_json = serde_json::to_string(&encrypted).ok()?;
                let code = if nfunc.contains("__nfunc") {
                    format!("{nfunc}\n__nfunc({n_json})")
                } else {
                    format!("({nfunc})({n_json})")
                };
                ctx.eval::<rquickjs::Value, _>(code.as_str())
                    .ok()?
                    .as_string()
                    .and_then(|s| s.to_string().ok())
            })
        })
        .await
        .ok()
        .flatten();
    }

    // nClass path (Metrolist-style): eval full player JS (tolerate throw), then call
    // `new _yt_player[nclass](url, true)` — the class is set on _yt_player before any throw.
    let (Some(nclass), Some(player_js)) =
        (info.nclass.as_deref(), info.player_js.as_deref())
    else {
        tracing::warn!("no nfunc or nclass available for nsig decryption");
        return None;
    };
    let nclass = nclass.to_owned();
    let player_js = player_js.to_owned();

    tokio::task::spawn_blocking(move || {
        let rt = rquickjs::Runtime::new().ok()?;
        let ctx = rquickjs::Context::full(&rt).ok()?;
        ctx.with(|ctx| {
            let _ = ctx.eval::<rquickjs::Value, _>(BROWSER_STUBS);
            // The IIFE argument is `_yt_player`; g.* assignments inside become _yt_player.*
            // and persist even if the IIFE throws partway through.
            if let Err(_) = ctx.eval::<rquickjs::Value, _>(player_js.as_str()) {
                let exc = format_js_exception(&ctx);
                tracing::debug!(exc, "player JS threw (tolerated)");
            }
            let n_json = serde_json::to_string(&encrypted).ok()?;
            let expr = format!(
                r#"(function(n){{try{{
    var ctor=_yt_player["{nclass}"];
    if(typeof ctor!=="function")return null;
    var u=new ctor("https://x.googlevideo.com/videoplayback?n="+n,true);
    var t=u&&typeof u.get==="function"?u.get("n"):null;
    return(t&&t!==n)?t:null;
}}catch(e){{return null;}}}})({})"#,
                n_json
            );
            let result = ctx
                .eval::<rquickjs::Value, _>(expr.as_str())
                .ok()?
                .as_string()
                .and_then(|s| s.to_string().ok());
            if result.is_none() {
                tracing::warn!(nclass, "nClass decryption returned null");
            }
            result
        })
    })
    .await
    .ok()
    .flatten()
}

/// Decrypt a YouTube `signatureCipher` value (base64 or raw encrypted sig).
pub async fn decrypt_sig(encrypted: &str) -> Option<String> {
    let info = player_info().await;

    // Classic path: self-contained snippet defines __sig(a).
    if let Some(sig_js) = info.sig_js.as_deref() {
        let encrypted = encrypted.to_owned();
        let sig_js = sig_js.to_owned();
        return tokio::task::spawn_blocking(move || {
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
        .flatten();
    }

    // Modern path: inject g.__sig before IIFE close, eval full player JS, call _yt_player.__sig.
    let (Some(injection), Some(player_js)) =
        (info.sig_injection.as_deref(), info.player_js.as_deref())
    else {
        tracing::warn!("no sig_js or sig_injection available for decrypt_sig");
        return None;
    };
    let encrypted = encrypted.to_owned();
    let injection = injection.to_owned();
    let player_js = player_js.to_owned();

    tokio::task::spawn_blocking(move || {
        // Patch player JS: insert injection before `})(_yt_player)`.
        let iife_close = player_js.rfind("})(_yt_player)")?;
        let patched = format!(
            "{}{}\n{}",
            &player_js[..iife_close],
            injection,
            &player_js[iife_close..]
        );

        let rt = rquickjs::Runtime::new().ok()?;
        let ctx = rquickjs::Context::full(&rt).ok()?;
        ctx.with(|ctx| {
            let _ = ctx.eval::<rquickjs::Value, _>(BROWSER_STUBS);
            if let Err(_) = ctx.eval::<rquickjs::Value, _>(patched.as_str()) {
                let exc = format_js_exception(&ctx);
                tracing::debug!(exc, "player JS threw during sig eval (tolerated)");
            }
            let s_json = serde_json::to_string(&encrypted).ok()?;
            let expr = format!("_yt_player.__sig({s_json})");
            let result = ctx
                .eval::<rquickjs::Value, _>(expr.as_str())
                .ok()?
                .as_string()
                .and_then(|s| s.to_string().ok());
            if result.is_none() {
                tracing::warn!("sig injection call returned null/undefined");
            }
            result
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

    // ANDROID/VR/VISIONOS clients use direct streams — no n-transform needed.
    // Only WEB-family clients embed an encrypted `n` param.
    let needs_transform = parsed
        .query_pairs()
        .find(|(k, _)| k == "c")
        .map(|(_, v)| matches!(v.as_ref(), "WEB" | "WEB_REMIX" | "TVHTML5" | "TVHTML5_SIMPLY"))
        .unwrap_or(false);
    if !needs_transform {
        return url.to_owned();
    }

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
    tracing::debug!(original_n = encrypted, decrypted_n = decrypted, "nsig replacement");

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
