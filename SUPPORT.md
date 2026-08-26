<!-- SPDX-License-Identifier: Apache-2.0 OR MIT -->

# Support

## Where to look first

- [`doc/USER-GUIDE.md`](doc/USER-GUIDE.md) — task-oriented tour of the
  API, with examples that are compiled by the test suite.
- [docs.rs/euxis-commons](https://docs.rs/euxis-commons) — exact
  signatures.
- [`doc/ARCHITECTURE.md`](doc/ARCHITECTURE.md) — how the pieces fit
  together, including the logging engine.

## Asking a question

Open a GitHub issue. Useful reports include the crate version, the
feature set you enabled, and the exact command you ran.

Feature flags matter more than usual here: most problems that look like
missing items are a feature that is not enabled.

## Reporting a bug

Say what you expected, what happened, and how to reproduce it. If it is
a build failure, include the target triple — several past issues were
specific to targets without 64-bit atomics and reproduced nowhere else.

## Security

Do not use issues for vulnerabilities. See [`SECURITY.md`](SECURITY.md).

## Expectations

This is maintained alongside other work. Bugs and security reports get
attention first; feature requests may sit. A pull request will usually
move faster than a request.
