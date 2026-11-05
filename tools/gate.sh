#!/bin/sh
# The push gate: formatting, clippy with warnings as errors, every test.
# The pre-push hook runs it; CI runs it only when started by hand.
set -eu
cd "$(dirname "$0")/.."
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
# the web crate is only code on the wasm target: clippy it there when the toolchain has the target
if rustup target list --installed 2>/dev/null | grep -q wasm32-unknown-unknown; then
  PATH="$HOME/.cargo/bin:$PATH" cargo clippy -p walkit-web --target wasm32-unknown-unknown -- -D warnings
fi
cargo test --workspace
# the Kotlin side of the Android library, where the SDK and cargo-ndk are present: no device needed
if [ -d "${ANDROID_HOME:-$HOME/Library/Android/sdk}" ] && [ -x "$HOME/.cargo/bin/cargo-ndk" ]; then
  (cd android && PATH="$HOME/.cargo/bin:$PATH" ./gradlew -q gate)
fi
