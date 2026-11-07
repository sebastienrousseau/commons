//! # Commons
//!
//! Shared Rust utilities and common patterns for the Sebastien Rousseau ecosystem.
//!
//! This crate provides reusable components, traits, and utilities used across
//! multiple Rust projects in the ecosystem.
//!
//! ## Features
//!
//! - `config` - Configuration file loading and management (TOML)
//! - `error` - Common error types and Result aliases
//! - `logging` - Near-lock-free structured logging engine
//! - `time` - Date/time utilities and formatting
//! - `collections` - Extended collection utilities (LRU cache)
//! - `validation` - Input validation utilities
//! - `retry` - Retry logic with backoff strategies
//! - `id` - ID generation (timestamp, random, UUID-like)
//! - `env` - Environment variable helpers
//! - `fs` - Cross-platform filesystem utilities
//!
//! ## Quick Start
//!
//! ```rust
//! use commons::prelude::*;
//!
//! // Use the LRU cache
//! let mut cache = LruCache::new(100);
//! cache.insert("key", "value");
//! ```
//!
//! ## Feature Flags
//!
//! Enable only what you need:
//!
//! ```toml
//! [dependencies]
//! euxis-commons = { version = "0.0.9", default-features = false, features = ["error", "validation"] }
//! ```

#![cfg_attr(docsrs, feature(doc_cfg))]
#![deny(missing_docs)]
#![deny(unsafe_code)]
// libtest's generated harness refers to every #[test] in the modules
// deprecated in 0.0.8 from outside them, which a module-level allow cannot
// reach. Applies only to this crate's own unit-test build.
#![cfg_attr(test, allow(deprecated))]
#![warn(clippy::all)]

/// Portable monotonic counters for targets without 64-bit atomics.
#[cfg(any(feature = "counter", feature = "id", feature = "logging"))]
#[cfg_attr(docsrs, doc(cfg(feature = "counter")))]
pub mod counter;

#[cfg(feature = "config")]
#[cfg_attr(docsrs, doc(cfg(feature = "config")))]
pub mod config;

#[cfg(feature = "error")]
#[cfg_attr(docsrs, doc(cfg(feature = "error")))]
pub mod error;

#[cfg(feature = "logging")]
#[cfg_attr(docsrs, doc(cfg(feature = "logging")))]
#[deprecated(
    since = "0.0.8",
    note = "use the rlg crate, which carries the same engine with the fixes this copy lacks; commons::logging will be removed"
)]
pub mod logging;

#[cfg(feature = "time")]
#[cfg_attr(docsrs, doc(cfg(feature = "time")))]
#[deprecated(
    since = "0.0.8",
    note = "no known user; use std::time, or the time / humantime crates; commons::time will be removed"
)]
pub mod time;

#[cfg(feature = "collections")]
#[cfg_attr(docsrs, doc(cfg(feature = "collections")))]
#[deprecated(
    since = "0.0.8",
    note = "no known user; use the lru or quick_cache crate; commons::collections will be removed"
)]
pub mod collections;

#[cfg(feature = "validation")]
#[cfg_attr(docsrs, doc(cfg(feature = "validation")))]
pub mod validation;

#[cfg(feature = "retry")]
#[cfg_attr(docsrs, doc(cfg(feature = "retry")))]
#[deprecated(
    since = "0.0.8",
    note = "no known user; use the backon or backoff crate; commons::retry will be removed"
)]
pub mod retry;

#[cfg(feature = "id")]
#[cfg_attr(docsrs, doc(cfg(feature = "id")))]
pub mod id;

#[cfg(feature = "env")]
#[cfg_attr(docsrs, doc(cfg(feature = "env")))]
#[deprecated(
    since = "0.0.8",
    note = "no known user; use std::env, or the envy crate for typed configuration; commons::env will be removed"
)]
pub mod env;

/// Path containment: join untrusted relative paths under a root.
#[cfg(feature = "path")]
#[cfg_attr(docsrs, doc(cfg(feature = "path")))]
pub mod path;

#[cfg(feature = "fs")]
#[cfg_attr(docsrs, doc(cfg(feature = "fs")))]
#[deprecated(
    since = "0.0.8",
    note = "no known user; use std::fs and the dirs crate; commons::fs will be removed"
)]
pub mod fs;

/// Create a logger for the current module.
///
/// Returns a [`logging::Logger`] whose module name is set to the caller's
/// [`module_path!()`]. This is the recommended way to obtain a logger
/// without hard-coding module strings.
#[cfg(feature = "logging")]
#[macro_export]
macro_rules! logger {
    () => {
        $crate::logging::Logger::new(module_path!())
    };
}

/// Prelude module for convenient imports.
///
/// Import everything commonly needed:
///
/// ```rust
/// use commons::prelude::*;
/// ```
// The prelude re-exports items deprecated in 0.0.8 so existing imports keep
// compiling; callers still get the deprecation warning when they use them.
#[allow(deprecated)]
pub mod prelude {
    #[cfg(feature = "error")]
    pub use crate::error::{CommonError, CommonResult};

    #[cfg(feature = "config")]
    pub use crate::config::{Config, ConfigBuilder, ConfigError};

    #[cfg(feature = "logging")]
    pub use crate::logging::{
        FlushGuard, Log, LogFormat, LogLevel, Logger, LoggingBuilder, LoggingConfig, LoggingError,
        LoggingResult, builder as logging_builder, init as logging_init,
    };

    #[cfg(feature = "time")]
    pub use crate::time::{format_duration, parse_duration, unix_timestamp, unix_timestamp_millis};

    #[cfg(feature = "collections")]
    pub use crate::collections::LruCache;

    #[cfg(feature = "validation")]
    pub use crate::validation::{
        Validator, is_valid_email, is_valid_url, validate_length, validate_range,
    };

    #[cfg(feature = "retry")]
    pub use crate::retry::{BackoffStrategy, RetryConfig, retry};

    #[cfg(feature = "id")]
    pub use crate::id::{IdFormat, IdGenerator, generate_id, generate_prefixed_id};

    #[cfg(feature = "env")]
    pub use crate::env::{get_env, get_env_or, is_development, is_production, require_env};

    #[cfg(feature = "path")]
    pub use crate::path::{PathError, is_within, safe_join};

    #[cfg(feature = "fs")]
    pub use crate::fs::{ensure_dir, from_wsl_path, is_wsl, resolve_path, to_wsl_path};
}

/// Crate version.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// Returns the crate version.
#[must_use]
pub const fn version() -> &'static str {
    VERSION
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn version_matches_the_manifest() {
        // Deliberately not a hardcoded literal. Pinning the version in a
        // second place means every release breaks this test, and the
        // "fix" is to edit the copy -- which tests nothing except that
        // someone remembered to edit it.
        assert_eq!(version(), env!("CARGO_PKG_VERSION"));
        assert_ne!(version(), "");
    }
}
