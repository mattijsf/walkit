#!/bin/sh
# Builds the page module and puts the walkit-web package together in a folder: the files
# web/package.json lists, with the module (wasm-opt'd when wasm-opt is installed) as
# walkit.wasm. A host that vendors the package copies that folder.
#
#   tools/web-pack.sh <out dir>          # WALKIT_PROFILE=release for speed over size
set -eu
[ $# -eq 1 ] || { echo "usage: tools/web-pack.sh <out dir>" >&2; exit 2; }
out=$1
cd "$(dirname "$0")/.."
PATH="$HOME/.cargo/bin:$PATH"
export PATH
profile=${WALKIT_PROFILE:-small}
cargo build -q -p walkit-web --profile "$profile" --target wasm32-unknown-unknown --no-default-features
wasm=target/wasm32-unknown-unknown/$profile/walkit_web.wasm
mkdir -p "$out"
if command -v wasm-opt >/dev/null 2>&1; then
  wasm-opt --enable-bulk-memory --enable-nontrapping-float-to-int --enable-sign-ext \
    --enable-mutable-globals --enable-multivalue --enable-reference-types \
    -Oz --strip-debug --strip-producers -o "$out/walkit.wasm" "$wasm"
else
  cp "$wasm" "$out/walkit.wasm"
fi
for f in package.json embed.mjs embed.d.ts walkit.mjs frame.mjs render.mjs pictures.mjs zip.mjs; do
  cp "web/$f" "$out/$f"
done
cp LICENSE "$out/LICENSE"
echo "walkit-web $(git rev-parse --short HEAD) in $out"
