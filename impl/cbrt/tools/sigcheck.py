#!/usr/bin/env python3
# Checks that every exported `cb_*` function the C runtime defines has the
# type cbrt.h declares (the interface must not change, D-0326 phase B), and
# that every function and variable cbrt.h declares is defined. A C file is
# written that includes cbrt.h, undefines its cb_* macros, and repeats each
# definition's prototype with the runtime's internal struct names mapped to
# cbrt.h's; the C compiler then rejects any conflicting type.
#
#   python3 -I tools/sigcheck.py [LIBCBRT.a]   (with a library: its globals are exports.txt's)
import os
import re
import shutil
import subprocess
import sys
import tempfile

HERE = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
SRC = os.path.join(HERE, "src")
H = os.path.join(HERE, "include", "cbrt.h")

MAP = {"CbRef": "cb_ref", "CbProj": "cb_proj", "CbType": "cb_type", "CbFmtArg": "cb_fmt_arg",
       "CbStr": "cb_str", "CbFnBox": "cb_fnbox", "CbField": "cb_field"}

defs = {}
DEF_RE = re.compile(r"^((?:_Noreturn |const |unsigned |struct )*[A-Za-z_][A-Za-z0-9_]*\s*\**\s*)(cb_[a-z0-9_]+)\s*\(([^;{]*)\)\s*$", re.M)
for f in sorted(os.listdir(SRC)):
    if not f.endswith(".c"):
        continue
    text = open(os.path.join(SRC, f), encoding="utf-8").read()
    for m in DEF_RE.finditer(text):
        ret, name, params = m.group(1), m.group(2), m.group(3)
        if "static" in ret:
            continue
        start = text.rfind("\n", 0, m.start()) + 1
        if text[start:m.start()].strip().startswith("static"):
            continue
        proto = "%s%s(%s);" % (ret, name, " ".join(params.split()))
        for a, b in MAP.items():
            proto = re.sub(r"\b%s\b" % a, b, proto)
        defs.setdefault(name, []).append((f, proto))

htext = open(H, encoding="utf-8").read()
macros = sorted(set(re.findall(r"^#define (cb_[a-z0-9_]+)\b", htext, re.M)))
out = ["#include \"%s\"" % H]
out += ["#undef %s" % m for m in macros]
for name in sorted(defs):
    for f, p in defs[name]:
        out.append("/* %s */ %s" % (f, p))
tmp = tempfile.mkdtemp(prefix="cbrt-sigcheck-")
chk = os.path.join(tmp, "sigcheck.c")
open(chk, "w").write("\n".join(out) + "\n")
r = subprocess.run(["gcc", "-std=gnu11", "-fsyntax-only", "-Wall", chk], capture_output=True, text=True)
shutil.rmtree(tmp, ignore_errors=True)
errs = [l for l in r.stderr.split("\n") if "error" in l or "conflicting" in l]
print("definitions found: %d" % len(defs))
for e in errs[:60]:
    print(e)
bad = False
if len(sys.argv) > 1:
    # The library's global symbols are exactly tools/exports.txt's (build.sh keeps only those).
    want = set(l.strip() for l in open(os.path.join(HERE, "tools", "exports.txt")) if l.strip())
    nm = subprocess.run(["nm", "-g", "--defined-only", sys.argv[1]], capture_output=True, text=True).stdout
    have = set(l.split()[2] for l in nm.split("\n") if len(l.split()) == 3)
    missing = sorted(want - have)
    extra = sorted(have - want)
    print("in exports.txt, not defined: %s" % (missing or "none"))
    print("defined, not in exports.txt: %s" % (extra or "none"))
    bad = bool(missing or extra)
sys.exit(1 if errs or bad else 0)
