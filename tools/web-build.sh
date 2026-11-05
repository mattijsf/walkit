#!/bin/sh
# Builds the engine for the web (wasm32-unknown-unknown) in both profiles, runs wasm-opt when
# it is installed, and prints each module's size raw, gzipped and brotli-compressed.
#
#   tools/web-build.sh [out dir]      # default: web/dist
#
# Needs the wasm target of a rustup toolchain (`rustup target add wasm32-unknown-unknown`);
# a cargo from Homebrew has no wasm target, so the rustup proxy in ~/.cargo/bin comes first.
set -eu
cd "$(dirname "$0")/.."
out=${1:-web/dist}
mkdir -p "$out"
PATH="$HOME/.cargo/bin:$PATH"
export PATH
# the page build (no recorder) and the build with the recorder (`trace` feature, for the
# parity run and the measurements), each in both profiles
build() {
  profile=$1
  name=$2
  shift 2
  cargo build -q -p walkit-web --profile "$profile" --target wasm32-unknown-unknown "$@"
  cp "target/wasm32-unknown-unknown/$profile/walkit_web.wasm" "$out/$name.wasm"
  if command -v wasm-opt >/dev/null 2>&1; then
    # the features Rust's wasm32-unknown-unknown target emits
    wasm-opt --enable-bulk-memory --enable-nontrapping-float-to-int --enable-sign-ext \
      --enable-mutable-globals --enable-multivalue --enable-reference-types \
      -Oz --strip-debug --strip-producers -o "$out/$name.opt.wasm" "$out/$name.wasm"
  fi
}
build release walkit-release --no-default-features
build small walkit-small --no-default-features
build release walkit-release-trace
build small walkit-small-trace
echo "module  raw  gzip-9  brotli-9"
for f in "$out"/walkit-*.wasm; do
  raw=$(wc -c < "$f" | tr -d ' ')
  gz=$(gzip -9 -c "$f" | wc -c | tr -d ' ')
  if command -v brotli >/dev/null 2>&1; then br=$(brotli -9 -c "$f" | wc -c | tr -d ' '); else br=-; fi
  echo "$(basename "$f")  $raw  $gz  $br"
done
