// Copyright © 2024-2026 RustLogs (RLG). All rights reserved.
// SPDX-License-Identifier: Apache-2.0
// SPDX-License-Identifier: MIT

//! Utility functions for the logging pipeline.

use super::log_error::LoggingResult;
use dtt::datetime::DateTime;

#[cfg(feature = "logging-tokio")]
use std::path::Path;

#[cfg(feature = "logging-tokio")]
use tokio::fs::{self, File, OpenOptions};
#[cfg(feature = "logging-tokio")]
use tokio::io::{AsyncReadExt, AsyncSeekExt, AsyncWriteExt};

/// Generates a timestamp string in ISO 8601 format.
///
/// # Returns
///
/// A `String` containing the current timestamp in ISO 8601 format.
#[must_use]
pub fn generate_timestamp() -> String {
    DateTime::new().to_string()
}

/// Sanitizes a string for use in log messages.
///
/// This function replaces newlines and control characters with spaces.
///
/// # Arguments
///
/// * `message` - A string slice that holds the message to be sanitized.
///
/// # Returns
///
/// A `String` with sanitized content.
///
/// # Examples
///
/// ```
/// use commons::logging::utils::sanitize_log_message;
///
/// let message = "Hello\nWorld\r\u{0007}";
/// let sanitized = sanitize_log_message(message);
/// assert_eq!(sanitized, "Hello World  ");
/// ```
#[must_use]
pub fn sanitize_log_message(message: &str) -> String {
    message
        .replace(['\n', '\r'], " ")
        .replace(|c: char| c.is_control(), " ")
}

/// Checks if a file exists and is writable.
///
/// # Arguments
///
/// * `path` - A reference to a `Path` that holds the file path to check.
///
/// # Returns
///
/// A `LoggingResult<bool>` which is `Ok(true)` if the file exists and is writable,
/// `Ok(false)` otherwise, or an error if the operation fails.
///
/// # Errors
///
/// This function returns an error if the file metadata cannot be read.
#[cfg(feature = "logging-tokio")]
pub async fn is_file_writable(path: &Path) -> LoggingResult<bool> {
    if path.exists() {
        let metadata = fs::metadata(path).await?;
        Ok(metadata.is_file() && !metadata.permissions().readonly())
    } else {
        // If the file doesn't exist, check if we can create it
        match File::create(path).await {
            Ok(_) => {
                fs::remove_file(path).await?;
                Ok(true)
            }
            Err(_) => Ok(false),
        }
    }
}

/// Truncates the file at the given path to the specified size.
///
/// # Arguments
///
/// * `path` - A reference to a `Path` that holds the file path to truncate.
/// * `size` - The size (in bytes) to truncate the file to.
///
/// # Returns
///
/// A `std::io::Result<()>` which is `Ok(())` if the operation succeeds,
/// or an error if it fails.
///
/// # Errors
///
/// This function returns an error if the file cannot be opened, or if
/// the seek or write operations fail.
#[cfg(feature = "logging-tokio")]
pub async fn truncate_file(
    path: &Path,
    size: u64,
) -> std::io::Result<()> {
    let mut file = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(path)
        .await?;

    let file_size = file.metadata().await?.len();

    if size < file_size {
        // Read the content
        // SAFETY: Casting size to usize is safe here as we're truncating to a size that fits in memory for this operation.
        #[allow(clippy::cast_possible_truncation)]
        let mut content = vec![0; size as usize];
        file.read_exact(&mut content).await?;

        // Seek to the beginning of the file
        file.seek(std::io::SeekFrom::Start(0)).await?;

        // Write the truncated content
        file.write_all(&content).await?;
    }

    // Set the file length
    file.set_len(size).await?;

    Ok(())
}

/// Formats a file size in a human-readable format.
///
/// # Arguments
///
/// * `size` - The file size in bytes.
///
/// # Returns
///
/// A `String` containing the formatted file size.
///
/// # Examples
///
/// ```
/// use commons::logging::utils::format_file_size;
///
/// let size = 1_500_000;
/// let formatted = format_file_size(size);
/// assert_eq!(formatted, "1.43 MB");
/// ```
#[must_use]
pub fn format_file_size(size: u64) -> String {
    const UNITS: [&str; 6] = ["B", "KB", "MB", "GB", "TB", "PB"];
    // SAFETY: Loss of precision is acceptable for human-readable file size formatting.
    #[allow(clippy::cast_precision_loss)]
    let mut size_f = size as f64;
    let mut unit_index = 0;

    while size_f >= 1024.0 && unit_index < UNITS.len() - 1 {
        size_f /= 1024.0;
        unit_index += 1;
    }

    format!("{size_f:.2} {unit}", unit = UNITS[unit_index])
}

/// Parses a datetime string in ISO 8601 format.
///
/// # Arguments
///
/// * `datetime_str` - A string slice containing the datetime in ISO 8601 format.
///
/// # Returns
///
/// A `LoggingResult<DateTime>` which is `Ok(DateTime)` if parsing succeeds,
/// or an error if parsing fails.
///
/// # Errors
///
/// This function returns an error if the datetime string cannot be parsed.
pub fn parse_datetime(datetime_str: &str) -> LoggingResult<DateTime> {
    DateTime::parse(datetime_str)
        .map_err(|e| super::log_error::LoggingError::custom(e.to_string()))
}

/// Generates a highly unique, 16-character pseudo-random hex string suitable for OTLP span IDs.
///
/// # Returns
/// A `String` containing the span ID.
#[cfg(feature = "id")]
#[must_use]
pub fn generate_span_id() -> String {
    crate::id::generate_random_hex()[..16].to_string()
}

/// Generates a highly unique, 32-character pseudo-random hex string suitable for OTLP trace IDs.
///
/// # Returns
/// A `String` containing the trace ID.
#[cfg(feature = "id")]
#[must_use]
pub fn generate_trace_id() -> String {
    crate::id::generate_random_hex()
}

/// Checks if a directory is writable.
///
/// # Arguments
///
/// * `path` - A reference to a `Path` that holds the directory path to check.
///
/// # Returns
///
/// A `LoggingResult<bool>` which is `Ok(true)` if the directory is writable,
/// `Ok(false)` otherwise, or an error if the operation fails.
///
/// # Errors
///
/// This function returns an error if the temporary file used for testing writability cannot be removed.
#[cfg(feature = "logging-tokio")]
pub async fn is_directory_writable(path: &Path) -> LoggingResult<bool> {
    if !path.is_dir() {
        return Ok(false);
    }

    let test_file = path.join(".logging_write_test");
    match File::create(&test_file).await {
        Ok(_) => {
            fs::remove_file(&test_file).await?;
            Ok(true)
        }
        Err(_) => Ok(false),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ---- generate_timestamp ----

    #[test]
    fn test_generate_timestamp_non_empty() {
        let ts = generate_timestamp();
        assert!(!ts.is_empty());
    }

    #[test]
    fn test_generate_timestamp_contains_date_like_content() {
        let ts = generate_timestamp();
        // ISO 8601 timestamps contain hyphens (date separators) and colons (time separators).
        assert!(ts.contains('-'), "timestamp should contain date separators: {ts}");
    }

    // ---- sanitize_log_message ----

    #[test]
    fn test_sanitize_replaces_newlines() {
        let msg = "line1\nline2\nline3";
        let result = sanitize_log_message(msg);
        assert!(!result.contains('\n'));
        assert_eq!(result, "line1 line2 line3");
    }

    #[test]
    fn test_sanitize_replaces_carriage_returns() {
        let msg = "hello\rworld";
        let result = sanitize_log_message(msg);
        assert!(!result.contains('\r'));
        assert_eq!(result, "hello world");
    }

    #[test]
    fn test_sanitize_replaces_control_chars() {
        // \x07 is BEL, \x01 is SOH
        let msg = "a\x07b\x01c";
        let result = sanitize_log_message(msg);
        assert!(!result.chars().any(char::is_control));
        assert_eq!(result, "a b c");
    }

    #[test]
    fn test_sanitize_clean_input_unchanged() {
        let msg = "This is a clean message.";
        let result = sanitize_log_message(msg);
        assert_eq!(result, msg);
    }

    #[test]
    fn test_sanitize_empty_string() {
        let result = sanitize_log_message("");
        assert!(result.is_empty());
    }

    // ---- format_file_size ----

    #[test]
    fn test_format_file_size_bytes() {
        let result = format_file_size(500);
        assert_eq!(result, "500.00 B");
    }

    #[test]
    fn test_format_file_size_zero() {
        let result = format_file_size(0);
        assert_eq!(result, "0.00 B");
    }

    #[test]
    fn test_format_file_size_kb() {
        // 1 KB = 1024 bytes
        let result = format_file_size(1024);
        assert_eq!(result, "1.00 KB");
    }

    #[test]
    fn test_format_file_size_mb() {
        // 1 MB = 1024 * 1024
        let result = format_file_size(1_048_576);
        assert_eq!(result, "1.00 MB");
    }

    #[test]
    fn test_format_file_size_gb() {
        // 1 GB = 1024^3
        let result = format_file_size(1_073_741_824);
        assert_eq!(result, "1.00 GB");
    }

    #[test]
    fn test_format_file_size_tb() {
        // 1 TB = 1024^4
        let result = format_file_size(1_099_511_627_776);
        assert_eq!(result, "1.00 TB");
    }

    #[test]
    fn test_format_file_size_fractional_mb() {
        // 1_500_000 bytes = ~1.43 MB (documented in doctest)
        let result = format_file_size(1_500_000);
        assert_eq!(result, "1.43 MB");
    }

    // ---- parse_datetime ----

    #[test]
    fn test_parse_datetime_valid_iso8601() {
        let result = parse_datetime("2024-01-15T10:30:00+00:00");
        assert!(result.is_ok(), "Expected Ok for valid ISO 8601, got {result:?}");
    }

    #[test]
    fn test_parse_datetime_invalid_string() {
        let result = parse_datetime("not-a-date");
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_datetime_empty_string() {
        let result = parse_datetime("");
        assert!(result.is_err());
    }

    // ---- generate_span_id (behind "id" feature) ----

    #[cfg(feature = "id")]
    #[test]
    fn test_generate_span_id_length() {
        let id = generate_span_id();
        assert_eq!(id.len(), 16, "span ID should be 16 characters, got {}", id.len());
    }

    #[cfg(feature = "id")]
    #[test]
    fn test_generate_span_id_hex() {
        let id = generate_span_id();
        assert!(
            id.chars().all(|c| c.is_ascii_hexdigit()),
            "span ID should be hex: {id}"
        );
    }

    #[cfg(feature = "id")]
    #[test]
    fn test_generate_span_id_unique() {
        let id1 = generate_span_id();
        let id2 = generate_span_id();
        assert_ne!(id1, id2, "two span IDs should differ");
    }

    // ---- generate_trace_id (behind "id" feature) ----

    #[cfg(feature = "id")]
    #[test]
    fn test_generate_trace_id_length() {
        let id = generate_trace_id();
        assert_eq!(id.len(), 32, "trace ID should be 32 characters, got {}", id.len());
    }

    #[cfg(feature = "id")]
    #[test]
    fn test_generate_trace_id_hex() {
        let id = generate_trace_id();
        assert!(
            id.chars().all(|c| c.is_ascii_hexdigit()),
            "trace ID should be hex: {id}"
        );
    }

    #[cfg(feature = "id")]
    #[test]
    fn test_generate_trace_id_unique() {
        let id1 = generate_trace_id();
        let id2 = generate_trace_id();
        assert_ne!(id1, id2, "two trace IDs should differ");
    }

    // ---- Async utility tests (logging-tokio feature) ----

    #[cfg(feature = "logging-tokio")]
    #[tokio::test]
    async fn test_is_file_writable_existing_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("writable.txt");
        fs::write(&path, "test").await.unwrap();
        assert!(is_file_writable(&path).await.unwrap());
    }

    #[cfg(feature = "logging-tokio")]
    #[tokio::test]
    async fn test_is_file_writable_nonexistent_creates() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("new_file.txt");
        assert!(is_file_writable(&path).await.unwrap());
        // File should be cleaned up after the check
        assert!(!path.exists());
    }

    #[cfg(feature = "logging-tokio")]
    #[tokio::test]
    async fn test_truncate_file_smaller() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("truncate.txt");
        fs::write(&path, "hello world this is a long string")
            .await
            .unwrap();
        truncate_file(&path, 5).await.unwrap();
        let content = fs::read_to_string(&path).await.unwrap();
        assert_eq!(content.len(), 5);
        assert_eq!(content, "hello");
    }

    #[cfg(feature = "logging-tokio")]
    #[tokio::test]
    async fn test_truncate_file_larger_than_content() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("truncate2.txt");
        fs::write(&path, "short").await.unwrap();
        truncate_file(&path, 100).await.unwrap();
        let metadata = fs::metadata(&path).await.unwrap();
        assert_eq!(metadata.len(), 100);
    }

    #[cfg(feature = "logging-tokio")]
    #[tokio::test]
    async fn test_is_directory_writable() {
        let dir = tempfile::tempdir().unwrap();
        assert!(is_directory_writable(dir.path()).await.unwrap());
    }

    #[cfg(feature = "logging-tokio")]
    #[tokio::test]
    async fn test_is_directory_writable_not_a_dir() {
        let dir = tempfile::tempdir().unwrap();
        let file_path = dir.path().join("not_a_dir.txt");
        fs::write(&file_path, "test").await.unwrap();
        assert!(!is_directory_writable(&file_path).await.unwrap());
    }

    #[cfg(feature = "logging-tokio")]
    #[tokio::test]
    async fn test_truncate_file_exact_size() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("exact.txt");
        fs::write(&path, "12345").await.unwrap();
        truncate_file(&path, 5).await.unwrap();
        let metadata = fs::metadata(&path).await.unwrap();
        assert_eq!(metadata.len(), 5);
    }

    #[cfg(feature = "logging-tokio")]
    #[tokio::test]
    async fn test_truncate_file_nonexistent_creates() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("new_truncate.txt");
        // truncate_file opens with create(true), so it should
        // create the file and set it to the specified size.
        truncate_file(&path, 10).await.unwrap();
        let metadata = fs::metadata(&path).await.unwrap();
        assert_eq!(metadata.len(), 10);
    }
}
