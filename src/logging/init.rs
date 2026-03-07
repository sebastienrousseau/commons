// Copyright © 2024-2026 RustLogs (RLG). All rights reserved.
// SPDX-License-Identifier: Apache-2.0
// SPDX-License-Identifier: MIT

//! One-call initialization for the logging engine.
//!
//! ```rust,no_run
//! // Sensible defaults -- auto-detects format (TTY -> Logfmt, pipe -> JSON).
//! let _guard = commons::logging::init::init().unwrap();
//!
//! // Custom configuration via builder.
//! let _guard = commons::logging::init::builder()
//!     .level(commons::logging::LogLevel::DEBUG)
//!     .format(commons::logging::LogFormat::JSON)
//!     .init()
//!     .unwrap();
//! ```

use super::engine::ENGINE;
use super::log_format::LogFormat;
use super::log_level::LogLevel;
use super::logger::{LoggingFacade, to_log_level_filter};
use std::fmt;
use std::sync::OnceLock;

/// Auto-detect the output format from the execution context.
///
/// - **TTY** -> `Logfmt` (human-readable key=value)
/// - **Pipe / file / CI** -> `JSON` (machine-parseable)
/// - **`RLG_ENV=production`** -> `JSON`
fn detect_default_format() -> LogFormat {
    if std::env::var("RLG_ENV")
        .map(|v| v == "production")
        .unwrap_or(false)
    {
        return LogFormat::JSON;
    }
    if atty_stdout() {
        LogFormat::Logfmt
    } else {
        LogFormat::JSON
    }
}

/// Returns `true` if stdout is connected to a terminal.
fn atty_stdout() -> bool {
    use std::io::IsTerminal;
    std::io::stdout().is_terminal()
}

/// Parse `RUST_LOG` for a level filter (e.g., `RUST_LOG=debug`).
///
/// Accepts `RUST_LOG=<level>` and `RUST_LOG=<crate>=<level>`.
/// Returns the most permissive level found. Returns `None` if unset.
fn parse_rust_log() -> Option<LogLevel> {
    let val = std::env::var("RUST_LOG").ok()?;
    let mut most_permissive: Option<LogLevel> = None;
    for directive in val.split(',') {
        let level_str = directive.split('=').next_back().unwrap_or(directive).trim();
        if let Ok(level) = level_str.parse::<LogLevel>() {
            match most_permissive {
                None => most_permissive = Some(level),
                Some(current) if level.to_numeric() < current.to_numeric() => {
                    most_permissive = Some(level);
                }
                _ => {}
            }
        }
    }
    most_permissive
}

/// Prevents double initialization via `OnceLock` (set-once semantics).
static INIT_GUARD: OnceLock<()> = OnceLock::new();

/// `&'static` logger instance required by `log::set_logger`.
static LOGGER: OnceLock<LoggingFacade> = OnceLock::new();

/// Initialization failures.
#[derive(Debug, Clone, Copy)]
pub enum InitError {
    /// A `log` crate logger was already registered globally.
    LoggerAlreadySet,
    /// A `tracing` subscriber was already registered globally.
    SubscriberAlreadySet,
    /// `init()` or `builder().init()` was called more than once.
    AlreadyInitialized,
}

impl fmt::Display for InitError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::LoggerAlreadySet => f.write_str("a log crate logger was already set"),
            Self::SubscriberAlreadySet => f.write_str("a tracing subscriber was already set"),
            Self::AlreadyInitialized => f.write_str("logging was already initialized"),
        }
    }
}

impl std::error::Error for InitError {}

/// Builder for customizing logging initialization.
#[derive(Debug, Clone, Copy)]
pub struct LoggingBuilder {
    level: LogLevel,
    format: LogFormat,
    install_log: bool,
    install_tracing: bool,
}

impl Default for LoggingBuilder {
    fn default() -> Self {
        Self {
            level: LogLevel::INFO,
            format: detect_default_format(),
            install_log: true,
            install_tracing: true,
        }
    }
}

impl LoggingBuilder {
    /// Set the minimum severity level. Events below this are dropped.
    #[must_use]
    pub const fn level(mut self, level: LogLevel) -> Self {
        self.level = level;
        self
    }

    /// Set the default output format. Overrides auto-detection.
    #[must_use]
    pub const fn format(mut self, format: LogFormat) -> Self {
        self.format = format;
        self
    }

    /// Skip installing the `log` crate facade bridge.
    #[must_use]
    pub const fn without_log(mut self) -> Self {
        self.install_log = false;
        self
    }

    /// Skip installing the `tracing` global subscriber.
    #[must_use]
    pub const fn without_tracing(mut self) -> Self {
        self.install_tracing = false;
        self
    }

    /// Register `LoggingFacade` as the global `log` facade.
    ///
    /// # Errors
    ///
    /// Returns `InitError::LoggerAlreadySet` if another logger was already registered.
    pub(crate) fn install_log_facade(format: LogFormat, level: LogLevel) -> Result<(), InitError> {
        let logger = LOGGER.get_or_init(|| LoggingFacade::new(format));
        log::set_logger(logger).map_err(|_| InitError::LoggerAlreadySet)?;
        log::set_max_level(to_log_level_filter(level));
        Ok(())
    }

    /// Register `LoggingSubscriber` as the global `tracing` dispatcher.
    ///
    /// # Errors
    ///
    /// Returns `InitError::SubscriberAlreadySet` if another subscriber was already registered.
    pub(crate) fn install_tracing_subscriber() -> Result<(), InitError> {
        let subscriber = super::tracing_bridge::LoggingSubscriber::new();
        let dispatch = tracing_core::dispatcher::Dispatch::new(subscriber);
        tracing_core::dispatcher::set_global_default(dispatch)
            .map_err(|_| InitError::SubscriberAlreadySet)?;
        Ok(())
    }

    /// Finalize and install the logging system as the global logger and subscriber.
    ///
    /// Applies `RUST_LOG` overrides and auto-detects the format
    /// (TTY -> Logfmt, pipe -> JSON) when none was explicitly set.
    ///
    /// # Errors
    ///
    /// Returns `InitError` if a global logger/subscriber already exists
    /// or if `init()` was already called.
    pub fn init(mut self) -> Result<FlushGuard, InitError> {
        if INIT_GUARD.set(()).is_err() {
            return Err(InitError::AlreadyInitialized);
        }

        // Apply RUST_LOG level override.
        if let Some(env_level) = parse_rust_log() {
            self.level = env_level;
        }

        // Set engine filter level
        ENGINE.set_filter(self.level.to_numeric());

        // Install log facade
        if self.install_log {
            Self::install_log_facade(self.format, self.level)?;
        }

        // Install tracing subscriber
        if self.install_tracing {
            Self::install_tracing_subscriber()?;
        }

        Ok(FlushGuard { _private: () })
    }
}

/// Create a new [`LoggingBuilder`] for custom initialization.
#[must_use]
pub fn builder() -> LoggingBuilder {
    LoggingBuilder::default()
}

/// RAII guard (resource-cleanup-on-drop) that flushes pending events on drop.
///
/// Returned by [`init`] and [`LoggingBuilder::init`]. **Hold it in `main`** --
/// dropping it calls [`ENGINE.shutdown()`](super::engine::LockFreeEngine::shutdown).
#[derive(Debug)]
pub struct FlushGuard {
    _private: (),
}

impl Drop for FlushGuard {
    fn drop(&mut self) {
        ENGINE.shutdown();
    }
}

/// Initialize the logging system with sensible defaults.
///
/// Auto-detects format (TTY -> Logfmt, pipe -> JSON) and respects `RUST_LOG`.
///
/// # Errors
///
/// Returns `InitError` if a global logger or subscriber already exists.
pub fn init() -> Result<FlushGuard, InitError> {
    builder().init()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_init_error_display_logger_already_set() {
        let err = InitError::LoggerAlreadySet;
        assert_eq!(err.to_string(), "a log crate logger was already set");
    }

    #[test]
    fn test_init_error_display_subscriber_already_set() {
        let err = InitError::SubscriberAlreadySet;
        assert_eq!(err.to_string(), "a tracing subscriber was already set");
    }

    #[test]
    fn test_init_error_display_already_initialized() {
        let err = InitError::AlreadyInitialized;
        assert_eq!(err.to_string(), "logging was already initialized");
    }

    #[test]
    fn test_init_error_debug() {
        let err = InitError::LoggerAlreadySet;
        assert_eq!(format!("{err:?}"), "LoggerAlreadySet");
    }

    #[test]
    fn test_init_error_clone_copy() {
        let err = InitError::AlreadyInitialized;
        let cloned = err;
        assert_eq!(format!("{err:?}"), format!("{cloned:?}"));
    }

    #[test]
    fn test_init_error_is_error() {
        let err = InitError::LoggerAlreadySet;
        // Verify it implements std::error::Error
        let _: &dyn std::error::Error = &err;
    }

    #[test]
    fn test_builder_defaults() {
        let b = LoggingBuilder::default();
        assert_eq!(b.level, LogLevel::INFO);
        assert!(b.install_log);
        assert!(b.install_tracing);
        // Format is auto-detected (Logfmt for TTY, JSON for pipe/CI)
        assert!(b.format == LogFormat::JSON || b.format == LogFormat::Logfmt);
    }

    #[test]
    fn test_builder_level() {
        let b = builder().level(LogLevel::DEBUG);
        assert_eq!(b.level, LogLevel::DEBUG);
    }

    #[test]
    fn test_builder_format() {
        let b = builder().format(LogFormat::JSON);
        assert_eq!(b.format, LogFormat::JSON);
    }

    #[test]
    fn test_builder_without_log() {
        let b = builder().without_log();
        assert!(!b.install_log);
        assert!(b.install_tracing);
    }

    #[test]
    fn test_builder_without_tracing() {
        let b = builder().without_tracing();
        assert!(b.install_log);
        assert!(!b.install_tracing);
    }

    #[test]
    fn test_builder_chaining() {
        let b = builder()
            .level(LogLevel::TRACE)
            .format(LogFormat::ECS)
            .without_log()
            .without_tracing();
        assert_eq!(b.level, LogLevel::TRACE);
        assert_eq!(b.format, LogFormat::ECS);
        assert!(!b.install_log);
        assert!(!b.install_tracing);
    }

    #[test]
    fn test_builder_clone_copy() {
        let b = builder().level(LogLevel::WARN);
        let b2 = b;
        // Both usable since LoggingBuilder is Copy
        assert_eq!(b.level, b2.level);
        assert_eq!(b.format, b2.format);
    }

    #[test]
    fn test_builder_without_facades_configuration() {
        let b = builder().without_log().without_tracing();
        assert!(!b.install_log);
        assert!(!b.install_tracing);
    }

    #[test]
    fn test_builder_fn() {
        let b = builder();
        assert_eq!(b.level, LogLevel::INFO);
        // Format is auto-detected based on output context
        assert!(b.format == LogFormat::JSON || b.format == LogFormat::Logfmt);
        assert!(b.install_log);
        assert!(b.install_tracing);
    }

    #[test]
    fn test_init_error_source() {
        let err = InitError::LoggerAlreadySet;
        // std::error::Error::source should return None
        assert!(std::error::Error::source(&err).is_none());
    }

    #[test]
    fn test_builder_default_impl() {
        let b1 = LoggingBuilder::default();
        let b2 = builder();
        assert_eq!(b1.level, b2.level);
        assert_eq!(b1.format, b2.format);
        assert_eq!(b1.install_log, b2.install_log);
        assert_eq!(b1.install_tracing, b2.install_tracing);
    }

    #[test]
    fn test_init_error_all_display_variants() {
        // Exercise all three Display paths
        let msgs: Vec<String> = vec![
            InitError::LoggerAlreadySet,
            InitError::SubscriberAlreadySet,
            InitError::AlreadyInitialized,
        ]
        .into_iter()
        .map(|e| e.to_string())
        .collect();
        assert_eq!(msgs.len(), 3);
        assert!(msgs[0].contains("log"));
        assert!(msgs[1].contains("tracing"));
        assert!(msgs[2].contains("already initialized"));
    }

    #[test]
    #[cfg_attr(miri, ignore)]
    fn test_init_guard_static() {
        // Exercise the OnceLock guard
        // First attempt may succeed or fail depending on test ordering
        let _ = INIT_GUARD.set(());
        // Second attempt should always fail
        assert!(INIT_GUARD.set(()).is_err());
    }

    #[test]
    #[cfg_attr(miri, ignore)]
    fn test_logger_static() {
        // Exercise the LOGGER OnceLock
        let logger = LOGGER.get_or_init(|| LoggingFacade::new(LogFormat::JSON));
        assert!(format!("{logger:?}").contains("LoggingFacade"));
    }

    #[test]
    #[cfg_attr(miri, ignore)]
    fn test_install_log_facade() {
        // First call may succeed or fail (test ordering is non-deterministic)
        let r1 = LoggingBuilder::install_log_facade(LogFormat::JSON, LogLevel::INFO);
        assert!(r1.is_ok() || matches!(r1, Err(InitError::LoggerAlreadySet)));
        // Second call should definitely fail
        let r2 = LoggingBuilder::install_log_facade(LogFormat::MCP, LogLevel::DEBUG);
        assert!(matches!(r2, Err(InitError::LoggerAlreadySet)));
    }

    #[test]
    #[cfg_attr(miri, ignore)]
    fn test_install_tracing_subscriber() {
        // First call may succeed or fail (test ordering is non-deterministic)
        let r1 = LoggingBuilder::install_tracing_subscriber();
        assert!(r1.is_ok() || matches!(r1, Err(InitError::SubscriberAlreadySet)));
        // Second call should definitely fail
        let r2 = LoggingBuilder::install_tracing_subscriber();
        assert!(matches!(r2, Err(InitError::SubscriberAlreadySet)));
    }

    #[test]
    #[allow(unsafe_code)]
    #[cfg_attr(miri, ignore)]
    fn test_detect_default_format_production() {
        // SAFETY: Test-only env var manipulation.
        unsafe { std::env::set_var("RLG_ENV", "production") };
        let format = detect_default_format();
        assert_eq!(format, LogFormat::JSON);
        // SAFETY: Cleanup.
        unsafe { std::env::remove_var("RLG_ENV") };
    }

    #[test]
    #[allow(unsafe_code)]
    #[cfg_attr(miri, ignore)]
    fn test_detect_default_format_non_production() {
        // SAFETY: Test-only env var manipulation.
        unsafe { std::env::remove_var("RLG_ENV") };
        let format = detect_default_format();
        // In test context stdout is typically not a TTY,
        // so we expect JSON. But TTY could yield Logfmt.
        assert!(format == LogFormat::JSON || format == LogFormat::Logfmt);
    }

    #[test]
    #[allow(unsafe_code)]
    #[cfg_attr(miri, ignore)]
    fn test_parse_rust_log_unset() {
        // SAFETY: Test-only env var manipulation.
        unsafe { std::env::remove_var("RUST_LOG") };
        assert!(parse_rust_log().is_none());
    }

    #[test]
    #[allow(unsafe_code)]
    #[cfg_attr(miri, ignore)]
    fn test_parse_rust_log_simple_level() {
        // SAFETY: Test-only env var manipulation.
        unsafe { std::env::set_var("RUST_LOG", "debug") };
        let level = parse_rust_log();
        assert_eq!(level, Some(LogLevel::DEBUG));
        // SAFETY: Cleanup.
        unsafe { std::env::remove_var("RUST_LOG") };
    }

    #[test]
    #[allow(unsafe_code)]
    #[cfg_attr(miri, ignore)]
    fn test_parse_rust_log_crate_directives() {
        // SAFETY: Test-only env var manipulation.
        unsafe {
            std::env::set_var("RUST_LOG", "crate=warn,other=trace");
        }
        let level = parse_rust_log();
        // The most permissive (lowest numeric) should be TRACE
        assert_eq!(level, Some(LogLevel::TRACE));
        // SAFETY: Cleanup.
        unsafe { std::env::remove_var("RUST_LOG") };
    }

    #[test]
    #[allow(unsafe_code)]
    #[cfg_attr(miri, ignore)]
    fn test_parse_rust_log_invalid_level() {
        // SAFETY: Test-only env var manipulation.
        unsafe {
            std::env::set_var("RUST_LOG", "nonsense_level");
        }
        let level = parse_rust_log();
        assert!(level.is_none());
        // SAFETY: Cleanup.
        unsafe { std::env::remove_var("RUST_LOG") };
    }

    #[test]
    #[cfg_attr(miri, ignore)]
    fn test_atty_stdout_callable() {
        // Just exercise the function -- result depends on context.
        let _is_tty = atty_stdout();
    }

    #[test]
    #[cfg_attr(miri, ignore)]
    fn test_flush_guard_drop() {
        // Construct and immediately drop a FlushGuard to exercise
        // the Drop impl that calls ENGINE.shutdown().
        let guard = FlushGuard { _private: () };
        drop(guard);
    }

    #[test]
    #[cfg_attr(miri, ignore)]
    fn test_init_function() {
        // init() uses global statics that may already be set by
        // other tests, so handle both cases.
        let result = init();
        assert!(
            result.is_ok()
                || matches!(
                    result,
                    Err(InitError::AlreadyInitialized
                        | InitError::LoggerAlreadySet
                        | InitError::SubscriberAlreadySet)
                )
        );
    }

    #[test]
    #[cfg_attr(miri, ignore)]
    fn test_init_function_returns_guard_or_error() {
        let result = init();
        // Either succeeds (first call) or returns AlreadyInitialized
        // (or LoggerAlreadySet / SubscriberAlreadySet in multi-test runs).
        match result {
            Ok(_guard) => {
                // Guard is valid; dropping it calls ENGINE.shutdown().
            }
            Err(
                InitError::AlreadyInitialized
                | InitError::LoggerAlreadySet
                | InitError::SubscriberAlreadySet,
            ) => { /* fine */ }
        }
    }

    #[test]
    #[cfg_attr(miri, ignore)]
    fn test_flush_guard_debug() {
        let guard = FlushGuard { _private: () };
        let dbg = format!("{guard:?}");
        assert!(dbg.contains("FlushGuard"));
        drop(guard);
    }
}
