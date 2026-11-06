# D-0295 — Light element references: a decided function's element references carry no record in any program; phase 3 closed

Status: ACCEPTED (2026-10-29, under D-0281's delegation: the owner handed the cost model to the assistant "to
manage autonomously, with authorisation given to make all necessary decisions", and asked for all of D-0281 to be
completed without further prompting). D-0281 §24 (as amended at Update 149): phase 3 steps 6–7, the programs that
keep paths, before the single port.
Kind: Decision (`spec/02-schema.md`)
Governed by: `CobaltC_Master_Instructions.md` §9, §12
Depends on: D-0281, D-0292, D-0293, D-0294, D-0202
Affects: `impl/cbrt/src/lib.rs`; `spec/14` §6 (the cost contract's tier-D lines); `CHG-0390`

## Problem

D-0292 made a decided function's references light in any program — formed with one check against the owner's
recorded references, recorded nowhere, ended by nothing — but left element references out: "an element reference,
a slice, a view, a lock's guard and what a thread is lent are recorded in every function" (`spec/14` §6), waiting
for phase 3's steps 6–7 (frame shadows, block objects and side tables: `stress/d0281/phase3/DESIGN.md` §2.7, §5).
So in a program that keeps paths, a decided function reading a `Vec`'s elements through `&v[i]` or `foreach (w in
&v)` still paid the whole record per read: the object over the element's cells looked up by address, a path
minted onto it, stamped into the statement's scope, put in flight to the caller and received there, and retired
at the statement's end. The micro program `d` (a table of 200k views built by one function, read by another in 20
passes) retired 10.6 G instructions for 4 M element reads, 2,600 each; its decided form costs 0.8 G. And every
call whose result travelled through the runtime's in-flight queue — an element borrow, a reference result, a
value holding references — made the statement's pending scope real (`ts()` realizes D-0196's pending scopes
before any use of the thread state) only for the scope to be popped empty: in `d`'s read loop that was a fifth of
the instructions.

After D-0291 the programs that keep paths are 2.9% of the corpus (322 of 11,022 in `stress/d0281/why/`; 59
hand-written: conformance cases that are the run's by design, raw-pointer programs whose reclaimed objects the run
checks, `fn` values and `PriorityQueue` comparators kept in data, threads lent references no fact describes); the
self-hosted compiler, the showcases and the controls are all decided, and D-0294 halved the decided programs'
cost. The question of §24 was how much of steps 6–7 those 2.9% warrant.

## Decision

1. **Element references are light in a decided function.** In a frame that forms light references (a function
   the static pass decided, in any program, D-0292), a borrow of an element whose object *persists* — one already
   established over the element's cells that is not ephemeral, or one established by this borrow for an element
   that holds references or a `fn` value — is a light reference (`LIGHT | obj`, the element's object): checked at
   its formation against the holders recorded on that object, exactly as the recorded borrow was
   (`check_root_access`: a shared borrow beside an exclusive holder, or an exclusive one beside any, is
   `diag.aliasing-conflict`), and then recorded nowhere, stamped nowhere, in flight as itself, and retired by
   nothing. Stored into data it becomes a holder with its place (`cb_hold_ref`, the element object's whole
   extent); arriving in a function that leaves a conflict to the run it is recorded on arrival (`cb_rec_ref`),
   as any light reference. A borrow of a plain element whose cells have no object (an *ephemeral* object would
   be established, ended when its last recorded path ends) keeps the recorded form: a light reference to an
   ephemeral object would not see that end. The two entries that go on to check or send the path themselves
   (`cb_elem_access_slow`, the native `HashMap::get`'s datum) keep the recorded form too.

   Why this is sound: D-0292's argument (DESIGN.md §2.1) holds for an element as for any place. Within the decided
   function, `rule.control.frame-references` has decided every access against every reference of the frame's
   that has facts, element references included; a holder that overlapped this reference would have met it at the
   holder's formation (the element object's list is checked at every store into it) or at this borrow's; the
   element's cells cannot be released while the function holds a reference to them, since the release is an
   exclusive access to the `Vec` the static pass refuses beside a held element (or leaves to the run, which makes
   the function record). Nothing in the runtime reads the record of a light reference, so a check that was made
   through the recorded element path is a check the static pass made instead.

2. **A reference's flight leaves the statement's scope pending.** The in-flight queue is not the stack: its users
   read the thread state without making the pending scopes (`ts_raw()`), so a call that passes a reference
   through it, in any function, leaves the caller's statement scope pending until something is actually stamped
   into it; a scope never made is popped by one test (D-0196).

3. **Phase 3's steps 6–7 are not built.** The frame shadows (step 6: a compiler-declared shadow of tokens beside
   each frame value holding references, moved by the C with the bytes) and the block objects with side tables
   (step 7: one object per heap block, element holders as `Index(i)` projections on it, a side table of tokens per
   word, a per-heap page radix for raw-pointer code, relocation at growth) would lower the constants of the
   remaining tier-D operations — a store of a reference into data, the end of an aggregate holding references,
   the element objects of a `Vec` of values holding references — not their orders, at the price of a header per
   heap block, an address-keyed radix that §14 wanted gone anyway, and a cross-object rule between a block's
   holders and an element's own object (DESIGN.md §6 R3) in `std`'s raw-pointer code. The programs they would
   serve are the 2.9% that keep paths, in which the decided functions — most of their functions — now pay nothing
   per reference, element references included; what those programs still pay is per store into data (the micro
   program `h`, a million times a struct holding a reference pushed into a `Vec`, cleared and read back: 18 k
   instructions per round) and per frame and statement scope of the objects their values keep (D-0294's remainder,
   which a decided program has lost and a tracked one keeps). The step is recorded as available, to be taken when
   a real program's profile is dominated by holders in heap data; none in the corpus is.

4. **The tier-D cost contract is restated as what this edition promises** (`spec/14` §6; D-0281 §14's provisional
   lines withdrawn). For a reference the run checks: forming one into an owner is amortised O(1) and may allocate
   (its record); a light reference is O(1) and allocates nothing; copying an aggregate that keeps k references is
   O(k); *moving* one is O(k) (its k slots are re-keyed; D-0281 §14's O(1) needs metadata beside the value, which
   `[Repr-Ref]`'s 8-byte reference leaves no room for); reading through a stored reference is one validity
   compare and a scan of the owner's *exclusive* holders (usually none); writing to an owner that has h holders
   is O(h); ending a holder is O(1); destroying or moving an owner with h holders is O(h); and an element of raw
   storage reached without a base (`std`'s raw-pointer code, an element borrow) is found by an address-keyed
   lookup, one per heap (D-0202). Granularity stays per element (D-0281 §16, `conf.vec-element-access-beside-
   held-element-ref`): nothing in the corpus asks for the coarser rule, and the count it would save is the
   holder list's length, already kept.

5. **Phase 3 closes.** Its steps 0–4 (D-0292, D-0293), the objectless remainder (D-0294) and this record are what
   phase 3 delivers; step 5 (a holder's formation site in a fault's note) is a diagnostics feature outside the cost
   model, deferred; step 8's measurement is item 4's; step 9's contract and documents are this Update's. D-0281's
   remaining item is the single port of the self-hosted toolchain (D-0278's amendment) and its measurement.

## Why these, and not the alternatives

- Making element references light needed no new runtime state and no change to the generated C: the element's
  object is the base a light reference needs (for a holder's place, for arrival in a recording function), and the
  check at formation is the one the recorded borrow made. Steps 6–7 would have changed every native of `Vec`,
  `HashMap`, `Box` and the queues, the allocation of every held block, and the compiler's lowering of every frame
  value holding references — the code the single port must then port — for the 2.9%.
- Slices and views stay recorded even in a decided function: a view stored into data needs its *range* in the
  holder (two views into disjoint parts of one `String`, one exclusive, must not meet), and a light token carries
  no range; rebuilding it at the store from the view's own value (DESIGN.md §2.4's plan) is a change to
  `cb_hold_ref`'s contract and to `cobc`'s call sites, deferred with steps 6–7. In the programs that keep paths the
  views are mostly formed in the function that keeps them (a splitter's results), a recording function already.
- The ephemeral exclusion keeps D-0023's accounting: an ephemeral element object ends when nothing recorded
  names it; a light reference is not recorded, so it must not be the only name of one.
- Restating §14 rather than meeting it: the O(1) move and the "no address-keyed lookup" lines were written before
  D-0290 and D-0291 made tier D rare, and before the measurements showed where a tracked program's instructions go
  (the objects of values, the scopes and frames: D-0292's and D-0294's findings). A contract that promises what is
  built, with the orders measured, is the review's second precondition (D-0281 §12) met honestly.

## Consequences

- Measured (`stress/d0281/phase3/BENCH.md`, `runs/elem` against `runs/base149`, Update 149's committed toolchain
  rebuilt; instructions, GCC `-O2`): `d` 10.57 G → 4.94 G (0.47: 4 M element reads light, the statement scopes of
  the read loop pending); `h` 0.90 and the self-hosted `cbc` 0.98 from the pending scopes alone (item 2 reaches
  every program that has flights); `d0` (its cost is the 200k stores in a recording function), `b`, the other
  decided programs and the controls within 1%. `d`'s profile after:
  `HeapReclaimed::get` 12% (the element object by address), the loop body's frame push and pop 21%, `cb_stmt_pop`
  7%, `cb_elem_borrow` 6%: the block frame the body opens for its binding, which `cobc` keeps in a tracked program
  because a binding's token might need holding — the per-function value lowering of `PERFN.md`, if a program
  asks for it.
- A program that keeps paths pays for its recording functions and for what it stores into data, not for the
  element reads of its decided functions.
- `COBALTC_RECORD=full` records element borrows as before this record (the oracle); `COBALTC_RECORD=check` aborts
  where a light element reference reaches a recording frame unrecorded.

## Verification

- G-dyn: the 82 dynamic-tracking conformance cases under `gate3.sh` (light under `COBALTC_RECORD=check`, full,
  `coby`), GCC and Clang: all SAME.
- The three-way differential (`gate3.sh`) over the 996 programs that keep paths (`kept146.list`), GCC; the
  repeated runs for the thread races; `cobmodel compare` over the modelled DIFFs.
- The bench (`bench.sh elem`), the controls unchanged.
- `cargo +1.77.2 check --workspace --tests`; the Windows runtime rebuilt; the release suites.
