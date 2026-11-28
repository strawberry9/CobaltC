#!/usr/bin/env bash
# The former battery's stages (D-0307: impl/gate.sh replaced the battery).
# The gate's release profile runs stages 5 (programs) and 6 (external)
# from here, with STAGES=5 or STAGES=6; the other stages are the gate's
# units now and are kept only so that this script stays whole.
#
# The battery: every check the project runs before a push, as stages in
# cost order, each gating the next. A failing stage stops the battery and
# writes the report; inside a stage every target runs (--no-fail-fast),
# so a stage's report is complete.
#
#   0 gate       preflight, the build (impl/build.sh), the seed and the runtime's interface
#                current, the Windows runtime, formatting, the `--check` sweep, cbtest's cases
#   1 hand       the conformance suite under COBALTC_VERIFY, the compiler against itself on
#                random programs, the model sample (impl/model, D-0219), std's tests; the
#                showcase runner, the guide check and the book check side by side
#   2 corpus     the conformance suite and the specification's rows (cbtest, against the
#                recorded runs), SHARDS processes each, and the tools' own program cases
#   3 checks     the conformance suite with every check kept
#   4 runtime    CBMC on the runtime's tables; ASan and UBSan, then ThreadSanitizer, over the suite
#   5 programs   the local stress programs, sanitizer runs, crypto sweep, local
#                TLS (needs the stress/ directory); the std fuzzer under
#                ASan/UBSan and the interop checks (impl/fuzz, CHG-0266)
#   6 external   public DoT resolvers, public HTTP sites, DoH (advisory; stress/)
#
# Usage, from anywhere:
#   impl/battery.sh                 # scope from the files changed since the last green battery
#   SCOPE=full impl/battery.sh      # every stage (and COBALTC_TEST_SLOW=1: `slow:` cases too)
#   STAGES="0 2 3" impl/battery.sh  # exactly these stages (manual; records no green tree)
#   FROM=3 impl/battery.sh          # resume: skip the stages below 3 (after a test-data fix)
#   SHARDS=2 impl/battery.sh        # processes per interpreter runner in stage 2 (default 2)
#   MAXLOAD=2 impl/battery.sh       # allow a busier machine (default 1.0)
#   OUT=dir impl/battery.sh         # logs there (default: <home>/runs/<UTC stamp>)
# <home> is stress/battery_staged beside impl/ when stress/ exists (this
# project's machine), else ~/.cobaltc-battery; last_green there holds the
# hash of the last tree every selected stage passed on.
# Output: $OUT/progress (one line per stage), $OUT/stage*.log, $OUT/REPORT.md, $OUT/SUMMARY,
# and the last progress line, "BATTERY DONE" or "BATTERY STOPPED at stage N".
set -u
H="$(cd "$(dirname "$0")" && pwd)"
R="$(cd "$H/../.." && pwd)"
S="$R/stress"
if [ -d "$S/d0166" ]; then HAVE_STRESS=1; BASE="$S/battery_staged"; else HAVE_STRESS=""; BASE="$HOME/.cobaltc-battery"; fi
STAMP="$(date -u +%Y%m%d_%H%M)"
D="${OUT:-$BASE/runs/$STAMP}"; mkdir -p "$D"; D=$(cd "$D" && pwd)   # absolute: stages cd elsewhere
P="$D/progress"
LAST_GREEN="$BASE/last_green"
SCOPE="${SCOPE:-auto}"
STAGES="${STAGES:-}"
FROM="${FROM:-0}"
SHARDS="${SHARDS:-2}"
MAXLOAD="${MAXLOAD:-1.0}"
export COBALTC_CPU_LIMIT=900
ulimit -t 36000
NAMES=(gate hand corpus checks runtime programs external)
FAILS=0; STOPPED=""; PREFAIL=""; ADVISORY=""; FLAKES=""; S1_PARTS="tests showcase guide book"; SEL=""; SCOPE_DESC=""; FULL=""; SCOPED_BASE=""
JOBS=(); JOBLOGS=()

say() { echo "$(date -u '+%H:%M:%S') $*" | tee -a "$P"; }

# ------------------------------------------------------------ helpers
# step NAME CMD...: runs CMD, brackets its output, counts a failure.
step() {
    local name="$1"; shift
    echo "== $name"
    "$@"; local st=$?
    echo "[exit $st]"
    [ $st -eq 0 ] || FAILS=$((FAILS + 1))
    return $st
}

# The working tree's hash (tracked and untracked files, .gitignore
# honoured), from a temporary index: the real index is never touched.
tree_hash() {
    local idx="$D/tmp-index"
    cp "$R/.git/index" "$idx" 2>/dev/null || : > "$idx"
    (cd "$R" && GIT_INDEX_FILE="$idx" git add -A . >/dev/null 2>&1 && GIT_INDEX_FILE="$idx" git write-tree)
    rm -f "$idx"
}

# start_job LABEL CMD...: runs CMD in the background into its own log.
start_job() {
    local label="$1"; shift
    local log="$D/job-$label.log"
    ( "$@" > "$log" 2>&1; echo "[exit $?]" >> "$log" ) &
    JOBS+=($!); JOBLOGS+=("$log")
}

# Waits for every job, folds the logs into the stage log, counts failures.
wait_jobs() {
    [ ${#JOBS[@]} -gt 0 ] && wait "${JOBS[@]}"
    local l st
    for l in "${JOBLOGS[@]}"; do
        echo "== $(basename "$l" .log)"
        /usr/bin/grep -a -E 'rows checked|cases \(|passed, |slowest:|^ +[0-9]+\.[0-9]+ s |showcases identical|examples, |^\[exit' "$l" | tail -n 30
        st=$(tail -n 1 "$l" | sed 's/^\[exit \(.*\)\]$/\1/')
        if [ "$st" != 0 ]; then
            FAILS=$((FAILS + 1))
            echo "--- failures (from $(basename "$l")):"
            /usr/bin/grep -a -A60 '^failures:$' "$l" | head -n 120
            /usr/bin/grep -a -E '^FAIL |^DIFF|^WRONG' "$l" | head -n 40
        fi
    done
    JOBS=(); JOBLOGS=()
}

# require LOG PATTERN MIN: at least MIN lines match (a sign of success).
require() {
    local n; n=$(/usr/bin/grep -a -c -E "$2" "$1")
    if [ "$n" -lt "$3" ]; then echo "REQUIRE FAILED: '$2' seen $n times, need $3 ($(basename "$1"))"; FAILS=$((FAILS + 1)); fi
}
# forbid LOG PATTERN: no line matches (a sign of failure).
forbid() {
    local bad; bad=$(/usr/bin/grep -a -n -E "$2" "$1")
    if [ -n "$bad" ]; then echo "FORBID FAILED: '$2' ($(basename "$1")):"; echo "$bad" | head -n 20; FAILS=$((FAILS + 1)); fi
}
# every `[exit N]` line in LOG is `[exit 0]`.
exits_zero() {
    local bad; bad=$(/usr/bin/grep -a -n -E '^\[exit [0-9]+\]' "$1" | /usr/bin/grep -v 'exit 0\]')
    if [ -n "$bad" ]; then echo "non-zero exits ($(basename "$1")):"; echo "$bad"; FAILS=$((FAILS + 1)); fi
}

has_part() { case " $S1_PARTS " in *" $1 "*) return 0 ;; *) return 1 ;; esac; }

# ------------------------------------------------------------ scope
# Chooses SEL (the stages) and S1_PARTS from the files changed since the
# last green battery. A core change (the toolchain, std, the tests, the
# spec's tables, the registry) selects everything; a change elsewhere
# selects the stages that read it.
classify() {
    local cur="$1" base files f
    declare -A w=()
    if [ -n "$STAGES" ]; then SEL="$STAGES"; SCOPE_DESC="manual STAGES=$STAGES"; return; fi
    if [ "$SCOPE" = full ] || [ ! -s "$LAST_GREEN" ]; then
        SEL="0 1 2 3 4 5 6"; FULL=1; SCOPE_DESC="full ($([ -s "$LAST_GREEN" ] && echo requested || echo 'no last green battery'))"; return
    fi
    base=$(head -n 1 "$LAST_GREEN")
    files=$(cd "$R" && git diff --name-only "$base" "$cur" 2>/dev/null)
    if [ -z "$files" ]; then SEL="0 1 2 3 4 5 6"; FULL=1; SCOPE_DESC="full (tree unchanged since last green battery $base)"; return; fi
    local parts=""
    while IFS= read -r f; do
        case "$f" in
            impl/front/*|impl/back/*|impl/cobc/*|impl/cbrt/*|impl/std/*|impl/tests/*|impl/bootstrap/*|impl/build.sh|spec/conformance.md|spec/examples.md|spec/registry/*)
                w[all]=1 ;;
            impl/battery.sh)                 w[0]=1 ;;
            impl/conformance/*)              w[0]=1; w[2]=1; w[3]=1 ;;
            showcase/*|bench/*|impl/cobaltc_examples/*) w[0]=1; w[1]=1; parts="$parts tests showcase" ;;
            book/*)                          w[0]=1; w[1]=1; parts="$parts book" ;;
            .github/*)                       w[0]=1 ;;
            impl/cobfmt/*|impl/cbparse/*)    w[0]=1 ;;
            impl/model/*)                    w[0]=1; w[1]=1; parts="$parts tests" ;;
            impl/fuzz/*)                     w[0]=1; w[5]=1 ;;
            htmlguide/*|spec/*|README.md|images/*|*.md|LICENSE*) w[0]=1; w[1]=1; parts="$parts guide" ;;
            *)                               w[all]=1 ;;
        esac
    done <<< "$files"
    if [ -n "${w[all]:-}" ]; then
        SEL="0 1 2 3 4 5 6"; FULL=1; SCOPE_DESC="full (a core file changed)"; S1_PARTS="tests showcase guide book"
    else
        SEL=""; for f in 0 1 2 3 4 5 6; do [ -n "${w[$f]:-}" ] && SEL="$SEL $f"; done; SEL="${SEL# }"
        S1_PARTS=$(echo "$parts" | tr ' ' '\n' | /usr/bin/grep -v '^$' | sort -u | tr '\n' ' '); S1_PARTS="${S1_PARTS% }"
        SCOPE_DESC="scoped: stages $SEL (stage 1 parts: ${S1_PARTS:-none})"
        SCOPED_BASE="$base"
    fi
    { echo "files changed since last green battery ($base):"; echo "$files"; } > "$D/scope.txt"
}

# ------------------------------------------------------------ preflight
# The battery owns the machine: nothing of ours running, load low enough
# that the times mean something and the perf assertions hold.
preflight() {
    echo "== preflight"
    local stray load
    stray=$(ps -eo pid,etimes,args | /usr/bin/grep -E 'cobc-[0-9]+-[0-9]+/a\.out|target/(release|tests)/(cobc|cobfmt|cobmodel|cbtest|difftest)( |$)' | /usr/bin/grep -v grep)
    if [ -n "$stray" ]; then echo "stray processes (kill them first):"; echo "$stray"; echo "[exit 1]"; return 1; fi
    load=$(cut -d' ' -f1 /proc/loadavg)
    if awk -v l="$load" -v m="$MAXLOAD" 'BEGIN { exit !(l > m) }'; then
        echo "load average $load is above $MAXLOAD: the machine is busy (MAXLOAD=n to override)"; echo "[exit 1]"; return 1
    fi
    echo "load $load, commit $(git -C "$R" rev-parse --short HEAD), tree $TREE"
    echo "[exit 0]"
}

# ------------------------------------------------------------ stages
stage0() {
    FAILS=0
    cd "$R" || return 1
    local B="$R/impl/target/release" T="$R/impl/target/tests"
    # D-0326: the toolchain from the compiler installed (impl/build.sh), the seed current, the runtime's
    # table and interface, the Windows runtime, formatting and the `--check` sweep.
    step "build (impl/build.sh)" timeout -k 30 3600 impl/build.sh
    step "seed current" bash -c "COBC='$B/cobc' timeout -k 30 1800 impl/bootstrap/mkseed.sh --check"
    step "prelude summary current" bash -c "'$B/cobc' --prelude-summary impl/std | cmp - impl/front/prelude.cb"
    step "runtime interface" python3 -I impl/cbrt/tools/sigcheck.py "$B/libcbrt.a"
    step "windows runtime" bash -c "impl/cbrt/build.sh --cc x86_64-w64-mingw32-gcc --out '$D/cbrt-win' 2>&1 | tee '$D/cbrt-win.log' | tail -1; ! grep -q 'warning:' '$D/cbrt-win.log'"
    local o bad=""
    o=$(for d in showcase impl/cobaltc_examples bench impl/std book/examples impl/front impl/back impl/cobc impl/cobfmt impl/model; do "$B/cobfmt" --check "$d" 2>&1 || bad=1; done)
    echo "== cobfmt --check"
    if [ -n "$o" ] || [ -n "$bad" ]; then echo "files cobfmt would change, or errors:"; echo "$o"; echo "[exit 1]"; FAILS=$((FAILS + 1)); else echo "[exit 0]"; fi
    local sweep="showcase impl/cobaltc_examples bench"
    [ -d "$S/perfguide" ] && sweep="$sweep stress/perfguide"
    step "cobc --check sweep" bash -c "cd '$R' && timeout -k 10 1800 '$B/cobc' --check $sweep"
    step "cases (cbtest)" timeout -k 30 7200 "$T/cbtest" cases --root impl/tests/cases --cobc "$B/cobc"
    step "formatter tests" timeout -k 30 1800 python3 -I "$R/impl/cobfmt/tests.py"
    [ $FAILS -eq 0 ]
}

stage1() {
    FAILS=0
    cd "$R" || return 1
    local B="$R/impl/target/release" T="$R/impl/target/tests"
    step "conformance under COBALTC_VERIFY (cobc)" bash -c "cd '$R/impl/conformance' && COBALTC_VERIFY=\${VERIFY_STRIDE:-1} timeout -k 10 5400 python3 run.py --impl '$B/cobc --run {file}' --jobs 3"
    step "the compiler against itself on random programs" bash -c "mkdir -p '$D/diff/tmp' && TMPDIR='$D/diff/tmp' timeout -k 30 7200 '$T/difftest' --self --cobc '$B/cobc' --n \${DIFF_N:-500} --seed \${DIFF_SEED:-7} --jobs 3 --work '$D/diff/work'"
    # The model (D-0219): every program of impl/model/sample under cobc with each C compiler and --explore,
    # each outcome one the model gives.
    step "model sample" bash -c "timeout -k 30 5400 '$B/cobmodel' compare '$R/impl/model/sample' --cobc '$B/cobc' --explore 8 --cc gcc --cc clang --jobs 2 --report '$D/model-sample.txt' && grep -q ' 0 with an outcome outside' '$D/model-sample.txt'"
    local dir="$D/std-tests"
    mkdir -p "$dir"; printf 'fn main()\n{\n}\n' > "$dir/std-tests.cb"
    for cc in gcc clang; do
        step "std tests (cobc --unittest-std, $cc)" bash -c "cd '$dir' && timeout -k 10 1800 '$B/cobc' --unittest-std --cc $cc --run std-tests.cb < /dev/null"
    done
    start_job showcase bash -c "cd '$R/showcase' && LIMIT=900 ./run_all.sh"
    start_job guide bash -c "cd '$R/htmlguide/generator' && python3 gen.py --check"
    start_job book bash -c "cd '$R/book' && ./check.sh"
    wait_jobs
    [ $FAILS -eq 0 ]
}

stage2() {
    FAILS=0
    cd "$R" || return 1
    local B="$R/impl/target/release" T="$R/impl/target/tests"
    local slow=""
    [ -n "${COBALTC_TEST_SLOW:-}" ] && slow="--slow"
    for i in $(seq 1 "$SHARDS"); do
        start_job "suite-$i" "$T/cbtest" conformance $slow --cobc "$B/cobc" --shard "$i/$SHARDS"
        start_job "rows-$i" "$T/cbtest" rows $slow --cobc "$B/cobc" --shard "$i/$SHARDS"
    done
    wait_jobs
    step "the tools' own program cases" "$T/cbtest" conformance --cobc "$B/cobc" --root impl/tests/programs
    [ $FAILS -eq 0 ]
}

stage3() {
    FAILS=0
    cd "$R" || return 1
    local B="$R/impl/target/release" T="$R/impl/target/tests"
    # Every check kept (`--all-checks`): the decided and the elided paths against the recorded runs.
    step "conformance, every check (gcc)" env COBALTC_ALL_CHECKS=1 "$T/cbtest" conformance --cobc "$B/cobc" --cc gcc
    [ $FAILS -eq 0 ]
}

stage4() {
    FAILS=0
    cd "$R" || return 1
    # The runtime and the program instrumented together (D-0326), and the runtime's bounded proofs.
    step "runtime bounded proofs (CBMC)" bash -c "cd '$R/impl/cbrt/tests/cbmc' && timeout -k 30 3600 ./run.sh"
    step "ASan and UBSan, conformance" timeout -k 30 7200 impl/cbrt/tests/sanitize.sh asan
    step "ThreadSanitizer, conformance" timeout -k 30 7200 impl/cbrt/tests/sanitize.sh tsan
    [ $FAILS -eq 0 ]
}

# Stages 5 and 6 run this machine's local programs under stress/.
stage5() {
    FAILS=0
    local L
    L="$D/stress.log"
    echo "== stress programs"
    {
        (cd "$S/datetime" && ../run.sh t1.cb)
        mkdir -p "$S/paths/scratch"; (cd "$S/paths" && ARGS="$S/paths/scratch" ../run.sh p1.cb)
        (cd "$S/random" && ../run.sh r1.cb)
        (cd "$S/process" && ../run.sh pr1.cb; ../run.sh intr.cb)
        (cd "$S/net" && ../run.sh n1.cb; ../run.sh echo50.cb; ../run.sh shutdown.cb)
        (cd "$S/net" && for t in "cobc --cc gcc --run" "cobc --cc clang --run"; do timeout -k 5 300 $R/impl/target/release/$t bulk.cb 1 2>&1 | /usr/bin/grep received; done | sort | uniq -c | awk 'END { print (NR == 1 && $1 == 2 ? "ok  bulk 1 MiB, both agree" : "DIFF  bulk 1 MiB") }')
        (cd "$S/net" && for cc in gcc clang; do timeout -k 5 300 $R/impl/target/release/cobc --cc $cc --run bulk.cb 16 2>&1 | tail -n 2 | tr '\n' ' '; echo "($cc, 16 MiB)"; done)
    } > "$L" 2>&1; cat "$L"
    require "$L" '^ok ' 9
    require "$L" 'received 16777216 bytes of 16777216' 2
    forbid  "$L" 'DIFF|WRONG|Traceback'

    L="$D/san.log"
    echo "== sanitizer runs"
    {
        for p in net/n1.cb net/echo50.cb net/shutdown.cb process/pr1.cb process/intr.cb d0166/t_hmac.cb d0166/t_ed.cb; do
            d=$(dirname "$S/$p"); b=$(basename "$p")
            echo "== $p"; (cd "$d" && setarch "$(uname -m)" -R timeout -k 10 900 "$R/impl/target/release/cobc" --cc "$S/round5/san-clang" --run "$b" 2>&1 | tail -4; echo "[exit ${PIPESTATUS[0]}]")
        done
        echo "== d0166/t_kdf.cb"; (cd "$S/d0166" && setarch "$(uname -m)" -R timeout -k 10 900 "$R/impl/target/release/cobc" --cc "$S/round5/san-clang" --run t_kdf.cb < t_kdf.stdin 2>&1 | python3 t_kdf_check.py /dev/stdin | tail -2)
        echo "== d0166/t_rsa.cb"; (cd "$S/d0166" && setarch "$(uname -m)" -R timeout -k 10 900 "$R/impl/target/release/cobc" --cc "$S/round5/san-clang" --run t_rsa.cb gen < t_rsa.stdin 2>/dev/null > t_rsa.san.txt; python3 t_rsa_check.py t_rsa.san.txt | tail -2)
    } > "$L" 2>&1; cat "$L"
    exits_zero "$L"
    require "$L" '^ALL OK' 2
    forbid  "$L" 'Sanitizer|runtime error|Traceback|FAIL|MISMATCH'

    L="$D/crypto_sweep.log"
    echo "== crypto sweep"
    (cd "$S/d0166" && ./cobc_sweep.sh) > "$L" 2>&1; cat "$L"
    require "$L" '^\[sweep done\]' 1
    require "$L" 'ALL OK' 8
    forbid  "$L" 'FAIL|DIFF|MISMATCH|Traceback|error'

    L="$D/local_tls.log"
    echo "== local TLS"
    (cd "$S/d0166" && ./local_tls.sh) > "$L" 2>&1; cat "$L"
    require "$L" '^reply: etius olleh' 8
    require "$L" '^\[local done\]' 1
    forbid  "$L" 'error|Error|Traceback'

    # The std fuzzer and the interop checks (WI-7, CHG-0266): every parser
    # for FUZZ_BUDGET seconds (default ten minutes) under AddressSanitizer
    # and UBSan; TLS against OpenSSL, HTTP against curl and Python.
    L="$D/fuzz.log"
    echo "== fuzz"
    python3 "$R/impl/fuzz/run.py" --budget "${FUZZ_BUDGET:-600}" --builds asan --jobs "$(nproc)" --out "$D/fuzz" > "$L" 2>&1; echo "[exit $?]" >> "$L"; cat "$L"
    exits_zero "$L"
    require "$L" '^fuzz: PASSED' 1

    L="$D/interop.log"
    echo "== interop"
    timeout -k 10 1800 "$R/impl/fuzz/interop/run.sh" > "$L" 2>&1; echo "[exit $?]" >> "$L"; cat "$L"
    exits_zero "$L"
    require "$L" ', 0 failed$' 1
    [ $FAILS -eq 0 ]
}

stage6() {
    FAILS=0
    local L
    L="$D/dot.log"
    echo "== DNS over TLS, public resolvers"
    {
        cd "$S/dot"
        for r in "1.1.1.1 cloudflare-dns.com" "8.8.8.8 dns.google" "9.9.9.9 dns.quad9.net" "94.140.14.14 dns.adguard-dns.com"; do
            for cc in gcc clang; do timeout -k 10 300 "$R/impl/target/release/cobc" --cc $cc --run smoke.cb $r 2>&1 | tail -1; done
        done
    } > "$L" 2>&1; cat "$L"
    require "$L" 'id ok true' 8

    L="$D/http.log"
    echo "== HTTP client, server, public sites"
    {
        cd "$S/d0173"
        rm -f port.txt; (python3 pyserver.py port.txt &); for i in $(seq 1 20); do [ -s port.txt ] && break; sleep 0.5; done; PP=$(cat port.txt)
        for cc in gcc clang; do echo "== client vs python ($cc)"; timeout -k 5 300 "$R/impl/target/release/cobc" --cc $cc --run t_client.cb "$PP" 2>&1; done
        pkill -f 'pyserver.py port.tx[t]'
        echo "== server vs curl (cobc)"; ./curl_drive.sh "$R/impl/target/release/cobc --cc gcc --run" 2>&1
        echo "== server vs curl (cobc, clang)"; ./curl_drive.sh "$R/impl/target/release/cobc --cc clang --run" 2>&1
        echo "== public sites and DoH (cobc)"; timeout -k 5 300 "$R/impl/target/release/cobc" --cc clang --run t_sites.cb 2>&1
    } > "$L" 2>&1; cat "$L"
    require "$L" '^big +200 1000000 bytes' 2     # the two clients
    require "$L" '^server port [0-9]+' 2         # both servers announced a port
    require "$L" '^ 413$' 2                      # and refused the oversize body
    require "$L" 'id ok true' 1                  # DoH
    forbid  "$L" '^000|Traceback|missing port'
    [ $FAILS -eq 0 ]
}

# ------------------------------------------------------------ driver
summarize() {  # one line from a stage log
    local bad
    bad=$(/usr/bin/grep -a -c -E 'FAILED|^FAIL |DIFF|WRONG|MISMATCH|REQUIRE FAILED|FORBID FAILED|non-zero exits|Sanitizer|runtime error|Traceback' "$1")
    echo "$bad failure lines"
}

run_stage() {
    local n="$1" name="${NAMES[$1]}" log t0 st dur
    log="$D/stage$n-$name.log"
    case " $SEL " in *" $n "*) ;; *) say "stage $n $name: skipped (out of scope)"; return 0 ;; esac
    if [ "$n" -lt "$FROM" ]; then say "stage $n $name: skipped (FROM=$FROM)"; return 0; fi
    if [ "$n" -ge 5 ] && [ -z "$HAVE_STRESS" ]; then say "stage $n $name: skipped (no stress/ directory on this machine)"; return 0; fi
    say "stage $n $name: start"
    t0=$(date +%s)
    "stage$n" > "$log" 2>&1; st=$?
    dur=$(( ($(date +%s) - t0 + 30) / 60 ))
    say "stage $n $name: exit $st, $dur min; $(summarize "$log")"
    if [ $st -ne 0 ]; then
        if [ "$n" -eq 6 ]; then say "stage 6 is advisory: recorded, not gating"; ADVISORY="failed"; return 0; fi
        STOPPED="$n"; return 1
    fi
    return 0
}

report() {
    {
        echo "# Battery $STAMP"
        echo
        echo "tree \`$TREE\`, commit \`$(git -C "$R" rev-parse --short HEAD)\`, scope: $SCOPE_DESC"
        [ -s "$D/scope.txt" ] && { echo; sed 's/^/    /' "$D/scope.txt"; }
        echo
        echo "load at start $LOAD0, at end $(cut -d' ' -f1-3 /proc/loadavg)"
        echo
        echo "| stage | result |"; echo "|---|---|"
        /usr/bin/grep -a ' stage [0-9] ' "$P" | /usr/bin/grep -v ': start$' | sed -E 's/^[0-9:]+ stage ([0-9]) ([a-z]+): (.*)$/| \1 \2 | \3 |/'
        echo
        if [ -n "$PREFAIL" ]; then echo "**STOPPED at preflight.** No stage ran."; echo; fi
        if [ -n "$STOPPED" ]; then echo "**STOPPED at stage $STOPPED (${NAMES[$STOPPED]}).** Later stages did not run."; echo; fi
        [ -n "$ADVISORY" ] && { echo "Stage 6 (external) failed; advisory, see stage6-external.log."; echo; }
        [ -n "$FLAKES" ] && { echo "Flakes rerun alone and passed:$FLAKES."; echo; }
        echo "## Slowest cases"
        echo
        echo '```'
        /usr/bin/grep -a -h -A10 'slowest:$' "$D"/job-*.log 2>/dev/null | head -n 120
        echo '```'
        echo
        echo "## Failure lines"
        echo
        echo '```'
        /usr/bin/grep -a -n -E 'FAILED|^FAIL |DIFF|WRONG|MISMATCH|non-zero exits|Sanitizer|runtime error|Traceback' "$D"/stage*.log 2>/dev/null | /usr/bin/grep -v -E 'FORBID FAILED|REQUIRE FAILED' | head -n 150
        /usr/bin/grep -a -h -E 'REQUIRE FAILED|FORBID FAILED' "$D"/stage*.log 2>/dev/null | head -n 50
        echo '```'
        echo
        echo "## Leftover processes"
        echo
        echo '```'
        ps -eo pid,etimes,pcpu,args | /usr/bin/grep -E 'cobc-[0-9]+-[0-9]+/a\.out|target/(release|tests)/(cobc|cobfmt|cobmodel|cbtest|difftest)( |$)' | /usr/bin/grep -v grep || echo none
        echo '```'
    } > "$D/REPORT.md"
    # SUMMARY (D-0212): what impl/tools/manifest.py reads -- the run, its
    # result, the tree fingerprint it tested and the toolchain.
    {
        echo "run $STAMP"
        echo "tree $TREE"
        if [ -n "$PREFAIL" ]; then echo "result STOPPED at preflight"
        elif [ -n "$STOPPED" ]; then echo "result STOPPED at stage $STOPPED (${NAMES[$STOPPED]})"
        elif [ -n "$STAGES" ]; then echo "result PARTIAL (stages $STAGES)"
        else echo "result DONE${ADVISORY:+ (stage 6 advisory)}"; fi
        echo "scope $SCOPE_DESC"
        # A scoped run: the green tree its scope was computed against, whose
        # run vouches for the stages and parts this one left out (the
        # manifest carries *modelled* from it, CHG-0263).
        [ -n "$SCOPED_BASE" ] && echo "base $SCOPED_BASE"
        # The model sample agreed with both tools (stage 1, D-0219).
        if [ -s "$D/model-sample.txt" ] && /usr/bin/grep -q ' 0 with an outcome outside' "$D/model-sample.txt"; then
            echo "modelled $(/usr/bin/grep -o 'compare: [0-9]* programs' "$D/model-sample.txt" | /usr/bin/grep -o '[0-9]*')"
        fi
        echo "fingerprint $(python3 "$R/impl/tools/manifest.py" --fingerprint 2>/dev/null || echo unknown)"
        echo "gcc $(gcc --version 2>/dev/null | head -n 1 || echo unknown)"
        echo "clang $(clang --version 2>/dev/null | head -n 1 || echo unknown)"
        echo "kernel $(uname -sr 2>/dev/null || echo unknown)"
    } > "$D/SUMMARY"
}

main() {
    echo "start $(date -u) in $D" > "$P"
    TREE=$(tree_hash); LOAD0="?"
    classify "$TREE"
    [ -n "$FULL" ] && export COBALTC_TEST_SLOW=1
    say "scope: $SCOPE_DESC${FULL:+; slow: cases included}"
    preflight > "$D/preflight.log" 2>&1 || { PREFAIL=1; tee -a "$P" < "$D/preflight.log"; say "BATTERY STOPPED at preflight"; report; exit 1; }
    LOAD0=$(cut -d' ' -f1 /proc/loadavg)
    local n
    for n in 0 1 2 3 4 5 6; do run_stage "$n" || break; done
    report
    if [ -n "$STOPPED" ]; then
        say "BATTERY STOPPED at stage $STOPPED (${NAMES[$STOPPED]}); report $D/REPORT.md"
        exit 1
    fi
    if [ -z "$STAGES" ]; then
        { echo "$TREE"; echo "$(date -u) $SCOPE_DESC"; } > "$LAST_GREEN"
        say "recorded green tree $TREE"
    fi
    say "BATTERY DONE${ADVISORY:+ (stage 6 advisory $ADVISORY)}; report $D/REPORT.md"
}

main "$@"
