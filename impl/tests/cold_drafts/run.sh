#!/usr/bin/env bash
# Checks each cold draft (cases/: four programs written from the card alone,
# before any check, CHG-0452) with a tool (default: cobc --check) and prints
# each draft's whole report, under a line `== name`. With --compare FILE,
# exits 1 if the output differs from FILE (expected.txt).
#   ./run.sh                          the reports, to standard output
#   TOOL="mycc --check" ./run.sh      another tool
#   ./run.sh --compare expected.txt
set -u
here=$(cd "$(dirname "$0")" && pwd)
tool=${TOOL:-$here/../../target/release/cobc --check}
ulimit -t 120
out=$(mktemp)
for f in "$here"/cases/*.cb; do
    name=$(basename "$f" .cb)
    echo "== $name" >> "$out"
    (cd "$here/cases" && timeout 60 $tool "$name.cb" < /dev/null >> "$out" 2>&1)
done
if [ "${1:-}" = "--compare" ]; then
    diff "$2" "$out" > /dev/null && { rm -f "$out"; echo "cold_drafts: all as expected"; exit 0; }
    diff "$2" "$out"
    rm -f "$out"
    exit 1
fi
cat "$out"
rm -f "$out"
