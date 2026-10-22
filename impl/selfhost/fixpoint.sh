#!/usr/bin/env bash
# The fixed point of the code generator written in CobaltC (D-0274): cbc,
# built by cobc, writes the C of cbc's own source (stage 1), which must be
# what `cobc --bootstrap-form` writes for it; the C compiler builds cbc2 from
# stage 1; cbc2 writes cbc's C again (stage 2), which must equal stage 1.
#
#   impl/selfhost/fixpoint.sh
#
# Takes about two hours and 1 GB (stage 2 runs the bootstrap form, every
# check on). CC picks the C compiler (default gcc); OUT the directory
# (default stress/selfhost/fixpoint-DATE). Exit status 0 at the fixed point.
set -u
here=$(cd "$(dirname "$0")" && pwd)
root=$(cd "$here/../.." && pwd)
cobc="$root/impl/target/release/cobc"
cc=${CC:-gcc}
OUT=${OUT:-$root/stress/selfhost/fixpoint-$(date +%Y%m%d-%H%M%S)}
mkdir -p "$OUT"
OUT=$(cd "$OUT" && pwd)
ulimit -t 40000
main="$here/cbc/main.cb"
cname="$OUT/stage.c"
cd "$root"
echo "== cbc1 (cobc -O 2) $(date +%T)"
timeout -k 10 2400 "$cobc" -O 2 -o "$OUT/cbc1" "$main" || { echo "fixpoint.sh: building cbc failed"; exit 1; }
echo "== stage 1 $(date +%T)"
timeout -k 10 14400 "$OUT/cbc1" --emit-c --c-name "$cname" "$main" > "$OUT/stage1.c" 2> "$OUT/stage1.err" \
    || { echo "fixpoint.sh: stage 1 failed"; tail -5 "$OUT/stage1.err"; exit 1; }
# cobc's C, at the name cbc's `#line` directives give it.
COBALTC_BOOTSTRAP_FORM=1 timeout -k 10 2400 "$cobc" --cc /bin/true --keep-c -o "$OUT/stage" "$main" > "$OUT/ref.log" 2>&1 \
    || { echo "fixpoint.sh: cobc --bootstrap-form failed"; exit 1; }
if cmp -s "$cname" "$OUT/stage1.c"; then
    echo "stage 1 == cobc --bootstrap-form"
else
    echo "fixpoint.sh: stage 1 differs from cobc --bootstrap-form"; cmp "$cname" "$OUT/stage1.c" | head -2; exit 1
fi
rm -f "$cname" "$OUT/stage"
echo "== $cc $(date +%T)"
libs=$(printf ' -l%s' pthread dl m)
timeout -k 10 14000 "$cc" -std=gnu11 -ffp-contract=off -fno-optimize-sibling-calls -w -O2 -o "$OUT/cbc2" "$OUT/stage1.c" \
    -L "$root/impl/target/release" -lcbrt $libs > "$OUT/cc.log" 2>&1 || { echo "fixpoint.sh: $cc failed"; tail "$OUT/cc.log"; exit 1; }
echo "== stage 2 $(date +%T)"
timeout -k 10 30000 "$OUT/cbc2" --emit-c --c-name "$cname" "$main" > "$OUT/stage2.c" 2> "$OUT/stage2.err" \
    || { echo "fixpoint.sh: stage 2 failed"; tail -5 "$OUT/stage2.err"; exit 1; }
echo "== done $(date +%T)"
if cmp -s "$OUT/stage1.c" "$OUT/stage2.c"; then
    echo "fixed point: stage 2 == stage 1 ($(wc -c < "$OUT/stage1.c") bytes)"
else
    echo "fixpoint.sh: stage 2 differs from stage 1"; cmp "$OUT/stage1.c" "$OUT/stage2.c" | head -2; exit 1
fi
