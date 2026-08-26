// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Integration coverage for `LogEntry`'s wire formats.
//!
//! These exercise the formatters through the public builder API rather
//! than reaching into private helpers, so they double as executable
//! documentation of what each `LogFormat` emits.

#![cfg(feature = "logging")]

use commons::logging::{Log, LogFormat};

/// Renders an entry using its configured format.
fn render(entry: &Log) -> String {
    format!("{entry}")
}

#[test]
fn logfmt_emits_key_value_pairs() {
    let out = render(
        &Log::info("service started")
            .component("api")
            .format(LogFormat::Logfmt),
    );
    assert!(out.contains("level=info"), "{out}");
    assert!(out.contains("msg=\"service started\""), "{out}");
    assert!(out.contains("component=\"api\""), "{out}");
    assert!(out.contains("session_id="), "{out}");
}

#[test]
fn logfmt_escapes_quotes_in_the_message() {
    let out = render(&Log::warn("he said \"hello\"").format(LogFormat::Logfmt));
    assert!(out.contains("\\\"hello\\\""), "{out}");
}

#[test]
fn logfmt_quotes_attribute_values_that_need_it() {
    let out = render(
        &Log::info("attrs")
            .with("plain", "value")
            .with("spaced", "two words")
            .with("empty", "")
            .with("quoted", "a\"b")
            .with("number", 42)
            .format(LogFormat::Logfmt),
    );
    // A bare token needs no quotes; the rest do.
    assert!(out.contains("plain=value"), "{out}");
    assert!(out.contains("spaced=\"two words\""), "{out}");
    assert!(out.contains("empty=\"\""), "{out}");
    assert!(out.contains("quoted=\"a\\\"b\""), "{out}");
    // Non-string JSON values are written through Display, unquoted.
    assert!(out.contains("number=42"), "{out}");
}

#[test]
fn json_escapes_control_characters() {
    let message = "tab\there\nnewline\r\u{1}ctrl \\ \" end";
    let out = render(&Log::error(message).format(LogFormat::JSON));
    assert!(out.contains("\\t"), "{out}");
    assert!(out.contains("\\n"), "{out}");
    assert!(out.contains("\\r"), "{out}");
    assert!(out.contains("\\\\"), "{out}");
    assert!(out.contains("\\\""), "{out}");
    // Other control characters use the \u00XX escape.
    assert!(out.contains("\\u0001"), "{out}");
}

#[test]
fn every_format_renders_without_panicking() {
    let formats = [
        LogFormat::CLF,
        LogFormat::JSON,
        LogFormat::CEF,
        LogFormat::ELF,
        LogFormat::GELF,
        LogFormat::ApacheAccessLog,
        LogFormat::Logstash,
        LogFormat::NDJSON,
        LogFormat::MCP,
        LogFormat::OTLP,
        LogFormat::Logfmt,
        LogFormat::ECS,
    ];
    for format in formats {
        let out = render(
            &Log::info("probe")
                .component("tests")
                .with("k", "v")
                .format(format),
        );
        assert!(!out.is_empty(), "{format:?} rendered nothing");
    }
}

#[test]
fn session_ids_are_monotonic() {
    let first = Log::info("one").session_id;
    let second = Log::info("two").session_id;
    assert!(second > first, "{first} -> {second}");
}
