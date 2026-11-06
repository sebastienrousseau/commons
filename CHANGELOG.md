# Changelog

All notable changes to this project are documented here.

Format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/).
This project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

Note that under SemVer, every `0.0.x` release is its own compatibility
range: `0.0.4` is not a drop-in for `0.0.3`. See
[MSRV-AND-DEPRECATION.md](doc/MSRV-AND-DEPRECATION.md).

## [v0.0.8] - 2026-11-06

### Deprecated

- **Most of the crate**, after a survey of the maintainer's 44 Rust
  repositories found one consumer (rlg) using five items. `logging`
  (use rlg), `time`, `collections`, `retry`, `env` and `fs` are
  deprecated whole; so are `Config`, `ConfigBuilder` and
  `FromTomlValue`, every `id` function except `generate_random_hex`,
  and every validator except `Validator` and `validate_not_empty`.
  Each warning names its replacement, and the README lists them. Removal
  comes no earlier than two releases from now.

### Security

- **IDs come from the OS random number generator.** They were seeded
  from fixed-key hashing of the time, pid and thread, so two processes
  with the same pid (containers running as pid 1) produced the same
  IDs in the same millisecond, and anyone knowing the time could
  predict them. rlg uses `generate_random_hex` for trace IDs. Short
  IDs also lose a 25% bias towards their first eight characters.
- **`EnvError` no longer prints the rejected value** in `Display` or
  `Debug`, so a mistyped secret stays out of logs and panic messages.
- **CI and publishing**: every action is pinned to a commit SHA and
  kept current by Dependabot, checkouts drop their credentials,
  `cargo deny` runs in CI, and publishing waits for tests, `cargo
  audit` and `cargo deny` on the tag, with no build cache in the job
  that can publish.

### Fixed

- `time::parse_duration` returns an error instead of panicking on
  negative, NaN, infinite or huge values (`"-1s"`, `"1e30s"`), and
  instead of overflowing on `"999999999999999999d"`.
- `Config::from_file` reports malformed TOML as `ConfigError::Parse`
  instead of loading an empty configuration.
- The docs no longer call `config` "layered", list the right
  dependencies per feature, and state the MSRV and deprecation rules
  in terms of releases this crate actually makes.
- The API docs deploy to GitHub Pages straight from the build, without
  a `gh-pages` branch.

## [v0.0.7] - 2026-11-06

### Changed

- **`toml` is now 1.0** (was 0.8). `toml::Value` is part of the
  `config` API, so code that names it must use `toml` 1.x too. Builds
  with `config` or `logging` no longer compile two copies of `toml`,
  `serde_spanned`, `toml_datetime` and `winnow`, since `config` 0.15
  was already on `toml` 1.x.

### Security

- **`tokio` 1.53.3 or later is required** (was 1.50.0) under
  `logging-tokio`, so no version affected by RUSTSEC-2026-0386 can
  resolve. commons never constructs an `AsyncFd`.

### Fixed

- Four broken intra-doc links in the `logging` module docs now
  resolve on stable and nightly rustdoc.
- Every workflow job runs with a `contents: read` token, clearing ten
  CodeQL `actions/missing-workflow-permissions` alerts, and a test no
  longer formats session ids into its assert message.

## [v0.0.6] - 2026-11-01

### Changed

- **The MSRV is now Rust 1.89.0** (was 1.88.0). No language feature
  needs it: 1.89.0 is the floor shared by the maintainer's Rust crates,
  released more than six months before this version as the policy
  requires. `rust-version`, the `msrv` CI job, `make msrv`, the READMEs
  and the policy documents move together.

### Fixed

- Lint on current stable is green again: ten `assert!(x.is_empty())`
  checks in unit tests became `assert_eq!`/`assert_ne!`, which the
  newer clippy asks for.

## [v0.0.5] - 2026-10-11

### Fixed

- Made `get_terminal_height` and `get_terminal_width` `const fn` when `logging-tui` is disabled, satisfying `clippy::missing_const_for_fn`.

### Added

- Governance and supply-chain files: `GOVERNANCE.md`, `CHANGELOG.md`,
  `REUSE.toml`, `deny.toml` and a `Makefile`. The `deny.toml` allow-list
  is derived from the actual dependency graph under `--all-features`,
  not copied from a sibling project.

### Fixed

- **Dropped the unmaintained `paste` crate** (RUSTSEC-2024-0436). It
  reached the tree through `dtt 0.0.9`, pinned since 2024. `dtt 0.0.10`
  had already migrated to `pastey`, the replacement the advisory itself
  recommends, so bumping to `0.0.11` clears it outright rather than
  needing an exemption. `cargo deny check` now passes all four gates.

## [v0.0.4] - 2026-08-26

### Added

- **`Counter` is now public** (#7). The portable monotonic counter added
  in 0.0.3 was reachable only from inside the crate, so the downstream
  that needed it - `rlg` - could not use the fix it was written for.

## [v0.0.3] - 2026-08-26

### Added

- **`crates/` workspace layout and the first integration tests** (#5).
  Moves the crate to `crates/commons` so the workspace root is virtual.
  Note that `[profile.*]` is only honoured at a workspace root; it is
  silently ignored elsewhere.

### Fixed

- **Builds on targets without 64-bit atomics** (#4). The counter used
  `AtomicU64` unconditionally, which does not exist on targets such as
  `powerpc-unknown-linux-gnu`. It is now gated on
  `target_has_atomic = "64"` with a `Mutex<u64>` fallback. See
  [ADR 0002](doc/adr/0002-portable-counters.md).
- **Release read the version from the wrong manifest** (#6). After the
  workspace root became virtual it no longer carried a `version`, so the
  tag guard compared the tag against a literal. It now uses
  `cargo metadata`.
- **Clippy gate on `main`** (#3).

### Changed

- Publishes to crates.io via Trusted Publishing rather than a stored
  token (#2).

## [v0.0.2] - 2026-03-07

### Added

- The `fs` module, edge-case hardening, and reusable CI workflows.

### Changed

- Renamed the published crate to **`euxis-commons`**. The library is
  still imported as `commons`.
- Edition 2024, with an MSRV of 1.88.0.

## [v0.0.1] - 2026-03-07

Initial release.

[Unreleased]: https://github.com/sebastienrousseau/commons/compare/v0.0.8...HEAD
[v0.0.8]: https://github.com/sebastienrousseau/commons/compare/v0.0.7...v0.0.8
[v0.0.7]: https://github.com/sebastienrousseau/commons/compare/v0.0.6...v0.0.7
[v0.0.6]: https://github.com/sebastienrousseau/commons/compare/v0.0.5...v0.0.6
[v0.0.5]: https://github.com/sebastienrousseau/commons/compare/v0.0.4...v0.0.5
[v0.0.4]: https://github.com/sebastienrousseau/commons/compare/v0.0.3...v0.0.4
[v0.0.3]: https://github.com/sebastienrousseau/commons/compare/v0.0.2...v0.0.3
[v0.0.2]: https://github.com/sebastienrousseau/commons/compare/v0.0.1...v0.0.2
[v0.0.1]: https://github.com/sebastienrousseau/commons/releases/tag/v0.0.1
