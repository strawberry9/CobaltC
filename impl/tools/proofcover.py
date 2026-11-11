#!/usr/bin/env python3
"""The mechanised fragment's coverage of spec/23 (D-0270): every rule label a row of
spec/23-preservation.md names is either a step of proofs/Cobalt/Step.lean (its docstring cites the
label) or listed here as outside the fragment, with the reason; and every label the Lean sources cite
is a label the specification defines. The table is written to proofs/COVERAGE.md, with a second one: the
axioms each result of proofs/Cobalt/Statements.lean rests on, read from that file's `#guard_msgs` blocks (the
build checks each block against `#print axioms`; this tool checks that only Lean's own three are listed).
    python3 impl/tools/proofcover.py            # writes the table
    python3 impl/tools/proofcover.py --check    # exits 1 if the table is stale, a spec/23 label is
                                                # neither a step nor listed outside, or a Lean citation
                                                # names no label of the specification, or a result of
                                                # Statements.lean rests on an axiom other than Lean's three
Standard library only; runs on Windows.
"""
import os
import re
import sys
ROOT = os.path.dirname(os.path.dirname(os.path.dirname(os.path.abspath(__file__))))
SPEC = os.path.join(ROOT, "spec")
PROOFS = os.path.join(ROOT, "proofs")
STEP = os.path.join(PROOFS, "Cobalt", "Step.lean")
OUT = os.path.join(PROOFS, "COVERAGE.md")
STATEMENTS = os.path.join(PROOFS, "Cobalt", "Statements.lean")
# The axioms of Lean's own logic; a result resting on any other (an `axiom` the development declared, or
# `sorryAx`, an unfinished proof) fails the check.
STANDARD_AXIOMS = ("propext", "Classical.choice", "Quot.sound")
GUARD = re.compile(r"/-- info: '([\w.']+)' (depends on axioms: \[([^\]]*)\]|does not depend on any axioms) -/\s*\n#guard_msgs in #print axioms ([\w.']+)")
LABEL = re.compile(r"`\[([A-Za-z][A-Za-z0-9-]*)\]`")
# Labels spec/23 argues over that the fragment has no step for, and why (proofs/README.md, "Outside it").
OUTSIDE = {
    "Allocate": "cells: the fragment has no addresses",
    "Deallocate": "cells: the fragment has no addresses",
    "Arg-Bytes": "a value construction moving references between slots (the fragment's `write` stands for it)",
    "Array-Construct": "a value construction moving references between slots",
    "Enum-Construct": "a value construction moving references between slots",
    "Struct-Construct": "a value construction moving references between slots",
    "Store-Sub-Value": "a store moving references between slots",
    "Store-Result-Place-Release": "a store moving references between slots",
    "Rawptr-Write": "a store over raw storage moving references between slots",
    "Copy-Raw": "a trusted copy over raw storage (spec/21 §0's claims)",
    "Str-Ptr": "a trusted pointer to text (spec/21 §0's claims)",
    "Run-Destructor": "the destructor's body: the fragment's other steps, run by `[Destroy]`",
    "Program": "`Σ_0`: `Reachable.init`, a state satisfying the invariants",
    "Extern-Call": "foreign calls are outside the fragment",
    "Thread-Step": "the interleaving itself: `Reachable.step` takes any thread's step",
}


def spec23_labels():
    path = os.path.join(SPEC, "23-preservation.md")
    labs = set()
    with open(path, encoding="utf-8") as f:
        for line in f:
            if line.startswith("| `["):
                labs.update(LABEL.findall(line.split(" | ")[0]))
    return labs


def spec_labels():
    """Every label the specification defines or cites in its normative files (spec/*.md)."""
    labs = set()
    for name in os.listdir(SPEC):
        if name.endswith(".md"):
            with open(os.path.join(SPEC, name), encoding="utf-8") as f:
                labs.update(LABEL.findall(f.read()))
    return labs


def lean_steps():
    """Each `def` of Step.lean with the labels its docstring cites: {label: [def names]}; and every label cited
    anywhere in the Lean sources."""
    with open(STEP, encoding="utf-8") as f:
        text = f.read()
    by_label = {}
    for m in re.finditer(r"/--(.*?)-/\s*\n\s*(?:def\s+|inductive\s+|\|\s*)([A-Za-z0-9_']+)", text, re.S):
        doc, name = m.group(1), m.group(2)
        for lab in set(LABEL.findall(doc)):
            by_label.setdefault(lab, []).append(name)
    cited = set()
    for name in sorted(os.listdir(os.path.join(PROOFS, "Cobalt"))):
        if name.endswith(".lean"):
            with open(os.path.join(PROOFS, "Cobalt", name), encoding="utf-8") as f:
                cited.update(LABEL.findall(f.read()))
    return by_label, cited


def axioms():
    """[(theorem, [axioms])] from Statements.lean's `#guard_msgs` blocks, in file order; and the names of
    `#print axioms` lines no `#guard_msgs` checks."""
    with open(STATEMENTS, encoding="utf-8") as f:
        text = f.read()
    out = []
    for m in GUARD.finditer(text):
        full, axs, short = m.group(1), m.group(3), m.group(4)
        out.append((full, short, [a.strip() for a in axs.split(",")] if axs else []))
    bare = [n for n in re.findall(r"^#print axioms ([\w.']+)", text, re.M)]
    return out, bare


def table():
    labs = spec23_labels()
    by_label, cited = lean_steps()
    rows = ["# The fragment's coverage of spec/23 (D-0270)", "",
            "Generated by `impl/tools/proofcover.py` from `spec/23-preservation.md`'s rows and the docstrings of",
            "`proofs/Cobalt/Step.lean`; `--check` fails when a row's rule has neither a step nor a listed reason to be",
            "outside the fragment. A rule's step is the Lean definition whose docstring cites it; its premises and",
            "effects are the fragment's reading of the rule.", "",
            "| spec/23 rule | Lean step | |", "|---|---|---|"]
    n_in = 0
    for lab in sorted(labs):
        if lab in by_label:
            rows.append("| `[%s]` | %s | step |" % (lab, ", ".join("`%s`" % d for d in sorted(by_label[lab]))))
            n_in += 1
        elif lab in OUTSIDE:
            rows.append("| `[%s]` | — | outside: %s |" % (lab, OUTSIDE[lab]))
        else:
            rows.append("| `[%s]` | — | **UNCOVERED** |" % lab)
    rows += ["", "%d of %d rules are steps; the rest are outside the fragment for the reasons given." % (n_in, len(labs)), ""]
    rows += ["## The axioms each result rests on", "",
             "From `proofs/Cobalt/Statements.lean`, which restates each result with its full statement and checks",
             "`#print axioms` for it under `#guard_msgs`, so `lake build` fails if a result comes to rest on another",
             "axiom (`sorryAx`, an unfinished proof, included). `--check` fails on any axiom but Lean's own three",
             "(`propext`, `Classical.choice`, `Quot.sound`).", "",
             "| result | axioms |", "|---|---|"]
    for full, _, axs in axioms()[0]:
        rows.append("| `%s` | %s |" % (full, ", ".join("`%s`" % a for a in axs) if axs else "none"))
    rows.append("")
    return "\n".join(rows), labs, by_label, cited


def main():
    text, labs, by_label, cited = table()
    errors = []
    for lab in sorted(labs):
        if lab not in by_label and lab not in OUTSIDE:
            errors.append("spec/23 names `[%s]`, which is neither a step of Step.lean nor listed as outside the fragment" % lab)
    for lab in sorted(OUTSIDE):
        if lab in by_label:
            errors.append("`[%s]` is listed as outside the fragment but Step.lean has a step for it" % lab)
    known = spec_labels()
    for lab in sorted(cited):
        if lab not in known:
            errors.append("the Lean sources cite `[%s]`, which the specification does not define" % lab)
    results, bare = axioms()
    if not results:
        errors.append("proofs/Cobalt/Statements.lean checks no result's axioms")
    for full, short, axs in results:
        if full.split(".")[-1] != short.split(".")[-1]:
            errors.append("a `#guard_msgs` block of Statements.lean expects `%s` but prints `%s`" % (full, short))
        for a in axs:
            if a not in STANDARD_AXIOMS:
                errors.append("`%s` rests on the axiom `%s`, which is not one of Lean's own" % (full, a))
    for n in bare:
        errors.append("Statements.lean prints the axioms of `%s` with no `#guard_msgs` to check them" % n)
    check = "--check" in sys.argv
    if check:
        current = open(OUT, encoding="utf-8").read() if os.path.exists(OUT) else ""
        if current != text:
            errors.append("proofs/COVERAGE.md is stale: run impl/tools/proofcover.py")
    else:
        with open(OUT, "w", encoding="utf-8", newline="\n") as f:
            f.write(text)
    for e in errors:
        print("proofcover: error: " + e)
    n_in = sum(1 for lab in labs if lab in by_label)
    print("proofcover: %d spec/23 rules, %d steps, %d outside; %d results, %s; %d errors%s" % (len(labs), n_in, len(labs) - n_in, len(results), "standard axioms only" if not any(a not in STANDARD_AXIOMS for _, _, axs in results for a in axs) else "AN AXIOM BEYOND LEAN'S OWN", len(errors), "" if check else "; wrote proofs/COVERAGE.md"))
    sys.exit(1 if errors else 0)


if __name__ == "__main__":
    main()
