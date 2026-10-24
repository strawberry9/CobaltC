#!/usr/bin/env bash
# The targeted suites behind the "tested" evidence tier (D-0212, CHG-0256):
# both conformance suites under both implementations, the compiled suite
# under --all-checks with GCC, and cobc's other test targets -- each to its
# own log -- then, optionally, `cobc --explore N` over given thread cases.
# Writes TESTED into its run directory: the run, its result, the tree
# fingerprint it tested (impl/tools/manifest.py --fingerprint), the
# toolchain, each target's `test result` lines, and the exploration bound
# when every explored case gave one outcome. impl/tools/manifest.py reads
# the newest TESTED whose fingerprint is the tree's.
#
# Usage, from anywhere:
#   impl/tools/suites.sh
#   EXPLORE=40 EXPLORE_CASES="impl/conformance/19-concurrency/a.cb …" impl/tools/suites.sh
#   OUT=dir impl/tools/suites.sh    # logs there (default: <home>/runs/<UTC stamp>)
# <home> is stress/suites beside impl/ when stress/ exists, else ~/.cobaltc-suites.
# Builds first: cargo build --release -p coby -p cbrt -p cobc.
set -u
R="$(cd "$(dirname "$0")/../.." && pwd)"
if [ -d "$R/stress" ]; then H="$R/stress/suites"; else H="$HOME/.cobaltc-suites"; fi
STAMP=$(date -u +%Y%m%d-%H%M%S)
D="${OUT:-$H/runs/$STAMP}"
mkdir -p "$D"
# A runaway test is stopped: CPU per process, and each target's wall clock.
ulimit -t 1800
export COBALTC_CPU_LIMIT=120
FAILED=""
run() {
    local name=$1; shift
    echo "== $name start $(date -u +%H:%M:%S)" >> "$D/progress"
    (cd "$R/impl" && timeout -k 60 9000 "$@") > "$D/$name.log" 2>&1
    local rc=$?
    echo "== $name exit $rc $(date -u +%H:%M:%S)" >> "$D/progress"
    if [ $rc -ne 0 ] || /usr/bin/env grep -aqE "^test result: FAILED|panicked at" "$D/$name.log"; then
        FAILED="$FAILED $name"
    fi
}
cd "$R/impl" && cargo build --release -p coby -p cbrt -p cobc -p cobmodel > "$D/build.log" 2>&1 || FAILED="$FAILED build"
if [ -z "$FAILED" ]; then
    run coby_suites cargo test --release -p coby --no-fail-fast --test cb_conformance_suite --test spec_rows --test conformance --test conf_coverage
    run cobc_spec_rows cargo test --release -p cobc --test compiled_spec_rows
    run cobc_suite cargo test --release -p cobc --test compiled_suite
    run cobc_suite_allchecks env COBALTC_ALL_CHECKS=1 COBC_TEST_CCS=gcc cargo test --release -p cobc --test compiled_suite
    run cobc_other cargo test --release -p cobc --no-fail-fast --test differential --test extern_code --test tracking --test runtime_bookkeeping --test output_order --test read_input --test driver
    # The model sample (D-0219): every outcome the tools show is one the
    # model's rules allow.
    if [ -d "$R/impl/model/sample" ]; then
        run model_sample "$R/impl/target/release/cobmodel" compare "$R/impl/model/sample" --coby "$R/impl/target/release/coby" \
            --cobc "$R/impl/target/release/cobc" --explore 8 --cc gcc --cc clang --jobs 2 --report "$D/model-sample.txt"
        run model_licensed "$R/impl/target/release/cobmodel" check --as-licensed $R/impl/model/sample/*.cb
    fi
fi
EXPLORED=""
if [ -n "${EXPLORE:-}" ] && [ -n "${EXPLORE_CASES:-}" ] && [ -z "$FAILED" ]; then
    EXPLORED=$EXPLORE
    mkdir -p "$D/explore"
    for c in $EXPLORE_CASES; do
        f="$c"; [ "${f#/}" = "$f" ] && f="$R/$c"
        out=$(cd "$D/explore" && COBALTC_CPU_LIMIT=600 timeout -k 10 900 "$R/impl/target/release/cobc" --explore "$EXPLORE" --cc gcc "$f" </dev/null 2>&1)
        echo "=== $c" >> "$D/explore.log"; echo "$out" | tail -8 >> "$D/explore.log"
        # A case the static pass rejects compiles to nothing to explore: one outcome.
        if echo "$out" | /usr/bin/env grep -q "distinct outcomes"; then EXPLORED=""; FAILED="$FAILED explore:$(basename "$c")"; fi
    done
fi
{
    echo "run $STAMP"
    if [ -z "$FAILED" ]; then echo "result PASSED"; else echo "result FAILED:$FAILED"; fi
    echo "fingerprint $(python3 "$R/impl/tools/manifest.py" --fingerprint 2>/dev/null || echo unknown)"
    [ -n "$EXPLORED" ] && echo "explored $EXPLORED"
    if [ -s "$D/model-sample.txt" ] && /usr/bin/env grep -q ' 0 with an outcome outside' "$D/model-sample.txt"; then
        echo "modelled $(/usr/bin/env grep -o 'compare: [0-9]* programs' "$D/model-sample.txt" | /usr/bin/env grep -o '[0-9]*')"
    fi
    echo "rustc $(rustc -V 2>/dev/null || echo unknown)"
    echo "gcc $(gcc --version 2>/dev/null | head -n 1 || echo unknown)"
    echo "clang $(clang --version 2>/dev/null | head -n 1 || echo unknown)"
    echo "kernel $(uname -sr 2>/dev/null || echo unknown)"
    for f in "$D"/*.log; do
        n=$(basename "$f" .log)
        [ "$n" = build ] || [ "$n" = explore ] && continue
        /usr/bin/env grep -aE "^test result:" "$f" | sed "s/^/target-$n /"
    done
} > "$D/TESTED"
touch "$D/SUITES_DONE"
cat "$D/TESTED"
[ -z "$FAILED" ]
