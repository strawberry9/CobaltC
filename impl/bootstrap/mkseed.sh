#!/usr/bin/env bash
# Writes the bootstrap seed (D-0326): impl/bootstrap/cobc-seed.c.gz, the C of
# the compiler's own source in this tree (impl/cobc/main.cb) without
# its `#line` directives, and impl/bootstrap/SEED, its length and CRC-32,
# which impl/bootstrap/boot.c checks before it builds the compiler from it.
#
#   impl/bootstrap/mkseed.sh            # write both
#   impl/bootstrap/mkseed.sh --check    # exit 1 unless the committed seed is
#                                       # the C of the committed source
#
# COBC names the compiler that writes the C (default: impl/target/release/cobc,
# else impl/target/release/cobc; every build of the compiler writes the same C
# for the same source, which the gate checks). The C is written in the
# compiler's own directory, as boot.c's proof writes it. A development tool:
# it needs bash, gzip and Python 3; boot.c needs none of them.
set -eu
here=$(cd "$(dirname "$0")" && pwd)
impl=$(cd "$here/.." && pwd)
if [ -n "${COBC:-}" ]; then
    cobc=$COBC
elif [ -x "$impl/target/release/cobc" ]; then
    cobc=$impl/target/release/cobc
else
    cobc=$impl/target/release/cobc
fi
info=$impl/cobc/build_info.cb
# The seed is the neutral build: no runtime stamp, the runtime beside the
# executable (impl/build.sh writes both in only while it builds).
grep -q '^export const str CBRT_STAMP = "";' "$info" && grep -q '^export const str CBRT_DIR = "";' "$info" ||
    { echo "mkseed.sh: $info is not in its committed form (is a build running?)" >&2; exit 1; }
tmp=$(mktemp -d)
trap 'rm -rf "$tmp"' EXIT
(cd "$impl/cobc" && "$cobc" --emit-c --c-name cobc.c -o "$tmp/full.c" main.cb)
grep -v '^#line ' "$tmp/full.c" > "$tmp/cobc.c"
python3 -I - "$tmp/cobc.c" "$tmp/SEED" <<'EOF'
import sys, zlib
data = open(sys.argv[1], "rb").read()
with open(sys.argv[2], "w") as f:
    f.write("# The bootstrap seed, impl/bootstrap/cobc-seed.c.gz (D-0326): written by\n")
    f.write("# impl/bootstrap/mkseed.sh; checked by impl/bootstrap/boot.c. Do not edit.\n")
    f.write("length %d\n" % len(data))
    f.write("crc32 %08x\n" % (zlib.crc32(data) & 0xffffffff))
EOF
if [ "${1:-}" = "--check" ]; then
    if cmp -s "$tmp/SEED" "$here/SEED" && [ -f "$here/cobc-seed.c.gz" ] && gzip -dc "$here/cobc-seed.c.gz" | cmp -s - "$tmp/cobc.c"; then
        echo "seed current ($(sed -n 's/^length //p' "$here/SEED") bytes)"
        exit 0
    fi
    echo "mkseed.sh: impl/bootstrap/cobc-seed.c.gz is not the C of this tree's compiler (run impl/bootstrap/mkseed.sh)" >&2
    exit 1
fi
gzip -9n -c "$tmp/cobc.c" > "$here/cobc-seed.c.gz"
cp "$tmp/SEED" "$here/SEED"
echo "mkseed.sh: wrote impl/bootstrap/cobc-seed.c.gz ($(wc -c < "$here/cobc-seed.c.gz") bytes) and SEED ($(sed -n 's/^length //p' "$here/SEED") bytes of C)"
