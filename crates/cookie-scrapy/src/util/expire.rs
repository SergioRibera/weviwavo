const WINDOWS_EPOCH_DELTA_SECONDS: i64 = 11_644_473_600;

pub fn normalize_expiration(expires: i64) -> Option<i64> {
    if expires <= 0 {
        return None;
    }
    // Chromium uses microseconds since 1601 (Windows epoch) in sqlite stores.
    if expires > 10_000_000_000_000 {
        return Some(expires / 1_000_000 - WINDOWS_EPOCH_DELTA_SECONDS);
    }
    // Milliseconds epoch
    if expires > 10_000_000_000 {
        return Some(expires / 1000);
    }
    // Seconds epoch
    Some(expires)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn zero_returns_none() {
        assert_eq!(normalize_expiration(0), None);
    }

    #[test]
    fn negative_returns_none() {
        assert_eq!(normalize_expiration(-1), None);
    }

    #[test]
    fn seconds_epoch() {
        assert_eq!(normalize_expiration(1_700_000_000), Some(1_700_000_000));
    }

    #[test]
    fn milliseconds_epoch() {
        assert_eq!(normalize_expiration(1_700_000_000_000), Some(1_700_000_000));
    }

    #[test]
    fn windows_epoch_microseconds() {
        // Chrome's expires_utc for a date around 2024
        let chrome_value: i64 = 13_350_000_000_000_000;
        let result = normalize_expiration(chrome_value).unwrap();
        // Should be a reasonable Unix timestamp (around 2023)
        assert!(result > 1_600_000_000);
        assert!(result < 2_000_000_000);
    }
}
