#!/usr/bin/env python3
"""CIR step T1: run cobc's typer check (`COBALTC_TYPER=check`) over the corpus and summarise it.

  typercheck.py [--cobc PATH] [--jobs 3] [--only SUBSTR] [--set small|all] [--levels 0] [--diffs N]

For each program `cobc --emit-c` with the check on. Summed over the corpus: expressions checked, the
same, different, and not typed yet (gaps) by reason. Prints the first N differences in full (default 40)
and writes all of them to stress/cir/typer_diffs.txt. Exit 1 if any difference; gaps do not fail.
"""
import argparse, collections, concurrent.futures, os, pathlib, resource, subprocess, sys

root = pathlib.Path(__file__).resolve().parents[3]


def corpus(which):
    files = sorted(str(p) for p in (root / "impl/conformance").rglob("*.cb"))
    files += sorted(str(p) for p in (root / "showcase").rglob("*.cb") if p.name == "main.cb" or p.parent.name.startswith("tier"))
    if which == "tools":
        files = []
    if which in ("all", "tools"):
        for t in ["impl/cobfmt/main.cb", "impl/tests/cbtest/main.cb", "impl/tests/differential/main.cb",
                  "impl/cbparse/main.cb", "impl/model/src/main.cb", "impl/cobc/main.cb"]:
            files.append(str(root / t))
    return list(dict.fromkeys(files))


def limits():
    resource.setrlimit(resource.RLIMIT_CPU, (3000, 3000))


def one(cobc, src, level):
    env = dict(os.environ, COBALTC_TYPER="check", COBALTC_CPU_LIMIT="3000")
    env.pop("COBALTC_BOOTSTRAP_FORM", None)
    env.pop("COBALTC_BOOTSTRAP_UPTO", None)
    if level:
        env["COBALTC_BOOTSTRAP_UPTO"] = str(level)
    p = subprocess.Popen([cobc, "--emit-c", "--c-name", "prog.c", "-o", os.devnull, src], env=env,
                         stderr=subprocess.PIPE, stdout=subprocess.DEVNULL, preexec_fn=limits,
                         cwd=os.path.dirname(src))
    sums = [0, 0, 0, 0]
    gaps = collections.Counter()
    diffs = []
    pending = []
    gapat = []
    for raw in p.stderr:
        f = raw.decode("utf8", "replace").rstrip("\n").split("\t")
        if f[0] == "TYPER-ABANDONED":
            # A lowering that ended unsupported: what it compared counts for nothing.
            pending = []
        elif f[0] == "TYPER-GAPAT":
            gapat.append("\t".join([os.path.relpath(src, root), str(level)] + f[1:]))
        elif f[0] == "TYPER-SUM" and len(f) == 6:
            diffs.extend(pending)
            pending = []
            for i in range(4):
                sums[i] += int(f[2 + i].split(" ")[1])
        elif f[0] == "TYPER-GAP" and len(f) == 3:
            gaps[f[1]] += int(f[2])
        elif f[0] == "TYPER-DIFF":
            pending.append("\t".join([os.path.relpath(src, root), str(level)] + f[1:]))
    return p.wait(), sums, gaps, diffs, gapat


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--cobc", default=str(root / "impl/target/dev/cobc"))
    ap.add_argument("--jobs", type=int, default=3)
    ap.add_argument("--only", default="")
    ap.add_argument("--set", default="small")
    ap.add_argument("--levels", default="0")
    ap.add_argument("--diffs", type=int, default=40)
    a = ap.parse_args()
    cobc = os.path.abspath(a.cobc)
    files = [f for f in corpus(a.set) if a.only in f]
    jobs = [(f, int(lv)) for f in files for lv in a.levels.split(",")]
    total = [0, 0, 0, 0]
    gaps = collections.Counter()
    diffs = []
    gapat = []
    with concurrent.futures.ThreadPoolExecutor(a.jobs) as ex:
        for status, sums, g, d, ga in ex.map(lambda j: one(cobc, *j), jobs):
            gapat.extend(ga)
            for i in range(4):
                total[i] += sums[i]
            gaps.update(g)
            diffs.extend(d)
    os.makedirs(root / "stress/cir", exist_ok=True)
    with open(root / "stress/cir/typer_diffs.txt", "w") as fh:
        fh.write("\n".join(diffs) + ("\n" if diffs else ""))
    with open(root / "stress/cir/typer_gaps.txt", "w") as fh:
        fh.write("\n".join(gapat) + ("\n" if gapat else ""))
    for d in diffs[:a.diffs]:
        print("DIFF\t" + d)
    pct = 100.0 * total[1] / total[0] if total[0] else 0.0
    print(f"typercheck: {len(jobs)} runs; expressions checked {total[0]}, same {total[1]} ({pct:.1f}%), "
          f"different {total[2]}, not typed yet {total[3]}")
    for why, n in gaps.most_common(60):
        print(f"  gap {n:>10}  {why}")
    sys.exit(1 if total[2] else 0)


main()
