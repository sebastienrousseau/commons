<!-- SPDX-License-Identifier: Apache-2.0 OR MIT -->

# Security policy

## Reporting a vulnerability

Report privately through the repository's **Security → Report a
vulnerability** page. Do not open a public issue.

Include what you can: affected version, a reproduction, and the impact
you believe it has. A first response should come within a few days.

## Supported versions

The crate is pre-1.0. Only the latest `0.0.x` receives fixes; there are
no maintained older branches.

## Posture

- `unsafe_code = "deny"` at the crate root. There is no `unsafe` in
  this crate, and adding any is a reviewable event.
- `cargo audit` runs in CI and an advisory blocks a release.
- Where an advisory cannot be fixed, because the fix lives in a
  transitive dependency with no released version, it is recorded with
  the reason rather than silenced.
- Publishing uses crates.io Trusted Publishing, so no long-lived
  registry credential exists in the repository.

Full detail in [`doc/POLICIES.md`](doc/POLICIES.md).
