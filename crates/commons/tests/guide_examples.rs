// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Compile-check for the examples in `doc/USER-GUIDE.md`.
//!
//! Documentation examples drift silently: nothing fails when a
//! signature changes underneath a code block in a Markdown file. Two
//! errors in the guide's first draft were caught here rather than by a
//! reader -- a `Config::get` type that is not implemented, and a
//! compound duration string that `parse_duration` does not accept.
//!
//! When the guide changes, change this alongside it.

#![allow(unused_variables, unused_imports, dead_code)]

use commons::collections::LruCache;
use commons::config::Config;
use commons::counter::Counter;
use commons::error::CommonError;
use commons::fs::{ensure_dir, is_wsl, resolve_path};
use commons::id::{IdFormat, generate_id, generate_prefixed_id, generate_timestamp_id};
use commons::logging::{Log, LogFormat};
use commons::retry::{BackoffStrategy, RetryConfig};
use commons::time::{format_duration, parse_duration, unix_timestamp};
use commons::validation::{is_valid_email, validate_length, validate_not_empty};
use std::sync::atomic::Ordering;
use std::time::Duration;

/// Counter from the guide's final section; at module scope because a
/// `static` after statements trips `clippy::items_after_statements`.
static REQUESTS: Counter = Counter::new(0);

#[test]
fn user_guide_examples_compile() {
    let e = CommonError::invalid_input("port must be 1-65535");
    assert!(e.is_input_error());
    let _ = e.is_recoverable();

    let cfg = Config::new("name = \"service\"\nport = 8080\n");
    assert!(cfg.has_key("port"));
    let _port: Option<i64> = cfg.get("port");
    let _ = cfg.raw();

    let name = validate_not_empty("ada").unwrap();
    let _name = validate_length(name, 2, 32).unwrap();
    let _ = is_valid_email("user@example.com");

    let policy = RetryConfig::new()
        .max_attempts(5)
        .backoff(BackoffStrategy::Exponential {
            initial: Duration::from_millis(100),
            max: Duration::from_secs(5),
            multiplier: 2.0,
        })
        .jitter(true);
    let _ = policy.backoff.delay_for_attempt(0);
    let _ = RetryConfig::no_retry();

    let _ = generate_prefixed_id("order");
    let _ = generate_timestamp_id();
    let _ = generate_id(IdFormat::Short);

    let _ = resolve_path("/tmp/notes");
    let _ = is_wsl();

    let _ = unix_timestamp();
    let d = parse_duration("2h").unwrap();
    let _ = format_duration(d);

    let mut cache = LruCache::new(2);
    let _ = cache.insert("a", 1);
    let _ = cache.get(&"a");
    let _ = cache.peek(&"a");

    let line = format!(
        "{}",
        Log::warn("slow request")
            .with("ms", 1500)
            .format(LogFormat::Logfmt)
    );
    assert!(line.contains("level=warn"));

    let _ = REQUESTS.fetch_add(1, Ordering::Relaxed);
}
