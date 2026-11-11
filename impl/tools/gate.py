#!/usr/bin/env python3
"""The gate (D-0307): every check the project runs, as units with declared inputs, run on a snapshot of the
tree, a unit's green verdict kept under the hash of its inputs so that it never runs again on the same inputs.

  impl/gate.sh                  # the gate profile on HEAD: what a commit and a push need
  impl/gate.sh release          # the gate and the release profile's slow units (the owner says when)
  impl/gate.sh all              # every unit of both profiles, the store of verdicts ignored
  impl/gate.sh UNIT...          # those units only (names: impl/gate.sh --list)
  impl/gate.sh --dirty ...      # the working tree as it is, not HEAD (for work in progress; a push needs HEAD)
  impl/gate.sh --list [PROFILE] # the units, their inputs' hash on HEAD and whether a green verdict is kept

A unit's inputs are tracked paths (git pathspecs) plus the versions of rustc, gcc and clang and the unit's own
command; README.md and spec/MANIFEST.md are outputs and never inputs. Units run one after another, each with at
most three processes, a CPU limit per process and a wall-clock limit; one gate runs at a time (a lock). The
corpus runners keep a result per case as well (`COBALTC_RESULT_STORE`, `tests/common/mod.rs`), so a unit that
runs after a change runs only the cases the change can reach.

Where: `<home>/snap` (a `git clone --shared` of this repository, its `impl/target` kept between runs),
`<home>/store` (verdicts and per-case results), `<home>/runs/<UTC stamp>` (logs, REPORT.md, SUMMARY);
<home> is stress/gate beside impl/ when stress/ exists, else ~/.cobaltc-gate.
"""
import fcntl
import hashlib
import json
import os
import re
import resource
import shutil
import signal
import subprocess
import sys
import time

ROOT = os.path.dirname(os.path.dirname(os.path.dirname(os.path.abspath(__file__))))
HOME = os.path.join(ROOT, "stress", "gate") if os.path.isdir(os.path.join(ROOT, "stress")) else os.path.expanduser("~/.cobaltc-gate")
SNAP = os.path.join(HOME, "snap")
STORE = os.path.join(HOME, "store")
RUNS = os.path.join(HOME, "runs")
OUTPUTS = [":(exclude)README.md", ":(exclude)spec/MANIFEST.md",
           # the driver itself: a unit's own command is part of its key instead
           ":(exclude)impl/gate.sh", ":(exclude)impl/battery.sh", ":(exclude)impl/tools/gate.py", ":(exclude)impl/tools/suites.sh"]

# Everything that builds or drives the tools a corpus case runs under: the per-case store's key.
TOOLCHAIN = ["impl/src", "impl/cobc", "impl/cbrt", "impl/std", "impl/Cargo.toml", "impl/Cargo.lock",
             "impl/tests", "spec/registry"]
SELFHOST = TOOLCHAIN + ["impl/selfhost"]
# The conformance cases and their fixtures, not the corpus's own runner or its documentation.
CORPUS = ["impl/conformance", ":(exclude)impl/conformance/run.py", ":(exclude)impl/conformance/*.md"]
EVERYTHING = ["."]

# The parts of the local stress/ directory the units run (the perf guide, the release profile's programs).
STRESS_PARTS = ["run.sh", "perfguide", "datetime", "paths", "random", "process", "net", "d0166", "round5", "d0173", "dot", "crypto"]

CARGO_FAILED = r"^test result: FAILED|panicked at|error: \d+ targets? failed"

# ---------------------------------------------------------------- the units
#
# name, profile ("gate" or "release"), inputs (pathspecs), command (bash, run from the snapshot's root with
# $G the run directory), wall-clock limit in seconds; optional: "fail" (a regex that makes a log red),
# "advisory" (red is recorded, not gating), "py" (a function of this file instead of a command).

UNITS = [
    dict(name="build", profile="gate", inputs=EVERYTHING, limit=5400, always=True,
         cmd="cd impl && cargo build --release --workspace && cargo test --release --workspace --no-run "
             "&& cd .. && { if [ -x impl/target/selfhost/cbc ] && [ \"$(cat impl/target/selfhost/.gate-key 2>/dev/null)\" = \"$GATE_SELFHOST_KEY\" ]; "
             "then echo 'self-hosted toolchain current'; else impl/selfhost/build.sh && echo \"$GATE_SELFHOST_KEY\" > impl/target/selfhost/.gate-key; fi; } "
             "&& impl/target/release/coby --prelude-summary | cmp - impl/selfhost/front/prelude.cb && echo prelude current"),
    dict(name="format", profile="gate", limit=1800,
         inputs=["impl/cobfmt", "impl/src", "impl/Cargo.lock", ":(glob)**/*.cb"],
         cmd="rc=0; F=impl/target/release/cobfmt; "
             "for d in showcase impl/cobaltc_examples bench impl/std book/examples impl/selfhost impl/fuzz; do "
             "  o=$(timeout -k 10 600 $F --check $d 2>&1) || { echo \"$d: $o\"; rc=1; }; done; "
             "o=$(timeout -k 10 600 $F --check impl/conformance 2>&1 | grep -v 'does not parse'); "
             "[ -n \"$o\" ] && { echo \"impl/conformance: $o\"; rc=1; }; exit $rc"),
    dict(name="records", profile="gate", limit=1800, inputs=EVERYTHING,
         cmd="set -eo pipefail; python3 impl/tools/speccheck.py; python3 impl/tools/proofcover.py --check; "
             "python3 impl/tools/assurance.py --check; python3 impl/tools/runtime_contract.py --check; "
             "python3 impl/tools/cobdoc.py --check; python3 impl/tools/std_coverage.py; "
             # CHG-0447: CARD.md is current; CHG-0445: each habit's message is as recorded.
             "python3 impl/tools/card.py --check; "
             "impl/tests/foreign_forms/run.sh --compare impl/tests/foreign_forms/expected.txt"),
    dict(name="portable", profile="gate", limit=3600,
         inputs=["impl/src", "impl/cobc", "impl/cbrt", "impl/cobfmt", "impl/model", "impl/std", "impl/tests",
                 "impl/conformance", ":(glob)impl/**/Cargo.toml", "impl/Cargo.lock", "spec/registry"],
         cmd="set -eo pipefail; cd impl; timeout -k 30 1500 cargo check --workspace --tests --target x86_64-pc-windows-msvc 2>&1 | tail -3; "
             "if cargo +1.77.2 --version >/dev/null 2>&1; then C=$G/rust177; rm -rf $C; mkdir -p $C; "
             "(cd .. && tar -cf - --exclude=target impl/src impl/std impl/tests impl/cbrt impl/cobc impl/cobfmt impl/model "
             "impl/conformance impl/Cargo.toml impl/Cargo.lock spec/registry | tar -xf - -C $C); "
             "for t in x86_64-unknown-linux-gnu x86_64-pc-windows-msvc; do (cd $C/impl && "
             "CARGO_TARGET_DIR=" + os.path.join(HOME, "rust177-target") + " timeout -k 30 1800 cargo +1.77.2 check --workspace --tests --target $t 2>&1 | tail -2); done; "
             "rm -rf $C; else echo 'rust 1.77.2 not installed: skipped'; fi"),
    dict(name="sweep-check", profile="gate", limit=3600,
         inputs=TOOLCHAIN + ["showcase", "impl/cobaltc_examples", "bench"],
         cmd="set -eo pipefail; for t in coby cobc; do timeout -k 10 1800 impl/target/release/$t --check showcase impl/cobaltc_examples bench; done"),
    dict(name="tests", profile="gate", limit=10800,
         inputs=["impl", "showcase", "spec/conformance.md", "spec/examples.md", "spec/registry", "book/examples"],
         cmd="cd impl; L=$G/tests.cargo.log; timeout -k 30 9000 cargo test --release --workspace --no-fail-fast -- "
             "--skip every_spec_conformance_row --skip cb_conformance_suite_all_cases --skip compiled_conformance_suite_matches_headers "
             "> $L 2>&1; rc=$?; grep -aE '^test result|FAILED|panicked|targets? failed' $L | grep -v '^test result: ok' ; "
             "echo \"$(grep -ac '^test result: ok' $L) test targets ok\"; "
             "if [ $rc -ne 0 ]; then f=$(grep -aE '^test [A-Za-z0-9_:]+ \\.\\.\\. FAILED' $L | awk '{print $2}'); "
             "if [ -n \"$f\" ] && ! echo \"$f\" | grep -qv '^perf_'; then echo \"only perf_ assertions failed ($f): rerun alone once\"; "
             "timeout -k 30 1800 cargo test --release -p coby --test conformance perf_ && { echo 'perf flake: passed alone'; rc=0; }; fi; fi; exit $rc"),
    dict(name="corpus-coby", fail=CARGO_FAILED, profile="gate", limit=7200,
         inputs=TOOLCHAIN + [*CORPUS, "spec/conformance.md", "spec/examples.md"],
         cmd="cd impl; set -o pipefail; rc=0; for t in cb_conformance_suite spec_rows; do pids=; for i in 1 2 3; do "
             "COBALTC_TEST_SHARD=$i/3 timeout -k 30 5400 cargo test -q --release -p coby --test $t -- --nocapture > $G/corpus-coby-$t-$i.log 2>&1 & pids=\"$pids $!\"; done; "
             "for p in $pids; do wait $p || rc=1; done; done; "
             "grep -ahE '^test result|cases \\(|rows checked|FAILED|disagree|failed:' $G/corpus-coby-*.log | head -40; exit $rc"),
    dict(name="corpus-cobc", fail=CARGO_FAILED, profile="gate", limit=14400,
         inputs=TOOLCHAIN + [*CORPUS, "spec/conformance.md", "spec/examples.md"],
         cmd="cd impl; rc=0; timeout -k 30 9000 cargo test -q --release -p cobc --test compiled_spec_rows -- --nocapture > $G/corpus-cobc-rows.log 2>&1 || rc=1; "
             "timeout -k 30 9000 cargo test -q --release -p cobc --test compiled_suite -- --nocapture --skip compiled_examples > $G/corpus-cobc-suite.log 2>&1 || rc=1; "
             "grep -ahE '^test result|passed, .* skipped|reused|disagree' $G/corpus-cobc-*.log | head -20; exit $rc"),
    dict(name="std-tests", profile="gate", limit=5400, inputs=TOOLCHAIN,
         cmd="set -eo pipefail; cd $G; printf 'fn main()\\n{\\n}\\n' > std-tests.cb; R=$OLDPWD; "
             "timeout -k 10 1800 $R/impl/target/release/coby --unittest-std std-tests.cb < /dev/null | tail -2; "
             "for cc in gcc clang; do timeout -k 10 1800 $R/impl/target/release/cobc --unittest-std --cc $cc --run std-tests.cb < /dev/null | tail -2; done"),
    dict(name="guide", profile="gate", limit=7200, inputs=TOOLCHAIN + ["htmlguide/generator"],
         cmd="cd htmlguide/generator && timeout -k 30 5400 python3 gen.py --check > $G/guide.log 2>&1; rc=$?; grep -a FAIL $G/guide.log | head -20; tail -1 $G/guide.log; exit $rc"),
    dict(name="book", profile="gate", limit=3600, inputs=TOOLCHAIN + ["impl/cobfmt", "book"],
         cmd="cd book && LIMIT=300 timeout -k 30 3000 ./check.sh 2>&1 | tail -15; exit ${PIPESTATUS[0]}"),
    dict(name="showcase", profile="gate", limit=7200, inputs=TOOLCHAIN + ["showcase"],
         cmd="cd showcase && LIMIT=900 timeout -k 30 7000 ./run_all.sh > $G/showcase.log 2>&1; rc=$?; grep -aE 'DIFF|FAIL|WRONG' $G/showcase.log | head -20; tail -3 $G/showcase.log; exit $rc"),
    dict(name="model", profile="gate", limit=10800,
         inputs=TOOLCHAIN + ["impl/model", "proofs", "impl/tools/certify_model.py", "impl/tools/factcheck.py", "impl/tools/validate_c.py", "impl/tools/replay_model.py", "impl/tools/replay_runtime.py", "showcase", "impl/conformance/14-control-flow", "spec/23-formal-model.md"],
         # The proofs first: the replays run the machine `lake build` makes (proofs/.lake/build/bin/replay).
         cmd="set -eo pipefail; B=impl/target/release; S=impl/model/sample; "
             "(cd proofs && export PATH=$HOME/.elan/bin:$PATH && timeout -k 30 1800 lake build > $G/lake.log 2>&1); tail -1 $G/lake.log; "
             "if grep -q \"declaration uses 'sorry'\" $G/lake.log; then echo 'a proof uses sorry'; exit 1; fi; "
             "timeout -k 30 5400 $B/cobmodel compare $S --coby $B/coby --cobc $B/cobc --explore 8 --cc gcc --cc clang --jobs 2 --report $G/model-sample.txt | tail -3; "
             "grep -q ' 0 with an outcome outside' $G/model-sample.txt; "
             "timeout -k 30 1800 $B/cobmodel check --as-licensed $S/*.cb | tail -2; "
             "timeout -k 30 1800 python3 impl/tools/certify_model.py $S --out $G/certify --check | tail -1; "
             "timeout -k 30 3600 python3 impl/tools/factcheck.py $S showcase impl/conformance/14-control-flow --out $G/factcheck --check | tail -1; "
             "timeout -k 30 3600 python3 impl/tools/validate_c.py $S showcase --out $G/validate-c --check | tail -1; "
             "timeout -k 30 1800 python3 impl/tools/replay_model.py $S --out $G/replay --check | tail -1; "
             "timeout -k 30 3600 python3 impl/tools/replay_runtime.py $S --out $G/replay-runtime --check | tail -1"),
    dict(name="selfhost", profile="gate", limit=10800, inputs=SELFHOST + [":(glob)**/*.cb"], py="unit_selfhost"),
    dict(name="lowering", profile="gate", limit=10800,
         inputs=["impl/src", "impl/cobc/src", "impl/cbrt", "impl/std", "impl/Cargo.lock", "bench"], py="unit_lowering"),
    # ---- the release profile: slow, and for classes of fault the gate's units have not been the ones to find
    dict(name="fixpoint", profile="release", limit=10800, inputs=SELFHOST,
         cmd="OUT=$G/fixpoint CBC1=$PWD/impl/target/selfhost/cbc timeout -k 30 9000 impl/selfhost/fixpoint.sh 2>&1 | tail -3; exit ${PIPESTATUS[0]}"),
    dict(name="selfhost-whole", profile="release", limit=14400, inputs=SELFHOST + ["impl/cobfmt", ":(glob)**/*.cb"],
         cmd="rc=0; OUT=$G/fmt timeout -k 30 7200 impl/selfhost/compare.sh 2>&1 | tail -2 || rc=1; "
             "OUT=$G/parse timeout -k 30 7200 impl/selfhost/compare_parser.sh 2>&1 | tail -2; [ ${PIPESTATUS[0]} -eq 0 ] || rc=1; exit $rc"),
    dict(name="all-checks", fail=CARGO_FAILED, profile="release", limit=10800,
         inputs=TOOLCHAIN + CORPUS,
         cmd="cd impl && COBALTC_ALL_CHECKS=1 COBC_TEST_CCS=gcc timeout -k 30 9000 cargo test -q --release -p cobc --test compiled_suite -- --nocapture --skip compiled_examples 2>&1 | tee $G/all-checks.cargo.log | grep -aE '^test result|passed,|FAILED|disagree' | head; exit ${PIPESTATUS[0]}"),
    dict(name="slow-cases", fail=CARGO_FAILED, profile="release", limit=14400,
         inputs=TOOLCHAIN + [*CORPUS, "spec/conformance.md", "spec/examples.md"],
         cmd="cd impl; export COBALTC_TEST_SLOW=1; rc=0; "
             "timeout -k 30 7200 cargo test -q --release -p coby --test cb_conformance_suite --test spec_rows -- --nocapture > $G/slow-coby.log 2>&1 || rc=1; "
             "timeout -k 30 7200 cargo test -q --release -p cobc --test compiled_spec_rows --test compiled_suite -- --nocapture --skip compiled_examples > $G/slow-cobc.log 2>&1 || rc=1; "
             "grep -ahE '^test result|disagree' $G/slow-*.log | head; exit $rc"),
    dict(name="debug", profile="release", limit=14400, inputs=["impl"],
         cmd="cd impl; rc=0; timeout -k 30 7200 cargo test --no-fail-fast -p coby --lib --bins --test cli --test conformance --test conf_coverage --test examples_run --test print_output --test read_input -- --skip perf_ > $G/debug.log 2>&1 || rc=1; "
             "timeout -k 30 1800 cargo build -p cobfmt >> $G/debug.log 2>&1 || rc=1; "  # the driver tests run the sibling cobfmt
             "timeout -k 30 7200 cargo test --no-fail-fast -p cobc --bins --test differential --test driver --test extern_code --test read_input --test runtime_bookkeeping --test tracking --test output_order >> $G/debug.log 2>&1 || rc=1; "
             "timeout -k 30 3600 cargo test --no-fail-fast -p cbrt -p cobfmt -- --skip already_formatted >> $G/debug.log 2>&1 || rc=1; "
             "grep -aE '^test result|FAILED|panicked' $G/debug.log | grep -v '^test result: ok' | head; echo \"$(grep -ac '^test result: ok' $G/debug.log) targets ok\"; exit $rc"),
    dict(name="verify", profile="release", limit=14400, inputs=TOOLCHAIN + ["impl/conformance", "impl/tools/trace_compare.py"],
         cmd="set -eo pipefail; R=$PWD; cd impl/conformance; "
             "COBALTC_VERIFY=1 timeout -k 10 5400 python3 run.py --jobs 3 --impl \"$R/impl/target/release/coby {file}\" --no-c-code 2>&1 | tail -2; "
             "COBALTC_VERIFY=1 timeout -k 10 5400 python3 run.py --jobs 3 --impl \"$R/impl/target/release/cobc --run {file}\" 2>&1 | tail -2; "
             "cd $R; timeout -k 10 3600 python3 impl/tools/trace_compare.py --cc gcc 2>&1 | tail -2"),
    dict(name="programs", profile="release", limit=14400, inputs=TOOLCHAIN + ["impl/fuzz", "impl/tools/battery_stages.sh"],
         cmd="STAGES=5 MAXLOAD=99 OUT=$G/programs impl/tools/battery_stages.sh > $G/programs.out 2>&1; rc=$?; tail -4 $G/programs.out; exit $rc"),
    # Proofs assurance S3 (private proposal of 2026-11-07): two kernels other than the one `lake build` runs
    # check the development again, leanchecker (Lean's own, replaying the .olean files) and nanoda (an
    # independent kernel in Rust, over leanexport's export) with Lean's three axioms permitted.
    dict(name="proofs-kernels", profile="release", limit=7200, inputs=["proofs"],
         cmd="cd proofs && export PATH=$HOME/.elan/bin:$PATH && rc=0; "
             "timeout -k 30 1800 lake build > $G/kernels-build.log 2>&1 || { tail -5 $G/kernels-build.log; exit 1; }; "
             "MODS=$(sed -n 's/.*roots := #\\[\\(.*\\)\\].*/\\1/p' lakefile.lean | tr -d '`,'); "
             "timeout -k 30 1800 lake env leanchecker $MODS > $G/leanchecker.log 2>&1 || { rc=1; echo 'leanchecker failed'; tail -5 $G/leanchecker.log; }; "
             "N=$(command -v nanoda_bin || ls $HOME/.elan/toolchains/*/bin/nanoda_bin 2>/dev/null | tail -1); "
             "if [ ! -x \"$N\" ]; then echo 'nanoda_bin not found'; exit 1; fi; "
             "timeout -k 30 1800 lake env leanexport $MODS > $G/export.ndjson 2> $G/leanexport.log || { rc=1; echo 'leanexport failed'; tail -5 $G/leanexport.log; }; "
             "printf '{\"export_file_path\": \"%s\", \"use_stdin\": false, \"permitted_axioms\": [\"propext\", \"Classical.choice\", \"Quot.sound\", \"Lean.trustCompiler\"], "
             "\"unpermitted_axiom_hard_error\": false, \"nat_extension\": true, \"string_extension\": true, \"print_success_message\": true}' $G/export.ndjson > $G/nanoda.json; "
             "timeout -k 30 3600 \"$N\" $G/nanoda.json > $G/nanoda.log 2>&1 || rc=1; rm -f $G/export.ndjson; "
             # The export declares core's `sorryAx` whether or not anything uses it, so an unpermitted axiom is
             # not a hard error here; which axioms each result rests on is Statements.lean's `#guard_msgs`, which
             # the build checks and leanchecker replays. nanoda's part is the typing of every declaration.
             "grep -q 'with no typechecker errors' $G/nanoda.log || { rc=1; head -5 $G/nanoda.log; }; "
             "echo \"leanchecker: $(echo $MODS | wc -w) modules; nanoda: $(grep -o 'Checked [0-9]* declarations with [a-z ]*errors' $G/nanoda.log | head -1)\"; exit $rc"),
    dict(name="external", profile="release", limit=3600, inputs=TOOLCHAIN + ["impl/tools/battery_stages.sh"], advisory=True,
         cmd="STAGES=6 MAXLOAD=99 OUT=$G/external impl/tools/battery_stages.sh > $G/external.out 2>&1; rc=$?; tail -4 $G/external.out; grep -q 'advisory failed' $G/external.out && rc=1; exit $rc"),
]
BY_NAME = {u["name"]: u for u in UNITS}

# ---------------------------------------------------------------- hashing


def git(*args, cwd=None, check=True, binary=False):
    r = subprocess.run(["git"] + list(args), cwd=cwd or ROOT, capture_output=True)
    if check and r.returncode != 0:
        raise SystemExit("git %s: %s" % (" ".join(args), r.stderr.decode(errors="replace").strip()))
    return r.stdout if binary else r.stdout.decode()


def versions():
    out = []
    for cmd in (["rustc", "-V"], ["gcc", "--version"], ["clang", "--version"]):
        try:
            out.append(subprocess.run(cmd, capture_output=True, text=True).stdout.split("\n")[0])
        except OSError:
            out.append("%s missing" % cmd[0])
    return "\n".join(out)


_VERSIONS = None


def tool_versions():
    global _VERSIONS
    if _VERSIONS is None:
        _VERSIONS = versions()
    return _VERSIONS


def paths_hash(repo, specs):
    """The hash of the index entries (mode, blob, path) under SPECS in REPO: the inputs' contents, exactly."""
    listing = git("ls-files", "-s", "-z", "--", *specs, *OUTPUTS, cwd=repo, binary=True)
    return hashlib.sha256(listing).hexdigest()


def unit_hash(repo, u):
    h = hashlib.sha256()
    for part in (u["name"], u.get("cmd", "") or u.get("py", ""), tool_versions(), paths_hash(repo, u["inputs"])):
        h.update(part.encode() + b"\0")
    return h.hexdigest()


def toolchain_key(repo, specs=TOOLCHAIN):
    return hashlib.sha256((tool_versions() + "\0" + paths_hash(repo, specs)).encode()).hexdigest()


def verdict_file(u, h):
    return os.path.join(STORE, "units", u["name"], h + ".json")


def kept(repo, u):
    """The green verdict kept for unit U on REPO's index, or None."""
    f = verdict_file(u, unit_hash(repo, u))
    if os.path.exists(f):
        with open(f) as fh:
            return json.load(fh)
    return None


def profile_units(profile):
    if profile == "gate":
        return [u for u in UNITS if u["profile"] == "gate"]
    return list(UNITS)

# ---------------------------------------------------------------- the snapshot


def snapshot(dirty):
    """The tree to run on, in SNAP: HEAD, or HEAD plus the working tree's changes. Returns (commit, tree)."""
    head = git("rev-parse", "HEAD").strip()
    if not os.path.isdir(os.path.join(SNAP, ".git")):
        os.makedirs(HOME, exist_ok=True)
        subprocess.run(["git", "clone", "-q", "--shared", "--no-checkout", ROOT, SNAP], check=True)
        with open(os.path.join(SNAP, ".git", "info", "exclude"), "a") as f:
            f.write("/stress\n")
    r = subprocess.run(["git", "checkout", "-q", "-f", "--detach", head], cwd=SNAP, capture_output=True)
    if r.returncode != 0:
        # The clone borrows this repository's objects; after a squash and `git gc` they may be gone: clone again,
        # keeping the build directory.
        keep = os.path.join(HOME, "snap-target")
        if os.path.isdir(os.path.join(SNAP, "impl", "target")):
            os.rename(os.path.join(SNAP, "impl", "target"), keep)
        shutil.rmtree(SNAP)
        subprocess.run(["git", "clone", "-q", "--shared", "--no-checkout", ROOT, SNAP], check=True)
        with open(os.path.join(SNAP, ".git", "info", "exclude"), "a") as f:
            f.write("/stress\n")
        git("checkout", "-q", "-f", "--detach", head, cwd=SNAP)
        if os.path.isdir(keep):
            os.rename(keep, os.path.join(SNAP, "impl", "target"))
    git("clean", "-q", "-ffdx", "-e", "/impl/target", "-e", "/stress", "-e", "/proofs/.lake", cwd=SNAP)
    # The local programs some units run (stress/, not tracked), copied in: their scripts reach the tools by
    # relative paths (`../../impl/target/release/cobc`), which must lead to the snapshot's tools.
    sdir = os.path.join(SNAP, "stress")
    if os.path.islink(sdir):
        os.remove(sdir)
    if os.path.isdir(os.path.join(ROOT, "stress")):
        os.makedirs(sdir, exist_ok=True)
        parts = [p for p in STRESS_PARTS if os.path.exists(os.path.join(ROOT, "stress", p))]
        subprocess.run(["rsync", "-a", "--delete", "--relative", "--exclude", "build/", "--exclude", "cache/",
                        "--max-size=20M"] + ["./" + p for p in parts] + [sdir + "/"],
                       cwd=os.path.join(ROOT, "stress"), check=True)
    if dirty:
        raw = git("status", "--porcelain=v1", "-z", "--untracked-files=all", binary=True).split(b"\0")
        i = 0
        while i < len(raw):
            e = raw[i].decode()
            i += 1
            if not e:
                continue
            xy, path = e[:2], e[3:]
            if xy[0] in "RC":
                orig = raw[i].decode()
                i += 1
                if xy[0] == "R" and os.path.lexists(os.path.join(SNAP, orig)):
                    os.remove(os.path.join(SNAP, orig))
            src, dst = os.path.join(ROOT, path), os.path.join(SNAP, path)
            if os.path.lexists(src) and not os.path.isdir(src):
                os.makedirs(os.path.dirname(dst), exist_ok=True)
                shutil.copy2(src, dst)
            elif os.path.lexists(dst):
                os.remove(dst)
        git("add", "-A", ".", cwd=SNAP)
    tree = git("write-tree", cwd=SNAP).strip()
    return head, tree

# ---------------------------------------------------------------- running


def limits():
    resource.setrlimit(resource.RLIMIT_CPU, (40000, 40000))
    os.setsid()


# The process groups of the units running now: a gate stopped by a signal stops them too.
RUNNING = set()


def session_pids(sid):
    """Every process in session SID (a unit runs in a session of its own; `timeout` makes process groups of its
    own inside it, so a group is not enough)."""
    out = []
    for d in os.listdir("/proc"):
        if d.isdigit():
            try:
                with open("/proc/%s/stat" % d) as f:
                    fields = f.read().rsplit(")", 1)[1].split()
                if int(fields[3]) == sid:
                    out.append(int(d))
            except (OSError, IndexError, ValueError):
                pass
    return out


def stop_group(sid):
    for sig in (signal.SIGTERM, signal.SIGKILL):
        pids = session_pids(sid)
        if not pids:
            return
        for pid in pids:
            try:
                os.kill(pid, sig)
            except ProcessLookupError:
                pass
        time.sleep(5)


def on_signal(signum, frame):
    for pgid in list(RUNNING):
        stop_group(pgid)
    raise SystemExit("gate: stopped by signal %d; the running unit was stopped with it" % signum)


def run_cmd(cmd, log, limit, env, cwd=SNAP):
    with open(log, "ab") as out:
        p = subprocess.Popen(["bash", "-c", cmd], cwd=cwd, stdout=out, stderr=subprocess.STDOUT, env=env, preexec_fn=limits)
        RUNNING.add(p.pid)
        try:
            return p.wait(timeout=limit)
        except subprocess.TimeoutExpired:
            stop_group(p.pid)
            p.wait()
            with open(log, "a") as f:
                f.write("\n[gate: killed after %d s]\n" % limit)
            return 124
        finally:
            RUNNING.discard(p.pid)


def unit_env(G):
    env = dict(os.environ)
    env.update(G=G, COBALTC_CPU_LIMIT="600", COBALTC_RESULT_STORE=os.path.join(STORE, "cases"),
               COBALTC_TOOLCHAIN_KEY=toolchain_key(SNAP), GATE_SELFHOST_KEY=toolchain_key(SNAP, SELFHOST), PATH=os.path.expanduser("~/.elan/bin") + ":" + env.get("PATH", ""))
    env.pop("COBALTC_TEST_SLOW", None)
    env.pop("COBALTC_TEST_SHARD", None)
    return env


def log_has(log, pattern):
    with open(log, errors="replace") as f:
        return re.search(pattern, f.read(), re.M) is not None

# ---- python units


def case_key(selfhost_key, path):
    blob = git("ls-files", "-s", "--", path, cwd=SNAP).split(" ")
    return hashlib.sha256(("%s\0%s\0%s" % (selfhost_key, path, blob[1] if len(blob) > 1 else "")).encode()).hexdigest()


def unit_selfhost(G, env, log):
    """cbc writes cobc's C for every tracked .cb file (compare_c.sh), a file's `same` kept by the hash of the
    toolchain, the self-hosted sources and the file; the formatters and the parsers on the showcases and the
    surface-syntax cases (the whole corpus is the release profile's `selfhost-whole`)."""
    key = toolchain_key(SNAP, SELFHOST)
    d = os.path.join(STORE, "selfhost-c")
    files = [f for f in git("ls-files", "*.cb", cwd=SNAP).split("\n") if f]
    todo = [f for f in files if not os.path.exists(os.path.join(d, case_key(key, f)))]
    with open(log, "a") as fh:
        fh.write("compare_c: %d files, %d kept, %d to compare\n" % (len(files), len(files) - len(todo), len(todo)))
    out = os.path.join(G, "selfhost-c")
    listf = os.path.join(G, "selfhost-c.files")
    with open(listf, "w") as fh:
        fh.write("\n".join(todo) + "\n")
    rc = run_cmd("OUT=%s timeout -k 30 9000 impl/selfhost/compare_c.sh $(cat %s) 2>&1 | tail -3; exit ${PIPESTATUS[0]}" % (out, listf),
                 log, 10000, env) if todo else 0
    res = os.path.join(out, "results")
    if os.path.exists(res):
        os.makedirs(d, exist_ok=True)
        for line in open(res):
            parts = line.split()
            if len(parts) >= 2 and parts[0] in ("same", "skip") and parts[-1] in todo:
                open(os.path.join(d, case_key(key, parts[-1])), "w").close()
    small = "$(git ls-files 'showcase/*.cb' 'impl/conformance/22-surface-syntax/*.cb')"
    rc2 = run_cmd("OUT=$G/selfhost-fmt timeout -k 30 3600 impl/selfhost/compare.sh -m plain,narrow,messy %s 2>&1 | tail -2; exit ${PIPESTATUS[0]}" % small, log, 4000, env)
    rc3 = run_cmd("OUT=$G/selfhost-parse timeout -k 30 3600 impl/selfhost/compare_parser.sh %s 2>&1 | tail -2; exit ${PIPESTATUS[0]}" % small, log, 4000, env)
    return 0 if rc == 0 and rc2 == 0 and rc3 == 0 else 1


def baseline(name):
    f = os.path.join(STORE, "baselines", name + ".json")
    return json.load(open(f)) if os.path.exists(f) else None


def save_baseline(name, data):
    os.makedirs(os.path.join(STORE, "baselines"), exist_ok=True)
    with open(os.path.join(STORE, "baselines", name + ".json"), "w") as f:
        json.dump(data, f, indent=0, sort_keys=True)


def sweep_list():
    """Every tracked program with a main, and the local stress programs (stress/, not tracked) listed in the
    decided baseline the sweep started from (stress/d0281/objectless/decided.list)."""
    files = [f for f in git("ls-files", "*.cb", cwd=SNAP).split("\n") if f]
    out = []
    for f in files:
        try:
            if re.search(r"^fn main\b", open(os.path.join(SNAP, f), errors="replace").read(), re.M):
                out.append(f)
        except OSError:
            pass
    dl = os.path.join(ROOT, "stress", "d0281", "objectless", "decided.list")
    if os.path.exists(dl):
        for line in open(dl):
            p = line.strip()
            rel = os.path.relpath(p, ROOT) if p.startswith(ROOT) else p
            if rel.startswith("stress/") and os.path.exists(os.path.join(ROOT, rel)):
                out.append(rel)
    return sorted(set(out))


def sweep(G, env, log):
    """Whether cobc uses its decided lowering (CB_DECIDED in the C) for each program, three at a time."""
    from concurrent.futures import ThreadPoolExecutor
    cobc = os.path.join(SNAP, "impl", "target", "release", "cobc")

    def one(rel):
        # A local stress program is read where it is (only some of stress/ is copied into the snapshot).
        p = os.path.join(ROOT if rel.startswith("stress/") else SNAP, rel)
        # 300 s: the largest program, impl/selfhost/cbc/main.cb (167 MB of C), takes about 120.
        try:
            r = subprocess.run([cobc, "--emit-c", os.path.basename(p)], cwd=os.path.dirname(p), capture_output=True, timeout=300,
                               env=env, preexec_fn=lambda: resource.setrlimit(resource.RLIMIT_CPU, (300, 300)))
        except subprocess.TimeoutExpired:
            return rel, "EMITFAIL"
        if r.returncode != 0:
            return rel, "EMITFAIL"
        return rel, "DECIDED" if b"CB_DECIDED" in r.stdout[:4000] else "UNDECIDED"

    # A program's result is kept under the hash of the lowering's inputs and the program's own text: the sweep runs
    # again only for what changed.
    key = toolchain_key(SNAP, BY_NAME["lowering"]["inputs"])
    d = os.path.join(STORE, "sweep")

    def cached(rel):
        try:
            text = open(os.path.join(ROOT if rel.startswith("stress/") else SNAP, rel), "rb").read()
        except OSError:
            return rel, "EMITFAIL"
        h = hashlib.sha256(key.encode() + b"\0" + rel.encode() + b"\0" + text).hexdigest()
        f = os.path.join(d, h[:2], h)
        if os.path.exists(f):
            return rel, open(f).read().strip()
        rel, verdict = one(rel)
        os.makedirs(os.path.dirname(f), exist_ok=True)
        with open(f, "w") as fh:
            fh.write(verdict + "\n")
        return rel, verdict

    with ThreadPoolExecutor(3) as ex:
        return dict(ex.map(cached, sweep_list()))


def instr_rows(text, pattern):
    rows = {}
    for line in text.split("\n"):
        m = re.match(pattern, line)
        if m:
            rows[m.group(1)] = int(m.group(2).replace(",", ""))
    return rows


def unit_lowering(G, env, log):
    """The undecided sweep, the perf guide and the bench, each compared with the last green run's numbers (the
    first time: the numbers the project kept by hand). Decided programs stay decided; no instruction count rises
    by more than 3%."""
    red = []
    w = lambda s: open(log, "a").write(s + "\n")
    # 1. sweep
    now = sweep(G, env, log)
    json.dump(now, open(os.path.join(G, "sweep.json"), "w"), indent=0, sort_keys=True)
    base = baseline("sweep")
    if base is None:
        base = {}
        dl = os.path.join(ROOT, "stress", "d0281", "objectless", "decided.list")
        for line in (open(dl) if os.path.exists(dl) else []):
            p = line.strip()
            base[os.path.relpath(p, ROOT) if p.startswith(ROOT) else p] = "DECIDED"
    lost = sorted(f for f, v in now.items() if v != "DECIDED" and base.get(f) == "DECIDED")
    counts = {}
    for v in now.values():
        counts[v] = counts.get(v, 0) + 1
    w("sweep: %s; %d decided before and not now" % (", ".join("%d %s" % (n, k) for k, n in sorted(counts.items())), len(lost)))
    for f in lost[:40]:
        w("  LOST %s %s" % (now[f], f))
    if lost:
        red.append("sweep")
    # 2. perf guide
    pg = os.path.join(SNAP, "stress", "perfguide", "run.sh")
    perf = {}
    if os.path.exists(pg):
        out = os.path.join(G, "perfguide.txt")
        rc = run_cmd("cd stress/perfguide && timeout -k 10 3600 ./run.sh > %s 2>&1" % out, log, 4000, env)
        perf = instr_rows(open(out).read(), r"^(\S+\s+\S+\s+\S+)\s+[\d.]+ s\s+(\d+) instr")
        if rc != 0 or not perf:
            red.append("perfguide (exit %d)" % rc)
        pbase = baseline("perfguide") or instr_rows(open(os.path.join(ROOT, "stress", "perfguide", "results.txt")).read(),
                                                    r"^(\S+\s+\S+\s+\S+)\s+[\d.]+ s\s+(\d+) instr")
        worse = [(k, pbase[k], v) for k, v in perf.items() if k in pbase and v > pbase[k] * 1.03]
        w("perf guide: %d rows, %d more than 3%% above the baseline" % (len(perf), len(worse)))
        for k, a, b in worse:
            w("  SLOWER %s %d -> %d (%+.1f%%)" % (re.sub(r"\s+", " ", k), a, b, 100.0 * (b - a) / a))
        if worse:
            red.append("perfguide")
    # 3. bench
    res = os.path.join(G, "bench-RESULTS.md")
    rc = run_cmd("cd bench && RESULTS=%s timeout -k 10 3600 ./run.sh > $G/bench.log 2>&1" % res, log, 4000, env)
    bench = {}
    if os.path.exists(res):
        sec = None
        for line in open(res):
            if line.startswith("## "):
                sec = line.strip()
            if sec == "## Instructions executed" and line.startswith("| `"):
                c = [x.strip() for x in line.strip().strip("|").split("|")]
                for col, v in (("gcc", c[1]), ("clang", c[2])):
                    if re.fullmatch(r"[\d,]+", v):
                        bench["%s %s" % (c[0], col)] = int(v.replace(",", ""))
    if rc != 0 or not bench:
        red.append("bench (exit %d)" % rc)
    bbase = baseline("bench")
    if bbase is None:
        first = os.path.join(ROOT, "stress", "gate", "bench-baseline.json")
        bbase = json.load(open(first)) if os.path.exists(first) else {}
    worse = [(k, bbase[k], v) for k, v in bench.items() if k in bbase and v > bbase[k] * 1.03]
    w("bench: %d cobc counts, %d more than 3%% above the baseline" % (len(bench), len(worse)))
    for k, a, b in worse:
        w("  SLOWER %s %d -> %d (%+.1f%%)" % (k, a, b, 100.0 * (b - a) / a))
    if worse:
        red.append("bench")
    if red:
        w("red: " + ", ".join(red))
        return 1
    save_baseline("sweep", now)
    if perf:
        save_baseline("perfguide", perf)
    save_baseline("bench", bench)
    return 0

# ---------------------------------------------------------------- housekeeping

KEEP_RUNS = 12          # run directories kept (the newest)
KEEP_DAYS = 30          # a per-case result or verdict not written for this long is dropped (it only runs again)


def prune():
    """Keeps the store and the runs bounded: the newest KEEP_RUNS run directories, and per-case results and
    verdicts written in the last KEEP_DAYS days (a result reused is rewritten by nobody, so an old one that is
    still valid is dropped too and simply runs again once). Returns the bytes freed."""
    freed = 0
    runs = sorted(d for d in os.listdir(RUNS) if os.path.isdir(os.path.join(RUNS, d))) if os.path.isdir(RUNS) else []
    for d in runs[:-KEEP_RUNS]:
        p = os.path.join(RUNS, d)
        for dp, _, fs in os.walk(p):
            freed += sum(os.path.getsize(os.path.join(dp, f)) for f in fs if os.path.isfile(os.path.join(dp, f)))
        shutil.rmtree(p, ignore_errors=True)
    cutoff = time.time() - KEEP_DAYS * 86400
    for sub in ("cases", "units", "selfhost-c", "sweep"):
        top = os.path.join(STORE, sub)
        for dp, _, fs in os.walk(top):
            for f in fs:
                p = os.path.join(dp, f)
                try:
                    st = os.stat(p)
                    if st.st_mtime < cutoff:
                        freed += st.st_size
                        os.remove(p)
                except OSError:
                    pass
    return freed

# ---------------------------------------------------------------- the driver


def stray():
    r = subprocess.run(["ps", "-eo", "pid,args"], capture_output=True, text=True).stdout
    mine = os.path.join(SNAP, "impl", "target")
    return [l for l in r.split("\n") if re.search(r"target/(release|debug)/(coby|cobc|cobfmt)( |$)|cargo (test|build|check)", l)
            and mine not in l and "gate.py" not in l]


def main(argv):
    for sig in (signal.SIGTERM, signal.SIGINT, signal.SIGHUP):
        signal.signal(sig, on_signal)
    dirty = "--dirty" in argv
    argv = [a for a in argv if a != "--dirty"]
    if argv[:1] == ["--list"]:
        prof = argv[1] if len(argv) > 1 else "release"
        for u in profile_units(prof):
            v = kept(ROOT, u)
            print("%-15s %-8s %s %s" % (u["name"], u["profile"], unit_hash(ROOT, u)[:12],
                                         "kept (%s, %s)" % (v["when"], v["run"]) if v else "-"))
        return 0
    force = argv[:1] == ["all"]
    if not argv or argv[0] == "gate":
        units, profile = profile_units("gate"), "gate"
    elif argv[0] in ("release", "all"):
        units, profile = profile_units("release"), argv[0]
    else:
        bad = [a for a in argv if a not in BY_NAME]
        if bad:
            raise SystemExit("no such unit: %s (impl/gate.sh --list)" % " ".join(bad))
        units, profile = [BY_NAME[a] for a in argv], "units " + " ".join(argv)
    os.makedirs(HOME, exist_ok=True)
    lock = open(os.path.join(HOME, "lock"), "w")
    try:
        fcntl.flock(lock, fcntl.LOCK_EX | fcntl.LOCK_NB)
    except OSError:
        raise SystemExit("another gate is running (%s)" % os.path.join(HOME, "lock"))
    s = stray()
    if s:
        raise SystemExit("stray processes (the gate owns the machine; stop them first):\n" + "\n".join(s))
    stamp = time.strftime("%Y%m%d_%H%M%S", time.gmtime())
    G = os.path.join(RUNS, stamp)
    os.makedirs(G)
    commit, tree = snapshot(dirty)
    env = unit_env(G)
    progress = os.path.join(G, "progress")

    def say(msg):
        line = "%s %s" % (time.strftime("%H:%M:%S", time.gmtime()), msg)
        print(line, flush=True)
        open(progress, "a").write(line + "\n")

    say("gate %s on %s %s (tree %s), run %s" % (profile, "the working tree over" if dirty else "commit", commit[:12], tree[:12], G))
    todo = []
    for u in units:
        v = None if force else kept(SNAP, u)
        if v:
            say("%-15s kept: green on %s (run %s)" % (u["name"], v["tree"][:12], v["run"]))
        else:
            todo.append(u)
    if todo and not any(u["name"] == "build" for u in todo):
        todo.insert(0, BY_NAME["build"])
    results = []
    for u in todo:
        log = os.path.join(G, u["name"] + ".log")
        t0 = time.time()
        say("%-15s start" % u["name"])
        if u.get("py"):
            rc = globals()[u["py"]](G, env, log)
        else:
            rc = run_cmd(u["cmd"], log, u["limit"], env)
        if rc == 0 and u.get("fail") and log_has(log, u["fail"]):
            rc = 1
        dur = int(time.time() - t0)
        ok = rc == 0
        tail = open(log, errors="replace").read().strip().split("\n")[-1:] if os.path.exists(log) else [""]
        say("%-15s %s, %d min%s  %s" % (u["name"], "green" if ok else ("red (advisory)" if u.get("advisory") else "RED"),
                                        (dur + 30) // 60, "" if ok else " (exit %d)" % rc, tail[0][:100]))
        results.append((u, ok, dur))
        if ok:
            h = unit_hash(SNAP, u)
            os.makedirs(os.path.dirname(verdict_file(u, h)), exist_ok=True)
            with open(verdict_file(u, h), "w") as f:
                json.dump(dict(result="green", unit=u["name"], tree=tree, commit=commit, dirty=dirty, run=stamp,
                               when=time.strftime("%Y-%m-%d %H:%M UTC", time.gmtime()), seconds=dur,
                               toolchain=tool_versions()), f, indent=1)
        elif u["name"] == "build":
            say("the build is red: nothing else can run")
            break
    red = [u["name"] for u, ok, _ in results if not ok and not u.get("advisory")]
    gating = [u for u in units if not u.get("advisory")]
    green_all = not red and all(kept(SNAP, u) for u in gating)
    with open(os.path.join(G, "SUMMARY"), "w") as f:
        f.write("run %s\nprofile %s\ncommit %s\ntree %s\ndirty %s\nresult %s\n" % (
            stamp, profile, commit, tree, dirty, "GREEN" if green_all else "RED: " + " ".join(red)))
    with open(os.path.join(G, "REPORT.md"), "w") as f:
        f.write("# Gate %s, %s\n\ncommit `%s`%s, tree `%s`\n\n| unit | profile | result | minutes |\n|---|---|---|---|\n" % (
            stamp, profile, commit[:12], " + working tree" if dirty else "", tree[:12]))
        ran = {u["name"]: (ok, d) for u, ok, d in results}
        for u in ([] if any(x["name"] == "build" for x in units) else [BY_NAME["build"]]) + units:
            if u["name"] in ran:
                ok, d = ran[u["name"]]
                f.write("| %s | %s | %s | %d |\n" % (u["name"], u["profile"], "green" if ok else "**red**", (d + 30) // 60))
            else:
                f.write("| %s | %s | kept | 0 |\n" % (u["name"], u["profile"]))
        f.write("\nLogs: `%s/<unit>.log`.\n" % G)
    last = os.path.join(HOME, "last")
    if os.path.lexists(last):
        os.remove(last)
    os.symlink(G, last)
    freed = prune()
    if freed:
        say("pruned %d MB of old runs and results" % (freed >> 20))
    say("GATE %s%s; report %s" % ("GREEN" if green_all else "RED", "" if green_all else " (" + " ".join(red) + ")",
                                  os.path.join(G, "REPORT.md")))
    return 0 if green_all else 1


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
