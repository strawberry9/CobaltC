#!/usr/bin/env python3
"""Runs the conformance cases under impl/conformance against any implementation.

    python3 impl/conformance/run.py --impl "coby {file}" --no-c-code
    python3 impl/conformance/run.py --impl "cobc --run {file}"
    python3 impl/conformance/run.py --impl "./mycc --run {file}" --jobs 4

Python 3 standard library only. Each case is a .cb file whose leading
`//` comment lines are its header (impl/conformance/README.md):

    kind:              positive or negative
    expect:            ok | exit N | diag.ID (static) | diag.ID (dynamic)
    expect-stdout-hex: the bytes the program writes, in hex
    stdin-hex:         its standard input, in hex (none: an empty input)
    args:              its arguments; `$TMP` is a fresh, empty directory,
                       `""` an empty argument, `\\xHH` a byte
    cobc-only:         it links the program's own C code
    platform:          unix, linux or windows: run only there
    slow:              run only with --slow

A file whose first line is `// CONFORMANCE-FIXTURE` is a module another
case loads, not a case.

The implementation is run in the case's own directory, as the command
template with `{file}` replaced by the case's path, then the case's
arguments; it is killed after --timeout seconds. Its outcome is:

    ok           exit status 0
    exit N       a non-zero status N with nothing on stderr
    diag.ID (P)  stderr's first line begins `error[diag.ID]` (P static)
                 or `fault[diag.ID]` (P dynamic), the form
                 spec/registry/diagnostics.md gives
    unsupported  exit status 3 with a first line beginning `unsupported:`

A case passes when its outcome is its `expect:` and, with
`expect-stdout-hex:`, its output is those bytes. A cobc-only case run with
--no-c-code (an implementation that does not link a program's own C) must
be refused as unsupported. An unsupported outcome otherwise fails the
case, unless --allow-unsupported is given; it is then counted apart.
Off Linux, coby's refusal to call C (`calls C functions only on x86-64
Linux`) skips the case, as the Rust runners do.

Exit status 0 when every case run passed, 1 when one did not, 2 for a
usage error. --list prints each case's result, for comparing the pass
sets of two implementations.
"""

import argparse
import concurrent.futures
import os
import shlex
import shutil
import subprocess
import sys
import tempfile
import time

FIELDS = ("spec", "facility", "kind", "expect", "expect-stdout-hex", "stdin-hex", "cobc-only", "args", "platform", "slow")


def header(src):
    """The header fields of a case: its leading `//` lines, `key: value`."""
    fields = {}
    for line in src.split("\n"):
        line = line.lstrip()
        if not line.startswith("//"):
            break
        body = line.lstrip("/").strip()
        key, sep, val = body.partition(":")
        if sep and key.strip() in FIELDS and key.strip() not in fields:
            fields[key.strip()] = val.strip()
    return fields


# The platform the programs run on: this machine's, unless --platform
# names another (D-0255: `cobc --target x86_64-pc-windows-gnu` with the
# programs run under Wine is `--platform windows`).
PLATFORM = None


def split_command(template):
    """The --impl command as a list of arguments. On Windows a backslash is a
    path separator, not an escape (shlex's POSIX rules dropped every one of
    them from `C:\\...\\cobc.exe`): words are split on spaces, and double
    quotes group words with spaces."""
    if os.name != "nt":
        return shlex.split(template)
    return [w[1:-1] if len(w) >= 2 and w[0] == w[-1] == '"' else w for w in shlex.split(template, posix=False)]


def platform_target():
    return PLATFORM or ("windows" if os.name == "nt" else "linux" if sys.platform.startswith("linux") else "unix")


def platform_ok(p):
    if p is None:
        return True
    target = platform_target()
    if p == "unix":
        return target in ("unix", "linux")
    if p == "linux":
        return target == "linux"
    if p == "windows":
        return target == "windows"
    raise SystemExit("platform: unknown platform `%s`" % p)


def case_args(h, tmp):
    """The program's arguments as bytes (`args:`), `$TMP` replaced by `tmp`."""
    out = []
    for a in h.split():
        if a == '""':
            out.append(b"")
            continue
        if "$TMP" in a:
            out.append(a.replace("$TMP", tmp).encode())
            continue
        b = bytearray()
        i = 0
        while i < len(a):
            if a[i] == "\\" and a[i + 1:i + 2] == "x" and i + 4 <= len(a):
                b.append(int(a[i + 2:i + 4], 16))
                i += 4
            else:
                b.extend(a[i].encode())
                i += 1
        out.append(bytes(b))
    return out


# The refusal of an implementation that calls C only on some platforms:
# elsewhere a case that calls C is skipped, as the Rust runners skip it
# (`tests/common/mod.rs` COBY_CALLS_C); on Linux it is a failure.
NO_C_CALLS = "unsupported (no C calls on this platform)"


def outcome(status, stdout, stderr):
    """The outcome vocabulary of the `expect:` header (module doc)."""
    if status == 0:
        return "ok"
    if not stderr:
        return "exit %d" % status
    first = stderr.decode("utf-8", "replace").split("\n", 1)[0]
    if status == 3 and first.startswith("unsupported:"):
        # coby calls C functions only on x86-64 Linux (`src/ffi.rs`).
        if "calls C functions only on" in first:
            return NO_C_CALLS
        return "unsupported"
    for prefix, phase in (("error[", "static"), ("fault[", "dynamic")):
        if first.startswith(prefix) and "]" in first:
            diag = first[len(prefix):first.index("]")]
            if diag.startswith("diag."):
                return "%s (%s)" % (diag, phase)
    return "unrecognised (exit %d): %s" % (status, first[:200])


def run_case(path, h, template, timeout):
    """Runs one case; (outcome, stdout)."""
    tmp = tempfile.mkdtemp(prefix="cobaltc-conf-") if "$TMP" in h.get("args", "") else ""
    try:
        argv = [a.replace("{file}", path) for a in split_command(template)]
        if not any("{file}" in a for a in split_command(template)):
            argv.append(path)
        if os.name == "posix":
            argv = [os.fsencode(a) for a in argv] + case_args(h.get("args", ""), tmp)
        else:
            argv = argv + [a.decode("utf-8") for a in case_args(h.get("args", ""), tmp)]
        stdin = bytes.fromhex(h["stdin-hex"]) if "stdin-hex" in h else b""
        # A process group of its own (POSIX), so a timed-out run's
        # children (`cobc --run`'s program) are killed with it.
        extra = {"start_new_session": True} if os.name == "posix" else {}
        try:
            p = subprocess.Popen(argv, cwd=os.path.dirname(path), stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=subprocess.PIPE, **extra)
        except OSError as e:
            return ("cannot run the implementation: %s" % e, b"")
        try:
            out, err = p.communicate(stdin, timeout=timeout)
        except subprocess.TimeoutExpired:
            if os.name == "posix":
                os.killpg(p.pid, 9)
            else:
                p.kill()
            out, _ = p.communicate()
            return ("timed out after %d s" % timeout, out)
        return (outcome(p.returncode, out, err), out)
    finally:
        if tmp:
            shutil.rmtree(tmp, ignore_errors=True)


def main():
    here = os.path.dirname(os.path.abspath(__file__))
    ap = argparse.ArgumentParser(description="Run the CobaltC conformance cases against an implementation.")
    ap.add_argument("--impl", required=True, help='the command, with {file} for the case\'s path, e.g. "coby {file}"')
    ap.add_argument("--root", default=here, help="the directory of cases (default: this script's)")
    ap.add_argument("--no-c-code", action="store_true", help="the implementation does not link a program's own C code: cobc-only cases must be refused")
    ap.add_argument("--allow-unsupported", action="store_true", help="count an `unsupported:` refusal apart instead of failing the case")
    ap.add_argument("--slow", action="store_true", help="also run the cases with a `slow:` header")
    ap.add_argument("--only", default="", help="only the cases whose path contains this text")
    ap.add_argument("--jobs", type=int, default=os.cpu_count() or 1, help="cases run at a time (default: every CPU)")
    ap.add_argument("--timeout", type=int, default=600, help="seconds a case may run (default 600)")
    ap.add_argument("--list", action="store_true", help="print every case's result")
    ap.add_argument("--platform", choices=("linux", "unix", "windows"), help="the platform the programs run on, when not this machine's (a cross-compiled build run under an emulator)")
    a = ap.parse_args()
    global PLATFORM
    PLATFORM = a.platform

    files = []
    for d, dirs, names in os.walk(a.root):
        dirs.sort()
        for n in sorted(names):
            if n.endswith(".cb"):
                files.append(os.path.join(d, n))
    files.sort()

    cases, fixtures, skipped = [], 0, []
    for path in files:
        if a.only and a.only not in path:
            continue
        with open(path, encoding="utf-8") as f:
            src = f.read()
        if src.split("\n", 1)[0].strip() == "// CONFORMANCE-FIXTURE":
            fixtures += 1
            continue
        h = header(src)
        for k in ("kind", "expect"):
            if k not in h:
                raise SystemExit("%s: missing required header field `%s:`" % (path, k))
        if not platform_ok(h.get("platform")):
            skipped.append((path, "its platform: header names another platform"))
            continue
        if "slow" in h and not a.slow:
            skipped.append((path, "slow (%s); --slow runs it" % h["slow"]))
            continue
        cases.append((path, h))

    failures, results, unsupported = [], {}, []
    counts = {"positive": 0, "negative": 0}
    started = time.time()

    def one(item):
        path, h = item
        return item, run_case(path, h, a.impl, a.timeout)

    with concurrent.futures.ThreadPoolExecutor(max_workers=max(1, a.jobs)) as pool:
        for (path, h), (got, stdout) in pool.map(one, cases):
            rel = os.path.relpath(path, a.root)
            kind = h["kind"]
            if kind not in counts:
                failures.append("%s: unknown kind `%s` (must be `positive` or `negative`)" % (rel, kind))
                continue
            if got == NO_C_CALLS and platform_target() != "linux" and "cobc-only" not in h:
                skipped.append((path, "the implementation calls C functions on Linux only"))
                continue
            counts[kind] += 1
            if got == NO_C_CALLS:
                got = "unsupported"
            if "cobc-only" in h and a.no_c_code:
                ok = got == "unsupported"
                results[rel] = "pass (refused, cobc-only)" if ok else "FAIL"
                if not ok:
                    failures.append("%s: cobc-only, so the implementation must refuse it (unsupported); observed `%s`" % (rel, got))
                continue
            if got == "unsupported" and a.allow_unsupported:
                unsupported.append(rel)
                results[rel] = "unsupported"
                continue
            if got != h["expect"]:
                failures.append("%s: expected `%s` but observed `%s`" % (rel, h["expect"], got))
                results[rel] = "FAIL"
                continue
            want = h.get("expect-stdout-hex")
            if want is not None and stdout.hex() != want:
                failures.append("%s: expect-stdout-hex mismatch: expected %s but observed %s" % (rel, want, stdout.hex()))
                results[rel] = "FAIL"
                continue
            results[rel] = "pass"

    if a.list:
        for rel in sorted(results):
            print("%-28s %s" % (results[rel], rel))
        for path, why in skipped:
            print("%-28s %s (%s)" % ("skipped", os.path.relpath(path, a.root), why))
    n = counts["positive"] + counts["negative"]
    print("conformance: %d cases (%d positive, %d negative), %d fixtures, %d skipped%s; %d passed, %d failed, in %.0f s"
          % (n, counts["positive"], counts["negative"], fixtures, len(skipped),
             (", %d unsupported" % len(unsupported)) if unsupported else "",
             n - len(failures) - len(unsupported), len(failures), time.time() - started))
    if failures:
        print("%d of %d case(s) failed:" % (len(failures), n))
        for f in failures:
            print("  " + f)
        return 1
    return 0


if __name__ == "__main__":
    sys.exit(main())
