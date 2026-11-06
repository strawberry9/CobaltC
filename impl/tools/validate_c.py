#!/usr/bin/env python3
"""Translation validation of the decided lowering (D-0299 step 5): what `cobc` asserts of its own C, checked
in the C, by a tool that shares no code with the compiler.

The certificate (`cobc --certificate`, D-0299 step 1) says which bodies the static pass decided and whether
the whole program is decided. A decided program is compiled with no record of references (D-0284), and a
decided body in any program forms light references (D-0292). Both claims are made by the lowering about
itself (`pathless`, `Items::tracking_needed`). This tool checks them in the C that `cobc` wrote, after the C
preprocessor has expanded the runtime header's macros (the form the C compiler compiles), against the
classification below of every entry point `impl/cbrt/include/cbrt.h` declares:

  V0  the verdict and the C agree: a program the certificate calls `decided` has every body decided and its
      C defines `CB_PATHLESS` (and, unless the second, decided lowering fell back to the first, D-0285,
      `CB_DECIDED`); a program whose C defines `CB_PATHLESS` is one the certificate calls decided.
  V1  a decided program's preprocessed C, the program's own part (after the header), calls no entry point
      that forms, records or reads an access path (class `path`); the entry points it does call are each
      of a known class (an unknown one is an error: a new entry point must be classified here).
  V2  in every program, each C function that implements a body the certificate calls `left` records the
      references it forms (`cb_frame_push_rec`, D-0292), or is a dispatcher that forms nothing and calls
      only its own variants (`__dir`, `__chk`, `__frz`, `__arrive`, D-0188), consulting at most the
      arguments' paths to choose one (`cb_frozen_ok`, `cb_direct_ok`, `cb_paths_disjoint`, `cb_paths_arrive`).

With `--deep`, the C is also compiled (`-O2 -g -c`) and each call that survives optimisation to an entry
point of class `owner` (a check the run still makes, against an object no recorded path can reach in a
decided program) is attributed to the source line it was written for, through the debug line table
(`objdump -d -l -r`): what a decided program still calls at run time, and where.

    python3 impl/tools/validate_c.py PATH... [--cobc PATH] [--cc gcc,clang] [--out DIR] [--deep] [--check]
    python3 impl/tools/validate_c.py --pair CERT C [--deep]   # a certificate and C written elsewhere
    python3 impl/tools/validate_c.py --classes      # the classification table, and any entry not in it

PATH is a `.cb` file or a directory of them (searched recursively). `--check` exits 1 on any finding, or on
an entry point of the header that the table does not classify. Needs a C compiler (and, for `--deep`,
`objdump`); standard library only.
"""
import os
import re
import subprocess
import sys

ROOT = os.path.dirname(os.path.dirname(os.path.dirname(os.path.abspath(__file__))))
sys.path.insert(0, os.path.join(ROOT, "impl", "tools"))
import runtime_contract  # noqa: E402

INCLUDE = os.path.join(ROOT, "impl", "cbrt", "include")

# Every entry point of cbrt.h, by what a call to it does to the record of references.
#   path   forms, records, reads or ends an access path, or checks an access against the recorded paths
#          through a path: none may remain in a decided program's preprocessed C (V1)
#   root   establishes an owner-like root path, never a derived one: `[Temp-Root]`, `[Reclaim]`
#   flight a value holding references in flight across a call (D-0285): its slots' tokens move with it; in a
#          program lowered pathless every slot's token is 0 (`cb_store_ref` is a no-op there), so nothing is
#          recorded by it; a program lowered knowing it is decided (`CB_DECIDED`) makes none of these calls
#   owner  checks a premise against the paths of the object it is given (a solitary owner, a dispatcher's
#          argument paths, a place written through its owner's root): in a decided program no derived path
#          is ever formed, so the check consults the owner's own state only
#   frame  frames, statement scopes and objects (`state.frame-stack`, `holder`, obligations)
#   value  a check or an operation on a value: bounds, overflow, a resource's state, text, numbers, I/O
#   thread threads, mutexes, atomics, channels' event counts, the schedule
#   prog   the program's start, end, faults and positions
CLASSES = {
    "path": """cb_borrow cb_borrow_i cb_borrow_range cb_borrow_unstamped cb_borrow_unstamped_i cb_borrow_check
        cb_borrow_check_i cb_borrow_unminted cb_borrow_unminted_i cb_borrow_range_unminted
        cb_borrow_range_unminted_i cb_read cb_read_i cb_write_i cb_valid cb_store_ref cb_hold_ref cb_rec_ref
        cb_load_ref cb_copy_datum cb_frame_hold cb_frame_unhold cb_frame_push_rec cb_elem_borrow
        cb_elem_borrow_here cb_elem_borrow_datum cb_call_self cb_call_done cb_frozen_ok
        cb_point_check cb_lock cb_vouched cb_marks cb_inline_off""",
    "root": "cb_temp_path cb_reclaim",
    "flight": "cb_send_datum cb_recv_datum cb_recv_datum_tok cb_send_tok_datum cb_send_datum_none cb_peek_datum_tok",
    "owner": """cb_write cb_write_pathless cb_write_quiet cb_take cb_destroy cb_destroy_via cb_lend cb_lend_fn
        cb_direct_ok cb_paths_disjoint cb_paths_arrive cb_elem_access cb_elem_access_if cb_elem_access_slow
        cb_live_in cb_live_in2 cb_live_at cb_reclaimed_near cb_reclaimed_entries cb_reclaimed_pages
        cb_fn_shared cb_fn_shared_value cb_fn_read cb_fn_copy cb_fn_split cb_owns cb_rebind cb_end_binding""",
    "frame": """cb_frame_push cb_frame_push_fn cb_frame_push_at cb_frame_pop cb_frame_here cb_stmt_push
        cb_stmt_push_at cb_stmt_pop cb_new cb_new_uninit cb_bind cb_move_to cb_absorb cb_absorb_refs cb_result
        cb_result_ref cb_consume cb_end_moved_out cb_send cb_recv cb_send_ref cb_recv_ref cb_forget_obj
        cb_drop_local cb_drop_plain_buf cb_drop_value cb_drop_fn cb_notes_push cb_notes_pop cb_fn_item
        cb_fn_item_pure cb_fn_pure cb_fn_mark_pure cb_fn_box cb_fn_moved cb_raw_move_in cb_raw_move_out
        cb_release cb_release_plain cb_allocate cb_allocate_at_once cb_alloc_traced cb_deallocate
        cb_deallocate_plain cb_reallocate_plain
        cb_copy_raw cb_vec_drop_plain""",
    "value": """cb_index_check cb_str_eq cb_str_string cb_bytes_eq cb_bytes_cmp cb_bytes_less cb_bytes_lt cb_hash_bytes
        cb_fnv_more cb_find_bytes cb_char_boundary cb_round cb_roundf cb_popcount64 cb_popcount128 cb_clz
        cb_clz128 cb_ctz cb_ctz128 cb_rotl cb_rotr cb_rotl128 cb_rotr128 cb_fmt_space cb_fmt_u64 cb_fmt_i64
        cb_fmt_hex cb_fmt_pad cb_format cb_print_int cb_print_f64 cb_print_f32 cb_print_bool cb_text_int
        cb_text_f64 cb_text_f32 cb_text_bool cb_parse_int cb_parse_float cb_parse_bool cb_sort_plain
        cb_sort_text cb_write_out cb_write_err cb_flush_out cb_read_in cb_read_line_in cb_arg_bytes
        cb_file_read cb_file_write cb_file_op cb_file_at cb_fs_op cb_fs_query cb_clock_read cb_tz_offset
        cb_sleep_ns cb_path_op cb_proc_op cb_net_op cb_os_random""",
    "thread": """cb_spawn cb_env_alloc cb_env_forget cb_hold cb_thread_start cb_thread_done cb_thread_failed
        cb_thread_jmp cb_thread_lent cb_join cb_join_status cb_cancel cb_task_op cb_lock_pathless cb_lock_bare
        cb_unlock_bare cb_unlock_signal cb_event_op cb_atomic_point cb_atomic_wait cb_spin_hint cb_live_threads
        cb_threaded cb_sched_flag""",
    "prog": """cb_init cb_fault cb_fault_msg cb_exit cb_terminate_ok cb_set_args cb_run_main cb_run_program cb_stack_check
        cb_stack_exhausted cb_stack_floor cb_set_stack_floor cb_at cb_at_pos cb_where cb_set_where cb_lib_enter
        cb_lib_leave cb_lib_shutdown cb_lib_jmp cb_lib_fault_code cb_lib_last_fault""",
}
CLASS_OF = {}
for _k, _v in CLASSES.items():
    for _n in _v.split():
        CLASS_OF[_n] = _k

# What a dispatcher may consult to choose among its variants: it reads the arguments' paths and forms
# none (D-0188, D-0205, D-0219).
DISPATCH = {"cb_frozen_ok", "cb_direct_ok", "cb_paths_disjoint", "cb_paths_arrive", "cb_peek_datum_tok", "cb_live_threads"}

# The C variants of one body (D-0188 dual bodies, D-0205 frozen parameters, D-0219 arrivals).
VARIANT = re.compile(r"^__(dir|chk|frz|arrive)$")
BOUNDARY = "/* ---- types ---- */"
SENTINEL = "typedef int cb_tv_boundary_;"


HEADER = set(n for n, *_ in runtime_contract.declarations())


def unclassified():
    names = sorted(HEADER)
    return [n for n in names if n not in CLASS_OF]


# The C name of a body (`cobc`'s `fn_cname` and `mangle`, read from the certificate's key).
def split_args(s):
    out, depth, cur = [], 0, ""
    for ch in s:
        if ch in "<(":
            depth += 1
        elif ch in ">)":
            depth -= 1
        if ch == "," and depth == 0:
            out.append(cur.strip())
            cur = ""
        else:
            cur += ch
    if cur.strip():
        out.append(cur.strip())
    return out


def san(n):
    return n.replace("::", "__")


def mangle(t):
    t = t.strip()
    m = re.match(r"^(ref|slice)<(.*)>$", t)
    if m:
        a = split_args(m.group(2))
        return "%s%s_%s" % (m.group(1), "s" if a[1] == "shared" else "x", mangle(a[0]))
    m = re.match(r"^(rawptr|handle|Mutex|guard)<(.*)>$", t)
    if m:
        return "%s_%s" % ({"rawptr": "ptr", "handle": "handle", "Mutex": "mutex", "guard": "guard"}[m.group(1)], mangle(m.group(2)))
    m = re.match(r"^(array|simd)<(.*)>$", t)
    if m:
        a = split_args(m.group(2))
        return "%s%s_%s" % ("arr" if m.group(1) == "array" else "simd", a[1], mangle(a[0]))
    m = re.match(r"^([A-Za-z_][A-Za-z0-9_:]*)<(.*)>$", t)
    if m:
        return "%s_L%s_R" % (san(m.group(1)), "_".join(mangle(x) for x in split_args(m.group(2))))
    return san(t)


def cname(key):
    m = re.match(r"^([^<]*)(?:<(.*)>)?$", key)
    c = "f_" + san(m.group(1))
    if m.group(2):
        c += "_L" + "".join(mangle(t) + "_" for t in split_args(m.group(2))) + "R"
    return c


def unqualified(c):
    """A C name with the module paths of its type arguments dropped: a certificate's key writes a type
    as `Type::to_string` does (`Option<…>`), the C name as `mangle` does (`std__Option_L…_R`)."""
    k = c.find("_L")
    if k < 0:
        return c
    return c[:k] + re.sub(r"(?<=[L_])(?:[a-z][a-z0-9]*__)+", "", c[k:])


def read_cert(path):
    verdict, bodies = None, {}
    for line in open(path, encoding="utf-8"):
        line = line.rstrip("\n")
        if line.startswith("verdict: "):
            verdict = line[len("verdict: "):]
        m = re.match(r"^fn (.*): (decided|left|rejected)$", line)
        if m:
            bodies[m.group(1)] = m.group(2)
    return verdict, bodies


def c_functions(text):
    """The program's C functions: name -> body lines (cobc writes `type name(params)` then `{` alone)."""
    lines = text.split("\n")
    fns, cur = {}, None
    for i, line in enumerate(lines):
        if cur is None:
            m = re.match(r"^(?:static )?(?:inline )?(?:__attribute__\(\(\w+\)\) )?[A-Za-z_][\w ]*?[ *]+(f_\w+)\(.*\)$", line)
            if m and i + 1 < len(lines) and lines[i + 1] == "{":
                cur = m.group(1)
                fns[cur] = []
            continue
        if line == "}":
            cur = None
        else:
            fns[cur].append(line)
    return fns


CALL = re.compile(r"\b(cb_[a-z0-9_]+)\s*\(")
NAME = re.compile(r"\b(cb_[a-z0-9_]+)\b")


def preprocess(cc, c_text, scratch):
    """The program's own part of the C after the preprocessor, or None when it fails."""
    if BOUNDARY not in c_text:
        return None
    marked = c_text.replace(BOUNDARY, SENTINEL + "\n" + BOUNDARY, 1)
    src = scratch + ".m.c"
    with open(src, "w", encoding="utf-8") as f:
        f.write(marked)
    r = subprocess.run([cc, "-E", "-P", "-I", INCLUDE, src], capture_output=True, text=True)
    if r.returncode != 0:
        return None
    out = r.stdout
    k = out.find(SENTINEL)
    return None if k < 0 else out[k + len(SENTINEL):]


def deep_calls(cc, c_path, scratch):
    """Calls to `owner` entry points that survive `-O2`, each attributed to the program's source line it
    was written for (the last `.cb` line the debug line table names before it in its function; a call the
    runtime header's inline code makes is attributed there too, through `cobc`'s `#line` directives)."""
    obj = scratch + ".o"
    # DWARF 4: the line tables binutils of every supported host reads.
    r = subprocess.run([cc, "-O2", "-g", "-gdwarf-4", "-w", "-c", "-I", INCLUDE, "-o", obj, c_path], capture_output=True, text=True)
    if r.returncode != 0:
        return None
    d = subprocess.run(["objdump", "-d", "-l", "-r", "--no-show-raw-insn", obj], capture_output=True, text=True).stdout
    loc, src, res = "?", None, set()
    for line in d.split("\n"):
        if re.match(r"^[0-9a-f]+ <.+>:$", line):
            loc, src = "?", None
            continue
        m = re.match(r"^(\S.*):(\d+)(?: \(discriminator \d+\))?$", line)
        if m:
            loc = "%s:%s" % (os.path.basename(m.group(1)), m.group(2))
            if m.group(1).endswith(".cb"):
                src = loc
            continue
        m = re.search(r"R_X86_64_\w+\s+(cb_\w+)", line)
        if m and CLASS_OF.get(m.group(1)) == "owner":
            res.add((m.group(1), src or loc))
    return sorted(res)


def validate(prog, cobc, ccs, out, deep):
    """(status, findings, notes) for one program; status in decided/undecided/rejected/error."""
    base = os.path.join(out, re.sub(r"[^A-Za-z0-9_.-]", "_", os.path.relpath(os.path.abspath(prog), ROOT))[:-3])
    cert, c_path = base + ".cert", base + ".c"
    for p in (cert, c_path):
        if os.path.exists(p):
            os.remove(p)
    d = os.path.dirname(os.path.abspath(prog))
    try:
        r = subprocess.run([cobc, "--certificate", cert, "--emit-c", "-o", c_path, os.path.basename(prog)], cwd=d, capture_output=True, text=True, timeout=300)
    except subprocess.TimeoutExpired:
        return "error", ["cobc timed out"], []
    if "is a library" in r.stderr:
        return "library", [], []
    if not os.path.exists(cert):
        return "error", ["no certificate written: " + (r.stderr or r.stdout).strip()[:200]], []
    if r.returncode != 0 or not os.path.exists(c_path):
        verdict, _ = read_cert(cert)
        if verdict and verdict.startswith("rejected"):
            return "rejected", [], []
        return "error", ["cobc failed: " + (r.stderr or r.stdout).strip()[:200]], []
    return check_pair(cert, c_path, ccs, base, deep)


def check_pair(cert, c_path, ccs, base, deep):
    """V0-V2 (and `--deep`) on a certificate and the C written with it."""
    verdict, bodies = read_cert(cert)
    if verdict and verdict.startswith("rejected"):
        return "rejected", [], []
    text = open(c_path, encoding="utf-8").read()
    head = text.split("\n", 2)[:2]
    pathless = head[:1] == ["#define CB_PATHLESS 1"]
    decided_c = pathless and head[1:2] == ["#define CB_DECIDED 1"]
    findings, notes = [], []
    decided = verdict == "decided"
    # V0
    if decided:
        left = [k for k, v in bodies.items() if v != "decided"]
        if left:
            findings.append("V0: the verdict is `decided` but the certificate lists bodies not decided: %s" % ", ".join(left[:5]))
        if not pathless:
            findings.append("V0: the verdict is `decided` but the C does not define CB_PATHLESS")
        elif not decided_c:
            notes.append("V0: lowered pathless by the first lowering (no CB_DECIDED, D-0285)")
    elif pathless:
        findings.append("V0: the C defines CB_PATHLESS but the verdict is `%s`" % verdict)
    # V1
    if decided:
        for cc in ccs:
            part = preprocess(cc, text, base + "." + os.path.basename(cc))
            if part is None:
                findings.append("V1: %s could not preprocess the C" % cc)
                continue
            # A name the header does not declare is the program's own (a thunk, a thread's trampoline,
            # `cb_program`, `cb_main_fn`): the C compiler would refuse a call to an undeclared one.
            # Every mention counts, not only a call: `(cb_borrow)(…)` or `&cb_borrow` reaches it too.
            seen = {}
            for m in NAME.finditer(part):
                n = m.group(1)
                if n not in HEADER:
                    continue
                seen[n] = seen.get(n, 0) + 1
            for n in sorted(seen):
                k = CLASS_OF.get(n)
                if k is None:
                    findings.append("V1 (%s): `%s` is called and is no entry point the table classifies" % (cc, n))
                elif k == "path":
                    findings.append("V1 (%s): `%s` (forms, records or reads an access path) is named %d time(s)" % (cc, n, seen[n]))
            notes.append("V1 (%s): %d entry point(s) called: %s" % (cc, len(seen), ", ".join("%s %s" % (n, CLASS_OF.get(n, "?")) for n in sorted(seen) if CLASS_OF.get(n) in ("owner", "root"))))
    # V2
    fns = c_functions(text)
    for key, st in bodies.items():
        if st != "left":
            continue
        c = cname(key)
        variants = [f for f in fns if unqualified(f) == unqualified(c) or (unqualified(f).startswith(unqualified(c) + "__") and VARIANT.match(unqualified(f)[len(unqualified(c)):]))]
        if not variants:
            notes.append("V2: no C function found for `%s` (%s)" % (key, c))
            continue
        for f in variants:
            body = fns[f]
            pushes = [re.search(r"cb_frame_push\w*", l).group(0) for l in body if re.search(r"\bcb_frame_push\w*\s*\(", l)]
            if pushes and pushes[0] == "cb_frame_push_rec":
                continue
            calls = set(CALL.findall("\n".join(body))) | set(re.findall(r"\b(f_\w+)\s*\(", "\n".join(body)))
            own_variants = {v for v in variants if v != f}
            forms = [n for n in calls if CLASS_OF.get(n) in ("path", "root") and n not in DISPATCH]
            others = [n for n in calls if n.startswith("f_") and n not in own_variants]
            if not pushes and not forms and not others:
                continue  # a dispatcher to its own variants
            findings.append("V2: `%s` implements `%s`, which the certificate leaves to the run, and does not record its references (first frame push: %s)" % (f, key, pushes[0] if pushes else "none"))
    if deep and decided:
        for cc in ccs:
            calls = deep_calls(cc, c_path, base + "." + os.path.basename(cc))
            if calls is None:
                notes.append("deep (%s): did not compile" % cc)
            else:
                notes.append("deep (%s): %d owner check(s) survive -O2: %s" % (cc, len(calls), ", ".join("%s at %s" % c for c in calls)))
    return ("decided" if decided else "undecided"), findings, notes


def programs(paths):
    for p in paths:
        if os.path.isdir(p):
            for d, _, fs in sorted(os.walk(p)):
                for f in sorted(fs):
                    if f.endswith(".cb"):
                        yield os.path.join(d, f)
        else:
            yield p


def main():
    args = sys.argv[1:]
    if args == ["--classes"]:
        for k, v in CLASSES.items():
            print("%-6s %s" % (k, " ".join(v.split())))
        u = unclassified()
        print("unclassified: %s" % (" ".join(u) if u else "none"))
        return 1 if u else 0
    cobc = os.path.join(ROOT, "impl", "target", "release", "cobc")
    ccs = ["gcc", "clang"]
    if args[:1] == ["--pair"] and len(args) >= 3:
        cert, c_path = os.path.abspath(args[1]), os.path.abspath(args[2])
        st, findings, notes = check_pair(cert, c_path, ccs, c_path[:-2] + ".tv", "--deep" in args[3:])
        for f in findings + notes:
            print("validate_c: %s: %s" % (args[2], f))
        print("validate_c: %s: %s, %d finding(s)" % (args[2], st, len(findings)))
        return 1 if findings else 0
    out = None
    deep = check = verbose = False
    paths = []
    i = 0
    while i < len(args):
        a = args[i]
        if a == "--cobc":
            cobc = os.path.abspath(args[i + 1])
            i += 2
        elif a == "--cc":
            ccs = [c for c in args[i + 1].split(",") if c]
            i += 2
        elif a == "--out":
            out = os.path.abspath(args[i + 1])
            i += 2
        elif a == "--deep":
            deep = True
            i += 1
        elif a == "--check":
            check = True
            i += 1
        elif a in ("-v", "--verbose"):
            verbose = True
            i += 1
        else:
            paths.append(a)
            i += 1
    if not paths:
        print(__doc__)
        return 2
    if out is None:
        out = os.path.join(ROOT, "stress", "verify", "validate_c")
    os.makedirs(out, exist_ok=True)
    bad = 0
    u = unclassified()
    if u:
        print("validate_c: cbrt.h declares entry points the table does not classify: %s" % " ".join(u))
        bad += 1
    tally = {}
    nfind = 0
    for prog in programs(paths):
        st, findings, notes = validate(prog, cobc, ccs, out, deep)
        tally[st] = tally.get(st, 0) + 1
        for f in findings:
            print("validate_c: %s: %s" % (prog, f))
        if findings:
            nfind += 1
        if verbose or deep:
            for n in notes:
                print("validate_c: %s: %s" % (prog, n))
    print("validate_c: %d program(s): %d decided, %d undecided, %d rejected, %d libraries, %d error; %d with a finding"
          % (sum(tally.values()), tally.get("decided", 0), tally.get("undecided", 0), tally.get("rejected", 0), tally.get("library", 0), tally.get("error", 0), nfind))
    if check and (nfind or bad or tally.get("error", 0)):
        return 1
    return 0


if __name__ == "__main__":
    sys.exit(main())
