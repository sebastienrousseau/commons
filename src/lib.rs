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
//! - `logging` - Simple structured logging
//! - `time` - Date/time utilities and formatting
//! - `collections` - Extended collection utilities (LRU cache)
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
//! commons = { version = "0.0.1", default-features = false, features = ["error", "time"] }
//! ```

#![cfg_attr(docsrs, feature(doc_cfg))]
#![deny(missing_docs)]
#![deny(unsafe_code)]
#![warn(clippy::all)]

#[cfg(feature = "config")]
#[cfg_attr(docsrs, doc(cfg(feature = "config")))]
pub mod config;

#[cfg(feature = "error")]
#[cfg_attr(docsrs, doc(cfg(feature = "error")))]
pub mod error;

#[cfg(feature = "logging")]
#[cfg_attr(docsrs, doc(cfg(feature = "logging")))]
pub mod logging;

#[cfg(feature = "time")]
#[cfg_attr(docsrs, doc(cfg(feature = "time")))]
pub mod time;

#[cfg(feature = "collections")]
#[cfg_attr(docsrs, doc(cfg(feature = "collections")))]
pub mod collections;

/// Prelude module for convenient imports.
///
/// Import everything commonly needed:
///
/// ```rust
/// use commons::prelude::*;
/// ```
pub mod prelude {
    #[cfg(feature = "error")]
    pub use crate::error::{CommonError, CommonResult};

    #[cfg(feature = "config")]
    pub use crate::config::{Config, ConfigBuilder, ConfigError};

    #[cfg(feature = "logging")]
    pub use crate::logging::{Logger, LogLevel};

    #[cfg(feature = "time")]
    pub use crate::time::{unix_timestamp, unix_timestamp_millis, format_duration, parse_duration};

    #[cfg(feature = "collections")]
    pub use crate::collections::LruCache;
}

/// Crate version.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// Returns the crate version.
#[must_use]
pub fn version() -> &'static str {
    VERSION
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_version() {
        assert_eq!(version(), "0.0.1");
    }
}
