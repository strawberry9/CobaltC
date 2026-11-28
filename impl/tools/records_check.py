#!/usr/bin/env python3
"""The records each push keeps up (run by impl/tools/pregate.py). Each is a hand-written line the gate's
generators do not write, and each has been left behind before:

  - README.md's "Commit Update N, … Specification edition E." line, spec/README.md's bold edition and the
    register's newest row name the same edition, and the line the same Update as the row;
  - the register holds the ten most recent editions (spec/README.md; the rest are in spec/EDITIONS-HISTORY.md),
    newest last, each later than the one before;
  - the register's newest row names the newest change record (a push without its row);
  - spec/MANIFEST.md names the same edition (its other contents: impl/tools/manifest.py --check);
  - the newest D-record is in impl/CHANGELOG.md's list of decisions and has a section there;
  - every spec file with a `Version:` line carries the version of its newest change-log entry, and every version
    the newest change record's `Affects:` line names is at most the file's own.

    python3 impl/tools/records_check.py      # exit 1, with each disagreement, when any fails
"""
import glob
import os
import re
import sys

ROOT = os.path.abspath(os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", ".."))


def read(rel):
    with open(os.path.join(ROOT, rel), encoding="utf-8") as f:
        return f.read()


def vkey(v):
    return tuple(int(x) for x in v.split("."))


def main():
    bad = []
    readme = read("README.md")
    m = re.search(r"^Commit Update (\d+), (\d{4}-\d\d-\d\d) [0-9:]+ UTC\. Specification edition (\d{4}\.\d{6})\.",
                  readme, re.M)
    if not m:
        bad.append("README.md: no \"Commit Update N, YYYY-MM-DD HH:MM UTC. Specification edition E.\" line")
    spec = read("spec/README.md")
    b = re.search(r"^\*\*CobaltC specification edition (\d{4}\.\d{6})\.\*\*", spec, re.M)
    if not b:
        bad.append("spec/README.md: no bold edition line")
    rows = re.findall(r"^\| (\d{4}\.\d{6}) \| Commit Update (\d+), (\d{4}-\d\d-\d\d) \|", spec, re.M)
    if len(rows) != 10:
        bad.append("spec/README.md: the register holds %d editions, not the ten most recent (move the older rows, "
                   "unchanged, to the end of spec/EDITIONS-HISTORY.md)" % len(rows))
    for a, c in zip(rows, rows[1:]):
        if c[0] <= a[0]:
            bad.append("spec/README.md: edition %s follows %s" % (c[0], a[0]))
    for e, u, d in rows:
        if e[:4] + e[5:9] != d[:4] + d[5:7] + d[8:10]:
            bad.append("spec/README.md: edition %s is not dated %s (editions follow the UTC date)" % (e, d))
    if rows:
        e, u, d = rows[-1]
        if b and b.group(1) != e:
            bad.append("spec/README.md: the bold edition %s is not the register's newest, %s" % (b.group(1), e))
        if m and (m.group(3) != e or m.group(1) != u):
            bad.append("README.md: names Update %s, edition %s; the register's newest row is Update %s, edition %s"
                       % (m.group(1), m.group(3), u, e))
        if m and m.group(2) != d:
            bad.append("README.md: dated %s; the register's newest row is dated %s" % (m.group(2), d))
        hist = read("spec/EDITIONS-HISTORY.md")
        old = re.findall(r"^\| (\d{4}\.\d{6}) \|", hist, re.M)
        if old and rows and old[-1] >= rows[0][0]:
            bad.append("spec/EDITIONS-HISTORY.md: its newest edition %s is not older than the register's oldest %s"
                       % (old[-1], rows[0][0]))
        mf = re.search(r"^\| Edition \| `(\d{4}\.\d{6})` \|", read("spec/MANIFEST.md"), re.M)
        if not mf or mf.group(1) != e:
            bad.append("spec/MANIFEST.md: names edition %s, not %s (run impl/tools/manifest.py)"
                       % (mf.group(1) if mf else "none", e))
    ds = sorted(glob.glob(os.path.join(ROOT, "spec", "decisions", "D-[0-9][0-9][0-9][0-9]-*.md")))
    if ds:
        d = os.path.basename(ds[-1])[:6]
        log = read("impl/CHANGELOG.md")
        if not re.search(r"^- %s — " % d, log, re.M):
            bad.append("impl/CHANGELOG.md: %s is not in the list of decisions" % d)
        if not re.search(r"^%s[,.]" % d, log, re.M):
            bad.append("impl/CHANGELOG.md: no section for %s (a section's first line starts \"%s, `CHG-…`\")" % (d, d))
    versions = {}
    for f in sorted(glob.glob(os.path.join(ROOT, "spec", "*.md")) + glob.glob(os.path.join(ROOT, "spec", "registry", "*.md"))):
        rel = os.path.relpath(f, ROOT)
        t = read(rel)
        v = re.search(r"^Version: (\d+\.\d+\.\d+)\s*$", t, re.M)
        if not v:
            continue
        versions[rel] = v.group(1)
        cl = t.split("## Change Log", 1)
        if len(cl) == 2:
            top = re.search(r"^- (\d+\.\d+\.\d+) —", cl[1], re.M)
            if top and top.group(1) != v.group(1):
                bad.append("%s: Version %s, but its newest change-log entry is %s" % (rel, v.group(1), top.group(1)))
    chg = sorted(glob.glob(os.path.join(ROOT, "spec", "changes", "CHG-[0-9][0-9][0-9][0-9]-*.md")))
    if chg and rows:
        c = os.path.basename(chg[-1])[:8]
        newest = [l for l in spec.splitlines() if l.startswith("| %s |" % rows[-1][0])][0]
        if c not in newest:
            bad.append("spec/README.md: the newest change record %s is in no edition (the register's newest row, "
                       "%s, does not name it): add the push's row" % (c, rows[-1][0]))
    if chg:
        t = read(os.path.relpath(chg[-1], ROOT))
        a = re.search(r"^Affects:(.*?)(?:\n\n|\n## )", t, re.M | re.S)
        for name, ver in re.findall(r"`spec/([A-Za-z0-9_./-]+)` (\d+\.\d+\.\d+)", a.group(1) if a else ""):
            hits = [r for r in versions if r == "spec/" + name or os.path.basename(r).startswith(name + "-")
                    or os.path.basename(r) == name]
            for r in hits:
                if vkey(versions[r]) < vkey(ver):
                    bad.append("%s: Version %s, but %s says %s" % (r, versions[r], os.path.basename(chg[-1])[:8], ver))
    for x in bad:
        print("records: " + x)
    print("records: %s" % ("%d disagreement(s)" % len(bad) if bad else "up to date"))
    return 1 if bad else 0


if __name__ == "__main__":
    sys.exit(main())
