// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Criterion benchmarks for the hot paths.
//!
//! ```sh
//! cargo bench --all-features
//! ```
//!
//! The three groups here cover what callers actually run in a loop:
//! identifier generation, input validation, and rendering a log record
//! to a wire format. The logging *engine* is deliberately not
//! benchmarked -- it owns a background thread, so a microbenchmark
//! would measure queue contention against the harness rather than
//! anything a caller controls.

use commons::id::{generate_short_id, generate_timestamp_id};
use commons::logging::{Log, LogFormat};
use commons::validation::{is_valid_email, is_valid_url, validate_not_empty};
use criterion::{Criterion, criterion_group, criterion_main};
use std::hint::black_box;

fn identifiers(c: &mut Criterion) {
    let mut group = c.benchmark_group("id");
    group.bench_function("timestamp", |b| {
        b.iter(|| black_box(generate_timestamp_id()));
    });
    group.bench_function("short", |b| {
        b.iter(|| black_box(generate_short_id()));
    });
    group.finish();
}

fn validation(c: &mut Criterion) {
    let mut group = c.benchmark_group("validation");
    group.bench_function("not_empty", |b| {
        b.iter(|| black_box(validate_not_empty(black_box("hello")).is_ok()));
    });
    group.bench_function("email", |b| {
        b.iter(|| black_box(is_valid_email(black_box("user@example.com"))));
    });
    group.bench_function("url", |b| {
        b.iter(|| black_box(is_valid_url(black_box("https://example.com/a/b"))));
    });
    group.finish();
}

fn formats(c: &mut Criterion) {
    let mut group = c.benchmark_group("log_format");
    // Rendering cost differs sharply between the terse and the
    // structured formats; both are measured so a regression in either
    // is visible.
    for (name, format) in [
        ("logfmt", LogFormat::Logfmt),
        ("json", LogFormat::JSON),
        ("clf", LogFormat::CLF),
        ("ecs", LogFormat::ECS),
    ] {
        group.bench_function(name, |b| {
            b.iter(|| {
                let entry = Log::info("benchmark record")
                    .component("bench")
                    .with("key", "value")
                    .format(format);
                black_box(format!("{entry}"))
            });
        });
    }
    group.finish();
}

criterion_group!(benches, identifiers, validation, formats);
criterion_main!(benches);
