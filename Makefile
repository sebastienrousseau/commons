# POSIX-compatible Makefile for commons
# Works on macOS, Linux, and WSL without modification.
#
# Usage:
#   make            — run check + clippy + test (default)
#   make check      — type-check every target
#   make clippy     — run clippy lints, warnings denied
#   make test       — run all tests
#   make fmt        — check formatting
#   make deny       — run cargo-deny supply-chain checks
#   make audit      — check for security advisories
#   make doc        — build documentation
#   make bench      — run the benchmark suite
#   make examples   — run all examples sequentially
#   make features   — check each feature in isolation
#   make msrv       — verify the crate builds on its declared MSRV
#   make clean      — remove build artifacts

.PHONY: all check clippy test fmt deny audit doc bench examples features msrv clean

all: check clippy test

check:
	cargo check --all-features --all-targets

clippy:
	cargo clippy --all-features --all-targets -- -D warnings

test:
	cargo test --all-features

fmt:
	cargo fmt --check

deny:
	cargo deny check

audit:
	cargo audit

doc:
	cargo doc --all-features --no-deps

bench:
	cargo bench

examples:
	@for e in $$(cargo metadata --no-deps --format-version 1 \
		| sed -n 's/.*"kind":\["example"\],"crate_types":\["bin"\],"name":"\([^"]*\)".*/\1/p'); do \
		echo "── $$e"; cargo run --all-features --example "$$e" || exit 1; \
	done

# Every feature is meant to compile on its own. `--all-features` hides
# gating bugs, so this checks each in isolation against a no-default
# baseline.
features:
	cargo check --no-default-features
	@for f in config error logging time collections validation retry id env fs counter; do \
		echo "── $$f"; \
		cargo check --no-default-features --features "$$f" || exit 1; \
	done

msrv:
	cargo +1.88.0 check --all-features

clean:
	cargo clean
