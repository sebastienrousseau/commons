// Copyright © 2024-2026 RustLogs (RLG). All rights reserved.
// SPDX-License-Identifier: Apache-2.0
// SPDX-License-Identifier: MIT

//! Convenience macros for span tracking, latency profiling, and MCP notifications.
//!
//! All macros dispatch through the lock-free [`ENGINE`](super::engine::ENGINE).

/// Execute a block within an OTLP-tagged span.
///
/// Emits an OTLP log with a generated `span_id`, increments the active
/// span counter, runs `$block`, then decrements. Returns the block's value.
///
/// Requires the `id` feature for span ID generation.
#[cfg(feature = "id")]
#[macro_export]
macro_rules! logging_span {
    ($name:expr, $block:block) => {{
        let span_id = $crate::logging::utils::generate_span_id();
        $crate::logging::engine::ENGINE.inc_spans();
        $crate::logging::log_entry::Log::info($name)
            .with("span_id", &span_id)
            .format($crate::logging::log_format::LogFormat::OTLP)
            .fire();
        let result = $block;
        $crate::logging::engine::ENGINE.dec_spans();
        result
    }};
}

/// Measure wall-clock latency of a block and emit a Logfmt metric.
///
/// Captures `Instant::now()` before the block, computes elapsed
/// microseconds after, and fires a Logfmt log with `latency_us`.
#[macro_export]
macro_rules! logging_time_it {
    ($action:expr, $block:block) => {{
        let start = std::time::Instant::now();
        let result = $block;
        let elapsed = start.elapsed().as_micros();

        $crate::logging::log_entry::Log::info(&format!("{} completed", $action))
            .with("latency_us", elapsed as u64)
            .format($crate::logging::log_format::LogFormat::Logfmt)
            .fire();

        result
    }};
}

/// Emit an MCP-formatted state transition notification.
///
/// Use for AI agent orchestration where state changes must be
/// machine-readable via JSON-RPC 2.0 notification semantics.
#[macro_export]
macro_rules! logging_mcp_notify {
    ($state_key:expr, $state_val:expr) => {
        $crate::logging::log_entry::Log::info("State transition")
            .with($state_key, $state_val)
            .format($crate::logging::log_format::LogFormat::MCP)
            .fire();
    };
}
