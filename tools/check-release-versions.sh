#!/bin/sh
# Every version release-please bumps (release-please-config.json's extra-files) is the version in
# .release-please-manifest.json, and every line it bumps by marker still has its marker. XcodeGen
# rewrites project.pbxproj without the marker comments, which this catches before a release does.
set -eu
cd "$(dirname "$0")/.."
v=$(sed -n 's/.*"\.": *"\([^"]*\)".*/\1/p' .release-please-manifest.json)
[ -n "$v" ] || { echo "no version in .release-please-manifest.json" >&2; exit 1; }
fail=0
bad() { echo "release version $v: $1" >&2; fail=1; }

grep -qx "version = \"$v\"" Cargo.toml || bad "Cargo.toml's [workspace.package] version"
lock_version() { # lockfile, package name
  awk -v n="$2" '$0 == "name = \"" n "\"" { getline; gsub(/version = |"/, ""); print }' "$1"
}
for p in walkit walkit-android walkit-trace walkit-web; do
  [ "$(lock_version Cargo.lock "$p")" = "$v" ] || bad "Cargo.lock's $p"
done
[ "$(lock_version examples/macos/rust/Cargo.lock walkit)" = "$v" ] || bad "examples/macos/rust/Cargo.lock's walkit"

# file, pattern of the lines that carry the version, how many there are
marked() {
  n=$(grep -c "$2" "$1" || true)
  m=$(grep "$2" "$1" | grep -F "$v" | grep -c 'x-release-please-version' || true)
  [ "$n" -eq "$3" ] && [ "$m" -eq "$3" ] || bad "$1: $3 line(s) like '$2' at $v with an x-release-please-version marker"
}
marked android/walkit/build.gradle.kts '^version = ' 1
marked examples/android/app/build.gradle.kts 'nl\.mattix\.walkit:walkit:' 1
marked examples/macos/project.yml 'MARKETING_VERSION:' 1
marked examples/macos/WalkitExample.xcodeproj/project.pbxproj 'MARKETING_VERSION = ' 2
# the Markdown takes a start/end block around the lines instead
[ "$(grep -c "nl\.mattix\.walkit:walkit:$v" android/README.md)" -eq 2 ] || bad "android/README.md's two nl.mattix.walkit:walkit versions"
[ "$(grep -c 'x-release-please-start-version' android/README.md)" -eq 2 ] || bad "android/README.md's two x-release-please-start-version blocks"
exit $fail
