#!/usr/bin/env bash
# Compares cbc (impl/selfhost/cbc, the code generator written in CobaltC)
# with `cobc --bootstrap-form` (D-0274), byte for byte, on each FILE given
# (default: every `.cb` file git tracks): the C each writes for it. A
# program cobc rejects is skipped. Also checks that
# impl/selfhost/back/header.cb is what gen_header.py makes of cbrt.h now.
#
#   impl/selfhost/compare_c.sh [-j JOBS] [FILE...]
#
# Results in $OUT/results (`same`, `DIFF`, `skip`), the two C files of each
# DIFF in $OUT/run. cbc is impl/target/selfhost/cbc (impl/selfhost/build.sh).
set -u
here=$(cd "$(dirname "$0")" && pwd)
root=$(cd "$here/../.." && pwd)
cobc="$root/impl/target/release/cobc"
cbc="${CBC:-$root/impl/target/selfhost/cbc}"
jobs=3
while getopts "j:" o; do
    case $o in
        j) jobs=$OPTARG ;;
        *) exit 2 ;;
    esac
done
shift $((OPTIND - 1))
for t in "$cobc" "$cbc"; do
    [ -x "$t" ] || { echo "compare_c.sh: $t is missing; build it first" >&2; exit 2; }
done
OUT=${OUT:-$root/stress/selfhost/c-$(date +%Y%m%d-%H%M%S)}
mkdir -p "$OUT/run"
OUT=$(cd "$OUT" && pwd)
ulimit -t 7200
if [ $# -gt 0 ]; then
    printf '%s\n' "$@" > "$OUT/files"
else
    (cd "$root" && git ls-files '*.cb') > "$OUT/files"
fi
: > "$OUT/results"
python3 -I "$here/back/gen_header.py" "$OUT/run/header.cb"
if cmp -s "$OUT/run/header.cb" "$here/back/header.cb"; then
    echo "same header" >> "$OUT/results"
else
    echo "DIFF header (regenerate: python3 -I impl/selfhost/back/gen_header.py)" >> "$OUT/results"
fi
rm -f "$OUT/run/header.cb"

one() {
    local f=$1 tag d
    tag=$(printf '%s' "$f" | tr '/' '_')
    d=$(mktemp -d "$OUT/run/c.XXXXXX")
    if ! (cd "$root" && COBALTC_BOOTSTRAP_FORM=1 timeout -k 5 600 "$cobc" --cc /bin/true --keep-c -o "$d/a" "$f" > "$d/cobc.log" 2>&1); then
        rm -rf "$d"
        echo "skip $f"
        return
    fi
    (cd "$root" && timeout -k 5 3600 "$cbc" --emit-c --c-name "$d/a.c" "$f" > "$d/b.c" 2> "$d/cbc.log")
    if cmp -s "$d/a.c" "$d/b.c"; then
        rm -rf "$d"
        echo "same $f"
    else
        local n
        n=$(cmp "$d/a.c" "$d/b.c" 2>/dev/null | sed -n 's/.*line \([0-9]*\).*/\1/p')
        mv "$d" "$OUT/run/$tag"
        echo "DIFF $f (first at line ${n:-?}; $(head -c 160 "$OUT/run/$tag/cbc.log" | tr '\n' ' '))"
    fi
}
export -f one
export root cobc cbc OUT
xargs -P "$jobs" -I{} bash -c 'one "$@"' _ {} < "$OUT/files" >> "$OUT/results"
sort -o "$OUT/results" "$OUT/results"
printf '%s same, %s DIFF, %s skip\n' "$(grep -c '^same' "$OUT/results")" "$(grep -c '^DIFF' "$OUT/results")" "$(grep -c '^skip' "$OUT/results")"
