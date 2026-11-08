use chrono::{DateTime, SecondsFormat, TimeZone, Utc};

/// Current time as Unix milliseconds.
pub fn now_ms() -> i64 {
    Utc::now().timestamp_millis()
}

/// Formats Unix milliseconds as `2024-01-02T03:04:05.678Z`. Zero is formatted as
/// an empty string, which is how unset timestamps appear in serialized items.
pub fn ms_to_iso(ms: i64) -> String {
    if ms == 0 {
        return String::new();
    }
    Utc.timestamp_millis_opt(ms)
        .single()
        .map(|d| d.to_rfc3339_opts(SecondsFormat::Millis, true))
        .unwrap_or_default()
}

/// Parses an ISO 8601 timestamp into Unix milliseconds.
pub fn iso_to_ms(s: &str) -> Option<i64> {
    let s = s.trim();
    if s.is_empty() {
        return Some(0);
    }
    DateTime::parse_from_rfc3339(s)
        .ok()
        .map(|d| d.timestamp_millis())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trip() {
        let ms = 1_700_000_000_123;
        let iso = ms_to_iso(ms);
        assert_eq!(iso, "2023-11-14T22:13:20.123Z");
        assert_eq!(iso_to_ms(&iso), Some(ms));
        assert_eq!(ms_to_iso(0), "");
        assert_eq!(iso_to_ms(""), Some(0));
    }
}
