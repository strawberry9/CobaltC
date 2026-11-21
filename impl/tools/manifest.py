#!/usr/bin/env python3
"""The edition manifest (D-0212): writes spec/MANIFEST.md from the tree.

    python3 impl/tools/manifest.py [--tier reviewed|tested|battery] [--explored N]
    python3 impl/tools/manifest.py --check          # exit 1 when the file is stale
    python3 impl/tools/manifest.py --fingerprint    # print the tree fingerprint

The manifest records the edition, the SHA-256 of every file under spec/
(the manifest itself excepted), each artifact's Version, the highest D and
CHG numbers, the conformance cases (count, listing hash, content hash), the
toolchain and result of the last battery, and the evidence tier. It holds
no date of its own: a second run on an unchanged tree rewrites nothing.

The tree fingerprint is the SHA-256 over every file git tracks or would
track (tracked and untracked, not ignored), spec/MANIFEST.md and the
top-level README.md excepted (CHG-0256: its "Commit Update" line is written
at push time, after any run, and no test reads it), as "path NUL sha256
NUL" in sorted order. A battery writes it into its SUMMARY; when the newest
green SUMMARY carries the current fingerprint, the tier is `battery`
whatever --tier says. impl/tools/suites.sh writes it into its TESTED
record; `tested` is claimed as such only when the newest passing TESTED
carries the current fingerprint, and says "no record for this tree"
otherwise (CHG-0256).

A scoped battery records the green tree its scope was computed against
(`base`); the stages and parts it left out are that run's. When it did not
compare the model's sample itself, *modelled* is carried from the run it
was scoped against, through any chain of scoped runs, back to one that did
(CHG-0263): its scope rule reruns the comparison whenever a changed file
could change the outcome.

Python 3 standard library only; runs on Windows."""
import argparse
import hashlib
import os
import re
import subprocess
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.abspath(os.path.join(HERE, "..", ".."))
SPEC = os.path.join(ROOT, "spec")
MANIFEST = os.path.join(SPEC, "MANIFEST.md")
TIERS = ("reviewed", "tested", "battery")


def sha256_file(path):
    h = hashlib.sha256()
    with open(path, "rb") as f:
        for chunk in iter(lambda: f.read(1 << 20), b""):
            h.update(chunk)
    return h.hexdigest()


def posix(rel):
    return rel.replace(os.sep, "/")


def edition():
    with open(os.path.join(SPEC, "README.md"), encoding="utf-8") as f:
        m = re.search(r"\*\*CobaltC specification edition (\S+?)\.\*\*", f.read())
    if not m:
        sys.exit("manifest: spec/README.md names no edition")
    return m.group(1)


def version_of(path):
    with open(path, encoding="utf-8", errors="replace") as f:
        for _ in range(12):
            line = f.readline()
            m = re.match(r"Version:\s*(\S+)", line)
            if m:
                return m.group(1)
    return ""


def spec_files():
    out = []
    for dirpath, dirnames, filenames in os.walk(SPEC):
        dirnames.sort()
        for n in sorted(filenames):
            p = os.path.join(dirpath, n)
            if os.path.abspath(p) == MANIFEST:
                continue
            out.append((posix(os.path.relpath(p, ROOT)), p))
    return out


def highest(dirname, prefix):
    best = 0
    d = os.path.join(SPEC, dirname)
    for n in os.listdir(d):
        m = re.match(prefix + r"-(\d{4})", n)
        if m:
            best = max(best, int(m.group(1)))
    return best


def conformance_cases():
    root = os.path.join(ROOT, "impl", "conformance")
    files = []
    for dirpath, dirnames, filenames in os.walk(root):
        dirnames.sort()
        for n in sorted(filenames):
            if n.endswith(".cb"):
                p = os.path.join(dirpath, n)
                files.append((posix(os.path.relpath(p, ROOT)), p))
    files.sort()
    listing = hashlib.sha256("\n".join(r for r, _ in files).encode("utf-8")).hexdigest()
    content = hashlib.sha256()
    for r, p in files:
        content.update(r.encode("utf-8") + b"\0" + sha256_file(p).encode("ascii") + b"\0")
    return len(files), listing, content.hexdigest()


def git_files():
    try:
        out = subprocess.run(["git", "-C", ROOT, "ls-files", "-z", "-co", "--exclude-standard"],
                             capture_output=True, check=True).stdout
    except (OSError, subprocess.CalledProcessError):
        return None
    files = [f.decode("utf-8", "surrogateescape") for f in out.split(b"\0") if f]
    return sorted(set(files))


def fingerprint():
    files = git_files()
    if files is None:
        return None
    h = hashlib.sha256()
    for rel in files:
        if rel in ("spec/MANIFEST.md", "README.md"):
            continue
        p = os.path.join(ROOT, *rel.split("/"))
        if not os.path.isfile(p):
            continue
        h.update(rel.encode("utf-8", "surrogateescape") + b"\0" + sha256_file(p).encode("ascii") + b"\0")
    return h.hexdigest()


def battery_summaries():
    """Every SUMMARY a battery wrote, oldest first, as dicts (with `_dir`)."""
    candidates = []
    for base in (os.path.join(ROOT, "stress", "battery_staged", "runs"),
                 os.path.join(os.path.expanduser("~"), ".cobaltc-battery", "runs")):
        if not os.path.isdir(base):
            continue
        for n in os.listdir(base):
            p = os.path.join(base, n, "SUMMARY")
            if os.path.isfile(p):
                candidates.append((os.path.getmtime(p), p))
    out = []
    for _, p in sorted(candidates):
        d = {"_dir": os.path.dirname(p)}
        with open(p, encoding="utf-8", errors="replace") as f:
            for line in f:
                k, _, v = line.rstrip("\n").partition(" ")
                if k:
                    d[k] = v.strip()
        out.append(d)
    return out


def battery_summary():
    """The newest SUMMARY a battery wrote, as a dict, or None."""
    sums = battery_summaries()
    return sums[-1] if sums else None


def scoped_base(d):
    """The green tree a scoped run was scoped against: its SUMMARY's `base`,
    or, for a run written before CHG-0263, the one its scope.txt names."""
    if not d.get("scope", "").startswith("scoped"):
        return None
    if d.get("base"):
        return d["base"]
    try:
        with open(os.path.join(d["_dir"], "scope.txt"), encoding="utf-8", errors="replace") as f:
            m = re.search(r"since last green battery \(([0-9a-f]+)\)", f.readline())
    except OSError:
        return None
    return m.group(1) if m else None


def compared_model(d):
    """Whether a scoped run's scope held the model comparison (stage 1's
    `tests` part): then its own `modelled`, or none, is the answer."""
    m = re.match(r"scoped: stages ([0-9 ]*) \(stage 1 parts: ([^)]*)\)", d.get("scope", ""))
    if not m:
        return True
    return "1" in m.group(1).split() and "tests" in m.group(2).split()


def modelled_chain(summ, sums):
    """(programs, [runs]) when summ, or the green run it was scoped against
    (recursively), compared the model's sample; else (None, [])."""
    chain = []
    seen = set()
    d = summ
    while d is not None and d.get("run") not in seen:
        seen.add(d.get("run"))
        chain.append(d.get("run", "?"))
        if d.get("modelled"):
            return d["modelled"], chain
        base = scoped_base(d)
        if base is None or compared_model(d):
            return None, []
        greens = [x for x in sums if x.get("tree") == base and x.get("result", "").startswith("DONE")]
        d = greens[-1] if greens else None
    return None, []


def tested_record(fp):
    """The newest TESTED impl/tools/suites.sh wrote whose fingerprint is fp
    and whose result is PASSED, as a dict, else the newest of any, else None."""
    candidates = []
    for base in (os.path.join(ROOT, "stress", "suites", "runs"),
                 os.path.join(os.path.expanduser("~"), ".cobaltc-suites", "runs")):
        if not os.path.isdir(base):
            continue
        for n in os.listdir(base):
            p = os.path.join(base, n, "TESTED")
            if os.path.isfile(p):
                candidates.append((os.path.getmtime(p), p))
    if not candidates:
        return None
    recs = []
    for _, p in sorted(candidates):
        d = {"targets": []}
        with open(p, encoding="utf-8", errors="replace") as f:
            for line in f:
                k, _, v = line.rstrip("\n").partition(" ")
                if k.startswith("target-"):
                    d["targets"].append((k[len("target-"):], v.strip()))
                elif k:
                    d[k] = v.strip()
        recs.append(d)
    for d in reversed(recs):
        if d.get("result") == "PASSED" and fp is not None and d.get("fingerprint") == fp:
            return d
    return recs[-1]


def gate_verdicts():
    """D-0307: each unit of the gate (impl/tools/gate.py) with its green verdict kept for this tree's inputs, or
    None; the tree is the index (the commit being written)."""
    sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
    try:
        import gate
    except Exception:
        return []
    return [(u, gate.kept(gate.ROOT, u)) for u in gate.UNITS]


def render(tier, explored):
    ed = edition()
    files = spec_files()
    n_cases, listing, content = conformance_cases()
    fp = fingerprint()
    hi_d, hi_chg = highest("decisions", "D"), highest("changes", "CHG")
    verdicts = gate_verdicts()
    gating = [(u, v) for u, v in verdicts if not u.get("advisory")]
    tested = bool(gating) and all(v for u, v in gating if u["profile"] == "gate")
    battery = tested and all(v for u, v in gating)
    # The tier is the gate's (D-0307): the claim is taken only as far as the verdicts bear it out.
    derived = "battery" if battery else ("tested" if tested else "reviewed")
    order = {t: i for i, t in enumerate(TIERS)}
    tier = derived if order.get(tier, 0) >= order[derived] else tier
    words = tier + (" and explored to %d" % explored if explored else "")
    model = [v for u, v in verdicts if u["name"] == "model"]
    if tier != "reviewed" and model and model[0]:
        words += ", modelled"
    L = []
    L.append("# CobaltC edition manifest")
    L.append("")
    L.append("Generated by `impl/tools/manifest.py` (D-0212); not a normative artifact. It names what edition "
             "**%s** consists of, so that a conformance claim or a review can say which bytes it looked at." % ed)
    L.append("")
    L.append("| | |")
    L.append("|---|---|")
    L.append("| Edition | `%s` |" % ed)
    L.append("| Evidence tier | **%s** (`spec/README.md` \"Evidence tiers\") |" % words)
    L.append("| Highest decision | `D-%04d` |" % hi_d)
    L.append("| Highest change record | `CHG-%04d` |" % hi_chg)
    L.append("| Tree fingerprint | `%s` |" % (fp or "(no git: not computed)"))
    L.append("| Conformance cases | %d files under `impl/conformance/`; listing `%s`; contents `%s` |" % (n_cases, listing, content))
    L.append("")
    L.append("## The gate")
    L.append("")
    L.append("Each unit of `impl/gate.sh` (D-0307) with the green verdict kept for this tree's inputs: the run, the "
             "tree it ran on and when. A unit without one has not been run on these inputs.")
    L.append("")
    L.append("| Unit | Profile | Verdict | Run | Tree | When |")
    L.append("|---|---|---|---|---|---|")
    for u, v in verdicts:
        if v:
            L.append("| `%s` | %s | green | `%s` | `%s` | %s |" % (u["name"], u["profile"], v.get("run", ""), v.get("tree", "")[:12], v.get("when", "")))
        else:
            L.append("| `%s` | %s | none%s | | | |" % (u["name"], u["profile"], " (advisory)" if u.get("advisory") else ""))
    if verdicts:
        v0 = next((v for u, v in verdicts if v), None)
        if v0:
            L.append("")
            L.append("Toolchain of the runs: %s." % "; ".join("`%s`" % t for t in v0.get("toolchain", "").split("\n") if t))
    L.append("")
    L.append("## Artifacts")
    L.append("")
    L.append("Every file under `spec/` but this one, with its `Version` where it carries one.")
    L.append("")
    L.append("| File | Version | SHA-256 |")
    L.append("|---|---|---|")
    for rel, p in files:
        L.append("| `%s` | %s | `%s` |" % (rel, version_of(p) or "", sha256_file(p)))
    L.append("")
    return "\n".join(L) + "\n"


def main():
    ap = argparse.ArgumentParser(description="write spec/MANIFEST.md")
    ap.add_argument("--tier", choices=TIERS, default="reviewed", help="the evidence tier claimed (default reviewed); "
                    "`battery` is taken only from a matching battery SUMMARY")
    ap.add_argument("--explored", type=int, default=0, metavar="N", help="thread cases were run under --explore N")
    ap.add_argument("--check", action="store_true", help="exit 1 when spec/MANIFEST.md is not what this tree gives")
    ap.add_argument("--fingerprint", action="store_true", help="print the tree fingerprint and exit")
    a = ap.parse_args()
    if a.fingerprint:
        fp = fingerprint()
        if fp is None:
            sys.exit("manifest: git not available")
        print(fp)
        return
    text = render(a.tier, a.explored)
    if a.check:
        try:
            with open(MANIFEST, encoding="utf-8") as f:
                old = f.read()
        except OSError:
            old = ""
        # The tier line and the gate's verdicts change with the runs, not the tree: not compared.
        def strip(t):
            out, skip = [], False
            for l in t.splitlines():
                if l.startswith("## "):
                    skip = l == "## The gate"
                if not skip and not l.startswith("| Evidence tier |"):
                    out.append(l)
            return "\n".join(out)
        if strip(old) != strip(text):
            sys.exit("manifest: spec/MANIFEST.md is stale; run impl/tools/manifest.py")
        print("manifest: up to date")
        return
    try:
        with open(MANIFEST, encoding="utf-8") as f:
            old = f.read()
    except OSError:
        old = None
    if old == text:
        print("manifest: unchanged")
        return
    with open(MANIFEST, "w", encoding="utf-8", newline="\n") as f:
        f.write(text)
    print("manifest: wrote spec/MANIFEST.md (edition %s)" % edition())


if __name__ == "__main__":
    main()
