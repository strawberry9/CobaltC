"""The std reference chapter (WI-18): every exported item of `impl/std`,
module by module, with its signature, its `///` documentation and its
examples, read by `impl/tools/cobdoc.py` when the page is built (D-0229).
Nothing here is written by hand, so the reference cannot fall behind
`std`. An example is a documented `unittest` block right after its item
(D-0230): the page shows its body, and `gen.py --check` runs that body as
`fn main()` of a program that imports `std` and the item's module, under
both tools, as it runs every other example."""
import html
import os
import re
import sys

R = os.path.dirname(os.path.dirname(os.path.dirname(os.path.abspath(__file__))))
sys.path.insert(0, os.path.join(R, "impl", "tools"))
import cobdoc  # noqa: E402

from gen import code, highlight, p, Section  # noqa: E402


def anchor(module, item=None):
    """The page id of a module's or an item's entry: `stdref-std-text`,
    `stdref-std-text-StringView-split`."""
    a = "stdref-" + module.replace("::", "-")
    return a + "-" + item.replace("::", "-") if item else a


def items():
    out = []
    cobdoc.items_of(cobdoc.STD, "std", out, set())
    out += cobdoc.builtins()
    return out


def _inline(text):
    """Markdown's inline forms the documentation uses: `code`, **strong**,
    *emphasis*; everything else escaped."""
    parts = text.split("`")
    out = []
    for i, part in enumerate(parts):
        if i % 2:
            out.append("<code>%s</code>" % html.escape(part))
        else:
            t = html.escape(part)
            t = re.sub(r"\*\*(.+?)\*\*", r"<strong>\1</strong>", t)
            t = re.sub(r"(?<![\w*])\*(?!\s)(.+?)(?<!\s)\*(?![\w*])", r"<em>\1</em>", t)
            out.append(t)
    return "".join(out)


def markdown(text):
    """The block forms: paragraphs, `- ` lists (a list item continues on
    the lines indented under it), and fenced or four-space-indented code,
    shown as a fragment."""
    lines = text.split("\n")
    out, i = [], 0
    while i < len(lines):
        line = lines[i]
        if not line.strip():
            i += 1
            continue
        if line.startswith("```"):
            j = i + 1
            while j < len(lines) and not lines[j].startswith("```"):
                j += 1
            out.append(code("\n".join(lines[i + 1:j])))
            i = j + 1
            continue
        if line.startswith("    "):
            j = i
            while j < len(lines) and (lines[j].startswith("    ") or not lines[j].strip()):
                j += 1
            out.append(code("\n".join(lines[i:j]).rstrip("\n")))
            i = j
            continue
        if line.startswith("- "):
            entries = []
            while i < len(lines) and (lines[i].startswith("- ") or (lines[i].startswith("  ") and entries)):
                if lines[i].startswith("- "):
                    entries.append(lines[i][2:].strip())
                else:
                    entries[-1] += " " + lines[i].strip()
                i += 1
            out.append("<ul>\n%s</ul>\n" % "".join("<li>%s</li>\n" % _inline(e) for e in entries))
            continue
        j = i
        para = []
        while j < len(lines) and lines[j].strip() and not lines[j].startswith(("- ", "```", "    ")):
            para.append(lines[j].strip())
            j += 1
        out.append("<p>%s</p>\n" % _inline(" ".join(para)))
        i = j
    return "".join(out)


def _program(module, body):
    imports = "import std;\n" + ("import %s;\n" % module if module != "std" else "")
    inner = "\n".join(("    " + l) if l.strip() else "" for l in body.split("\n"))
    return "%s\nfn main()\n{\n%s\n}\n" % (imports, inner)


def _entry(x):
    name = x["item"]
    head = '<h4 id="%s" class="stdref-item"><code>%s</code> <span class="std-file">%s</span></h4>\n' % (
        anchor(x["module"], name), html.escape(name), html.escape(x["kind"]))
    sig = '<figure class="code sig"><pre><code>%s</code></pre></figure>\n' % highlight(x["signature"])
    doc = markdown(x["doc"]) if x["doc"] else '<p class="stdref-missing">Not documented yet.</p>\n'
    ex = ""
    for e in x["examples"]:
        title = "Example: %s" % e["doc"].rstrip(".") if e["doc"] else "Example"
        ex += code(e["code"], title=title, expect="ok", section="stdref",
                   slug="%s %s" % (x["module"], e["name"] or name), run_src=_program(x["module"], e["code"]))
    return '<div class="stdref-entry">\n%s%s%s%s</div>\n' % (head, sig, doc, ex)


def _order(module_items):
    """A module's items: each type with its associated functions after
    it, then the free functions, the constants and the externs, each
    group in the order of the source."""
    types = [x for x in module_items if x["kind"] in ("struct", "enum", "bitstruct")]
    assoc = {}
    for x in module_items:
        if x["kind"] in ("fn", "extern fn", "built in fn") and "::" in x["item"]:
            assoc.setdefault(x["item"].split("::")[0], []).append(x)
    out = []
    for t in types:
        out.append((t, assoc.pop(t["item"], [])))
    # associated functions of a type declared elsewhere (a native one)
    for fs in assoc.values():
        out.append((None, fs))
    free = [x for x in module_items if x["kind"] in ("fn", "built in fn") and "::" not in x["item"]]
    consts = [x for x in module_items if x["kind"] == "const"]
    externs = [x for x in module_items if x["kind"] == "extern fn" and "::" not in x["item"]]
    return out, free, consts, externs


def reference_html():
    all_items = items()
    modules = []
    for x in all_items:
        if x["module"] not in modules:
            modules.append(x["module"])
    total = len(all_items)
    documented = sum(1 for x in all_items if x["doc"])
    b = p("""Every item a program can use from `std`, module by module, with its declaration, what it does,
and for some an example. The text is the `///` documentation in `impl/std` itself
(`spec/22` §1), read by `impl/tools/cobdoc.py` when this page is built; each example is a `unittest`
block of `std` (`coby --unittest-std`), run here as a program's `main` under both tools. The items both
tools build in, which have no CobaltC body (`printf`, `sqrt`, `exit`, …), are documented by hand in
`impl/std/builtins.md` and shown here as <em>built in</em>. §21 says which module to look in;
`spec/21` is the normative text.""")
    if documented < total:
        b += p("%d of the %d items are documented so far." % (documented, total))
    nav = ", ".join('<a href="#%s"><code>%s</code></a>' % (anchor(m), m) for m in modules)
    b += '<p class="std-nav">%s</p>\n' % nav
    for m in modules:
        mi = [x for x in all_items if x["module"] == m]
        src = [x for x in mi if x["kind"] != "built in fn"]
        b += '<h3 id="%s"><code>%s</code> <span class="std-file">impl/std/%s</span></h3>\n' % (
            anchor(m), m, html.escape((src or mi)[0]["file"]))
        typed, free, consts, externs = _order(mi)
        for t, fs in typed:
            if t is not None:
                b += _entry(t)
            for f in fs:
                b += _entry(f)
        for f in free:
            b += _entry(f)
        for c in consts + externs:
            b += _entry(c)
    return b


def section():
    s = Section("stdref", "R", "The std reference", None,
                "Every exported item of `std`: its declaration, its documentation and its examples, "
                "generated from the library's source.",
                reference_html())
    s.group = "Library reference"
    return s
