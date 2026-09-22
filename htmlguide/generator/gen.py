#!/usr/bin/env python3
"""Generator for the CobaltC HTML language guide.

Usage:
    python3 gen.py            -> writes ../index.html
    python3 gen.py --check    -> runs every full example through ~/CobaltC/impl/target/release/coby
                                 and cobc --run (same directory) under each C compiler in $COBC_CCS
                                 (default "gcc clang"); override the tools with $COBALTC / $COBC
                                 (writes the extracted programs to ./check/)
"""
import html
import os
import re
import subprocess
import sys

OUT = os.path.join(os.path.dirname(os.path.dirname(os.path.abspath(__file__))), "index.html")
# The reference interpreter; override with COBALTC=/path/to/coby if the repo moves.
COBALTC = os.environ.get("COBALTC", os.path.expanduser("~/CobaltC/impl/target/release/coby"))
# The native compiler; every example must give the same verdict and output under it.
COBC = os.environ.get("COBC", os.path.join(os.path.dirname(COBALTC), "cobc"))
# The C compilers `cobc` is checked under: each must give the same verdict and output.
COBC_CCS = os.environ.get("COBC_CCS", "gcc clang").split()
CHECK_DIR = os.path.join(os.path.dirname(os.path.abspath(__file__)), "check")

# ---------------------------------------------------------------- highlighting

KEYWORDS = set("""fn struct enum resource match if else while return auto module
import export unsafe extern break continue move mut true false as void for const""".split())
TYPES = set("""i8 i16 i32 i64 i128 u8 u16 u32 u64 u128 isize usize f32 f64 bool str
ref rawptr array handle mutex guard shared exclusive""".split())
PRELUDE = set("""Vec String Rc Option Result Some None Ok Err Mutex AllocError
Utf8Error drop spawn join lock sizeof alignof widen narrow narrow_wrapping
reinterpret to_float to_int wrapping_add wrapping_sub wrapping_mul
saturating_add saturating_sub saturating_mul checked_add checked_sub
checked_mul checked_div checked_rem rawptr_of reclaim release copy_raw
allocate deallocate reinterpret_ptr write map_err dangling fault
print str_len str_byte str_ptr min_value max_value""".split())

TOKEN_RE = re.compile(
    r"(?P<comment>//[^\n]*)"
    r'|(?P<string>b?"(?:\\.|[^"\\\n])*")'
    r"|(?P<num>\b\d+(?:\.\d+)?(?::[a-z][a-z0-9]*)?)"
    r"|(?P<ident>[A-Za-z_][A-Za-z0-9_]*)"
    r"|(?P<other>.)",
    re.S,
)

def _hl_comment(text):
    t = html.escape(text)
    t = t.replace("✓", '<span class="ok">✓</span>')
    t = t.replace("✗", '<span class="bad">✗</span>')
    t = re.sub(r"(diag\.[a-z0-9-]+)", r'<span class="dg">\1</span>', t)
    return '<span class="c">%s</span>' % t

def highlight(src):
    out = []
    for m in TOKEN_RE.finditer(src):
        kind = m.lastgroup
        text = m.group()
        if kind == "comment":
            out.append(_hl_comment(text))
        elif kind == "string":
            out.append('<span class="s">%s</span>' % html.escape(text))
        elif kind == "num":
            out.append('<span class="n">%s</span>' % html.escape(text))
        elif kind == "ident":
            if text in KEYWORDS:
                out.append('<span class="k">%s</span>' % text)
            elif text in TYPES:
                out.append('<span class="t">%s</span>' % text)
            elif text in PRELUDE:
                out.append('<span class="p">%s</span>' % text)
            else:
                out.append(html.escape(text))
        else:
            out.append(html.escape(text))
    return "".join(out)

# ---------------------------------------------------------------- inline prose

def inl(text):
    """Convert `code` spans to <code>, escaping their contents. Everything
    outside backticks is passed through as HTML."""
    parts = text.split("`")
    for i in range(1, len(parts), 2):
        parts[i] = "<code>%s</code>" % html.escape(parts[i])
    return "".join(parts)

def p(text):
    return "<p>%s</p>\n" % inl(text)

def h3(text, anchor=None):
    a = ' id="%s"' % anchor if anchor else ""
    return "<h3%s>%s</h3>\n" % (a, inl(text))

def h4(text):
    return "<h4>%s</h4>\n" % inl(text)

def ul(items):
    return "<ul>\n%s</ul>\n" % "".join("<li>%s</li>\n" % inl(i) for i in items)

def ol(items):
    return "<ol>\n%s</ol>\n" % "".join("<li>%s</li>\n" % inl(i) for i in items)

def table(headers, rows, cls=""):
    c = ' class="%s"' % cls if cls else ""
    s = "<table%s>\n<thead><tr>%s</tr></thead>\n<tbody>\n" % (
        c, "".join("<th>%s</th>" % inl(h) for h in headers))
    for r in rows:
        s += "<tr>%s</tr>\n" % "".join("<td>%s</td>" % inl(x) for x in r)
    return s + "</tbody>\n</table>\n"

def note(text, kind="note", title=None):
    """kind: note | tip | warn | rule"""
    labels = {"note": "Note", "tip": "Tip", "warn": "Watch out", "rule": "Rule of thumb"}
    t = title or labels[kind]
    return '<div class="callout %s"><div class="callout-title">%s</div>%s</div>\n' % (
        kind, inl(t), inl(text) if not text.startswith("<") else text)

def rules(items):
    """A 'rules of thumb' box with bullets."""
    return '<div class="callout rule"><div class="callout-title">Rules of thumb</div>%s</div>\n' % ul(items)

# ---------------------------------------------------------------- code blocks

EXAMPLES = []   # (section_id, slug, src, expect, output, interp, files, cobc_only, args, status)

def code(src, title=None, expect=None, output=None, section=None, slug=None, interp=None, files=None, cobc_only=False, args=(), status=0):
    """A code block.

    expect: "ok" | "diag.xxx" | None (fragment, not run)
    output: expected stdout bytes (str) for full programs that print
    interp: what the reference interpreter actually reports, when it
            differs from the language's own verdict (documented divergence)
    files:  {name: source} companion files a multi-file program loads
            with `module m "./name";` -- written beside the program
            (which is then saved as main.cb) when checked; C files too
    cobc_only: only the compiler can run it (it links C code or reads
            input): the interpreter must refuse it (exit 3, `unsupported:`)
    args:   the program's arguments, passed after the file to both tools
    status: the exit status an "ok" example ends with (`fn main() : u8`)
    """
    src = src.strip("\n")
    lines = src.split("\n")
    # normalise: strip common indentation
    indents = [len(l) - len(l.lstrip()) for l in lines if l.strip()]
    if indents and min(indents) > 0:
        k = min(indents)
        lines = [l[k:] if l.strip() else "" for l in lines]
    src = "\n".join(lines)
    assert "\t" not in src, "tabs in example"
    if "fn main" in src and expect is not None:
        EXAMPLES.append((section, slug or title or "example", src, expect, output, interp, files, cobc_only, list(args), status))
    badge = ""
    if expect == "ok":
        badge = '<span class="badge ok">runs</span>'
    elif expect and expect.startswith("diag."):
        badge = '<span class="badge bad">rejected: %s</span>' % html.escape(expect)
    elif expect == "parse-error":
        badge = '<span class="badge bad">parse error</span>'
    head = ""
    if title or badge:
        head = '<div class="code-head"><span class="code-title">%s</span>%s</div>' % (
            inl(title) if title else "", badge)
    out = ""
    if output is not None:
        out = '<div class="code-output"><span class="out-label">output</span><pre>%s</pre></div>' % html.escape(output)
    return '<figure class="code">%s<pre><code>%s</code></pre>%s</figure>\n' % (head, highlight(src), out)

# ---------------------------------------------------------------- sections

class Section:
    def __init__(self, sid, num, title, spec=None, blurb="", body=""):
        self.id = sid
        self.num = num
        self.title = title
        self.spec = spec
        self.blurb = blurb
        self.body = body

def section_html(sec, prev_s, next_s):
    spec_line = ""
    if sec.spec:
        spec_line = '<div class="spec-ref">Governing specification file: <code>spec/%s</code></div>' % html.escape(sec.spec)
    num = '<span class="secnum">%s</span>' % html.escape(sec.num) if sec.num else ""
    nav = '<nav class="pager">'
    if prev_s:
        nav += '<a class="prev" href="#%s">&larr; %s</a>' % (prev_s.id, html.escape(prev_s.short()))
    nav += '<a class="top" href="#top">Top</a>'
    if next_s:
        nav += '<a class="next" href="#%s">%s &rarr;</a>' % (next_s.id, html.escape(next_s.short()))
    nav += "</nav>"
    return (
        '<section id="%s">\n<h2>%s%s</h2>\n%s'
        '<p class="blurb">%s</p>\n%s\n%s</section>\n'
        % (sec.id, num, html.escape(sec.title), spec_line, inl(sec.blurb), sec.body, nav)
    )

def _short(self):
    return ("%s %s" % (self.num, self.title)) if self.num else self.title
Section.short = _short

# ---------------------------------------------------------------- page shell

CSS = r"""
:root {
  --bg: #23272e;
  --bg-side: #1e2228;
  --surface: #2b3039;
  --surface-2: #323845;
  --code-bg: #1c2027;
  --border: #3a4150;
  --text: #d9dee7;
  --muted: #9aa5b5;
  --accent: #7fb0ff;
  --accent-2: #4f8ff7;
  --ok: #7ed491;
  --bad: #ff8a80;
  --warn: #ffd27a;
  --kw: #c792ea;
  --ty: #82c7ff;
  --pre: #ffd27a;
  --num: #f7a86b;
  --cmt: #8b96a8;
  --side-w: 290px;
}
* { box-sizing: border-box; }
html { scroll-behavior: smooth; scroll-padding-top: 16px; }
body {
  margin: 0; background: var(--bg); color: var(--text);
  font: 16px/1.6 -apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, Helvetica, Arial, sans-serif;
}
a { color: var(--accent); text-decoration: none; }
a:hover { text-decoration: underline; }
code, pre { font-family: ui-monospace, SFMono-Regular, Menlo, Consolas, "Liberation Mono", monospace; }
code { background: var(--code-bg); border: 1px solid var(--border); border-radius: 4px; padding: 0 5px; font-size: 0.9em; color: #e6ebf3; white-space: nowrap; }
pre code { background: none; border: none; padding: 0; white-space: pre; font-size: inherit; color: inherit; }
.layout { display: flex; min-height: 100vh; }
.sidebar {
  width: var(--side-w); flex: 0 0 var(--side-w); background: var(--bg-side);
  border-right: 1px solid var(--border); position: sticky; top: 0; height: 100vh; overflow-y: auto;
  padding: 18px 14px 40px;
}
.sidebar .brand { font-weight: 700; font-size: 1.25rem; color: #fff; margin: 4px 0 2px; }
.sidebar .brand span { color: var(--accent-2); }
.sidebar .sub { color: var(--muted); font-size: 0.8rem; margin-bottom: 14px; }
.sidebar ol { list-style: none; margin: 0; padding: 0; }
.sidebar li a { display: block; padding: 5px 8px; border-radius: 6px; color: var(--text); font-size: 0.9rem; line-height: 1.3; }
.sidebar li a:hover { background: var(--surface); text-decoration: none; }
.sidebar li a.active { background: var(--surface-2); color: #fff; }
.sidebar .n { color: var(--accent); font-variant-numeric: tabular-nums; margin-right: 6px; font-size: 0.8rem; }
.sidebar .group { color: var(--muted); font-size: 0.72rem; text-transform: uppercase; letter-spacing: .08em; margin: 14px 8px 4px; }
main { flex: 1 1 auto; min-width: 0; padding: 28px 48px 80px; max-width: 1100px; }
section { margin-bottom: 64px; padding-top: 8px; border-top: 1px solid var(--border); }
section:first-of-type { border-top: none; }
h1 { font-size: 2.2rem; margin: 0 0 6px; color: #fff; }
h2 { font-size: 1.7rem; margin: 22px 0 6px; color: #fff; }
h3 { font-size: 1.22rem; margin: 30px 0 8px; color: #eef2f8; }
h4 { font-size: 1.02rem; margin: 20px 0 6px; color: #dfe5ee; }
.secnum { display: inline-block; color: var(--accent); font-size: 1rem; font-weight: 600; margin-right: 10px; vertical-align: middle; background: var(--surface); padding: 2px 8px; border-radius: 6px; border: 1px solid var(--border); }
.spec-ref { color: var(--muted); font-size: 0.85rem; margin-bottom: 8px; }
.blurb { color: #c3cad6; font-size: 1.05rem; border-left: 3px solid var(--accent-2); padding-left: 12px; margin: 10px 0 18px; }
p { margin: 0 0 12px; }
ul, ol { margin: 0 0 12px; padding-left: 26px; }
li { margin: 3px 0; }
table { border-collapse: collapse; width: 100%; margin: 10px 0 18px; font-size: 0.93rem; }
th, td { border: 1px solid var(--border); padding: 7px 10px; text-align: left; vertical-align: top; }
th { background: var(--surface); color: #fff; }
tbody tr:nth-child(even) { background: rgba(255,255,255,0.025); }
td code { white-space: nowrap; }
figure.code { margin: 14px 0 18px; background: var(--code-bg); border: 1px solid var(--border); border-radius: 8px; overflow: hidden; }
figure.code pre { margin: 0; padding: 12px 16px; overflow-x: auto; font-size: 0.88rem; line-height: 1.5; }
.code-head { display: flex; justify-content: space-between; align-items: center; background: var(--surface); padding: 6px 14px; border-bottom: 1px solid var(--border); font-size: 0.85rem; color: #dfe5ee; }
.code-title { font-weight: 600; }
.badge { font-size: 0.72rem; padding: 2px 8px; border-radius: 999px; border: 1px solid; font-family: ui-monospace, monospace; }
.badge.ok { color: var(--ok); border-color: var(--ok); }
.badge.bad { color: var(--bad); border-color: var(--bad); }
.code-output { border-top: 1px dashed var(--border); background: #181c22; }
.code-output pre { padding: 8px 16px; color: #c9d2de; }
.out-label { display: block; padding: 4px 16px 0; font-size: 0.72rem; color: var(--muted); text-transform: uppercase; letter-spacing: .08em; }
.k { color: var(--kw); } .t { color: var(--ty); } .p { color: var(--pre); } .n { color: var(--num); }
.s { color: #a5d6a7; }
.c { color: var(--cmt); font-style: italic; }
.c .ok { color: var(--ok); font-style: normal; } .c .bad { color: var(--bad); font-style: normal; }
.c .dg { color: #e2b7ff; font-style: normal; }
.callout { border: 1px solid var(--border); border-left-width: 4px; border-radius: 8px; padding: 10px 14px; margin: 14px 0 18px; background: var(--surface); }
.callout p:last-child, .callout ul:last-child { margin-bottom: 0; }
.callout-title { font-weight: 700; font-size: 0.85rem; text-transform: uppercase; letter-spacing: .06em; margin-bottom: 4px; }
.callout.note { border-left-color: var(--accent-2); } .callout.note .callout-title { color: var(--accent); }
.callout.tip { border-left-color: var(--ok); } .callout.tip .callout-title { color: var(--ok); }
.callout.warn { border-left-color: var(--warn); } .callout.warn .callout-title { color: var(--warn); }
.callout.rule { border-left-color: #b48cff; } .callout.rule .callout-title { color: #c9adff; }
.pager { display: flex; justify-content: space-between; gap: 12px; margin-top: 28px; padding-top: 12px; border-top: 1px dashed var(--border); font-size: 0.9rem; }
.pager a { padding: 6px 10px; border-radius: 6px; background: var(--surface); border: 1px solid var(--border); }
.pager a:hover { background: var(--surface-2); text-decoration: none; }
.pager .top { color: var(--muted); }
.legend { display: grid; grid-template-columns: auto 1fr; gap: 6px 14px; align-items: center; margin: 10px 0 18px; }
.two { display: grid; grid-template-columns: 1fr 1fr; gap: 0 24px; }
.menu-btn { display: none; }
@media (max-width: 900px) {
  .layout { display: block; }
  .sidebar { position: static; height: auto; width: auto; border-right: none; border-bottom: 1px solid var(--border); }
  main { padding: 18px 16px 60px; }
  .two { grid-template-columns: 1fr; }
  table { display: block; overflow-x: auto; }
}
"""

JS = r"""
(function () {
  var links = Array.prototype.slice.call(document.querySelectorAll('.sidebar a[href^="#"]'));
  var secs = links.map(function (a) { return document.getElementById(a.getAttribute('href').slice(1)); });
  function update() {
    var y = window.scrollY + 90, cur = 0;
    for (var i = 0; i < secs.length; i++) { if (secs[i] && secs[i].offsetTop <= y) cur = i; }
    links.forEach(function (a, i) { a.classList.toggle('active', i === cur); });
  }
  window.addEventListener('scroll', update, { passive: true });
  update();
})();
"""

def build(sections, title, description):
    nav = []
    group = None
    for s in sections:
        g = getattr(s, "group", None)
        if g and g != group:
            nav.append('<li class="group">%s</li>' % html.escape(g))
            group = g
        n = '<span class="n">%s</span>' % html.escape(s.num) if s.num else ""
        nav.append('<li><a href="#%s">%s%s</a></li>' % (s.id, n, html.escape(s.title)))
    body = []
    for i, s in enumerate(sections):
        prev_s = sections[i - 1] if i > 0 else None
        next_s = sections[i + 1] if i + 1 < len(sections) else None
        body.append(section_html(s, prev_s, next_s))
    return """<!DOCTYPE html>
<html lang="en">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<title>%s</title>
<meta name="description" content="%s">
<style>%s</style>
</head>
<body>
<a id="top"></a>
<div class="layout">
<aside class="sidebar">
<div class="brand">Cobalt<span>C</span></div>
<div class="sub">Language guide &amp; reference</div>
<ol>
%s
</ol>
</aside>
<main>
%s
</main>
</div>
<script>%s</script>
</body>
</html>
""" % (html.escape(title), html.escape(description), CSS, "\n".join(nav), "".join(body), JS)

# ---------------------------------------------------------------- checking

def check_examples():
    import gen as g                       # the content modules append to gen.EXAMPLES, not __main__'s
    examples = g.EXAMPLES if g is not sys.modules[__name__] else EXAMPLES
    os.makedirs(CHECK_DIR, exist_ok=True)
    fails = 0
    for i, (sec, slug, src, expect, output, interp, files, cobc_only, args, status) in enumerate(examples):
        expect = interp or expect
        name = re.sub(r"[^a-z0-9]+", "_", (slug or "ex").lower()).strip("_")
        if files:
            d = os.path.join(CHECK_DIR, "%02d_%s_%s" % (i, sec, name))
            os.makedirs(d, exist_ok=True)
            for fname, fsrc in files.items():
                with open(os.path.join(d, fname), "w") as f:
                    f.write(fsrc.strip("\n") + "\n")
            path = os.path.join(d, "main.cb")
        else:
            path = os.path.join(CHECK_DIR, "%02d_%s_%s.cb" % (i, sec, name))
        with open(path, "w") as f:
            f.write(src + "\n")
        verdicts = []
        tools = [("coby", [COBALTC, path] + args)] + [("cobc/" + cc, [COBC, "--cc", cc, "--run", path] + args) for cc in COBC_CCS]
        for tool, cmd in tools:
            try:
                # In the example's own directory under check/, so a file it
                # writes lands there, not beside the generator.
                r = subprocess.run(cmd, capture_output=True, timeout=60, cwd=os.path.dirname(path))
            except subprocess.TimeoutExpired:
                verdicts.append((tool, False, "TIMEOUT", "")); continue
            err = r.stderr.decode("utf-8", "replace")
            first = err.strip().split("\n")[0] if err.strip() else ""
            got_out = r.stdout.decode("utf-8", "replace")
            if cobc_only and tool == "coby":
                verdicts.append((tool, r.returncode == 3 and first.startswith("unsupported:"), first[:60], got_out))
                continue
            if expect == "ok":
                ok = r.returncode == status
            elif expect == "parse-error":
                ok = r.returncode != 0 and "parse error" in first
            else:
                ok = r.returncode != 0 and expect in first
            if ok and output is not None and got_out != output:
                ok = False
            verdicts.append((tool, ok, first[:60] if first else "exit %d" % r.returncode, got_out))
        ok = all(v[1] for v in verdicts)
        status = "PASS" if ok else "FAIL"
        if not ok:
            fails += 1
        print("%s  %-8s %-45s expect=%-40s %s%s" % (
            status, sec, name[:45], expect,
            "  ".join("%s=%s" % (v[0], v[2]) for v in verdicts),
            ("  stdout=%r" % verdicts[-1][3]) if output is not None else ""))
    print("\n%d examples, %d failures" % (len(examples), fails))
    return fails

def main():
    import content_intro, content_a, content_b, content_c
    sections = content_intro.SECTIONS + content_a.SECTIONS + content_b.SECTIONS + content_c.SECTIONS
    if "--check" in sys.argv:
        sys.exit(1 if check_examples() else 0)
    page = build(sections, "CobaltC Language Guide",
                 "A human-oriented guide to the CobaltC programming language: objectives, a tour, and one section per specification chapter.")
    os.makedirs(os.path.dirname(OUT), exist_ok=True)
    with open(OUT, "w", encoding="utf-8") as f:
        f.write(page)
    import gen as g
    print("wrote %s (%d bytes, %d sections, %d runnable examples)" % (OUT, len(page.encode()), len(sections), len(g.EXAMPLES)))

if __name__ == "__main__":
    main()
