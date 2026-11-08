#!/bin/sh
# Diffs the traces of a walkit run against a reference set (the last release's
# traces), scenario by scenario, and prints the first line where each pair parts.
#
#   tools/trace-diff.sh [walkit traces dir] [reference traces dir]
#
# Defaults: traces/ here, and the reference set at $WALKIT_REFERENCE_TRACES.
set -eu
cd "$(dirname "$0")/.."
ours=${1:-traces}
theirs=${2:-${WALKIT_REFERENCE_TRACES:-}}
[ -n "$theirs" ] || { echo "no reference traces: set WALKIT_REFERENCE_TRACES"; exit 2; }
[ -d "$ours" ] || { echo "no traces at $ours"; exit 2; }
[ -d "$theirs" ] || { echo "no reference traces at $theirs"; exit 2; }
same=0
differ=0
missing=0
for ref in "$theirs"/*.trace; do
  name=$(basename "$ref")
  if [ ! -f "$ours/$name" ]; then
    missing=$((missing + 1))
    echo "$name: not recorded"
    continue
  fi
  if cmp -s "$ours/$name" "$ref"; then
    same=$((same + 1))
  else
    differ=$((differ + 1))
    line=$(cmp "$ours/$name" "$ref" 2>/dev/null | sed -n 's/.*line \([0-9]*\).*/\1/p')
    [ -n "$line" ] || line=$(( $(wc -l < "$ours/$name") + 1 ))
    echo "$name: differs at line $line"
    echo "  walkit:    $(sed -n "${line}p" "$ours/$name")"
    echo "  reference: $(sed -n "${line}p" "$ref")"
  fi
done
echo "$same same, $differ differ, $missing not recorded"
[ "$differ" -eq 0 ] && [ "$missing" -eq 0 ]
