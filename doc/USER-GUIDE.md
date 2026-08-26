<!-- SPDX-FileCopyrightText: 2026 Euxis Commons -->
<!-- SPDX-License-Identifier: MIT OR Apache-2.0 -->

# `euxis-commons` user guide

Task-oriented tour of the public API. For how the pieces fit together
internally see [`ARCHITECTURE.md`](ARCHITECTURE.md); for exact
signatures see [docs.rs](https://docs.rs/euxis-commons).

## Contents

1. [Choosing features](#1-choosing-features)
2. [Errors](#2-errors)
3. [Configuration](#3-configuration)
4. [Validation](#4-validation)
5. [Retries](#5-retries)
6. [Identifiers](#6-identifiers)
7. [Environment](#7-environment)
8. [Filesystem](#8-filesystem)
9. [Time](#9-time)
10. [Collections](#10-collections)
11. [Logging](#11-logging)
12. [Portable counters](#12-portable-counters)

## 1. Choosing features

Every module is feature gated. `default = ["full"]` enables all of
them, which is convenient and rarely what a library wants:

```toml
# Everything.
euxis-commons = "0.0.4"

# Just what you use.
euxis-commons = { version = "0.0.4", default-features = false, features = ["validation", "retry"] }
```

The crate is imported as `commons`, not `euxis_commons`:

```rust
use commons::validation::is_valid_email;
```

A feature enables everything it needs, so `config` pulls in `error`
because it derives `thiserror::Error`. You never have to work that out
yourself — but if you add a module, remember its derives are part of
its dependency set. That mistake was live in this crate and invisible
to CI until a per-feature job was added.

## 2. Errors

`CommonError` is a single error type with constructors that read as
categories rather than codes:

```rust
use commons::error::CommonError;

let e = CommonError::invalid_input("port must be 1-65535");
assert!(e.is_input_error());

// `is_recoverable` distinguishes "retry might help" from "give up".
if e.is_recoverable() {
    // back off and try again
}
```

Constructors take `impl Into<String>`, so `&str` and `String` both work
without ceremony. The other categories are `config`, `parse`,
`not_found` and `custom`.

## 3. Configuration

`Config` wraps TOML content, from a string or a file:

```rust
use commons::config::Config;

let cfg = Config::new(r#"
    name = "service"
    port = 8080
"#);

assert!(cfg.has_key("port"));

// `get` is implemented for String, i64, f64, bool and toml::Value.
// Narrow integers go through i64 and are converted by the caller.
let port: Option<i64> = cfg.get("port");

// Or deserialise the whole document into your own type.
#[derive(serde::Deserialize)]
struct Settings { name: String, port: u16 }
let settings: Settings = cfg.parse()?;
# Ok::<(), commons::config::ConfigError>(())
```

`Config::from_file` reads from disk and returns `ConfigError` rather
than panicking on a missing or malformed file.

## 4. Validation

Two families, deliberately different in shape:

- `validate_*` returns `ValidationResult<T>` and **gives the input
  back** on success, so calls chain.
- `is_valid_*` answers a yes/no question and returns `bool`.

```rust
use commons::validation::{validate_length, validate_not_empty, is_valid_email};

let name = validate_not_empty("ada")?;
let name = validate_length(name, 2, 32)?;

if is_valid_email("user@example.com") {
    // ...
}
# Ok::<(), commons::validation::ValidationError>(())
```

Also available: `validate_range`, `is_valid_url`, `is_valid_ip`,
`is_valid_ipv4`.

## 5. Retries

`RetryConfig` describes *when* to retry. It does not run the work —
that stays in your control, so the policy is testable on its own.

```rust
use commons::retry::{BackoffStrategy, RetryConfig};
use std::time::Duration;

let policy = RetryConfig::new()
    .max_attempts(5)
    .backoff(BackoffStrategy::Exponential {
        initial: Duration::from_millis(100),
        max: Duration::from_secs(5),
        multiplier: 2.0,
    })
    .jitter(true);

for attempt in 0..policy.max_attempts {
    let delay = policy.backoff.delay_for_attempt(attempt);
    // sleep(delay); try again
}
```

Strategies are `None`, `Constant(Duration)`, `Linear { initial,
increment, max }` and `Exponential { initial, max, multiplier }`.
`RetryConfig::no_retry()` states the intent more clearly than
`max_attempts(1)`.

Enable `jitter` when many clients retry against the same service;
without it they synchronise and arrive together.

## 6. Identifiers

```rust
use commons::id::{IdFormat, generate_id, generate_prefixed_id, generate_timestamp_id};

let order = generate_prefixed_id("order");   // order_xUBnzTmDl5pl
let ts    = generate_timestamp_id();          // sortable, monotonic
let any   = generate_id(IdFormat::Short);
```

Formats are `Timestamp`, `RandomHex`, `Short` and `Prefixed`.

Timestamp identifiers embed a monotonic counter, so two taken in the
same millisecond still differ — worth knowing if you use them as keys.

## 7. Environment

Typed access with three escalating strictness levels:

```rust
use commons::env::{get_env, get_env_or, require_env, get_bool, get_list};

let port: Option<u16> = get_env("PORT");        // None if unset or unparseable
let port: u16 = get_env_or("PORT", 8080);       // default
let secret: String = require_env("API_KEY");    // panics if absent

let debug = get_bool("DEBUG");
let hosts = get_list("HOSTS", ",");
```

`try_get_env` returns `Result<T, EnvError>` when you want to
distinguish "unset" from "set but unparseable".

## 8. Filesystem

```rust
use commons::fs::{ensure_dir, is_wsl, resolve_path, to_wsl_path};

ensure_dir("out/reports")?;              // mkdir -p, idempotent
let abs = resolve_path("~/notes");

if is_wsl() {
    let p = to_wsl_path("C:\\Users\\ada");   // /mnt/c/Users/ada
}
# Ok::<(), std::io::Error>(())
```

`is_wsl` reads `/proc/version` on Linux and is a `const fn` returning
`false` everywhere else, so it costs nothing off-Linux.

## 9. Time

```rust
use commons::time::{format_duration, parse_duration, unix_timestamp};

let now = unix_timestamp();                 // seconds
let d = parse_duration("2h")?;              // one unit per call
let s = format_duration(d);                 // human-readable
# Ok::<(), String>(())
```

`parse_duration` takes a single unit — `"100ms"`, `"30s"`, `"2h"` —
not a compound string like `"1h30m"`. Sum two calls if you need one.

## 10. Collections

`LruCache` is a bounded cache with the usual eviction behaviour:

```rust
use commons::collections::LruCache;

let mut cache = LruCache::new(2);
cache.insert("a", 1);
cache.insert("b", 2);
let _ = cache.get(&"a");     // marks "a" as recently used
cache.insert("c", 3);        // evicts "b", not "a"
```

`get` counts as a use; `peek` does not, which matters when you are
inspecting the cache rather than reading through it.

## 11. Logging

The largest module. Records go onto a bounded lock-free queue and are
formatted and written by a background thread, so the calling thread
pays only for building the event.

```rust
use commons::logging::{Log, LogFormat};

Log::info("service started")
    .component("api")
    .with("port", 8080)
    .fire();

// Or render one without emitting it.
let line = format!("{}", Log::warn("slow request")
    .with("ms", 1500)
    .format(LogFormat::Logfmt));
```

Twelve formats are available — CLF, JSON, CEF, ELF, GELF,
ApacheAccessLog, Logstash, NDJSON, MCP, OTLP, Logfmt and ECS — and
sinks cover stdout, rotating files, `journald` and `os_log`.

Two things worth knowing before you rely on it in anger:

- **The queue is bounded.** Under sustained overload, pushes fail
  rather than growing memory without limit. That is a deliberate
  trade: dropping records beats exhausting the process.
- **Ordering is per-producer.** Records from one thread keep their
  order; records from different threads interleave by arrival.

## 12. Portable counters

`counter::Counter` is a monotonic `u64` that works on targets without
64-bit atomics, where `AtomicU64` does not exist at all:

```rust
use commons::counter::Counter;
use std::sync::atomic::Ordering;

static REQUESTS: Counter = Counter::new(0);
let n = REQUESTS.fetch_add(1, Ordering::Relaxed);
```

It is lock-free where the target supports it and mutex-backed where it
does not, chosen at compile time. Use it instead of `AtomicU64` if you
publish a crate and care about targets such as
`powerpc-unknown-linux-gnu` — see
[ADR-0002](adr/0002-portable-counters.md) for why this exists.
