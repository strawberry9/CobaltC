#!/usr/bin/env bash
# Builds the programs written in CobaltC with cobc: cobfmt-cb (cobfmt),
# cbparse (coby's parser) and cbc (the code generator), into
# impl/target/selfhost/. CC picks the C
# compiler (cobc --cc); COBC another cobc; OUT another directory.
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
"$cobc" "${cc[@]}" -O 2 -o "$out/cbc" "$here/cbc/main.cb"
