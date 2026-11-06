#!/usr/bin/env python3
"""The assurance case kept honest (D-0299): `spec/ASSURANCE.md` names files, theorems, functions, tool options,
commands and records as its evidence; this check fails when one of them no longer exists, so that the page
cannot drift from the tree it describes.
    python3 impl/tools/assurance.py --check    # exits 1 naming each stale reference
    python3 impl/tools/assurance.py            # lists what it checked
What is checked, from the page's backticked text:
  - a path (it has a `/`, or ends in .md/.py/.sh/.lean/.rs/.h) exists, relative to the repository or to spec/;
  - a `D-nnnn` or `CHG-nnnn` has its record file;
  - an option (`--word`) or a `COBALTC_*` variable named inside a command appears in the tools' sources;
  - `cobmodel SUB` names a subcommand of impl/model/src/main.rs;
  - a bare identifier (`reachable_invariants`, `verify_state`, `cb_*`) is declared or used somewhere in
    proofs/ or the implementations' sources.
Standard library only; runs on Windows.
"""
import glob
import os
import re
import sys
ROOT = os.path.dirname(os.path.dirname(os.path.dirname(os.path.abspath(__file__))))
PAGE = os.path.join(ROOT, "spec", "ASSURANCE.md")
SOURCE_DIRS = [os.path.join(ROOT, "proofs", "Cobalt"), os.path.join(ROOT, "proofs"), os.path.join(ROOT, "impl", "src"),
               os.path.join(ROOT, "impl", "cobc", "src"), os.path.join(ROOT, "impl", "cbrt", "src"), os.path.join(ROOT, "impl", "cbrt", "include"),
               os.path.join(ROOT, "impl", "model", "src"), os.path.join(ROOT, "impl", "tools"), os.path.join(ROOT, "impl", "selfhost")]
SOURCE_EXTS = (".lean", ".rs", ".h", ".py", ".sh", ".cb")
# Words that are prose, not names.
PROSE = {"sorry", "decided", "pathless", "unsafe", "light", "std", "main", "proven", "clash", "left", "unknown", "fn", "ref"}


def sources():
    text = []
    for d in SOURCE_DIRS:
        for dp, dns, fns in os.walk(d):
            dns[:] = [x for x in dns if x not in (".lake", "target", "__pycache__")]
            for f in fns:
                if f.endswith(SOURCE_EXTS):
                    try:
                        with open(os.path.join(dp, f), encoding="utf-8", errors="replace") as h:
                            text.append(h.read())
                    except OSError:
                        pass
    return "\n".join(text)


def main():
    check = "--check" in sys.argv
    with open(PAGE, encoding="utf-8") as f:
        page = f.read()
    src = sources()
    with open(os.path.join(ROOT, "impl", "model", "src", "main.rs"), encoding="utf-8") as f:
        model_main = f.read()
    bad = []
    seen = []
    tokens = re.findall(r"`([^`\n]+)`", page)
    for tok in tokens:
        t = tok.strip().rstrip(":,;")
        words = t.split()
        # Paths.
        if len(words) == 1 and ("/" in t or t.endswith((".md", ".py", ".sh", ".lean", ".rs", ".h"))) and not t.startswith("-"):
            p = t.rstrip("/")
            candidates = [os.path.join(ROOT, p), os.path.join(ROOT, "spec", p)]
            if not any(os.path.exists(c) for c in candidates):
                bad.append(f"no such file: `{t}`")
            else:
                seen.append(("path", t))
            continue
        # Records.
        m = re.fullmatch(r"(D|CHG)-(\d{4})", t)
        if m:
            d = "decisions" if m.group(1) == "D" else "changes"
            if not glob.glob(os.path.join(ROOT, "spec", d, f"{t}-*.md")):
                bad.append(f"no record file for {t}")
            else:
                seen.append(("record", t))
            continue
        # Commands and options: each `--word` and each COBALTC_* name in them.
        if t.startswith("-") or len(words) > 1 or t.startswith("COBALTC_"):
            for w in words:
                w = w.rstrip(",;")
                if w.startswith("--"):
                    opt = w.split("=")[0]
                    if f'"{opt}"' not in src and opt not in src:
                        bad.append(f"no tool knows the option {opt} (in `{t}`)")
                    else:
                        seen.append(("option", opt))
                elif w.startswith("COBALTC_"):
                    name = w.split("=")[0]
                    if name not in src:
                        bad.append(f"no tool reads {name} (in `{t}`)")
                    else:
                        seen.append(("variable", name))
            if words and words[0] == "cobmodel" and len(words) > 1 and not words[1].startswith("-"):
                if f'"{words[1]}"' not in model_main:
                    bad.append(f"cobmodel has no subcommand `{words[1]}` (in `{t}`)")
                else:
                    seen.append(("subcommand", f"cobmodel {words[1]}"))
            continue
        # Bare identifiers: theorems, functions, modules.
        if re.fullmatch(r"[a-z][a-z0-9_]*", t) and t not in PROSE and "_" in t:
            if not re.search(r"\b" + re.escape(t) + r"\b", src):
                bad.append(f"no declaration or use of `{t}` in proofs/ or the implementations")
            else:
                seen.append(("name", t))
            continue
        if re.fullmatch(r"cb_[a-z0-9_]*\*?", t):
            if "cb_" not in src:
                bad.append(f"`{t}`: the runtime's prefix is gone")
            continue
    # Every spec chapter the page names by number exists.
    for ch in re.findall(r"`(\d\d)`", page):
        if not glob.glob(os.path.join(ROOT, "spec", f"{ch}-*.md")):
            bad.append(f"no spec chapter {ch}")
    if not check:
        kinds = {}
        for k, v in seen:
            kinds.setdefault(k, set()).add(v)
        for k in sorted(kinds):
            print(f"{k}: {len(kinds[k])} checked")
    for b in bad:
        print(f"assurance: {b}")
    if bad:
        return 1
    if check:
        print("assurance: every file, record, option, variable and name spec/ASSURANCE.md names exists")
    return 0


if __name__ == "__main__":
    sys.exit(main())
