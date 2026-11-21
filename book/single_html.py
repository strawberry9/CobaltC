#!/usr/bin/env python3
"""The book as one self-contained HTML file, for reading offline.

    python3 book/single_html.py [BUILD_DIR] [OUT]

Run after `mdbook build book`. It reads mdBook's print page (every chapter on
one page), puts its stylesheets, fonts, icons and scripts inside it, points the
sidebar's chapter links at the chapters on that page, and leaves out what
needs the site: the search (its results open the chapter pages; the browser's
find works on the one page) and the print dialog the print page opens on
load. Default: book/build to book/build/cobaltc-book.html.
"""

import base64
import os
import re
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
BUILD = os.path.abspath(sys.argv[1]) if len(sys.argv) > 1 else os.path.join(HERE, "build")
OUT = os.path.abspath(sys.argv[2]) if len(sys.argv) > 2 else os.path.join(BUILD, "cobaltc-book.html")

MIME = {".woff2": "font/woff2", ".woff": "font/woff", ".ttf": "font/ttf", ".svg": "image/svg+xml",
        ".png": "image/png", ".jpg": "image/jpeg", ".gif": "image/gif"}


def fail(why):
    sys.exit("single_html: " + why)


def read(rel, base=BUILD):
    p = os.path.normpath(os.path.join(base, rel))
    if not os.path.isfile(p):
        fail("missing %s (run `mdbook build book` first)" % p)
    with open(p, encoding="utf-8") as f:
        return f.read(), os.path.dirname(p)


def data_uri(path):
    ext = os.path.splitext(path)[1].lower()
    if ext not in MIME:
        fail("no media type for %s" % path)
    with open(path, "rb") as f:
        return "data:%s;base64,%s" % (MIME[ext], base64.b64encode(f.read()).decode("ascii"))


def inline_css(rel):
    css, d = read(rel)

    def url(m):
        u = m.group(2)
        if u.startswith(("data:", "http:", "https:", "#")):
            return m.group(0)
        return "url('%s')" % data_uri(os.path.normpath(os.path.join(d, u.split("?")[0].split("#")[0])))

    return re.sub(r"""url\((['"]?)([^'")]+)\1\)""", url, css)


def script_text(js):
    # A script's text may not close its element.
    return js.replace("</script", "<\\/script")


page, _ = read("print.html")

# Each chapter's first heading, in the order of the sidebar (`SUMMARY.md`):
# the print page separates the chapters with a page break.
body = page.split('<main>', 1)[-1]
firsts = []
for part in body.split('<div style="break-before: page; page-break-before: always;"></div>'):
    m = re.search(r'<h1 id="([^"]+)"', part)
    firsts.append(m.group(1) if m else None)

# The search, and the print dialog.
page = re.sub(r'\s*<script src="(elasticlunr|mark|searcher)-[0-9a-f]+(\.min)?\.js"></script>', "", page)
page = re.sub(r"\s*<script>\s*window\.addEventListener\('load', function\(\) \{\s*window\.setTimeout\(window\.print, 100\);\s*\}\);\s*</script>", "", page)
page = re.sub(r'\s*window\.path_to_searchindex_js = "[^"]*";', "", page)
page = re.sub(r'\s*<noscript>\s*<iframe class="sidebar-iframe-outer" src="toc\.html"></iframe>\s*</noscript>', "", page)
page = page.replace('<meta name="robots" content="noindex">', "")

# The sidebar's links: chapter pages become their chapters here.
toc_rel = re.search(r'<script src="(toc-[0-9a-f]+\.js)"></script>', page)
if not toc_rel:
    fail("no toc script in print.html")
toc, _ = read(toc_rel.group(1))
chapters = []
for h in re.findall(r'href="([^"#]+\.html)"', toc):
    if h not in chapters and h != "print.html":
        chapters.append(h)
if len(chapters) != len(firsts) or None in firsts:
    fail("%d chapters in the sidebar, %d on the print page" % (len(chapters), len(firsts)))
anchor = dict(zip(chapters, firsts))
toc = re.sub(r'href="([^"#]+\.html)(#[^"]*)?"', lambda m: 'href="%s"' % (m.group(2) or "#" + anchor.get(m.group(1), "")), toc)
# Its links are then read as relative to the page root; here they are anchors.
toc = toc.replace("path_to_root + href", "href")
page = page.replace(toc_rel.group(0), "<script>\n%s\n</script>" % script_text(toc))

# Stylesheets, fonts and icons.
page = re.sub(r'<link rel="stylesheet"((?: id="[^"]+")?) href="([^"]+)"( media="[^"]+")?>',
              lambda m: "<style%s%s>\n%s\n</style>" % (m.group(1), m.group(3) or "", inline_css(m.group(2))), page)
page = re.sub(r'<link rel="(icon|shortcut icon)" href="([^"]+)">',
              lambda m: '<link rel="%s" href="%s">' % (m.group(1), data_uri(os.path.join(BUILD, m.group(2)))), page)

# The other scripts.
page = re.sub(r'<script src="([^"]+)"></script>', lambda m: "<script>\n%s\n</script>" % script_text(read(m.group(1))[0]), page)

# The preface's pointer to this file.
page, n = re.subn(r'\s*<p class="offline-copy">.*?</p>', "", page, flags=re.S)
if n != 1:
    fail("the preface's offline-copy paragraph was found %d times" % n)

# Images in the chapters.
page = re.sub(r'<img src="(?!data:|https?:)([^"]+)"', lambda m: '<img src="%s"' % data_uri(os.path.join(BUILD, m.group(1))), page)

# The print button prints this page (there is no other to go to).
page = page.replace('href="print.html"', 'href="#" onclick="window.print(); return false;"')

# No search here.
page = page.replace("</head>", "<style>#mdbook-search-toggle, #mdbook-search-wrapper { display: none !important; }</style>\n</head>", 1)

left = re.findall(r'(?:src|href)="(?!data:|https?:|#|mailto:)([^"]+)"', page)
left = [u for u in left if not u.startswith("javascript:")]
if left:
    fail("still points outside itself: %s" % ", ".join(sorted(set(left))[:10]))

with open(OUT, "w", encoding="utf-8") as f:
    f.write(page)
print("single_html: %s, %d chapters, %d KB" % (os.path.relpath(OUT), len(chapters), os.path.getsize(OUT) // 1024))
