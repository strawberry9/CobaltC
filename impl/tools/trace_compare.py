#!/usr/bin/env python3
"""Trace equivalence between the interpreter and the compiler (D-0264, plan B3).
Runs every single-threaded conformance case under `coby` and under `cobc`
with `COBALTC_TRACE_EVENTS=1`, and compares the two sequences of events the
tools write on standard error: each user destructor run (`EV drop T`), the
program's fault (`EV fault diag.x`), an `exit` (`EV exit n`) and the end
(`EV end n`). Outputs are compared by the conformance runner; this compares
what the rules do between outputs — the order destructors run in, which
the elisions (`cobc`'s removed checks and objects) must not change.
    python3 impl/tools/trace_compare.py [--cc gcc] [--only TEXT] [--jobs N]
A case that spawns a thread is compared as a multiset (its events
interleave, so their order is not the rules' to fix, but what runs is: the
same destructors the same number of times, the same fault and end); a case
the interpreter refuses (`unsupported`) is skipped. A type's name is
compared by its last segment (`m::T` and `m__T` are one name, a generic's
arguments are dropped). Exit 1 if any case's traces differ.
"""
import os
import re
import subprocess
import sys
from concurrent.futures import ThreadPoolExecutor
ROOT = os.path.dirname(os.path.dirname(os.path.dirname(os.path.abspath(__file__))))
CONF = os.path.join(ROOT, "impl", "conformance")
COBY = os.path.join(ROOT, "impl", "target", "release", "coby")
COBC = os.path.join(ROOT, "impl", "target", "release", "cobc")


def norm(ev):
    kind, _, rest = ev.partition(" ")
    if kind == "drop":
        name = re.split(r"[<(\[ ]", rest, 1)[0]
        name = re.split(r"::|__", name)[-1].replace("$", "_")
        return "drop " + name
    return ev


def events(text):
    return [norm(l[3:].strip()) for l in text.split("\n") if l.startswith("EV ")]


def run(cmd, cwd, env):
    try:
        r = subprocess.run(cmd, cwd=cwd, env=env, capture_output=True, text=True, errors="replace", timeout=600)
        return r.returncode, r.stdout, r.stderr
    except subprocess.TimeoutExpired:
        return -1, "", "timed out"


def main():
    cc = sys.argv[sys.argv.index("--cc") + 1] if "--cc" in sys.argv else "gcc"
    only = sys.argv[sys.argv.index("--only") + 1] if "--only" in sys.argv else ""
    jobs = int(sys.argv[sys.argv.index("--jobs") + 1]) if "--jobs" in sys.argv else max(1, (os.cpu_count() or 2) // 2)
    cases = []
    for d, _, fs in os.walk(CONF):
        for f in sorted(fs):
            if f.endswith(".cb") and (not only or only in os.path.join(d, f)):
                p = os.path.join(d, f)
                src = open(p, encoding="utf-8", errors="replace").read()
                if src.startswith("// CONFORMANCE-FIXTURE") or "cobc-only" in src.split("\n", 8)[0:8].__str__():
                    continue
                cases.append((p, "spawn(" in src))
    env = dict(os.environ)
    env["COBALTC_TRACE_EVENTS"] = "1"
    env["COBALTC_CPU_LIMIT"] = env.get("COBALTC_CPU_LIMIT", "300")

    def one(case):
        p, threaded = case
        d, f = os.path.dirname(p), os.path.basename(p)
        rc1, _, e1 = run([COBY, f], d, env)
        if rc1 == 3 or "unsupported:" in e1:
            return (p, "skipped", None, None)
        rc2, _, e2 = run([COBC, "--cc", cc, "--run", f], d, env)
        a, b = events(e1), events(e2)
        if threaded:
            a, b = sorted(a), sorted(b)
        return (p, "same" if a == b else "differ", a, b)

    with ThreadPoolExecutor(max_workers=jobs) as ex:
        results = list(ex.map(one, cases))
    same = sum(1 for r in results if r[1] == "same")
    skipped = sum(1 for r in results if r[1] == "skipped")
    bad = [r for r in results if r[1] == "differ"]
    for p, _, a, b in bad:
        rel = os.path.relpath(p, ROOT)
        print("DIFFER %s" % rel)
        print("  coby: %s" % a[:12])
        print("  cobc: %s" % b[:12])
    threaded = sum(1 for c in cases if c[1])
    print("trace_compare: %d cases (%d threaded, compared as multisets), %d same, %d differ, %d skipped (unsupported); %d events compared" % (len(results), threaded, same, len(bad), skipped, sum(len(r[2] or []) for r in results)))
    return 1 if bad else 0


if __name__ == "__main__":
    sys.exit(main())
