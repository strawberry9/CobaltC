#!/usr/bin/env python3
"""The documentation of a CobaltC program's or library's exported items, as JSON.

    python3 impl/tools/cobdoc.py                    # std: impl/std/std.cb
    python3 impl/tools/cobdoc.py prog.cb            # a program and its module files
    python3 impl/tools/cobdoc.py --missing          # the exported items with no documentation
    python3 impl/tools/cobdoc.py --check            # exit 1 if any exported item has no summary

D-0229: a run of `///` line comments immediately before an item (no blank
line between) is that item's documentation, in Markdown; its first
sentence is the summary. Nothing else about the language changes: `///`
is a line comment like any other, and `cobfmt` keeps it where it is.

From the root file, the module tree is followed through each `module NAME
"FILE";` (the path relative to the declaring file). Every item declared
`export` is listed: functions (associated ones as `T::name`), structs,
enums, bitstructs and constants, each with its module, kind, signature
(its declaration up to its body), documentation (or null), file and line,
and its examples: each `///`-documented `unittest` block right after it
(D-0230), with its name, documentation and code.
The items that both tools build in, which have no CobaltC body (`printf`,
`sqrt`, `exit`, …), are documented by hand in `impl/std/builtins.md`, one
entry per item:

    ## `printf` in `std::io`
    Signature: `fn printf(str f, a1, …, an)`

    The documentation, in Markdown, as a `///` block would carry it.

They are listed with kind `built in fn`. `--check` also fails when a
built-in item the tools know (`impl/src/modres.rs`'s `native(…)`
registrations and `typecheck.rs`'s float-function lists) has no entry,
or an entry names no such item.

Python 3 standard library only.
"""

import argparse
import json
import os
import re
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
STD = os.path.join(os.path.dirname(HERE), "std", "std.cb")
BUILTINS = os.path.join(os.path.dirname(HERE), "std", "builtins.md")
MODRES = os.path.join(os.path.dirname(HERE), "src", "modres.rs")
TYPECHECK = os.path.join(os.path.dirname(HERE), "src", "typecheck.rs")

FN = re.compile(r'^export\s+(unsafe\s+)?(extern\s+)?fn\s+([A-Za-z_]\w*(?:::[A-Za-z_]\w*)?)')
TY = re.compile(r'^export\s+(resource\s+|quiet\s+)*(struct|enum|bitstruct)\s+([A-Za-z_]\w*)')
CO = re.compile(r'^export\s+const\s+.*?([A-Za-z_]\w*)\s*=')
MOD = re.compile(r'^(?:export\s+)?module\s+(\w+)\s+"([^"]+)"\s*;')
# Documented tests that follow no item of their file (--check reports them).
ORPHANS = []

TEST = re.compile(r'^(\s*)unittest(?:\s+"((?:[^"\\]|\\.)*)")?\s*(\{)?\s*$')


def summary(doc):
    """The first sentence of a documentation text: up to the first `. `
    outside backticks (or the end of the first paragraph)."""
    para = doc.split("\n\n", 1)[0].replace("\n", " ").strip()
    # A `.` inside backticks (`&s[0 .. $]`) ends no sentence.
    code = False
    for i, ch in enumerate(para):
        if ch == "`":
            code = not code
        elif ch == "." and not code and (i + 1 == len(para) or para[i + 1].isspace()):
            return para[:i + 1]
    return para


def signature(lines, i):
    """An item's declaration from line `i`, up to its body or its `;`."""
    out = []
    for line in lines[i:i + 40]:
        cut = len(line)
        for stop in ("{", ";"):
            k = line.find(stop)
            if k >= 0:
                cut = min(cut, k + (1 if stop == ";" else 0))
        out.append(line[:cut].strip())
        if cut < len(line) or line.rstrip().endswith((";", "{")):
            break
    sig = " ".join(p for p in out if p)
    sig = re.sub(r'\(\s+', '(', sig)
    sig = re.sub(r'\s+\)', ')', sig)
    sig = re.sub(r',\s*\)', ')', sig)
    return re.sub(r'\s+', ' ', sig).rstrip(";").strip()


def rel(path, module):
    return os.path.relpath(path, os.path.dirname(STD) if module.startswith("std") else os.getcwd())


def items_of(path, module, out, seen):
    if path in seen:
        return
    seen.add(path)
    with open(path, encoding="utf-8") as f:
        lines = f.read().split("\n")
    doc = []
    for i, line in enumerate(lines):
        s = line.strip()
        if line.startswith("///"):
            text = line[3:]
            doc.append(text[1:] if text.startswith(" ") else text)
            continue
        m = MOD.match(line)
        if m:
            sub = os.path.join(os.path.dirname(path), m.group(2))
            items_of(sub, module + "::" + m.group(1), out, seen)
            doc = []
            continue
        # D-0230: a documented test right after an item is that item's
        # example; an undocumented one is not shown.
        m = TEST.match(line)
        if m:
            if doc and out and out[-1]["file"] == rel(path, module):
                indent = m.group(1)
                body = []
                for later in lines[i + 1:]:
                    if later.strip() == "}" and later[:len(later) - len(later.lstrip())] == indent:
                        break
                    body.append(later)
                if body and body[0].strip() == "{":
                    body = body[1:]
                width = min((len(b) - len(b.lstrip()) for b in body if b.strip()), default=0)
                out[-1]["examples"].append({
                    "name": m.group(2),
                    "doc": "\n".join(doc).strip(),
                    "code": "\n".join(b[width:] for b in body).strip("\n"),
                    "line": i + 1,
                })
            elif doc:
                ORPHANS.append((rel(path, module), i + 1))
            doc = []
            continue
        kind = name = None
        m = FN.match(line)
        if m:
            kind, name = ("extern fn" if m.group(2) else "fn"), m.group(3)
        else:
            m = TY.match(line)
            if m:
                kind, name = m.group(2), m.group(3)
            else:
                m = CO.match(line)
                if m:
                    kind, name = "const", m.group(1)
        if kind:
            text = "\n".join(doc).strip() if doc else None
            out.append({
                "module": module,
                "item": name,
                "kind": kind,
                "signature": signature(lines, i),
                "doc": text,
                "summary": summary(text) if text else None,
                "file": rel(path, module),
                "line": i + 1,
                "examples": [],
            })
        # Only `///` lines immediately before an item are its documentation.
        doc = []


def natives():
    """The exported items both tools build in, as (module, item), read from
    the tools' own registrations so the list cannot fall behind them."""
    out = []
    with open(MODRES, encoding="utf-8") as f:
        for mod, name, exp in re.findall(r'native\("([\w:]+)", "([\w:]+)", (true|false)\)', f.read()):
            if exp == "true":
                out.append((mod, name))
    with open(TYPECHECK, encoding="utf-8") as f:
        tc = f.read()
    for arr in ("FLOAT_FNS", "FLOAT2_FNS"):
        m = re.search(r'pub const %s: &\[&str\] = &\[(.*?)\];' % arr, tc, re.S)
        for name in re.findall(r'"std::(\w+)"', m.group(1) if m else ""):
            out.append(("std::math", name))
    return out


HEAD = re.compile(r'^## `([^`]+)` in `([\w:]+)`\s*$')
SIG = re.compile(r'^Signature:\s*(`.+`)\s*$')   # one or more `…` spans (an overloaded item has two)


def builtins(path=BUILTINS):
    """The entries of `impl/std/builtins.md`, in the shape `items_of` gives."""
    out = []
    if not os.path.isfile(path):
        return out
    with open(path, encoding="utf-8") as f:
        lines = f.read().split("\n")
    i = 0
    while i < len(lines):
        m = HEAD.match(lines[i])
        if not m:
            i += 1
            continue
        item, module, start = m.group(1), m.group(2), i + 1
        sig, body = "", []
        i += 1
        while i < len(lines) and not HEAD.match(lines[i]):
            s = SIG.match(lines[i])
            if s and not sig:
                sig = s.group(1).replace("`", "").strip()
            else:
                body.append(lines[i])
            i += 1
        text = "\n".join(body).strip() or None
        out.append({
            "module": module,
            "item": item,
            "kind": "built in fn",
            "signature": sig,
            "doc": text,
            "summary": summary(text) if text else None,
            "file": os.path.basename(path),
            "line": start,
            "examples": [],
        })
    return out


def main():
    ap = argparse.ArgumentParser(description="List a CobaltC program's or library's exported items and their documentation as JSON.")
    ap.add_argument("root", nargs="?", default=STD, help="the root file (default: std's, impl/std/std.cb)")
    ap.add_argument("--module", help="the root module's name (default: std for std, else the file's stem)")
    ap.add_argument("--missing", action="store_true", help="print the exported items with no documentation, and the counts")
    ap.add_argument("--check", action="store_true", help="exit 1 if any exported item has no summary (battery stage 0, WI-18)")
    a = ap.parse_args()
    root = os.path.abspath(a.root)
    module = a.module or ("std" if root == os.path.abspath(STD) else os.path.splitext(os.path.basename(root))[0])
    out = []
    items_of(root, module, out, set())
    is_std = root == os.path.abspath(STD)
    builtin_errors = []
    if is_std:
        docs = builtins()
        known = natives()
        have = {(x["module"], x["item"]) for x in docs}
        for mod, name in known:
            if (mod, name) not in have:
                builtin_errors.append("%s: built-in item `%s::%s` (known to the tools) has no entry" % (os.path.basename(BUILTINS), mod, name))
        for x in docs:
            if (x["module"], x["item"]) not in set(known):
                builtin_errors.append("%s:%d: `%s::%s` names no item the tools build in" % (x["file"], x["line"], x["module"], x["item"]))
            if not x["signature"]:
                builtin_errors.append("%s:%d: `%s::%s` has no `Signature:` line" % (x["file"], x["line"], x["module"], x["item"]))
        out += docs
    if a.missing:
        missing = [x for x in out if not x["doc"]]
        for x in missing:
            print("%s::%s  (%s:%d)" % (x["module"], x["item"], x["file"], x["line"]))
        print("%d exported items, %d documented, %d without documentation" % (len(out), len(out) - len(missing), len(missing)))
        return 0
    if a.check:
        # WI-18: every exported item has a summary, the first sentence the
        # std reference shows beside it.
        bad = [x for x in out if not x["summary"]]
        for x in bad:
            print("%s:%d: `%s::%s` has no `///` summary" % (x["file"], x["line"], x["module"], x["item"]), file=sys.stderr)
        for f, n in ORPHANS:
            print("%s:%d: a documented `unittest` that follows no exported item of its file" % (f, n), file=sys.stderr)
        for e in builtin_errors:
            print(e, file=sys.stderr)
        print("cobdoc: %d exported items (%d built in), %d without a summary, %d examples"
              % (len(out), sum(1 for x in out if x["kind"] == "built in fn"), len(bad), sum(len(x["examples"]) for x in out)))
        return 1 if bad or ORPHANS or builtin_errors else 0
    json.dump(out, sys.stdout, indent=1, ensure_ascii=False)
    sys.stdout.write("\n")
    return 0


if __name__ == "__main__":
    sys.exit(main())
