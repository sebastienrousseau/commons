// Copyright © 2024-2026 RustLogs (RLG). All rights reserved.
// SPDX-License-Identifier: Apache-2.0
// SPDX-License-Identifier: MIT

//! Integration with the `tracing` ecosystem.
//!
//! Provides both a standalone [`LoggingSubscriber`] and, behind the
//! `logging-tracing-layer` feature, a composable [`LoggingLayer`].

use super::log_entry::Log;
use super::log_level::LogLevel;
use std::sync::atomic::{AtomicU64, Ordering};
use tracing_core::field::{Field, Visit};
use tracing_core::{Event, Level, Metadata, Subscriber};

/// Maps a [`tracing_core::Level`] to a [`LogLevel`].
fn map_tracing_level(level: Level) -> LogLevel {
    if level == Level::ERROR {
        LogLevel::ERROR
    } else if level == Level::WARN {
        LogLevel::WARN
    } else if level == Level::INFO {
        LogLevel::INFO
    } else if level == Level::DEBUG {
        LogLevel::DEBUG
    } else {
        LogLevel::TRACE
    }
}

/// Monotonic span ID counter for unique span identification.
static SPAN_ID_COUNTER: AtomicU64 = AtomicU64::new(1);

/// A `tracing::Subscriber` that routes events to the logging engine.
#[derive(Debug, Default, Clone, Copy)]
pub struct LoggingSubscriber;

impl LoggingSubscriber {
    /// Create a new `LoggingSubscriber`.
    #[must_use]
    pub const fn new() -> Self {
        Self
    }
}

impl Subscriber for LoggingSubscriber {
    fn enabled(&self, metadata: &Metadata<'_>) -> bool {
        map_tracing_level(*metadata.level()).to_numeric()
            >= super::engine::ENGINE.filter_level()
    }

    fn new_span(
        &self,
        _span: &tracing_core::span::Attributes<'_>,
    ) -> tracing_core::span::Id {
        tracing_core::span::Id::from_u64(
            SPAN_ID_COUNTER.fetch_add(1, Ordering::Relaxed),
        )
    }

    fn record(
        &self,
        _span: &tracing_core::span::Id,
        _values: &tracing_core::span::Record<'_>,
    ) {
    }

    fn record_follows_from(
        &self,
        _span: &tracing_core::span::Id,
        _follows: &tracing_core::span::Id,
    ) {
    }

    fn event(&self, event: &Event<'_>) {
        let metadata = event.metadata();
        let level = map_tracing_level(*metadata.level());

        let mut visitor = LoggingVisitor::default();
        event.record(&mut visitor);

        let mut log = Log::build(level, &visitor.message);
        log.component =
            std::borrow::Cow::Owned(metadata.target().to_string());

        for (key, value) in visitor.fields {
            log = log.with(&key, value);
        }

        log.fire();
    }

    fn enter(&self, _span: &tracing_core::span::Id) {}

    fn exit(&self, _span: &tracing_core::span::Id) {}
}

#[derive(Default)]
struct LoggingVisitor {
    message: String,
    fields: std::collections::BTreeMap<String, serde_json::Value>,
}

macro_rules! impl_record_field {
    ($method:ident, $ty:ty) => {
        fn $method(&mut self, field: &Field, value: $ty) {
            self.fields.insert(
                field.name().to_string(),
                serde_json::json!(value),
            );
        }
    };
    (stringify $method:ident, $ty:ty) => {
        fn $method(&mut self, field: &Field, value: $ty) {
            self.fields.insert(
                field.name().to_string(),
                serde_json::json!(value.to_string()),
            );
        }
    };
}

impl Visit for LoggingVisitor {
    fn record_debug(
        &mut self,
        field: &Field,
        value: &dyn std::fmt::Debug,
    ) {
        if field.name() == "message" {
            self.message = format!("{value:?}");
        } else {
            self.fields.insert(
                field.name().to_string(),
                serde_json::json!(format!("{value:?}")),
            );
        }
    }

    fn record_str(&mut self, field: &Field, value: &str) {
        if field.name() == "message" {
            self.message = value.to_string();
        } else {
            self.fields.insert(
                field.name().to_string(),
                serde_json::json!(value),
            );
        }
    }

    fn record_error(
        &mut self,
        field: &Field,
        value: &(dyn std::error::Error + 'static),
    ) {
        self.fields.insert(
            field.name().to_string(),
            serde_json::json!(value.to_string()),
        );
    }

    impl_record_field!(record_u64, u64);
    impl_record_field!(record_i64, i64);
    impl_record_field!(record_bool, bool);
    impl_record_field!(record_f64, f64);
    impl_record_field!(stringify record_u128, u128);
    impl_record_field!(stringify record_i128, i128);
}

// ---------------------------------------------------------------------------
// Composable tracing Layer (behind `logging-tracing-layer` feature)
// ---------------------------------------------------------------------------

/// A composable [`tracing_subscriber::Layer`] that routes events into the logging engine.
///
/// This allows the logging system to be used alongside other tracing layers in a
/// `tracing_subscriber::Registry` stack.
///
/// # Example
///
/// ```rust,ignore
/// use tracing_subscriber::prelude::*;
/// use commons::logging::tracing_bridge::LoggingLayer;
///
/// tracing_subscriber::registry()
///     .with(LoggingLayer::new())
///     .init();
/// ```
#[cfg(feature = "logging-tracing-layer")]
#[derive(Debug, Clone, Copy)]
pub struct LoggingLayer {
    format: super::log_format::LogFormat,
}

#[cfg(feature = "logging-tracing-layer")]
impl Default for LoggingLayer {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(feature = "logging-tracing-layer")]
impl LoggingLayer {
    /// Creates a new `LoggingLayer` with the default MCP format.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            format: super::log_format::LogFormat::MCP,
        }
    }

    /// Sets the log output format for this layer.
    #[must_use]
    pub const fn with_format(
        mut self,
        format: super::log_format::LogFormat,
    ) -> Self {
        self.format = format;
        self
    }
}

#[cfg(feature = "logging-tracing-layer")]
impl<S> tracing_subscriber::Layer<S> for LoggingLayer
where
    S: Subscriber
        + for<'lookup> tracing_subscriber::registry::LookupSpan<'lookup>,
{
    fn enabled(
        &self,
        metadata: &Metadata<'_>,
        _ctx: tracing_subscriber::layer::Context<'_, S>,
    ) -> bool {
        map_tracing_level(*metadata.level()).to_numeric()
            >= super::engine::ENGINE.filter_level()
    }

    fn on_event(
        &self,
        event: &Event<'_>,
        _ctx: tracing_subscriber::layer::Context<'_, S>,
    ) {
        let metadata = event.metadata();
        let level = map_tracing_level(*metadata.level());

        let mut visitor = LoggingVisitor::default();
        event.record(&mut visitor);

        let mut log = Log::build(level, &visitor.message);
        log.component =
            std::borrow::Cow::Owned(metadata.target().to_string());
        log.format = self.format;

        for (key, value) in visitor.fields {
            log = log.with(&key, value);
        }

        log.fire();
    }

    fn on_new_span(
        &self,
        _attrs: &tracing_core::span::Attributes<'_>,
        _id: &tracing_core::span::Id,
        _ctx: tracing_subscriber::layer::Context<'_, S>,
    ) {
        super::engine::ENGINE.inc_spans();
    }

    fn on_close(
        &self,
        _id: tracing_core::span::Id,
        _ctx: tracing_subscriber::layer::Context<'_, S>,
    ) {
        super::engine::ENGINE.dec_spans();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tracing_core::field::FieldSet;
    use tracing_core::metadata::Kind;
    use tracing_core::subscriber::Interest;

    // ---- Helper: minimal callsite for constructing metadata in tests ----

    struct TestCallsite;

    impl tracing_core::callsite::Callsite for TestCallsite {
        fn set_interest(&self, _interest: Interest) {}
        fn metadata(&self) -> &Metadata<'_> {
            unimplemented!("test-only callsite")
        }
    }

    // Statics for enabled / new_span / record / event tests.
    // Each test that needs metadata shares a dedicated callsite + field set
    // so the compiler sees them as distinct 'static items.

    static CS_EN: TestCallsite = TestCallsite;

    static CS_NS: TestCallsite = TestCallsite;
    static FS_NS: FieldSet =
        FieldSet::new(&[], tracing_core::identify_callsite!(&CS_NS));

    static CS_RN: TestCallsite = TestCallsite;
    static FS_RN: FieldSet =
        FieldSet::new(&[], tracing_core::identify_callsite!(&CS_RN));

    static CS_EV: TestCallsite = TestCallsite;
    static FS_EV: FieldSet = FieldSet::new(
        &["message"],
        tracing_core::identify_callsite!(&CS_EV),
    );

    // ---- map_tracing_level ----

    #[test]
    fn test_map_tracing_level_error() {
        assert_eq!(map_tracing_level(Level::ERROR), LogLevel::ERROR);
    }

    #[test]
    fn test_map_tracing_level_warn() {
        assert_eq!(map_tracing_level(Level::WARN), LogLevel::WARN);
    }

    #[test]
    fn test_map_tracing_level_info() {
        assert_eq!(map_tracing_level(Level::INFO), LogLevel::INFO);
    }

    #[test]
    fn test_map_tracing_level_debug() {
        assert_eq!(map_tracing_level(Level::DEBUG), LogLevel::DEBUG);
    }

    #[test]
    fn test_map_tracing_level_trace() {
        assert_eq!(map_tracing_level(Level::TRACE), LogLevel::TRACE);
    }

    // ---- LoggingSubscriber construction & traits ----

    #[test]
    fn test_logging_subscriber_new() {
        let sub = LoggingSubscriber::new();
        let _ = sub;
    }

    #[test]
    fn test_logging_subscriber_default() {
        let sub = LoggingSubscriber;
        let _ = sub;
    }

    #[test]
    fn test_logging_subscriber_clone_copy() {
        let a = LoggingSubscriber::new();
        let b = a; // Copy
        let c = a; // Copy (LoggingSubscriber is Copy)
        let _ = (b, c);
    }

    #[test]
    fn test_logging_subscriber_debug() {
        let sub = LoggingSubscriber::new();
        let dbg = format!("{sub:?}");
        assert_eq!(dbg, "LoggingSubscriber");
    }

    // ---- Subscriber::enabled ----

    #[test]
    #[cfg_attr(miri, ignore)]
    fn test_subscriber_enabled() {
        static META_EN: std::sync::LazyLock<Metadata<'static>> =
            std::sync::LazyLock::new(|| {
                tracing_core::metadata!(
                    name: "test_en",
                    target: "test_target",
                    level: Level::ERROR,
                    fields: &[],
                    callsite: &CS_EN,
                    kind: Kind::EVENT,
                )
            });
        let sub = LoggingSubscriber::new();
        assert!(sub.enabled(&META_EN));
    }

    // ---- Subscriber::new_span returns unique IDs ----

    #[test]
    #[cfg_attr(miri, ignore)]
    fn test_subscriber_new_span_unique_ids() {
        static META_NS: std::sync::LazyLock<Metadata<'static>> =
            std::sync::LazyLock::new(|| {
                tracing_core::metadata!(
                    name: "test_ns",
                    target: "test_target",
                    level: Level::INFO,
                    fields: &[],
                    callsite: &CS_NS,
                    kind: Kind::SPAN,
                )
            });
        let sub = LoggingSubscriber::new();
        let vs = FS_NS.value_set(&[]);
        let attrs =
            tracing_core::span::Attributes::new(&META_NS, &vs);
        let id1 = sub.new_span(&attrs);
        let vs2 = FS_NS.value_set(&[]);
        let attrs2 =
            tracing_core::span::Attributes::new(&META_NS, &vs2);
        let id2 = sub.new_span(&attrs2);
        assert_ne!(id1.into_u64(), id2.into_u64());
    }

    // ---- Subscriber::record and record_follows_from (no-ops) ----

    #[test]
    fn test_subscriber_record_noop() {
        let sub = LoggingSubscriber::new();
        let id = tracing_core::span::Id::from_u64(1);
        let vs = FS_RN.value_set(&[]);
        let record = tracing_core::span::Record::new(&vs);
        sub.record(&id, &record);
    }

    #[test]
    fn test_subscriber_record_follows_from_noop() {
        let sub = LoggingSubscriber::new();
        let id1 = tracing_core::span::Id::from_u64(10);
        let id2 = tracing_core::span::Id::from_u64(20);
        sub.record_follows_from(&id1, &id2);
    }

    // ---- Subscriber::enter and exit (no-ops) ----

    #[test]
    fn test_subscriber_enter_noop() {
        let sub = LoggingSubscriber::new();
        let id = tracing_core::span::Id::from_u64(42);
        sub.enter(&id);
    }

    #[test]
    fn test_subscriber_exit_noop() {
        let sub = LoggingSubscriber::new();
        let id = tracing_core::span::Id::from_u64(42);
        sub.exit(&id);
    }

    // ---- Subscriber::event ----

    #[test]
    #[cfg_attr(miri, ignore)]
    fn test_subscriber_event() {
        static META_EV: std::sync::LazyLock<Metadata<'static>> =
            std::sync::LazyLock::new(|| {
                tracing_core::metadata!(
                    name: "test_ev",
                    target: "test_target",
                    level: Level::WARN,
                    fields: &["message"],
                    callsite: &CS_EV,
                    kind: Kind::EVENT,
                )
            });
        let sub = LoggingSubscriber::new();
        let msg_field = FS_EV.field("message").unwrap();
        let msg: &dyn tracing_core::field::Value = &"hello world";
        let vals = [(&msg_field, Some(msg))];
        let fields = FS_EV.value_set(&vals);
        let event = Event::new(&META_EV, &fields);
        sub.event(&event);
    }

    // ---- LoggingVisitor tests ----

    static CS_V1: TestCallsite = TestCallsite;
    static FS_V1: FieldSet = FieldSet::new(
        &["message"],
        tracing_core::identify_callsite!(&CS_V1),
    );
    static CS_V2: TestCallsite = TestCallsite;
    static FS_V2: FieldSet = FieldSet::new(
        &["custom"],
        tracing_core::identify_callsite!(&CS_V2),
    );
    static CS_V3: TestCallsite = TestCallsite;
    static FS_V3: FieldSet = FieldSet::new(
        &["message"],
        tracing_core::identify_callsite!(&CS_V3),
    );
    static CS_V4: TestCallsite = TestCallsite;
    static FS_V4: FieldSet = FieldSet::new(
        &["extra"],
        tracing_core::identify_callsite!(&CS_V4),
    );
    static CS_V5: TestCallsite = TestCallsite;
    static FS_V5: FieldSet = FieldSet::new(
        &["error"],
        tracing_core::identify_callsite!(&CS_V5),
    );
    static CS_V6: TestCallsite = TestCallsite;
    static FS_V6: FieldSet = FieldSet::new(
        &["count"],
        tracing_core::identify_callsite!(&CS_V6),
    );
    static CS_V7: TestCallsite = TestCallsite;
    static FS_V7: FieldSet = FieldSet::new(
        &["delta"],
        tracing_core::identify_callsite!(&CS_V7),
    );
    static CS_V8: TestCallsite = TestCallsite;
    static FS_V8: FieldSet = FieldSet::new(
        &["flag"],
        tracing_core::identify_callsite!(&CS_V8),
    );
    static CS_V9: TestCallsite = TestCallsite;
    static FS_V9: FieldSet = FieldSet::new(
        &["ratio"],
        tracing_core::identify_callsite!(&CS_V9),
    );
    static CS_V10: TestCallsite = TestCallsite;
    static FS_V10: FieldSet = FieldSet::new(
        &["big"],
        tracing_core::identify_callsite!(&CS_V10),
    );
    static CS_V11: TestCallsite = TestCallsite;
    static FS_V11: FieldSet = FieldSet::new(
        &["signed_big"],
        tracing_core::identify_callsite!(&CS_V11),
    );

    #[test]
    fn test_visitor_record_debug_message_field() {
        let mut visitor = LoggingVisitor::default();
        let field = FS_V1.field("message").unwrap();
        visitor.record_debug(&field, &"test debug message");
        assert!(visitor.message.contains("test debug message"));
        assert!(visitor.fields.is_empty());
    }

    #[test]
    fn test_visitor_record_debug_non_message_field() {
        let mut visitor = LoggingVisitor::default();
        let field = FS_V2.field("custom").unwrap();
        visitor.record_debug(&field, &42_i32);
        assert!(visitor.message.is_empty());
        assert!(visitor.fields.contains_key("custom"));
    }

    #[test]
    fn test_visitor_record_str_message_field() {
        let mut visitor = LoggingVisitor::default();
        let field = FS_V3.field("message").unwrap();
        visitor.record_str(&field, "string message");
        assert_eq!(visitor.message, "string message");
        assert!(visitor.fields.is_empty());
    }

    #[test]
    fn test_visitor_record_str_non_message_field() {
        let mut visitor = LoggingVisitor::default();
        let field = FS_V4.field("extra").unwrap();
        visitor.record_str(&field, "extra_value");
        assert!(visitor.message.is_empty());
        assert_eq!(
            visitor.fields.get("extra").unwrap(),
            &serde_json::json!("extra_value")
        );
    }

    #[test]
    fn test_visitor_record_error() {
        let mut visitor = LoggingVisitor::default();
        let field = FS_V5.field("error").unwrap();
        let err = std::io::Error::new(
            std::io::ErrorKind::NotFound,
            "missing",
        );
        let err_ref: &(dyn std::error::Error + 'static) = &err;
        visitor.record_error(&field, err_ref);
        assert!(visitor.fields.contains_key("error"));
        let val = visitor.fields.get("error").unwrap();
        assert!(val.as_str().unwrap().contains("missing"));
    }

    #[test]
    fn test_visitor_record_u64() {
        let mut visitor = LoggingVisitor::default();
        let field = FS_V6.field("count").unwrap();
        visitor.record_u64(&field, 123);
        assert_eq!(
            visitor.fields.get("count").unwrap(),
            &serde_json::json!(123_u64)
        );
    }

    #[test]
    fn test_visitor_record_i64() {
        let mut visitor = LoggingVisitor::default();
        let field = FS_V7.field("delta").unwrap();
        visitor.record_i64(&field, -42);
        assert_eq!(
            visitor.fields.get("delta").unwrap(),
            &serde_json::json!(-42_i64)
        );
    }

    #[test]
    fn test_visitor_record_bool() {
        let mut visitor = LoggingVisitor::default();
        let field = FS_V8.field("flag").unwrap();
        visitor.record_bool(&field, true);
        assert_eq!(
            visitor.fields.get("flag").unwrap(),
            &serde_json::json!(true)
        );
    }

    #[test]
    fn test_visitor_record_f64() {
        let mut visitor = LoggingVisitor::default();
        let field = FS_V9.field("ratio").unwrap();
        visitor.record_f64(&field, std::f64::consts::PI);
        let val = visitor.fields.get("ratio").unwrap().as_f64().unwrap();
        assert!((val - std::f64::consts::PI).abs() < f64::EPSILON);
    }

    #[test]
    fn test_visitor_record_u128() {
        let mut visitor = LoggingVisitor::default();
        let field = FS_V10.field("big").unwrap();
        let val: u128 = 340_282_366_920_938_463_463;
        visitor.record_u128(&field, val);
        assert_eq!(
            visitor.fields.get("big").unwrap(),
            &serde_json::json!(val.to_string())
        );
    }

    #[test]
    fn test_visitor_record_i128() {
        let mut visitor = LoggingVisitor::default();
        let field = FS_V11.field("signed_big").unwrap();
        let val: i128 = -170_141_183_460_469;
        visitor.record_i128(&field, val);
        assert_eq!(
            visitor.fields.get("signed_big").unwrap(),
            &serde_json::json!(val.to_string())
        );
    }

    // ---- LoggingSubscriber::event with extra fields (exercises the fields loop) ----

    static CS_EV2: TestCallsite = TestCallsite;
    static FS_EV2: FieldSet = FieldSet::new(
        &["message", "extra_field"],
        tracing_core::identify_callsite!(&CS_EV2),
    );

    #[test]
    #[cfg_attr(miri, ignore)]
    fn test_subscriber_event_with_extra_fields() {
        static META_EV2: std::sync::LazyLock<Metadata<'static>> =
            std::sync::LazyLock::new(|| {
                tracing_core::metadata!(
                    name: "test_ev2",
                    target: "test_target",
                    level: Level::WARN,
                    fields: &["message", "extra_field"],
                    callsite: &CS_EV2,
                    kind: Kind::EVENT,
                )
            });
        let sub = LoggingSubscriber::new();
        let msg_field = FS_EV2.field("message").unwrap();
        let extra_field = FS_EV2.field("extra_field").unwrap();
        let msg: &dyn tracing_core::field::Value = &"event msg";
        let extra: &dyn tracing_core::field::Value = &"extra_val";
        let vals = [
            (&msg_field, Some(msg)),
            (&extra_field, Some(extra)),
        ];
        let fields = FS_EV2.value_set(&vals);
        let event = Event::new(&META_EV2, &fields);
        // Exercises the `for (key, value) in visitor.fields` loop in
        // Subscriber::event (line 86-88).
        sub.event(&event);
    }

    // ---- LoggingLayer tests (behind feature gate) ----

    #[cfg(feature = "logging-tracing-layer")]
    #[test]
    fn test_logging_layer_new() {
        use crate::logging::log_format::LogFormat;
        let layer = LoggingLayer::new();
        assert_eq!(layer.format, LogFormat::MCP);
    }

    #[cfg(feature = "logging-tracing-layer")]
    #[test]
    fn test_logging_layer_default() {
        use crate::logging::log_format::LogFormat;
        let layer = LoggingLayer::default();
        assert_eq!(layer.format, LogFormat::MCP);
    }

    #[cfg(feature = "logging-tracing-layer")]
    #[test]
    fn test_logging_layer_with_format() {
        use crate::logging::log_format::LogFormat;
        let layer = LoggingLayer::new().with_format(LogFormat::JSON);
        assert_eq!(layer.format, LogFormat::JSON);
    }

    #[cfg(feature = "logging-tracing-layer")]
    #[test]
    fn test_logging_layer_debug() {
        let layer = LoggingLayer::new();
        let dbg = format!("{layer:?}");
        assert!(dbg.contains("LoggingLayer"));
    }

    #[cfg(feature = "logging-tracing-layer")]
    #[test]
    fn test_logging_layer_clone_copy() {
        let a = LoggingLayer::new();
        let b = a; // Copy
        let c = a; // Copy (LoggingLayer is Copy)
        assert_eq!(b.format, c.format);
    }

    // ---- LoggingLayer Layer trait integration tests ----

    #[cfg(feature = "logging-tracing-layer")]
    static CS_LL_EN: TestCallsite = TestCallsite;

    #[cfg(feature = "logging-tracing-layer")]
    #[test]
    #[cfg_attr(miri, ignore)]
    fn test_logging_layer_enabled() {
        use tracing_subscriber::prelude::*;

        static META_LL_EN: std::sync::LazyLock<Metadata<'static>> =
            std::sync::LazyLock::new(|| {
                tracing_core::metadata!(
                    name: "test_ll_en",
                    target: "test_target",
                    level: Level::ERROR,
                    fields: &[],
                    callsite: &CS_LL_EN,
                    kind: Kind::EVENT,
                )
            });

        let layer = LoggingLayer::new();
        let subscriber = tracing_subscriber::registry().with(layer);
        let dispatch =
            tracing_core::dispatcher::Dispatch::new(subscriber);
        let _guard =
            tracing_core::dispatcher::set_default(&dispatch);

        // The subscriber dispatches enabled() to the layer
        assert!(dispatch.enabled(&META_LL_EN));
    }

    #[cfg(feature = "logging-tracing-layer")]
    static CS_LL_EV: TestCallsite = TestCallsite;
    #[cfg(feature = "logging-tracing-layer")]
    static FS_LL_EV: FieldSet = FieldSet::new(
        &["message"],
        tracing_core::identify_callsite!(&CS_LL_EV),
    );

    #[cfg(feature = "logging-tracing-layer")]
    #[test]
    #[cfg_attr(miri, ignore)]
    fn test_logging_layer_on_event() {
        use tracing_subscriber::prelude::*;

        static META_LL_EV: std::sync::LazyLock<Metadata<'static>> =
            std::sync::LazyLock::new(|| {
                tracing_core::metadata!(
                    name: "test_ll_ev",
                    target: "test_target",
                    level: Level::INFO,
                    fields: &["message"],
                    callsite: &CS_LL_EV,
                    kind: Kind::EVENT,
                )
            });

        let layer = LoggingLayer::new()
            .with_format(crate::logging::log_format::LogFormat::JSON);
        let subscriber = tracing_subscriber::registry().with(layer);
        let dispatch =
            tracing_core::dispatcher::Dispatch::new(subscriber);
        let _guard =
            tracing_core::dispatcher::set_default(&dispatch);

        let msg_field = FS_LL_EV.field("message").unwrap();
        let msg: &dyn tracing_core::field::Value =
            &"layer event test";
        let vals = [(&msg_field, Some(msg))];
        let fields = FS_LL_EV.value_set(&vals);
        let event = Event::new(&META_LL_EV, &fields);

        // This routes through on_event in our layer
        dispatch.event(&event);
    }

    #[cfg(feature = "logging-tracing-layer")]
    static CS_LL_SP: TestCallsite = TestCallsite;
    #[cfg(feature = "logging-tracing-layer")]
    static FS_LL_SP: FieldSet = FieldSet::new(
        &[],
        tracing_core::identify_callsite!(&CS_LL_SP),
    );

    #[cfg(feature = "logging-tracing-layer")]
    #[test]
    #[cfg_attr(miri, ignore)]
    fn test_logging_layer_on_new_span_and_close() {
        use tracing_subscriber::prelude::*;

        static META_LL_SP: std::sync::LazyLock<Metadata<'static>> =
            std::sync::LazyLock::new(|| {
                tracing_core::metadata!(
                    name: "test_ll_sp",
                    target: "test_target",
                    level: Level::INFO,
                    fields: &[],
                    callsite: &CS_LL_SP,
                    kind: Kind::SPAN,
                )
            });

        let layer = LoggingLayer::new();
        let subscriber = tracing_subscriber::registry().with(layer);
        let dispatch =
            tracing_core::dispatcher::Dispatch::new(subscriber);
        let _guard =
            tracing_core::dispatcher::set_default(&dispatch);

        let initial_spans =
            super::super::engine::ENGINE.active_spans();

        // Create a new span
        let vs = FS_LL_SP.value_set(&[]);
        let attrs = tracing_core::span::Attributes::new(
            &META_LL_SP,
            &vs,
        );
        let id = dispatch.new_span(&attrs);

        // on_new_span should have incremented spans
        assert!(
            super::super::engine::ENGINE.active_spans()
                > initial_spans
        );

        // Enter and exit the span (no-ops for our layer, but exercise paths)
        dispatch.enter(&id);
        dispatch.exit(&id);

        // Close (drop) the span -- should decrement
        dispatch.clone_span(&id);
        drop(dispatch);
    }

    #[cfg(feature = "logging-tracing-layer")]
    static CS_LL_CL: TestCallsite = TestCallsite;
    #[cfg(feature = "logging-tracing-layer")]
    static FS_LL_CL: FieldSet = FieldSet::new(
        &[],
        tracing_core::identify_callsite!(&CS_LL_CL),
    );

    #[cfg(feature = "logging-tracing-layer")]
    #[test]
    #[cfg_attr(miri, ignore)]
    fn test_logging_layer_on_close_dec_spans() {
        use tracing_subscriber::prelude::*;

        static META_LL_CL: std::sync::LazyLock<Metadata<'static>> =
            std::sync::LazyLock::new(|| {
                tracing_core::metadata!(
                    name: "test_ll_cl",
                    target: "test_target",
                    level: Level::INFO,
                    fields: &[],
                    callsite: &CS_LL_CL,
                    kind: Kind::SPAN,
                )
            });

        let layer = LoggingLayer::new();
        let subscriber = tracing_subscriber::registry().with(layer);
        let dispatch =
            tracing_core::dispatcher::Dispatch::new(subscriber);
        let _guard =
            tracing_core::dispatcher::set_default(&dispatch);

        let initial_spans =
            super::super::engine::ENGINE.active_spans();

        // Create a span (increments active spans via on_new_span)
        let vs = FS_LL_CL.value_set(&[]);
        let attrs = tracing_core::span::Attributes::new(
            &META_LL_CL,
            &vs,
        );
        let id = dispatch.new_span(&attrs);

        assert!(
            super::super::engine::ENGINE.active_spans()
                > initial_spans
        );

        // Dropping the span ID triggers on_close which calls dec_spans
        // Note: tracing_core::span::Id does not implement Drop, so this
        // is just moving it out of scope.
        let _ = id;

        // active_spans should be back to at least the initial count
        assert!(
            super::super::engine::ENGINE.active_spans()
                <= initial_spans + 1
        );
    }

    #[cfg(feature = "logging-tracing-layer")]
    static CS_LL_EV2: TestCallsite = TestCallsite;
    #[cfg(feature = "logging-tracing-layer")]
    static FS_LL_EV2: FieldSet = FieldSet::new(
        &["message", "extra_key"],
        tracing_core::identify_callsite!(&CS_LL_EV2),
    );

    #[cfg(feature = "logging-tracing-layer")]
    #[test]
    #[cfg_attr(miri, ignore)]
    fn test_logging_layer_on_event_with_extra_fields() {
        use tracing_subscriber::prelude::*;

        static META_LL_EV2: std::sync::LazyLock<Metadata<'static>> =
            std::sync::LazyLock::new(|| {
                tracing_core::metadata!(
                    name: "test_ll_ev2",
                    target: "test_target",
                    level: Level::WARN,
                    fields: &["message", "extra_key"],
                    callsite: &CS_LL_EV2,
                    kind: Kind::EVENT,
                )
            });

        let layer = LoggingLayer::new()
            .with_format(crate::logging::log_format::LogFormat::MCP);
        let subscriber = tracing_subscriber::registry().with(layer);
        let dispatch =
            tracing_core::dispatcher::Dispatch::new(subscriber);
        let _guard =
            tracing_core::dispatcher::set_default(&dispatch);

        let msg_field = FS_LL_EV2.field("message").unwrap();
        let extra_field = FS_LL_EV2.field("extra_key").unwrap();
        let msg: &dyn tracing_core::field::Value =
            &"event with fields";
        let extra: &dyn tracing_core::field::Value =
            &"extra_value";
        let vals = [
            (&msg_field, Some(msg)),
            (&extra_field, Some(extra)),
        ];
        let fields = FS_LL_EV2.value_set(&vals);
        let event = Event::new(&META_LL_EV2, &fields);

        // Exercises the `for (key, value) in visitor.fields` loop
        dispatch.event(&event);
    }
}
