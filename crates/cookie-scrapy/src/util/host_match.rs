pub fn host_matches_cookie_domain(host: &str, cookie_domain: &str) -> bool {
    let normalized_host = host.to_lowercase();
    let stripped = cookie_domain.strip_prefix('.').unwrap_or(cookie_domain);
    let domain_lower = stripped.to_lowercase();
    normalized_host == domain_lower || normalized_host.ends_with(&format!(".{domain_lower}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exact_match() {
        assert!(host_matches_cookie_domain("example.com", "example.com"));
    }

    #[test]
    fn subdomain_match() {
        assert!(host_matches_cookie_domain("sub.example.com", "example.com"));
    }

    #[test]
    fn leading_dot() {
        assert!(host_matches_cookie_domain("example.com", ".example.com"));
        assert!(host_matches_cookie_domain(
            "sub.example.com",
            ".example.com"
        ));
    }

    #[test]
    fn case_insensitive() {
        assert!(host_matches_cookie_domain("Example.COM", "example.com"));
        assert!(host_matches_cookie_domain("example.com", "Example.COM"));
    }

    #[test]
    fn no_match() {
        assert!(!host_matches_cookie_domain("other.com", "example.com"));
        assert!(!host_matches_cookie_domain("notexample.com", "example.com"));
    }
}
