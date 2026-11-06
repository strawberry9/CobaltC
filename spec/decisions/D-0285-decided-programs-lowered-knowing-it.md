# D-0285 — A decided program is lowered knowing it is decided

Status: ACCEPTED (2026-10-26, under D-0281's delegation: the owner handed the cost model to the assistant "to
manage autonomously, with authorisation given to make all necessary decisions", and on 2026-10-26 asked for all
of D-0281 to be completed without further prompting). D-0281 phase 2, step 4d, first part.
Kind: Decision (`spec/02-schema.md`)
Governed by: `CobaltC_Master_Instructions.md` §9, §12
Depends on: D-0284 (decided programs run pathless), D-0281 (tier S), D-0188 (dual bodies), D-0196 and D-0200
(objectless locals)
Affects: `cobc` (`main.rs`, `lower.rs`, `lower/layout.rs`, `lower/passes.rs`, `lower/functions.rs`,
`lower/closures.rs`, `lower/program.rs`), `cbrt` (`cbrt.h`), `CHG-0376`

## Problem

D-0284 builds a decided program with no record of references, but it decides that after the program is lowered:
`cobc` lowers every program as if its references were tracked, and `cbrt.h`'s `CB_PATHLESS` block then turns the
path entries into constants. What the lowering chose because references are tracked stays in the C:

- a value that holds a reference (a `StringView`, an `Option<ref<T>>`, a struct with a view in it) is given a
  runtime object when it is copied or received, and each copy copies its slots' holds (`cb_copy_datum`), though
  no slot ever holds a path in a pathless run;
- such a value travels between caller and callee through the runtime's queue of values in flight
  (`cb_send_datum`, `cb_recv_datum`);
- a statement that forms a path opens a statement scope, which a pathless run makes and pops for nothing;
- each dual body (D-0188) asks the runtime whether its frozen twin may run, and the answer is always yes.

After D-0284, csvstat spent most of its remaining runtime calls on these: 229,000 statement scopes, 91,000
objects for one view copied in a loop, and 250,000 calls that end a token that is always 0.

## Options

- **A. Lower twice.** The first lowering finds whether the program is decided (D-0284's test, which reads the
  lowered program); a decided program is lowered again with a flag that says so, and the second lowering's C is
  used. Cost: a second lowering of the decided programs (tens of milliseconds; the C compiler's time dominates).
- **B. Decide before lowering.** Compute decidedness from the front end alone. The front end knows the
  functions with conflicts left to the run and the types that keep references in data, but two of D-0284's
  blockers (a thread spawned, a point check at a call) are found in the lowered program. Moving them would
  duplicate the lowering's knowledge.
- **C. Clean the C afterwards.** Text passes over the pathless C. The lowering's choices (an object, a datum's
  flight) are made in pairs across functions; removing one side by text is fragile.

## Decision

A.

In the second lowering (`Gen::decided`):

- a value that holds references inside it is plain data: no object, no copied holds, no flight of its own. A
  reference or a slice itself keeps its C form (an address and a token, the token 0);
- `cbrt.h`'s new `CB_DECIDED` block, defined with `CB_PATHLESS` before the header, makes the datum flights empty
  on both sides, a frozen parameter's test and a call's token end constants, and the elision of empty statement
  scopes (`resolve_scopes`) counts those entries, and an element's reference, as recording nothing;
- if the second lowering cannot lower the program, or finds it not decided, the first lowering's C is used, and
  `-v` says why. `COBALTC_DECIDED=0` keeps the first.

`-v` says "lowered pathless and decided". The outcome of every program is unchanged: the second lowering only
leaves out entries whose effect, in a pathless run, is none.

## Consequences

- No language change: D-0284's *Decided programs* paragraph already lets a decided program run with no record
  of references; a reference held inside a value is one.
- The objects that remain in a decided program are those of values the runtime destroys (a `String`, a `Vec`, a
  resource) and of `fn` values. Taking those off the runtime, with drops the compiler places, is the next step.
- `coby` is unchanged.

## Verification

- A differential gate (`stress/d0281/objectless/dgate.sh`): each program `cobc` lowers decided is built twice,
  decided and with COBALTC_DECIDED=0, and both runs must end alike (status, standard output and standard
  error), over two random samples of the 8,854 programs the pathless gate built (Update 139), 1,800 programs: 1,776 the same; 6 no longer decided (they reclaim raw storage in their own code, D-0284 repair 7); 18 different, 16 only in the times they print, one printing the bytes of stack addresses and one a benchmark of random input.
- The compiled conformance suite and the compiled `spec/conformance.md` rows pass under GCC and Clang.
- A driver test: the C of a decided program says `CB_DECIDED`, a function that copies a view and walks it makes
  no object and opens no scope, and the program ends as its first lowering does, under both C compilers.

Measured (best of five, GCC): csvstat 95 → 73 ms, extsort 484 → 431 ms (the first lowering, pathless, against
the second).
