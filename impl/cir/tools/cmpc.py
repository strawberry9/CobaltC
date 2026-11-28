#!/usr/bin/env python3
"""Compare the C two cobc builds emit, file by file, over the corpus (CIR steps T1..T6).

  cmpc.py --ref REF_COBC --new NEW_COBC [--levels 0,10] [--jobs 3] [--only SUBSTR] [--set small|all]
          [--env NAME=VALUE ...] [--out DIR] [--items]

For every program: `cobc --emit-c -o X FILE` under each compiler and each stage level
(`COBALTC_BOOTSTRAP_UPTO=N`; 0 = the default form). Compared: exit status, stderr, and the C bytes.
`--env` settings go to the NEW compiler only (e.g. COBALTC_TYPER=check). Prints one line per difference
and a summary; exit 1 if anything differs. Programs both refuse identically count as equal.

`--items` (from step T2 on, where the order in which functions are first requested changes): C that differs in
bytes is compared as a multiset of top-level items (the paragraphs between blank lines). The same items in another
order count as `reordered`; otherwise the items only one side has are compared line by line and written to
`<out>/<n>_<level>.items` (lines only in the reference, lines only in the new C).

Corpus: every conformance case, every showcase program, and (set `all`) the tools written in CobaltC.
"""
import argparse, collections, concurrent.futures, hashlib, os, pathlib, re, resource, subprocess, sys, time

root = pathlib.Path(__file__).resolve().parents[3]


def corpus(which):
    files = sorted(str(p) for p in (root / "impl/conformance").rglob("*.cb"))
    files += sorted(str(p) for p in (root / "showcase").rglob("*.cb") if p.name == "main.cb" or p.parent.name.startswith("tier"))
    if which == "all":
        for t in ["impl/cobfmt/main.cb", "impl/tests/cbtest/main.cb", "impl/tests/differential/main.cb",
                  "impl/cbparse/main.cb", "impl/model/src/main.cb", "impl/cobc/main.cb"]:
            files.append(str(root / t))
    seen, out = set(), []
    for f in files:
        if f not in seen:
            seen.add(f)
            out.append(f)
    return out


def limits():
    resource.setrlimit(resource.RLIMIT_CPU, (900, 900))


def emit(cobc, src, level, out, extra):
    env = dict(os.environ)
    env.pop("COBALTC_BOOTSTRAP_FORM", None)
    env.pop("COBALTC_BOOTSTRAP_UPTO", None)
    if level:
        env["COBALTC_BOOTSTRAP_UPTO"] = str(level)
    env["COBALTC_CPU_LIMIT"] = "900"
    env.update(extra)
    try:
        r = subprocess.run([cobc, "--emit-c", "--c-name", "prog.c", "-o", out, src], env=env, capture_output=True, timeout=1200,
                           preexec_fn=limits, cwd=os.path.dirname(src))
        return r.returncode, r.stderr
    except subprocess.TimeoutExpired:
        return -99, b"timeout"


def digest(path):
    if not os.path.exists(path):
        return None
    h = hashlib.sha256()
    with open(path, "rb") as f:
        for chunk in iter(lambda: f.read(1 << 20), b""):
            h.update(chunk)
    return h.hexdigest()


def items_diff(a, b, report):
    """The two C files as multisets of paragraphs. Returns (reordered, summary)."""
    def paras(path):
        # Without the directives that return to the C's own numbering (`#line N "prog.c"`): they move
        # with everything before them.
        with open(path, "rb") as f:
            text = re.sub(rb'(?m)^#line \d+ "[^"]*\.c"\n', b"", f.read())
        return collections.Counter(q.strip(b"\n") for q in text.split(b"\n\n") if q.strip())
    pa, pb = paras(a), paras(b)
    only_a, only_b = pa - pb, pb - pa
    if not only_a and not only_b:
        return True, ""
    def lines(c):
        out = collections.Counter()
        for q, n in c.items():
            for l in q.split(b"\n"):
                out[l] += n
        return out
    la, lb = lines(only_a), lines(only_b)
    ra, rb = la - lb, lb - la
    def heads(c):
        return sorted(q.split(b"\n", 1)[0][:160] for q in c)
    with open(report, "wb") as f:
        f.write(b"== paragraphs only in ref: %d, only in new: %d\n" % (sum(only_a.values()), sum(only_b.values())))
        f.write(b"== first lines of paragraphs only in ref\n" + b"\n".join(heads(only_a)) + b"\n")
        f.write(b"== first lines of paragraphs only in new\n" + b"\n".join(heads(only_b)) + b"\n")
        f.write(b"== lines only in ref: %d\n" % sum(ra.values()))
        f.write(b"\n".join(l for l in sorted(ra.elements())) + b"\n")
        f.write(b"== lines only in new: %d\n" % sum(rb.values()))
        f.write(b"\n".join(l for l in sorted(rb.elements())) + b"\n")
    return False, (f"items: paragraphs -{sum(only_a.values())} +{sum(only_b.values())}, "
                   f"lines -{sum(ra.values())} +{sum(rb.values())}")


def one(args, src, level, idx):
    base = os.path.join(args.out, f"{idx:05d}_{level}")
    a, b = base + ".ref.c", base + ".new.c"
    for p in (a, b):
        if os.path.exists(p):
            os.remove(p)
    t0 = time.time()
    sa, ea = emit(args.ref, src, level, a, {})
    t1 = time.time()
    sb, eb = emit(args.new, src, level, b, args.envd)
    t2 = time.time()
    # The verdict check's own lines (`COBALTC_VERDICT=check`, step T3) are not the compiler's messages.
    eb = b"".join(l for l in eb.splitlines(keepends=True) if not l.startswith(b"VERDICT-"))
    da, db = digest(a), digest(b)
    what = []
    if sa != sb:
        what.append(f"status {sa} vs {sb}")
    if ea != eb:
        what.append("stderr")
    if da != db:
        if args.items and da is not None and db is not None:
            reordered, summary = items_diff(a, b, base + ".items")
            what.append("reordered" if reordered else summary)
        else:
            what.append("C")
    if not what:
        for p in (a, b):
            if os.path.exists(p):
                os.remove(p)
    else:
        with open(base + ".stderr", "wb") as f:
            f.write(b"--- ref\n" + ea + b"\n--- new\n" + eb)
    return src, level, what, sa, t1 - t0, t2 - t1, da is not None


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--ref", required=True)
    ap.add_argument("--new", required=True)
    ap.add_argument("--levels", default="0,10")
    ap.add_argument("--jobs", type=int, default=3)
    ap.add_argument("--only", default="")
    ap.add_argument("--set", default="small")
    ap.add_argument("--env", action="append", default=[])
    ap.add_argument("--out", default=str(root / "stress/cir/cmp"))
    ap.add_argument("--items", action="store_true")
    args = ap.parse_args()
    args.envd = dict(e.split("=", 1) for e in args.env)
    args.ref, args.new = os.path.abspath(args.ref), os.path.abspath(args.new)
    os.makedirs(args.out, exist_ok=True)
    files = [f for f in corpus(args.set) if args.only in f]
    levels = [int(x) for x in args.levels.split(",")]
    jobs = [(f, lv, i) for i, f in enumerate(files) for lv in levels]
    bad = same = lowered = moved = 0
    tref = tnew = 0.0
    with concurrent.futures.ThreadPoolExecutor(args.jobs) as ex:
        for src, level, what, status, ta, tb, has_c in ex.map(lambda j: one(args, *j), jobs):
            tref += ta
            tnew += tb
            lowered += 1 if has_c else 0
            if what == ["reordered"]:
                moved += 1
            elif what:
                bad += 1
                print(f"DIFF level {level} {os.path.relpath(src, root)}: {', '.join(what)}", flush=True)
            else:
                same += 1
    print(f"cmpc: {len(jobs)} runs ({len(files)} programs, levels {levels}): {same} equal, {moved} reordered, {bad} differ; "
          f"{lowered} lowered to C; cobc time ref {tref:.0f} s, new {tnew:.0f} s")
    sys.exit(1 if bad else 0)


main()
