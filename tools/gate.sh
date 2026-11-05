#!/bin/sh
# The push gate: formatting, clippy with warnings as errors, every test.
# The pre-push hook runs it; CI runs it only when started by hand.
set -eu
cd "$(dirname "$0")/.."
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
