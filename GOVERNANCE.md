# Governance

## Current model

`commons` is maintained by a single maintainer,
[Sebastian Rousseau](https://github.com/sebastienrousseau). This
document records how decisions are actually made today rather than
describing a larger structure the project does not yet have.

## Roles

**Users** file issues and ask questions. No commitment is expected.

**Contributors** open pull requests. Every contribution is reviewed by
the maintainer; there is no separate approval tier.

**Maintainers** merge pull requests, cut releases and hold publish
rights on crates.io.

### Becoming a maintainer

There is no formal process yet. A contributor with a sustained record of
reviewed, merged work may be invited by the existing maintainer. If that
ever happens, this document is updated in the same pull request.

## How decisions are made

Ordinary changes are decided by the maintainer on the pull request.
Anything that changes the public API, the MSRV, or the feature layout is
recorded as an ADR under [`doc/adr/`](doc/adr/) so the reasoning survives
the pull request that introduced it.

Disagreements are resolved in the open, on the issue or pull request. If
consensus is not reached, the maintainer decides and says why.

## Compatibility and releases

Every `0.0.x` release is its own SemVer compatibility range — `0.0.4` is
not a drop-in replacement for `0.0.3`. Breaking changes are therefore
permitted between patch releases at this stage, but they are called out
in [CHANGELOG.md](CHANGELOG.md) rather than left for downstreams to
discover.

The MSRV and deprecation policy is in
[doc/MSRV-AND-DEPRECATION.md](doc/MSRV-AND-DEPRECATION.md). Releases are
published to crates.io through Trusted Publishing, so no long-lived
registry token exists for this repository.

## Code of conduct and security

Behaviour is governed by [CODE_OF_CONDUCT.md](CODE_OF_CONDUCT.md).
Vulnerabilities should be reported as described in
[SECURITY.md](SECURITY.md), not as public issues.

## Changing this document

By pull request, like any other change.
