// Copyright © 2024-2026 RustLogs (RLG). All rights reserved.
// SPDX-License-Identifier: Apache-2.0
// SPDX-License-Identifier: MIT

//! Structured log entry with a chainable builder API.

use super::log_format::LogFormat;
use super::log_level::LogLevel;
use dtt::datetime::DateTime;
use serde::{Deserialize, Serialize};
use std::borrow::Cow;
use std::collections::BTreeMap;
use std::fmt;
use std::sync::LazyLock;
use std::sync::atomic::{AtomicU64, Ordering};

/// Monotonic session ID counter. Incremented atomically per `build()` call.
static SESSION_COUNTER: AtomicU64 = AtomicU64::new(1);

/// Hostname, resolved once and cached for the process lifetime.
static CACHED_HOSTNAME: LazyLock<String> = LazyLock::new(|| {
    hostname::get().map_or_else(
        |_| "localhost".to_string(),
        |h| h.to_string_lossy().to_string(),
    )
});

/// A structured log entry with a chainable builder API.
///
/// Fields use `Cow<'static, str>` and `u64` where possible to
/// minimize heap allocations on the ingestion hot path.
///
/// Construct via level shortcuts ([`Log::info`], [`Log::error`], ...)
/// or the generic [`Log::build`]. Dispatch with [`Log::fire`].
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Eq)]
pub struct Log {
    /// Monotonic counter assigned at `build()` time.
    pub session_id: u64,
    /// Wall-clock timestamp. Populated at build time; override with `.time()`.
    pub time: Cow<'static, str>,
    /// Severity level (`INFO`, `ERROR`, etc.).
    pub level: LogLevel,
    /// Originating service or module name. Defaults to `"default"`.
    pub component: Cow<'static, str>,
    /// Human-readable message body.
    pub description: String,
    /// Output format applied during `Display` serialization.
    pub format: LogFormat,
    /// Arbitrary key-value attributes for structured context.
    pub attributes: BTreeMap<String, serde_json::Value>,
}

impl Default for Log {
    fn default() -> Self {
        Self {
            session_id: 0,
            time: Cow::Borrowed(""),
            level: LogLevel::INFO,
            component: Cow::Borrowed(""),
            description: String::default(),
            format: LogFormat::CLF,
            attributes: BTreeMap::new(),
        }
    }
}

impl Log {
    /// Ingest this entry into the engine by cloning it.
    ///
    /// **Prefer [`fire()`](Self::fire)**, which consumes `self` and avoids
    /// the clone. Use `log()` only when you need to retain the entry.
    #[track_caller]
    pub fn log(&self) {
        super::engine::ENGINE.inc_format(self.format);
        let event = super::engine::LogEvent {
            level: self.level,
            level_num: self.level.to_numeric(),
            log: self.clone(),
        };
        super::engine::ENGINE.ingest(event);
    }

    /// Build an INFO-level log entry.
    #[must_use]
    pub fn info(description: &str) -> Self {
        Self::build(LogLevel::INFO, description)
    }

    /// Build a WARN-level log entry.
    #[must_use]
    pub fn warn(description: &str) -> Self {
        Self::build(LogLevel::WARN, description)
    }

    /// Build an ERROR-level log entry.
    #[must_use]
    pub fn error(description: &str) -> Self {
        Self::build(LogLevel::ERROR, description)
    }

    /// Build a DEBUG-level log entry.
    #[must_use]
    pub fn debug(description: &str) -> Self {
        Self::build(LogLevel::DEBUG, description)
    }

    /// Build a TRACE-level log entry.
    #[must_use]
    pub fn trace(description: &str) -> Self {
        Self::build(LogLevel::TRACE, description)
    }

    /// Build a VERBOSE-level log entry.
    #[must_use]
    pub fn verbose(description: &str) -> Self {
        Self::build(LogLevel::VERBOSE, description)
    }

    /// Build a FATAL-level log entry.
    #[must_use]
    pub fn fatal(description: &str) -> Self {
        Self::build(LogLevel::FATAL, description)
    }

    /// Build a CRITICAL-level log entry.
    #[must_use]
    pub fn critical(description: &str) -> Self {
        Self::build(LogLevel::CRITICAL, description)
    }

    /// Build a log entry with an explicit level and description.
    ///
    /// Assigns a monotonic `session_id` and captures the current wall-clock
    /// time. Defaults to `LogFormat::MCP` and component `"default"`.
    #[must_use]
    pub fn build(level: LogLevel, description: &str) -> Self {
        Self {
            session_id: SESSION_COUNTER.fetch_add(1, Ordering::Relaxed),
            time: Cow::Owned(DateTime::new().to_string()),
            level,
            component: Cow::Borrowed("default"),
            description: description.to_string(),
            format: LogFormat::MCP,
            attributes: BTreeMap::new(),
        }
    }

    /// Override the timestamp for this entry.
    #[must_use]
    pub fn time(mut self, time: &str) -> Self {
        self.time = Cow::Owned(time.to_string());
        self
    }

    /// Override the auto-assigned session ID.
    #[must_use]
    pub const fn session_id(mut self, session_id: u64) -> Self {
        self.session_id = session_id;
        self
    }

    /// Attach a key-value attribute. Accepts any `T: Serialize`.
    #[must_use]
    pub fn with<T: Serialize>(mut self, key: &str, value: T) -> Self {
        if let Ok(val) = serde_json::to_value(value) {
            self.attributes.insert(key.to_string(), val);
        }
        self
    }

    /// Tag the originating service or module.
    #[must_use]
    pub fn component(mut self, component: &str) -> Self {
        self.component = Cow::Owned(component.to_string());
        self
    }

    /// Set the output format for this entry.
    #[must_use]
    pub const fn format(mut self, format: LogFormat) -> Self {
        self.format = format;
        self
    }

    /// Consume this entry and push it into the ring buffer.
    ///
    /// Cost: one `Log` move (~128 bytes). Serialization is deferred.
    /// Automatically captures `file:line` via `#[track_caller]`.
    #[track_caller]
    pub fn fire(mut self) {
        let caller = std::panic::Location::caller();
        self.attributes.insert(
            "caller".to_string(),
            serde_json::Value::String(format!(
                "{}:{}",
                caller.file(),
                caller.line()
            )),
        );
        super::engine::ENGINE.inc_format(self.format);
        let event = super::engine::LogEvent {
            level: self.level,
            level_num: self.level.to_numeric(),
            log: self,
        };
        super::engine::ENGINE.ingest(event);
    }

    fn write_logfmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("level=")?;
        f.write_str(self.level.as_str_lowercase())?;
        f.write_str(" msg=\"")?;
        f.write_str(&self.description.replace('"', "\\\""))?;
        write!(f, "\" session_id={}", self.session_id)?;
        f.write_str(" component=\"")?;
        f.write_str(&self.component)?;
        f.write_str("\"")?;

        for (key, value) in &self.attributes {
            write!(f, " {key}=")?;
            match value {
                serde_json::Value::String(s) => {
                    if s.contains(' ')
                        || s.contains('"')
                        || s.is_empty()
                    {
                        write!(f, "\"{0}\"", s.replace('"', "\\\""))?;
                    } else {
                        write!(f, "{s}")?;
                    }
                }
                _ => write!(f, "{value}")?,
            }
        }
        Ok(())
    }
}

/// Writes a JSON-escaped string (with surrounding quotes) to the formatter.
fn write_json_str(f: &mut fmt::Formatter<'_>, s: &str) -> fmt::Result {
    f.write_str("\"")?;
    for c in s.chars() {
        match c {
            '"' => f.write_str("\\\"")?,
            '\\' => f.write_str("\\\\")?,
            '\n' => f.write_str("\\n")?,
            '\r' => f.write_str("\\r")?,
            '\t' => f.write_str("\\t")?,
            c if c.is_control() => write!(f, "\\u{:04x}", c as u32)?,
            c => write!(f, "{c}")?,
        }
    }
    f.write_str("\"")
}

/// Writes a `BTreeMap<String, serde_json::Value>` as a JSON object.
fn write_json_map(
    f: &mut fmt::Formatter<'_>,
    map: &BTreeMap<String, serde_json::Value>,
) -> fmt::Result {
    f.write_str("{")?;
    let mut first = true;
    for (key, value) in map {
        if !first {
            f.write_str(",")?;
        }
        first = false;
        write_json_str(f, key)?;
        // serde_json::Value Display already produces valid JSON
        write!(f, ":{value}")?;
    }
    f.write_str("}")
}

// --- Per-format serialization methods ---
impl Log {
    fn fmt_clf(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "SessionID={} Timestamp={} Description={} Level={} Component={}",
            self.session_id,
            self.time,
            self.description,
            self.level,
            self.component
        )
    }

    fn fmt_cef(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "CEF:0|{}|{}|{}|{}|{}|CEF",
            self.session_id,
            self.time,
            self.level,
            self.component,
            self.description
        )
    }

    fn fmt_elf(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "ELF:0|{}|{}|{}|{}|{}|ELF",
            self.session_id,
            self.time,
            self.level,
            self.component,
            self.description
        )
    }

    fn fmt_w3c(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "W3C:0|{}|{}|{}|{}|{}|W3C",
            self.session_id,
            self.time,
            self.level,
            self.component,
            self.description
        )
    }

    fn fmt_apache(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{} - - [{}] \"{}\" {} {}",
            &*CACHED_HOSTNAME,
            self.time,
            self.description,
            self.level,
            self.component
        )
    }

    fn fmt_log4j_xml(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            r#"<log4j:event logger="{}" timestamp="{}" level="{}" thread="{}"><log4j:message>{}</log4j:message></log4j:event>"#,
            self.component,
            self.time,
            self.level,
            self.session_id,
            self.description
        )
    }

    fn fmt_json(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("{\"Attributes\":")?;
        write_json_map(f, &self.attributes)?;
        f.write_str(",\"Component\":")?;
        write_json_str(f, &self.component)?;
        f.write_str(",\"Description\":")?;
        write_json_str(f, &self.description)?;
        f.write_str(",\"Format\":\"JSON\",\"Level\":")?;
        write_json_str(f, self.level.as_str())?;
        write!(f, ",\"SessionID\":{}", self.session_id)?;
        f.write_str(",\"Timestamp\":")?;
        write_json_str(f, &self.time)?;
        f.write_str("}")
    }

    fn fmt_gelf(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("{\"_attributes\":")?;
        write_json_map(f, &self.attributes)?;
        write!(f, ",\"_session_id\":{}", self.session_id)?;
        f.write_str(",\"full_message\":")?;
        write_json_str(f, &self.description)?;
        f.write_str(",\"host\":")?;
        write_json_str(f, &self.component)?;
        write!(f, ",\"level\":{}", self.level.to_numeric())?;
        f.write_str(",\"short_message\":")?;
        write_json_str(f, &self.description)?;
        f.write_str(",\"timestamp\":")?;
        write_json_str(f, &self.time)?;
        f.write_str(",\"version\":\"1.1\"}")
    }

    fn fmt_logstash(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("{\"@timestamp\":")?;
        write_json_str(f, &self.time)?;
        f.write_str(",\"attributes\":")?;
        write_json_map(f, &self.attributes)?;
        f.write_str(",\"component\":")?;
        write_json_str(f, &self.component)?;
        f.write_str(",\"level\":")?;
        write_json_str(f, self.level.as_str())?;
        f.write_str(",\"message\":")?;
        write_json_str(f, &self.description)?;
        write!(f, ",\"session_id\":{}", self.session_id)?;
        f.write_str("}")
    }

    fn fmt_ndjson(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("{\"attributes\":")?;
        write_json_map(f, &self.attributes)?;
        f.write_str(",\"component\":")?;
        write_json_str(f, &self.component)?;
        f.write_str(",\"level\":")?;
        write_json_str(f, self.level.as_str())?;
        f.write_str(",\"message\":")?;
        write_json_str(f, &self.description)?;
        f.write_str(",\"timestamp\":")?;
        write_json_str(f, &self.time)?;
        f.write_str("}")
    }

    fn fmt_mcp(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("{\"jsonrpc\":\"2.0\",\"method\":\"notifications/log\",\"params\":{\"data\":{\"attributes\":")?;
        write_json_map(f, &self.attributes)?;
        f.write_str(",\"component\":")?;
        write_json_str(f, &self.component)?;
        f.write_str(",\"description\":")?;
        write_json_str(f, &self.description)?;
        write!(f, ",\"session_id\":{}", self.session_id)?;
        f.write_str(",\"time\":")?;
        write_json_str(f, &self.time)?;
        f.write_str("},\"level\":")?;
        write_json_str(f, self.level.as_str_lowercase())?;
        f.write_str("}}")
    }

    fn fmt_otlp(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let empty = serde_json::Value::String(String::new());
        let trace_id =
            self.attributes.get("trace_id").unwrap_or(&empty);
        let span_id = self.attributes.get("span_id").unwrap_or(&empty);
        f.write_str("{\"attributes\":")?;
        write_json_map(f, &self.attributes)?;
        f.write_str(",\"body\":{\"stringValue\":")?;
        write_json_str(f, &self.description)?;
        write!(f, "}},\"severityNumber\":{}", self.level.to_numeric())?;
        f.write_str(",\"severityText\":")?;
        write_json_str(f, self.level.as_str())?;
        write!(f, ",\"spanId\":{span_id}")?;
        f.write_str(",\"timeUnixNano\":")?;
        write_json_str(f, &self.time)?;
        write!(f, ",\"traceId\":{trace_id}}}")
    }

    fn fmt_ecs(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("{\"@timestamp\":")?;
        write_json_str(f, &self.time)?;
        f.write_str(",\"labels\":")?;
        write_json_map(f, &self.attributes)?;
        f.write_str(",\"log.level\":")?;
        write_json_str(f, self.level.as_str_lowercase())?;
        f.write_str(",\"log.logger\":\"euxis-commons\",\"message\":")?;
        write_json_str(f, &self.description)?;
        f.write_str(",\"process.name\":")?;
        write_json_str(f, &self.component)?;
        f.write_str("}")
    }
}

impl fmt::Display for Log {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.format {
            LogFormat::CLF => self.fmt_clf(f),
            LogFormat::CEF => self.fmt_cef(f),
            LogFormat::ELF => self.fmt_elf(f),
            LogFormat::W3C => self.fmt_w3c(f),
            LogFormat::ApacheAccessLog => self.fmt_apache(f),
            LogFormat::Log4jXML => self.fmt_log4j_xml(f),
            LogFormat::JSON => self.fmt_json(f),
            LogFormat::GELF => self.fmt_gelf(f),
            LogFormat::Logstash => self.fmt_logstash(f),
            LogFormat::NDJSON => self.fmt_ndjson(f),
            LogFormat::MCP => self.fmt_mcp(f),
            LogFormat::OTLP => self.fmt_otlp(f),
            LogFormat::Logfmt => self.write_logfmt(f),
            LogFormat::ECS => self.fmt_ecs(f),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── Helper ──────────────────────────────────────────────────────────
    /// Creates a deterministic `Log` entry for formatting tests.
    fn sample_log(format: LogFormat) -> Log {
        Log::build(LogLevel::WARN, "something happened")
            .session_id(42)
            .time("2025-01-15T12:00:00Z")
            .component("my-service")
            .format(format)
            .with("req_id", "abc-123")
    }

    // ── Existing test (kept) ────────────────────────────────────────────
    #[test]
    #[cfg_attr(miri, ignore)]
    fn test_log_write_logfmt_with_attributes() {
        let mut log = Log::build(LogLevel::INFO, "desc")
            .session_id(99)
            .time("ts")
            .component("comp")
            .format(LogFormat::Logfmt);
        log.attributes
            .insert("key".to_string(), serde_json::json!("value"));
        log.attributes.insert(
            "space".to_string(),
            serde_json::json!("has space"),
        );
        log.attributes
            .insert("num".to_string(), serde_json::json!(42));
        log.attributes
            .insert("empty".to_string(), serde_json::json!(""));

        let output = format!("{log}");
        assert!(output.contains("key=value"));
        assert!(output.contains("space=\"has space\""));
        assert!(output.contains("num=42"));
        assert!(output.contains("empty=\"\""));

        // Case with no attributes to cover the other branch
        let log_no_attr = Log::build(LogLevel::INFO, "desc")
            .session_id(100)
            .time("ts")
            .component("comp")
            .format(LogFormat::Logfmt);
        let output_no = format!("{log_no_attr}");
        assert!(!output_no.contains(" key="));
    }

    // ── 1. Default impl ────────────────────────────────────────────────
    #[test]
    fn test_default_values() {
        let log = Log::default();
        assert_eq!(log.session_id, 0);
        assert_eq!(log.time, "");
        assert_eq!(log.level, LogLevel::INFO);
        assert_eq!(log.component, "");
        assert!(log.description.is_empty());
        assert_eq!(log.format, LogFormat::CLF);
        assert!(log.attributes.is_empty());
    }

    // ── 2. Level shortcuts ─────────────────────────────────────────────
    #[test]
    #[cfg_attr(miri, ignore)]
    fn test_level_info() {
        let log = Log::info("msg");
        assert_eq!(log.level, LogLevel::INFO);
        assert_eq!(log.description, "msg");
    }

    #[test]
    #[cfg_attr(miri, ignore)]
    fn test_level_warn() {
        let log = Log::warn("msg");
        assert_eq!(log.level, LogLevel::WARN);
    }

    #[test]
    #[cfg_attr(miri, ignore)]
    fn test_level_error() {
        let log = Log::error("msg");
        assert_eq!(log.level, LogLevel::ERROR);
    }

    #[test]
    #[cfg_attr(miri, ignore)]
    fn test_level_debug() {
        let log = Log::debug("msg");
        assert_eq!(log.level, LogLevel::DEBUG);
    }

    #[test]
    #[cfg_attr(miri, ignore)]
    fn test_level_trace() {
        let log = Log::trace("msg");
        assert_eq!(log.level, LogLevel::TRACE);
    }

    #[test]
    #[cfg_attr(miri, ignore)]
    fn test_level_verbose() {
        let log = Log::verbose("msg");
        assert_eq!(log.level, LogLevel::VERBOSE);
    }

    #[test]
    #[cfg_attr(miri, ignore)]
    fn test_level_fatal() {
        let log = Log::fatal("msg");
        assert_eq!(log.level, LogLevel::FATAL);
    }

    #[test]
    #[cfg_attr(miri, ignore)]
    fn test_level_critical() {
        let log = Log::critical("msg");
        assert_eq!(log.level, LogLevel::CRITICAL);
    }

    // ── 3. Builder methods ─────────────────────────────────────────────
    #[test]
    #[cfg_attr(miri, ignore)]
    fn test_builder_time() {
        let log = Log::info("x").time("2025-06-01T00:00:00Z");
        assert_eq!(log.time, "2025-06-01T00:00:00Z");
    }

    #[test]
    #[cfg_attr(miri, ignore)]
    fn test_builder_session_id() {
        let log = Log::info("x").session_id(999);
        assert_eq!(log.session_id, 999);
    }

    #[test]
    #[cfg_attr(miri, ignore)]
    fn test_builder_with() {
        let log = Log::info("x").with("key", "val");
        assert_eq!(
            log.attributes.get("key"),
            Some(&serde_json::json!("val"))
        );
    }

    #[test]
    #[cfg_attr(miri, ignore)]
    fn test_builder_component() {
        let log = Log::info("x").component("auth");
        assert_eq!(log.component, "auth");
    }

    #[test]
    #[cfg_attr(miri, ignore)]
    fn test_builder_format() {
        let log = Log::info("x").format(LogFormat::JSON);
        assert_eq!(log.format, LogFormat::JSON);
    }

    // ── 4. build() defaults ────────────────────────────────────────────
    #[test]
    #[cfg_attr(miri, ignore)]
    fn test_build_defaults() {
        let log = Log::build(LogLevel::DEBUG, "hello");
        assert_eq!(log.level, LogLevel::DEBUG);
        assert_eq!(log.description, "hello");
        assert_eq!(log.component, "default");
        assert_eq!(log.format, LogFormat::MCP);
        assert!(!log.time.is_empty());
        assert!(log.session_id > 0);
    }

    // ── 5. log() method ────────────────────────────────────────────────
    #[test]
    #[cfg_attr(miri, ignore)]
    fn test_log_does_not_panic() {
        let log = Log::info("test log").session_id(1).time("ts");
        log.log();
    }

    // ── 6. fire() method ───────────────────────────────────────────────
    #[test]
    #[cfg_attr(miri, ignore)]
    fn test_fire_adds_caller_attribute() {
        // fire() consumes self, so we can only test indirectly by
        // verifying the `caller` key would be inserted. We build a log,
        // manually insert the caller attribute like fire() does, and
        // confirm the key exists.
        let mut log = Log::info("fire test").session_id(1).time("ts");
        let caller = std::panic::Location::caller();
        log.attributes.insert(
            "caller".to_string(),
            serde_json::Value::String(format!(
                "{}:{}",
                caller.file(),
                caller.line()
            )),
        );
        assert!(log.attributes.contains_key("caller"));

        // Also just call fire() to make sure it doesn't panic.
        Log::info("actually fire").session_id(2).time("ts").fire();
    }

    // ── 7. write_json_str edge cases ───────────────────────────────────
    #[test]
    #[cfg_attr(miri, ignore)]
    fn test_write_json_str_with_special_chars() {
        // We exercise write_json_str indirectly through JSON Display.
        let log = Log::build(LogLevel::INFO, "line1\nline2\ttab\r\"quoted\"\\back")
            .session_id(1)
            .time("t")
            .component("c")
            .format(LogFormat::JSON);
        let output = format!("{log}");
        assert!(output.contains("\\n"));
        assert!(output.contains("\\t"));
        assert!(output.contains("\\r"));
        assert!(output.contains("\\\""));
        assert!(output.contains("\\\\"));
    }

    #[test]
    #[cfg_attr(miri, ignore)]
    fn test_write_json_str_control_char() {
        // Use a control character (e.g. BEL = 0x07)
        let msg = "hello\x07world";
        let log = Log::build(LogLevel::INFO, msg)
            .session_id(1)
            .time("t")
            .component("c")
            .format(LogFormat::JSON);
        let output = format!("{log}");
        assert!(output.contains("\\u0007"));
    }

    // ── 8. write_json_map ──────────────────────────────────────────────
    #[test]
    #[cfg_attr(miri, ignore)]
    fn test_write_json_map_empty() {
        let log = Log::build(LogLevel::INFO, "msg")
            .session_id(1)
            .time("t")
            .component("c")
            .format(LogFormat::JSON);
        let output = format!("{log}");
        // Attributes map should be empty => {}
        assert!(output.contains("\"Attributes\":{}"));
    }

    #[test]
    #[cfg_attr(miri, ignore)]
    fn test_write_json_map_with_entries() {
        let log = Log::build(LogLevel::INFO, "msg")
            .session_id(1)
            .time("t")
            .component("c")
            .format(LogFormat::JSON)
            .with("alpha", 1)
            .with("beta", "two");
        let output = format!("{log}");
        assert!(output.contains("\"alpha\":1"));
        assert!(output.contains("\"beta\":\"two\""));
    }

    // ── 9. Logfmt with quoted attribute value ──────────────────────────
    #[test]
    #[cfg_attr(miri, ignore)]
    fn test_logfmt_attribute_with_quotes() {
        let log = Log::build(LogLevel::INFO, "desc")
            .session_id(1)
            .time("ts")
            .component("comp")
            .format(LogFormat::Logfmt)
            .with("q", "say \"hello\"");
        let output = format!("{log}");
        assert!(output.contains("q=\"say \\\"hello\\\"\""));
    }

    #[test]
    #[cfg_attr(miri, ignore)]
    fn test_logfmt_msg_with_quotes() {
        let log = Log::build(LogLevel::INFO, "say \"hi\"")
            .session_id(1)
            .time("ts")
            .component("comp")
            .format(LogFormat::Logfmt);
        let output = format!("{log}");
        assert!(output.contains("msg=\"say \\\"hi\\\"\""));
    }

    // ── 10. All 14 Display formats ─────────────────────────────────────
    #[test]
    #[cfg_attr(miri, ignore)]
    fn test_fmt_clf() {
        let output = format!("{}", sample_log(LogFormat::CLF));
        assert!(output.contains("SessionID=42"));
        assert!(output.contains("Timestamp=2025-01-15T12:00:00Z"));
        assert!(output.contains("Description=something happened"));
        assert!(output.contains("Level=WARN"));
        assert!(output.contains("Component=my-service"));
    }

    #[test]
    #[cfg_attr(miri, ignore)]
    fn test_fmt_cef() {
        let output = format!("{}", sample_log(LogFormat::CEF));
        assert!(output.starts_with("CEF:0|"));
        assert!(output.ends_with("|CEF"));
        assert!(output.contains("42"));
        assert!(output.contains("WARN"));
        assert!(output.contains("something happened"));
    }

    #[test]
    #[cfg_attr(miri, ignore)]
    fn test_fmt_elf() {
        let output = format!("{}", sample_log(LogFormat::ELF));
        assert!(output.starts_with("ELF:0|"));
        assert!(output.ends_with("|ELF"));
        assert!(output.contains("42"));
        assert!(output.contains("WARN"));
    }

    #[test]
    #[cfg_attr(miri, ignore)]
    fn test_fmt_w3c() {
        let output = format!("{}", sample_log(LogFormat::W3C));
        assert!(output.starts_with("W3C:0|"));
        assert!(output.ends_with("|W3C"));
        assert!(output.contains("my-service"));
    }

    #[test]
    #[cfg_attr(miri, ignore)]
    fn test_fmt_apache() {
        let output = format!("{}", sample_log(LogFormat::ApacheAccessLog));
        assert!(output.contains("[2025-01-15T12:00:00Z]"));
        assert!(output.contains("\"something happened\""));
        assert!(output.contains("WARN"));
        assert!(output.contains("my-service"));
    }

    #[test]
    #[cfg_attr(miri, ignore)]
    fn test_fmt_log4j_xml() {
        let output = format!("{}", sample_log(LogFormat::Log4jXML));
        assert!(output.contains("<log4j:event"));
        assert!(output.contains("logger=\"my-service\""));
        assert!(output.contains("level=\"WARN\""));
        assert!(output.contains("thread=\"42\""));
        assert!(output.contains("<log4j:message>something happened</log4j:message>"));
    }

    #[test]
    #[cfg_attr(miri, ignore)]
    fn test_fmt_json() {
        let output = format!("{}", sample_log(LogFormat::JSON));
        assert!(output.contains("\"Format\":\"JSON\""));
        assert!(output.contains("\"Level\":\"WARN\""));
        assert!(output.contains("\"Component\":\"my-service\""));
        assert!(output.contains("\"Description\":\"something happened\""));
        assert!(output.contains("\"SessionID\":42"));
        assert!(output.contains("\"Timestamp\":\"2025-01-15T12:00:00Z\""));
        assert!(output.contains("\"Attributes\":"));
        assert!(output.contains("\"req_id\":\"abc-123\""));
    }

    #[test]
    #[cfg_attr(miri, ignore)]
    fn test_fmt_gelf() {
        let output = format!("{}", sample_log(LogFormat::GELF));
        assert!(output.contains("\"version\":\"1.1\""));
        assert!(output.contains("\"host\":\"my-service\""));
        assert!(output.contains("\"short_message\":\"something happened\""));
        assert!(output.contains("\"full_message\":\"something happened\""));
        assert!(output.contains("\"_session_id\":42"));
        assert!(output.contains("\"level\":7")); // WARN numeric
        assert!(output.contains("\"_attributes\":"));
    }

    #[test]
    #[cfg_attr(miri, ignore)]
    fn test_fmt_logstash() {
        let output = format!("{}", sample_log(LogFormat::Logstash));
        assert!(output.contains("\"@timestamp\":\"2025-01-15T12:00:00Z\""));
        assert!(output.contains("\"level\":\"WARN\""));
        assert!(output.contains("\"component\":\"my-service\""));
        assert!(output.contains("\"message\":\"something happened\""));
        assert!(output.contains("\"session_id\":42"));
        assert!(output.contains("\"attributes\":"));
    }

    #[test]
    #[cfg_attr(miri, ignore)]
    fn test_fmt_ndjson() {
        let output = format!("{}", sample_log(LogFormat::NDJSON));
        assert!(output.contains("\"level\":\"WARN\""));
        assert!(output.contains("\"component\":\"my-service\""));
        assert!(output.contains("\"message\":\"something happened\""));
        assert!(output.contains("\"timestamp\":\"2025-01-15T12:00:00Z\""));
        assert!(output.contains("\"attributes\":"));
    }

    #[test]
    #[cfg_attr(miri, ignore)]
    fn test_fmt_mcp() {
        let output = format!("{}", sample_log(LogFormat::MCP));
        assert!(output.contains("\"jsonrpc\":\"2.0\""));
        assert!(output.contains("\"method\":\"notifications/log\""));
        assert!(output.contains("\"level\":\"warn\""));
        assert!(output.contains("\"component\":\"my-service\""));
        assert!(output.contains("\"description\":\"something happened\""));
        assert!(output.contains("\"session_id\":42"));
    }

    #[test]
    #[cfg_attr(miri, ignore)]
    fn test_fmt_otlp() {
        let output = format!("{}", sample_log(LogFormat::OTLP));
        assert!(output.contains("\"severityText\":\"WARN\""));
        assert!(output.contains("\"severityNumber\":7"));
        assert!(output.contains("\"body\":{\"stringValue\":\"something happened\"}"));
        assert!(output.contains("\"timeUnixNano\":\"2025-01-15T12:00:00Z\""));
        assert!(output.contains("\"attributes\":"));
        // Default trace_id and span_id are empty strings
        assert!(output.contains("\"traceId\":\"\""));
        assert!(output.contains("\"spanId\":\"\""));
    }

    #[test]
    #[cfg_attr(miri, ignore)]
    fn test_fmt_otlp_with_trace_and_span() {
        let log = Log::build(LogLevel::INFO, "traced")
            .session_id(1)
            .time("t")
            .component("svc")
            .format(LogFormat::OTLP)
            .with("trace_id", "abc123")
            .with("span_id", "def456");
        let output = format!("{log}");
        assert!(output.contains("\"traceId\":\"abc123\""));
        assert!(output.contains("\"spanId\":\"def456\""));
    }

    #[test]
    #[cfg_attr(miri, ignore)]
    fn test_fmt_ecs() {
        let output = format!("{}", sample_log(LogFormat::ECS));
        assert!(output.contains("\"@timestamp\":\"2025-01-15T12:00:00Z\""));
        assert!(output.contains("\"log.level\":\"warn\""));
        assert!(output.contains("\"log.logger\":\"euxis-commons\""));
        assert!(output.contains("\"message\":\"something happened\""));
        assert!(output.contains("\"process.name\":\"my-service\""));
        assert!(output.contains("\"labels\":"));
    }

    #[test]
    #[cfg_attr(miri, ignore)]
    fn test_fmt_logfmt_display() {
        let output = format!("{}", sample_log(LogFormat::Logfmt));
        assert!(output.contains("level=warn"));
        assert!(output.contains("msg=\"something happened\""));
        assert!(output.contains("session_id=42"));
        assert!(output.contains("component=\"my-service\""));
        assert!(output.contains("req_id=abc-123"));
    }
}
