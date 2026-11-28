#!/usr/bin/env python3
# cbtest keeps a case's result under a key that takes every setting of the
# compiler and the runtime that changes a run (impl/tests/cbtest/main.cb,
# RUN_SETTINGS). This holds that list to the names the sources read: each
# `COBALTC_*` name in the compiler, the back end, the front end, the runtime
# and the compiler's host code is in RUN_SETTINGS, or is named here as one
# that changes no run's result.
#
#   python3 -I impl/tests/checks/store_env.py
import os
import re
import sys

ROOT = os.path.dirname(os.path.dirname(os.path.dirname(os.path.dirname(os.path.abspath(__file__)))))
# Not part of a result: where things are, limits, the store itself, reports written beside the run.
NOT_A_SETTING = {
    "COBALTC_CBRT_DIR",          # where the runtime is
    "COBALTC_CPU_LIMIT",         # a limit: a case that meets it fails, and no failure is kept
    "COBALTC_RESULT_STORE",      # the store
    "COBALTC_TOOLCHAIN_KEY",     # the store's key
    "COBALTC_CERTIFICATE",       # a file written beside the run
    "COBALTC_TIER_REPORT",       # a report on standard error of the compiler, not of the program
    "COBALTC_HASH_SEED",         # the hash tables' seed (D-0330): where entries sit, which no program observes
    "COBALTC_TYPER",             # D-0342: a report on standard error of the compiler (the typer against the lowering)
    "COBALTC_VERDICT",           # D-0342: a report on standard error of the compiler (the walk against the lowering)
    "COBALTC_PROTO",             # D-0342: a report on standard error of the compiler (how the statements were built)
}
names = set()
for d in ("impl/cobc", "impl/back", "impl/cir", "impl/front", "impl/cbrt/src"):
    for f in sorted(os.listdir(os.path.join(ROOT, d))):
        if f.endswith((".cb", ".c", ".h")) and f != "prelude.cb":
            names.update(re.findall(r"COBALTC_[A-Z][A-Z_]*[A-Z]", open(os.path.join(ROOT, d, f), encoding="utf-8", errors="replace").read()))
main = open(os.path.join(ROOT, "impl", "tests", "cbtest", "main.cb"), encoding="utf-8").read()
m = re.search(r"const array<str, (\d+)> RUN_SETTINGS = \[(.*?)\];", main, re.S)
if not m:
    sys.exit("store_env: RUN_SETTINGS not found in impl/tests/cbtest/main.cb")
listed = set(re.findall(r'"(COBALTC_[A-Z_]+)"', m.group(2)))
bad = 0
if len(listed) != int(m.group(1)):
    print("store_env: RUN_SETTINGS declares %s names and lists %d" % (m.group(1), len(listed)))
    bad = 1
# A name a source writes across two string literals is read as its first part.
names = {n for n in names if not any(l != n and l.startswith(n) for l in listed | NOT_A_SETTING)}
for n in sorted(names - listed - NOT_A_SETTING):
    print("store_env: %s is read by the sources and is neither in RUN_SETTINGS nor named as changing no result" % n)
    bad = 1
for n in sorted(listed - names):
    print("store_env: %s is in RUN_SETTINGS and no source reads it" % n)
    bad = 1
print("store_env: %d settings in the store's key, %d names the sources read" % (len(listed), len(names)))
sys.exit(bad)
