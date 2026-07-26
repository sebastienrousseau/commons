// Copyright © 2024-2026 RustLogs (RLG). All rights reserved.
// SPDX-License-Identifier: Apache-2.0
// SPDX-License-Identifier: MIT

//! Error types for the logging pipeline.

use super::log_config::LoggingConfigError;
#[cfg(feature = "logging-miette")]
use miette::Diagnostic;
use std::fmt;
use std::io;
use thiserror::Error;

/// Error variants for the logging pipeline.
#[derive(Error, Debug)]
#[cfg_attr(feature = "logging-miette", derive(Diagnostic))]
pub enum LoggingError {
    /// I/O error
    #[error("I/O error: {0}")]
    #[cfg_attr(
        feature = "logging-miette",
        diagnostic(
            code(logging::io_error),
            help("Ensure the log directory exists and is writable.")
        )
    )]
    IoError(#[from] io::Error),

    /// Configuration error
    #[error("Configuration error: {0}")]
    #[cfg_attr(
        feature = "logging-miette",
        diagnostic(
            code(logging::config_error),
            help("Check your configuration file or environment variables.")
        )
    )]
    ConfigError(#[from] LoggingConfigError),

    /// Log format parse error
    #[error("Log format parse error: {0}")]
    #[cfg_attr(
        feature = "logging-miette",
        diagnostic(
            code(logging::format_parse_error),
            help("Ensure the format string matches supported variants (JSON, OTLP, MCP, etc.).")
        )
    )]
    FormatParseError(String),

    /// Log level parse error
    #[error("Log level parse error: {0}")]
    #[cfg_attr(
        feature = "logging-miette",
        diagnostic(
            code(logging::level_parse_error),
            help("Supported levels: ALL, TRACE, DEBUG, INFO, WARN, ERROR, FATAL.")
        )
    )]
    LevelParseError(String),

    /// Unsupported log format
    #[error("Unsupported log format: {0}")]
    #[cfg_attr(
        feature = "logging-miette",
        diagnostic(
            code(logging::unsupported_format),
            help("Visit docs.rs/euxis-commons for a list of supported industry formats.")
        )
    )]
    UnsupportedFormat(String),

    /// Log formatting error
    #[error("Log formatting error: {0}")]
    #[cfg_attr(
        feature = "logging-miette",
        diagnostic(
            code(logging::formatting_error),
            help("This may happen if attributes contain non-serializable data.")
        )
    )]
    FormattingError(String),

    /// Log rotation error
    #[error("Log rotation error: {0}")]
    #[cfg_attr(
        feature = "logging-miette",
        diagnostic(
            code(logging::rotation_error),
            help("Ensure the logger has permission to rename or delete old log files.")
        )
    )]
    RotationError(String),

    /// Network error
    #[error("Network error: {0}")]
    #[cfg_attr(
        feature = "logging-miette",
        diagnostic(
            code(logging::network_error),
            help("Check your network connection or the OTLP collector endpoint.")
        )
    )]
    NetworkError(String),

    /// `DateTime` parse error
    #[error("DateTime parse error: {0}")]
    #[cfg_attr(
        feature = "logging-miette",
        diagnostic(
            code(logging::datetime_parse_error),
            help("Expects RFC 3339 / ISO 8601 timestamps.")
        )
    )]
    DateTimeParseError(String),

    /// Custom error
    #[error("{0}")]
    #[cfg_attr(feature = "logging-miette", diagnostic(code(logging::custom_error)))]
    Custom(String),

    /// Native OS sink failure
    #[error("Native OS sink failure: {0}")]
    #[cfg_attr(
        feature = "logging-miette",
        diagnostic(
            code(logging::native_sink_failure),
            help(
                "Check if systemd-journald is running (Linux). Ensure RLG_FALLBACK_STDOUT is set to bypass native hooks."
            )
        )
    )]
    NativeSinkError(String),
}

impl From<crate::error::CommonError> for LoggingError {
    fn from(err: crate::error::CommonError) -> Self {
        Self::Custom(err.to_string())
    }
}

impl LoggingError {
    /// Create a custom error with the given message.
    #[must_use]
    pub fn custom<T: fmt::Display>(msg: T) -> Self {
        Self::Custom(msg.to_string())
    }
}

/// Convenience alias: `Result<T, LoggingError>`.
pub type LoggingResult<T> = Result<T, LoggingError>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_display() {
        let err = LoggingError::FormatParseError("Invalid format".to_string());
        assert_eq!(err.to_string(), "Log format parse error: Invalid format");
    }

    #[test]
    fn test_custom_error() {
        let err = LoggingError::custom("Custom error message");
        assert_eq!(err.to_string(), "Custom error message");
    }

    #[test]
    fn test_common_error_conversion() {
        let common_err = crate::error::CommonError::custom("test");
        let logging_err: LoggingError = common_err.into();
        assert!(matches!(logging_err, LoggingError::Custom(_)));
        assert!(logging_err.to_string().contains("test"));
    }

    #[test]
    fn test_io_error_variant() {
        let io_err = io::Error::new(io::ErrorKind::NotFound, "file missing");
        let logging_err: LoggingError = io_err.into();
        assert!(matches!(logging_err, LoggingError::IoError(_)));
        assert!(logging_err.to_string().contains("file missing"));
    }

    #[test]
    fn test_all_variant_display() {
        let variants: Vec<LoggingError> = vec![
            LoggingError::IoError(io::Error::other("test")),
            LoggingError::ConfigError(LoggingConfigError::ValidationError("v".into())),
            LoggingError::FormatParseError("f".into()),
            LoggingError::LevelParseError("l".into()),
            LoggingError::UnsupportedFormat("u".into()),
            LoggingError::FormattingError("fm".into()),
            LoggingError::RotationError("r".into()),
            LoggingError::NetworkError("n".into()),
            LoggingError::DateTimeParseError("d".into()),
            LoggingError::Custom("c".into()),
            LoggingError::NativeSinkError("ns".into()),
        ];
        for err in &variants {
            assert!(!format!("{err:?}").is_empty());
        }
    }

    #[test]
    fn test_logging_result_ok() {
        let r: LoggingResult<i32> = Ok(42);
        assert!(matches!(r, Ok(42)));
    }

    #[test]
    fn test_logging_result_err() {
        let r: LoggingResult<i32> = Err(LoggingError::custom("fail"));
        assert!(r.is_err());
    }
}
