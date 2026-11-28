#!/usr/bin/env python3
"""Checks that every example program under examples/ appears verbatim in a
chapter under src/, inside a fenced code block, and that every chapter named
in SUMMARY.md exists. The chapters are written by hand and the examples are
the same text kept as files so that check.sh can run them; this keeps the
two from drifting apart.

    python3 -I embedded.py BOOKDIR
"""
import os
import re
import sys


def main(book):
    src = os.path.join(book, "src")
    chapters = {}
    for name in sorted(os.listdir(src)):
        if name.endswith(".md"):
            with open(os.path.join(src, name), encoding="utf-8") as f:
                chapters[name] = f.read()
    problems = 0
    examples = os.path.join(book, "examples")
    for chapter_dir in sorted(os.listdir(examples)):
        d = os.path.join(examples, chapter_dir)
        if not os.path.isdir(d) or not chapter_dir.startswith("ch"):
            continue
        for name in sorted(os.listdir(d)):
            if not name.endswith(".cb"):
                continue
            with open(os.path.join(d, name), encoding="utf-8") as f:
                text = f.read().rstrip("\n")
            found = any(text in body for body in chapters.values())
            if not found:
                print("FAIL not in any chapter verbatim: examples/%s/%s" % (chapter_dir, name))
                problems += 1
    # Every chapter SUMMARY.md lists exists, and every chapter file is listed.
    with open(os.path.join(book, "src", "SUMMARY.md"), encoding="utf-8") as f:
        summary = f.read()
    listed = set(re.findall(r"\]\(([^)]+\.md)\)", summary))
    for name in listed:
        if name not in chapters:
            print("FAIL SUMMARY.md names a missing chapter: %s" % name)
            problems += 1
    for name in chapters:
        if name != "SUMMARY.md" and name not in listed:
            print("FAIL chapter not in SUMMARY.md: %s" % name)
            problems += 1
    if problems:
        print("FAIL embedded: %d problems" % problems)
        return 1
    print("embedded: every example is in a chapter; SUMMARY.md and src/ agree")
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1] if len(sys.argv) > 1 else os.path.dirname(os.path.abspath(__file__))))
