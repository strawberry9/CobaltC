#!/usr/bin/env bash
# The fixed point of the code generator written in CobaltC (D-0274): cbc,
# built by cobc, writes the C of cbc's own source (stage 1), which must be
# what cobc writes for it; the C compiler builds cbc2 from stage 1; cbc2
# writes cbc's C again (stage 2), which must equal stage 1.
#
#   impl/selfhost/fixpoint.sh
#
# LEVEL (default 0) is the D-0274 level both compilers write at
# (`COBALTC_BOOTSTRAP_UPTO=LEVEL`; 0 the default form, every optimisation on;
# 10 the bootstrap form, which takes about two hours, its stage 2 running with
# every check on). CC picks the C compiler (default gcc); OUT the directory
# (default stress/selfhost/fixpoint-DATE). Exit status 0 at the fixed point.
set -u
here=$(cd "$(dirname "$0")" && pwd)
root=$(cd "$here/../.." && pwd)
cobc="$root/impl/target/release/cobc"
cc=${CC:-gcc}
level=${LEVEL:-0}
if [ "$level" -gt 0 ]; then export COBALTC_BOOTSTRAP_UPTO=$level; else unset COBALTC_BOOTSTRAP_UPTO COBALTC_BOOTSTRAP_FORM; fi
OUT=${OUT:-$root/stress/selfhost/fixpoint-$(date +%Y%m%d-%H%M%S)}
mkdir -p "$OUT"
OUT=$(cd "$OUT" && pwd)
ulimit -t 40000
main="$here/cbc/main.cb"
cname="$OUT/stage.c"
cd "$root"
echo "== cbc1 (cobc -O 2, default form) $(date +%T)"
env -u COBALTC_BOOTSTRAP_UPTO timeout -k 10 2400 "$cobc" -O 2 -o "$OUT/cbc1" "$main" || { echo "fixpoint.sh: building cbc failed"; exit 1; }
echo "== stage 1 (level $level) $(date +%T)"
timeout -k 10 14400 "$OUT/cbc1" --emit-c --c-name "$cname" "$main" > "$OUT/stage1.c" 2> "$OUT/stage1.err" \
    || { echo "fixpoint.sh: stage 1 failed"; tail -5 "$OUT/stage1.err"; exit 1; }
# cobc's C at the same level, at the name cbc's `#line` directives give it.
timeout -k 10 2400 "$cobc" --cc /bin/true --keep-c -o "$OUT/stage" "$main" > "$OUT/ref.log" 2>&1 \
    || { echo "fixpoint.sh: cobc failed"; exit 1; }
if cmp -s "$cname" "$OUT/stage1.c"; then
    echo "stage 1 == cobc (level $level)"
else
    echo "fixpoint.sh: stage 1 differs from cobc (level $level)"; cmp "$cname" "$OUT/stage1.c" | head -2; exit 1
fi
rm -f "$cname" "$OUT/stage"
# cbc's own C code (`extern "./host.c";`, cbc/host.cb) is linked with it.
echo "== $cc $(date +%T)"
libs=$(printf ' -l%s' pthread dl m)
timeout -k 10 14000 "$cc" -std=gnu11 -ffp-contract=off -fno-optimize-sibling-calls -w -O2 -o "$OUT/cbc2" "$OUT/stage1.c" "$here/cbc/host.c" \
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
