# CHG-0014 — `spec/21` Library Derived Statement by Statement

Kind: Change record (`spec/02-schema.md` §2)
Status: ACCEPTED
Governed by: `CobaltC_Master_Instructions.md` §20, §21
Depends on: rule.init.let, rule.resauth.relocate-out, rule.resauth.destroy, rule.type.typing, rule.stdlib.vec, rule.stdlib.string, rule.stdlib.rc, rule.trust.rawptr
Affects: conf.string-into-bytes-roundtrip, conf.map-err-transforms, conf.rc-clone-shares-allocation, conf.rc-last-drop-frees

## Problem / motivation

Derive every `spec/21` library function as a program in
its own right, including every branch, and write down every `unsafe`
block's trusted condition and why it holds.

## What was derived

- **`Vec::grow` with `cap > 0` and a live element reference across
  it:** already fully derived by `CHG-0012`'s
  `conf.e2e-vec-realloc-stale-ref` (the second push inside
  `hold_across_grow` triggers exactly this branch, with `r` live
  across it). No new artifact needed; cited here for completeness.
- **`String::into_bytes`/`String::len`:**
  `conf.string-into-bytes-roundtrip` added. `into_bytes`'s `String{bytes} = s;` exercises
  `[Let-Destructure]`'s resource-field branch
  (`rule.resauth.relocate-out`) on a by-value parameter, then destroys
  the now-emptied container with `[Run-Destructor-None]` (`String` has
  no user `drop`). No gap.
- **`Rc::new`/`clone`/`get`/`drop` with a resource `T`** (already
  covered by `conf.e2e-rc-resource-payload`,
  `conf.e2e-closure-move-rc`) **and a plain `T`:** `conf.rc-clone-shares-allocation`/
  `conf.rc-last-drop-frees` re-derived with the `[Rawptr-Write]` (not
  `[Rawptr-Move-In]`) construction and `[Destroy-Plain]` (not
  `[Destroy]`) final drop made explicit — for a plain `T`, `Rc::new`
  writes the box without establishing any object at all; the first
  `reclaim` call anywhere (here, `Rc::clone`'s) is what first
  establishes it, with no authority or obligation granted
  (`¬is-resource(RcBox<T>)`). No gap.
- **`map_err`:** `conf.map-err-transforms` added, deriving both the
  static typing (`[T-Match]` over the two arms, `[T-Call]` on the `fn`
  parameter via `callable`) and a concrete evaluation. No gap.
- **Every `unsafe` block in `Vec`/`Rc`** (`String` has none): written
  down in `spec/AUDIT-STATUS.md`'s seventh-pass table, one sentence
  each.

## Rule changes

None for the items above. One finding did not get a rule change:

### F-05 (not fixed — escalated)

Deriving `Vec::pop` immediately followed by `Vec::push` at the same
index (reached through a reference parameter, so the same-function
static front line `conf.vec-ref-then-push-rejected` uses does not
apply) exposed that a live `Vec::index_shared` reference into a
*plain*-typed element survives `pop` (`[Rawptr-Read]` does not end the
reclaimed object, correctly, since reading owns nothing) and is then
silently overwritten by the next `push`'s `[Rawptr-Write]`, whose
trusted condition explicitly exempts `reclaimed-at(addrs)` from its
"no live derived path reaches them" requirement. No diagnostic, static
or dynamic, catches it — unlike a resource-typed element, where
`Vec::pop`'s `[Rawptr-Move-Out]` unconditionally ends the source, so
the same reuse is caught as `diag.stale-binding` exactly like
`CHG-0012`'s finding. This is not completing an incomplete rule; it is
a question of what the raw-pointer trust boundary (D-0017) is supposed
to guarantee when an address is reused without deallocation, and the
candidate fixes touch either a foundational rule (`[Rawptr-Write]`'s
condition) or the library's own contract (`Vec::pop`'s plain-element
path) in ways a designer, not a mechanical completion, should choose
among. Recorded with three candidate directions and no selection in
`spec/AUDIT-STATUS.md`'s seventh-pass section, per
`CobaltC_Master_Instructions.md` §3's instruction to write up rather
than patch this class of finding unilaterally. (Later closed by the
human owner: `CHG-0019`.)

## Affected invariants

None restated (F-05 is recorded, not fixed).

## Dependency impact

None on existing entities.

## Compatibility classification

The four conformance changes are purely additive/enriching (no
outcome changed). F-05 changes nothing (not fixed).

## Migration implications

None.

## Example changes

None (all four `spec/21` items were derived as conformance rows, not
new examples — none is a feature-interaction end-to-end program).

## Conformance changes

`conf.string-into-bytes-roundtrip`, `conf.map-err-transforms` added;
`conf.rc-clone-shares-allocation`, `conf.rc-last-drop-frees`
derivations enriched (`spec/conformance.md` 3.9.0). No outcome of any
row changed.

## Future implementation implications

None beyond what `spec/21`'s existing library bodies already require.
F-05, if resolved by tightening `[Rawptr-Write]`, would require an
implementation's `Vec::pop` to additionally release or invalidate a
plain element's reclaimed object; if resolved by widening the
documented hazard instead, no implementation change follows.

## Prior-art status

Not applicable (a Change record).

## Revisit conditions

F-05 needs a human decision among its three candidate directions (or a
fourth) before any rule or library body changes.
