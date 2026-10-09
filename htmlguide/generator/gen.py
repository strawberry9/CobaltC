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
import resource
import subprocess
import sys

# Every program the checker runs: at most 120 s of CPU, passed on by
# `cobc --run` to what it compiled (COBALTC_CPU_LIMIT).
def _limit_cpu():
    resource.setrlimit(resource.RLIMIT_CPU, (120, 121))

_run_env = dict(os.environ, COBALTC_CPU_LIMIT="120")

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
import export unsafe extern break continue move mut true false as void for const
foreach""".split())
TYPES = set("""i8 i16 i32 i64 i128 u8 u16 u32 u64 u128 isize usize f32 f64 bool str
ref rawptr array handle mutex guard shared exclusive slice""".split())
PRELUDE = set("""Vec String Rc Option Result Some None Ok Err Mutex AllocError
Utf8Error drop spawn join lock sizeof alignof widen narrow narrow_wrapping
reinterpret to_float to_int wrapping_add wrapping_sub wrapping_mul
saturating_add saturating_sub saturating_mul checked_add checked_sub
checked_mul checked_div checked_rem rawptr_of reclaim release copy_raw
allocate deallocate reinterpret_ptr write map_err dangling fault
print str_len str_byte str_ptr min_value max_value
Box HashMap HashSet Queue PriorityQueue File StringView Channel FileError ParseError
PathKind read read_line read_file write_file read_bytes write_bytes
printf eprintf sprintf parse swap assert static_assert
min max abs pow sqrt floor ceil round trunc
arg arg_count arg_bytes make_dir remove_file remove_dir rename list_dir
path_kind env_var current_dir monotonic_ns unix_seconds
DateTime Weekday unix_ms local_offset_seconds Sha256 HmacSha256 sha256 hmac_sha256 digest_eq hkdf_extract hkdf_expand hkdf_sha256 chacha20 poly1305 chacha20_poly1305_seal chacha20_poly1305_open x25519 x25519_public_key x25519_private_key Sha512 Sha384 sha512 sha384 HashKind hash RsaPublicKey rsa_verify_pkcs1v15 rsa_verify_pss EcCurve ecdsa_verify hkdf_expand_label Hasher Hmac hmac hkdf_extract_with hkdf_expand_with hkdf_with hkdf_expand_label_with ec_private_key ec_public_key ecdh ecdsa_sign ed25519_private_key ed25519_public_key ed25519_sign ed25519_verify RsaPrivateKey rsa_sign_pkcs1v15 rsa_sign_pss rsa_generate_key Blake2b blake2b pbkdf2 argon2id hash_password verify_password EcPrivateKey PrivateKey sign BigUint BigDivision Certificate CertKey CertError SignatureScheme TrustStore verify_chain verify_signature certificates_from_pem der_read Der TlsStream TlsError Url percent_encode percent_decode Header find_header Request Response reason_phrase HttpClient http_get http_post HttpError HttpServer HttpConnection to_hex from_hex base64_encode base64_decode exit FileInfo file_info copy_file make_dir_all
remove_dir_all set_current_dir temp_dir home_dir path_join path_parent
path_file_name path_stem path_extension path_is_absolute path_normalize
path_canonical os_random_bytes os_random_u64 Command Child Output
watch_interrupts interrupt_requested IpAddr SocketAddr resolve
TcpListener TcpStream UdpSocket Datagram NetError read_all read_all_bytes flush_stdout PgConnection PgUrl PgSslMode PgValue PgColumn PgRow PgResult PgError PgServerError Scram scram_salted_password
clamp is_nan is_finite gcd asin acos atan uuid_v4 iso_from_unix_ms unix_ms_from_iso current_exe hostname process_id cpu_count Sha1 sha1 Param find_param form_decode form_encode Json JsonMember JsonNull JsonBool JsonNumber JsonText JsonArray JsonObject CompressError deflate inflate zlib_deflate zlib_inflate gzip gunzip adler32 Args""".split())

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

# The code font, embedded so the page is one file that needs nothing else:
# Cascadia Mono's latin subset (a variable font, one file for every weight),
# as Google Fonts serves it, in fonts/ with its licence (SIL OFL 1.1, which
# allows embedding with the notice). Characters outside the range come
# from the fallback fonts in CSS's `font-family`, as they did before.
def font_css():
    import base64
    here = os.path.dirname(os.path.abspath(__file__))
    with open(os.path.join(here, "fonts", "CascadiaMono-latin.woff2"), "rb") as f:
        data = base64.b64encode(f.read()).decode("ascii")
    return ("/* Cascadia Mono, Copyright (c) 2019 - Present, Microsoft Corporation, with Reserved Font Name"
            " Cascadia Code; SIL Open Font License 1.1 (https://openfontlicense.org). */\n"
            "@font-face { font-family: 'Cascadia Mono'; font-style: normal; font-weight: 200 700; font-display: swap;"
            " src: url(data:font/woff2;base64,%s) format('woff2');"
            " unicode-range: U+0000-00FF, U+0131, U+0152-0153, U+02BB-02BC, U+02C6, U+02DA, U+02DC, U+0304, U+0308,"
            " U+0329, U+2000-206F, U+20AC, U+2122, U+2191, U+2193, U+2212, U+2215, U+FEFF, U+FFFD; }\n" % data)


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
/* A code font whose zero cannot be taken for a letter O: Cascadia Mono's
   zero is dotted by design (no OpenType feature needed, so the mark
   survives font fallback), it has no ligatures (`->`, `=>` and `!=`
   render exactly as typed), and it reads quieter than a slashed zero.
   The fallbacks (Consolas, Menlo) mark their zeros too. */
code, pre { font-family: "Cascadia Mono", Consolas, ui-monospace, SFMono-Regular, Menlo, "DejaVu Sans Mono", "Liberation Mono", monospace; }
code { background: var(--code-bg); border: 1px solid var(--border); border-radius: 4px; padding: 0 5px; font-size: 0.9em; color: #e6ebf3; white-space: nowrap; }
code.priv { color: var(--bad); opacity: 0.55; background: none; border-color: transparent; font-size: 0.8em; }
.nat { color: var(--muted); font-size: 0.7em; margin-left: 1px; }
.std-file { color: var(--muted); font-size: 0.8rem; font-weight: normal; margin-left: 8px; }
.std-nav { line-height: 2; }
table.std-inv td:first-child { white-space: nowrap; vertical-align: top; }
table.std-inv td:last-child { line-height: 1.9; }
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
.book-navigation
{
            display: flex;
            justify-content: space-between;
            align-items: center;
            margin-top: 50px;
            padding-top: 20px;
            border-top: 1px solid #374151;
            font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, Helvetica, Arial, sans-serif;
        }
        .nav-btn {
            background-color: #374151;
            color: #f3f4f6;
            padding: 8px 16px;
            border-radius: 6px;
            text-decoration: none;
            font-size: 14px;
            transition: background-color 0.2s;
        }
        .nav-btn:hover {
            background-color: #4b5563;
        }
        .nav-info {
            color: #9ca3af;
            font-size: 14px;
        }

        .logo-container {
            float: left;
            margin: 0 30px 20px 0;
            width: 30%;
            height: 30%;          
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

# The preface (generator/preface.html), verbatim, without its leading
# comment.
PREFACE = os.path.join(os.path.dirname(os.path.abspath(__file__)), "preface.html")


# The specification edition the guide describes: the one `spec/README.md`
# records (its "Edition" section), so the two never disagree.
SPEC_README = os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", "..", "spec", "README.md")


def edition():
    m = re.search(r"\*\*CobaltC specification edition (\d{4}\.(?:\d{6}|\d{3}))\.\*\*", open(SPEC_README, encoding="utf-8").read())
    if not m:
        sys.exit("gen.py: no \"**CobaltC specification edition YYYY.MMDDnn.**\" line in " + SPEC_README)
    return m.group(1)


def preface():
    text = open(PREFACE, encoding="utf-8").read()
    return re.sub(r"\A<!--.*?-->\n", "", text, flags=re.S)


def build(sections, title, description):
    # The preface (a hand-written section) gets the first entry; the
    # rest come from the registered sections.
    nav = ['<li><a href="#preface">Preface</a></li>']
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
<meta name="description" content="Appendix VI - CobaltC Programming Language Guide.">
<style>%s</style>
</head>
<body>
<a id="top"></a>
<div class="layout">
<aside class="sidebar">
<div class="brand">Cobalt<span>C</span></div>
<div class="sub">Language guide &amp; reference</div>
<div class="sub">Specification edition %s</div>
<ol>
%s
</ol>
</aside>
<main>
%s
<p class="edition">This guide describes the CobaltC specification, edition <strong>%s</strong>.</p>
%s

<div class="book-navigation">
    <a href="Appendix_05.html" class="nav-btn">&larr; Previous</a>
    <a href="Appendix_07.html" class="nav-btn">Appendix VII &rarr;</a>
</div>

<footer>
  <p>
    This work is licensed under a 
    <a rel="license" href="https://creativecommons.org/licenses/by-nc-nd/4.0/" target="_blank" rel="noopener">
      Creative Commons Attribution-NonCommercial-NoDerivatives 4.0 International License
    </a>.
  </p>
</footer>

</main>
</div>
<script>%s</script>
</body>
</html>
""" % (html.escape(title), font_css() + CSS, edition(), "\n".join(nav), preface(), edition(), "".join(body), JS)

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
                # CPU-limited, and killed on time out; `cobc --run`'s program
                # dies with `cobc`, so nothing it started is left running.
                r = subprocess.run(cmd, capture_output=True, timeout=60, cwd=os.path.dirname(path), preexec_fn=_limit_cpu, env=_run_env)
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
                ok = r.returncode != 0 and "diag.syntax-error" in first
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
    import content_intro, content_a, content_b, content_c, content_perf
    # The practice chapter (content_perf) sits after the reference, before the appendix.
    sections = (content_intro.SECTIONS + content_a.SECTIONS + content_b.SECTIONS + content_c.SECTIONS[:-1]
                + content_perf.SECTIONS + content_c.SECTIONS[-1:])
    if "--check" in sys.argv:
        sys.exit(1 if check_examples() else 0)
    page = build(sections, "CobaltC Language Guide",
                 "A human-oriented guide to the CobaltC programming language: objectives, a tour, and one section per specification chapter.")
    os.makedirs(os.path.dirname(OUT), exist_ok=True)
    with open(OUT, "w", encoding="utf-8") as f:
        f.write(page)
    # The same page as Appendix_06.html beside it, for publishing
    # elsewhere, where the images sit in the same directory as the page;
    # any earlier copy is replaced.
    with open(os.path.join(os.path.dirname(OUT), "Appendix_06.html"), "w", encoding="utf-8") as f:
        f.write(page.replace('src="../images/', 'src="'))
    import gen as g
    print("wrote %s (%d bytes, %d sections, %d runnable examples)" % (OUT, len(page.encode()), len(sections), len(g.EXAMPLES)))

if __name__ == "__main__":
    main()
