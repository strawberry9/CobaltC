#!/usr/bin/env python3
"""What must hold before the gate spends its hours (CHG-0475): run by impl/tools/gate.py before a profile's
units, on the working tree, which must be clean (the gate runs HEAD). Each check takes seconds; together a few
minutes, against a gate's hour and a half. It stops at nothing: every check runs, and the gate does not start
when one fails.

  - Generated files are what their generators write from the committed sources: the registry inside cobc
    (impl/cobc/build_info.cb), the runtime's diagnostic table and interface (impl/cbrt/src/diag_table.h,
    impl/back/header.cb), std's prelude summary and its function values (impl/front/prelude.cb,
    impl/back/std_values.cb, as the built cobc writes them), spec/21's trusted table, CARD.md, the guide's HTML,
    and the bootstrap seed.
  - The records: speccheck.py, runtime_contract.py, cobdoc.py, std_coverage.py, conf_coverage.py.
  - The documentation keeps up (docs_coverage.py): every diagnostic in the guide's table, and each rule and
    diagnostic new since the last push in the guide or the book.

The prelude, std_values and seed checks use impl/target/release/cobc: build from the committed sources first.

    python3 impl/tools/pregate.py            # exit 1, with what to run, when anything fails
"""
import os
import subprocess
import sys
import tempfile

HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.abspath(os.path.join(HERE, "..", ".."))
COBC = os.path.join(ROOT, "impl", "target", "release", "cobc")


def sh(cmd, cwd=ROOT):
    r = subprocess.run(cmd, cwd=cwd, shell=True, capture_output=True, text=True)
    return r.returncode, (r.stdout + r.stderr).strip()


def main():
    rc, st = sh("git status --porcelain --untracked-files=no")
    if st:
        print("pregate: the tree has changes the gate would not see; commit them first:\n" + st)
        return 1
    tmp = tempfile.mkdtemp(prefix="pregate-")
    checks = [
        ("cobc's registry (impl/cobc/gen_build_info.py)", "python3 -I impl/cobc/gen_build_info.py --check"),
        ("the runtime's diagnostic table (impl/cbrt/tools/gen_diag_table.py)",
         "python3 -I impl/cbrt/tools/gen_diag_table.py spec/registry/diagnostics.md %s/diag_table.h && "
         "cmp %s/diag_table.h impl/cbrt/src/diag_table.h" % (tmp, tmp)),
        ("the runtime's interface (impl/back/gen_header.py)",
         "python3 -I impl/back/gen_header.py %s/header.cb > /dev/null && cmp %s/header.cb impl/back/header.cb" % (tmp, tmp)),
        ("std's prelude summary (cobc --prelude-summary impl/std > impl/front/prelude.cb)",
         "%s --prelude-summary impl/std | cmp - impl/front/prelude.cb" % COBC),
        ("std's function values (impl/back/gen_std_values.sh)", "impl/back/gen_std_values.sh --check"),
        ("spec/21's trusted table (impl/tools/trusted_table.py --write)", "python3 impl/tools/trusted_table.py --check"),
        ("CARD.md (impl/tools/card.py)", "python3 impl/tools/card.py --check"),
        ("the guide's HTML (cd htmlguide/generator && python3 gen.py)",
         "cd htmlguide/generator && python3 gen.py > %s/gen.log 2>&1; r=$?; cd ../..; "
         "if [ $r -ne 0 ]; then tail -3 %s/gen.log; exit 1; fi; "
         "if git status --porcelain htmlguide | grep -q .; then git status --porcelain htmlguide; "
         "git checkout -q -- htmlguide; exit 1; fi" % (tmp, tmp)),
        ("speccheck", "python3 impl/tools/speccheck.py"),
        ("the runtime contract", "python3 impl/tools/runtime_contract.py --check"),
        ("std's documentation (cobdoc)", "python3 impl/tools/cobdoc.py --check"),
        ("std's coverage by conformance cases", "python3 impl/tools/std_coverage.py"),
        ("the conformance rows' coverage", "python3 -I impl/tests/checks/conf_coverage.py"),
        ("the documentation (impl/tools/docs_coverage.py)", "python3 impl/tools/docs_coverage.py"),
        ("the bootstrap seed (impl/bootstrap/mkseed.sh)", "COBC=%s impl/bootstrap/mkseed.sh --check" % COBC),
    ]
    failed = []
    for name, cmd in checks:
        rc, out = sh(cmd)
        last = out.splitlines()[-1] if out else ""
        print("pregate: %-4s %s%s" % ("ok" if rc == 0 else "FAIL", name, "" if rc == 0 else "\n    " + out.replace("\n", "\n    ")[-1500:]))
        if rc != 0:
            failed.append(name)
    print("pregate: %d of %d checks failed%s" % (len(failed), len(checks), ": " + "; ".join(failed) if failed else ""))
    return 1 if failed else 0


if __name__ == "__main__":
    sys.exit(main())
