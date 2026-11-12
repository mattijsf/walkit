#!/bin/sh
# The push gate: formatting, clippy with warnings as errors, every test.
# The pre-push hook runs it; CI runs it only when started by hand.
set -eu
cd "$(dirname "$0")/.."
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
# the web crate is only code on the wasm target: clippy it there when the toolchain has the target
if "$HOME/.cargo/bin/rustup" target list --installed 2>/dev/null | grep -q wasm32-unknown-unknown; then
  PATH="$HOME/.cargo/bin:$PATH" cargo clippy -p walkit-web --target wasm32-unknown-unknown -- -D warnings
fi
cargo test --workspace
# the engine's optional TrueType letters, which no crate of the workspace turns on
cargo clippy -p walkit --features truetype --all-targets -- -D warnings
cargo test -p walkit --features truetype truetype
# the web player's modules parse (node is what serves them)
if command -v node >/dev/null 2>&1; then
  for f in web/*.mjs; do node --check "$f"; done
fi
# the Kotlin side of the Android library, where the SDK and cargo-ndk are present: no device needed
if [ -d "${ANDROID_HOME:-$HOME/Library/Android/sdk}" ] && [ -x "$HOME/.cargo/bin/cargo-ndk" ]; then
  (cd android && PATH="$HOME/.cargo/bin:$PATH" ./gradlew -q gate)
fi
# the versions release-please bumps agree with its manifest and keep their markers
tools/check-release-versions.sh
