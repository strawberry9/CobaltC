#!/usr/bin/env python3
"""The runtime's runs replayed through the proven machine (D-0299 step 4): for each program, `cobc` builds it
with every check kept (`--all-checks`, `COBALTC_PATHLESS=0`), the program runs once with every reference
recorded (`COBALTC_RECORD=full`) and its steps written in the replay's action format
(`COBALTC_REPLAY_TRACE`, `impl/cbrt/src/replay.rs`), and `proofs`' `replay` executable checks every action's
premises and performs it (`Cobalt/Exec.lean`, whose `exec_sound` makes an admitted action a `Step`).

A run replays whole; or stops where it leaves the fragment (threads, vectors, raw storage, closures, slices,
reference data, destructors: what came before was checked); or faulted as the rules say (the runtime's
alias or temporal fault is the machine's refusal of the same action); or is REFUSED (an action the runtime
took that the machine does not admit) or MISSED (the runtime faulted where the machine admits every step).
The last two are findings about the runtime, the trace or the fragment. With two C compilers the two
builds' traces must be the same.

    python3 impl/tools/replay_runtime.py PATH... [--cobc PATH] [--replay PATH] [--cc gcc,clang] [--out DIR] [--check]

PATH is a `.cb` file or a directory of them. `--check` exits 1 on a refusal, a miss, a malformed trace, or
traces that differ between the C compilers. Linux and macOS (the run is limited with `ulimit -t`);
standard library only.
"""
import os
import re
import subprocess
import sys

ROOT = os.path.dirname(os.path.dirname(os.path.dirname(os.path.abspath(__file__))))


def programs(paths):
    for p in paths:
        if os.path.isdir(p):
            for d, _, fs in sorted(os.walk(p)):
                for f in sorted(fs):
                    if f.endswith(".cb"):
                        yield os.path.join(d, f)
        else:
            yield p


def run_one(prog, cobc, replay, cc, out):
    """(kind, detail, trace path): kind in whole/outside/agreed/refused/missed/malformed/build/run."""
    base = os.path.join(out, re.sub(r"[^A-Za-z0-9_.-]", "_", os.path.relpath(os.path.abspath(prog), ROOT))[:-3] + "." + os.path.basename(cc))
    exe, trace = base + ".exe", base + ".tr"
    for p in (exe, trace):
        if os.path.exists(p):
            os.remove(p)
    env = dict(os.environ, COBALTC_PATHLESS="0")
    env.pop("COBALTC_REPLAY_TRACE", None)
    b = subprocess.run([cobc, "--all-checks", "--cc", cc, "-o", exe, os.path.basename(prog)], cwd=os.path.dirname(os.path.abspath(prog)), env=env, capture_output=True, text=True, timeout=600)
    if b.returncode != 0 or not os.path.exists(exe):
        msg = (b.stderr or b.stdout).strip()
        if b.returncode == 1 and ("(static)" in msg or "error[" in msg or "diag." in msg):
            return "rejected", "", None
        return "build", msg[:200], None
    renv = dict(env, COBALTC_RECORD="full", COBALTC_REPLAY_TRACE=trace, COBALTC_CPU_LIMIT="20")
    try:
        subprocess.run(["bash", "-c", 'ulimit -t 30; exec "$0"', exe], cwd=out, env=renv, capture_output=True, timeout=120, stdin=subprocess.DEVNULL)
    except subprocess.TimeoutExpired:
        return "run", "timed out", None
    if not os.path.exists(trace):
        return "run", "no trace written", None
    r = subprocess.run([replay, trace], capture_output=True, text=True, timeout=600)
    line = (r.stdout or r.stderr).strip().split("\n")[-1]
    if "REFUSED" in line:
        return "refused", line, trace
    if "MISSED" in line:
        return "missed", line, trace
    if "faulted as the rules say" in line:
        return "agreed", line, trace
    if "outside the fragment (" in line:
        return "outside", line.split("outside the fragment (")[1].split("):")[0], trace
    if ": ok:" in line:
        return "whole", line, trace
    return "malformed", line[:300], trace


def steps_of(line):
    m = re.search(r"(\d+) step\(s\) admitted", line)
    return int(m.group(1)) if m else 0


def main():
    args = sys.argv[1:]
    cobc = os.path.join(ROOT, "impl", "target", "release", "cobc")
    replay = os.path.join(ROOT, "proofs", ".lake", "build", "bin", "replay")
    ccs = ["gcc", "clang"]
    out = None
    check = False
    paths = []
    i = 0
    while i < len(args):
        a = args[i]
        if a == "--cobc":
            cobc = os.path.abspath(args[i + 1])
            i += 2
        elif a == "--replay":
            replay = os.path.abspath(args[i + 1])
            i += 2
        elif a == "--cc":
            ccs = [c for c in args[i + 1].split(",") if c]
            i += 2
        elif a == "--out":
            out = os.path.abspath(args[i + 1])
            i += 2
        elif a == "--check":
            check = True
            i += 1
        else:
            paths.append(a)
            i += 1
    if not paths:
        print(__doc__)
        return 2
    if out is None:
        out = os.path.join(ROOT, "stress", "verify", "replay_runtime")
    os.makedirs(out, exist_ok=True)
    tally, reasons, bad = {}, {}, []
    steps = 0
    for prog in programs(paths):
        traces = []
        first = None
        for cc in ccs:
            kind, detail, trace = run_one(prog, cobc, replay, cc, out)
            traces.append(trace)
            if first is None:
                first = (kind, detail)
                tally[kind] = tally.get(kind, 0) + 1
                if kind == "outside":
                    reasons[detail] = reasons.get(detail, 0) + 1
                if kind in ("refused", "missed", "malformed", "build", "run"):
                    bad.append("%s: %s: %s" % (prog, kind, detail))
                if kind in ("whole", "agreed"):
                    steps += steps_of(detail)
        if len(traces) > 1 and all(traces):
            texts = [open(t, encoding="utf-8").read() for t in traces]
            if any(t != texts[0] for t in texts[1:]):
                tally["differ"] = tally.get("differ", 0) + 1
                bad.append("%s: the traces differ between %s" % (prog, " and ".join(ccs)))
    for b in bad:
        print("replay_runtime: " + b)
    outs = ", ".join("%s %d" % (k, v) for k, v in sorted(reasons.items(), key=lambda kv: -kv[1]))
    print("replay_runtime: %d program(s): %d replayed whole, %d faulted as the rules say, %d stopped outside the fragment (%s), %d rejected statically, %d REFUSED, %d MISSED, %d other; %d action(s) admitted in the runs that replayed to their end"
          % (sum(v for k, v in tally.items() if k != "differ"), tally.get("whole", 0), tally.get("agreed", 0), tally.get("outside", 0), outs, tally.get("rejected", 0), tally.get("refused", 0), tally.get("missed", 0),
             sum(tally.get(k, 0) for k in ("malformed", "build", "run")), steps))
    if tally.get("differ"):
        print("replay_runtime: %d program(s) whose traces differ between the C compilers" % tally["differ"])
    if check and (tally.get("refused") or tally.get("missed") or tally.get("malformed") or tally.get("differ") or tally.get("build") or tally.get("run")):
        return 1
    return 0


if __name__ == "__main__":
    sys.exit(main())
