// Copyright © 2024-2026 RustLogs (RLG). All rights reserved.
// SPDX-License-Identifier: Apache-2.0
// SPDX-License-Identifier: MIT

//! Severity levels for structured logging.

use serde::{Deserialize, Serialize};
use std::{convert::TryFrom, error::Error, fmt, str::FromStr};

/// Custom error type for `LogLevel` parsing with context.
#[derive(Debug, Clone)]
pub struct ParseLogLevelError {
    /// The invalid log level value.
    pub invalid_value: String,
}

impl ParseLogLevelError {
    /// Creates a new instance of `ParseLogLevelError`.
    #[must_use]
    pub fn new(invalid_value: &str) -> Self {
        Self {
            invalid_value: invalid_value.to_string(),
        }
    }
}

impl fmt::Display for ParseLogLevelError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Invalid log level: {0}", self.invalid_value)
    }
}

impl Error for ParseLogLevelError {}

/// An enumeration of the different levels that a log message can have, ordered by severity.
#[derive(
    Clone,
    Copy,
    Debug,
    Default,
    Deserialize,
    Eq,
    Hash,
    Ord,
    PartialEq,
    PartialOrd,
    Serialize,
)]
pub enum LogLevel {
    /// `ALL`: The log level includes all levels.
    ALL,
    /// `NONE`: No logging.
    NONE,
    /// `DISABLED`: Logging is disabled.
    DISABLED,
    /// `TRACE`: Finer-grained informational events than `DEBUG`.
    TRACE,
    /// `DEBUG`: Debugging information, typically useful for developers.
    DEBUG,
    /// `VERBOSE`: Detailed logging, often more detailed than `INFO`.
    VERBOSE,
    /// `INFO`: Informational messages that highlight the progress of the application.
    #[default]
    INFO,
    /// `WARN`: Potentially harmful situations.
    WARN,
    /// `ERROR`: Error events that might still allow the application to continue running.
    ERROR,
    /// `FATAL`: Very severe error events that will presumably lead the application to abort.
    FATAL,
    /// `CRITICAL`: Critical conditions, often requiring immediate attention.
    CRITICAL,
}

macro_rules! define_log_levels {
    ( $( $variant:ident, $num:expr, $upper:expr, $lower:expr );+ $(;)? ) => {
        impl LogLevel {
            /// Converts the log level to its corresponding numeric value.
            #[must_use]
            pub const fn to_numeric(self) -> u8 {
                match self { $( Self::$variant => $num, )+ }
            }

            /// Returns the uppercase string representation of the log level.
            #[must_use]
            pub const fn as_str(&self) -> &'static str {
                match self { $( Self::$variant => $upper, )+ }
            }

            /// Returns the lowercase string representation of the log level.
            #[must_use]
            pub const fn as_str_lowercase(&self) -> &'static str {
                match self { $( Self::$variant => $lower, )+ }
            }

            /// Creates a `LogLevel` from a numeric value.
            #[must_use]
            pub const fn from_numeric(value: u8) -> Option<Self> {
                match value {
                    $( $num => Some(Self::$variant), )+
                    _ => None,
                }
            }
        }

        impl FromStr for LogLevel {
            type Err = ParseLogLevelError;

            fn from_str(s: &str) -> Result<Self, Self::Err> {
                match s.to_uppercase().as_str() {
                    $( $upper => Ok(Self::$variant), )+
                    _ => Err(ParseLogLevelError::new(s)),
                }
            }
        }
    };
}

define_log_levels! {
    ALL, 0, "ALL", "all";
    NONE, 1, "NONE", "none";
    DISABLED, 2, "DISABLED", "disabled";
    TRACE, 3, "TRACE", "trace";
    DEBUG, 4, "DEBUG", "debug";
    VERBOSE, 5, "VERBOSE", "verbose";
    INFO, 6, "INFO", "info";
    WARN, 7, "WARN", "warn";
    ERROR, 8, "ERROR", "error";
    FATAL, 9, "FATAL", "fatal";
    CRITICAL, 10, "CRITICAL", "critical";
}

impl LogLevel {
    /// Checks if the current log level includes another log level.
    #[must_use]
    pub const fn includes(self, other: Self) -> bool {
        match self {
            Self::ALL => true,
            Self::NONE => false,
            _ => self.to_numeric() >= other.to_numeric(),
        }
    }
}

impl TryFrom<String> for LogLevel {
    type Error = ParseLogLevelError;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        Self::from_str(&value)
    }
}

impl fmt::Display for LogLevel {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_log_level_from_str() {
        assert_eq!(LogLevel::from_str("info").unwrap(), LogLevel::INFO);
        assert_eq!(LogLevel::from_str("ERROR").unwrap(), LogLevel::ERROR);
        assert!(LogLevel::from_str("invalid").is_err());
    }

    #[test]
    fn test_log_level_numeric() {
        assert_eq!(LogLevel::ERROR.to_numeric(), 8);
        assert_eq!(LogLevel::DEBUG.to_numeric(), 4);
        assert_eq!(LogLevel::from_numeric(8), Some(LogLevel::ERROR));
        assert_eq!(LogLevel::from_numeric(99), None);
    }

    #[test]
    fn test_log_level_includes() {
        assert!(LogLevel::ERROR.includes(LogLevel::DEBUG));
        assert!(!LogLevel::DEBUG.includes(LogLevel::WARN));
        assert!(LogLevel::ALL.includes(LogLevel::FATAL));
        assert!(!LogLevel::NONE.includes(LogLevel::TRACE));
    }

    #[test]
    fn test_log_level_display() {
        assert_eq!(LogLevel::INFO.to_string(), "INFO");
        assert_eq!(LogLevel::WARN.as_str_lowercase(), "warn");
    }

    #[test]
    fn test_log_level_try_from_string() {
        let level = LogLevel::try_from("debug".to_string()).unwrap();
        assert_eq!(level, LogLevel::DEBUG);
    }

    #[test]
    fn test_parse_log_level_error() {
        let err = ParseLogLevelError::new("bad");
        assert_eq!(err.to_string(), "Invalid log level: bad");
    }

    #[test]
    fn test_from_numeric_all_valid_values() {
        assert_eq!(LogLevel::from_numeric(0), Some(LogLevel::ALL));
        assert_eq!(LogLevel::from_numeric(1), Some(LogLevel::NONE));
        assert_eq!(LogLevel::from_numeric(2), Some(LogLevel::DISABLED));
        assert_eq!(LogLevel::from_numeric(3), Some(LogLevel::TRACE));
        assert_eq!(LogLevel::from_numeric(4), Some(LogLevel::DEBUG));
        assert_eq!(LogLevel::from_numeric(5), Some(LogLevel::VERBOSE));
        assert_eq!(LogLevel::from_numeric(6), Some(LogLevel::INFO));
        assert_eq!(LogLevel::from_numeric(7), Some(LogLevel::WARN));
        assert_eq!(LogLevel::from_numeric(8), Some(LogLevel::ERROR));
        assert_eq!(LogLevel::from_numeric(9), Some(LogLevel::FATAL));
        assert_eq!(LogLevel::from_numeric(10), Some(LogLevel::CRITICAL));
        // Invalid values
        assert_eq!(LogLevel::from_numeric(11), None);
        assert_eq!(LogLevel::from_numeric(255), None);
    }
}
