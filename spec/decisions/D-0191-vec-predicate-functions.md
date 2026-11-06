# D-0191 — Predicates that are functions and `fn` values

Status: ACCEPTED (2026-10-08; the owner: "address the still open issue", the choices delegated as for D-0189)
Kind: Decision (`spec/02-schema.md`)
Governed by: `CobaltC_Master_Instructions.md` §1, §6, §17
Depends on: D-0189, D-0190
Affects: `impl/cobc/src/lower.rs`; `impl/cbrt/src/lib.rs`, `impl/cbrt/include/cbrt.h`; `spec/conformance.md` (3.184.0); the guide; `CHG-0219`

## Problem

After D-0190, `retain`, `position` and `binary_search_by` ran as plain loops
for closure literals only. A named function (`Vec::retain(&mut v, is_even)`),
a `fn` value in a local, and `binary_search_by` in any form still took the
prelude's body, which mints a borrow of each element for the call: about
1.1 µs an element, 9 µs a lookup.

## Decisions

1. **Named functions.** A predicate that names a non-generic function whose
   reference parameters are all pure direct (`Gen::pure_direct`: read, never
   kept or handed to anything that keeps them) is called with bare element
   addresses, as a direct call to it would be.
2. **`fn` values.** A function whose reference parameters are all pure direct
   (and that has no slice parameter) gets a marked handle
   (`cb_fn_item_pure`). A `fn` value in a local, read once, is tested at the
   call (`cb_fn_pure`): a marked handle runs the plain loop through its
   thunk; any other (a closure value, which may keep what it is given) runs
   the prelude's function, called as the call would call it. Which branch is
   taken is unobservable: both make the same checks in the same order, and
   only the second forms paths a kept reference needs
   (`conf.vec-predicate-value-keeps-element`).
3. **`binary_search_by`** joins `retain` and `position`: with a literal, a
   named function or a `fn` value, each element borrowed for its call
   (`cb_elem_access`) and the key read as the body reads it. For a literal or
   a named function the key reaches only parameters that read it: a place is
   passed by address after its borrow's check, a temporary as its value's
   address.

## Results

`retain` with a named function 1,123 → 3.4 ns an element, with a `fn` value
holding one 1,129 → 8.5; `binary_search_by` 9,164 → 388 ns a lookup (the rest
is the key local's object when it is borrowed).

## Not decided here

A closure value that captures, stored in a variable, keeps the prelude's
path (~1.1 µs an element): its thunk could keep what it is given, and only a
cheaper element borrow in `cbrt` would help it.
