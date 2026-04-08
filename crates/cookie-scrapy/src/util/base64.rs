use base64::Engine;

pub fn try_decode_base64_json(input: &str) -> Option<String> {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return None;
    }

    let decoded_bytes = if trimmed.contains('-') || trimmed.contains('_') {
        base64::engine::general_purpose::URL_SAFE_NO_PAD
            .decode(trimmed)
            .or_else(|_| base64::engine::general_purpose::URL_SAFE.decode(trimmed))
            .ok()?
    } else {
        base64::engine::general_purpose::STANDARD
            .decode(trimmed)
            .or_else(|_| base64::engine::general_purpose::STANDARD_NO_PAD.decode(trimmed))
            .ok()?
    };

    let decoded = String::from_utf8(decoded_bytes).ok()?;
    let decoded = decoded.trim().to_string();
    if decoded.is_empty() {
        return None;
    }

    // Validate it's valid JSON
    serde_json::from_str::<serde_json::Value>(&decoded).ok()?;
    Some(decoded)
}

#[cfg(test)]
mod tests {
    use super::*;
    use base64::Engine;

    #[test]
    fn standard_base64() {
        let json = r#"[{"name":"foo","value":"bar"}]"#;
        let encoded = base64::engine::general_purpose::STANDARD.encode(json);
        let result = try_decode_base64_json(&encoded).unwrap();
        assert_eq!(result, json);
    }

    #[test]
    fn base64url() {
        let json = r#"[{"name":"foo","value":"bar"}]"#;
        let encoded = base64::engine::general_purpose::URL_SAFE.encode(json);
        let result = try_decode_base64_json(&encoded).unwrap();
        assert_eq!(result, json);
    }

    #[test]
    fn invalid_base64() {
        assert!(try_decode_base64_json("not-valid-base64!!!").is_none());
    }

    #[test]
    fn not_json() {
        let encoded = base64::engine::general_purpose::STANDARD.encode("this is not json");
        assert!(try_decode_base64_json(&encoded).is_none());
    }

    #[test]
    fn empty_input() {
        assert!(try_decode_base64_json("").is_none());
        assert!(try_decode_base64_json("  ").is_none());
    }
}
