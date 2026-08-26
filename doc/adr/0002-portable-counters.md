<!-- SPDX-FileCopyrightText: 2026 Euxis Commons -->
<!-- SPDX-License-Identifier: MIT OR Apache-2.0 -->

# ADR-0002 — Portable monotonic counters

**Status:** accepted (2026-08-26); made public in 0.0.4

## Context

Three modules need a process-wide monotonic `u64`: `id` for timestamp
and entropy sequencing, `logging::log_entry` for session identifiers,
and `logging::tracing_bridge` for span identifiers. All four statics
used `AtomicU64`.

`AtomicU64` does not exist on targets without 64-bit atomic
instructions. On `powerpc-unknown-linux-gnu` the import fails
outright:

```
error[E0432]: unresolved import `std::sync::atomic::AtomicU64`
```

This was not a theoretical portability concern. It broke consumers:
the crate is published on crates.io, and `xtasks` builds a 22-target
release matrix in which this was the single failing target.

## Decision

Introduce a `counter` module exposing a `Counter` type with `new` and
`fetch_add`, selected at compile time:

- `#[cfg(target_has_atomic = "64")]` — a newtype over `AtomicU64`.
- otherwise — a `Mutex<u64>`.

Every affected static switches to `Counter`.

## Consequences

The fallback is behaviourally identical for this use. All four call
sites are monotonic sequence numbers read through `fetch_add`; none
depends on lock-freedom for correctness, and each is incremented once
per generated identifier rather than in a hot loop.

`Mutex::new` has been `const` since Rust 1.63, well below the 1.88
MSRV, so the statics remain const-constructed.

The fallback branch cannot be covered on a 64-bit host, which is why
the crate's coverage figure will not reach a literal 100%. Compile it
with:

```sh
cargo check --target powerpc-unknown-linux-gnu --all-features
```

Poisoned locks are recovered from rather than propagated. A counter
has no invariant a panicking writer could corrupt.

## Follow-up: made public in 0.0.4

Releasing 0.0.3 did not fix the powerpc build downstream, because
`rlg` carries the same bug in its own source — `AtomicU64` statics in
`tracing.rs` and `log.rs`. Writing a second copy of the pattern there
would be the wrong answer for a crate whose stated purpose is shared
utilities.

`counter` is therefore public from 0.0.4, behind its own feature
(implied by `id` and `logging`, so existing consumers are unaffected).
Downstream crates use `commons::counter::Counter`.

## Alternatives considered

**`portable-atomic`.** A well-maintained crate that provides
`AtomicU64` everywhere. Rejected because it adds a dependency to a
crate whose whole point is to be cheap to depend on, for four counters
that a dozen lines cover.

**`AtomicUsize`.** Free on 64-bit, but wrong on 32-bit targets where
`usize` is 32 bits — session and span identifiers are `u64` and would
silently truncate.

**Drop the targets.** Would have fixed the build by narrowing what the
ecosystem supports, to avoid a portability bug rather than fix it.
