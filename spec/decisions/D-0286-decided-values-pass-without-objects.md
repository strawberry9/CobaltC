# D-0286 — In a decided program, values pass between functions with no object

Status: ACCEPTED (2026-10-27, under D-0281's delegation: the owner handed the cost model to the assistant "to
manage autonomously, with authorisation given to make all necessary decisions", and asked for all of D-0281 to be
completed without further prompting). D-0281 phase 2, step 4d, second part.
Kind: Decision (`spec/02-schema.md`)
Governed by: `CobaltC_Master_Instructions.md` §9, §12
Depends on: D-0285 (the decided lowering), D-0284 (decided programs), D-0281 (tier S), D-0196 and D-0200
(objectless locals, `__raw` forms), D-0188 (native bodies)
Affects: `cobc` (`lower/flights.rs`, `user_calls.rs`, `calls.rs`, `values.rs`, `intrinsics.rs`, `fresh.rs`,
`frames.rs`, `functions.rs`, `program.rs`, `analysis_strings.rs`, `matching.rs`, `stmts.rs`, `exprs.rs`,
`operators.rs`, `places.rs`), `CHG-0377`

## Problem

After D-0285 a decided program keeps a runtime object only for a value the runtime destroys: a `String`, a
`Vec`, a resource, an enum or struct holding one. Most of those objects exist only between functions. A resource
passed by value, or returned, travels through the runtime's queue of values in flight: the sender's object is put
in flight (`cb_send`), and the receiver takes it from there (`cb_recv`), moves it to where the value lands and
binds it or stamps it into a statement. A native body (D-0188's fixed texts) that returns a resource makes an
object only to send it, and one that takes a resource receives an object only to end its record. extsort spent
half its time in this machinery: per input line, seven objects in `main` and four in the function reading the next
line of a chunk, each made, moved, bound and ended.

In a decided program such an object does nothing a program can see: no path is ever formed on it, and its one use,
destroying the value at the end of the scope that owns it, is arranged again by whoever receives the value.

## Options

- **A. A convention for decided programs.** In the decided lowering no value is ever in flight: the sender ends
  its record, the receiver takes the value with no object where it can and gives it one where it must. The
  natives' fixed texts are rewritten to match. Values travel as C values; objects remain only where a value is
  owned by a binding or statement that the lowering cannot yet drop itself.
- **B. A cheaper runtime.** Keep the convention and make the queue, objects and frames cheaper under
  `CB_DECIDED`. The calls stay; the gain is bounded by their remaining cost, and the runtime then has two object
  models.
- **C. A new lowering for decided programs**, built on drops the compiler places rather than objects. The right
  end state, but a second code generator; A reaches much of it by extending what exists.

## Decision

A, in the decided lowering only:

- a by-value argument of a resource type is handed over as its value: the caller ends its object's record
  (`cb_forget_obj`) instead of sending it, and an inert value has none to end;
- every resource result leaves as a `__raw` form's does (D-0200): its record ends where the function returns;
- a callee binds a by-value resource parameter objectless where the objectless analysis allows it (parameters
  are now its candidates, as `let` bindings are), and otherwise gives it an object of its own (`cb_new`);
- a call's result is taken with no object by an objectless binding, an argument, a struct or enum literal's
  part, a `return` or a function's last expression, and a `match` on it; an arm of that `match` that moves
  nothing out drops it where the arm ends. Elsewhere it is given an object where it lands;
- an inert literal argument, and a literal that is a variant's payload or a function's result, is built with no
  object, and an objectless local moved into an argument, a part or a result goes with none;
- the objectless analysis (D-0196) also allows, in a decided program, a candidate borrowed whole or by a field
  anywhere (its address with token 0), its plain fields read, and enums (`Option`, `Result`, the program's own)
  not declared `resource` and with no destructor, over such values or plain data. A field read by value must be
  plain data: no field is moved out of an objectless value. The last block of a function that returns a
  resource (`unsafe { … x }`) ends it as its last expression does;
- the natives' fixed texts are made to match (`flights::ground_native`): a parameter's object that the body only
  ends, sends on or moves is not made (the rule of the natives' `_raw` forms), an object made only to be sent is
  not made, and any other send ends its record instead. A text the rule does not recognise keeps the program on
  the first lowering (D-0285's fallback).

An inert value held while later arguments or fields are evaluated is only one whose destruction frees memory
alone (`objectless_type`), as D-0200's objectless fields are: a fault there leaves bytes unfreed that no code of
the program would have seen destroyed. Where nothing is evaluated between the value and its consumer, any type.

## Consequences

- No language change and no runtime change: `cbrt` keeps the queue for the programs that keep paths, and
  `COBALTC_DECIDED=0` keeps D-0284's lowering.
- A decided program's C neither sends nor receives an object. The objects that remain are those of bindings
  and temporaries the objectless analysis does not yet cover; option C stays the direction for them.
- `coby` is unchanged.

## Verification

- `stress/d0281/objectless/dgate.sh` (decided against COBALTC_DECIDED=0, both `cobc`): the 300-program sample
  after each stage (all the same but one benchmark's printed rate), then the 1,500-program sample: 1,471 the
  same; 17 different, 14 only in the times they print, two of random input and one printing the bytes of stack
  addresses; 12 not decided, 6 of them reclaiming raw storage in their own code (D-0284 repair 7) and 6 that fell
  back to the first lowering, a `match (&x)` on an objectless local, now lowered (all 7 such programs the same).
- The compiled conformance suite and `spec/conformance.md`'s rows under GCC and Clang; a driver test
  (`decided_program_passes_values_without_objects`): past the runtime's header the C neither sends nor receives,
  a `String` parameter is bound objectless, and the program ends as its first lowering does.

Measured (best of five, GCC): extsort 431 → 288 ms; csvstat 73 → 72 ms.
