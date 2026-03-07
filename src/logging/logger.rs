// Copyright © 2024-2026 RustLogs (RLG). All rights reserved.
// SPDX-License-Identifier: Apache-2.0
// SPDX-License-Identifier: MIT

//! Bridge from the [`log`](https://docs.rs/log) crate facade into the logging engine.
//!
//! Installed automatically by [`init()`](super::init::init) unless
//! you call `.without_log()` on the builder.

use super::engine::ENGINE;
use super::log_entry::Log;
use super::log_format::LogFormat;
use super::log_level::LogLevel;

/// Convert a [`log::Level`] to the corresponding [`LogLevel`].
#[must_use]
pub const fn map_log_level(level: log::Level) -> LogLevel {
    match level {
        log::Level::Error => LogLevel::ERROR,
        log::Level::Warn => LogLevel::WARN,
        log::Level::Info => LogLevel::INFO,
        log::Level::Debug => LogLevel::DEBUG,
        log::Level::Trace => LogLevel::TRACE,
    }
}

/// Convert a [`LogLevel`] to a [`log::LevelFilter`].
#[must_use]
pub const fn to_log_level_filter(level: LogLevel) -> log::LevelFilter {
    match level {
        LogLevel::ALL | LogLevel::TRACE => log::LevelFilter::Trace,
        LogLevel::DEBUG => log::LevelFilter::Debug,
        LogLevel::VERBOSE | LogLevel::INFO => log::LevelFilter::Info,
        LogLevel::WARN => log::LevelFilter::Warn,
        LogLevel::ERROR | LogLevel::FATAL | LogLevel::CRITICAL => {
            log::LevelFilter::Error
        }
        LogLevel::NONE | LogLevel::DISABLED => log::LevelFilter::Off,
    }
}

/// [`log::Log`] implementation that routes records into the logging ring buffer.
#[derive(Debug, Clone, Copy)]
pub struct LoggingFacade {
    format: LogFormat,
}

impl LoggingFacade {
    /// Create a `LoggingFacade` that formats output in the given format.
    #[must_use]
    pub const fn new(format: LogFormat) -> Self {
        Self { format }
    }
}

impl log::Log for LoggingFacade {
    fn enabled(&self, metadata: &log::Metadata<'_>) -> bool {
        map_log_level(metadata.level()).to_numeric()
            >= ENGINE.filter_level()
    }

    fn log(&self, record: &log::Record<'_>) {
        if !self.enabled(record.metadata()) {
            return;
        }

        let level = map_log_level(record.level());
        let mut entry = Log::build(level, &record.args().to_string());
        entry.component =
            std::borrow::Cow::Owned(record.target().to_string());
        entry.format = self.format;

        if let Some(file) = record.file() {
            entry = entry.with("file", file);
        }
        if let Some(line) = record.line() {
            entry = entry.with("line", line);
        }
        if let Some(module) = record.module_path() {
            entry = entry.with("module", module);
        }

        entry.fire();
    }

    fn flush(&self) {
        // The background flusher thread handles I/O.
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_map_log_level_all_variants() {
        assert_eq!(map_log_level(log::Level::Error), LogLevel::ERROR);
        assert_eq!(map_log_level(log::Level::Warn), LogLevel::WARN);
        assert_eq!(map_log_level(log::Level::Info), LogLevel::INFO);
        assert_eq!(map_log_level(log::Level::Debug), LogLevel::DEBUG);
        assert_eq!(map_log_level(log::Level::Trace), LogLevel::TRACE);
    }

    #[test]
    fn test_to_log_level_filter_all_variants() {
        assert_eq!(
            to_log_level_filter(LogLevel::ALL),
            log::LevelFilter::Trace
        );
        assert_eq!(
            to_log_level_filter(LogLevel::TRACE),
            log::LevelFilter::Trace
        );
        assert_eq!(
            to_log_level_filter(LogLevel::DEBUG),
            log::LevelFilter::Debug
        );
        assert_eq!(
            to_log_level_filter(LogLevel::VERBOSE),
            log::LevelFilter::Info
        );
        assert_eq!(
            to_log_level_filter(LogLevel::INFO),
            log::LevelFilter::Info
        );
        assert_eq!(
            to_log_level_filter(LogLevel::WARN),
            log::LevelFilter::Warn
        );
        assert_eq!(
            to_log_level_filter(LogLevel::ERROR),
            log::LevelFilter::Error
        );
        assert_eq!(
            to_log_level_filter(LogLevel::FATAL),
            log::LevelFilter::Error
        );
        assert_eq!(
            to_log_level_filter(LogLevel::CRITICAL),
            log::LevelFilter::Error
        );
        assert_eq!(
            to_log_level_filter(LogLevel::NONE),
            log::LevelFilter::Off
        );
        assert_eq!(
            to_log_level_filter(LogLevel::DISABLED),
            log::LevelFilter::Off
        );
    }

    #[test]
    fn test_logging_facade_new() {
        let logger = LoggingFacade::new(LogFormat::JSON);
        assert_eq!(format!("{logger:?}"), "LoggingFacade { format: JSON }");
    }

    #[test]
    fn test_logging_facade_clone_copy() {
        let logger = LoggingFacade::new(LogFormat::MCP);
        let cloned = logger;
        // Both are valid since LoggingFacade is Copy
        let _ = format!("{logger:?}");
        let _ = format!("{cloned:?}");
    }

    #[test]
    #[cfg_attr(miri, ignore)]
    fn test_logging_facade_enabled() {
        let logger = LoggingFacade::new(LogFormat::JSON);
        // Reset filter to 0 (ALL) so everything is enabled
        ENGINE.set_filter(0);

        let metadata = log::MetadataBuilder::new()
            .level(log::Level::Trace)
            .build();
        assert!(log::Log::enabled(&logger, &metadata));

        let metadata = log::MetadataBuilder::new()
            .level(log::Level::Error)
            .build();
        assert!(log::Log::enabled(&logger, &metadata));

        // Test with a high filter: only ERROR should be enabled
        ENGINE.set_filter(LogLevel::ERROR.to_numeric());
        let trace_metadata = log::MetadataBuilder::new()
            .level(log::Level::Trace)
            .build();
        assert!(!log::Log::enabled(&logger, &trace_metadata));

        let error_metadata = log::MetadataBuilder::new()
            .level(log::Level::Error)
            .build();
        assert!(log::Log::enabled(&logger, &error_metadata));

        // Reset filter back
        ENGINE.set_filter(0);
    }

    #[test]
    #[cfg_attr(miri, ignore)]
    fn test_logging_facade_log_with_metadata() {
        let logger = LoggingFacade::new(LogFormat::JSON);

        // Build a record with file/line/module metadata
        let record = log::RecordBuilder::new()
            .args(format_args!("test log message"))
            .level(log::Level::Info)
            .target("test_target")
            .file(Some("test_file.rs"))
            .line(Some(42))
            .module_path(Some("test_module"))
            .build();

        log::Log::log(&logger, &record);
    }

    #[test]
    #[cfg_attr(miri, ignore)]
    fn test_logging_facade_log_without_metadata() {
        let logger = LoggingFacade::new(LogFormat::MCP);

        // Build a record without optional metadata
        let record = log::RecordBuilder::new()
            .args(format_args!("minimal message"))
            .level(log::Level::Warn)
            .target("minimal_target")
            .build();

        log::Log::log(&logger, &record);
    }

    #[test]
    #[cfg_attr(miri, ignore)]
    fn test_logging_facade_log_all_levels() {
        let logger = LoggingFacade::new(LogFormat::JSON);

        for level in &[
            log::Level::Error,
            log::Level::Warn,
            log::Level::Info,
            log::Level::Debug,
            log::Level::Trace,
        ] {
            let record = log::RecordBuilder::new()
                .args(format_args!("level test"))
                .level(*level)
                .target("level_test")
                .build();
            log::Log::log(&logger, &record);
        }
    }

    #[test]
    fn test_logging_facade_flush() {
        let logger = LoggingFacade::new(LogFormat::JSON);
        log::Log::flush(&logger); // Should be a no-op
    }

    #[test]
    #[cfg_attr(miri, ignore)]
    fn test_logging_facade_log_filtered_out() {
        ENGINE.set_filter(LogLevel::FATAL.to_numeric());
        let logger = LoggingFacade::new(LogFormat::JSON);
        let record = log::RecordBuilder::new()
            .args(format_args!("should be filtered"))
            .level(log::Level::Trace)
            .target("filter_test")
            .build();
        // This should hit the early return on line 64
        log::Log::log(&logger, &record);
        ENGINE.set_filter(0);
    }
}
