<!-- SPDX-FileCopyrightText: 2026 Euxis Commons -->
<!-- SPDX-License-Identifier: MIT OR Apache-2.0 -->

# Testing strategy

What each layer catches, where it runs, and how to extend it.

## The layers

```
Doctests            — every public item with an example
Unit tests          — inline #[cfg(test)], per module
Integration tests   — tests/*.rs, public API only
Feature matrix      — each module built in isolation
Coverage            — cargo llvm-cov, reported to Codecov
Cross-target checks — cfg-gated code compiled for other targets
```

Unit tests carry most of the weight: the crate is a collection of
small utilities, and each module tests its own edge cases next to the
code. Integration tests prove the *public* surface behaves as
documented and catch anything that only breaks when the crate is
consumed from outside.

## Running them

```sh
cargo test --workspace --all-features
cargo llvm-cov --all-features --workspace --summary-only
cargo bench --all-features --bench benchmarks
```

## The feature matrix matters

`--all-features` is the one configuration where feature-gating bugs
are invisible. Two real examples from this crate:

- a module compiled even when its feature was off, because a `cfg`
  attribute had drifted onto the wrong item;
- a feature that derived `thiserror::Error` without enabling the
  feature providing it.

Neither was reachable from any job that built everything. The
`features` CI job builds each module alone, plus a few pairings.

```sh
cargo check -p euxis-commons --no-default-features
cargo check -p euxis-commons --no-default-features --features validation
```

## Coverage

The crate sits at roughly **97% region / 98% line** coverage. The
remainder is not untested logic — it is dominated by:

- derive-generated regions (`Debug`, `Clone`, `serde`),
- match arms that exist for exhaustiveness,
- `cfg`-gated code that cannot execute on the host measuring it.

That last category deserves emphasis: the `Mutex` branch of
`counter.rs` is unreachable on any 64-bit machine, so it can never be
covered by a normal run. Chasing a literal 100% would mean deleting
real portability code.

Region and line coverage differ sharply here, and the smaller number
is easy to misread. `log_entry.rs` reports around 100 missed *regions*
but only one missed *line*.

## Testing `cfg`-gated code

A lint or type error inside `#[cfg(target_os = "linux")]` is invisible
when you build on macOS. This has bitten this crate: a clippy failure
in the Linux branch of `fs::is_wsl` survived a local run that reported
clean.

Check the other branches explicitly:

```sh
rustup target add x86_64-unknown-linux-gnu
cargo clippy --target x86_64-unknown-linux-gnu --all-features -- -D warnings

rustup target add powerpc-unknown-linux-gnu   # no 64-bit atomics
cargo check --target powerpc-unknown-linux-gnu --all-features
```

The second is the only way to exercise `counter.rs`'s fallback from a
normal workstation.

## Matching CI's toolchain

CI installs `dtolnay/rust-toolchain@stable`. If your shell pins
`RUSTUP_TOOLCHAIN` to something else, local runs can disagree with CI
and a clean local clippy means nothing. Select the toolchain
explicitly, and use a fresh target directory so a cached result is not
mistaken for a passing run:

```sh
RUSTUP_TOOLCHAIN= CARGO_TARGET_DIR=/tmp/check \
  cargo +stable clippy --workspace --all-targets --all-features -- -D warnings
```

## Adding a test

- One function's edge cases → inline `#[cfg(test)]` in that module.
- Behaviour a consumer depends on → `tests/`, importing only through
  `commons::`.
- Fixing a bug → add the failing case first and watch it fail. A test
  that has never been red proves nothing.
