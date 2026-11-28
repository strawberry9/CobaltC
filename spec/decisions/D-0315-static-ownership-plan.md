# D-0315 — The static ownership plan: a decided program's values are ended by the compiler, and the runtime keeps a record only where no static fact stands in for it

Status: ACCEPTED (2026-11-08, the owner, on the proposal of that day, "the static ownership plan": "I approve. Do all
of this work autonomously as you see fit, to ensure the goals of the proposal are met in full."); implemented in
steps: the proposal's steps 1–3 and 5 by `CHG-0417`; step 4 (a cleanup table for values whose destruction a fault
must run and could observe) by `CHG-0418` (the unwinders' fold, `fn` parameters) and `CHG-0419` (locals whose type
has a destructor of its own); steps 6 (location stores), 7 (`spec/14` §6's cost contract) and 8 (decided on its
measurement) by `CHG-0419`
Kind: Decision (`spec/02-schema.md`)
Governed by: `CobaltC_Master_Instructions.md` §9, §12, §21
Depends on: D-0008 (scope-end destruction), D-0009 (faults are not caught), D-0200 (compile-time discharge), D-0204
(`[Fault-Contain]`), D-0269 (objectless locals and the contained unwind), D-0281 (the cost model), D-0284/D-0285
(decided programs and their lowering), D-0286 (values pass without objects), D-0294 (the objectless remainder), D-0296
(both toolchains in step), D-0309 (decided programs against checked Rust), CHG-0389 (`elide_record_chains`)
Affects: `impl/cobc/src/lower/*` and their twins in `impl/selfhost/back/*` (D-0296), `impl/cbrt/src/lib.rs`,
`impl/cbrt/include/cbrt.h`, `impl/cobc/src/report.rs` and `impl/selfhost/cbc/report.cb` (`--report-tracking`),
`spec/conformance.md`; `spec/14` §6 (the cost contract) at the last step

## Problem

A decided program (D-0284: every reference conflict refuted before the run) still told the runtime, at every call,
statement and resource value, what exists, so that a fault could find it: a frame per call, a statement scope per
statement that makes a temporary, an object per `match` and `?` binder, per call result handed on, per borrowed
temporary. In such a program no check reads those records; their only reader is the unwind. Measured
2026-11-08 (the owner's speed test): the runtime was 71% of the self-hosted compiler's time and 75% of the
formatter's, and `cbc` took 4.8 times `cobc`'s time; in `cbc`'s own C, 14,893 objects, 29,785 statement scopes and
15,292 block frames were written.

## Decision

In a decided program the compiler decides, for each value a body owns, who ends it and when, and emits the end
itself; a value has a runtime record only where no static fact can stand in for it. Concretely, a value whose end
frees memory alone (`objectless_type`: no destructor of its own, no `fn` value, no handle, guard or OS resource
inside) is a C value:

1. **Its consumer takes it as it is.** A consumer that ends or keeps the value — a binding, an argument, a
   variant's payload or a struct's field, an assignment, `Box::new`, a `match`'s scrutinee, a discarded statement —
   arms the producer (`arm_inert`), and the producer builds it with no object: a call's result, a literal, a
   `match`, `if`, block or `?` (`raw_arm`, its sink raw). A call takes its arming as its lowering begins, so its
   arguments' own lowering cannot clear it. A consumer that needs an object gives it one where the value lands
   (`materialize`), never earlier than before.
2. **A statement's temporaries end with the statement, by C.** A borrowed call result (`&f(…)`, a view of one) is
   a C local with token 0, dropped where its statement ends or a jump leaves it (`stmt_temp_drop`), last made
   first, `[Stmt-Exit]`'s order; a flag per temporary (`cb_tl`) says whether it was made, since a short-circuit
   operand may pass it by. It is noted in the body's table (D-0269) so a contained fault frees it.
3. **`?` is a `match` like any other.** Its binders are objectless locals; a consumer's arming of the `?` passes to
   the `match` it is.
4. **The candidate walk follows places by their whole path.** A borrow or slice of `x.f[i].g` is a borrow; a read
   records the path, allowed when what it reaches is plain data; a `String` field compared is read where it lies.
   A `Vec` of memory-only elements may be indexed. A `resource struct` with no destructor, and a `std` struct, are
   judged by their fields.
5. **Plain data is plain.** A slice, a view and any value holding only references and plain data are plain in a
   decided program: no object for one, and a local of one pinned by a borrow has root token 0.
6. **Frames nothing binds into are not pushed**, the natives' fixed texts included (`elide_empty_frames`);
   `Vec::drop` of plain elements takes the plain loop.
7. **The runtime knows the program is decided** (`cb_set_decided`, from `cbrt.h`'s `cb_init` under `CB_DECIDED`):
   a reference's flight and re-stamp are constants there (nothing else is ever in flight), and a destructor run on a
   value with no object gets token 0, with no object made to carry it.
8. **Values are ended by drop glue** (`cb_dg_<type>`): a generated function per memory-only type ends a value as
   the runtime's walk would — a struct's fields last first, an enum's payload, a `Vec`'s elements then its buffer,
   a `Box`'s referent then its block, a `HashMap`'s three vectors — where no object can lie over raw cells
   (`cb_reclaimed_any()`); past a depth of 1000, the runtime's walk, which ends a `Box` chain of any length in
   constant stack, takes the rest. A quiet write ends the old value the same way where its base token is 0.
9. **More forms are followed**: a `match` arm binding the whole value of an enum; a destructure (`P { c, .. } = p;`,
   its binders candidates as a `let`'s are, its source moved, the fields `..` passes over bound with no object);
   `drop(x)` of an objectless local (its drop where it is, then its moved flag); `foreach` by value (the vector moved
   into the drain, the drain borrowed by `$each_len` and `$each_take`; `std::Drain` and `std::HashDrain`, whose
   destructors free memory alone, are memory-only where their parts are); a reference binding (root token 0: every
   token is 0 there); `_ = e;` and a discarded `?`, ended where the statement ends; a `match` keeps its consumer's
   arming across its scrutinee's lowering. `HashMap::insert`'s native text gives a `String` key no object.
10. **A block of plain cells grows by `realloc`** (`cbrt.h`'s `cb_reallocate_plain`, inline) where no object lies
   over raw cells and no thread was ever started, which is what the runtime's function did there.
11. **Values a fault must end are noted, not tracked** (step 4): a function's table of noted values records the frame
   stack's depth at its registration, and every unwinder folds frames and tables together, innermost first. A `fn`
   parameter the body only calls is such a value: no object, an entry registered on every call, its end after the
   body's frame at every return (`CHG-0418`). The other values with a destructor of their own follow.

What keeps a runtime record in a decided program after this: a value whose type has a destructor of its own, a
handle, a guard, an OS resource or a `fn` value, until the table of step 4 of the proposal (one static cleanup table
per function) replaces them; and the forms the analysis does not yet follow, which `--report-tracking` lists.
Programs that keep paths (tier D) are unchanged: every rule is gated on `Gen::decided`.

## Why these, and not the alternatives

- The proposal's alternatives (an unchecked build, removing `[Fault-Unwind]`'s destructors, landing pads in C) stay
  rejected or deferred for its reasons.
- The steps are corrections of what the analysis reaches, measured on the self-hosted compiler's own C, rather
  than new text passes over the emitted C: CHG-0389's lesson was that a text pass that cannot see the program can
  remove a destructor a fault must run. Every rule here is decided where the type and the consumer are known.
- Drop glue rather than a cheaper runtime walk: a generated function knows the layout; the walk reads a descriptor
  per part and costs about a thousand instructions per value in `cbc`.

## Consequences

- No language change, no change to a program's outcome, fault, phase or output; `coby` is unchanged.
- `--report-tracking` on a decided program lists the cleanup bookkeeping its C still carries, by source line; for
  every program of the speed test it lists none.
- Measured: see `CHG-0417`. The acceptance targets of the proposal stand: `cbc` within 1.5 times `cobc`'s time on
  every program of the speed test (met on seven of ten by `CHG-0417`; the three largest remain), `cobfmt-cb` no slower
  than `cobfmt`, the runtime under 15% of `cbc`'s profile on the Lisp interpreter (met: 13%), no frame, scope or
  object in the C of the decided programs of the speed test (met), the showcases and the benchmarks.

## Completion (CHG-0419, 2026-11-09)

All the plan's steps are done. Step 4's second part notes a local whose type has a destructor of its own in its
function's table; step 6 stores a statement's location before a call only where the callee may read it (the reason
D-0309's round had dropped such a pass, no fewer cycles, stood for the benchmarks and not for an interpreter: 4.2%
fewer cycles there); step 7 is `spec/14` §6's table of what a decided program's value costs at its end; step 8 stops,
as the plan says, since what a cleanup attribute could remove (the tables' registrations, under 0.1% of `cbc`'s
instructions) is under 5%. The acceptance targets, measured on the source of `CHG-0419`:

- `cbc` within 1.5 times `cobc`'s time on every program of the speed test: met, 0.29–1.49 (the type inferencer
  1.49, the largest program 1.47).
- `cobfmt-cb` no slower than `cobfmt` on every file: met, 0.03–0.70.
- The runtime under 15% of `cbc`'s profile on the Lisp interpreter: met, 10.8% of its cycles.
- No frame, scope or object in the C of the decided programs of the speed test and the benchmarks: met. For the
  showcases: not met. 48 of the 84 decided showcases list nothing; the other 36 list 217 lines, most of them values
  the runtime owns by design (§3.5 of the proposal: threads' handles, guards, channels, closures' boxes, `Rc`) and
  `std`'s resource types with destructors (`File`, sockets, HTTP), which the noted values do not yet admit, and the
  frames of the bodies that hold them.
- Both toolchains in step, `cbc`'s C identical to `cobc`'s, the fixpoint, the gate: as for every Update.

## The showcases (CHG-0420, 2026-11-09)

The showcases' target, not met in `CHG-0419`, is met: `--report-tracking` lists nothing for any of the 84 decided
showcases, and nothing still for the speed test and the benchmarks. The values §3.5 of the proposal had left to the
runtime have no runtime object in a decided program:

- `std`'s resource types, pattern binders and statement temporaries whose types have destructors of their own: noted
  in their function's table.
- Threads' handles and lock guards: noted.
- Mutexes and arrays: judged by their contents.
- Closures: a static or stack box where no `fn` parameter lets them escape.
- A `let` with no value: bound with no object.

The other targets hold on that source too: `cbc` 0.27–1.47 times `cobc`'s time, `cobfmt-cb` 0.03–0.66 times
`cobfmt`'s, the runtime 9.8% of `cbc`'s cycles on the Lisp interpreter.

Step 8 was also carried out as the plan says:
- The design is on paper and one program is prototyped.
- The prototype's costs lie in the runtime's generic end of a value and in a registration made by a call. Both can be
  removed without unwinding: a generated drop for such a type, and an inline registration.
- O3 itself is not adopted.

Not yet decided:
- A program that spawns a value whose type has a destructor of its own.
- An array of handles to threads that were lent references.

No showcase does either.
