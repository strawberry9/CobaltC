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
stamp=$(LC_ALL=C grep -aoE 'CBRT-STAMP:[0-9a-f]{16}' "$cbrt_dir/libcbrt.a" | head -n 1 | cut -d: -f2 || true)
python3 "$here/cbc/gen_build_info.py" --stamp "$stamp" --cbrt-dir "$cbrt_dir"
trap 'python3 "$here/cbc/gen_build_info.py"' EXIT
"$cobc" "${cc[@]}" -O 2 -o "$out/cbc" "$here/cbc/main.cb"
