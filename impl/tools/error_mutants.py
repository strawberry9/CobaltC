#!/usr/bin/env python3
"""Programs with one to three injected static errors, and what a checker reports for them (D-0323).

D-0323 lets a body report an error for each of its statements that stands on its own. This tool measures the
promise that goes with it: no error is reported that follows from another. From each accepted single-file
program it writes mutants with one, two or three injected errors, each on its own line inside a function body,
of three kinds:

  name     a use of a local replaced by a name nothing declares (`count` -> `count_zq`);
  literal  an integer literal that is an argument, an operand or a typed declaration's value replaced by
           `true` (never one an `auto` declaration takes its type from);
  call     the last letter of a qualified function's name doubled (`Vec::len` -> `Vec::lenn`).

A mutant with several injections is written with each of them alone beside it. The errors injected
separately are independent, so the report of the whole must be the union of the reports of its parts: a
diagnostic in the whole that no part reports is a follow-on (an error that exists only because another
statement failed first). `run` checks every mutant and reports:

  - each such follow-on (there must be none), with its first line;
  - each part (one injection) whose report has more than one error, to be read: one mistake used in two
    statements is two true errors, but nothing else should give two;
  - of the mutants with two or three injections, how many reported every part's errors (the gain), and how
    many reported only their first (what D-0321 alone gave).

    python3 impl/tools/error_mutants.py make OUT [--per N] [--seed S] FILE.cb...
    python3 impl/tools/error_mutants.py run OUT [--tool "impl/target/release/cobc --check"] [--jobs N]

Standard library only; runs on Windows (the `run` mode needs a `timeout`-free subprocess only).
"""
import json
import os
import random
import re
import subprocess
import sys
from concurrent.futures import ThreadPoolExecutor

DECL = re.compile(r"^\s*(?:auto|[A-Za-z_][\w:]*(?:<[^=;]*>)?)\s+([a-z_]\w*)\s*(?:=|;)")
QUAL = re.compile(r"\b([A-Z]\w*)::([a-z_]\w*)\(")
LIT = re.compile(r"(?<=[(,=+\-*<>] )(\d+)(?=[\s,);])|(?<=\()(\d+)(?=[,)])")
KEYWORDS = {"fn", "if", "else", "while", "for", "foreach", "in", "match", "return", "break", "continue", "auto", "true",
            "false", "import", "std", "module", "struct", "enum", "const", "unsafe", "move", "drop", "export",
            "resource", "extern", "unittest", "bitstruct", "Some", "None", "Ok", "Err", "assert"}


def outside_text(line, pos):
    """Whether column `pos` of `line` is outside a string or byte literal and before a `//` comment."""
    quote = None
    i = 0
    while i < pos:
        c = line[i]
        if quote:
            if c == "\\":
                i += 2
                continue
            if c == quote:
                quote = None
        elif c == '"' or (c == "'" and i > 0 and line[i - 1] == "b"):
            quote = c
        elif line.startswith("//", i):
            return False
        i += 1
    return quote is None


def bodies(lines):
    """The 0-based indices of the lines inside a function body (depth counted from a line beginning `fn`)."""
    inside = []
    depth = 0
    in_fn = False
    for i, l in enumerate(lines):
        s = l.strip()
        if depth == 0 and (s.startswith("fn ") or s.startswith("export fn ")):
            in_fn = True
        opened = l.count("{")
        closed = l.count("}")
        if in_fn and depth > 0 and not s.startswith("//") and s not in ("{", "}"):
            inside.append(i)
        depth += opened - closed
        if depth <= 0 and closed:
            depth = 0
            in_fn = False
    return inside


def candidates(lines):
    """Every (line index, kind, column, old, new) injection the program admits."""
    inside = set(bodies(lines))
    declared = {}
    out = []
    for i, l in enumerate(lines):
        m = DECL.match(l)
        if m and i in inside:
            declared.setdefault(m.group(1), i)
    for i in sorted(inside):
        l = lines[i]
        s = l.strip()
        if s.startswith("//") or s.startswith("unittest"):
            continue
        for name, at in declared.items():
            if at >= i or name in KEYWORDS or len(name) < 2:
                continue
            for m in re.finditer(r"(?<![\w.:$])%s(?![\w(:])" % re.escape(name), l):
                if outside_text(l, m.start()) and not DECL.match(l[: m.end()] + " ="):
                    out.append((i, "name", m.start(), name, name + "_zq"))
                    break
        if not s.startswith("auto ") and "[" not in s:
            for m in LIT.finditer(l):
                g = m.group(1) or m.group(2)
                start = m.start(1) if m.group(1) else m.start(2)
                if outside_text(l, start) and ":" not in l[start + len(g): start + len(g) + 2]:
                    out.append((i, "literal", start, g, "true"))
                    break
        for m in QUAL.finditer(l):
            if outside_text(l, m.start()):
                out.append((i, "call", m.start(2), m.group(2), m.group(2) + m.group(2)[-1]))
                break
    return out


def write_mutant(d, lines, meta):
    os.makedirs(d, exist_ok=True)
    with open(os.path.join(d, "m.cb"), "w", encoding="utf-8") as fh:
        fh.write("\n".join(lines))
    with open(os.path.join(d, "injected.json"), "w") as fh:
        json.dump(meta, fh)


def make(out_dir, files, per, seed):
    rng = random.Random(seed)
    os.makedirs(out_dir, exist_ok=True)
    made = 0
    for path in files:
        lines = open(path, encoding="utf-8").read().split("\n")
        cands = candidates(lines)
        by_line = {}
        for c in cands:
            by_line.setdefault(c[0], []).append(c)
        if not by_line:
            continue
        base = os.path.splitext(os.path.basename(path))[0]
        keys = sorted(by_line)
        for k in range(per):
            n = 1 + k % 3
            if len(keys) < n:
                continue
            chosen = rng.sample(keys, n)
            new = list(lines)
            inj = []
            for li in sorted(chosen):
                i, kind, col, old, rep = rng.choice(by_line[li])
                new[i] = new[i][:col] + rep + new[i][col + len(old):]
                inj.append({"line": i + 1, "kind": kind, "old": old, "new": rep})
            name = "%s_%03d" % (base, k)
            write_mutant(os.path.join(out_dir, name), new, {"source": path, "injected": inj, "parts": [] if n == 1 else ["%s_p%d" % (name, j) for j in range(n)]})
            if n > 1:
                for j, li in enumerate(sorted(chosen)):
                    part = list(lines)
                    x = inj[j]
                    i = x["line"] - 1
                    col = [c for c in by_line[li] if c[3] == x["old"] and c[4] == x["new"]][0][2]
                    part[i] = part[i][:col] + x["new"] + part[i][col + len(x["old"]):]
                    write_mutant(os.path.join(out_dir, "%s_p%d" % (name, j)), part, {"source": path, "injected": [x], "parts": [], "part_of": name})
            made += 1
    print("%d mutants in %s" % (made, out_dir))


ARROW = re.compile(r"^\s*--> (.+):(\d+):(\d+)\s*$")


def report_of(text):
    """The (first line, file, line) of each diagnostic in a report."""
    out = []
    lines = text.split("\n")
    for i, l in enumerate(lines):
        if l.startswith("error[") or l.startswith("warning["):
            where = None
            for j in range(i + 1, min(i + 4, len(lines))):
                m = ARROW.match(lines[j])
                if m:
                    where = (m.group(1), int(m.group(2)))
                    break
            out.append((l, where))
    return out


def run_one(d, tool):
    try:
        p = subprocess.run(tool + ["m.cb"], cwd=d, capture_output=True, text=True, timeout=120)
        return d, p.returncode, p.stderr
    except subprocess.TimeoutExpired:
        return d, -1, "timeout"


def run(out_dir, tool, jobs):
    dirs = sorted(os.path.join(out_dir, x) for x in os.listdir(out_dir) if os.path.isdir(os.path.join(out_dir, x)))
    tool = tool.split()
    results = []
    with ThreadPoolExecutor(max_workers=jobs) as ex:
        for r in ex.map(lambda d: run_one(d, tool), dirs):
            results.append(r)
    reports = {}
    timeouts = 0
    for d, rc, err in results:
        if rc == -1:
            timeouts += 1
            continue
        reports[os.path.basename(d)] = [x for x in report_of(err) if x[0].startswith("error[")]
    metas = {os.path.basename(d): json.load(open(os.path.join(d, "injected.json"))) for d in dirs}
    follow, doubles, multi, all_found, first_only, accepted = [], [], 0, 0, 0, 0
    key = lambda x: (x[0], x[1])
    for name, meta in sorted(metas.items()):
        if name not in reports:
            continue
        diags = reports[name]
        if meta.get("part_of") is not None:
            if len(diags) > 1:
                doubles.append((name, diags))
            continue
        if not diags:
            accepted += 1
            continue
        if not meta["parts"]:
            if len(diags) > 1:
                doubles.append((name, diags))
            continue
        multi += 1
        union = set()
        for p in meta["parts"]:
            union |= {key(x) for x in reports.get(p, [])}
        got = {key(x) for x in diags}
        for x in diags:
            if key(x) not in union:
                follow.append((name, x[0], x[1]))
        if union <= got:
            all_found += 1
        elif len(got) <= 1:
            first_only += 1
    singles = sum(1 for m in metas.values() if not m["parts"] and m.get("part_of") is None)
    print("mutants: %d with one injection, %d with several (and their %d parts); no error at all: %d; timeouts: %d"
          % (singles, multi, sum(len(m["parts"]) for m in metas.values()), accepted, timeouts))
    print("several injections, every part's errors reported: %d of %d; one or none: %d" % (all_found, multi, first_only))
    print("follow-ons (in the whole, in no part): %d" % len(follow))
    for name, first, where in follow:
        print("  %s %s: %s" % (name, "%s:%s" % where if where else "(no location)", first))
    print("one injection, several errors (to read): %d" % len(doubles))
    for name, diags in doubles:
        print("  %s: %s" % (name, " | ".join("%s %s" % ("%s:%s" % w if w else "-", f[:90]) for f, w in diags)))
    return 1 if follow else 0


def main(argv):
    if len(argv) < 3 or argv[1] not in ("make", "run"):
        print(__doc__)
        return 2
    args = argv[2:]
    opts = {"--per": "30", "--seed": "189", "--tool": "impl/target/release/cobc --check", "--jobs": "3"}
    rest = []
    i = 0
    while i < len(args):
        if args[i] in opts:
            opts[args[i]] = args[i + 1]
            i += 2
        else:
            rest.append(args[i])
            i += 1
    if argv[1] == "make":
        make(rest[0], rest[1:], int(opts["--per"]), int(opts["--seed"]))
        return 0
    tool = opts["--tool"]
    if not os.path.isabs(tool.split()[0]):
        tool = os.path.abspath(tool.split()[0]) + " " + " ".join(tool.split()[1:])
    return run(rest[0], tool, int(opts["--jobs"]))


if __name__ == "__main__":
    sys.exit(main(sys.argv))
