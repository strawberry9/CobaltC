# CHG-0471 — The checked machine, faster (implementation only)

Status: implemented (2026-11-18); Update 199
Affects: `impl/cbrt/src/rt.h`, `rt_objs.c`, `rt_refs.c`, `rt_tables.c`, `rt_lock.c`, `rt_init.c`,
`rt_threads.c`, `rt_objs.h`; `impl/CHANGELOG.md`, `impl/STATUS.md`; two comments in `impl/tools/gate.py` and
`fixpoint.sh` that gave the fixpoint's time. No change to `cbrt.h`, to the generated C or to any rule.

## Why

`CHG-0468` left one thing open and said so: with every check on, an element borrow still cost about 2,000
instructions. The release profile's run on Update 198 then showed how much that mattered: the compiler in
the bootstrap form (every D-0274 stage off, which the gate's `fixpoint` unit runs for four hours) had gained
12% where `--all-checks` had gained 32 to 55%. A profile of it had 92% of its time in the runtime, spread
over some fifty functions, none above 6%: the general machine itself (frames, statement scopes, an object's
making and end, a path's, the reference slots), which the quick access path and the allocator of `CHG-0468`
do not reach. The owner's goal for the runtime stands (2026-11-18: as fast as it can be made), so the
general machine was gone through, one operation at a time, each step measured.

No rule changes. Every fault a program could get it still gets, with the same text.

## What changes

**One entry on the stack for a frame.** A frame was an entry, a location in a second list and a recording
flag in a third, and its list of what it owns came from the pool at every push and went back at every pop.
The entry now carries its location and its flag, and takes a list from the pool when it first owns
something (`Entry`, `owned_push`).

**The lone thread's paths, specialised.** While no thread was ever started and no trace is written
(`lone()`, `rt.h`), an entry is thread 1 on heap 0 with the main state and takes no lock. Each entry that
runs at every statement of a checked program does its common case under that test, by a core that is
written once and inlined twice: with the thread known (`L` a constant: no thread-local read, no test for
other threads, no trace hook) into the entry's lone path, and into the general function of the same name.
The cores: a frame's push and pop, a statement scope's push and pop, an object's making (`cb_new`), its
binding (`cb_bind`), its end and its identity's end, a path's minting and its retirement, a scope's end, a
frame's end, the element borrow, the borrow of a place. The general functions do what they did, and are
what runs with threads or under the trace.

**Records looked up once.** An object's end looked its record up four times and its type three; a path's
retirement its object twice; a borrow viewed its base three times. Each now passes the record it has.
`[Swap-Places]`' validity check (`cb_valid`) built `path_target`'s two lists, allocating, to throw them
away: it checks what `path_target` faults on and builds nothing.

**The reference slots.** The load of a stored reference (`cb_load_ref`) reads the slot where it is when it
is on the page used last. A local that holds a reference emptied its page at every call's end and filled it
at the next: the page emptied last now stays in the table, still counted (`SlotMap.kept`; a table with only
that page is empty to whoever asks, so a thread's heap is reused as before). A few cells on one page are
read where they are, at an object's end and at a datum's copy (`slots_few`), not scanned into a list.

**The quick access path takes more.** A read or write through a path derived from another found that other
path held and went to the general check, which then excluded it as the accessing path's ancestor. The quick
path excludes ancestors itself (`rf_access_quick_rest`); `cb_read` and `cb_write` call nothing but in tail
position.

**Smaller steps**: the trace hooks are tested for where they are called, not called to test; the state
verifier's switch (`COBALTC_VERIFY`) is one load; an object's record is written as one value; an entry in
hand is dropped from its arena without being looked for again; the pools' take and give are in line in the
cores only.

## Measured

Quiet machine, GCC; instructions with `perf`, cycles where given. Update 198 against this.

| | Update 198 | Update 199 |
|---|---|---|
| the compiler in the bootstrap form compiling `tinyos.cb` | 855.4 G; 471.9 G cycles; 177 s | 541.2 G; 292.1 G cycles; 110 s |
| bootstrap form: `records` (500 rounds) | 6,286 M; 1.24 s | 3,557 M; 0.58 s |
| bootstrap form: `hashcount` (400,000 keys) | 44,624 M; 11.0 s | 28,072 M; 6.3 s |
| bootstrap form: `strings` | 136,484 M; 35.6 s | 86,192 M; 22.9 s |
| bootstrap form: `tree` (2 rounds) | 41,154 M; 9.6 s | 26,212 M; 6.0 s |
| every check on: `records` | 2,683.5 M; 0.453 s | 1,764.2 M; 0.265 s |
| every check on: `hashcount` | 2,754.5 M; 0.541 s | 1,840.6 M; 0.326 s |
| every check on: `strings` | 4,336.9 M; 0.896 s | 2,990.6 M; 0.632 s |
| every check on: `tree` | 7,348.3 M; 1.521 s | 5,562.7 M; 1.094 s |
| bootstrap form: one round of a loop that passes a vector's element to a function by exclusive reference | 7,289 | 4,142 |

The compiler's run is the gate's `fixpoint` unit in small: the same compiler, written in the bootstrap form,
compiling a program of 11,000 lines instead of itself. Its output is byte for byte what Update 198's wrote,
and its peak memory the same (75 MB).

In the default form the benchmark programs execute the instructions they did (`strings` 507.1 M,
`hashcount` 615.7 M, `tree` 139.2 M).

Size: the runtime's text is 434 KB where it was 406 KB (the lone cores are second copies of what the
general functions do). A program's file, GCC: an empty one 54,056
bytes where it was 54,104, and `strings` in the default form 117,752 where 117,808 (they link none of it);
`tree` in the default form, which makes objects, 194,200 where 177,288; `records` with every check on
242,816 where 221,392, `strings` 273,520 where 248,000. A program that links the object machinery is about
a tenth larger, and one that does not is not.

**Tried and not kept**, each measured on the compiler in the bootstrap form and on the four workloads:
keeping a new temporary beside a pending statement scope instead of making the scope (it rarely applies:
a parameter's object and most locals' are made where a scope already is; 1% slower); a lower load limit
for the hash tables (1.3% fewer cycles on the compiler, nothing elsewhere, 8% more memory in the tables);
taking the product's high half as the hash; a filter by the page counts and a table of recent answers in
front of the reclaimed objects' map (its misses are cache misses on a large table, which neither avoids:
the counts are all nonzero in a large program, and the lookups do not repeat); sharing one lone copy of a
path's minting and of an identity's end (1.5 KB smaller, 2% slower).

**What is left.** With every check on, an element borrow is now about 1,000 instructions from its forming
to its end, a local that holds a reference about 970 from `cb_new` to its frame's end: an object's record,
a path's, a list on each for the other, a statement scope's list, the table of reclaimed objects and the
page counts, each made and unmade. That is the model's state, kept as the model has it. Less would take
records of another shape (an element's object and its one path as one record; lists that live inside the
records), which every entry that reads them would have to know; nothing in this change goes that way.

## Checked

On the final source, before the gate (`stress/rtopt/LOG.md` has each run):

- The conformance suite, 1,423 cases: in the bootstrap form, with every check on and in the default form
  under GCC; in the bootstrap form and with every check on under Clang. All pass. The suite on three earlier
  states of the work passed too.
- The same in the bootstrap form and with every check on with the runtime's state verifier at every
  statement's end (`COBALTC_VERIFY=1`): all pass.
- The 683 recorded programs, whose records hold the status and both outputs byte for byte: all pass.
- The command cases: 146 pass.
- The suite with the runtime and the programs under AddressSanitizer with UBSan, the programs in the
  bootstrap form (`impl/cbrt/tests/sanitize.sh asan` with `COBALTC_BOOTSTRAP_FORM=1`): 1,423 passed. Under
  ThreadSanitizer in the same form: 1,422 passed, and `15-function-semantics/requires_self_tail_ok` did not:
  three million self tail calls, which the bootstrap form makes as calls, end in a signal inside the
  sanitizer; Update 198's runtime gives the same in that form, which had not been run under the sanitizer
  before.
- The runtime's bounded proofs (`impl/cbrt/tests/cbmc/run.sh`): the six harnesses pass.
- A program compiled for Windows under Wine in the three forms: the outputs Linux gives.
- The runtime built by GCC, Clang and MinGW-w64 without a warning.
- The four workloads' outputs in the three forms, and the C the bootstrap-form compiler writes for
  `tinyos.cb`: what Update 198 gives, byte for byte.
