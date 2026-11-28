#!/usr/bin/env python3
"""CIR step T3: run cobc's verdict check (`COBALTC_VERDICT=check`) over the corpus and summarise it.

  verdictcheck.py [--cobc PATH] [--jobs 2] [--only SUBSTR] [--set small|all|tools] [--levels 0] [--env K=V]...

For each program `cobc --emit-c` with the check on: the typing-only walk (`impl/cir/reach.cb`) runs before
the first lowering and is compared with it. Summed over the corpus: programs, verdicts equal, MISS lines by
class (a function, closure signature, type or point check the lowering has and the walk has not), the
functions only the walk reaches (extra), the typer's gaps in the walk, and the two times. Every DIFF, MISS
and GAP is listed, and written with the extras to stress/cir/verdict_*.txt. Exit 1 on any MISS or DIFF.
"""
import argparse, collections, concurrent.futures, os, pathlib, re, resource, subprocess, sys

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


def one(cobc, src, level, extra_env):
    env = dict(os.environ, COBALTC_VERDICT="check", COBALTC_CPU_LIMIT="3000")
    env.pop("COBALTC_BOOTSTRAP_FORM", None)
    env.pop("COBALTC_BOOTSTRAP_UPTO", None)
    if level:
        env["COBALTC_BOOTSTRAP_UPTO"] = str(level)
    env.update(extra_env)
    p = subprocess.Popen([cobc, "--emit-c", "--c-name", "prog.c", "-o", os.devnull, src], env=env,
                         stderr=subprocess.PIPE, stdout=subprocess.DEVNULL, preexec_fn=limits,
                         cwd=os.path.dirname(src))
    rel = os.path.relpath(src, root)
    r = {"src": rel, "sum": None, "miss": [], "diff": [], "text": [], "gap": [], "extra": [], "note": []}
    for raw in p.stderr:
        f = raw.decode("utf8", "replace").rstrip("\n").split("\t")
        if f[0] == "VERDICT-MISS":
            r["miss"].append((f[1] if len(f) > 1 else "?", "\t".join(f[2:])))
        elif f[0] == "VERDICT-DIFF":
            r["diff"].append("\t".join(f[1:]))
        elif f[0] == "VERDICT-TEXT":
            r["text"].append("\t".join(f[1:]))
        elif f[0] == "VERDICT-GAP":
            r["gap"].append("\t".join(f[1:]))
        elif f[0] == "VERDICT-NOTE":
            r["note"].append("\t".join(f[1:]))
        elif f[0] == "VERDICT-EXTRA":
            r["extra"].append(f[2] if len(f) > 2 else "?")
        elif f[0] == "VERDICT-SUM":
            r["sum"] = [int(n) for n in re.findall(r"(?<![\w-])\d+", "\t".join(f[1:7]))] + [f[-1].replace("verdict ", "", 1)]
    r["status"] = p.wait()
    return r


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--cobc", default=str(root / "impl/target/dev/cobc"))
    ap.add_argument("--jobs", type=int, default=2)
    ap.add_argument("--only", default="")
    ap.add_argument("--set", default="small")
    ap.add_argument("--levels", default="0")
    ap.add_argument("--env", action="append", default=[])
    ap.add_argument("--top", type=int, default=15)
    a = ap.parse_args()
    cobc = os.path.abspath(a.cobc)
    extra_env = dict(e.split("=", 1) for e in a.env)
    files = [f for f in corpus(a.set) if a.only in f]
    jobs = [(f, int(lv)) for f in files for lv in a.levels.split(",")]
    results = []
    with concurrent.futures.ThreadPoolExecutor(a.jobs) as ex:
        for r in ex.map(lambda j: one(cobc, j[0], j[1], extra_env), jobs):
            results.append(r)
    compared = [r for r in results if r["sum"]]
    miss = collections.Counter()
    extra_names = collections.Counter()
    tot = collections.Counter()
    out = {"miss": [], "diff": [], "gap": [], "extra": [], "text": [], "note": []}
    for r in compared:
        s = r["sum"]
        # fns requested, reached, extra; closures, walked; types, walked; walk-ms; lower-ms; gaps; verdict
        for k, v in zip(["fns", "reached", "extra", "closures", "wclosures", "types", "wtypes", "walk_ms",
                         "lower_ms", "gaps"], s[:10]):
            tot[k] += v
        tot["same" if not r["diff"] else "differ"] += 1
        tot["kept" if s[-1].startswith("kept") else "decided"] += 1
        tot["programs with extra"] += 1 if r["extra"] else 0
        for cls, what in r["miss"]:
            miss[cls] += 1
            out["miss"].append(f"{r['src']}\t{cls}\t{what}")
        for d in r["diff"]:
            out["diff"].append(f"{r['src']}\t{d}")
        for d in r["text"]:
            out["text"].append(f"{r['src']}\t{d}")
        for d in r["gap"]:
            out["gap"].append(f"{r['src']}\t{d}")
        for d in r["note"]:
            out["note"].append(f"{r['src']}\t{d}")
        for e in r["extra"]:
            out["extra"].append(f"{r['src']}\t{e}")
            extra_names[e.split("<", 1)[0]] += 1
    os.makedirs(root / "stress/cir", exist_ok=True)
    for k, lines in out.items():
        with open(root / f"stress/cir/verdict_{k}.txt", "w") as fh:
            fh.write("\n".join(lines) + ("\n" if lines else ""))
    for k in ("diff", "miss", "gap", "note"):
        for line in out[k]:
            print(f"{k.upper()}\t{line}")
    not_compared = [r["src"] for r in results if not r["sum"]]
    # A program with no first lowering was refused before it (status 1); any other status is the check's fault.
    odd = [f"{r['src']}\tstatus {r['status']}" for r in results if not r["sum"] and r["status"] not in (0, 1)]
    odd += [f"{r['src']}\tstatus 0 with no VERDICT-SUM" for r in results if not r["sum"] and r["status"] == 0]
    for line in odd:
        print(f"ODD\t{line}")
    print(f"verdictcheck: {len(jobs)} runs; compared {len(compared)} (no first lowering to compare: "
          f"{len(not_compared)}); verdicts equal {tot['same']}, different {tot['differ']} "
          f"(decided {tot['decided']}, kept {tot['kept']}; blocker texts differing {len(out['text'])})")
    print(f"verdictcheck: MISS fn {miss['fn']}, closure {miss['closure']}, type {miss['type']}, "
          f"point {miss['point']}; gaps {len(out['gap'])}")
    pct = 100.0 * tot["extra"] / tot["fns"] if tot["fns"] else 0.0
    print(f"verdictcheck: functions requested {tot['fns']}, reached {tot['reached']}, extra {tot['extra']} "
          f"({pct:.2f}%, in {tot['programs with extra']} programs); closures {tot['closures']} walked "
          f"{tot['wclosures']}; types {tot['types']} walked {tot['wtypes']}")
    print(f"verdictcheck: walk {tot['walk_ms']} ms, first lowering {tot['lower_ms']} ms")
    big = sorted(compared, key=lambda r: -r["sum"][8])[:5]
    for r in big:
        s = r["sum"]
        print(f"  largest  {r['src']}: fns {s[0]} reached {s[1]} extra {s[2]}; walk {s[7]} ms, lowering {s[8]} ms")
    for name, n in extra_names.most_common(a.top):
        print(f"  extra {n:>8}  {name}")
    sys.exit(1 if out["miss"] or out["diff"] or odd else 0)


main()
