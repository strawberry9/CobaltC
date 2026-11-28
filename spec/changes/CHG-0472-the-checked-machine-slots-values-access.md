# CHG-0472 — The checked machine: slots, by-value passing, the access past its plain case (implementation only)

Status: implemented (2026-11-19); Update 200
Affects: `impl/cbrt/src/rt.h`, `rt_objs.c`, `rt_refs.c`, `rt_tables.c`, `rt_lock.c`, `rt_init.c`, `replay.c`,
`rt_objs.h`, `ds.h`; `impl/CHANGELOG.md`, `impl/STATUS.md`. No change to `cbrt.h`, to the generated C or to
any rule.

## Why

`CHG-0471` ended with a list of what was left and the owner's goal stands (2026-11-18: the runtime as fast
as it can be made). Its profile of the compiler in the bootstrap form was taken again on Update 199, and
three things `CHG-0471` had not looked at were on it: the passing of a value that holds a reference (a
slice, a view: sent, received and copied through lists built for each operation, about 15% of the run), the
reference slots' own entries (a store had no lone path at all), and the checked access when the object has
another exclusive path (two thirds of the compiler's reads: a shared reference formed from an exclusive one
that is held), which left the quick path, called a second function that looked everything up again, and a
third.

No rule changes. Every fault a program could get it still gets, with the same text.

## What changes

**One flag for the lone thread.** `lone()` read two variables (no thread started; the trace read and off)
at every entry. It reads one (`LONE`), written where either changes (`go_multi`, `go_solo`,
`replay_init`).

**The reference slots.** The page used last answers "is this address on me, and aligned" in one compare
(`slotmap_last_at`: the address less the page's base, rotated right by three, below 512), and is never
absent (`SLOT_NONE`, an empty page nothing writes). `cb_load_ref` is that compare and a load. `cb_store_ref`
has a lone path: the slot written where it is, the path's count moved. `forget_slots`, which every object
whose type can hold a reference calls at its end, looks at up to four cells where they are and calls
nothing while they hold no slot.

**By-value passing.** A type whose reference places do not depend on the value has them in its record;
`cb_send_datum`, `cb_recv_datum` and `cb_copy_datum` read them there (`rf_offs`) where they built a list
through `ref_offsets` at each call, and read the slots in place on the lone thread. A received reference
whose path is held goes into an empty slot directly (the store's hold and the flight's release cancel).
`detach_tok` takes the last entry of the scope's list where it is the last.

**The access past its plain case.** `cb_read` and `cb_write` hand the path's and the object's records to
what follows (`rf_read_scan`, `rf_write_scan`; file and line travel in one argument so that the call stays
a tail call), which looks at one or two other paths in line: nothing is looked up twice. Path 0 is not
tested for in the entry: it has no record, and what follows returns on it.

**Scopes and frames.** Every stack entry carries its frame's recording flag, so a block's frame reads the
top entry's where it searched down the stack for its frame (`top_rec`). The lone cores make one pending
scope where it goes (`make_pending_one`); `make_pending`, which every program links, stays the compact
loop. A made scope that never held anything is popped in `cb_stmt_pop` itself. The general parts of
`cb_stmt_push_at`, `cb_new`, `cb_borrow` and `cb_store_ref` are functions of their own, so the entries keep
no frame for what only threads or the trace need.

## Measured

Quiet machine, GCC; instructions with `perf`, cycles and time where given. Update 199 against this.

| | Update 199 | Update 200 |
|---|---|---|
| the compiler in the bootstrap form compiling `tinyos.cb` | 540.0 G; 288.9 G cycles; 109 s | 448.7 G; 246.2 G cycles; 93 s |
| bootstrap form: `records` (500 rounds) | 3,557 M; 0.57 s | 3,186 M; 0.52 s |
| bootstrap form: `hashcount` (400,000 keys) | 28,075 M; 6.4 s | 24,831 M; 5.8 s |
| bootstrap form: `strings` | 86,479 M; 23.5 s | 77,523 M; 21.5 s |
| bootstrap form: `tree` (2 rounds) | 26,212 M; 6.0 s | 23,639 M; 5.4 s |
| every check on: `records` | 1,764.2 M; 0.271 s | 1,703.1 M; 0.248 s |
| every check on: `hashcount` | 1,840.6 M; 0.322 s | 1,785.9 M; 0.302 s |
| every check on: `strings` | 2,990.6 M; 0.622 s | 2,888.8 M; 0.591 s |
| every check on: `tree` | 5,562.7 M; 1.084 s | 5,521.3 M; 1.094 s |
| bootstrap form: one round of a loop that passes a vector's element to a function by exclusive reference | 4,142 | 3,720 |
| bootstrap form: one round of a loop that passes a view by value, returns it and copies it | 25,438 | 20,668 |
| bootstrap form: one round of a loop that passes an exclusive reference down three calls and reads through a shared one formed from it | 6,953 | 6,071 |
| every check on: the first of those loops | 1,816 | 1,752 |

The compiler's output is byte for byte what Update 199's wrote, and its peak memory the same (75 MB). The
release profile's `fixpoint` unit, the same compiler compiling itself, took 171 minutes on Update 199 (247 on
Update 198), on a machine that was not quiet.

With every check on the gain is small: those programs are decided where they can be (D-0274) and what is
left of them is mostly the plain access, which `CHG-0471` had already. The bootstrap form decides nothing,
passes every value through the machine and is where the by-value passing and the second path were.

In the default form the benchmark programs execute the instructions they did (`strings` 507.1 M,
`hashcount` 615.7 M, `tree` 139.2 M).

Size: the runtime's text is 438 KB where it was 434 KB. A program's file, GCC: an empty one 54,152 bytes
where it was 54,056 (28 bytes of text), and `strings` in the default form 117,848 where 117,752; `tree` in
the default form 194,504 where 194,200; `records` with every check on 247,312 where 242,816, `strings`
273,920 where 273,520. A program that links the object machinery has about 2 KB more text.

## Tried and not kept

Each measured on the compiler in the bootstrap form and on the workloads:

- `make_pending` itself with a fast path in front of its loop, and as one loop with its growth apart:
  faster at each use, 100 to 316 bytes more in every program, since a program's first frame links it. The
  fast path is a function the lone cores alone call (`make_pending_one`), so a program that links no core
  is not larger.
- The look at one or two other paths in `cb_read` itself: the compiler ran 0.5% fewer instructions and
  the plain read went from 57 instructions to 74, its registers saved at entry for a case it does not
  take. The look is in what `cb_read` tail-calls.
- `forget_slots`' taking of the slots through a small array: an array on the stack puts a canary and a
  176-byte frame on every call. Four scalars (`forget_cells`).
- The runtime built without the stack protector (`-fno-stack-protector`): the compiler in the bootstrap form
  runs 1.4% fewer instructions and 0.4% fewer cycles. Not taken: the runtime is the trusted C under every
  program, and its distribution's hardening stays on for that.

## What is left

The profile of the compiler in the bootstrap form is flat: `cb_new` 9% of its cycles, an object's end 7%, a
place's borrow 5%, a statement scope's end 4%, `cb_read` 4%, and nothing else above 3.5%. Each was read
instruction by instruction (`stress/rtopt/cgasm.py`): straight-line work on the model's records, with no
second lookup and no list built to be thrown away. The cycles inside them that stand out are the first
touch of an object's slot and of the reclaimed objects' table, which are the records themselves.

So what `CHG-0471` said is left is what is left: the number of records the model keeps for each object and
path (a round of the element-borrow loop above is 3,720 instructions in the bootstrap form: an object and
a path made and ended, three frames, eleven checked accesses and as many loads of a stored reference).
Fewer would take records of another shape (an element's object and its one path as one record; lists that
live inside the records), which is a change to the machine that every entry, the state verifier and the
bounded proofs read, not to how fast the present one runs. Nothing here goes that way.

## Checked

On the final source, before the gate (`stress/rtopt/LOG.md` has each run):

- The conformance suite, 1,423 cases: in the bootstrap form, with every check on and in the default form
  under GCC; in the bootstrap form and with every check on under Clang. All pass.
- The same in the bootstrap form and with every check on with the runtime's state verifier at every
  statement's end (`COBALTC_VERIFY=1`): all pass.
- The 683 recorded programs, whose records hold the status and both outputs byte for byte: all pass.
- The command cases: 146 pass.
- The suite with the runtime and the programs under AddressSanitizer with UBSan, the programs in the
  bootstrap form: 1,423 passed. Under ThreadSanitizer in the same form: 1,422 passed, and
  `15-function-semantics/requires_self_tail_ok` did not, as on Updates 198 and 199 (`CHG-0471`: three
  million self tail calls made as calls end in a signal inside the sanitizer).
- The runtime's bounded proofs (`impl/cbrt/tests/cbmc/run.sh`): the six harnesses pass.
- Three programs compiled for Windows under Wine in the three forms: the outputs Linux gives.
- The runtime built by GCC, Clang and MinGW-w64 without a warning.
- The four workloads' outputs in the three forms, and the C the bootstrap-form compiler writes for
  `tinyos.cb`: what Update 199 gives, byte for byte.
