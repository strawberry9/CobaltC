#!/usr/bin/env python3
"""The model checker's runs replayed through the proven machine (D-0270): for each program of a directory,
`cobmodel trace` writes one run as the mechanised fragment's actions and `proofs`' `replay` executable
checks every action's premises and performs it (`Cobalt/Exec.lean`, whose `exec_sound` makes an admitted
action a `Step`). A run replays whole, stops where the model leaves the fragment (vectors, channels,
atomics, slices, closures, statics, arrays: what came before was checked), or is REFUSED: an action the
model took that the fragment's rules do not admit, which is a finding about one of them.
    python3 impl/tools/replay_model.py DIR [--cobmodel PATH] [--replay PATH] [--out DIR] [--check]
`--check` exits 1 when any run is refused or a trace is malformed. Standard library only.
"""
import os
import subprocess
import sys
ROOT = os.path.dirname(os.path.dirname(os.path.dirname(os.path.abspath(__file__))))


def main():
    args = sys.argv[1:]
    cobmodel = os.path.join(ROOT, "impl", "target", "release", "cobmodel")
    replay = os.path.join(ROOT, "proofs", ".lake", "build", "bin", "replay")
    out = None
    check = False
    dirs = []
    i = 0
    while i < len(args):
        a = args[i]
        if a == "--cobmodel":
            cobmodel = args[i + 1]
            i += 2
        elif a == "--replay":
            replay = args[i + 1]
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
        out = os.path.join(ROOT, "stress", "verify", "replay", "traces")
    os.makedirs(out, exist_ok=True)
    whole = outside = refused = other = 0
    steps = gaps = 0
    reasons = {}
    bad = []
    for d in dirs:
        for name in sorted(os.listdir(d)):
            if not name.endswith(".cb"):
                continue
            prog = os.path.join(d, name)
            trace = os.path.join(out, name[:-3] + ".tr")
            t = subprocess.run([cobmodel, "trace", prog, "--out", trace], capture_output=True, text=True, timeout=120)
            if t.returncode != 0:
                other += 1
                bad.append("%s: trace failed: %s" % (name, (t.stderr or t.stdout).strip()[:200]))
                continue
            r = subprocess.run([replay, trace], capture_output=True, text=True, timeout=300)
            line = (r.stdout or r.stderr).strip()
            if "REFUSED" in line:
                refused += 1
                bad.append("%s: %s" % (name, line))
            elif "outside the fragment" in line:
                outside += 1
                reason = line.split("outside the fragment (")[1].split(")")[0]
                reasons[reason] = reasons.get(reason, 0) + 1
                try:
                    steps += int(line.split("): ")[1].split(" step")[0])
                    gaps += int(line.split(", ")[1].split(" gap")[0])
                except (IndexError, ValueError):
                    pass
            elif line.endswith("gap(s) applied") and "ok:" in line:
                whole += 1
                try:
                    steps += int(line.split("ok: ")[1].split(" step")[0])
                    gaps += int(line.split(", ")[1].split(" gap")[0])
                except (IndexError, ValueError):
                    pass
            else:
                other += 1
                bad.append("%s: %s" % (name, line[:200]))
    for b in bad:
        print("replay_model: " + b)
    outs = ", ".join("%s %d" % (k, v) for k, v in sorted(reasons.items(), key=lambda kv: -kv[1]))
    print("replay_model: %d program(s): %d replayed whole, %d stopped outside the fragment (%s), %d REFUSED, %d other; %d action(s) admitted, %d gap(s) applied"
          % (whole + outside + refused + other, whole, outside, outs, refused, other, steps, gaps))
    if check and (refused or other):
        return 1
    return 0


if __name__ == "__main__":
    sys.exit(main())
