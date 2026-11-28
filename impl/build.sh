#!/usr/bin/env bash
# Builds CobaltC with the compiler already installed (D-0326): the runtime
# (impl/cbrt, C) when its sources changed, then the compiler, the formatter
# and the model, written in CobaltC, into impl/target/release/, and the test
# runner and the differential tester into impl/target/tests/. A new machine
# starts from impl/bootstrap/boot.c instead, which needs no compiler.
#
#   impl/build.sh
#
# The runtime: rebuilt into impl/target/release when its sources' stamp is
# not the one in the libcbrt.a there, the library it replaces kept as
# impl/target/prev/libcbrt.a; the Windows runtime too, into
# impl/target/x86_64-pc-windows-gnu/release, where MinGW-w64 is installed.
#
# The compiler it builds with: COBC if set; else impl/target/release/cobc,
# copied first to impl/target/prev/cobc (so the build does not overwrite the
# program running it, and so a broken new cobc leaves the previous one to go
# back to). A compiler refuses a runtime built from other sources than the
# ones it was built with (the stamp), so when the runtime has changed since
# the installed cobc was built (impl/target/release/cobc.stamp), the build
# takes two stages: the previous cobc, given its own runtime (impl/target/prev,
# through COBALTC_CBRT_DIR), builds a cobc that knows the new stamp; that
# cobc builds everything again against the new runtime. The first stage's
# cobc is built at -O 1: it runs once, and its C compiles in under half the
# time (it writes the same C a fifth slower).
#
# Every program is compiled as one C file with no link-time optimisation,
# cobc's default since Update 209 (`--lto` builds in parts), the way
# impl/bootstrap/boot.c compiles the compiler from the seed: built in eight
# parts with link-time optimisation cobc ran 6% slower and was 9% larger.
# A compiler from before Update 209 builds in parts by default and is given
# `--one-unit` for cobc (stage 1 below, or the installed compiler of the first
# build); one from before that option builds in parts.
#
# cobc's build-time constants (impl/cobc/build_info.cb): the runtime's stamp
# and the runtime's directory, written in for the build and taken out again
# after it, so the committed file keeps its neutral form (no stamp, the
# runtime beside the executable) -- the form the seed is made from.
#
# CC picks the C compiler (cobc --cc; default gcc). PROGRAMS the programs
# (default "cobc cobfmt cobmodel cbparse cbtest difftest"; cbparse prints the
# parse as text for impl/tools/factcheck.py). JOBS how many of the programs
# after cobc are built at a time (default 2; each C compile is itself two or
# three processes).
set -eu
impl=$(cd "$(dirname "$0")" && pwd)
rel=$impl/target/release
prev=$impl/target/prev
tests=$impl/target/tests
mkdir -p "$rel" "$prev" "$tests"
cc=()
[ -n "${CC:-}" ] && cc=(--cc "$CC")

stamp_of() {
    LC_ALL=C grep -aoE 'CBRT-STAMP:[0-9a-f]{16}' "$1" 2>/dev/null | head -n 1 | cut -d: -f2 || true
}

# The runtime.
want=$(CC=${CC:-gcc} "$impl/cbrt/build.sh" --stamp)
if [ "$want" != "$(stamp_of "$rel/libcbrt.a")" ]; then
    if [ -f "$rel/libcbrt.a" ]; then
        cp -f "$rel/libcbrt.a" "$prev/libcbrt.a"
    fi
    CC=${CC:-gcc} "$impl/cbrt/build.sh" --out "$rel"
fi
win=$impl/target/x86_64-pc-windows-gnu/release
if command -v x86_64-w64-mingw32-gcc > /dev/null 2>&1 && [ "$want" != "$(stamp_of "$win/libcbrt.a")" ]; then
    "$impl/cbrt/build.sh" --cc x86_64-w64-mingw32-gcc --out "$win"
fi

# The compiler to build with.
if [ -n "${COBC:-}" ]; then
    cobc=$COBC
elif [ -x "$rel/cobc" ]; then
    # Kept as the previous one only when this build replaces it.
    case " ${PROGRAMS:-cobc} " in
    *" cobc "*)
        cp -f "$rel/cobc" "$prev/cobc"
        [ -f "$rel/cobc.stamp" ] && cp -f "$rel/cobc.stamp" "$prev/cobc.stamp"
        cobc=$prev/cobc
        ;;
    *)
        cobc=$rel/cobc
        ;;
    esac
else
    echo "impl/build.sh: no compiler in $rel: build one first with impl/bootstrap/boot.c" >&2
    exit 1
fi

# cobc itself, against the runtime in DIR ($3), with the compiler $1, into
# $2, at the optimisation level $4.
build_cobc() {
    local with=$1 out=$2 dir=$3 level=$4
    local info=$impl/cobc/build_info.cb
    local s
    s=$(stamp_of "$dir/libcbrt.a")
    local d=$dir
    if command -v cygpath > /dev/null 2>&1; then
        d=$(cygpath -m "$d")
    fi
    cp "$info" "$out.build_info.orig"
    trap 'mv -f "'"$out"'.build_info.orig" "'"$info"'"' EXIT
    # A CobaltC literal escapes \ and "; the sed replacement then \, & and |.
    local esc
    esc=$(printf '%s' "$d" | sed -e 's/[\\"]/\\&/g' | sed -e 's/[\\&|]/\\&/g')
    sed -e "s|^export const str CBRT_STAMP = \"\";|export const str CBRT_STAMP = \"$s\";|" \
        -e "s|^export const str CBRT_DIR = \"\";|export const str CBRT_DIR = \"$esc\";|" \
        "$out.build_info.orig" > "$info"
    # One C file, no link-time optimisation, as boot.c builds the compiler
    # from the seed: the default of a compiler from Update 209 on, which has
    # no `--one-unit`; an older one is asked for it (and one from before the
    # option builds in parts; the next build, by the compiler this one
    # makes, does not).
    local one=()
    if "$with" --help 2> /dev/null | grep -q -- '--one-unit'; then
        one=(--one-unit)
    fi
    "$with" "${cc[@]}" "${one[@]}" -O "$level" -o "$out.new" "$impl/cobc/main.cb"
    mv -f "$out.build_info.orig" "$info"
    trap - EXIT
    mv -f "$out.new" "$out"
    echo "$s" > "$out.stamp"
}

programs=" ${PROGRAMS:-cobc cobfmt cobmodel cbparse cbtest difftest} "
case $programs in
*" cobc "*)
    # A cobc boot made from the seed has no stamp (it accepts any runtime):
    # no cobc.stamp, or an empty one; one stage then.
    old=$(cat "$prev/cobc.stamp" 2>/dev/null || true)
    if [ -z "${COBC:-}" ] && [ -n "$old" ] && [ "$old" != "$want" ]; then
        if [ ! -f "$prev/libcbrt.a" ] || [ "$(stamp_of "$prev/libcbrt.a")" != "$old" ]; then
            echo "impl/build.sh: the runtime changed and the one $prev/cobc was built with is not kept;" >&2
            echo "  build again from the seed: gcc impl/bootstrap/boot.c -o boot && ./boot" >&2
            exit 1
        fi
        echo "impl/build.sh: the runtime changed: stage 1 with $cobc and the runtime it was built with"
        mkdir -p "$impl/target/stage1"
        COBALTC_CBRT_DIR=$prev build_cobc "$cobc" "$impl/target/stage1/cobc" "$rel" 1
        cobc=$impl/target/stage1/cobc
    fi
    echo "impl/build.sh: building cobc with $cobc"
    # The copy in target/prev finds no runtime beside it; the one in release is
    # the one it accepts (a cobc boot made, with no stamp, accepts any).
    COBALTC_CBRT_DIR=${COBALTC_CBRT_DIR:-$rel} build_cobc "$cobc" "$rel/cobc" "$rel" 2
    cobc=$rel/cobc
    ;;
esac
# The other programs, JOBS at a time.
jobs=${JOBS:-2}
running=0
bad=0
tool() {
    local name=$1 out=$2 src=$3
    case $programs in *" $name "*) ;; *) return 0 ;; esac
    if [ "$running" -ge "$jobs" ]; then
        wait -n || bad=1
        running=$((running - 1))
    fi
    ("$cobc" "${cc[@]}" -O 2 -o "$out" "$src" && echo "impl/build.sh: built $name" ||
        { echo "impl/build.sh: $name did not build" >&2; exit 1; }) &
    running=$((running + 1))
}
# The longest first.
tool cbparse "$rel/cbparse" "$impl/cbparse/main.cb"
tool cobmodel "$rel/cobmodel" "$impl/model/src/main.cb"
tool cobfmt "$rel/cobfmt" "$impl/cobfmt/main.cb"
tool cbtest "$tests/cbtest" "$impl/tests/cbtest/main.cb"
tool difftest "$tests/difftest" "$impl/tests/differential/main.cb"
while [ "$running" -gt 0 ]; do
    wait -n || bad=1
    running=$((running - 1))
done
exit $bad
