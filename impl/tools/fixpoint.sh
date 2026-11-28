#!/usr/bin/env bash
# The compiler's fixed point (D-0274, D-0326): the installed cobc writes the C
# of its own source (stage 1); the C compiler builds cobc2 from stage 1; cobc2
# writes the C of the same source again (stage 2), which must equal stage 1.
#
#   impl/tools/fixpoint.sh
#
# LEVEL (default 0) is the D-0274 level the C is written at
# (`COBALTC_BOOTSTRAP_UPTO=LEVEL`; 0 the default form, every optimisation on,
# which impl/bootstrap/boot.c and the gate's `bootstrap` unit also check; 10
# the bootstrap form, which takes hours alone on the machine and more beside
# other work, its stage 2 running with every check on).
# CC picks the C compiler (default gcc); OUT the directory (default
# stress/fixpoint/DATE, else a temporary one); COBC the compiler of stage 1
# (default impl/target/release/cobc). Exit status 0 at the fixed point.
set -u
here=$(cd "$(dirname "$0")" && pwd)
root=$(cd "$here/../.." && pwd)
cobc=${COBC:-$root/impl/target/release/cobc}
cc=${CC:-gcc}
level=${LEVEL:-0}
if [ "$level" -gt 0 ]; then export COBALTC_BOOTSTRAP_UPTO=$level; else unset COBALTC_BOOTSTRAP_UPTO COBALTC_BOOTSTRAP_FORM; fi
if [ -z "${OUT:-}" ]; then
    if [ -d "$root/stress" ]; then OUT=$root/stress/fixpoint/$(date +%Y%m%d-%H%M%S); else OUT=$(mktemp -d); fi
fi
mkdir -p "$OUT"
OUT=$(cd "$OUT" && pwd)
ulimit -t 40000
src=$root/impl/cobc
cname="$OUT/stage.c"
cd "$src"
echo "== stage 1 (level $level, $cobc) $(date +%T)"
timeout -k 10 14400 "$cobc" --emit-c --c-name "$cname" main.cb > "$OUT/stage1.c" 2> "$OUT/stage1.err" \
    || { echo "fixpoint.sh: stage 1 failed"; tail -5 "$OUT/stage1.err"; exit 1; }
# The compiler's own C code (`extern "./host.c";`, cobc/host.cb) is linked with it.
echo "== $cc $(date +%T)"
libs=$(printf ' -l%s' pthread dl m)
timeout -k 10 14000 "$cc" -std=gnu11 -ffp-contract=off -fno-optimize-sibling-calls -w -O2 -o "$OUT/cobc2" "$OUT/stage1.c" "$src/host.c" \
    -L "$(dirname "$cobc")" -lcbrt $libs > "$OUT/cc.log" 2>&1 || { echo "fixpoint.sh: $cc failed"; tail "$OUT/cc.log"; exit 1; }
cp "$(dirname "$cobc")/libcbrt.a" "$OUT/libcbrt.a"
echo "== stage 2 $(date +%T)"
timeout -k 10 30000 "$OUT/cobc2" --emit-c --c-name "$cname" main.cb > "$OUT/stage2.c" 2> "$OUT/stage2.err" \
    || { echo "fixpoint.sh: stage 2 failed"; tail -5 "$OUT/stage2.err"; exit 1; }
echo "== done $(date +%T)"
if cmp -s "$OUT/stage1.c" "$OUT/stage2.c"; then
    echo "fixed point: stage 2 == stage 1 ($(wc -c < "$OUT/stage1.c") bytes, level $level)"
else
    echo "fixpoint.sh: stage 2 differs from stage 1"; cmp "$OUT/stage1.c" "$OUT/stage2.c" | head -2; exit 1
fi
