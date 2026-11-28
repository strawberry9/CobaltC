#!/usr/bin/env bash
# The runtime and the programs instrumented together (D-0326 phase B): the
# runtime built with AddressSanitizer and UBSan (asan) or ThreadSanitizer
# (tsan), every compiled conformance case built by `cobc --cc` a Clang that
# instruments the program the same way, linked against that runtime
# (COBALTC_CBRT_DIR), and run by the conformance runner. A sanitizer report
# fails its case (the runner compares standard error).
#
#   impl/cbrt/tests/sanitize.sh asan|tsan [--only TEXT]
#
# COBC names the compiler (default impl/target/release/cobc). Address space
# layout randomisation is turned off for the runs (`setarch -R`): clang 14's
# sanitizers fail to start under the 6.8 kernel's layout otherwise.
set -eu
san=${1:?asan or tsan}
shift
impl=$(cd "$(dirname "$0")/../.." && pwd)
cobc=${COBC:-$impl/target/release/cobc}
work=$(mktemp -d "${TMPDIR:-/tmp}/cbrt-$san-XXXXXX")
trap 'rm -rf "$work"' EXIT
case "$san" in
asan) flags="-fsanitize=address,undefined -fno-sanitize-recover=undefined -fno-omit-frame-pointer -g" ;;
tsan) flags="-fsanitize=thread -g" ;;
*) echo "sanitize.sh: asan or tsan" >&2; exit 2 ;;
esac
CC=clang "$impl/cbrt/build.sh" --cc clang --san "$san" --out "$work/rt" > "$work/build.log" 2>&1 || { cat "$work/build.log"; exit 1; }
cat > "$work/clang-$san" <<EOF
#!/bin/sh
exec clang $flags "\$@"
EOF
chmod +x "$work/clang-$san"
cat > "$work/run-case" <<EOF
#!/bin/sh
exec setarch "\$(uname -m)" -R "$cobc" --cc "$work/clang-$san" --run "\$@"
EOF
chmod +x "$work/run-case"
export COBALTC_CBRT_DIR=$work/rt
export ASAN_OPTIONS=detect_leaks=0:abort_on_error=0
export TSAN_OPTIONS=halt_on_error=1
cd "$impl/conformance"
python3 run.py --jobs 2 --timeout 900 --impl "$work/run-case {file}" "$@"
