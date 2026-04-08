use url::Url;

pub fn normalize_origins(url_str: &str, extra_origins: Option<&[String]>) -> Vec<String> {
    let mut origins = Vec::new();

    if let Ok(parsed) = Url::parse(url_str) {
        let origin = parsed.origin().unicode_serialization();
        origins.push(ensure_trailing_slash(&origin));
    }

    if let Some(extras) = extra_origins {
        for raw in extras {
            let trimmed = raw.trim();
            if trimmed.is_empty() {
                continue;
            }
            if let Ok(parsed) = Url::parse(trimmed) {
                let origin = parsed.origin().unicode_serialization();
                origins.push(ensure_trailing_slash(&origin));
            }
        }
    }

    // Dedupe while preserving order
    let mut seen = std::collections::HashSet::new();
    origins.retain(|o| seen.insert(o.clone()));
    origins
}

fn ensure_trailing_slash(origin: &str) -> String {
    if origin.ends_with('/') {
        origin.to_string()
    } else {
        format!("{origin}/")
    }
}

pub fn extract_host(origin: &str) -> Option<String> {
    Url::parse(origin)
        .ok()
        .and_then(|u| u.host_str().map(|h| h.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn basic_origin() {
        let origins = normalize_origins("https://example.com/path", None);
        assert_eq!(origins, vec!["https://example.com/"]);
    }

    #[test]
    fn with_extras() {
        let extras = vec!["https://other.com".to_string()];
        let origins = normalize_origins("https://example.com", Some(&extras));
        assert_eq!(origins.len(), 2);
        assert!(origins.contains(&"https://example.com/".to_string()));
        assert!(origins.contains(&"https://other.com/".to_string()));
    }

    #[test]
    fn dedupes() {
        let extras = vec!["https://example.com/".to_string()];
        let origins = normalize_origins("https://example.com", Some(&extras));
        assert_eq!(origins.len(), 1);
    }

    #[test]
    fn ignores_malformed() {
        let extras = vec!["not-a-url".to_string()];
        let origins = normalize_origins("https://example.com", Some(&extras));
        assert_eq!(origins.len(), 1);
    }
}
