#!/usr/bin/env python3
"""`cobc --doc` matches cobdoc.py (CHG-0446, CHG-0451): it reads `std`'s
documentation as impl/tools/cobdoc.py does (the guide's std reference is
built from cobdoc.py): every item, in the same order, with the same module,
signature, documentation, number of requirements and examples' code; then
the built-in functions of impl/std/builtins.md and impl/std/intrinsics.md.
The records are compared through what `cobc --doc` prints:

- `cobc --doc` lists the modules of cobdoc's items (other than the
  language's), and how many each exports;
- `cobc --doc MODULE` lists that module's items (but the removed ones) in
  cobdoc's order, by their signatures, and `cobc --doc language` the
  language's own functions;
- `cobc --doc ITEM` shows each item's record as the tools lay it out:
  signature, module, retirement, requirements (as many as cobdoc reads),
  documentation, examples (their names and code).

(An item's kind is not printed apart from its signature, so it is compared
only as the signature shows it.)

And, with no `--doc` run (CHG-0445): every name the hints table offers for
`T::name` (impl/front/hints.cb's `fn_name`, the table the front end keeps)
is a function of some `std` type. What `--doc` answers for an item, a type
and a misspelling is cases/tools/doc_item_type_misspelling.case.

    python3 impl/tests/checks/doc_matches_cobdoc.py [--cobc PATH] [--repo DIR]

Exit status 0 when everything agrees, 1 at the first difference (named).
Python 3 standard library only.
"""
import os
import re
import subprocess
import sys

LANGUAGE = "the language"


def indent(text, by):
    return "\n".join(by + l if l else "" for l in text.split("\n"))


def doc(cobc, *args):
    p = subprocess.run([cobc, "--doc"] + list(args), stdout=subprocess.PIPE, stderr=subprocess.PIPE, timeout=120)
    return p.returncode, p.stdout.decode("utf-8"), p.stderr.decode("utf-8")


def listed_signatures(text):
    """The signatures a listing names: its lines indented by two spaces."""
    return [l[2:] for l in text.split("\n")[1:] if l.startswith("  ") and not l.startswith("   ")]


def record_holds(out, x):
    """Whether `out` holds `x`'s record as the tools lay it out: None, else why not."""
    head = "%s\n    in `%s`" % (x["signature"], x["module"])
    retired = x.get("retired")
    if retired:
        head += "\n    %s: %s" % (retired["kind"], retired.get("message") or "")
    tail = ""
    if x.get("doc"):
        tail += "\n\n" + indent(x["doc"], "    ")
    for e in x.get("examples", []):
        if e.get("name"):
            tail += '\n\n    Example, "%s":\n' % e["name"]
        else:
            tail += "\n\n    Example:\n"
        tail += indent(e["code"], "        ")
    tail += "\n"
    n = len(x.get("requires", []))
    why = "no record beginning %r" % head
    k = out.find(head)
    while k >= 0:
        rest = out[k + len(head):]
        got = 0
        if rest.startswith("\n    requires"):
            rest = rest[len("\n    requires"):]
            while rest.startswith("\n        "):
                got += 1
                nl = rest.find("\n", 1)
                rest = rest[nl:] if nl >= 0 else ""
        if got != n:
            why = "%d requirements; cobdoc.py reads %d" % (got, n)
        elif not rest.startswith(tail):
            why = "its documentation or examples differ; after the head cobc --doc gives:\n%s\n--- cobdoc.py:\n%s" % (rest[:300], tail[:300])
        else:
            return None
        k = out.find(head, k + 1)
    return why


def hints_targets(repo):
    """The names `fn_name` of hints.cb offers, for every word it knows."""
    with open(os.path.join(repo, "impl", "front", "hints.cb"), encoding="utf-8") as f:
        src = f.read()
    start = src.index("export fn fn_name(")
    body = src[start:src.index("\n}\n", start)]
    out = {}
    words = []
    for line in body.split("\n"):
        if "name ==" in line:
            words = re.findall(r'name == "([^"]+)"', line)
        for target in re.findall(r'Vec::push\(&mut out, "([^"]+)"\)', line):
            for w in words:
                out.setdefault(w, []).append(target)
    return out


def main():
    args = sys.argv[1:]
    repo = os.path.normpath(os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", "..", ".."))
    if "--repo" in args:
        repo = args[args.index("--repo") + 1]
    cobc = os.path.join(repo, "impl", "target", "release", "cobc")
    if "--cobc" in args:
        cobc = args[args.index("--cobc") + 1]
    sys.path.insert(0, os.path.join(repo, "impl", "tools"))
    import cobdoc

    items = []
    cobdoc.items_of(cobdoc.STD, "std", items, set())
    items += cobdoc.builtins()
    items += cobdoc.intrinsics()

    def fail(msg):
        print("doc_matches_cobdoc: " + msg)
        return 1

    # The modules and their counts.
    counts = {}
    for x in items:
        if x["module"] != LANGUAGE:
            counts[x["module"]] = counts.get(x["module"], 0) + 1
    rc, out, err = doc(cobc)
    if rc != 0:
        return fail("cobc --doc: status %d: %s" % (rc, err))
    want = ["  %s (%d)" % (m, counts[m]) for m in sorted(counts)]
    got = [l for l in out.split("\n")[1:] if l.startswith("  ")]
    if want != got:
        return fail("cobc --doc lists other modules or counts:\n%s\n--- cobdoc.py:\n%s" % ("\n".join(got), "\n".join(want)))
    # Each module's items, in order.
    for m in sorted(counts) + [LANGUAGE]:
        rc, out, err = doc(cobc, "language" if m == LANGUAGE else m)
        if rc != 0:
            return fail("cobc --doc %s: status %d: %s" % (m, rc, err))
        want = [x["signature"] for x in items if x["module"] == m and (x.get("retired") or {}).get("kind") != "removed"]
        got = listed_signatures(out)
        if want != got:
            for k, (w, g) in enumerate(zip(want, got)):
                if w != g:
                    return fail("module %s, item %d: cobdoc.py has %r, cobc --doc %r" % (m, k, w, g))
            return fail("module %s: cobdoc.py has %d items, cobc --doc %d" % (m, len(want), len(got)))
    # Each item's record.
    for k, x in enumerate(items):
        name = x["item"] if x["module"] == LANGUAGE else "%s::%s" % (x["module"], x["item"])
        rc, out, err = doc(cobc, name)
        if rc != 0:
            return fail("item %d, cobc --doc %s: status %d: %s" % (k, name, rc, err))
        why = record_holds(out, x)
        if why:
            return fail("item %d (%s) differs: %s" % (k, name, why))
    # The hints table's names.
    names = set(x["item"].rsplit("::", 1)[-1] for x in items)
    targets = hints_targets(repo)
    if len(targets) < 12:
        return fail("read only %d words from hints.cb's fn_name" % len(targets))
    for word, ts in sorted(targets.items()):
        for t in ts:
            if t not in names:
                return fail("`%s` (for `%s`) is no std function's name" % (t, word))
    print("doc_matches_cobdoc: %d items agree; %d hint words name std functions" % (len(items), len(targets)))
    return 0


if __name__ == "__main__":
    sys.exit(main())
