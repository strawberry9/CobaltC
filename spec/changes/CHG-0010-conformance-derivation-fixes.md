# CHG-0010 — Fixes from the Per-Case Derivation of the Conformance Suite

Kind: Change record (`spec/02-schema.md` §2)
Status: ACCEPTED
Governed by: `CobaltC_Master_Instructions.md` §19, §20, §21
Depends on: inv.temporal-validity, inv.resource-authority, inv.alias-validity, inv.trust-transition, inv.arith.range-validity
Affects: rule.type.typing, rule.type.expected, rule.arith.convert, rule.arith.literal, rule.arith.sizeof, rule.arith.represent, rule.control.flow-analysis, rule.fail.propagate, rule.conc.lock, rule.trust.rawptr, rule.expr.context, rule.resauth.destroy

## Problem / motivation

`spec/AUDIT-2.md` §5 step 6 required every conformance case to carry
a written derivation; `spec/AUDIT-STATUS.md` records that this was
done at 2.0.0. Re-deriving all 112 rows against the rules as they
stand after `CHG-0009` (2026-09-20) showed that fourteen derivations
cited a rule that did not exist, did not apply to the fragment, or was
contradicted by another rule. None of the fragments' expected outcomes
was wrong; the rules were incomplete beneath them. Each gap is closed
here toward the outcome the suite already states.

## Affected entities and what changed

### D-01 · `rule.type.expected`, `rule.arith.literal` (`spec/12` 1.5.0, `spec/06` 1.2.0)
**Previous:** no expected type for a shift amount or an index; a
literal there defaulted to `i32` and `[T-Shift]`/`[T-Index]`
(requiring `u32`/`usize`) rejected `x >> 1` and `a[2]`. Expected
types did not propagate through `if`/`match`/blocks, so `usize
new_cap = if (…) { 4 } else { … }` in `spec/21` was ill-typed.
**New:** `u32`/`usize` at those positions; propagation through `if`
branches, `match` arm bodies, block trailing expressions, and `(e)`.
Rows: `conf.shr-arithmetic`, `conf.index-*`, `conf.vec-push-len`.

### D-02 · `rule.type.typing` `[T-Block]`, `[T-Let-Destructure]` (`spec/12` 1.5.0)
**Previous:** a block with no trailing expression was `unit` even
when its last statement was `return …;`, so `String::from_utf8`'s
`else { return Err(…); }` branch mismatched its `usize` siblings; no
typing rule covered `Name{f} = e`.
**New:** such a block is `never`; `[T-Let-Destructure]` added.
Rows: `conf.string-from-utf8-*`, `conf.let-destructure`.

### D-03 · `rule.arith.convert` `[Narrow-*]` (`spec/06` 1.2.0)
**Previous:** required equal signedness; `narrow<u8>(x : i32)` had no
rule. **New:** any integer pair to which `[Widen]` does not apply.
Row: `conf.narrow-overflow`.

### D-04 · `rule.arith.sizeof`, `rule.arith.represent` (`spec/06` 1.2.0)
**Previous:** `sizeof(mutex<τ>) = AddrWidth/8`, contradicting
`spec/19`'s layout (an `inner` plus `usize` of state); no `represent`
case for `handle`, `guard`, or `mutex`, though `[Spawn]` and `[Lock]`
write them and `[Join]`/`[Guard-Deref]` read them.
**New:** `[Sizeof-Mutex]`, `[Repr-Handle]`, `[Repr-Guard]`,
`[Repr-Mutex]`. Rows: `conf.spawn-join-value`, `conf.mutex-*`.

### D-05 · `rule.control.flow-analysis` (`spec/14` 1.3.0)
**Previous:** the consuming-use transfer row omitted `match (x)`,
`Name{f} = x`, `*p = x`, and payload initializers, so after
`match (e)` on a resource enum `valid(e)` stayed `T`, a later use of
`e` was "proven" valid, and its runtime check was elided — a real
soundness hole in the static/dynamic boundary. No discharge row
existed for `¬live-resource-at`, which `[Write]` cites.
**New:** both rows added. Rows: `conf.match-resource-payload-
transfers`, `conf.overwrite-live-resource-rejected`.

### D-06 · `rule.fail.propagate` (`spec/18` 1.2.0)
**Previous:** `[Propagate-Ok]` reduced `e?` only when the discriminant
was `Ok`; on `Err` no rule applied. **New:** `[Propagate]`, either
variant. Row: `conf.propagate-err`.

### D-07 · `rule.conc.lock` `[Guard-Deref]`, `rule.expr.context` (`spec/19` 1.4.0, `spec/13` 1.5.0)
**Previous:** `*g` read `g` — a resource — as a value, which
`[Read-Resource-Rejected]` forbids; the same for `*lock(&m)`.
**New:** the operand of `*` is a place position when its type is
`guard<τ>`; `[Guard-Deref]` takes a place or temporary guard. Rows:
`conf.mutex-lock-unlock`, `conf.mutex-shared-across-threads`.

### D-08 · `rule.resauth.destroy` §1 (`spec/07` 1.2.0)
**Previous:** `destructor(τ)` was defined only as a `τ::drop` item;
`[Run-Destructor]` could not reach the built-in destructors of
`handle`/`mutex`/`guard`. **New:** defined for them. Rows:
`conf.unjoined-handle-waits`, `conf.mutex-lock-unlock`.

### D-09 · `rule.trust.rawptr` `[Rawptr-Read]`/`[Rawptr-Write]` (`spec/20` 1.4.0)
**Previous:** trusted conditions "no live path … conflicts"/"no live
path reaches them other than through reclaimed-at" were false for
every raw access to a local's cells, since the local's own root path
always exists — including `conf.rawptr-deref-inside-unsafe-ok`.
**New:** stated over derived paths (`base ≠ None`) and concurrent
threads; a root path does not count. What an author asserts is
narrowed to what the pattern actually needs.

### Derivation-text corrections (no rule involved)
`conf.let-copy` and `conf.struct-copy` cited
`[Store-Binding-Place-Copy]` for an initializer in value position; the
path is `[LValue-To-RValue]` then `[Store-Binding-Value]`.
`conf.vec-drop-destroys-elements`'s outcome text mis-ordered
`deallocate` relative to the outer object's end. Several rows now
name the rule steps this pass had to supply.

## Affected invariants

D-05 restores the guarantee `inv.temporal-validity` relies on from
`rule.control.flow-analysis` — that "proven" means proven — for
consumed scrutinees, destructured containers, and raw-written
sources. D-09 narrows a `discharge: trusted` assertion under
`inv.trust-transition` to what the rules can honor. The rest complete
static typing beneath rules whose dynamic semantics were already
sound.

## Dependency impact

No `Depends on`/`Affects` line changes: every rule touched already
depended on the entities its new text names.

## Compatibility classification

Semantics-completing, not source-breaking: every affected program was
ill-formed or underivable by omission and is now well-formed with the
outcome the suite states. D-03 admits `narrow` between signednesses
that previously had no rule. No previously well-formed program changes
meaning.

## Migration implications

None.

## Example changes

None; `ex.bounds-check`, `ex.threads`, `ex.unsafe-required`,
`ex.resource-payload` now derive as written.

## Conformance changes

`spec/conformance.md` 3.5.0: derivations rewritten where this record
supplies the rule; no fragment or outcome changed.

## Future implementation implications

A type checker implements the two expected-type positions, expected-
type propagation through `if`/`match`/blocks, the `never` block rule,
and destructure typing; a flow analysis treats the listed consuming
uses uniformly; `narrow` is a checked conversion between any two
integer types; a mutex is laid out as a two-field struct.

## Prior-art status

Not applicable (a Change record).

## Revisit conditions

If a full derivation pass is repeated after further rule changes,
re-check D-05's transfer row against every rule that ends or moves an
object: the row is a closed list and must be kept in step with them.
