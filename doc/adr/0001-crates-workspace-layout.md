<!-- SPDX-FileCopyrightText: 2026 Euxis Commons -->
<!-- SPDX-License-Identifier: MIT OR Apache-2.0 -->

# ADR-0001 — `crates/` workspace layout

**Status:** accepted (2026-08-26)

## Context

The crate lived at the repository root: `src/` beside `Cargo.toml`,
with the package and the build profiles in one manifest.

That is the simplest layout for a single crate, but it makes adding a
second one a breaking move — every path in CI, every `include` entry
and every contributor's muscle memory shifts at once. The rest of the
ecosystem already uses `crates/<name>/`, so the root layout was also
the odd one out.

## Decision

Move the package to `crates/commons/` behind a virtual workspace root.

The root manifest holds `[workspace]` and the shared `[profile.*]`
tables. The member manifest holds the package, features, dependencies
and lints.

## Consequences

Profiles **must** live at the root. Cargo only honours `[profile.*]`
in the workspace root manifest and ignores it elsewhere without
warning, so leaving them in the package would have silently dropped
the release profile — LTO, `codegen-units = 1`, symbol stripping —
from every release build.

Anything that reads the manifest must be checked, not just the build.
Two release-workflow steps assumed a root package:

- `cargo publish` needs `-p euxis-commons`; a bare invocation cannot
  infer the package from a virtual root.
- The tag-validation step grepped `^version` from the root
  `Cargo.toml`, which now has none. It compared the tag against the
  literal `"v"` and failed the first v0.0.3 release. It now uses
  `cargo metadata`, which asks cargo where the package is rather than
  assuming a path.

Adding a satellite crate is now additive: a new directory and one line
in `members`.

## Alternatives considered

**Stay flat.** Cheapest today, but pays the same relocation cost later
with more consumers depending on the paths.

**Real multi-crate split now.** Premature. There is no second crate
yet, and splitting before the seams are obvious tends to put them in
the wrong place.
