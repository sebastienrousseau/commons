<!-- SPDX-FileCopyrightText: 2026 Euxis Commons -->
<!-- SPDX-License-Identifier: MIT OR Apache-2.0 -->

# `euxis-commons` architecture

The map a contributor needs to find their way around the codebase.
Companion to the per-module rustdoc on
[docs.rs](https://docs.rs/euxis-commons).

## Workspace layout

```
crates/
└── commons/              # the library — public API
    ├── src/
    │   ├── lib.rs        # public re-exports + prelude
    │   ├── config.rs     # layered configuration loading
    │   ├── error.rs      # shared error type and result alias
    │   ├── env.rs        # environment inspection
    │   ├── fs.rs         # filesystem helpers, WSL detection
    │   ├── id.rs         # identifier generation
    │   ├── time.rs       # timestamps
    │   ├── collections.rs
    │   ├── validation.rs
    │   ├── retry.rs      # backoff and retry policies
    │   ├── counter.rs    # portable monotonic counters
    │   └── logging/      # the logging engine and its formats
    ├── tests/            # integration tests
    ├── benches/          # criterion harnesses
    └── examples/
```

The root manifest is a virtual workspace. It exists so satellite
crates can be added later without relocating the primary crate a
second time, and it is where the shared `[profile.*]` tables live —
Cargo only honours those at the workspace root and silently ignores
them in a member manifest. See
[ADR-0001](adr/0001-crates-workspace-layout.md).

## Feature graph

Every module is behind a feature, and `default = ["full"]` turns them
all on. Consumers that want one utility do not pay for the rest.

```
full ─┬─ config ──── serde, toml, error
      ├─ error ───── thiserror
      ├─ logging ──┬─ time, error, serde, toml
      │            └─ crossbeam-queue, dtt, parking_lot, serde_json,
      │               log, tracing-core, hostname, regex, itoa, ryu
      ├─ counter ─── (no dependencies; implied by id and logging)
      ├─ time · collections · validation · retry · id · env · fs
      └─ (no dependencies)

logging-tokio         ─ logging + tokio, notify
logging-tui           ─ logging + terminal_size
logging-miette        ─ logging + miette
logging-tracing-layer ─ logging + tracing-subscriber
```

`logging` is the only feature with a substantial dependency set.

A feature must enable everything it uses, including for derive macros.
`config` derives `thiserror::Error`, so it implies `error`; without
that, `--features config` alone failed to compile and no CI job noticed
because they all built `--all-features`. The `features` CI job now
checks each module in isolation.

## The logging engine

`logging` is the largest subsystem and the only one with a runtime.

Records are pushed onto a bounded lock-free ring buffer
(`crossbeam_queue::ArrayQueue`) and drained by a single background
flusher thread. The producer side never blocks on I/O or formatting:
serialisation happens on the flusher, so an application thread pays
only the cost of constructing the event and a queue push.

```
  application threads          flusher thread
  ───────────────────          ──────────────
  Log::info(..)                 pop from queue
    └─ build LogEvent           format into a reusable buffer
       └─ queue.push()  ──────► write to PlatformSink
                                   ├─ stdout
                                   ├─ file (with rotation)
                                   ├─ journald   (Linux)
                                   └─ os_log     (macOS)
```

Two consequences worth knowing:

- The queue is **bounded**. Under sustained overload pushes fail
  rather than growing memory without limit.
- Under Miri the flusher is not spawned, because Miri cannot run the
  background thread the engine relies on.

## Wire formats

`LogFormat` has twelve variants — CLF, JSON, CEF, ELF, GELF,
ApacheAccessLog, Logstash, NDJSON, MCP, OTLP, Logfmt and ECS. Each is
a `Display` implementation over `Log`, writing directly into the
formatter rather than building an intermediate `String`.

Two have escaping rules that are easy to get wrong and are covered
explicitly by `tests/logging_formats.rs`:

- **Logfmt** quotes an attribute value only when it contains a space,
  contains a quote, or is empty. Bare tokens stay unquoted.
- **JSON** escapes `"`, `\\`, newline, carriage return and tab
  individually, and falls back to `\u00XX` for any other control
  character.

## Portable counters

`id`, `logging::log_entry` and `logging::tracing_bridge` each need a
monotonic `u64`. `AtomicU64` does not exist on targets without 64-bit
atomics — `powerpc-unknown-linux-gnu` among them — so `counter.rs`
selects between a lock-free implementation and a `Mutex<u64>` fallback
behind `#[cfg(target_has_atomic = "64")]`.

The module is public so downstream crates can use it instead of
writing the same fallback again. See
[ADR-0002](adr/0002-portable-counters.md).

## Cross-platform

| Surface | Linux | macOS | Windows |
|---|---|---|---|
| stdout / file sinks | yes | yes | yes |
| `journald` sink | yes | — | — |
| `os_log` sink | — | yes | — |
| WSL detection (`fs::is_wsl`) | reads `/proc/version` | const `false` | const `false` |

Platform-specific code is `cfg`-gated rather than runtime-branched,
which means a lint or type error in one branch is invisible when
building on another host. `TESTING.md` explains how to cover this.
