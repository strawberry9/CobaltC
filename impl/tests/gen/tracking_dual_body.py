#!/usr/bin/env python3
"""Writes cases/cobc/tracking_dual_body_within_cap.case (D-0188: a dual
body within the cap, one body past it). The program is made here:
`feed_big`'s body is 700 lines, past `DUAL_LINES` (impl/back/bodies.cb).

    python3 impl/tests/gen/tracking_dual_body.py [--check]

Python 3 standard library only.
"""
import os
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
CASE = os.path.join(HERE, "..", "cases", "cobc", "tracking_dual_body_within_cap.case")

HEAD = """# D-0188: a function whose references are one exclusive and others shared,
# all to plain data and used directly, is emitted as a dispatcher over an
# unchecked `__fast` body and the checked `__slow` one (each defined: a line
# naming it that ends its signature); past `DUAL_LINES` (impl/back/bodies.cb)
# of fast body it is emitted once, checked (no `__fast` or `__slow` of it in
# the C at all). Both give the same results, and the dispatcher's in-flight
# assertion holds. With the record kept. Written by
# gen/tracking_dual_body.py, which makes the program's 700-line body.
"""

TAIL = """run env COBALTC_PATHLESS=0 $COBC --keep-c -o prog prog.cb
status 0
file-like prog.c \\ f_feed__fast(…)\\n
file-like prog.c \\ f_feed__slow(…)\\n
file-like prog.c \\ f_feed(…)\\n
file-lacks prog.c \\ f_feed_big__fast(
file-lacks prog.c \\ f_feed_big__slow(
file-like prog.c \\ f_feed_big(…)\\n
run ./prog
stdout-is 30 2110\\n
"""


def program():
    big = "".join("    a.n += widen<usize>(data[%d]);\n" % (k % 4) for k in range(700))
    return (
        "import std;\n\nstruct Acc\n{\n    u64 sum;\n    usize n;\n}\n\n"
        "fn feed(ref<Acc, exclusive> a, slice<u8, shared> data)\n{\n"
        "    for (usize i = 0; i < slice_len(data); i += 1)\n    {\n"
        "        a.sum += widen<u64>(data[i]);\n        a.n += 1;\n    }\n}\n\n"
        "fn feed_big(ref<Acc, exclusive> a, slice<u8, shared> data)\n{\n"
        + big
        + "}\n\nfn main()\n{\n    Vec<u8> v = Vec::filled(10, 3: u8);\n"
        "    Acc a = Acc { .sum = 0, .n = 0 };\n    feed(&mut a, &v[0..$]);\n"
        "    feed_big(&mut a, &v[2..6]);\n    printf(\"%v %v\\n\", a.sum, a.n);\n}\n"
    )


def text():
    src = program()
    assert src.endswith("\n")
    return HEAD + "file prog.cb\n" + src[:-1] + "\nend\n" + TAIL


def main():
    want = text()
    if "--check" in sys.argv[1:]:
        try:
            with open(CASE, encoding="utf-8") as f:
                have = f.read()
        except OSError:
            have = None
        if have != want:
            sys.exit("%s is not current: run gen/tracking_dual_body.py" % os.path.relpath(CASE))
        return
    if "--program" in sys.argv[1:]:
        sys.stdout.write(program())
        return
    with open(CASE, "w", encoding="utf-8") as f:
        f.write(want)


if __name__ == "__main__":
    main()
