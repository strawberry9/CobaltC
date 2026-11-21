#!/usr/bin/env python3
"""The compiler's certificates checked against the model (D-0299): for each program of a directory, `cobc
--certificate` writes what the static pass decided (which bodies keep no record of their references, and the
program's verdict), and `cobmodel certify` runs the program through every interleaving of the model written
from the rules alone, with those bodies kept unrecorded as a compiler keeps them: an alias or temporal fault
the rules find at a step of such a body is `model.decided-but-false`, a claim the static pass made that the
run refutes. The static verdicts are compared too.
    python3 impl/tools/certify_model.py DIR... [--cobc PATH] [--cobmodel PATH] [--out DIR] [--check]
`--check` exits 1 when any program is not certified. The certificates are kept under `--out` (default
`stress/verify/certify`). Standard library only.
"""
import os
import subprocess
import sys
ROOT = os.path.dirname(os.path.dirname(os.path.dirname(os.path.abspath(__file__))))


def main():
    args = sys.argv[1:]
    cobc = os.path.join(ROOT, "impl", "target", "release", "cobc")
    cobmodel = os.path.join(ROOT, "impl", "target", "release", "cobmodel")
    out = None
    check = False
    dirs = []
    i = 0
    while i < len(args):
        a = args[i]
        if a == "--cobc":
            cobc = args[i + 1]
            i += 2
        elif a == "--cobmodel":
            cobmodel = args[i + 1]
            i += 2
        elif a == "--out":
            out = args[i + 1]
            i += 2
        elif a == "--check":
            check = True
            i += 1
        else:
            dirs.append(a)
            i += 1
    if not dirs:
        print(__doc__)
        return 2
    if out is None:
        out = os.path.join(ROOT, "stress", "verify", "certify")
    os.makedirs(out, exist_ok=True)
    certified = rejected = failed = other = 0
    whole = partial = 0
    bodies_decided = bodies_all = 0
    bad = []
    for d in dirs:
        for name in sorted(os.listdir(d)):
            if not name.endswith(".cb"):
                continue
            prog = os.path.abspath(os.path.join(d, name))
            cert = os.path.abspath(os.path.join(out, name[:-3] + ".cert"))
            # The compiler's verdict needs the lowering: `--emit-c` runs it and compiles nothing.
            c = subprocess.run([cobc, "--certificate", cert, "--emit-c", "-o", os.devnull, prog], capture_output=True, text=True, timeout=300, cwd=out)
            if c.returncode not in (0, 1) or not os.path.exists(cert):
                other += 1
                bad.append((name, "cobc: " + (c.stderr.strip().splitlines() or ["no output"])[-1]))
                continue
            with open(cert, encoding="utf-8") as f:
                lines = f.read().splitlines()
            verdict = next((l[len("verdict: "):] for l in lines if l.startswith("verdict: ")), "")
            user = [l for l in lines if l.startswith("fn ") and "::" not in l.split(":")[0]]
            bodies_all += len(user)
            bodies_decided += sum(1 for l in user if l.endswith(": decided"))
            m = subprocess.run([cobmodel, "certify", cert, prog], capture_output=True, text=True, timeout=600)
            first = (m.stdout.strip().splitlines() or [""])[0]
            if m.returncode == 0 and "rejected by both" in first:
                rejected += 1
            elif m.returncode == 0 and ": certified:" in first:
                certified += 1
                if verdict == "decided":
                    whole += 1
                else:
                    partial += 1
            elif m.returncode == 1:
                failed += 1
                bad.append((name, first or m.stderr.strip()))
            else:
                other += 1
                bad.append((name, "cobmodel: " + (m.stderr.strip().splitlines() or [first or "no output"])[-1]))
    print(f"certify: {certified} certified ({whole} decided whole, {partial} in part), {rejected} rejected by both, {failed} NOT certified, {other} not run; {bodies_decided} of {bodies_all} bodies decided")
    for name, why in bad:
        print(f"  {name}: {why}")
    if check and (failed or other):
        return 1
    return 0


if __name__ == "__main__":
    sys.exit(main())
