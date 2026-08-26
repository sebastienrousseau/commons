<!-- SPDX-License-Identifier: Apache-2.0 OR MIT -->

# Contributing

## Before you start

Small fixes and documentation corrections need no discussion — open a
pull request. For anything that changes the public API, open an issue
first; the API is small on purpose and each addition is a maintenance
commitment.

## The gate

Everything CI runs, you can run:

```sh
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-features
```

Three things trip people up, all documented in
[`doc/TESTING.md`](doc/TESTING.md):

- **Match CI's toolchain.** CI installs `stable`. If your shell pins
  `RUSTUP_TOOLCHAIN`, a clean local clippy means nothing — select the
  toolchain explicitly and use a fresh `CARGO_TARGET_DIR`, because a
  cached result looks identical to a passing one.
- **`--all-features` hides feature bugs.** A module compiled when its
  feature is off, or a feature missing a dependency it derives from,
  are both invisible there. CI has a job that builds each module alone.
- **`cfg`-gated code never compiles on the host that excludes it.**
  Check the other branches with `--target`.

## Adding a module

1. Put it behind its own feature in `Cargo.toml`, and make that feature
   enable everything it uses — including crates needed only by derive
   macros.
2. Add it to the `full` feature and to the table in the README.
3. Document it in [`doc/USER-GUIDE.md`](doc/USER-GUIDE.md), and add the
   example to `tests/guide_examples.rs` so it is compiled rather than
   merely written.

## Tests

Testing one function's edge cases belongs inline. Testing behaviour a
consumer depends on belongs in `tests/`, importing only through
`commons::`.

When fixing a bug, add the failing case first and watch it fail. A test
that has never been red proves nothing.

## Commits

Conventional commits. The body should explain *why* — what was
observed, what was ruled out — rather than restate the diff. If a
change is a consequence of an earlier mistake, say so; that is the part
worth reading later.

## Licence

Contributions are dual-licensed under Apache-2.0 and MIT, matching the
crate.
