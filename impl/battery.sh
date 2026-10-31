#!/usr/bin/env bash
# The battery: every check the project runs before a push, as stages in
# cost order, each gating the next. A failing stage stops the battery and
# writes the report; inside a stage every target runs (--no-fail-fast),
# so a stage's report is complete.
#
#   0 gate       preflight, builds, Windows and Rust 1.77.2 checks, formatting,
#                `--check` sweep, unit/cli/bookkeeping tests (release)
#   1 hand       hand-written release tests, the model sample (impl/model,
#                D-0219); the showcase runner, the guide check and the book check side by side
#   2 interp     cb_conformance_suite and spec_rows (coby, release), as
#                2 x SHARDS processes (COBALTC_TEST_SHARD, tests/common/mod.rs)
#   3 compiled   compiled_spec_rows, then compiled_suite (cobc, release); each
#                runs GCC and Clang in parallel and shares one interpreter
#                (oracle) run per file
#   4 debug      the hand-written tests under the debug profile (no perf_
#                assertions; formatting was stage 0's)
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
R="$(cd "$H/.." && pwd)"
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
NAMES=(gate hand interp compiled debug programs external)
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

# The test binary cargo built for `cargo test ARGS...`.
test_exe() {
    (cd "$R/impl" && cargo test "$@" --no-run --message-format=json 2>/dev/null \
        | jq -r 'select(.reason == "compiler-artifact" and .executable != null and (.target.kind | index("test"))) | .executable' | tail -1)
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
        /usr/bin/grep -a -E 'test result|rows checked|cases \(|passed, |slowest:|^ +[0-9]+\.[0-9]+ s |showcases identical|examples, |^\[exit' "$l" | tail -n 30
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
# last green battery. A core change (coby, std, the shared test code, the
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
            impl/src/*|impl/std/*|impl/tests/common/*|spec/conformance.md|spec/examples.md|spec/registry/*|impl/Cargo.*|impl/build.rs)
                w[all]=1 ;;
            impl/battery.sh)                 w[0]=1 ;;
            impl/tests/spec_rows.rs|impl/tests/cb_conformance_suite.rs) w[0]=1; w[2]=1 ;;
            impl/cobc/tests/compiled_spec_rows.rs|impl/cobc/tests/compiled_suite.rs|impl/cobc/tests/oracle/*) w[0]=1; w[3]=1 ;;
            impl/cobc/*|impl/cbrt/*)         w[0]=1; w[1]=1; w[3]=1; w[4]=1; w[5]=1; w[6]=1; parts="$parts tests showcase guide book" ;;
            impl/conformance/*)              w[0]=1; w[2]=1; w[3]=1 ;;
            showcase/*|bench/*|impl/cobaltc_examples/*) w[0]=1; w[1]=1; parts="$parts tests showcase" ;;
            book/*)                          w[0]=1; w[1]=1; parts="$parts book" ;;
            .github/*)                       w[0]=1 ;;
            impl/cobfmt/*)                   w[0]=1; w[4]=1 ;;
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
    stray=$(ps -eo pid,etimes,args | /usr/bin/grep -E 'cobc-[0-9]+-[0-9]+/a\.out|target/(release|debug)/(coby|cobc|cobfmt)( |$)|cargo (test|build|run|check)' | /usr/bin/grep -v grep)
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
    cd "$R/impl" || return 1
    # `cargo test --no-run` builds no staticlib: `cargo build` first, so the
    # `libcbrt.a` cobc links is built from the sources under test, never one
    # left in target/ by an earlier build (a stale one failed the driver test).
    step "build release (bins + libs)"  timeout -k 30 3600 cargo build --release --workspace
    step "build release (tests)"        timeout -k 30 3600 cargo test --release --workspace --no-run
    step "build debug (bins + libs)"    timeout -k 30 3600 cargo build --workspace
    step "build debug (tests)"          timeout -k 30 3600 cargo test --workspace --no-run
    step "windows check" timeout -k 30 1500 cargo check --workspace --tests --target x86_64-pc-windows-msvc
    if cargo +1.77.2 --version >/dev/null 2>&1; then
        local C="$BASE/rust177" t
        rm -rf "$C"; mkdir -p "$C"
        (cd "$R" && tar -cf - --exclude=target impl/src impl/std impl/tests impl/cbrt impl/cobc impl/cobfmt impl/model impl/conformance impl/Cargo.toml impl/Cargo.lock spec/registry | tar -xf - -C "$C")
        for t in x86_64-unknown-linux-gnu x86_64-pc-windows-msvc; do
            step "rust 1.77.2 check $t" bash -c "cd '$C/impl' && CARGO_TARGET_DIR='$BASE/rust177-target' timeout -k 30 1800 cargo +1.77.2 check --workspace --tests --target $t"
        done
    else
        echo "== rust 1.77.2 check: toolchain not installed, skipped"
    fi
    # Formatting, with the release cobfmt (the debug one takes minutes).
    local F="$R/impl/target/release/cobfmt" d o bad
    for d in showcase impl/cobaltc_examples bench impl/std book/examples; do
        step "cobfmt --check $d" timeout -k 10 600 "$F" --check "$R/$d"
    done
    # D-0263: the runtime's contract table is current and every entry point names its rule.
    step "runtime contract (cbrt.h ↔ IMPLEMENTATION-NOTES §6)" timeout -k 10 120 python3 "$R/impl/tools/runtime_contract.py" --check
    # Every exported std item has a `///` summary (WI-18, D-0229).
    step "cobdoc --check (std documented)" timeout -k 10 120 python3 "$R/impl/tools/cobdoc.py" --check
    echo "== cobfmt --check impl/conformance (syntax-error cases may not parse)"
    o=$(timeout -k 10 600 "$F" --check "$R/impl/conformance" 2> "$D/fmt-conformance.err")
    bad=$(/usr/bin/grep -v 'does not parse' "$D/fmt-conformance.err")
    if [ -n "$o" ] || [ -n "$bad" ]; then echo "files cobfmt would change, or errors:"; echo "$o"; echo "$bad"; echo "[exit 1]"; FAILS=$((FAILS + 1)); else echo "[exit 0]"; fi
    local sweep="showcase impl/cobaltc_examples bench"
    [ -d "$S/perfguide" ] && sweep="$sweep stress/perfguide"
    for t in coby cobc; do
        step "$t --check sweep" bash -c "cd '$R' && timeout -k 10 1800 impl/target/release/$t --check $sweep"
    done
    step "release unit, cli, bookkeeping tests (coby)" timeout -k 30 1800 cargo test --release --no-fail-fast -p coby --lib --bins --test cli --test conf_coverage --test print_output --test read_input
    step "release unit, cli, bookkeeping tests (cobc)" timeout -k 30 1800 cargo test --release --no-fail-fast -p cobc --bins --test read_input --test driver --test output_order --test runtime_bookkeeping --test tracking
    step "release cbrt + cobfmt tests" timeout -k 30 1800 cargo test --release --no-fail-fast -p cbrt -p cobfmt
    [ $FAILS -eq 0 ]
}

stage1() {
    FAILS=0
    cd "$R/impl" || return 1
    # D-0261: both conformance suites under the verify modes (the
    # interpreter's whole-state re-derivation, the runtime's table check).
    # Each tool's stride adapts to its state's size (README), so 1 is fine.
    if has_part tests; then
        step "conformance under COBALTC_VERIFY (coby)" bash -c "cd '$R/impl/conformance' && COBALTC_VERIFY=\${VERIFY_STRIDE:-1} timeout -k 10 5400 python3 run.py --impl '$R/impl/target/release/coby {file}' --no-c-code"
        step "conformance under COBALTC_VERIFY (cobc, gcc)" bash -c "cd '$R/impl/conformance' && COBALTC_VERIFY=\${VERIFY_STRIDE:-1} timeout -k 10 5400 python3 run.py --impl '$R/impl/target/release/cobc --run {file}'"
        # D-0264: the destructor and outcome events of both tools agree on every single-threaded case.
        step "trace equivalence (coby vs cobc, gcc)" timeout -k 10 3600 python3 "$R/impl/tools/trace_compare.py" --cc gcc
    fi
    if has_part tests; then
        local L="$D/stage1-coby-tests.log" failing
        step "release hand-written tests (coby)" bash -c "timeout -k 30 3600 cargo test --release --no-fail-fast -p coby --test conformance --test examples_run 2>&1 | tee '$L'; exit \${PIPESTATUS[0]}"
        if [ $? -ne 0 ]; then
            # A perf assertion is rerun alone once before it counts: it
            # measures the machine as much as the interpreter.
            failing=$(/usr/bin/grep -a -E '^test [A-Za-z0-9_:]+ \.\.\. FAILED' "$L" | awk '{ print $2 }')
            if [ -n "$failing" ] && ! echo "$failing" | /usr/bin/grep -q -v '^perf_'; then
                echo "== only perf_ assertions failed ($(echo $failing)); rerunning them alone once"
                if timeout -k 30 1800 cargo test --release -p coby --test conformance perf_; then
                    echo "perf flake: passed alone, not counted"; FLAKES="$FLAKES perf"; FAILS=$((FAILS - 1)); echo "[exit 0]"
                else
                    echo "[exit 1]"
                fi
            fi
        fi
        step "release hand-written tests (cobc)" timeout -k 30 3600 cargo test --release --no-fail-fast -p cobc --test differential --test extern_code
        step "compiled examples" timeout -k 30 1800 cargo test --release -p cobc --test compiled_suite -- --exact compiled_examples_run_ok_or_are_skipped
    fi
    # The model (D-0219): every program of impl/model/sample under coby, and
    # under cobc --explore 8 with GCC and with Clang; every outcome a tool
    # shows must be one the model's rules allow.
    if has_part tests && [ -d "$R/impl/model/sample" ]; then
        step "model sample (cobmodel compare)" timeout -k 30 5400 "$R/impl/target/release/cobmodel" compare "$R/impl/model/sample" \
            --coby "$R/impl/target/release/coby" --cobc "$R/impl/target/release/cobc" --explore 8 --cc gcc --cc clang --jobs 2 --report "$D/model-sample.txt"
        # D-0241: no access the flow table proves clash-free is one that
        # clash refutes, in any interleaving of any sample program.
        step "model sample as licensed (cobmodel check --as-licensed)" timeout -k 30 1800 "$R/impl/target/release/cobmodel" check --as-licensed $R/impl/model/sample/*.cb
        # D-0299: the compiler's certificate for each sample program (which
        # bodies keep no record of their references) checked against every
        # interleaving: no alias or temporal fault in a body it calls decided.
        step "compiler certificates checked against the model (certify_model.py)" timeout -k 30 1800 python3 "$R/impl/tools/certify_model.py" "$R/impl/model/sample" --out "$D/certify" --check
    fi
    # CHG-0346, CHG-0350, CHG-0391: the toolchain written in CobaltC, built by
    # cobc and compared with the Rust originals on the showcases and the
    # surface-syntax cases (the whole corpus: impl/selfhost/compare.sh,
    # compare_parser.sh, compare_c.sh; D-0296 keeps both toolchains in step).
    if has_part tests; then
        step "selfhost build (cobfmt-cb, cbparse, cbc)" timeout -k 30 1800 "$R/impl/selfhost/build.sh"
        step "selfhost compilers compared" bash -c "cd '$R' && OUT='$D/selfhost-c' timeout -k 30 3600 impl/selfhost/compare_c.sh \$(git ls-files 'showcase/*.cb')"
        step "selfhost formatters compared" bash -c "cd '$R' && OUT='$D/selfhost-fmt' timeout -k 30 3600 impl/selfhost/compare.sh -m plain,narrow,messy \$(git ls-files 'showcase/*.cb' 'impl/conformance/22-surface-syntax/*.cb')"
        step "selfhost parsers compared" bash -c "cd '$R' && OUT='$D/selfhost-parse' timeout -k 30 3600 impl/selfhost/compare_parser.sh \$(git ls-files 'showcase/*.cb' 'impl/conformance/22-surface-syntax/*.cb')"
    fi
    # The normative corpus, machine-checked (D-0207): entity links, tag
    # vocabularies, label and diagnostic references, numbering, the
    # edition, the rows' files, spec/21 §0's trusted table.
    step "speccheck" timeout -k 10 120 python3 "$R/impl/tools/speccheck.py"
    # CHG-0350: the parser written in CobaltC reads the prelude's skeleton
    # from impl/selfhost/front/prelude.cb, which must be what coby makes of
    # std now (`coby --prelude-summary`).
    step "selfhost prelude summary current" bash -c "'$R/impl/target/release/coby' --prelude-summary | cmp - '$R/impl/selfhost/front/prelude.cb'"
    # D-0270: the mechanised fragment (proofs/) builds with no `sorry`, and its
    # coverage of spec/23's rule rows is the table proofs/COVERAGE.md states:
    # a rule row with neither a Lean step nor a listed reason to be outside
    # the fragment fails here, and so does a rule change that breaks a lemma.
    # `lake` comes from elan (~/.elan/bin); a clean build takes under a minute.
    step "proof coverage (proofcover --check)" timeout -k 10 120 python3 "$R/impl/tools/proofcover.py" --check
    # D-0299: the assurance case (spec/ASSURANCE.md) names only evidence that exists.
    step "assurance case current (assurance.py --check)" timeout -k 10 120 python3 "$R/impl/tools/assurance.py" --check
    step "lean proofs (lake build, no sorry)" bash -c "set -o pipefail; cd '$R/proofs' && export PATH=\$HOME/.elan/bin:\$PATH && timeout -k 30 1800 lake build 2>&1 | tee '$D/lake.log' && ! /usr/bin/grep -q \"declaration uses 'sorry'\" '$D/lake.log'"
    # D-0270: the model checker's runs replayed through the proven machine
    # (proofs/Replay.lean, impl/tools/replay_model.py): every sample program's
    # trace is admitted whole or stops where the model leaves the fragment;
    # an action the fragment's rules refuse fails here.
    if has_part tests && [ -d "$R/impl/model/sample" ]; then
        step "model runs replayed through the proven machine" timeout -k 30 1800 python3 "$R/impl/tools/replay_model.py" "$R/impl/model/sample" --out "$D/replay" --check
    fi
    # Every exported std item named by some conformance case, every case
    # file named by a row or marked a fixture (WI-6, CHG-0265).
    step "std coverage" timeout -k 10 120 python3 "$R/impl/tools/std_coverage.py"
    # std's own tests, the std reference's examples (WI-18), under both
    # tools and both C compilers; run from the run's own directory.
    if has_part tests; then
        printf 'fn main()\n{\n}\n' > "$D/std-tests.cb"
        step "std tests (coby --unittest-std)" bash -c "cd '$D' && timeout -k 10 1800 '$R/impl/target/release/coby' --unittest-std std-tests.cb < /dev/null"
        local cc
        for cc in gcc clang; do
            step "std tests (cobc --unittest-std, $cc)" bash -c "cd '$D' && timeout -k 10 1800 '$R/impl/target/release/cobc' --unittest-std --cc $cc --run std-tests.cb < /dev/null"
        done
    fi
    if has_part showcase; then start_job showcase bash -c "cd '$R/showcase' && LIMIT=900 timeout -k 30 7200 ./run_all.sh"; fi
    if has_part guide;    then start_job guide    bash -c "cd '$R/htmlguide/generator' && timeout -k 30 5400 python3 gen.py --check"; fi
    # The book's examples under both tools and both C compilers, and each
    # one's text in its chapter (book/check.sh).
    if has_part book;     then start_job book     bash -c "cd '$R/book' && LIMIT=300 timeout -k 30 3600 ./check.sh"; fi
    wait_jobs
    [ $FAILS -eq 0 ]
}

# Runs one corpus binary as N processes (COBALTC_TEST_SHARD=i/N).
start_corpus() {
    local label="$1" exe="$2" n="$3"; shift 3
    local i
    if [ "$n" -gt 1 ]; then
        for i in $(seq 1 "$n"); do start_job "$label-$i" env COBALTC_TEST_SHARD="$i/$n" "$exe" --nocapture "$@"; done
    else
        start_job "$label" "$exe" --nocapture "$@"
    fi
}

stage2() {
    FAILS=0
    cd "$R/impl" || return 1
    local a b
    a=$(test_exe --release -p coby --test cb_conformance_suite)
    b=$(test_exe --release -p coby --test spec_rows)
    [ -x "$a" ] && [ -x "$b" ] || { echo "cannot locate the test binaries: '$a' '$b'"; return 1; }
    echo "== $(basename "$a") and $(basename "$b"), $SHARDS shard(s) each, side by side"
    start_corpus cb_conformance_suite "$a" "$SHARDS"
    start_corpus spec_rows "$b" "$SHARDS"
    wait_jobs
    [ $FAILS -eq 0 ]
}

stage3() {
    FAILS=0
    cd "$R/impl/cobc" || return 1
    local a b
    a=$(test_exe --release -p cobc --test compiled_spec_rows)
    b=$(test_exe --release -p cobc --test compiled_suite)
    [ -x "$a" ] && [ -x "$b" ] || { echo "cannot locate the test binaries: '$a' '$b'"; return 1; }
    # One after the other: each already runs GCC and Clang side by side,
    # and four compiler threads on four cores only slowed the first run.
    echo "== $(basename "$a"), then $(basename "$b") (the examples were stage 1's)"
    start_corpus compiled_spec_rows "$a" 1
    wait_jobs
    start_corpus compiled_suite "$b" 1 --skip compiled_examples
    wait_jobs
    # D-0209: the same file suite with every proof about a function's
    # reference parameters switched off (`cobc --all-checks`), under one C
    # compiler: the elided forms and the fully checked ones must both be
    # what `coby` does, on every case.
    echo "== compiled_suite again under COBALTC_ALL_CHECKS=1 (gcc)"
    start_job compiled_suite_allchecks env COBALTC_ALL_CHECKS=1 COBC_TEST_CCS=gcc "$b" --nocapture --skip compiled_examples
    wait_jobs
    [ $FAILS -eq 0 ]
}

stage4() {
    FAILS=0
    cd "$R/impl" || return 1
    step "debug coby (no perf_ assertions)" timeout -k 30 7200 cargo test --no-fail-fast -p coby --lib --bins --test cli --test conformance --test conf_coverage --test examples_run --test print_output --test read_input -- --skip perf_
    step "debug cobc" timeout -k 30 7200 cargo test --no-fail-fast -p cobc --bins --test differential --test driver --test extern_code --test read_input --test runtime_bookkeeping --test tracking --test output_order
    step "debug cbrt + cobfmt (formatting was checked in stage 0)" timeout -k 30 3600 cargo test --no-fail-fast -p cbrt -p cobfmt -- --skip already_formatted
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
        (cd "$S/net" && for t in coby "cobc --cc gcc --run" "cobc --cc clang --run"; do timeout -k 5 300 $R/impl/target/release/$t bulk.cb 1 2>&1 | /usr/bin/grep received; done | sort | uniq -c | awk 'END { print (NR == 1 && $1 == 3 ? "ok  bulk 1 MiB, all three agree" : "DIFF  bulk 1 MiB") }')
        (cd "$S/net" && for cc in gcc clang; do timeout -k 5 300 $R/impl/target/release/cobc --cc $cc --run bulk.cb 16 2>&1 | tail -n 2 | tr '\n' ' '; echo "($cc, 16 MiB)"; done)
    } > "$L" 2>&1; cat "$L"
    require "$L" '^ok ' 9
    require "$L" 'received 16777216 bytes of 16777216' 2
    forbid  "$L" 'DIFF|WRONG|panicked|Traceback'

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
    forbid  "$L" 'Sanitizer|runtime error|Traceback|FAIL|MISMATCH|panicked'

    L="$D/crypto_sweep.log"
    echo "== crypto sweep"
    (cd "$S/d0166" && ./cobc_sweep.sh) > "$L" 2>&1; cat "$L"
    require "$L" '^\[sweep done\]' 1
    require "$L" 'ALL OK' 8
    forbid  "$L" 'FAIL|DIFF|MISMATCH|Traceback|panicked|error'

    L="$D/local_tls.log"
    echo "== local TLS"
    (cd "$S/d0166" && ./local_tls.sh) > "$L" 2>&1; cat "$L"
    require "$L" '^reply: etius olleh' 8
    require "$L" '^\[local done\]' 1
    forbid  "$L" 'error|Error|panicked|Traceback'

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
        echo "== coby, cloudflare"; timeout -k 10 900 "$R/impl/target/release/coby" smoke.cb 1.1.1.1 cloudflare-dns.com 2>&1 | tail -1
    } > "$L" 2>&1; cat "$L"
    require "$L" 'id ok true' 9

    L="$D/http.log"
    echo "== HTTP client, server, public sites"
    {
        cd "$S/d0173"
        rm -f port.txt; (python3 pyserver.py port.txt &); for i in $(seq 1 20); do [ -s port.txt ] && break; sleep 0.5; done; PP=$(cat port.txt)
        for cc in gcc clang; do echo "== client vs python ($cc)"; timeout -k 5 300 "$R/impl/target/release/cobc" --cc $cc --run t_client.cb "$PP" 2>&1; done
        echo "== client vs python (coby)"; timeout -k 5 600 "$R/impl/target/release/coby" t_client.cb "$PP" 2>&1
        pkill -f 'pyserver.py port.tx[t]'
        echo "== server vs curl (cobc)"; ./curl_drive.sh "$R/impl/target/release/cobc --cc gcc --run" 2>&1
        echo "== server vs curl (coby)"; ./curl_drive.sh "$R/impl/target/release/coby" 2>&1
        echo "== public sites and DoH (cobc)"; timeout -k 5 300 "$R/impl/target/release/cobc" --cc clang --run t_sites.cb 2>&1
    } > "$L" 2>&1; cat "$L"
    require "$L" '^big +200 1000000 bytes' 3     # the three clients
    require "$L" '^server port [0-9]+' 2         # both servers announced a port
    require "$L" '^ 413$' 2                      # and refused the oversize body
    require "$L" 'id ok true' 1                  # DoH
    forbid  "$L" '^000|Traceback|panicked|missing port'
    [ $FAILS -eq 0 ]
}

# ------------------------------------------------------------ driver
summarize() {  # one line from a stage log
    local ok fail bad
    ok=$(/usr/bin/grep -a -c '^test result: ok' "$1")
    fail=$(/usr/bin/grep -a -c '^test result: FAILED' "$1")
    bad=$(/usr/bin/grep -a -c -E 'FAILED|panicked|targets failed|^FAIL |DIFF|WRONG|MISMATCH|REQUIRE FAILED|FORBID FAILED|non-zero exits|Sanitizer|runtime error|Traceback' "$1")
    echo "$ok test targets ok, $fail failed, $bad failure lines"
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
        /usr/bin/grep -a -n -E 'FAILED|panicked|targets failed|^FAIL |DIFF|WRONG|MISMATCH|non-zero exits|Sanitizer|runtime error|Traceback' "$D"/stage*.log 2>/dev/null | /usr/bin/grep -v -E 'FORBID FAILED|REQUIRE FAILED' | head -n 150
        /usr/bin/grep -a -h -E 'REQUIRE FAILED|FORBID FAILED' "$D"/stage*.log 2>/dev/null | head -n 50
        echo '```'
        echo
        echo "## Leftover processes"
        echo
        echo '```'
        ps -eo pid,etimes,pcpu,args | /usr/bin/grep -E 'cobc-[0-9]+-[0-9]+/a\.out|target/(release|debug)/(coby|cobc|cobfmt)( |$)' | /usr/bin/grep -v grep || echo none
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
        echo "rustc $(rustc -V 2>/dev/null || echo unknown)"
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
