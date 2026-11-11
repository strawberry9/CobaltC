#!/usr/bin/env bash
# Checks each one-habit program (cases/, written by make_cases.py) with a
# tool (default: coby --check) and prints, per case, its habit and the first
# line of the diagnostic it gets: `name | habit | first line`. With
# --compare FILE, exits 1 if any line differs from FILE (expected.txt).
#   ./run.sh                          the table, to standard output
#   TOOL="cbc --check" ./run.sh       another tool
#   ./run.sh --compare expected.txt
set -u
here=$(cd "$(dirname "$0")" && pwd)
tool=${TOOL:-$here/../../target/release/coby --check}
ulimit -t 120
out=$(mktemp)
for f in "$here"/cases/*.cb; do
    name=$(basename "$f" .cb)
    habit=$(head -n 1 "$f" | sed 's|^// ||')
    first=$(cd "$here/cases" && timeout 60 $tool "$name.cb" < /dev/null 2>&1 | head -n 1)
    [ -z "$first" ] && first="(accepted)"
    printf '%s | %s | %s\n' "$name" "$habit" "$first" >> "$out"
done
if [ "${1:-}" = "--compare" ]; then
    diff "$2" "$out" > /dev/null && { rm -f "$out"; echo "foreign_forms: all as expected"; exit 0; }
    diff "$2" "$out"
    rm -f "$out"
    exit 1
fi
cat "$out"
rm -f "$out"
