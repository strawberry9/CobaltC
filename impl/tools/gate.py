#!/usr/bin/env python3
"""The gate (D-0307): every check the project runs, as units with declared inputs, run on a snapshot of the
tree, a unit's green verdict kept under the hash of its inputs so that it never runs again on the same inputs.

  impl/gate.sh                  # the gate profile on HEAD: what a commit and a push need
  impl/gate.sh release          # the gate and the release profile's slow units (the owner says when)
  impl/gate.sh all              # every unit of both profiles, the store of verdicts ignored
  impl/gate.sh UNIT...          # those units only (names: impl/gate.sh --list)
  impl/gate.sh --dirty ...      # the working tree as it is, not HEAD (for work in progress; a push needs HEAD)
  impl/gate.sh --list [PROFILE] # the units, their inputs' hash on HEAD and whether a green verdict is kept
  impl/gate.sh --no-pregate ... # skip impl/tools/pregate.py, which a profile's run makes first (CHG-0475)

A unit's inputs are tracked paths (git pathspecs) plus the versions of gcc and clang and the unit's own
command; README.md and spec/MANIFEST.md are outputs and never inputs. Units run one after another, each with at
most three processes, a CPU limit per process and a wall-clock limit; one gate runs at a time (a lock). The
corpus runners keep a result per case as well (`COBALTC_RESULT_STORE`, `impl/tests/cbtest/store.cb`), so a unit that
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
TOOLCHAIN = ["impl/front", "impl/back", "impl/cobc", "impl/cbrt", "impl/std", "impl/tests", "impl/build.sh",
             "impl/bootstrap", "spec/registry",
             # texts beside the tests: not tools
             ":(exclude)impl/tests/*.md"]
# The conformance cases and their fixtures (and their recorded runs), not the corpus's own runner or its documentation.
CORPUS = ["impl/conformance", ":(exclude)impl/conformance/run.py", ":(exclude)impl/conformance/*.md"]
EVERYTHING = ["."]

# The parts of the local stress/ directory the units run (the perf guide, the release profile's programs).
STRESS_PARTS = ["run.sh", "perfguide", "datetime", "paths", "random", "process", "net", "d0166", "round5", "d0173", "dot", "crypto"]

CBTEST_FAILED = r"^  FAILED |^  DISAGREE "
B = "impl/target/release"
T = "impl/target/tests"

# ---------------------------------------------------------------- the units
#
# name, profile ("gate" or "release"), inputs (pathspecs), command (bash, run from the snapshot's root with
# $G the run directory), wall-clock limit in seconds; optional: "fail" (a regex that makes a log red),
# "advisory" (red is recorded, not gating), "py" (a function of this file instead of a command).

UNITS = [
    # D-0326: the toolchain is built with the compiler the snapshot's last build left (impl/build.sh), or, when
    # none was built that way (no cobc.stamp: the first time, or a compiler from before D-0326), from the seed
    # (impl/bootstrap/boot.c); then what is generated is current.
    dict(name="build", profile="gate", inputs=EVERYTHING, limit=5400, always=True,
         cmd="set -eo pipefail; if [ ! -f " + B + "/cobc.stamp ]; then gcc impl/bootstrap/boot.c -o $G/boot && $G/boot --no-verify > $G/boot.log 2>&1; fi; "
             # The compiler; then the other programs two at a time, beside the check that the committed seed is
             # the C of the committed compiler source (the new compiler writing its own C again).
             "PROGRAMS=cobc impl/build.sh > $G/build-tools.log 2>&1 || { tail -20 $G/build-tools.log; exit 1; }; "
             "(COBC=$PWD/" + B + "/cobc impl/bootstrap/mkseed.sh --check > $G/seed-check.log 2>&1) & seed=$!; "
             "PROGRAMS='cobfmt cobmodel cbparse cbtest difftest' impl/build.sh >> $G/build-tools.log 2>&1 || { tail -20 $G/build-tools.log; exit 1; }; "
             "tail -1 $G/build-tools.log; "
             # The generated view of `std` is current, as cobc writes it.
             + B + "/cobc --prelude-summary impl/std | cmp - impl/front/prelude.cb && echo prelude current; "
             "impl/back/gen_std_values.sh --check && echo std_values current; "
             # CHG-0475: the registry cobc renders diagnostics from is the committed one.
             "python3 -I impl/cobc/gen_build_info.py --check && echo build_info current; "
             # The runtime's table of diagnostics is the registry's, and its interface cbrt.h's.
             "python3 -I impl/cbrt/tools/gen_diag_table.py spec/registry/diagnostics.md $G/diag_table.h; "
             "cmp $G/diag_table.h impl/cbrt/src/diag_table.h; python3 -I impl/cbrt/tools/sigcheck.py " + B + "/libcbrt.a > $G/sigcheck.txt; "
             "tail -2 $G/sigcheck.txt; "
             "wait $seed || { cat $G/seed-check.log; exit 1; }; tail -1 $G/seed-check.log"),
    dict(name="format", profile="gate", limit=1800,
         inputs=["impl/cobfmt", "impl/front", ":(glob)**/*.cb"],
         cmd="rc=0; F=" + B + "/cobfmt; "
             "for d in showcase impl/cobaltc_examples bench impl/std book/examples impl/front impl/back impl/cobc impl/cobfmt impl/cbparse impl/model impl/tests/cbtest impl/tests/differential impl/fuzz; do "
             "  o=$(timeout -k 10 600 $F --check $d 2>&1) || { echo \"$d: $o\"; rc=1; }; done; "
             "for d in impl/conformance impl/tests/programs; do "
             "o=$(timeout -k 10 600 $F --check $d 2>&1 | grep -v 'does not parse'); "
             "[ -n \"$o\" ] && { echo \"$d: $o\"; rc=1; }; done; exit $rc"),
    # D-0326: the formatter's own tests.
    dict(name="formatter", profile="gate", limit=3600,
         inputs=["impl/cobfmt", "impl/front", "showcase", "impl/cobaltc_examples", "bench", *CORPUS],
         cmd="timeout -k 10 3000 python3 -I impl/cobfmt/tests.py 2>&1 | tail -15; exit ${PIPESTATUS[0]}"),
    dict(name="records", profile="gate", limit=1800, inputs=EVERYTHING,
         cmd="set -eo pipefail; python3 impl/tools/speccheck.py; python3 impl/tools/proofcover.py --check; "
             "python3 impl/tools/assurance.py --check; python3 impl/tools/runtime_contract.py --check; "
             "python3 impl/tools/cobdoc.py --check; python3 impl/tools/std_coverage.py; "
             # CHG-0447: CARD.md is current; CHG-0445: each habit's message is as recorded.
             "python3 impl/tools/card.py --check; "
             "impl/tests/foreign_forms/run.sh --compare impl/tests/foreign_forms/expected.txt; "
             # CHG-0452: what the cold drafts get.
             "impl/tests/cold_drafts/run.sh --compare impl/tests/cold_drafts/expected.txt; "
             # D-0326: what the tools' tests check of the records.
             "python3 -I impl/tests/checks/conf_coverage.py; python3 -I impl/tests/checks/doc_matches_cobdoc.py --cobc " + B + "/cobc; "
             # CHG-0459: the compiler reads deeply nested expressions.
             "python3 -I impl/tests/checks/deep_expressions.py --cobc " + B + "/cobc; "
             # CHG-0461: the settings in the key cbtest keeps a result under are the ones the sources read.
             "python3 -I impl/tests/checks/store_env.py; "
             "for g in impl/tests/gen/*.py; do python3 -I $g --check; done"),
    # D-0326: the Windows build: the runtime by MinGW-w64 and by Clang without a warning, and a program compiled
    # for Windows run under Wine. (The whole suite runs on Windows itself, GitHub's runner, at every push; the
    # release profile's `wine` unit, the suite under Wine, was taken out by CHG-0467.)
    dict(name="portable", profile="gate", limit=3600,
         inputs=["impl/cbrt", "impl/cobc", "impl/back", "impl/front", "impl/std"],
         cmd="set -eo pipefail; "
             "for c in x86_64-w64-mingw32-gcc clang; do impl/cbrt/build.sh --cc $c --out $G/cbrt-$c > $G/cbrt-$c.log 2>&1 "
             "|| { tail -5 $G/cbrt-$c.log; exit 1; }; if grep -a 'warning:' $G/cbrt-$c.log; then exit 1; fi; tail -1 $G/cbrt-$c.log; done; rm -rf $G/cbrt-*/; "
             "printf 'import std;\\n\\nfn main()\\n{\\n    printf(\"%%v\\\\n\", 6 * 7);\\n}\\n' > $G/w.cb; "
             "if command -v wine64 > /dev/null; then (cd $G && COBALTC_RUNNER=wine64 WINEDEBUG=-all timeout -k 10 600 $OLDPWD/" + B + "/cobc --target x86_64-pc-windows-gnu --run w.cb) | grep -qx 42 "
             "&& echo 'a Windows program runs under Wine'; else echo 'wine64 not installed: the Windows program not run'; fi"),
    dict(name="sweep-check", profile="gate", limit=3600,
         inputs=TOOLCHAIN + ["showcase", "impl/cobaltc_examples", "bench"],
         cmd="set -eo pipefail; timeout -k 10 1800 " + B + "/cobc --check showcase impl/cobaltc_examples bench"),
    # D-0326: the tools' own cases, as files cbtest runs.
    dict(name="tests", profile="gate", limit=10800, fail=CBTEST_FAILED,
         inputs=TOOLCHAIN + ["showcase", "spec/conformance.md", "spec/examples.md", "book/examples"],
         cmd="rc=0; pids=; for i in 1 2 3; do "
             "timeout -k 30 9000 " + T + "/cbtest cases --root impl/tests/cases --cobc " + B + "/cobc --shard $i/3 > $G/tests-$i.log 2>&1 & pids=\"$pids $!\"; done; "
             "for p in $pids; do wait $p || rc=1; done; cat $G/tests-[123].log > $G/tests.log; "
             "grep -aE 'FAILED|cbtest cases' $G/tests.log | head -40; exit $rc"),
    # The corpus: every conformance case, every row of the specification's table and the tools' own program
    # cases (impl/tests/programs), compiled under GCC and Clang, each against its header and its recorded run
    # (`NAME.expect`, D-0326 phase E), each of the three in three shards at a time.
    dict(name="corpus", fail=CBTEST_FAILED, profile="gate", limit=14400,
         inputs=TOOLCHAIN + [*CORPUS, "spec/conformance.md", "spec/examples.md"],
         cmd="rc=0; pids=; for i in 1 2 3; do "
             "timeout -k 30 9000 " + T + "/cbtest conformance --cobc " + B + "/cobc --shard $i/3 > $G/corpus-suite-$i.log 2>&1 & pids=\"$pids $!\"; done; "
             "for p in $pids; do wait $p || rc=1; done; pids=; for i in 1 2 3; do "
             "timeout -k 30 9000 " + T + "/cbtest rows --cobc " + B + "/cobc --shard $i/3 > $G/corpus-rows-$i.log 2>&1 & pids=\"$pids $!\"; done; "
             "for p in $pids; do wait $p || rc=1; done; pids=; for i in 1 2 3; do "
             "timeout -k 30 9000 " + T + "/cbtest conformance --cobc " + B + "/cobc --root impl/tests/programs --shard $i/3 > $G/corpus-programs-$i.log 2>&1 & pids=\"$pids $!\"; done; "
             "for p in $pids; do wait $p || rc=1; done; "
             "grep -ahE '^cbtest|FAILED' $G/corpus-*.log | head -40; exit $rc"),
    dict(name="std-tests", profile="gate", limit=5400, inputs=TOOLCHAIN,
         cmd="set -eo pipefail; cd $G; printf 'fn main()\\n{\\n}\\n' > std-tests.cb; R=$OLDPWD; "
             "for cc in gcc clang; do timeout -k 10 1800 $R/" + B + "/cobc --unittest-std --cc $cc --run std-tests.cb < /dev/null | tail -2; done"),
    dict(name="guide", profile="gate", limit=7200, inputs=TOOLCHAIN + ["htmlguide/generator"],
         cmd="cd htmlguide/generator && JOBS=3 timeout -k 30 5400 python3 gen.py --check > $G/guide.log 2>&1; rc=$?; grep -a FAIL $G/guide.log | head -20; tail -1 $G/guide.log; exit $rc"),
    dict(name="book", profile="gate", limit=3600, inputs=TOOLCHAIN + ["impl/cobfmt", "impl/front", "book"],
         cmd="cd book && LIMIT=300 timeout -k 30 3000 ./check.sh 2>&1 | tail -15; exit ${PIPESTATUS[0]}"),
    dict(name="showcase", profile="gate", limit=7200, inputs=TOOLCHAIN + ["showcase"],
         cmd="cd showcase && JOBS=3 LIMIT=900 timeout -k 30 7000 ./run_all.sh > $G/showcase.log 2>&1; rc=$?; grep -aE 'DIFF|FAIL|WRONG' $G/showcase.log | head -20; tail -3 $G/showcase.log; exit $rc"),
    dict(name="model", profile="gate", limit=10800,
         inputs=TOOLCHAIN + ["impl/model", "impl/cbparse", "proofs", "impl/tools/certify_model.py", "impl/tools/factcheck.py", "impl/tools/validate_c.py",
                             "impl/tools/replay_model.py", "impl/tools/replay_runtime.py", "showcase", "impl/conformance/14-control-flow", "spec/23-formal-model.md"],
         # The proofs first: the replays run the machine `lake build` makes (proofs/.lake/build/bin/replay).
         cmd="set -eo pipefail; S=impl/model/sample; "
             "(cd proofs && export PATH=$HOME/.elan/bin:$PATH && timeout -k 30 1800 lake build > $G/lake.log 2>&1); tail -1 $G/lake.log; "
             "if grep -q \"declaration uses 'sorry'\" $G/lake.log; then echo 'a proof uses sorry'; exit 1; fi; "
             "timeout -k 30 5400 " + B + "/cobmodel compare $S --cobc " + B + "/cobc --explore 8 --cc gcc --cc clang --jobs 3 --report $G/model-sample.txt | tail -3; "
             "grep -q ' 0 with an outcome outside' $G/model-sample.txt; "
             # The readings share nothing but their inputs: three chains at a time, the longest alone.
             "rc=0; "
             "(timeout -k 30 3600 python3 impl/tools/replay_runtime.py $S --out $G/replay-runtime --check > $G/model-1.log 2>&1) & a=$!; "
             "(timeout -k 30 3600 python3 impl/tools/validate_c.py $S showcase --out $G/validate-c --check > $G/model-2.log 2>&1 "
             "&& timeout -k 30 1800 python3 impl/tools/certify_model.py $S --out $G/certify --check > $G/model-3.log 2>&1) & b=$!; "
             "(timeout -k 30 3600 python3 impl/tools/factcheck.py $S showcase impl/conformance/14-control-flow --out $G/factcheck --check > $G/model-4.log 2>&1 "
             "&& timeout -k 30 1800 python3 impl/tools/replay_model.py $S --out $G/replay --check > $G/model-5.log 2>&1 "
             "&& timeout -k 30 1800 " + B + "/cobmodel check --as-licensed $S/*.cb > $G/model-6.log 2>&1) & c=$!; "
             "for p in $a $b $c; do wait $p || rc=1; done; "
             "tail -q -n 2 $G/model-6.log; tail -q -n 1 $G/model-3.log $G/model-4.log $G/model-2.log $G/model-5.log $G/model-1.log; exit $rc"),
    # D-0326 §6, in place of the two toolchains compared: a clone of the commit built from the seed alone, by GCC
    # and by Clang (the two side by side), each compiler proving it writes the seed again (the fixed point, the
    # C compilers against each other), then a program run with each.
    dict(name="bootstrap", profile="gate", limit=7200, inputs=EVERYTHING,
         cmd="rc=0; pids=; for c in gcc clang; do rm -rf $G/boot-$c; git clone -q --shared . $G/boot-$c || exit 1; "
             "(cd $G/boot-$c && $c impl/bootstrap/boot.c -o boot && CC=$c timeout -k 30 3000 ./boot > $G/boot-$c.log 2>&1) & pids=\"$pids $!\"; done; "
             "for p in $pids; do wait $p || rc=1; done; "
             "for c in gcc clang; do grep -a 'byte for byte' $G/boot-$c.log || { tail -20 $G/boot-$c.log; rc=1; }; rm -rf $G/boot-$c; done; exit $rc"),
    # D-0326 §6: the compiler against itself on random programs: the default form, every check and the full
    # record, under GCC and Clang, must behave alike.
    dict(name="differential", profile="gate", limit=7200, inputs=TOOLCHAIN,
         cmd="set -eo pipefail; mkdir -p $G/diff/tmp; TMPDIR=$G/diff/tmp timeout -k 30 6000 " + T + "/difftest --self --cobc $PWD/" + B + "/cobc "
             "--n 100 --seed 7 --jobs 3 --work $G/diff/work 2>&1 | tail -6"),
    dict(name="lowering", profile="gate", limit=10800,
         inputs=["impl/front", "impl/back", "impl/cobc", "impl/cbrt", "impl/std", "bench"], py="unit_lowering"),
    # ---- the release profile: slow, and for classes of fault the gate's units have not been the ones to find
    # The bootstrap form (D-0274): cobc written as the bootstrap form and checked in it, every check on in stage 2.
    # Level 10: stage 2 is the compiler compiling itself with every check on: hours alone on the machine (264
    # minutes on 2026-11-16, 171 on 2026-11-19 with `CHG-0471`; a run's report has the unit's time), more
    # beside other work.
    dict(name="fixpoint", profile="release", limit=30000, inputs=TOOLCHAIN,
         cmd="OUT=$G/fixpoint LEVEL=10 timeout -k 30 28800 impl/tools/fixpoint.sh 2>&1 | tail -3; exit ${PIPESTATUS[0]}"),
    dict(name="all-checks", fail=CBTEST_FAILED, profile="release", limit=10800,
         inputs=TOOLCHAIN + CORPUS,
         cmd="rc=0; pids=; for i in 1 2 3; do "
             "COBALTC_ALL_CHECKS=1 timeout -k 30 9000 " + T + "/cbtest conformance --cobc " + B + "/cobc --cc gcc --shard $i/3 > $G/all-checks-$i.log 2>&1 & pids=\"$pids $!\"; done; "
             "for p in $pids; do wait $p || rc=1; done; cat $G/all-checks-[123].log > $G/all-checks.log; "
             "grep -aE '^cbtest|FAILED' $G/all-checks.log | head -20; exit $rc"),
    dict(name="slow-cases", fail=CBTEST_FAILED, profile="release", limit=14400,
         inputs=TOOLCHAIN + [*CORPUS, "spec/conformance.md", "spec/examples.md"],
         cmd="rc=0; timeout -k 30 7200 " + T + "/cbtest conformance --slow --cobc " + B + "/cobc > $G/slow-suite.log 2>&1 || rc=1; "
             "timeout -k 30 7200 " + T + "/cbtest rows --slow --cobc " + B + "/cobc > $G/slow-rows.log 2>&1 || rc=1; "
             "grep -ahE '^cbtest|FAILED' $G/slow-*.log | head; exit $rc"),
    # D-0326: the runtime and the program instrumented together, and the runtime's bounded proofs.
    dict(name="runtime", profile="release", limit=14400, inputs=TOOLCHAIN + CORPUS,
         cmd="rc=0; (cd impl/cbrt/tests/cbmc && timeout -k 30 3600 ./run.sh > $G/cbmc.log 2>&1) || rc=1; tail -6 $G/cbmc.log; "
             "for s in asan tsan; do timeout -k 30 7200 impl/cbrt/tests/sanitize.sh $s > $G/san-$s.log 2>&1 || rc=1; grep -aE '^conformance:|failed:' $G/san-$s.log | head -5; done; exit $rc"),
    dict(name="verify", profile="release", limit=14400, inputs=TOOLCHAIN + ["impl/conformance"],
         cmd="set -eo pipefail; R=$PWD; cd impl/conformance; "
             "COBALTC_VERIFY=1 timeout -k 10 5400 python3 run.py --jobs 3 --impl \"$R/" + B + "/cobc --run {file}\" 2>&1 | tail -2"),
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
    for cmd in (["gcc", "--version"], ["clang", "--version"]):
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
               COBALTC_TOOLCHAIN_KEY=toolchain_key(SNAP), PATH=os.path.expanduser("~/.elan/bin") + ":" + env.get("PATH", ""))
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
        # 300 s: the largest program, impl/cobc/main.cb (167 MB of C), takes about 120.
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
    # D-0330: one seed for the hash tables' hash, so that a count repeats from run to run.
    env = dict(env, COBALTC_HASH_SEED="1")
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
    # D-0340: a program whose header now expects a static refusal (a fault the check finds) is no
    # decided program to keep: it is refused before any C exists.
    def refused_on_purpose(f):
        try:
            head = open(os.path.join(ROOT, f), errors="replace").read(4000)
        except OSError:
            return False
        return any(l.startswith("// expect:") and l.rstrip().endswith("(static)") for l in head.splitlines())
    # D-0340 (`[Certain-Fault-Refused]`): a program the check now refuses with a fault's diagnostic (one the
    # registry lets a run raise: phase `dynamic` or `both`) faulted on every run that reached it before; it is
    # a certain fault refused, listed apart, not a decided program lost.
    fault_diags = set()
    for line in open(os.path.join(ROOT, "spec", "registry", "diagnostics.md"), errors="replace"):
        m = re.match(r"- `(diag\.[a-z0-9-]+)` — .*?Phase: (dynamic|both)\b", line)
        if m:
            fault_diags.add(m.group(1))
    cobc_now = os.path.join(SNAP, "impl", "target", "release", "cobc")

    def refused_fault(f):
        p = os.path.join(ROOT if f.startswith("stress/") else SNAP, f)
        try:
            r = subprocess.run([cobc_now, "--check", os.path.basename(p)], cwd=os.path.dirname(p), capture_output=True,
                               timeout=300, env=env)
        except (subprocess.TimeoutExpired, OSError):
            return None
        m = re.match(rb"error\[(diag\.[a-z0-9-]+)\]", r.stderr)
        return m.group(1).decode() if r.returncode == 1 and m and m.group(1).decode() in fault_diags else None

    cand = sorted(f for f, v in now.items() if v != "DECIDED" and base.get(f) == "DECIDED" and not refused_on_purpose(f))
    refused = {}
    lost = []
    for f in cand:
        d = refused_fault(f)
        if d:
            refused[f] = d
        else:
            lost.append(f)
    for f in sorted(refused):
        w("  REFUSED %s %s (a certain fault, D-0340)" % (refused[f], f))
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
    return [l for l in r.split("\n") if re.search(r"target/(release|tests)/(cobc|cobfmt|cobmodel|cbtest|difftest)( |$)", l)
            and mine not in l and "gate.py" not in l]


def main(argv):
    for sig in (signal.SIGTERM, signal.SIGINT, signal.SIGHUP):
        signal.signal(sig, on_signal)
    dirty = "--dirty" in argv
    pregate = "--no-pregate" not in argv
    argv = [a for a in argv if a not in ("--dirty", "--no-pregate")]
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
    # CHG-0475: what takes minutes to find (a generated file not regenerated, a feature undocumented) is
    # found before a profile's units start, not by a unit an hour in.
    if pregate and not dirty and (not argv or argv[0] in ("gate", "release", "all")):
        if subprocess.run([sys.executable, os.path.join(ROOT, "impl", "tools", "pregate.py")], cwd=ROOT).returncode != 0:
            raise SystemExit("pregate failed: nothing run (impl/gate.sh --no-pregate to run anyway)")
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
