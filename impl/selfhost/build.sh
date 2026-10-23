#!/usr/bin/env bash
# Builds the programs written in CobaltC with cobc: cobfmt-cb (cobfmt),
# cbparse (coby's parser) and cbc (the code generator and cobc's driver),
# into impl/target/selfhost/. CC picks the C compiler (cobc --cc); COBC
# another cobc; OUT another directory.
#
# cbc's build-time constants (cbc/build_info.cb, as cobc's build.rs gives
# cobc its own): the stamp of the runtime beside the cobc that builds it
# (`CBRT_STAMP`, read from that `libcbrt.a`) and that runtime's directory
# (`CBRT_DIR`), written in for the build and taken out again after it, so
# the committed file keeps its neutral form (no stamp, the runtime beside
# the executable).
set -eu
here=$(cd "$(dirname "$0")" && pwd)
impl=$(cd "$here/.." && pwd)
cobc=${COBC:-$impl/target/release/cobc}
out=${OUT:-$impl/target/selfhost}
mkdir -p "$out"
cc=()
[ -n "${CC:-}" ] && cc=(--cc "$CC")
"$cobc" "${cc[@]}" -O 2 -o "$out/cobfmt-cb" "$here/cobfmt/main.cb"
"$cobc" "${cc[@]}" -O 2 -o "$out/cbparse" "$here/cbparse/main.cb"
cbrt_dir=$(cd "$(dirname "$cobc")" && pwd)
# Under MSYS2 `pwd` gives /e/..., which a native cbc cannot open: take the
# E:/... form.
if command -v cygpath > /dev/null 2>&1; then
    cbrt_dir=$(cygpath -m "$cbrt_dir")
fi
stamp=$(LC_ALL=C grep -aoE 'CBRT-STAMP:[0-9a-f]{16}' "$cbrt_dir/libcbrt.a" | head -n 1 | cut -d: -f2 || true)
# The two constants are written with sed rather than gen_build_info.py, so
# the build needs no Python; the committed file is put back on exit.
info=$here/cbc/build_info.cb
cp "$info" "$out/build_info.cb.orig"
trap 'mv -f "$out/build_info.cb.orig" "$info"' EXIT
# A CobaltC literal escapes \ and "; the sed replacement then \, & and |.
escaped_dir=$(printf '%s' "$cbrt_dir" | sed -e 's/[\\"]/\\&/g' | sed -e 's/[\\&|]/\\&/g')
sed -e "s|^export const str CBRT_STAMP = \"\";|export const str CBRT_STAMP = \"$stamp\";|" \
    -e "s|^export const str CBRT_DIR = \"\";|export const str CBRT_DIR = \"$escaped_dir\";|" \
    "$out/build_info.cb.orig" > "$info"
grep -q "^export const str CBRT_DIR = \"$(printf '%s' "$cbrt_dir" | head -c 1)" "$info" ||
    { echo "build.sh: could not write CBRT_DIR into $info" >&2; exit 1; }
"$cobc" "${cc[@]}" -O 2 -o "$out/cbc" "$here/cbc/main.cb"
