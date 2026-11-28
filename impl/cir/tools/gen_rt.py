#!/usr/bin/env python3
"""Writes impl/cir/rt.cb, the runtime-entry table (T4 stage C), from the sources it ports:

  the `cb_...` names the lowering writes in its C (the string literals of impl/back/*.cb, header.cb aside, and the
  entries its constructors name by id, `RT_...`), and
  the name lists the passes ask today: passes.cb's FREE_NAMES, SCOPE_NAMES, DECIDED_FREE_NAMES, ZERO_DROP,
  IMPLIED_PURE, LEAF_NAMES, LOCATION_PURE and FRAME_USE_NAMES, the names `reads_location` gives a location of
  their own, and gen.cb's `quiet_cells` makers.

  impl/cir/tools/gen_rt.py            (run from the repository's root; rewrites impl/cir/rt.cb)

The lists stay where they are: the passes' name sets are made of them (`passes::scope_name_sets`,
`passes::name_set`), and nothing but this script keeps the table in step with them.
"""
import glob, pathlib, re

root = pathlib.Path(__file__).resolve().parents[3]
passes = (root / "impl/back/passes.cb").read_text()


def name_list(const):
    m = re.search(r"const str " + const + r" =\s*((?:\"[^\"]*\"\s*)+);", passes)
    assert m, const
    text = "".join(re.findall(r"\"([^\"]*)\"", m.group(1)))
    return [w.rstrip("(") for w in text.split()]


OWN_LOC = ("cb_fault cb_fault_index cb_fault_range cb_fault_arith cb_fault_arith_r cb_fault_arith_c cb_fault_op "
           "cb_fault_narrow cb_fault_to_int cb_index_check cb_elem_access cb_elem_access_if").split()
MAKERS = "cb_reclaim cb_raw_move_in cb_spawn cb_lib_enter cb_entries_on".split()
MAKERS_KEPT = "cb_elem_borrow cb_elem_borrow_here cb_elem_borrow_datum".split()
PATH_CHECKS = "cb_read cb_borrow_unminted cb_borrow_check".split()
FRAME_LINES = "cb_frame_push cb_frame_pop cb_frame_push_fn cb_frame_push_at".split()
SCOPE_LINES = "cb_frame_push cb_frame_push_fn cb_frame_push_rec cb_frame_push_at cb_frame_pop cb_stmt_push cb_stmt_push_at cb_stmt_pop".split()
FLIGHT = "cb_send cb_recv".split()
NOT_ENTRIES = {"cb_lib_init", "cb_main_fn", "cb_program"}

props = [
    ("RP_FREE", "records nothing in a scope or a frame (`passes::FREE_NAMES`)", name_list("FREE_NAMES")),
    ("RP_SCOPE", "records in the innermost statement scope (`passes::SCOPE_NAMES`)", name_list("SCOPE_NAMES")),
    ("RP_DECIDED_FREE", "records nothing in a decided program (`passes::DECIDED_FREE_NAMES`)",
     name_list("DECIDED_FREE_NAMES")),
    ("RP_ZERO_DROP", "is a check that goes when its token is known 0 (`passes::ZERO_DROP`)", name_list("ZERO_DROP")),
    ("RP_IMPLIED_PURE", "leaves what `elide_implied_checks_nodes` knows as it is (`passes::IMPLIED_PURE`)",
     name_list("IMPLIED_PURE")),
    ("RP_LEAF", "cannot call back into the program (`passes::LEAF_NAMES`)", name_list("LEAF_NAMES")),
    ("RP_LOC_PURE", "reads no location (`passes::LOCATION_PURE`)", name_list("LOCATION_PURE")),
    ("RP_OWN_LOC", "carries its own location as its last arguments (`passes::reads_location`)", OWN_LOC),
    ("RP_FRAME_USE", "binds into, stamps or ends something in a frame (`passes::FRAME_USE_NAMES`)",
     name_list("FRAME_USE_NAMES")),
    ("RP_MAKER", "can put an object over raw cells or start a thread (`gen::quiet_cells`)", MAKERS),
    ("RP_MAKER_KEPT", "is an element's borrow, a maker where paths are kept (`gen::quiet_cells`)", MAKERS_KEPT),
    ("RP_PATH_CHECK", "is a path check: `cb_read`, `cb_borrow_unminted`, `cb_borrow_check`", PATH_CHECKS),
    ("RP_FRAME_LINE", "is a frame's push or pop as `elide_empty_frames_nodes` takes it out", FRAME_LINES),
    ("RP_SCOPE_LINE", "is a push or pop `resolve_scopes_nodes` writes for a scope marker", SCOPE_LINES),
    ("RP_FLIGHT", "puts a value in flight or takes one (`pprog::ground_native_nodes`)", FLIGHT),
]

names = set()
for p in glob.glob(str(root / "impl/back/*.cb")):
    if p.endswith("header.cb"):
        continue
    for lit in re.findall(r"\"((?:[^\"\\]|\\.)*)\"", pathlib.Path(p).read_text()):
        names.update(re.findall(r"\b(cb_[a-z0-9_]+)\(", lit))
# An entry a constructor names by its id (`em_rt…(x, RT_NEW, …)`) is written in no literal.
for p in glob.glob(str(root / "impl/back/*.cb")) + glob.glob(str(root / "impl/cir/*.cb")):
    if p.endswith("/rt.cb"):
        continue
    names.update("cb_" + n.lower() for n in re.findall(r"\bRT_([A-Z0-9_]+)\b", pathlib.Path(p).read_text()))
names -= {"cb_count", "cb_none"}
for _, _, ns in props:
    names.update(n for n in ns if n.startswith("cb_"))
names -= NOT_ENTRIES
names = sorted(names)

out = []
w = out.append
w("// Protocol CIR (step T4, stage C): the runtime-entry table. One row for each")
w("// `cb_…` name the lowering writes in its C or a pass asks about, in the order")
w("// of the names' bytes; a row's id is its place in that order from 1 (0: no")
w("// runtime entry), and its properties are what the passes ask today by name")
w("// lists and string tests, one bit each. A call node carries its callee's id")
w("// (`PNode::rt`): set by the constructor that built it, or looked up by")
w("// `classify` (`rt_lookup`).")
w("//")
w("// Written by `impl/cir/tools/gen_rt.py` from `passes.cb`'s lists, `gen.cb`'s")
w("// `quiet_cells` and the lowering's string literals: change those, then run")
w("// it again (the passes' name sets are made of the same lists).")
w("")
w("import std;")
w("")
for k, (c, doc, _) in enumerate(props):
    w(f"// The entry {doc}.")
    w(f"export const u32 {c} = {1 << k};")
w("")
w("// No runtime entry.")
w("export const u16 RT_NONE = 0;")
for i, n in enumerate(names):
    w(f"export const u16 RT_{n[3:].upper()} = {i + 1};")
w("")
w(f"export const usize RT_COUNT = {len(names)};")
w("")
w(f"const array<str, {len(names)}> RT_NAMES = [")
for n in names:
    w(f"    \"{n}\",")
w("];")
w("")
w(f"const array<u32, {len(names)}> RT_PROPS = [")
for n in names:
    bits = 0
    for k, (_, _, ns) in enumerate(props):
        if n in ns:
            bits |= 1 << k
    w(f"    {bits},")
w("];")
w("")
w("""// The entry's name as C writes it (`id` from 1).
export fn rt_name(u16 id) : str
{
    RT_NAMES[widen<usize>(id) - 1]
}

// The entry's properties, a set of `RP_…` bits; none for `RT_NONE`.
export fn rt_props(u16 id) : u32
{
    if (id == 0)
    {
        return 0;
    }
    RT_PROPS[widen<usize>(id) - 1]
}

// Whether the entry `id` has the property `prop`.
export fn rt_is(u16 id, u32 prop) : bool
{
    (rt_props(id) & prop) != 0
}

// How `a` stands to the name `b` in the order of their bytes: below (0), the
// same (1), above (2).
fn name_order(slice<u8, shared> a, str b) : u8
{
    slice<u8, shared> bb = StringView::as_bytes(b);
    usize n = slice_len(a);
    usize m = slice_len(bb);
    usize k = 0;
    while (k < n && k < m)
    {
        if (a[k] != bb[k])
        {
            return if (a[k] < bb[k]) { 0 } else { 2 };
        }
        k += 1;
    }
    if (n == m)
    {
        return 1;
    }
    if (n < m) { 0 } else { 2 }
}

// The id of the runtime entry named `name`, `RT_NONE` when there is none.
export fn rt_lookup(StringView name) : u16
{
    slice<u8, shared> nb = StringView::as_bytes(name);
    if (slice_len(nb) < 4 || nb[0] != b'c' || nb[1] != b'b' || nb[2] != b'_')
    {
        return RT_NONE;
    }
    usize lo = 0;
    usize hi = RT_COUNT;
    while (lo < hi)
    {
        usize mid = (lo + hi) / 2;
        u8 o = name_order(nb, RT_NAMES[mid]);
        if (o == 1)
        {
            return narrow<u16>(mid + 1);
        }
        if (o == 0)
        {
            hi = mid;
        }
        else
        {
            lo = mid + 1;
        }
    }
    RT_NONE
}""")
(root / "impl/cir/rt.cb").write_text("\n".join(out) + "\n")
print(len(names), "entries")
