//! ID generation utilities.
//!
//! Provides various ID generation strategies including timestamp-based,
//! random, and sortable IDs.
//!
//! # Example
//!
//! ```rust
//! use commons::id::{generate_id, IdFormat};
//!
//! let id = generate_id(IdFormat::Timestamp);
//! println!("Generated ID: {}", id);
//! ```

// the deprecated ID generators are deprecated in 0.0.8 and refer to one another here
// (impls, constructors, tests). The deprecation is for callers outside
// this module; it is removed together with them.
#![allow(deprecated)]

use crate::counter::Counter;
use std::fmt::Write;
use std::sync::atomic::Ordering;
use std::time::{SystemTime, UNIX_EPOCH};

/// Counter for timestamp-based ID uniqueness within the same millisecond.
static TIMESTAMP_COUNTER: Counter = Counter::new(0);

/// ID format options.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[deprecated(
    since = "0.0.8",
    note = "no known user; use the uuid crate (v4 or v7); generate_random_hex stays"
)]
pub enum IdFormat {
    /// Timestamp-based ID (sortable, 20 chars).
    Timestamp,
    /// Random hex ID (32 chars).
    RandomHex,
    /// Short random ID (12 chars, base62).
    Short,
    /// Prefixed ID with custom prefix.
    Prefixed,
}

/// Generate a unique ID.
///
/// # Arguments
///
/// * `format` - The ID format to use
///
/// # Returns
///
/// A unique string ID.
#[must_use]
#[deprecated(
    since = "0.0.8",
    note = "no known user; use the uuid crate (v4 or v7); generate_random_hex stays"
)]
pub fn generate_id(format: IdFormat) -> String {
    match format {
        IdFormat::RandomHex => generate_random_hex(),
        IdFormat::Short => generate_short_id(),
        IdFormat::Timestamp | IdFormat::Prefixed => generate_timestamp_id(),
    }
}

/// Generate a prefixed ID.
///
/// # Arguments
///
/// * `prefix` - Prefix string (e.g., "usr", "ord")
///
/// # Returns
///
/// A prefixed unique ID like `usr_abc123`.
#[must_use]
#[deprecated(
    since = "0.0.8",
    note = "no known user; use the uuid crate (v4 or v7); generate_random_hex stays"
)]
pub fn generate_prefixed_id(prefix: &str) -> String {
    format!("{prefix}_{}", generate_short_id())
}

/// Generate a timestamp-based sortable ID.
///
/// Format: 13 digits timestamp + 7 digits counter = 20 chars
/// IDs generated in the same millisecond are still unique and sortable.
#[must_use]
#[deprecated(
    since = "0.0.8",
    note = "no known user; use the uuid crate (v4 or v7); generate_random_hex stays"
)]
pub fn generate_timestamp_id() -> String {
    let timestamp = current_timestamp_millis();
    let counter = TIMESTAMP_COUNTER.fetch_add(1, Ordering::SeqCst) % 10_000_000;
    format!("{timestamp:013}{counter:07}")
}

/// Generate a random hexadecimal ID (32 characters, 128 random bits).
///
/// The bits come from the operating system's random number generator, so
/// IDs are unique with overwhelming probability across processes and
/// machines. They are suitable as W3C Trace Context IDs.
///
/// # Panics
///
/// Panics if the operating system's random number generator fails.
#[must_use]
pub fn generate_random_hex() -> String {
    let mut bytes = [0u8; 16];
    fill_random_bytes(&mut bytes);
    bytes.iter().fold(String::with_capacity(32), |mut s, b| {
        let _ = write!(s, "{b:02x}");
        s
    })
}

/// Generate a short random ID (12 characters, base62).
///
/// Each character is drawn uniformly from `[0-9A-Za-z]` (about 71 bits in
/// all) by rejection sampling, so no character is more likely than another.
///
/// # Panics
///
/// Panics if the operating system's random number generator fails.
#[must_use]
#[deprecated(
    since = "0.0.8",
    note = "no known user; use the uuid crate (v4 or v7); generate_random_hex stays"
)]
pub fn generate_short_id() -> String {
    const CHARS: &[u8] = b"0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz";
    // 248 = 4 * 62: bytes at or above it would favour the first characters.
    const LIMIT: u8 = 248;
    let mut id = String::with_capacity(12);
    let mut buf = [0u8; 16];
    while id.len() < 12 {
        fill_random_bytes(&mut buf);
        for &b in buf.iter().filter(|&&b| b < LIMIT) {
            if id.len() == 12 {
                break;
            }
            id.push(CHARS[usize::from(b) % CHARS.len()] as char);
        }
    }
    id
}

/// Generate a random UUID version 4 string.
///
/// The 122 random bits come from the operating system's random number
/// generator.
///
/// # Panics
///
/// Panics if the operating system's random number generator fails.
#[must_use]
#[deprecated(
    since = "0.0.8",
    note = "no known user; use the uuid crate (v4 or v7); generate_random_hex stays"
)]
pub fn generate_uuid_like() -> String {
    let mut bytes = [0u8; 16];
    fill_random_bytes(&mut bytes);

    // Set version (4) and variant bits
    bytes[6] = (bytes[6] & 0x0f) | 0x40;
    bytes[8] = (bytes[8] & 0x3f) | 0x80;

    format!(
        "{:02x}{:02x}{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}{:02x}{:02x}{:02x}{:02x}",
        bytes[0],
        bytes[1],
        bytes[2],
        bytes[3],
        bytes[4],
        bytes[5],
        bytes[6],
        bytes[7],
        bytes[8],
        bytes[9],
        bytes[10],
        bytes[11],
        bytes[12],
        bytes[13],
        bytes[14],
        bytes[15]
    )
}

/// Get current timestamp in milliseconds.
#[must_use]
#[allow(clippy::cast_possible_truncation)]
#[deprecated(
    since = "0.0.8",
    note = "no known user; use the uuid crate (v4 or v7); generate_random_hex stays"
)]
pub fn current_timestamp_millis() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

/// Fill a byte slice from the operating system's random number generator.
///
/// The previous implementation seeded a xorshift from the time, a counter,
/// the process id and the thread id through a fixed-key hasher, so two
/// processes with the same pid (for example containers running as pid 1)
/// produced the same IDs in the same millisecond.
fn fill_random_bytes(bytes: &mut [u8]) {
    getrandom::fill(bytes).expect("the operating system's random number generator failed");
}

/// ID generator with configuration.
#[derive(Debug, Clone)]
#[deprecated(
    since = "0.0.8",
    note = "no known user; use the uuid crate (v4 or v7); generate_random_hex stays"
)]
pub struct IdGenerator {
    prefix: Option<String>,
    format: IdFormat,
}

impl IdGenerator {
    /// Create a new ID generator.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Set a prefix for generated IDs.
    #[must_use]
    pub fn with_prefix(mut self, prefix: impl Into<String>) -> Self {
        self.prefix = Some(prefix.into());
        self
    }

    /// Set the ID format.
    #[must_use]
    pub const fn with_format(mut self, format: IdFormat) -> Self {
        self.format = format;
        self
    }

    /// Generate an ID.
    #[must_use]
    pub fn generate(&self) -> String {
        let id = generate_id(self.format);
        match &self.prefix {
            Some(p) => format!("{p}_{id}"),
            None => id,
        }
    }
}

impl Default for IdGenerator {
    fn default() -> Self {
        Self {
            prefix: None,
            format: IdFormat::Short,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    #[test]
    fn random_hex_ids_do_not_repeat() {
        let ids: HashSet<String> = (0..100_000).map(|_| generate_random_hex()).collect();
        assert_eq!(ids.len(), 100_000);
    }

    #[test]
    fn random_bytes_cover_every_value() {
        // 64 KiB of OS randomness: each byte value is expected 256 times,
        // so a missing value means the source is broken, not unlucky.
        let mut seen = [false; 256];
        let mut buf = [0u8; 4096];
        for _ in 0..16 {
            fill_random_bytes(&mut buf);
            for &b in &buf {
                seen[usize::from(b)] = true;
            }
        }
        assert!(seen.iter().all(|&s| s), "some byte values never appeared");
    }

    #[test]
    fn short_ids_use_every_base62_character() {
        let mut counts = std::collections::HashMap::new();
        for _ in 0..20_000 {
            for c in generate_short_id().chars() {
                *counts.entry(c).or_insert(0u32) += 1;
            }
        }
        // 240,000 characters over 62 symbols: about 3,871 each.
        assert_eq!(counts.len(), 62);
        let (min, max) = (
            counts.values().min().unwrap(),
            counts.values().max().unwrap(),
        );
        assert!(
            *min > 3_400 && *max < 4_350,
            "skewed base62 distribution: {min}..{max}"
        );
    }

    #[test]
    fn test_timestamp_id_format() {
        let id = generate_timestamp_id();
        assert_eq!(id.len(), 20);
        assert!(id.chars().all(|c| c.is_ascii_digit()));
    }

    #[test]
    fn test_timestamp_ids_are_sortable() {
        let id1 = generate_timestamp_id();
        std::thread::sleep(std::time::Duration::from_millis(1));
        let id2 = generate_timestamp_id();
        assert!(id1 < id2);
    }

    #[test]
    fn test_timestamp_ids_unique_in_same_ms() {
        let ids: Vec<String> = (0..100).map(|_| generate_timestamp_id()).collect();
        let unique: HashSet<_> = ids.iter().collect();
        assert_eq!(ids.len(), unique.len());
    }

    #[test]
    fn test_random_hex_format() {
        let id = generate_random_hex();
        assert_eq!(id.len(), 32);
        assert!(id.chars().all(|c| c.is_ascii_hexdigit()));
    }

    #[test]
    fn test_short_id_format() {
        let id = generate_short_id();
        assert_eq!(id.len(), 12);
        assert!(id.chars().all(|c| c.is_ascii_alphanumeric()));
    }

    #[test]
    fn test_uuid_like_format() {
        let id = generate_uuid_like();
        assert_eq!(id.len(), 36);
        assert_eq!(id.chars().filter(|&c| c == '-').count(), 4);
    }

    #[test]
    fn test_prefixed_id() {
        let id = generate_prefixed_id("usr");
        assert!(id.starts_with("usr_"));
        assert_eq!(id.len(), 4 + 12); // "usr_" + 12 char short id
    }

    #[test]
    fn test_id_generator() {
        let generator = IdGenerator::new()
            .with_prefix("order")
            .with_format(IdFormat::Short);

        let id = generator.generate();
        assert!(id.starts_with("order_"));
    }

    #[test]
    fn test_uniqueness() {
        let ids: HashSet<String> = (0..1000).map(|_| generate_short_id()).collect();
        assert_eq!(ids.len(), 1000);
    }

    #[test]
    fn test_counter_isolation() {
        // Heavy random ID generation should not exhaust the timestamp counter domain.
        let _random_ids: Vec<String> = (0..1000).map(|_| generate_random_hex()).collect();

        // Timestamp IDs should still be unique after heavy random generation.
        let ts_ids: HashSet<String> = (0..100).map(|_| generate_timestamp_id()).collect();
        assert_eq!(ts_ids.len(), 100);
    }

    #[test]
    fn test_generate_id_random_hex_format() {
        let id = generate_id(IdFormat::RandomHex);
        assert_eq!(id.len(), 32);
        assert!(id.chars().all(|c| c.is_ascii_hexdigit()));
    }

    #[test]
    fn test_generate_id_timestamp_format() {
        let id = generate_id(IdFormat::Timestamp);
        assert_eq!(id.len(), 20);
        assert!(id.chars().all(|c| c.is_ascii_digit()));
    }

    #[test]
    fn test_generate_id_prefixed_format() {
        // IdFormat::Prefixed uses the same path as Timestamp
        let id = generate_id(IdFormat::Prefixed);
        assert_eq!(id.len(), 20);
        assert!(id.chars().all(|c| c.is_ascii_digit()));
    }

    #[test]
    fn test_id_generator_without_prefix() {
        let generator = IdGenerator::new().with_format(IdFormat::RandomHex);
        let id = generator.generate();
        // No prefix, just the raw hex
        assert_eq!(id.len(), 32);
    }
}
