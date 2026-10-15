#!/usr/bin/env python3
"""Runs the std fuzzer (impl/fuzz/fuzz.cb) to a time budget (WI-7, CHG-0266).

    python3 impl/fuzz/run.py                      # every parser, gcc, clang and clang+ASan/UBSan, 600 s each
    python3 impl/fuzz/run.py --budget 60 --parsers json,text --builds asan
    python3 impl/fuzz/run.py --builds coby --budget 120   # the interpreter, slower per input

Each (parser, build) pair runs `fuzz PARSER COUNT SEED` again and again,
with a new seed each time, until its budget is spent; pairs run in
parallel (--jobs, default the CPU count). A pair fails on a fault the
program reports (its exit status 1, and a fault_PARSER_N.hex input file
written beside it), any other exit status, a sanitizer report, or a run
that outlives its timeout. The output directory (--out, default
stress/fuzz/runs/STAMP when stress/ exists, else ~/.cobaltc-fuzz/runs/STAMP)
holds a log per pair, the inputs that faulted, and SUMMARY. Exit status 1
when any pair failed.

Needs the release tools (cargo build --release -p coby -p cobc -p cbrt).
Linux for the sanitizer build (clang with ASan and UBSan); gcc, clang and
coby builds run wherever the tools do. Standard library only.
"""
import argparse
import concurrent.futures as cf
import datetime
import os
import re
import shutil
import subprocess
import sys
import time

HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.abspath(os.path.join(HERE, "..", ".."))
REL = os.path.join(ROOT, "impl", "target", "release")
EXE = ".exe" if os.name == "nt" else ""
PARSERS = ["inflate", "json", "der", "pem", "text", "http-server", "http-client", "pg", "tls"]
BUILDS = ["gcc", "clang", "asan"]
# Inputs per run: enough to amortize the process, few enough to stop near
# the budget; the network rounds are slower per input.
CHUNK = {"http-server": 300, "http-client": 300, "pg": 200, "tls": 100}
SANITIZER = re.compile(r"Sanitizer|runtime error:")


def out_dir(arg):
    if arg:
        return os.path.abspath(arg)
    stamp = datetime.datetime.utcnow().strftime("%Y%m%d_%H%M%S")
    base = os.path.join(ROOT, "stress", "fuzz", "runs") if os.path.isdir(os.path.join(ROOT, "stress")) else os.path.join(os.path.expanduser("~"), ".cobaltc-fuzz", "runs")
    return os.path.join(base, stamp)


def build(name, out):
    """The fuzzer built for `name`: a command prefix, or None for coby."""
    if name == "coby":
        return [os.path.join(REL, "coby" + EXE), os.path.join(HERE, "fuzz.cb")]
    cc = {"gcc": "gcc", "clang": "clang", "asan": os.path.join(HERE, "clang-asan")}[name]
    exe = os.path.join(out, "bin", "fuzz_" + name + EXE)
    os.makedirs(os.path.dirname(exe), exist_ok=True)
    env = dict(os.environ, COBC_CACHE=os.path.join(out, "cache_" + name))
    r = subprocess.run([os.path.join(REL, "cobc" + EXE), "--cc", cc, "-o", exe, os.path.join(HERE, "fuzz.cb")],
                       capture_output=True, text=True, env=env)
    if r.returncode != 0:
        sys.exit("fuzz: build %s failed:\n%s%s" % (name, r.stdout, r.stderr))
    wrap = ["setarch", "-R"] if name == "asan" and shutil.which("setarch") else []
    return wrap + [exe]


def run_pair(parser, bname, cmd, budget, out, seed0, timeout):
    work = os.path.join(out, "%s_%s" % (parser, bname))
    os.makedirs(work, exist_ok=True)
    log = open(os.path.join(work, "log"), "w")
    chunk = CHUNK.get(parser, 1000)
    if bname == "coby":
        chunk = max(10, chunk // 20)
    env = dict(os.environ, ASAN_OPTIONS="detect_leaks=0", COBALTC_CPU_LIMIT=str(timeout))
    tally = {"ok": 0, "err": 0, "fault": 0, "skipped": 0}
    runs, failed, why = 0, False, ""
    end = time.monotonic() + budget
    seed = seed0
    while time.monotonic() < end and not failed:
        try:
            r = subprocess.run(cmd + [parser, str(chunk), str(seed)], capture_output=True, text=True, errors="replace",
                               cwd=work, env=env, timeout=timeout)
            text = r.stdout + r.stderr
            code = r.returncode
        except subprocess.TimeoutExpired as e:
            text = (e.stdout or b"").decode("utf-8", "replace") if isinstance(e.stdout, bytes) else (e.stdout or "")
            code = None
        log.write("== seed %d count %d exit %s\n%s\n" % (seed, chunk, code, text))
        runs += 1
        m = re.search(r"tally: ok (\d+), err (\d+), fault (\d+), skipped (\d+)", text)
        if m:
            for k, v in zip(("ok", "err", "fault", "skipped"), m.groups()):
                tally[k] += int(v)
        if code is None:
            failed, why = True, "timed out (seed %d)" % seed
        elif SANITIZER.search(text):
            failed, why = True, "sanitizer report (seed %d)" % seed
        elif code != 0:
            failed, why = True, ("faults (seed %d)" % seed) if code == 1 and m and int(m.group(3)) > 0 else "exit %s (seed %d)" % (code, seed)
        seed += 1
    log.close()
    inputs = tally["ok"] + tally["err"] + tally["skipped"] + tally["fault"]
    return parser, bname, runs, inputs, tally, failed, why


def main():
    ap = argparse.ArgumentParser(description=__doc__.split("\n")[0])
    ap.add_argument("--budget", type=float, default=600, help="seconds per parser and build (default 600)")
    ap.add_argument("--parsers", default=",".join(PARSERS))
    ap.add_argument("--builds", default=",".join(BUILDS), help="gcc, clang, asan, coby")
    ap.add_argument("--jobs", type=int, default=os.cpu_count() or 2)
    ap.add_argument("--seed", type=int, default=int(time.time()) % 1000000007, help="first seed (default: from the clock)")
    ap.add_argument("--timeout", type=int, default=1800, help="seconds one run may take")
    ap.add_argument("--out")
    a = ap.parse_args()
    out = out_dir(a.out)
    os.makedirs(out, exist_ok=True)
    parsers = [p for p in a.parsers.split(",") if p]
    builds = [b for b in a.builds.split(",") if b]
    for p in parsers:
        if p not in PARSERS:
            sys.exit("fuzz: unknown parser %s (one of %s)" % (p, ", ".join(PARSERS)))
    cmds = {b: build(b, out) for b in builds}
    print("fuzz: %s; %s; %d s each; first seed %d; %s" % (",".join(parsers), ",".join(builds), a.budget, a.seed, out), flush=True)
    pairs = [(p, b) for b in builds for p in parsers]
    results = []
    with cf.ThreadPoolExecutor(max_workers=max(1, a.jobs)) as ex:
        futs = [ex.submit(run_pair, p, b, cmds[b], a.budget, out, a.seed + 1000003 * i, a.timeout) for i, (p, b) in enumerate(pairs)]
        for f in cf.as_completed(futs):
            r = f.result()
            results.append(r)
            p, b, runs, inputs, t, failed, why = r
            print("  %-12s %-6s %6d inputs in %4d runs: ok %d, err %d, skipped %d, fault %d%s" % (
                p, b, inputs, runs, t["ok"], t["err"], t["skipped"], t["fault"], ("  FAILED: " + why) if failed else ""), flush=True)
    bad = [r for r in results if r[5]]
    with open(os.path.join(out, "SUMMARY"), "w") as f:
        f.write("seed %d budget %d builds %s\n" % (a.seed, a.budget, ",".join(builds)))
        for p, b, runs, inputs, t, failed, why in sorted(results):
            f.write("%s %s inputs %d fault %d %s\n" % (p, b, inputs, t["fault"], ("FAILED " + why) if failed else "ok"))
        f.write("result %s\n" % ("FAILED" if bad else "PASSED"))
    print("fuzz: %s (%d of %d pairs failed); %s" % ("FAILED" if bad else "PASSED", len(bad), len(results), out))
    return 1 if bad else 0


if __name__ == "__main__":
    sys.exit(main())
