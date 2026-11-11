# CHG-0093 — Third stress round: fixes

Kind: Change record (`spec/02-schema.md` §2)
Status: ACCEPTED (2026-09-28, owner-delegated)
Governed by: `CobaltC_Master_Instructions.md` §12, §19, §21
Depends on: CHG-0092
Affects: rule.fn.closure, rule.type.expected, rule.conc.spawn, rule.conc.join; the `cbrt` runtime

## Problem / motivation

Defects found by the third round of stress programs.

## What changed

- **A captured name keeps its type in the closure body (shared
  checker).** The body was checked with every capture untyped, so an
  operand like the `10` in `u8 t = e + 10;` took the default `i32`, and
  a valid program was rejected with `diag.type-mismatch`. The captured
  name now has its binding's type: it denotes the captured place
  (`spec/15` `[Closure-Call]`), whatever the capture mode. No spec text
  changes, since the spec already said this.
- **`spawn` and `join` are typed (shared checker).** `spec/19`'s
  `[Spawn]` and `[Join]` give `spawn(f, …) : handle<R>` and
  `join(handle<R>) : R`. The checker typed neither, so `2 + join(h)`
  took `2` as an `i32` and rejected a valid program. `i64 x = join(h)`
  for a `handle<u32>` also went unreported. Both calls are now typed as
  the rules say.
- **Calls of closures and fn values type their literal arguments.**
  The checker checked a closure's or fn value's arguments without the
  parameter types, so `f(5000000000)` for `f : fn(i64) : i64` was
  rejected as out of range for an `i32`. `coby` evaluated them the same
  way, so for `[](u8 a)`, `f(9) + 250` computed 259 in `i32` where `cobc`
  faulted `diag.arith-overflow`. Both now pass each parameter's type
  down, as for a named function.
- **A write to a captured name inside the closure body no longer
  starts flow tracking there (shared checker).** `x = e` re-validates a
  whole binding (D-0033), and a captured name counted as one, so
  `[n](u32 k) { n += k; n }` rejected its last `n` as
  `diag.use-of-uninitialized`.
- **Messages.**
  - `foreach (x in &f())` names the form that iterates a call's
    result, `foreach (x in f())`.
  - A statement starting `Mutex<`, `Array<`, `Handle<`, … names the
    lower-case built-in type (`mutex<…>`, with `Mutex::new` as the
    function that makes one).
  - A non-key type given to `HashMap`/`HashSet`, `Vec::sort` or
    `Vec::binary_search`, a non-printable one given to
    `String::append`, and a non-number given to `parse` now say which
    type and what is allowed (they were bare `diag.type-mismatch`).
  - `diag.ambiguous-name` lists the qualified candidates (`store::summary`
    and `report::summary`; `a::Light::Red` and `b::Flag::Red`).
- **`cbrt`: an unheld path ends when its last holder lets go, whoever
  that holder is.** CHG-0086 applied D-0018's rule only when a
  holder's storage was released (`forget_slots`). Re-pointing a
  reference variable (`cur = Vec::index_shared(&cur.kids, i)` walking
  down a tree) left the old path on its object's list, so every later
  check of that object scanned one more path. A B-tree whose searches
  did this ran three times slower after 20,000 operations than after
  1,000. `drop_occ` now applies the rule for every holder, and
  re-storing the token a slot already holds is a no-op.
- **`cbrt`: one object per element's cells.** A value moved into
  raw storage (`[Rawptr-Move-In]`, e.g. `Vec::push`) keeps its identity
  but has no root path. `[Reclaim]` and the native element borrow
  treated such an element as having no object, so they established a
  second object over the same cells and orphaned the first (alive and
  unreachable until exit). One helper, `live_reclaimed`, now recognizes
  it at all three sites, so the existing object gets a root path when
  the program takes one. Accesses to such an element through
  `cb_elem_access` are now checked against that object's paths, as
  every other element's are.
- **`cbrt` test hook:** with `COBALTC_RT_STATS` set, a program that ends
  normally reports how many objects and paths the runtime still
  records. `cobc/tests/runtime_bookkeeping.rs` checks that both stay
  near zero after thousands of tree walks and element reads.

## Compatibility classification

Fix: valid programs that were rejected are now accepted, and
`i64 x = join(h)` for a `handle<u32>` is now rejected, as `[Join]`
always required. The runtime fixes change no program's behaviour, only
its time and memory.

## Conformance changes

**Added:** `conf.closure-capture-keeps-its-type`, `conf.join-result-typed`,
`conf.join-result-type-mismatch`, `conf.capitalized-builtin-type`,
`conf.foreach-borrowed-temporary`.

## Revisit conditions

None.
