# D-0190 — The `Vec` items D-0189 left open

Status: ACCEPTED (2026-10-08; the owner: "address the still open issues you identified", the choices delegated as for D-0189)
Kind: Decision (`spec/02-schema.md`)
Governed by: `CobaltC_Master_Instructions.md` §1, §6, §17
Depends on: D-0188, D-0189
Affects: `impl/cobc/src/lower.rs`; `impl/cbrt/src/lib.rs` (`cb_paths_disjoint`); `spec/conformance.md` (3.183.0); `CHG-0218`

## Problem

D-0189 left three `Vec` shapes paying the runtime's tracking: a struct
inside a `Vec` given to a looping function (its path, over an element's
object, was refused by the entry test), `foreach (x in v)` over a `Vec`
parameter (29 ns an element), and `retain`/`position` with a closure that
captures (about 1 µs an element).

## Decisions

1. **A single path over an element's object passes the entry test.**
   `cb_paths_disjoint` refused a path over a reclaimed object because, for
   pairs, a clash with a range of its container is not seen through the two
   paths' projections. A field dual body (D-0189) passes one path: there is
   no pair, and a range of the container overlapping it could not have been
   live when it was formed (`[Borrow-Denied]`,
   `conf.vec-struct-in-vec-slice-held`). With more than one path the refusal
   stands.
2. **The struct's own path is direct in the fast half** when the body
   borrows nothing under it (nor matches on a place under it) and every
   field but the confined vectors is plain: its plain fields are then read
   and written without checks, as a direct parameter's are. A borrowed
   field could be handed on, which needs a real path; such a body keeps
   `b`'s path. (Without this, `b.total += …` in the loop cost two checked
   accesses per element.)
3. **A `foreach` holder is confined before the loop's condition.** The
   holder `foreach_alias` confines for the loop holds the vector from before
   the first step; registering it before the condition is lowered makes
   `$each_len` a plain read as well.
4. **Capturing predicate literals.** `retain` and `position` with a
   closure literal that captures (not the vector's own root) build the
   closure value where the call's argument would be built, borrow it
   exclusively once for the loop (nothing else reaches it meanwhile, as the
   prelude's body's per-call borrows would show), and compile its body with
   `self` and its parameter bound directly: the parameter is used only
   directly, and each element's check (`cb_elem_access`) runs before the
   closure, so a capture holding the element exclusively faults first
   (`conf.vec-capturing-predicate-held-element`). Captured references are
   still read through their own paths.

## Results (ns per element)

Struct inside a `Vec`, field loop with a plain-field update: 615 (Oct 7) →
59 → 2.6; `foreach` over a `Vec` parameter 29 → 1.8; `position` with a
capturing closure 1,100 → 36.

## Not decided here

A closure *value* (not a literal) handed to `retain`, `position` or
`binary_search_by`, and `binary_search_by` itself, still take the
prelude's body: each element a minted borrow. That needs a cheaper element
borrow in `cbrt` (group 3 proper).
