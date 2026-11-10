#!/bin/sh
# SPDX-License-Identifier: GPL-3.0-or-later
#
# Builds the walkit-macos static library for the architectures Xcode builds (ARCHS; this Mac's
# own when run by hand) and puts one library for all of them at target/macos/libwalkit_macos.a,
# where the Xcode project links it from. Xcode runs it before compiling the Swift sources.
set -eu
cd "$(dirname "$0")"

# a build phase gets a short PATH: rustup's cargo lives in ~/.cargo/bin
export PATH="$HOME/.cargo/bin:/opt/homebrew/bin:/usr/local/bin:$PATH"
# Xcode's SDK settings are for clang, not for the Rust build
unset SDKROOT

libs=""
for arch in ${ARCHS:-$(uname -m)}; do
    case "$arch" in
        arm64) target=aarch64-apple-darwin ;;
        x86_64) target=x86_64-apple-darwin ;;
        *) echo "build.sh: no Rust target for $arch" >&2; exit 1 ;;
    esac
    rustup target add "$target" >/dev/null 2>&1 || true
    cargo build --release --target "$target"
    libs="$libs target/$target/release/libwalkit_macos.a"
done

mkdir -p target/macos
# shellcheck disable=SC2086 # one path per architecture
lipo -create $libs -output target/macos/libwalkit_macos.a
