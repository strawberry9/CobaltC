# D-0095 — Overwriting a live part of an object is refuted statically

Status: ACCEPTED (2026-09-29, owner: "accept it as D-0095"; work begun on the owner's "proceed with D-0095")
Kind: Decision (`spec/02-schema.md`)
Governed by: `CobaltC_Master_Instructions.md` §8, §12, §20
Depends on: D-0049 (`always_owns`), D-0071, D-0080 (`overwrite`), D-0087
Affects: `spec/14` §6 (flow analysis), the shared checker, conformance

## Problem

`[Write-Resource-Overwrite-Rejected]` (`spec/05`) forbids a write that
would lose a live resource. The flow analysis refuted it statically
only for a *whole-object* write (`v = Vec::new();` over a live `v`). A
write to a *part* — `g.start = …`, `t.rows = …` through a reference,
`*s = …` — was always left to the run, although the checker can prove
the old value there in the same way. In the fifth stress round it was
the one recurring ergonomic: two of seventeen programs hit it, both
only when that line first ran; a line on an untested path would carry
the mistake silently.

## The fact relied upon, and its derivation

The refutation needs: *a part of a valid object always holds its
value.* Every way a part of a live object could be empty was checked:

- **Moving a part out** is rejected in safe code
  (`[Store-Binding-Place-Transfer-Sub]`, `diag.move-out-of-field`);
  taking an object apart (`Name { a, b } = x;`) consumes it whole, so
  `valid(x)` is no longer `T`.
- **Moving a payload out by `match`** consumes the scrutinee; through
  a reference it is rejected (`[Match-Move-Through-Ref]`).
- **`drop` of a part** is `diag.move-out-of-field`.
- **Initialization**: a projection write already requires
  `init(x) = T` (`[Write-Partial-Init]`); a part of an initialized
  object is initialized.
- **Trusted raw operations**: `[Rawptr-Move-Out]` (`spec/20`) could
  empty any cells, but its trusted side-condition requires cells "that
  no live fresh-storage object claims", whose obligation "is not
  tracked anywhere else". The cells of a part of a live object are
  claimed, and their obligation is the object's; a program that moves
  them out has made a false trusted claim, and CobaltC guarantees
  nothing about it (Master Instructions §12). So the fact holds for
  every program whose trusted claims are true — the scope of every
  guarantee.
- **Through a reference**: a write needs an exclusive reference; what
  could end or empty its referent (a move, a reallocating push, the
  owner's destruction) would first clash with that reference's live
  borrow, or is the reference's own staleness. A reference binding the
  flow analysis knows valid reaches a live object.

With the fact, the part holds a live resource exactly when its value
owns one — which is certain when every value of its type owns
something (D-0049's `always_owns`). An `Option<String>` part may be
`None`, which owns nothing; that stays the value's to decide.

## Candidate mechanisms

1. **Status quo**: parts are always checked at run time. Simplest; a
   mistake on an untested path survives.
2. **Refute statically wherever the fact is proven, keep the run-time
   check as the backstop.** The same rule, the same diagnostic, the
   same repairs (`overwrite`, `replace`), reported before the program
   runs. No new syntax, semantics or diagnostic. Selected.
3. **Require `overwrite` for every resource part write.** Rejects
   writes over a `None` or a moved-into value that are fine. Rejected.

## Selected design

`¬live-resource-at` for a write to a part `x.π` is refuted statically
when every value of the part's type owns something and either
- `x` is a binding with `valid(x) = T` and `init(x) = T`, or
- the place is reached through a reference binding `r` (`*r.π`, or
  `r.π`, which reaches through `r` implicitly) with `valid(r) = T`;

and every index step of `π` is a literal inside a fixed array's
length. Left to the run, as before:
- a computed index — the part may not exist, and then the write is
  `diag.index-out-of-bounds`, which comes first;
- a `Vec` element — reached through a call (`index_exclusive`), not a
  place the flow analysis tracks;
- a binding that may have been moved (`valid(x) = ?`) — it may be empty;
- a raw pointer's cells — trusted territory with its own rules.

`x = f(x)` is unaffected: the right side moves `x` before the write,
and re-establishing a moved binding is `[Assign-Reestablish]`.

## Found on the way: a checker defect (fixed with this decision)

The first run of every program in the repository under the new rule
rejected a correct showcase program (`tier1/generic_function.cb`). The
cause was older than D-0095: a generic instantiation was recorded under
the function's bare name, so when the program's own `swap<T>` hid
`std::swap` (D-0024), std's internal `swap<Vec<usize>>` instantiation
was checked against the *program's* body, and std's own body at that
type never at all. D-0095 only made it visible (the program's
`*x = *y` is an overwrite at `Vec<usize>`, a type the program never
uses it at). Instantiations are now recorded under the item's
module-qualified key. Pinned by
`conf.own-generic-hiding-std-checked-at-own-types`, which fails
without the fix.

## Compatibility impact

Breaking in principle, as D-0087 was: a program that overwrote a live
part on a path it never took was accepted and ran; it is now rejected.
Every program in the repository (conformance, showcase, examples, all
five stress rounds) behaves as before — each reported diagnostic was
compared with the previous checker's.

## Revisit conditions

If `Vec` elements, computed indices, or maybe-moved roots can be
proven live in some shapes, those shapes can join the static rule by
the same derivation.
