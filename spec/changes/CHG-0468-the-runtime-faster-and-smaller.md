# CHG-0468 — The runtime, faster and smaller (implementation only)

Status: implemented (2026-11-18); Update 198
Affects: `impl/cbrt/src/rt_alloc.c` (new), `rt.h`, `rt_refs.c`, `rt_objs.c`, `rt_tables.c`, `rt_init.c`,
`rt_lock.c`, `rt_threads.c`, `replay.c`, `ds.c`, `ds.h`, `diagnostics.c`, `diag_table.h` (generated),
`rt_objs.h`, `rt_refs.h`, `rt_threads.h`, `lib.c`; `impl/cbrt/include/cbrt.h`;
`impl/cbrt/tools/gen_diag_table.py`, `exports.txt`; `impl/cbrt/SOURCES`; `impl/back/gen.cb`, `header.cb`
(generated); `impl/tools/validate_c.py` (the new entry points classified); `impl/cobc/driver.cb` (the help's
sizes); `spec/IMPLEMENTATION-NOTES.md` (the runtime contract's
table); `impl/STATUS.md`; the guide (`content_intro.py`); `bench/RESULTS.md`; D-0329 (the allocator)

## Why

The owner asked for the runtime's C, and the C the compiler emits around it, to be as fast and as small as
they can be made, and for the result to be pushed in a state that needs no second pass (2026-11-18). Measured
first (`stress/rtopt/LOG.md`): in the default form the library proper was hardly on any profile, and the C
library's allocator was (20% of the instructions the compiler itself executes); with every check on, each
checked read or write cost about 410 instructions and each element borrow about 2,500 more; an empty program
was 234 KB.

No rule changes. Every fault a program could get it still gets, with the same text.

## What changes

**The default form**

- The runtime allocates small blocks itself, in line in the generated C (D-0329).
- `[Copy-Raw]` is `memmove` in line while no reference was ever stored into data (`cb_slots_used`) and no
  thread was started; `[Deallocate]` of a small block is a push in line while also no object lies over raw
  cells; the first growth of an empty vector allocates in line, and a small block moves to another class in
  line.
- The generated C and the header no longer call `free` or `realloc` on a program's block themselves
  (`cb_block_free`; `glue_free` in the back end; `tree_drop` in the runtime).

**Every check on**

- A checked read or write (`cb_read`, `cb_write`) looks its path and its object up once, and the access
  with nothing to decide -- this thread alone, no trace, a valid recorded path over a live object whose
  other paths that could clash are not held -- is answered at once (`rf_access_quick`); anything else goes
  to the general check from the start.
- The arenas' lookup is one bounds test and one word compare (the generation and the filled mark read as one
  word); heap 0, the lone thread's, is at a fixed place.
- An element's or a `Box`'s ephemeral object is kept in a short list per heap (`Eph`: sixteen places, then
  the ordered map), not in an ordered map with a size map beside it; the ordered map keeps its last leaf when
  it empties; an object's list of exclusive paths comes from the pool; a short token list is pushed and
  emptied in line; an empty list frees nothing.

**Size**

- The diagnostics registry in the runtime is one text and a table of 16-bit offsets, and holds the 38 entries
  a running program can raise: those whose Phase names the dynamic phase, and three the generated C raises
  though the registry calls them static. It was 85 entries of 11 pointers, each pointer a relocation in
  every program. The compiler renders its own diagnostics from the whole registry, as before.
- What only some programs need is reached through pointers filled where the need arises, so a program links
  what it uses: the end of a thread handle (`cb_spawn`) and of a guard (`cb_lock*`); the ends of objects, of
  objects over raw cells and of stored references (FULL, `rt.h`: filled where the first of each is made);
  and everything the entries do only while threads run -- the locks, the heap holds, a thread's contained
  fault (the MT_ pointers, filled by `go_multi` before it turns `multi` on). Where a pointer is null there is
  no such state, and the entry does what the full code does with none. The pointers and the two flags beside
  them are read and written atomically (relaxed): every writer writes the same value.

**Kept as it was, on purpose**: the unwind tables (`.eh_frame`, about a tenth of a small program), which a
debugger's `bt` through the runtime needs; the replay trace and the scheduler, which an environment variable
turns on in any program; the distribution's hardening of the C compilers (measured: nothing to gain).

## Measured

Quiet machine, median of five runs; instructions with `perf`.

| | Update 197 | Update 198 |
|---|---|---|
| the compiler compiling `tinyos.cb` (both built with LTO, one file) | 1814.5 M; 1895 M cycles | 1412.2 M; 1569 M cycles |
| `strings`, GCC / Clang | 0.311 / 0.316 s; 645.8 / 633.5 M | 0.230 / 0.226 s; 507.1 / 500.2 M |
| `hashcount` | 0.218 / 0.216 s; 855.2 / 846.7 M | 0.169 / 0.167 s; 615.7 / 629.3 M |
| `tree` | 0.255 / 0.257 s; 260.7 / 263.4 M | 0.208 / 0.222 s; 139.2 / 150.6 M |
| every check on: `records` (500 rounds), GCC | 5994 M | 2684 M |
| every check on: `hashcount` (400,000 keys) | 5202 M | 2755 M |
| every check on: `strings` | 7468 M | 4337 M |
| every check on: `tree` (2 rounds) | 10872 M | 7348 M |
| an empty program, file bytes, GCC / Clang | 233,760 / 234,992 | 54,104 / 54,576 |
| `strings`, file bytes | 257,184 / 257,576 | 117,808 / 114,072 |
| `hashcount`, file bytes | 252,464 / 244,672 | 109,024 / 109,352 |
| `tree`, file bytes (it makes objects) | 243,824 / 244,344 | 177,288 / 177,736 |

`sieve`, `sieve_fn`, `collatz`, `collatz_wrapping` and `records` in the default form do not reach the runtime
and execute the instructions they did, to the digit. `bench/RESULTS.md` is measured again on this source.

What is left, for the record: with every check on, an element borrow still costs about 2,000 instructions
(an object, a path and a scope entry made and ended through the general tables). Bringing that down further
means giving the one-statement element borrow a representation of its own, which every other entry would
have to recognise; it was not done.

## Checked

No gate ran while the work was done (the owner's instruction then); what ran, on the final source unless
said (`stress/rtopt/LOG.md` has each run):

- The conformance suite, 1,423 cases, with every check on: GCC, on the final source; Clang, on the source
  before the last hooks. In the default form: GCC and Clang (and the gate's `corpus`).
- The 683 recorded programs, whose records hold the status and both outputs byte for byte: all pass with the
  smaller registry and the hooks, so no fault's text changed.
- The command cases, 146 with the two of `CHG-0470`: all pass. One failed on the way,
  `driver_sanitizer_reports_program_line`: a program compiled by a sanitizing C compiler against the normal
  runtime no longer had its overflow reported; D-0329's item 5 (the program says it is sanitized) is the fix.
- The suite with the runtime and the programs under AddressSanitizer with UBSan, and under ThreadSanitizer
  (`impl/cbrt/tests/sanitize.sh`): 1,423 passed under each. ThreadSanitizer first reported races on the new
  shared pointers and flags in 13 cases; the atomic accesses are the fix.
- `stress/rtopt/alloc/stress.cb`, the allocator's own test (every class and past the largest; one thread,
  then five threads handing blocks through a channel; every block checked byte by byte): the same output
  from Update 197's toolchain and from this one under GCC and Clang, default form and every check on, and
  compiled for Windows under Wine.
- The runtime built by GCC, Clang and MinGW-w64 without a warning; `runtime_contract.py --check`,
  `trusted_table.py --check`, `sigcheck.py`.
