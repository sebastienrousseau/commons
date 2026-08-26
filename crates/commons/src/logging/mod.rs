// Copyright © 2024-2026 RustLogs (RLG). All rights reserved.
// SPDX-License-Identifier: Apache-2.0
// SPDX-License-Identifier: MIT

//! Near-lock-free structured logging engine.

/// Near-lock-free ingestion engine backed by a bounded ring buffer.
pub mod engine;
/// One-call initialization for the logging engine.
pub mod init;
/// TOML-based configuration: loading, validation, diffing, and hot-reload.
pub mod log_config;
/// Structured log entry with a chainable builder API.
pub mod log_entry;
/// Error types for the logging pipeline.
pub mod log_error;
/// Structured output formats (JSON, MCP, OTLP, ECS, CEF, ...).
pub mod log_format;
/// Severity levels for structured logging.
pub mod log_level;
/// Bridge from the `log` crate facade into the logging engine.
pub mod logger;
/// Convenience macros for span tracking, latency profiling, and MCP notifications.
pub mod macros;
/// Log rotation policies: size, time, date, and count-based.
pub mod rotation;
/// Platform-native logging sinks.
pub mod sink;
/// Integration with the `tracing` ecosystem.
pub mod tracing_bridge;
/// Opt-in terminal dashboard for live observability metrics.
pub mod tui;
/// Utility functions for the logging pipeline.
pub mod utils;

// Re-exports
pub use engine::{ENGINE, FastSerializer, LockFreeEngine, LogEvent};
pub use init::{FlushGuard, InitError, LoggingBuilder, builder, init};
pub use log_config::{LogRotation, LoggingConfig, LoggingConfigError, LoggingDestination};
pub use log_entry::Log;
pub use log_error::{LoggingError, LoggingResult};
pub use log_format::LogFormat;
pub use log_level::{LogLevel, ParseLogLevelError};
pub use logger::LoggingFacade;
pub use sink::PlatformSink;
#[cfg(feature = "logging-tracing-layer")]
pub use tracing_bridge::LoggingLayer;
pub use tracing_bridge::LoggingSubscriber;

// ---------------------------------------------------------------------------
// Legacy simple logger (backward-compatible re-export)
// ---------------------------------------------------------------------------

/// Simple structured logger.
///
/// Lightweight wrapper that prints timestamped, level-filtered messages to
/// stdout. Each `Logger` owns its module name as a `String` -- creating one
/// allocates, so prefer storing it rather than constructing per-call.
///
/// For high-throughput or production logging, consider using the
/// [`Log`] builder API and the lock-free engine instead.
#[derive(Debug)]
pub struct Logger {
    level: LogLevel,
    module: String,
}

impl Logger {
    /// Create a new logger for a module.
    #[must_use]
    pub fn new(module: &str) -> Self {
        Self {
            level: LogLevel::INFO,
            module: module.to_string(),
        }
    }

    /// Set the minimum log level.
    pub const fn set_level(&mut self, level: LogLevel) {
        self.level = level;
    }

    /// Log a message at the given level.
    pub fn log(&self, level: LogLevel, message: &str) {
        if level >= self.level {
            let timestamp = Self::timestamp();
            let level_str = level.as_str();
            println!("[{timestamp}] {level_str} [{}] {message}", self.module);
        }
    }

    /// Get the current Unix timestamp in seconds.
    #[cfg(feature = "time")]
    fn timestamp() -> u64 {
        crate::time::unix_timestamp()
    }

    /// Fallback timestamp when the `time` feature is absent.
    #[cfg(not(feature = "time"))]
    fn timestamp() -> u64 {
        use std::time::{SystemTime, UNIX_EPOCH};
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs()
    }

    /// Log a trace message.
    pub fn trace(&self, message: &str) {
        self.log(LogLevel::TRACE, message);
    }

    /// Log a debug message.
    pub fn debug(&self, message: &str) {
        self.log(LogLevel::DEBUG, message);
    }

    /// Log an info message.
    pub fn info(&self, message: &str) {
        self.log(LogLevel::INFO, message);
    }

    /// Log a warning message.
    pub fn warn(&self, message: &str) {
        self.log(LogLevel::WARN, message);
    }

    /// Log an error message.
    pub fn error(&self, message: &str) {
        self.log(LogLevel::ERROR, message);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_logger_basic() {
        let logger = Logger::new("test_module");
        logger.info("basic log test");
    }

    #[test]
    fn test_logger_level_filtering() {
        let mut logger = Logger::new("filter_test");
        logger.set_level(LogLevel::WARN);

        // These should not panic -- they are simply filtered out.
        logger.trace("should be filtered");
        logger.debug("should be filtered");
        logger.info("should be filtered");

        // These should print (level >= Warn).
        logger.warn("visible warning");
        logger.error("visible error");
    }
}
