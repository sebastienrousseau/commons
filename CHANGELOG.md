# Changelog

All notable changes to this project are documented here.

Format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/).
This project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

Note that under SemVer, every `0.0.x` release is its own compatibility
range: `0.0.4` is not a drop-in for `0.0.3`. See
[MSRV-AND-DEPRECATION.md](doc/MSRV-AND-DEPRECATION.md).

## [Unreleased]

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
  that needed it — `rlg` — could not use the fix it was written for.

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

[Unreleased]: https://github.com/sebastienrousseau/commons/compare/v0.0.4...HEAD
[v0.0.4]: https://github.com/sebastienrousseau/commons/compare/v0.0.3...v0.0.4
[v0.0.3]: https://github.com/sebastienrousseau/commons/compare/v0.0.2...v0.0.3
[v0.0.2]: https://github.com/sebastienrousseau/commons/compare/v0.0.1...v0.0.2
[v0.0.1]: https://github.com/sebastienrousseau/commons/releases/tag/v0.0.1
