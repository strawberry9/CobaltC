#!/usr/bin/env python3
"""A machine check of the normative corpus (D-0207).

Reads spec/ and reports, with file and line, what a reader would otherwise
have to find by hand:

  - every entity heading (`## `inv.x``, `### `rule.x``, `term.`, `type.`,
    `state.`, `ex.`) has a **Status:** from the closed set, and its
    artifact's Version is the version of its first Change Log entry;
  - every **Depends on:** target (`term.*`, `inv.*`, `type.*`, `state.*`,
    `rule.*`, `feat.*`, `D-nnnn`, `CHG-nnnn`) resolves;
  - every rule label `[Name]` cited in the artifacts, the conformance cases
    and the registries is defined by some artifact (a label cited only in
    a decision or change record is a warning: that history is append-only);
  - every `diag.*` named anywhere in spec/ is in the diagnostics registry,
    and every registry diagnostic is named by some artifact;
  - `disposition:` and `outcome:` values come from spec/01 §5's closed
    vocabularies, and a `discharge: trusted` side-condition lies under a
    `trusted-unchecked` rule;
  - every `D-nnnn` and `CHG-nnnn` cited has its file, the numbers are
    contiguous, and every change record names its decision;
  - spec/README.md's bold edition is the last row of its edition table;
  - every `conf.*` row's program is an inline fragment, an existing case
    file under impl/conformance/, or an `ex.*` of spec/examples.md;
  - spec/21 §0's trusted table is what impl/tools/trusted_table.py
    generates (`term.trusted-library-base`);
  - spec/23's preservation lemmas (D-0262) cover every rule block that
    writes a component an invariant ranges over: the writers are read from
    the rules' own text (`Σ[ c := … ]`, `c(…) := …`), and an unlisted
    writer is an error, a listed rule that writes none of the invariant's
    components a warning.

What is history is a warning, not an error: a decision or change record,
and an artifact's Change Log, cite labels and ids as they were. The
built-in types (`type.bool`, `type.ref`, `type.i8` … `type.usize`,
`type.unit`) have no entity heading — spec/06 §1 gives them in a table —
and are accepted as references to it. A rule label defined in prose or in
a table cell (`\`[Contains]\` — …`, `\`[Fault]\`: …`) counts as defined.

    python3 impl/tools/speccheck.py            # errors to stderr, exit 1 if any
    python3 impl/tools/speccheck.py --warnings # warnings too

Standard library only; runs on Windows.
"""
import os
import re
import subprocess
import sys

ROOT = os.path.dirname(os.path.dirname(os.path.dirname(os.path.abspath(__file__))))
SPEC = os.path.join(ROOT, "spec")

STATUS_SET = {"ACCEPTED", "REJECTED", "ABSENT"}
DISPOSITIONS = {"rejected", "checked", "fallible", "trusted-unchecked", "unsupported"}
OUTCOMES = {"deterministic", "impl-defined", "unspecified"}
KINDS = ("term", "inv", "type", "state", "rule", "ex", "feat", "diag")

ENTITY_CITE = re.compile(r"`((?:term|inv|type|state|rule|ex|feat)\.[A-Za-z0-9._-]*[A-Za-z0-9])`")
ENTITY = re.compile(r"^#{2,3} `((?:term|inv|type|state|rule|ex|feat)\.[A-Za-z0-9._-]+)`")
LABEL_DEF = re.compile(r"^ {4,}\[([A-Z][A-Za-z0-9]*(?:-[A-Za-z0-9]+)*)\]")
LABEL_DEF_ALL = re.compile(r"\[([A-Z][A-Za-z0-9]*(?:-[A-Za-z0-9]+)*)\]")
LABEL_PROSE_DEF = re.compile(r"`\[([A-Z][A-Za-z0-9]*(?:-[A-Za-z0-9]+)*)\]`\)?\s*(?:—|:|is\b)")
LABEL_CITE = re.compile(r"\[([A-Z][A-Za-z0-9]+(?:-[A-Za-z0-9]+)*)\]")
BUILTIN_TYPES = {"type.%s" % t for t in ("bool", "unit", "void", "ref", "f32", "f64", "isize", "usize", "i8", "i16", "i32", "i64", "i128", "u8", "u16", "u32", "u64", "u128")}
DIAG_PLACEHOLDERS = {"diag.x", "diag.d", "diag.name"}
DIAG = re.compile(r"\bdiag\.[a-z0-9]+(?:-[a-z0-9]+)*")
DEP_TOKEN = re.compile(r"(?:term|inv|type|state|rule|feat)\.[A-Za-z0-9._-]+|D-\d{4}|CHG-\d{4}")
DREF = re.compile(r"\bD-(\d{4})\b")
CHGREF = re.compile(r"\bCHG-(\d{4})\b")
DISP = re.compile(r"disposition:\s*([A-Za-z-]+)")
OUTC = re.compile(r"outcome:\s*([A-Za-z-]+)")
VERSION = re.compile(r"^Version: (\d+\.\d+\.\d+)")
LOGENTRY = re.compile(r"^- (\d+\.\d+\.\d+) ")

errors = []
warnings = []


def err(path, line, msg):
    errors.append("%s:%d: %s" % (os.path.relpath(path, ROOT), line, msg))


def warn(path, line, msg):
    warnings.append("%s:%d: %s" % (os.path.relpath(path, ROOT), line, msg))


def read(path):
    with open(path, encoding="utf-8") as f:
        return f.read().split("\n")


def artifacts():
    return sorted(os.path.join(SPEC, f) for f in os.listdir(SPEC) if re.match(r"^[0-2]\d-.*\.md$", f))


# The components of Σ (spec/04 §1) a rule block may assign.
COMPONENTS = ("items", "objects", "storage", "access-paths", "bindings", "frame-stack", "temp-scope-stack", "holder", "init", "destruction-obligations", "authority", "sync", "threads", "trust")
COMP_UPDATE = re.compile(r"Σ[\'0-9_]*\[(.*?)\]⟩", re.S)


def rule_writers(arts):
    """{label: (path, line, set of components it assigns)} over every rule
    block (`    [Label]` and the indented lines after it) of the artifacts."""
    out = {}
    for path in arts:
        if os.path.basename(path).startswith("23-"):
            continue
        lines = read(path)
        i = 0
        while i < len(lines):
            m = LABEL_DEF.match(lines[i])
            if not m or not lines[i].startswith("    [") :
                i += 1
                continue
            lab, start = m.group(1), i
            i += 1
            while i < len(lines) and (lines[i].startswith("    ") or not lines[i].strip()) and not (lines[i].startswith("    [") and LABEL_DEF.match(lines[i])):
                i += 1
            text = "\n".join(lines[start:i])
            writes = set()
            for seg in COMP_UPDATE.findall(text):
                for c in COMPONENTS:
                    if re.search(r"(?<![A-Za-z-])" + re.escape(c) + r"(?![A-Za-z-])", seg):
                        writes.add(c)
            for c in COMPONENTS:
                if re.search(r"Σ[\'0-9_]*\[\s*" + re.escape(c) + r"\b", text) or re.search(r"\b" + re.escape(c) + r"(\([^)]*\))?(\.[a-z-]+)*\s*:=", text):
                    writes.add(c)
            if writes:
                out.setdefault(lab, (path, start + 1, writes))
    return out


def check_preservation(arts):
    """spec/23: every writer of an invariant's components has a row."""
    path = os.path.join(SPEC, "23-preservation.md")
    if not os.path.exists(path):
        return 0
    writers = rule_writers(arts)
    lines = read(path)
    sections = []  # (inv, line, components, listed labels)
    cur = None
    in_table = False
    for i, line in enumerate(lines, 1):
        m = re.match(r"^## \d+[a-z]?\.\s+`(inv\.[A-Za-z0-9._-]+)`", line)
        if m:
            cur = [m.group(1), i, set(), set()]
            sections.append(cur)
            in_table = False
            continue
        if line.startswith("## "):
            cur = None
            in_table = False
            continue
        if cur is None:
            continue
        if line.startswith("**Components:**"):
            cur[2] = {c.strip().strip("`") for c in line[len("**Components:**"):].split(",") if c.strip()}
            for c in cur[2]:
                if c not in COMPONENTS:
                    err(path, i, "`%s` is not a component of Σ (spec/04 §1)" % c)
        if line.startswith("| Rule | Writes |"):
            in_table = True
            continue
        if in_table:
            if not line.startswith("|"):
                in_table = False
                continue
            if line.startswith("|---"):
                continue
            first = line.split(" | ")[0]
            for lab in LABEL_CITE.findall(first):
                cur[3].add(lab)
                if lab in writers and not (writers[lab][2] & cur[2]):
                    warn(path, i, "`%s` lists `[%s]`, which writes %s, none of its components" % (cur[0], lab, sorted(writers[lab][2])))
                elif lab not in writers:
                    warn(path, i, "`%s` lists `[%s]`, which assigns no component of Σ" % (cur[0], lab))
    lemmas = 0
    for inv, line, comps, listed in sections:
        if not comps:
            continue
        lemmas += len(listed)
        for lab, (wp, wl, writes) in sorted(writers.items()):
            if (writes & comps) and lab not in listed:
                err(path, line, "`%s` ranges over %s but has no row for `[%s]` (%s:%d), which writes %s" % (inv, sorted(comps), lab, os.path.relpath(wp, ROOT), wl, sorted(writes & comps)))
    return lemmas


def main():
    arts = artifacts()
    registry_dir = os.path.join(SPEC, "registry")
    decisions_dir = os.path.join(SPEC, "decisions")
    changes_dir = os.path.join(SPEC, "changes")
    conformance = os.path.join(SPEC, "conformance.md")
    examples = os.path.join(SPEC, "examples.md")
    readme = os.path.join(SPEC, "README.md")

    defined = {}      # entity id -> (path, line)
    labels = {}       # label -> (path, line)
    diag_defined = {}
    diag_cited = {}   # diag -> first citing artifact

    # --- entities, labels, tags in the artifacts ---
    for path in arts + [examples]:
        lines = read(path)
        version = None
        first_log = None
        in_log = False
        pending = None  # (id, line) awaiting its Status
        in_code = False
        current_disp = None
        for i, line in enumerate(lines, 1):
            m = VERSION.match(line)
            if m and version is None:
                version = m.group(1)
            if line.startswith("## Change Log"):
                in_log = True
            elif in_log and first_log is None:
                m = LOGENTRY.match(line)
                if m:
                    first_log = m.group(1)
            m = ENTITY.match(line)
            if m:
                if pending:
                    err(path, pending[1], "entity `%s` has no **Status:** line" % pending[0])
                eid = m.group(1)
                if eid in defined:
                    err(path, i, "entity `%s` defined twice (first at %s:%d)" % (eid, os.path.relpath(defined[eid][0], ROOT), defined[eid][1]))
                defined[eid] = (path, i)
                # an example's status is the artifact's (spec/examples.md: "Every example has Status: ACCEPTED")
                pending = None if eid.startswith("ex.") else (eid, i)
                continue
            if pending and line.startswith("**Status:**"):
                status = line[len("**Status:**"):].strip().split()[0] if line[len("**Status:**"):].strip() else ""
                if status not in STATUS_SET:
                    err(path, i, "entity `%s` has status `%s`, not one of %s" % (pending[0], status, sorted(STATUS_SET)))
                pending = None
            if not in_log:
                for m in LABEL_PROSE_DEF.finditer(line):
                    labels.setdefault(m.group(1), (path, i))
            if line.startswith("    "):
                m = LABEL_DEF.match(line)
                if m:
                    for lab in LABEL_DEF_ALL.findall(line.split("disposition:")[0].split("outcome:")[0]):
                        labels.setdefault(lab, (path, i))
                    current_disp = None
                    d = DISP.search(line)
                    if d:
                        current_disp = d.group(1)
                for d in DISP.finditer(line):
                    if d.group(1) not in DISPOSITIONS:
                        err(path, i, "`disposition: %s` is not in spec/01 §5's vocabulary %s" % (d.group(1), sorted(DISPOSITIONS)))
                    current_disp = d.group(1)
                for o in OUTC.finditer(line):
                    if o.group(1) not in OUTCOMES:
                        err(path, i, "`outcome: %s` is not in spec/01 §5's vocabulary %s" % (o.group(1), sorted(OUTCOMES)))
                if "⟧ discharge: trusted" in line and current_disp != "trusted-unchecked" and not path.endswith("01-metalanguage.md"):
                    err(path, i, "`discharge: trusted` under a rule whose disposition is `%s`, not `trusted-unchecked`" % current_disp)
            else:
                current_disp = None
            for d in DIAG.findall(line):
                diag_cited.setdefault(d, (path, i))
        if pending:
            err(path, pending[1], "entity `%s` has no **Status:** line" % pending[0])
        if version and first_log and version != first_log and path != examples:
            err(path, 1, "Version: %s but the first Change Log entry is %s" % (version, first_log))

    # --- registries ---
    for name in ("diagnostics.md", "features.md"):
        path = os.path.join(registry_dir, name)
        for i, line in enumerate(read(path), 1):
            if line.startswith("- `"):
                head = line.split(" — ")[0]
                for eid in re.findall(r"`((?:diag|feat)\.[a-z0-9-]+)`", head):
                    if eid in defined or eid in diag_defined:
                        err(path, i, "`%s` listed twice" % eid)
                    (diag_defined if eid.startswith("diag.") else defined)[eid] = (path, i)
            m = ENTITY.match(line)
            if m:
                defined[m.group(1)] = (path, i)

    # --- decisions and changes: files, numbering, each CHG names its D ---
    dnums = sorted(int(m.group(1)) for f in os.listdir(decisions_dir) for m in [re.match(r"^D-(\d{4})-", f)] if m)
    cnums = sorted(int(m.group(1)) for f in os.listdir(changes_dir) for m in [re.match(r"^CHG-(\d{4})-", f)] if m)
    for kind, nums, d in (("D", dnums, decisions_dir), ("CHG", cnums, changes_dir)):
        if nums != list(range(1, len(nums) + 1)):
            missing = sorted(set(range(1, max(nums) + 1)) - set(nums))
            err(d, 0, "%s numbers are not contiguous: missing %s" % (kind, ["%s-%04d" % (kind, n) for n in missing][:10]))
    dset = set(dnums)
    cset = set(cnums)
    for f in sorted(os.listdir(changes_dir)):
        path = os.path.join(changes_dir, f)
        text = "\n".join(read(path))
        if not DREF.search(text):
            warn(path, 1, "change record names no decision (D-nnnn)")

    # --- cross references: Depends on, D/CHG citations, labels, diags ---
    overview = os.path.join(SPEC, "OVERVIEW.md")
    # An entity may also head a numbered section (`## 1. `type.ref``).
    numbered_heads = set()
    for path in arts:
        for line in read(path):
            m = re.match(r"^#{2,3} \d+[a-z]?\.\s+`((?:term|inv|type|state|rule|ex|feat)\.[A-Za-z0-9._-]+)`", line)
            if m:
                numbered_heads.add(m.group(1))
    all_files = arts + [examples, conformance, readme] + [os.path.join(registry_dir, n) for n in ("diagnostics.md", "features.md")]
    # WI-19: the non-normative overview cites rules; every label, record,
    # diagnostic and entity it names must exist.
    if os.path.exists(overview):
        all_files.append(overview)
    history = [os.path.join(decisions_dir, f) for f in sorted(os.listdir(decisions_dir))] + [os.path.join(changes_dir, f) for f in sorted(os.listdir(changes_dir))]
    for path in all_files + history:
        lines = read(path)
        is_history = path in history
        i = 0
        in_log = False
        while i < len(lines):
            line = lines[i]
            if line.startswith("## Change Log"):
                in_log = True
            report = warn if (is_history or in_log) else err
            if line.startswith("**Depends on:**"):
                block = line[len("**Depends on:**"):]
                j = i + 1
                while j < len(lines) and lines[j].strip() and not lines[j].startswith("**") and not lines[j].startswith("#"):
                    block += " " + lines[j]
                    j += 1
                for tok in DEP_TOKEN.findall(block):
                    if tok.startswith("D-"):
                        if int(tok[2:]) not in dset:
                            report(path, i + 1, "Depends on `%s`: no such decision" % tok)
                    elif tok.startswith("CHG-"):
                        if int(tok[4:]) not in cset:
                            report(path, i + 1, "Depends on `%s`: no such change record" % tok)
                    elif tok not in defined and tok not in diag_defined and tok not in BUILTIN_TYPES:
                        report(path, i + 1, "Depends on `%s`: no such entity" % tok)
            for m in DREF.finditer(line):
                if int(m.group(1)) not in dset:
                    report(path, i + 1, "cites `D-%s`, which has no file" % m.group(1))
            for m in CHGREF.finditer(line):
                if int(m.group(1)) not in cset:
                    report(path, i + 1, "cites `CHG-%s`, which has no file" % m.group(1))
            for d in DIAG.findall(line):
                if d not in diag_defined and d not in DIAG_PLACEHOLDERS:
                    report(path, i + 1, "names `%s`, which the diagnostics registry does not list" % d)
            if path == overview:
                for m in ENTITY_CITE.finditer(line):
                    if m.group(1) not in defined and m.group(1) not in numbered_heads:
                        report(path, i + 1, "cites `%s`, which no artifact defines" % m.group(1))
            if not line.startswith("    ") or path == conformance or is_history:
                for m in LABEL_CITE.finditer(line):
                    lab = m.group(1)
                    if lab not in labels:
                        report(path, i + 1, "cites rule label `[%s]`, which no artifact defines" % lab)
            i += 1

    for d, where in diag_defined.items():
        if d not in diag_cited:
            err(where[0], where[1], "`%s` is listed but no artifact names it" % d)

    # --- edition ---
    rl = read(readme)
    bold = None
    last_row = None
    for i, line in enumerate(rl, 1):
        m = re.match(r"^\*\*CobaltC specification edition (\d{4}\.\d{3,6})\.\*\*", line)
        if m:
            bold = (m.group(1), i)
        m = re.match(r"^\| (\d{4}\.\d{3,6}) \| Commit Update", line)
        if m:
            last_row = (m.group(1), i)
    if not bold:
        err(readme, 1, "no bold edition line")
    elif not last_row:
        err(readme, 1, "no edition table row")
    elif bold[0] != last_row[0]:
        err(readme, bold[1], "bold edition %s but the table's last row is %s (line %d)" % (bold[0], last_row[0], last_row[1]))

    # --- conformance rows ---
    ex_ids = {m.group(1) for line in read(examples) for m in [re.match(r"^#{2,3} `(ex\.[A-Za-z0-9._-]+)`", line)] if m}
    rows = 0
    for i, line in enumerate(read(conformance), 1):
        if not line.startswith("| `conf."):
            continue
        rows += 1
        cols = line.split(" | ")
        if len(cols) < 3:
            err(conformance, i, "row has fewer than three columns")
            continue
        frag = cols[1].strip()
        for part in frag.split(", "):
            part = part.strip().strip("`")
            if part.startswith("impl/conformance/"):
                if not os.path.isfile(os.path.join(ROOT, part)):
                    err(conformance, i, "names case file `%s`, which does not exist" % part)
            elif re.match(r"^ex\.[A-Za-z0-9._-]+$", part):
                if part not in ex_ids:
                    err(conformance, i, "names `%s`, which spec/examples.md does not define" % part)
    # Case files no row names. A fixture (a file another case loads; its
    # first line is `// CONFORMANCE-FIXTURE`, as std_coverage.py requires)
    # needs no row; any other such file is a warning here and a failure in
    # std_coverage.py (battery stage 1).
    fixtures, unnamed = 0, 0
    conf_text = "\n".join(read(conformance))
    for dirpath, _, files in os.walk(os.path.join(ROOT, "impl", "conformance")):
        for f in sorted(files):
            if f.endswith(".cb"):
                full = os.path.join(dirpath, f)
                rel = os.path.relpath(full, ROOT).replace("\\", "/")
                if rel not in conf_text:
                    if read(full) and read(full)[0].startswith("// CONFORMANCE-FIXTURE"):
                        fixtures += 1
                    else:
                        unnamed += 1
                        warn(rel, 1, "case file named by no conformance row and not marked `// CONFORMANCE-FIXTURE`")

    # --- the trusted table ---
    r = subprocess.run([sys.executable, os.path.join(ROOT, "impl", "tools", "trusted_table.py"), "--check"], capture_output=True, text=True)
    if r.returncode != 0:
        err(os.path.join(SPEC, "21-standard-library-semantics.md"), 1, "trusted table: " + (r.stderr.strip() or r.stdout.strip()))

    # --- the preservation lemmas (spec/23, D-0262) ---
    lemmas = check_preservation(arts)

    for w in (warnings if "--warnings" in sys.argv else []):
        print("warning: " + w)
    for e in errors:
        print(e, file=sys.stderr)
    print("speccheck: %d entities, %d rule labels, %d diagnostics, %d decisions, %d change records, %d conformance rows (%d fixture files, %d other case files named by no row), %d preservation rows; %d errors, %d warnings"
          % (len(defined), len(labels), len(diag_defined), len(dnums), len(cnums), rows, fixtures, unnamed, lemmas, len(errors), len(warnings)))
    return 1 if errors else 0


if __name__ == "__main__":
    sys.exit(main())
