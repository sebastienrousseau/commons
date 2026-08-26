<!-- SPDX-FileCopyrightText: 2026 Euxis Commons -->
<!-- SPDX-License-Identifier: MIT OR Apache-2.0 -->

# `euxis-commons` — engineering policies

What this crate promises, and what it does not.

## Contents

1. [MSRV](#1-msrv)
2. [SemVer and API stability](#2-semver-and-api-stability)
3. [Features](#3-features)
4. [Security and audits](#4-security-and-audits)
5. [Supply chain](#5-supply-chain)
6. [Platform support](#6-platform-support)
7. [Testing](#7-testing)

## 1. MSRV

**1.88.0**, declared as `rust-version` and enforced by a dedicated CI
job so it cannot drift unnoticed.

Raising it is deliberate: it happens in a minor release, the notes say
which feature required it, and the new floor is at least six months old
at the time of release. Full detail in
[`MSRV-AND-DEPRECATION.md`](MSRV-AND-DEPRECATION.md).

## 2. SemVer and API stability

The crate is `0.0.x`. Under Cargo's rules **every `0.0.x` release is its
own compatibility range** — `0.0.4` does not satisfy `"0.0.3"`. That is
not a formality: a consumer pinning `"0.0.3"` will not pick up `0.0.4`
from `cargo update` alone, and the requirement has to be edited. Expect
to do that on each release until 1.0.

### Breaking

Removing or renaming a public item; narrowing a function's accepted
types or widening its returned ones; adding a required field to a
public struct; adding a variant to a non-`#[non_exhaustive]` enum;
raising the MSRV.

### Not breaking

Adding a new module, function or feature; adding a variant to a
`#[non_exhaustive]` enum; performance changes; documentation; anything
behind `#[doc(hidden)]`.

### Deprecation

Two steps, never one: deprecate with a note naming the replacement,
then remove no earlier than two minor releases later. Pre-1.0 this is a
commitment rather than a SemVer guarantee.

## 3. Features

Every module is feature gated and `default = ["full"]`. Three rules:

1. **A feature enables everything it uses**, including for derive
   macros. `config` derives `thiserror::Error`, so it implies `error`.
2. **Features are additive.** Enabling one never removes or changes
   behaviour another provides.
3. **Every feature is checked in isolation by CI.** `--all-features` is
   the one configuration in which feature-gating bugs are invisible, and
   two lived in this crate undetected because that was all CI built.

## 4. Security and audits

- `unsafe_code = "deny"` at the crate root. There is no `unsafe` in this
  crate and adding any is a reviewable event.
- `cargo audit` runs in CI. An advisory is a release blocker.
- Where an advisory cannot be fixed — the fix lives in a transitive
  dependency with no released version — it is recorded with the reason
  rather than silenced, so it is visible at the next audit rather than
  quietly inherited.

Report a vulnerability privately via the repository's security advisory
page rather than a public issue.

## 5. Supply chain

Dependencies are minimal and optional. Seven of the ten modules pull
nothing at all; `config` and `error` pull one well-known crate each;
`logging` is the only substantial set.

Publishing uses **crates.io Trusted Publishing**: the release workflow
authenticates over OIDC and no long-lived registry token exists in the
repository.

One consequence worth recording for downstream consumers: crates
published this way report `published_by: null`, because the publisher is
a workflow rather than a user. `cargo-vet` `[[trusted]]` entries key on
a publisher, so they cannot cover this crate — downstream users need an
exemption or an audit entry instead.

## 6. Platform support

Tier 1 is Linux, macOS and Windows on x86-64 and aarch64. Beyond that
the crate is expected to *compile* anywhere `std` is available,
including targets without 64-bit atomics — see
[ADR-0002](adr/0002-portable-counters.md).

Platform-specific behaviour is `cfg`-gated rather than
runtime-branched, so a lint or type error in one branch is invisible
when building on another host. CI compiles the other branches
explicitly; [`TESTING.md`](TESTING.md) explains how to do the same
locally.

## 7. Testing

Coverage sits at roughly 97% region / 98% line. The remainder is
derive-generated regions, exhaustiveness arms, and `cfg`-gated code
that cannot execute on the host measuring it. A literal 100% would mean
deleting real portability code, so it is not a target.

Documentation examples are compiled, not merely written:
`tests/guide_examples.rs` exercises every snippet in the user guide.
Two errors in the first draft — a `Config::get` type that is not
implemented, and a compound duration string that is not parsed — were
caught by that test rather than by a reader.
