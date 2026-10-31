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


def render(tier, explored):
    ed = edition()
    files = spec_files()
    n_cases, listing, content = conformance_cases()
    fp = fingerprint()
    sums = battery_summaries()
    summ = sums[-1] if sums else None
    hi_d, hi_chg = highest("decisions", "D"), highest("changes", "CHG")
    green = summ is not None and summ.get("result", "").startswith("DONE") and fp is not None and summ.get("fingerprint") == fp
    if green:
        tier = "battery"
    elif tier == "battery":
        tier = "tested"  # a battery claim needs the summary's fingerprint to match
    rec = tested_record(fp)
    rec_ok = rec is not None and rec.get("result") == "PASSED" and fp is not None and rec.get("fingerprint") == fp
    if tier == "tested" and rec_ok and rec.get("explored", "").isdigit():
        explored = max(explored, int(rec["explored"]))
    words = tier + (" and explored to %d" % explored if explored else "")
    # modelled (D-0219): the run the tier names compared impl/model/sample with
    # both tools and every outcome was one the model allows.
    modelled, mchain = modelled_chain(summ, sums) if green else (None, [])
    if modelled or (tier == "tested" and rec_ok and rec.get("modelled")):
        words += ", modelled"
    if tier == "tested" and not rec_ok:
        words += " (no record for this tree)"
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
    L.append("## The last battery")
    L.append("")
    if summ is None:
        L.append("No battery summary found (`impl/battery.sh` writes `SUMMARY` into its run directory).")
    else:
        L.append("| | |")
        L.append("|---|---|")
        for k, label in (("run", "Run"), ("result", "Result"), ("scope", "Scope"), ("fingerprint", "Tree fingerprint"),
                         ("rustc", "rustc"), ("gcc", "gcc"), ("clang", "clang"), ("kernel", "Kernel")):
            if k in summ:
                L.append("| %s | `%s` |" % (label, summ[k].replace("|", "\\|")))
        if scoped_base(summ):
            L.append("| Scoped against | green tree `%s` |" % scoped_base(summ))
        if modelled and len(mchain) > 1:
            L.append("| Modelled | %s programs, in run `%s` (carried through %s) |" % (
                modelled, mchain[-1], ", ".join("`%s`" % r for r in mchain[:-1])))
        elif modelled:
            L.append("| Modelled | %s programs |" % modelled)
        L.append("")
        L.append("The tier is `battery` only when that run's fingerprint is this tree's; it is %s." % ("so" if green else "not"))
    L.append("")
    L.append("## The last suite run")
    L.append("")
    if rec is None:
        L.append("No suite record found (`impl/tools/suites.sh` writes `TESTED` into its run directory).")
    else:
        L.append("| | |")
        L.append("|---|---|")
        for k, label in (("run", "Run"), ("result", "Result"), ("fingerprint", "Tree fingerprint"), ("explored", "Explored"),
                         ("rustc", "rustc"), ("gcc", "gcc"), ("clang", "clang"), ("kernel", "Kernel")):
            if k in rec:
                L.append("| %s | `%s` |" % (label, rec[k].replace("|", "\\|")))
        for name, res in rec.get("targets", []):
            L.append("| `%s` | %s |" % (name, res.replace("|", "\\|")))
        L.append("")
        L.append("The tier `tested` names this run only when its fingerprint is this tree's; it is %s." % ("so" if rec_ok else "not"))
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
        # The tier line is the writer's claim: not compared.
        strip = lambda s: "\n".join(l for l in s.splitlines() if not l.startswith("| Evidence tier |"))
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
