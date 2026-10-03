# D-0116 — The `clone` bound: copying in generic code

Status: ACCEPTED (2026-09-30, the owner: "proceed with F4 as D-0116" — the findings report's F4; D-0090's revisit condition)
Kind: Decision (`spec/02-schema.md`)
Governed by: `CobaltC_Master_Instructions.md` §1, §8, §9, §18
Depends on: D-0090 (built-in bounds), D-0082 (`Vec::clone`), D-0110 (structural key types, the derivation precedent), `spec/07` §1 (`T::drop` recognised by name, the naming precedent), D-0010 (monomorphization)
Affects: `spec/12` §1 (`rule.type.bound`, `[T-Clone]`), `spec/16` §4a (`rule.agg.derived-clone`), `spec/21` §0, `spec/22` §3 and §5, `spec/conformance.md`, the guide (§07, §12, §21, §22 and the omissions list), the shared front end (`derive.rs`, the checker), both tools' `clone` intrinsic, `std`

## Problem

A generic body could not copy a `T`. So every container or helper that
must copy an element took a copying function from its caller:

    fn Lru::get<K, V>(…, fn(ref<V, shared>) : V copy) : Option<V>
    fn union_of<K>(…, fn(ref<K, shared>) : K copy) : HashSet<K>
    Vec::clone_by(&names, String::clone)            // std's own workaround
    fn clone_value(ref<Value, shared> v) : Value    // nine arms by hand, lisp.cb

14 such `copy` parameters and 6 hand-written copiers in the corpus;
`Vec::clone`, `from_slice`, `filled` and `extend_from` refused any
resource element, so a `Vec<String>` could not be copied at all except
by `clone_by`.

## Candidate mechanisms

1. **Keep passing `copy` functions.** Explicit; a parameter on every
   such function; no `Vec::clone` for resources.
2. **A trait system** (`trait Clone`, `impl Clone for T`). D-0090's
   rejected option 2: new keywords, coherence, dispatch.
3. **A fifth built-in bound, `clone`, satisfied by user types through
   two mechanisms the language already has** — a function recognised
   by its name, as `T::drop` is, and structural derivation, as D-0110
   makes a struct of key types a key type. Selected.

## Selected design

- `clone-holds(τ)` (`spec/12`): a plain type (its read is a copy); a
  type `N<…>` with a declared `N::clone` whose bounds its arguments
  meet (`String`, `Rc<T>`, `Box<T: clone>`, `Vec<T: clone>`,
  `HashMap<K: clone, V: clone>`, `HashSet<K: clone>`, a user's own); a
  struct or enum declared without `resource` and without its own
  `clone`, whose every field is `clone` — for which the front end
  derives `N::clone` as ordinary CobaltC (`spec/16` §4a: a field-by-field
  literal, or a `match` over the variants); an array of plain elements.
  Never: a `resource`-marked type without its own `clone` (its
  destructor says only it knows how), `fn` values, handles, mutexes,
  guards.
- One intrinsic, `clone<T: clone>(ref<T, m> x) : T` (`[T-Clone]`): the
  value read through the reference for a plain `T`, else `T::clone`.
  Spelled as an ordinary call, `clone(&x)`, per the method-call decision
  (CHG-0132).
- `clone` is implied by `eq`, `ordered`, `number` and `integer` (all
  their types are `clone`) and implies none of them. The four operator
  bounds are still never satisfied by a user type: `==` on a struct
  would be operator overloading, which stays out.
- `std`: `Vec::clone`, `from_slice`, `filled`, `extend_from` take
  `T: clone` and copy by `clone`, so `Vec<String>` and `Vec<Vec<u8>>`
  are copied like anything else; `Box::clone`, `HashMap::clone`,
  `HashSet::clone` added; `clone_by` stays for a copy that is not the
  type's own.
- Both tools: the checker's `clone-holds` decides at every use, naming
  the field or payload that fails; `coby` and `cobc` reduce the
  intrinsic to a read or to a call of `N::clone` at `N`'s arguments,
  and compile a derived `N::clone` like any function.

## What this crosses

The guide and `spec/22` §5 said no struct or enum satisfies a bound.
That is now true of the four operator bounds only; `clone` is the one
a user type satisfies. D-0090 named this revisit; the means are the
narrowest the language has.

## Compatibility impact

Additive for programs that ran. Programs rejected before are accepted
(`Vec::clone` of a `Vec<String>`); the conformance rows that expected
those rejections now use a `resource` type without a `clone`. A
program's own `fn N::clone` with another shape (a different first
parameter, a different result) is an ordinary function and makes `N`
not `clone` by derivation — the checker says so at the first `clone`
of it.

## Revisit conditions

If `eq` or `ordered` for user types is ever wanted, that is operator
overloading and a new decision, not an extension of this one.
