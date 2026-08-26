<!-- SPDX-License-Identifier: Apache-2.0 OR MIT -->

<p align="center">
  <img src="https://kura.pro/euxis/images/logos/euxis.svg" alt="EUXIS Commons logo" width="128" />
</p>

<h1 align="center">euxis-commons</h1>

<p align="center">
  Shared Rust utilities for the EUXIS ecosystem &mdash; configuration,
  errors, logging, validation, retries, identifiers and filesystem
  helpers, each behind its own feature flag.
</p>

<p align="center">
  <a href="https://github.com/sebastienrousseau/commons/actions"><img src="https://img.shields.io/github/actions/workflow/status/sebastienrousseau/commons/ci.yml?style=for-the-badge&logo=github" alt="Build" /></a>
  <a href="https://crates.io/crates/euxis-commons"><img src="https://img.shields.io/crates/v/euxis-commons.svg?style=for-the-badge&color=fc8d62&logo=rust" alt="Crates.io" /></a>
  <a href="https://docs.rs/euxis-commons"><img src="https://img.shields.io/badge/docs.rs-euxis--commons-66c2a5?style=for-the-badge&labelColor=555555&logo=docs.rs" alt="Docs.rs" /></a>
  <a href="https://codecov.io/gh/sebastienrousseau/commons"><img src="https://img.shields.io/codecov/c/github/sebastienrousseau/commons?style=for-the-badge&logo=codecov" alt="Coverage" /></a>
</p>

---

## Contents

**Getting started**

- [Install](#install) — Cargo, feature selection
- [Quick start](#quick-start) — the three most common uses

**Reference**

- [Modules](#modules) — what each one does, and its feature flag
- [Features](#features) — the dependency graph, and what each costs
- [Logging](#logging) — the engine, its sinks and twelve wire formats
- [Benchmarks](#benchmarks) — measured numbers, and what is not measured

**Project**

- [Documentation](#documentation) — architecture, testing, decisions
- [Testing](#testing) — how to run it, including cfg-gated branches
- [Platform support](#platform-support)
- [MSRV](#minimum-supported-rust-version)
- [Licence](#licence)

## Install

```toml
[dependencies]
euxis-commons = "0.0.3"
```

The default feature set (`full`) enables every module. Most consumers
want less:

```toml
# Just validation and retries — no logging stack, no serde.
euxis-commons = { version = "0.0.3", default-features = false, features = ["validation", "retry"] }
```

The library is imported as `commons`:

```rust
use commons::validation::is_valid_email;
```

## Quick start

```rust
use commons::id::generate_prefixed_id;
use commons::retry::{BackoffStrategy, RetryConfig};
use commons::validation::is_valid_email;
use std::time::Duration;

// Identifiers — monotonic, collision-free within a millisecond.
let order = generate_prefixed_id("order");

// Validation — `is_valid_*` answers yes/no, `validate_*` returns the
// input so calls chain.
assert!(is_valid_email("user@example.com"));

// Retries — a policy describes *when* to retry; it does not run the work.
let policy = RetryConfig::new()
    .max_attempts(5)
    .backoff(BackoffStrategy::Exponential {
        initial: Duration::from_millis(100),
        max: Duration::from_secs(5),
        multiplier: 2.0,
    });
```

Runnable versions of each live in [`crates/commons/examples/`](crates/commons/examples).

```sh
cargo run --example identifiers --all-features
cargo run --example validation  --all-features
cargo run --example retry       --all-features
```

## Modules

| Module | Feature | What it covers |
|---|---|---|
| `config` | `config` | Layered configuration loading |
| `error` | `error` | Shared error type and result alias |
| `logging` | `logging` | Lock-free engine, sinks, twelve wire formats |
| `time` | `time` | Timestamps |
| `collections` | `collections` | Collection helpers |
| `validation` | `validation` | Emails, URLs, IPs, lengths, ranges |
| `retry` | `retry` | Backoff strategies and retry policies |
| `id` | `id` | Timestamp, hex, short and prefixed identifiers |
| `env` | `env` | Typed environment variable access |
| `fs` | `fs` | Filesystem helpers, WSL detection |

## Features

Every module is feature gated, so depending on this crate for one
utility does not drag in the rest.

```
full ─┬─ config ──── serde, toml
      ├─ error ───── thiserror
      ├─ logging ──┬─ time, error, serde, toml
      │            └─ crossbeam-queue, dtt, parking_lot, serde_json,
      │               log, tracing-core, hostname, regex, itoa, ryu
      ├─ time · collections · validation · retry · id · env · fs
      └─ (no dependencies)

logging-tokio         ─ logging + tokio, notify
logging-tui           ─ logging + terminal_size
logging-miette        ─ logging + miette
logging-tracing-layer ─ logging + tracing-subscriber
```

`logging` is the only feature with a substantial dependency set. The
seven utility modules pull nothing; `config` and `error` pull one
well-known crate each.

## Logging

Records go onto a bounded lock-free ring buffer
(`crossbeam_queue::ArrayQueue`) and are drained by a single background
flusher thread. Formatting and I/O happen on the flusher, so an
application thread pays only for building the event and a queue push.

The queue is bounded on purpose: under sustained overload, pushes fail
rather than growing memory without limit.

Twelve wire formats are supported — CLF, JSON, CEF, ELF, GELF,
ApacheAccessLog, Logstash, NDJSON, MCP, OTLP, Logfmt and ECS — and
sinks cover stdout, rotating files, `journald` on Linux and `os_log`
on macOS.

See [`doc/ARCHITECTURE.md`](doc/ARCHITECTURE.md) for the data flow and
the escaping rules that differ between formats.

## Benchmarks

```sh
cargo bench --all-features --bench benchmarks
```

Measured on an Apple silicon laptop at a short sample size, so treat
them as orders of magnitude rather than published figures:

| Benchmark | Time |
|---|---|
| `validation/not_empty` | ~5.9 ns |
| `validation/email` | ~98 ns |
| `id/short` | ~134 ns |
| `id/timestamp` | ~207 ns |
| `log_format/logfmt` | ~1.06 µs |
| `log_format/json` | ~1.48 µs |

The logging *engine* is deliberately not benchmarked: it owns a
background thread, so a microbenchmark would measure queue contention
against the harness rather than anything a caller controls.

Note that `cargo bench` on its own also runs the library's test
harness, which rejects criterion's flags — pass `--bench benchmarks`.

## Documentation

- [`doc/USER-GUIDE.md`](doc/USER-GUIDE.md) — task-oriented tour of every module
- [`doc/ARCHITECTURE.md`](doc/ARCHITECTURE.md) — layout, feature graph, logging internals, platform matrix
- [`doc/POLICIES.md`](doc/POLICIES.md) — MSRV, SemVer, security, supply chain
- [`doc/TESTING.md`](doc/TESTING.md) — test layers, coverage, cfg-gated branches
- [`doc/MSRV-AND-DEPRECATION.md`](doc/MSRV-AND-DEPRECATION.md) — version floor and removal policy
- [`doc/adr/`](doc/adr) — architecture decision records

Project: [CONTRIBUTING](CONTRIBUTING.md) · [SECURITY](SECURITY.md) ·
[SUPPORT](SUPPORT.md) · [CODE_OF_CONDUCT](CODE_OF_CONDUCT.md)

## Testing

```sh
cargo test --workspace --all-features
cargo llvm-cov --all-features --workspace --summary-only
```

Coverage sits at roughly **97% region / 98% line**. The remainder is
dominated by derive-generated regions, exhaustiveness match arms, and
`cfg`-gated code that cannot execute on the host measuring it — the
`Mutex` counter fallback is unreachable on any 64-bit machine. Chasing
a literal 100% would mean deleting real portability code.

Code behind a `cfg` never compiles on the host that excludes it, so
check the other branches explicitly:

```sh
cargo clippy --target x86_64-unknown-linux-gnu --all-features -- -D warnings
cargo check  --target powerpc-unknown-linux-gnu --all-features   # no 64-bit atomics
```

## Platform support

| Surface | Linux | macOS | Windows |
|---|---|---|---|
| stdout / file sinks | yes | yes | yes |
| `journald` sink | yes | — | — |
| `os_log` sink | — | yes | — |
| `fs::is_wsl` | reads `/proc/version` | const `false` | const `false` |

The crate builds on targets without 64-bit atomics, such as
`powerpc-unknown-linux-gnu`; see
[ADR-0002](doc/adr/0002-portable-counters.md).

## Minimum supported Rust version

**1.88.0**, enforced by a dedicated CI job. Raising it happens in a
minor release with a documented reason — see
[`doc/MSRV-AND-DEPRECATION.md`](doc/MSRV-AND-DEPRECATION.md).

## Licence

Licensed under either of

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE))
- MIT licence ([LICENSE-MIT](LICENSE-MIT))

at your option.
