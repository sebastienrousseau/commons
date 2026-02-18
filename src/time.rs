//! Time handling and duration utilities.

use std::time::{Duration, SystemTime, UNIX_EPOCH};

/// Get the current Unix timestamp in seconds
pub fn unix_timestamp() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

/// Get the current Unix timestamp in milliseconds
pub fn unix_timestamp_millis() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

/// Format a duration in a human-readable way
pub fn format_duration(duration: Duration) -> String {
    let secs = duration.as_secs();
    let millis = duration.subsec_millis();

    if secs >= 86400 {
        let days = secs / 86400;
        let hours = (secs % 86400) / 3600;
        format!("{}d {}h", days, hours)
    } else if secs >= 3600 {
        let hours = secs / 3600;
        let minutes = (secs % 3600) / 60;
        format!("{}h {}m", hours, minutes)
    } else if secs >= 60 {
        let minutes = secs / 60;
        let seconds = secs % 60;
        format!("{}m {}s", minutes, seconds)
    } else if secs > 0 {
        format!("{}.{:03}s", secs, millis)
    } else {
        format!("{}ms", millis)
    }
}

/// Parse a duration from a human-readable string
pub fn parse_duration(s: &str) -> Result<Duration, String> {
    let s = s.trim();

    if s.ends_with("ms") {
        let num: u64 = s[..s.len()-2].parse()
            .map_err(|_| "Invalid milliseconds format")?;
        Ok(Duration::from_millis(num))
    } else if s.ends_with('s') {
        let num: f64 = s[..s.len()-1].parse()
            .map_err(|_| "Invalid seconds format")?;
        Ok(Duration::from_secs_f64(num))
    } else if s.ends_with('m') {
        let num: u64 = s[..s.len()-1].parse()
            .map_err(|_| "Invalid minutes format")?;
        Ok(Duration::from_secs(num * 60))
    } else if s.ends_with('h') {
        let num: u64 = s[..s.len()-1].parse()
            .map_err(|_| "Invalid hours format")?;
        Ok(Duration::from_secs(num * 3600))
    } else if s.ends_with('d') {
        let num: u64 = s[..s.len()-1].parse()
            .map_err(|_| "Invalid days format")?;
        Ok(Duration::from_secs(num * 86400))
    } else {
        // Assume seconds if no suffix
        let num: f64 = s.parse()
            .map_err(|_| "Invalid duration format")?;
        Ok(Duration::from_secs_f64(num))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_duration() {
        assert_eq!(parse_duration("100ms").unwrap(), Duration::from_millis(100));
        assert_eq!(parse_duration("5s").unwrap(), Duration::from_secs(5));
        assert_eq!(parse_duration("2m").unwrap(), Duration::from_secs(120));
        assert_eq!(parse_duration("1h").unwrap(), Duration::from_secs(3600));
        assert_eq!(parse_duration("1d").unwrap(), Duration::from_secs(86400));
    }

    #[test]
    fn test_format_duration() {
        assert_eq!(format_duration(Duration::from_millis(500)), "500ms");
        assert_eq!(format_duration(Duration::from_secs(5)), "5.000s");
        assert_eq!(format_duration(Duration::from_secs(65)), "1m 5s");
        assert_eq!(format_duration(Duration::from_secs(3665)), "1h 1m");
    }
}