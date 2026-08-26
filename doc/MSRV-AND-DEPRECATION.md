<!-- SPDX-FileCopyrightText: 2026 Euxis Commons -->
<!-- SPDX-License-Identifier: MIT OR Apache-2.0 -->

# MSRV and deprecation policy

## Minimum supported Rust version

The MSRV is **1.88.0**, declared as `rust-version` in the crate
manifest and enforced by a dedicated CI job so it cannot drift
unnoticed.

Raising it is a deliberate act, not a side effect of using a new
language feature:

- it happens in a minor release, never a patch,
- the release notes say which feature required it and why the
  workaround was worse,
- the new floor is at least six months old at the time of release.

`Mutex::new` being `const` since 1.63 is an example of the policy
working: `counter.rs` needs a const constructor for its statics, and
that requirement sits comfortably below the floor, so it cost nothing.

## Editions

The crate is on the 2024 edition. Edition upgrades follow the same
rule as MSRV bumps: minor release, documented rationale.

## Deprecation

Public items are removed in two steps.

1. **Deprecate.** The item gains `#[deprecated(since = "…", note =
   "…")]` with a note naming the replacement. It keeps working.
2. **Remove.** No earlier than two minor releases later, and only in a
   release whose notes list it.

While the crate is pre-1.0 this policy is a commitment rather than a
SemVer guarantee: `0.0.x` versions may technically break at any time,
and the policy exists so that in practice they do not.

## Dependency policy

Every dependency except the `logging` set is optional and feature
gated, so a consumer that wants one utility does not inherit the rest.

Security advisories are treated as release blockers. Where an advisory
cannot be fixed — because the fix lives in a transitive dependency
with no released version — that is recorded rather than silenced, so
the reason is visible at the next audit.
