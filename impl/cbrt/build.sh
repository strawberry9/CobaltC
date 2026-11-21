#!/usr/bin/env bash
# Builds the runtime, written in C (D-0326), as OUT/libcbrt.a.
#
#   impl/cbrt/build.sh [--cc CC] [--out DIR] [--san asan|tsan] [--stamp]
#
# --cc names the C compiler (default $CC, else gcc); a compiler whose name
# starts with x86_64-w64-mingw32- builds the Windows runtime. --out is
# where libcbrt.a goes (default impl/target/release). --san builds an
# instrumented runtime (AddressSanitizer with UBSan, or ThreadSanitizer).
# --stamp prints the sources' stamp and builds nothing.
#
# The flags are FLAGS's; every src/*.c is a translation unit except the
# parts of the core (src/rt_*.c, src/replay.c), which src/lib.c includes, so
# the core is one unit. The library is one relocatable object whose only
# global symbols are tools/exports.txt's, so the runtime's own names never
# meet a program's C code at link time. The stamp (tools/stamp.c, over the
# files SOURCES lists) is baked into the library; the compilers read it from
# there or compute it the same way. impl/bootstrap/boot.c does the same
# steps on a machine with no shell.
set -eu
here=$(cd "$(dirname "$0")" && pwd)
cc=${CC:-gcc}
out=$here/../target/release
san=
only_stamp=
while [ $# -gt 0 ]; do
    case "$1" in
    --cc) cc=$2; shift 2 ;;
    --out) out=$2; shift 2 ;;
    --san) san=$2; shift 2 ;;
    --stamp) only_stamp=1; shift ;;
    *) echo "build.sh: unknown argument $1" >&2; exit 2 ;;
    esac
done
case "$cc" in
x86_64-w64-mingw32-*) tool_prefix=x86_64-w64-mingw32- ;;
*) tool_prefix= ;;
esac
host_cc=${HOST_CC:-}
if [ -z "$host_cc" ]; then
    if [ -n "$tool_prefix" ]; then host_cc=gcc; else host_cc=$cc; fi
fi
mkdir -p "$out"
out=$(cd "$out" && pwd)
work=$out/cbrt-build
rm -rf "$work"
mkdir -p "$work"
"$host_cc" -O1 -o "$work/stamp" "$here/tools/stamp.c"
stamp=$("$work/stamp" "$here")
if [ -n "$only_stamp" ]; then
    echo "$stamp"
    rm -rf "$work"
    exit 0
fi
flags="$(cat "$here/FLAGS") -Wall -Wextra -DCBRT_STAMP=\"$stamp\""
case "$san" in
"") ;;
asan) flags="$flags -g -fsanitize=address,undefined -fno-omit-frame-pointer -fno-sanitize-recover=undefined" ;;
tsan) flags="$flags -g -fsanitize=thread" ;;
*) echo "build.sh: --san asan or tsan" >&2; exit 2 ;;
esac
objs=
for f in "$here"/src/*.c; do
    b=$(basename "$f" .c)
    case "$b" in rt_*|replay) continue ;; esac
    # shellcheck disable=SC2086
    "$cc" $flags -c "$f" -o "$work/$b.o"
    objs="$objs $work/$b.o"
done
ld=${tool_prefix}ld
objcopy=${tool_prefix}objcopy
ar=${tool_prefix}ar
command -v "$ld" >/dev/null 2>&1 || ld=ld.lld
command -v "$objcopy" >/dev/null 2>&1 || objcopy=llvm-objcopy
command -v "$ar" >/dev/null 2>&1 || ar=llvm-ar
# shellcheck disable=SC2086
"$ld" -r -o "$work/cbrt.o" $objs
"$objcopy" --keep-global-symbols="$here/tools/exports.txt" "$work/cbrt.o"
rm -f "$out/libcbrt.a"
"$ar" rcs "$out/libcbrt.a" "$work/cbrt.o"
rm -rf "$work"
echo "built $out/libcbrt.a (stamp $stamp)"
