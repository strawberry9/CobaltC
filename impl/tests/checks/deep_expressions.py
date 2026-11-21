#!/usr/bin/env python3
# The compiler reads a program however deeply its expressions nest (D-0326,
# CHG-0459): 20 000 nested parentheses and a sum of 50 000 terms are written
# out as C, each within a minute. The parser, the checker and the lowering
# recurse on the nesting, so the compiler runs on a stack larger than a
# program's (impl/cobc/host.c), and a node answers whether it is a literal
# expression without walking its subtree (impl/front/ast.cb).
#
#   python3 -I impl/tests/checks/deep_expressions.py [--cobc PATH]
import os
import resource
import subprocess
import sys
import tempfile

ROOT = os.path.dirname(os.path.dirname(os.path.dirname(os.path.dirname(os.path.abspath(__file__)))))
cobc = os.path.join(ROOT, "impl", "target", "release", "cobc")
args = sys.argv[1:]
if len(args) == 2 and args[0] == "--cobc":
    cobc = os.path.abspath(args[1])
elif args:
    sys.exit("usage: deep_expressions.py [--cobc PATH]")

PROGRAMS = {
    "parens": "i64 x = " + "(" * 20000 + "1" + ")" * 20000 + ";",
    "sum": "i64 x = " + " + ".join(["1"] * 50000) + ";",
}


def limit():
    resource.setrlimit(resource.RLIMIT_CPU, (60, 60))


bad = 0
with tempfile.TemporaryDirectory() as d:
    for name, line in PROGRAMS.items():
        path = os.path.join(d, name + ".cb")
        with open(path, "w") as f:
            f.write('import std;\n\nfn main()\n{\n    %s\n    printf("%%v\\n", x);\n}\n' % line)
        r = subprocess.run([cobc, "--emit-c", "-o", name + ".c", name + ".cb"], cwd=d, capture_output=True, text=True, preexec_fn=limit)
        if r.returncode != 0 or not os.path.exists(os.path.join(d, name + ".c")):
            bad += 1
            print("deep_expressions: %s: cobc failed (%s): %s" % (name, r.returncode, r.stderr.strip()[:200]))
print("deep_expressions: %d programs, %d not compiled" % (len(PROGRAMS), bad))
sys.exit(1 if bad else 0)
