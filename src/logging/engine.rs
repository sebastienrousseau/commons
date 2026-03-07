// Copyright © 2024-2026 RustLogs (RLG). All rights reserved.
// SPDX-License-Identifier: Apache-2.0
// SPDX-License-Identifier: MIT

//! Near-lock-free ingestion engine backed by a bounded ring buffer.
//!
//! The global [`ENGINE`] accepts [`LogEvent`]s via [`LockFreeEngine::ingest()`]
//! using only atomic operations. A dedicated background thread drains events
//! in batches of 64 and writes them through [`PlatformSink`](super::sink::PlatformSink).
//!
//! **The Mutex is never locked on the hot path.** It exists solely for
//! `shutdown()` to join the flusher thread.

use super::log_level::LogLevel;
#[cfg(not(miri))]
use super::sink::PlatformSink;
use super::tui::TuiMetrics;
#[cfg(not(miri))]
use super::tui::spawn_tui_thread;
use crossbeam_queue::ArrayQueue;
use std::fmt;
use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};
use std::sync::{Arc, LazyLock, Mutex};
use std::thread;
#[cfg(not(miri))]
use std::time::Duration;

/// Capacity of the lock-free ring buffer (number of log events).
const RING_BUFFER_CAPACITY: usize = 65_536;

/// Maximum number of events drained per flusher wake-up cycle.
#[cfg(not(miri))]
const MAX_DRAIN_BATCH_SIZE: usize = 64;

/// A structured log event passed through the ring buffer.
///
/// The caller pays only for a `Log` move (~128-byte memcpy).
/// Serialization happens on the flusher thread.
#[derive(Debug, Clone)]
pub struct LogEvent {
    /// Severity level of this event.
    pub level: LogLevel,
    /// Numeric severity for fast level-gating comparisons.
    pub level_num: u8,
    /// Structured log data. Formatted on the flusher thread, not here.
    pub log: super::log_entry::Log,
}

/// The near-lock-free ingestion engine.
///
/// Owns the ring buffer, flusher thread, and TUI metrics counters.
/// Access the global instance via [`ENGINE`].
pub struct LockFreeEngine {
    /// Bounded ring buffer (lock-free push/pop via `crossbeam`).
    queue: Arc<ArrayQueue<LogEvent>>,
    /// Signals the flusher thread to drain and exit.
    shutdown_flag: Arc<AtomicBool>,
    /// Atomic counters consumed by the opt-in TUI dashboard.
    metrics: Arc<TuiMetrics>,
    /// Minimum severity level. Events below this are dropped at `ingest()`.
    filter_level: AtomicU8,
    /// Flusher thread handle for lock-free `unpark()`. No Mutex involved.
    flusher_thread_handle: Option<thread::Thread>,
    /// `JoinHandle` for `shutdown()` only. **Never locked on the hot path.**
    flusher_join: Mutex<Option<thread::JoinHandle<()>>>,
}

impl fmt::Debug for LockFreeEngine {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("LockFreeEngine")
            .field("queue", &self.queue)
            .field("shutdown_flag", &self.shutdown_flag)
            .field("metrics", &self.metrics)
            .field("filter_level", &self.filter_level)
            .field(
                "flusher_thread_handle",
                &self.flusher_thread_handle.as_ref().map(thread::Thread::id),
            )
            .finish_non_exhaustive()
    }
}

/// Global engine instance, lazily initialized on first access.
pub static ENGINE: LazyLock<LockFreeEngine> =
    LazyLock::new(|| LockFreeEngine::new(RING_BUFFER_CAPACITY));

impl LockFreeEngine {
    /// Create a new engine with the given buffer capacity and spawn the flusher.
    ///
    /// # Panics
    ///
    /// Panics if the OS cannot spawn the background flusher thread.
    #[must_use]
    pub fn new(capacity: usize) -> Self {
        let queue = Arc::new(ArrayQueue::new(capacity));
        let shutdown_flag = Arc::new(AtomicBool::new(false));
        let metrics = Arc::new(TuiMetrics::default());
        let filter_level = AtomicU8::new(0); // Default to ALL

        // Under MIRI, skip spawning background threads to avoid
        // "main thread terminated without waiting" errors.
        #[cfg(not(miri))]
        let flusher_handle = {
            let flusher_queue = queue.clone();
            let flusher_shutdown = shutdown_flag.clone();

            // Spawn lightweight OS thread (Runtime Agnostic)
            let handle = thread::Builder::new()
                .name("logging-flusher".into())
                .spawn(move || {
                    use std::io::Write;
                    let mut sink = PlatformSink::native();
                    let mut fmt_buf = Vec::with_capacity(512);

                    loop {
                        let mut batch: [Option<LogEvent>; MAX_DRAIN_BATCH_SIZE] =
                            std::array::from_fn(|_| None);
                        let mut count = 0;
                        while count < MAX_DRAIN_BATCH_SIZE {
                            match flusher_queue.pop() {
                                Some(event) => {
                                    batch[count] = Some(event);
                                    count += 1;
                                }
                                None => break,
                            }
                        }
                        for event in batch.iter().flatten() {
                            fmt_buf.clear();
                            let _ = writeln!(fmt_buf, "{}", &event.log);
                            sink.emit(event.level.as_str(), &fmt_buf);
                        }

                        if flusher_shutdown.load(Ordering::Relaxed) && flusher_queue.is_empty() {
                            break;
                        }

                        // Park briefly as fallback; real wakeup comes from unpark() in ingest().
                        thread::park_timeout(Duration::from_millis(5));
                    }
                })
                .expect("Failed to spawn logging-flusher background thread");

            // Spawn the TUI dashboard thread if RLG_TUI=1
            if std::env::var("RLG_TUI").map(|v| v == "1").unwrap_or(false) {
                spawn_tui_thread(metrics.clone(), shutdown_flag.clone());
            }

            Some(handle)
        };

        #[cfg(miri)]
        let flusher_handle: Option<thread::JoinHandle<()>> = None;

        let flusher_thread_handle = flusher_handle.as_ref().map(|h| h.thread().clone());

        Self {
            queue,
            shutdown_flag,
            metrics,
            filter_level,
            flusher_thread_handle,
            flusher_join: Mutex::new(flusher_handle),
        }
    }

    /// Appends an event to the ring buffer.
    ///
    /// If the buffer is full, the oldest event is evicted to make room.
    /// Dropped events are tracked via `TuiMetrics::dropped_events`.
    pub fn ingest(&self, event: LogEvent) {
        if event.level_num < self.filter_level.load(Ordering::Acquire) {
            return;
        }

        self.metrics.inc_events();
        self.metrics.inc_level(event.level);

        if event.level_num >= LogLevel::ERROR.to_numeric() {
            self.metrics.inc_errors();
        }

        // If the buffer is full, evict and retry with bounded retries.
        if let Err(rejected) = self.queue.push(event) {
            self.metrics.inc_dropped();
            let mut to_push = rejected;
            for _ in 0..3 {
                let _ = self.queue.pop();
                match self.queue.push(to_push) {
                    Ok(()) => break,
                    Err(e) => to_push = e,
                }
            }
        }

        // Wake the flusher thread -- no Mutex on the hot path.
        if let Some(thread) = &self.flusher_thread_handle {
            thread.unpark();
        }
    }

    /// Sets the global log level filter.
    pub fn set_filter(&self, level: u8) {
        self.filter_level.store(level, Ordering::Release);
    }

    /// Returns the current global log level filter.
    #[must_use]
    pub fn filter_level(&self) -> u8 {
        self.filter_level.load(Ordering::Relaxed)
    }

    /// Increments the format counter in the TUI metrics.
    pub fn inc_format(&self, format: super::log_format::LogFormat) {
        self.metrics.inc_format(format);
    }

    /// Increments the active span count in the TUI metrics.
    pub fn inc_spans(&self) {
        self.metrics.inc_spans();
    }

    /// Decrements the active span count in the TUI metrics.
    pub fn dec_spans(&self) {
        self.metrics.dec_spans();
    }

    /// Returns the current number of active spans.
    #[must_use]
    pub fn active_spans(&self) -> usize {
        self.metrics.active_spans.load(Ordering::Relaxed)
    }

    /// Applies configuration settings to the engine.
    ///
    /// Sets the log level filter from the config. File sink construction
    /// and rotation are handled by the flusher thread at startup via
    /// [`PlatformSink::from_config`](super::sink::PlatformSink::from_config).
    pub fn apply_config(&self, config: &super::log_config::LoggingConfig) {
        self.set_filter(config.log_level.to_numeric());
    }

    /// Safely halts the background thread, flushing pending logs.
    ///
    /// Signals the flusher thread to stop and waits for it to finish
    /// draining any remaining events from the queue.
    pub fn shutdown(&self) {
        self.shutdown_flag.store(true, Ordering::SeqCst);
        // Wake the flusher so it can drain and exit.
        if let Some(thread) = &self.flusher_thread_handle {
            thread.unpark();
        }
        if let Ok(mut guard) = self.flusher_join.lock()
            && let Some(handle) = guard.take()
        {
            let _ = handle.join();
        }
    }
}

/// Zero-Allocation Serializer Helper
#[derive(Debug, Clone, Copy)]
pub struct FastSerializer;

impl FastSerializer {
    /// Appends a u64 integer to a buffer using `itoa` without allocating a String.
    pub fn append_u64(buf: &mut Vec<u8>, val: u64) {
        let mut buffer = itoa::Buffer::new();
        buf.extend_from_slice(buffer.format(val).as_bytes());
    }

    /// Appends an f64 float to a buffer using `ryu` without allocating a String.
    pub fn append_f64(buf: &mut Vec<u8>, val: f64) {
        let mut buffer = ryu::Buffer::new();
        buf.extend_from_slice(buffer.format(val).as_bytes());
    }
}

#[cfg(test)]
mod tests {
    use super::super::log_entry::Log;
    use super::super::log_format::LogFormat;
    use super::*;

    // ── FastSerializer ─────────────────────────────────────────────────
    #[test]
    fn test_fast_serializer_append_u64_zero() {
        let mut buf = Vec::new();
        FastSerializer::append_u64(&mut buf, 0);
        assert_eq!(std::str::from_utf8(&buf).unwrap(), "0");
    }

    #[test]
    fn test_fast_serializer_append_u64_small() {
        let mut buf = Vec::new();
        FastSerializer::append_u64(&mut buf, 42);
        assert_eq!(std::str::from_utf8(&buf).unwrap(), "42");
    }

    #[test]
    fn test_fast_serializer_append_u64_max() {
        let mut buf = Vec::new();
        FastSerializer::append_u64(&mut buf, u64::MAX);
        assert_eq!(std::str::from_utf8(&buf).unwrap(), u64::MAX.to_string());
    }

    #[test]
    fn test_fast_serializer_append_f64_zero() {
        let mut buf = Vec::new();
        FastSerializer::append_f64(&mut buf, 0.0);
        let s = std::str::from_utf8(&buf).unwrap();
        let parsed: f64 = s.parse().unwrap();
        assert!((parsed - 0.0_f64).abs() < f64::EPSILON);
    }

    #[test]
    fn test_fast_serializer_append_f64_pi() {
        let mut buf = Vec::new();
        FastSerializer::append_f64(&mut buf, std::f64::consts::PI);
        let s = std::str::from_utf8(&buf).unwrap();
        let parsed: f64 = s.parse().unwrap();
        assert!((parsed - std::f64::consts::PI).abs() < f64::EPSILON);
    }

    #[test]
    fn test_fast_serializer_append_f64_negative() {
        let mut buf = Vec::new();
        FastSerializer::append_f64(&mut buf, -1.5);
        let s = std::str::from_utf8(&buf).unwrap();
        let parsed: f64 = s.parse().unwrap();
        assert!((parsed - (-1.5_f64)).abs() < f64::EPSILON);
    }

    #[test]
    fn test_fast_serializer_debug_and_clone() {
        let s = FastSerializer;
        let s2 = s;
        let dbg = format!("{s:?}");
        assert_eq!(dbg, "FastSerializer");
        let _ = s2; // confirm Copy
    }

    // ── LogEvent ───────────────────────────────────────────────────────
    #[test]
    #[cfg_attr(miri, ignore)]
    fn test_log_event_debug() {
        let event = LogEvent {
            level: LogLevel::INFO,
            level_num: LogLevel::INFO.to_numeric(),
            log: Log::info("debug-test").session_id(1).time("ts"),
        };
        let dbg = format!("{event:?}");
        assert!(dbg.contains("LogEvent"));
        assert!(dbg.contains("INFO"));
    }

    #[test]
    #[cfg_attr(miri, ignore)]
    fn test_log_event_clone() {
        let event = LogEvent {
            level: LogLevel::WARN,
            level_num: LogLevel::WARN.to_numeric(),
            log: Log::warn("clone-test").session_id(1).time("ts"),
        };
        let cloned = event.clone();
        assert_eq!(cloned.level, LogLevel::WARN);
        assert_eq!(cloned.level_num, event.level_num);
        assert_eq!(cloned.log.description, "clone-test");
    }

    // ── LockFreeEngine: filter level ───────────────────────────────────
    #[test]
    #[cfg_attr(miri, ignore)]
    fn test_engine_set_filter_and_filter_level() {
        let engine = LockFreeEngine::new(16);
        assert_eq!(engine.filter_level(), 0);
        engine.set_filter(5);
        assert_eq!(engine.filter_level(), 5);
        engine.set_filter(0);
        assert_eq!(engine.filter_level(), 0);
        engine.shutdown();
    }

    // ── LockFreeEngine: inc_format ─────────────────────────────────────
    #[test]
    #[cfg_attr(miri, ignore)]
    fn test_engine_inc_format() {
        let engine = LockFreeEngine::new(16);
        engine.inc_format(LogFormat::JSON);
        engine.inc_format(LogFormat::JSON);
        engine.inc_format(LogFormat::MCP);
        assert_eq!(engine.metrics.fmt_json.load(Ordering::Relaxed), 2);
        assert_eq!(engine.metrics.fmt_mcp.load(Ordering::Relaxed), 1);
        engine.shutdown();
    }

    // ── LockFreeEngine: spans ──────────────────────────────────────────
    #[test]
    #[cfg_attr(miri, ignore)]
    fn test_engine_inc_dec_active_spans() {
        let engine = LockFreeEngine::new(16);
        assert_eq!(engine.active_spans(), 0);
        engine.inc_spans();
        engine.inc_spans();
        assert_eq!(engine.active_spans(), 2);
        engine.dec_spans();
        assert_eq!(engine.active_spans(), 1);
        engine.shutdown();
    }

    // ── LockFreeEngine: ingest filters below level ─────────────────────
    #[test]
    #[cfg_attr(miri, ignore)]
    fn test_engine_ingest_drops_below_filter() {
        let engine = LockFreeEngine::new(16);
        // Set filter to ERROR (numeric 8)
        engine.set_filter(LogLevel::ERROR.to_numeric());

        let event = LogEvent {
            level: LogLevel::INFO,
            level_num: LogLevel::INFO.to_numeric(),
            log: Log::info("filtered out").session_id(1).time("ts"),
        };
        let events_before = engine.metrics.total_events.load(Ordering::Relaxed);
        engine.ingest(event);
        let events_after = engine.metrics.total_events.load(Ordering::Relaxed);
        // Event should have been dropped, so total_events unchanged
        assert_eq!(events_before, events_after);
        engine.shutdown();
    }

    // ── LockFreeEngine: ingest ERROR increments error count ────────────
    #[test]
    #[cfg_attr(miri, ignore)]
    fn test_engine_ingest_error_increments_errors() {
        let engine = LockFreeEngine::new(16);
        let errors_before = engine.metrics.error_count.load(Ordering::Relaxed);
        let event = LogEvent {
            level: LogLevel::ERROR,
            level_num: LogLevel::ERROR.to_numeric(),
            log: Log::error("fail").session_id(1).time("ts"),
        };
        engine.ingest(event);
        let errors_after = engine.metrics.error_count.load(Ordering::Relaxed);
        assert_eq!(errors_after, errors_before + 1);
        engine.shutdown();
    }

    // ── LockFreeEngine: ingest FATAL also counts as error ──────────────
    #[test]
    #[cfg_attr(miri, ignore)]
    fn test_engine_ingest_fatal_increments_errors() {
        let engine = LockFreeEngine::new(16);
        let errors_before = engine.metrics.error_count.load(Ordering::Relaxed);
        let event = LogEvent {
            level: LogLevel::FATAL,
            level_num: LogLevel::FATAL.to_numeric(),
            log: Log::fatal("doom").session_id(1).time("ts"),
        };
        engine.ingest(event);
        let errors_after = engine.metrics.error_count.load(Ordering::Relaxed);
        assert_eq!(errors_after, errors_before + 1);
        engine.shutdown();
    }

    // ── LockFreeEngine: ingest increments level counter ────────────────
    #[test]
    #[cfg_attr(miri, ignore)]
    fn test_engine_ingest_increments_level() {
        let engine = LockFreeEngine::new(16);
        let event = LogEvent {
            level: LogLevel::WARN,
            level_num: LogLevel::WARN.to_numeric(),
            log: Log::warn("w").session_id(1).time("ts"),
        };
        engine.ingest(event);
        assert_eq!(engine.metrics.level_warn.load(Ordering::Relaxed), 1);
        engine.shutdown();
    }

    // ── LockFreeEngine: ingest with full buffer evicts ─────────────────
    #[test]
    #[cfg_attr(miri, ignore)]
    fn test_engine_ingest_full_buffer_eviction() {
        // Capacity 2 so we can fill it and trigger eviction.
        // Push many events rapidly to overwhelm the flusher.
        let engine = LockFreeEngine::new(2);

        for i in 0_u64..50 {
            let event = LogEvent {
                level: LogLevel::INFO,
                level_num: LogLevel::INFO.to_numeric(),
                log: Log::info(&format!("msg-{i}")).session_id(i).time("ts"),
            };
            engine.ingest(event);
        }
        // With 50 events into a buffer of size 2, eviction is virtually
        // guaranteed even with a background flusher.
        assert!(engine.metrics.dropped_events.load(Ordering::Relaxed) >= 1);
        engine.shutdown();
    }

    // ── LockFreeEngine: apply_config ───────────────────────────────────
    #[test]
    #[cfg_attr(miri, ignore)]
    fn test_engine_apply_config() {
        let engine = LockFreeEngine::new(16);
        let config = super::super::log_config::LoggingConfig {
            log_level: LogLevel::ERROR,
            ..super::super::log_config::LoggingConfig::default()
        };
        engine.apply_config(&config);
        assert_eq!(engine.filter_level(), LogLevel::ERROR.to_numeric());
        engine.shutdown();
    }

    // ── LockFreeEngine: Debug impl ─────────────────────────────────────
    #[test]
    #[cfg_attr(miri, ignore)]
    fn test_engine_debug_impl() {
        let engine = LockFreeEngine::new(16);
        let dbg = format!("{engine:?}");
        assert!(dbg.contains("LockFreeEngine"));
        assert!(dbg.contains("queue"));
        assert!(dbg.contains("shutdown_flag"));
        assert!(dbg.contains("metrics"));
        assert!(dbg.contains("filter_level"));
        engine.shutdown();
    }

    // ── LockFreeEngine: shutdown is idempotent ─────────────────────────
    #[test]
    #[cfg_attr(miri, ignore)]
    fn test_engine_shutdown_idempotent() {
        let engine = LockFreeEngine::new(16);
        engine.shutdown();
        // Second shutdown should not panic
        engine.shutdown();
    }

    // ── Global ENGINE smoke test ───────────────────────────────────────
    #[test]
    #[cfg_attr(miri, ignore)]
    fn test_global_engine_accessible() {
        // Just access the global ENGINE to verify lazy init works
        let level = ENGINE.filter_level();
        assert!(level <= 10);
    }
}
