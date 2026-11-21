#!/usr/bin/env python3
"""The trusted library base, listed (D-0206, `term.trusted-library-base`).

Every `unsafe { }` block in `impl/std/**/*.cb` must be covered by a
`// trusted: ...` comment: one directly above the block, or one in the
comment block directly above the function that holds it (the nearest
preceding one applies, so one comment may cover every block of a
function). The comment names the trusted rule(s) in brackets, as
`[Rawptr-Write]`, and says why their side-conditions hold there.

    python3 impl/tools/trusted_table.py            # writes the table into spec/21 §0
    python3 impl/tools/trusted_table.py --check    # exits 1 if the table is stale or a block is uncovered

The table is written between the markers `<!-- trusted-table:begin -->`
and `<!-- trusted-table:end -->` in spec/21-standard-library-semantics.md.
Standard library only, no third-party modules; runs on Windows too.
"""
import os
import re
import sys

ROOT = os.path.dirname(os.path.dirname(os.path.dirname(os.path.abspath(__file__))))
STD = os.path.join(ROOT, "impl", "std")
SPEC21 = os.path.join(ROOT, "spec", "21-standard-library-semantics.md")
BEGIN = "<!-- trusted-table:begin -->"
END = "<!-- trusted-table:end -->"

UNSAFE = re.compile(r"(^|[\s(=])unsafe\s*\{?\s*$")
FN = re.compile(r"^(\s*)(export\s+)?fn\s+([A-Za-z_][A-Za-z0-9_:]*)")
TRUSTED = re.compile(r"^\s*//\s*trusted:\s*(.*)$")
RULE = re.compile(r"\[[A-Z][A-Za-z0-9-]*\]")


def module_name(rel):
    parts = rel.replace("\\", "/")[:-3].split("/")
    if parts == ["std"]:
        return "std"
    return "std::" + "::".join(parts)


def scan(path):
    lines = open(path, encoding="utf-8").read().split("\n")
    out = []
    errors = []
    for i, line in enumerate(lines):
        if not UNSAFE.search(line) or "unsafe extern" in line or line.lstrip().startswith("//"):
            continue
        # the function holding the block
        h = None
        for j in range(i, -1, -1):
            m = FN.match(lines[j])
            if m and len(m.group(1)) < len(line) - len(line.lstrip()):
                h = j
                break
        if h is None:
            errors.append("%s:%d: unsafe block outside any function" % (path, i + 1))
            continue
        # the nearest preceding `// trusted:` line, inside the function or in its header comment
        found = None
        for j in range(i - 1, h, -1):
            t = TRUSTED.match(lines[j])
            if t:
                found = t.group(1).strip()
                break
        if found is None:
            j = h - 1
            while j >= 0 and lines[j].lstrip().startswith("//"):
                t = TRUSTED.match(lines[j])
                if t:
                    found = t.group(1).strip()
                    break
                j -= 1
        if found is None:
            errors.append("%s:%d: unsafe block in `%s` has no `// trusted:` comment" % (path, i + 1, FN.match(lines[h]).group(3)))
            continue
        out.append((FN.match(lines[h]).group(3), i + 1, found))
    return out, errors


def collect():
    rows = []
    errors = []
    for dirpath, _, files in os.walk(STD):
        for f in sorted(files):
            if not f.endswith(".cb"):
                continue
            path = os.path.join(dirpath, f)
            rel = os.path.relpath(path, STD)
            blocks, errs = scan(path)
            errors += errs
            mod = module_name(rel)
            for name, line, why in blocks:
                rows.append((mod, name, rel.replace("\\", "/"), line, why))
    rows.sort(key=lambda r: (r[0], r[2], r[3]))
    return rows, errors


def table(rows):
    # one row per (module, function, comment): the lines of the blocks it covers, joined
    merged = []
    for mod, name, rel, line, why in rows:
        if merged and merged[-1][0] == mod and merged[-1][1] == name and merged[-1][4] == why:
            merged[-1][3].append(line)
        else:
            merged.append([mod, name, rel, [line], why])
    out = ["| Module | Item | Block(s) | Rule(s) | Why the side-conditions hold |", "|---|---|---|---|---|"]
    for mod, name, rel, ls, why in merged:
        rules = ", ".join("`%s`" % r for r in dict.fromkeys(RULE.findall(why)))
        text = RULE.sub(lambda m: "`%s`" % m.group(0), why)
        out.append("| `%s` | `%s` | `%s` %s | %s | %s |" % (mod, name, rel, ", ".join(str(l) for l in ls), rules, text))
    return "\n".join(out)


def main():
    rows, errors = collect()
    if errors:
        for e in errors:
            print(e, file=sys.stderr)
        return 1
    body = table(rows)
    spec = open(SPEC21, encoding="utf-8").read()
    b = spec.find(BEGIN)
    e = spec.find(END)
    if b < 0 or e < 0 or e < b:
        print("spec/21: markers %s / %s missing" % (BEGIN, END), file=sys.stderr)
        return 1
    new = spec[: b + len(BEGIN)] + "\n" + body + "\n" + spec[e:]
    if "--check" in sys.argv:
        if new != spec:
            print("spec/21 §0's trusted table is stale: run impl/tools/trusted_table.py", file=sys.stderr)
            return 1
        print("trusted table: %d blocks in %d functions, up to date" % (len(rows), len({(r[0], r[1]) for r in rows})))
        return 0
    if new != spec:
        open(SPEC21, "w", encoding="utf-8", newline="\n").write(new)
    print("trusted table: %d blocks in %d functions written" % (len(rows), len({(r[0], r[1]) for r in rows})))
    return 0


if __name__ == "__main__":
    sys.exit(main())
