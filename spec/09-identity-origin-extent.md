# CobaltC Identity, Origin, and Extent: References

Status: normative artifact
Version: 1.5.0
Conforms to: `spec/02-schema.md` (Kind: Type `type.ref`; Kind: Rule
`rule.ref.*`)
Governed by: `CobaltC_Master_Instructions.md` §17 (Identity, Origin,
and Spatial Validity), §23
Realizes: D-0017 (`type.ref`), D-0018

## Purpose

Introduces references: the value-level form of an access path. A
reference is exactly an `AccessPathToken` reinterpreted as a value;
forming one is `rule.alias.borrow`, using one delegates to
`rule.value-object.read`/`write`. Raw pointers are a separate
mechanism (`spec/20` §2).

## 1. `type.ref`
**Status:** ACCEPTED

`type.ref<τ, m>` for pointee type `τ` and `m ∈ Mode`. Represented
domain: access-path tokens `a` with `type(a) = τ` and `mode(a) = m`.
`is-resource(type.ref<τ,m>) = false`: a reference holds no destroy
authority and is freely copyable; copying it (`[Read]`) yields the
same token, and storing the copy records another holder
(`rule.value-object.write`). Distinct `(τ, m)` are distinct nominal
types (D-0006). `sizeof`/representation: `spec/06` §7.

**Depends on:** term.access-path, term.object, term.type,
inv.alias-validity, D-0006

## 2. Formation and dereference

### `rule.ref.form`
**Status:** ACCEPTED

    [Ref-Form]   disposition: checked
        ⟨e, Σ⟩ →* ⟨place a0, Σ1⟩                -- e in place position (spec/13 §1)
        ⟨borrow(a0, m), Σ1⟩ →^ℓ ⟨a, Σ2⟩          -- rule.alias.borrow
        ────────────────────────────────────────────
        ⟨&_m e, Σ⟩ →^ℓ ⟨a : type.ref<type(a0,Σ1), m>, Σ2⟩
        side-conditions: inherited from rule.alias.borrow (its own discharge targets)

    [Ref-Form-Not-Place]   disposition: rejected
        e is not a place expression (spec/13 §1: not a name, projection, *e', or reclaim)
        ────────────────────────────────────────────
        ill-formed; diag.borrow-of-non-place

    [Ref-Form-Temporary]   disposition: rejected
        e is a place expression whose root is a temporary (a projection of an aggregate literal
        or call result, spec/16 [Temp-Root]) -- not one reached through a reference, and not
        an argument ([Ref-Form-Temporary-Part-Argument]): for a call
        result of reference type, `f().x` is `(*f()).x` (spec/16 [Field-Access-Auto-Deref]), a
        place through that reference, and `&f().x` borrows it
        ────────────────────────────────────────────
        ill-formed; diag.borrow-of-temporary

    [Ref-Form-Temporary-Argument]                                     -- D-0073, D-0084
        &_m e an argument of a call whose result type is not a reference, e of non-reference
        type and one of: a call, an aggregate literal (spec/16 §2–§3), an operator
        expression (a unary or binary operator, spec/13), `e'?` (spec/18), a number
        or `bool` literal, or a variant without a payload (`None`, `E::V`; CHG-0130)
        ⟨e, Σ⟩ →* ⟨r, Σ1⟩ — a literal e at the referent type τ when the argument's expected
            type is ref<τ, _> (rule.type.expected);  o = the object r is (a value r is
            established as a temporary o)
        ────────────────────────────────────────────
        ⟨&_m e, Σ⟩ →^ℓ ⟨a : type.ref<type(o), m>, Σ2⟩   a borrowed from o's root path; o a
                                                         temporary of the current statement

    [Ref-Form-Temporary-Part-Argument]                                -- D-0103
        &_m e (or &_m e[lo .. hi]) an argument of a call whose result type is not a reference,
        e a projection (fields, elements) of a temporary root o (spec/16 [Temp-Root]), or, for
        a slice, e itself such a temporary (an array literal, a call's result)
        ────────────────────────────────────────────
        the borrow (or slice) is formed as for a place; o is a temporary of the current
        statement and ends at its exit, after the call; a view of a String (`&s[lo .. hi]`,
        spec/21 [View-Form]) the same (spec/21 [View-Form-Temporary-Argument], D-0135)

`&e` is `&_shared e`; `&mut e` is `&_exclusive e` (`spec/22` §2).
Borrowing a temporary (`&make()`) is rejected rather than given a
lifetime: the temporary would end at the statement's end (D-0019)
and the reference with it; bind it first. The one exception is an
argument (`[Ref-Form-Temporary-Argument]`, D-0073): the call returns
before its statement ends, so `printf("%v", &describe(e))` and
`total(&make(10))` need no binding. The same holds for an operator's
result, a `?`'s payload and a literal (D-0084): `HashMap::get(&m, &(k <<
32 | c))`, `Vec::len(&names()?)`, `HashMap::get(&m, &3)`. A literal has
no type of its own to take, so it is typed as the parameter's referent
(`&3` is a `ref<u64, shared>` where a `ref<u64, shared>` is expected);
outside an argument, `&5` is not a place and is rejected. A result that is itself a reference
is excluded. A reference that a returned aggregate holds, if it came
from the temporary, is stale after the statement, and its use is
`diag.stale-binding` (dynamic).

**Depends on:** rule.alias.borrow, type.ref

### `rule.ref.deref`
**Status:** ACCEPTED

    [Ref-Deref-Place]
        ⟨e, Σ⟩ →* ⟨v, Σ1⟩,  v = a : type.ref<τ, m>       -- e read to its ref value
        ────────────────────────────────────────────
        ⟨*e, Σ⟩ →^ℓ ⟨place a, Σ1⟩

`*e` is a place expression (`spec/13` §1): in value position it is
read by `[LValue-To-RValue]` (`rule.value-object.read` through `a`),
as the target of `=` it is written (`rule.value-object.write` through
`a`, which requires `m = exclusive`), as the operand of `&`/`&mut` it
is reborrowed (`rule.alias.borrow` from `a`), and as the base of `.f`
or `[i]` it is projected. Every validity check — `alive`,
`temporally-valid`, `init-state`, mode, `clash` — is performed by the
delegated-to rule; a reference neither weakens nor bypasses any of
them.

**Depends on:** rule.value-object.read, rule.value-object.write,
rule.alias.borrow, type.ref

## 3. Identity comparison

### `rule.ref.identity-eq`
**Status:** ACCEPTED

    [Ref-Identity-Eq]
        r1 : type.ref<τ, m1>,  r2 : type.ref<τ, m2>
        ────────────────────────────────────────────
        ⟨r1 == r2, Σ⟩ → ⟨of(r1,Σ) = of(r2,Σ) ∧ target(r1,Σ) = target(r2,Σ), Σ⟩

Two references are equal iff they denote the same sub-range of the
same object (`spec/AUDIT-2.md` B-16: `&p.a == &p.b` is `false`). This
is `=_τ` for reference types (`rule.type.eq`), reached through
`rule.arith.cmp`'s `==`/`!=`; there is no separate operator. Modes may
differ; the comparison is about identity, not capability.

**Depends on:** inv.identity, rule.type.eq

## Change Log

- 1.5.0 — `CHG-0162` (D-0135): `[Ref-Form-Temporary-Part-Argument]`
  covers a view of a temporary `String` as it covers a slice.
- 1.4.0 — `CHG-0130`: `[Ref-Form-Temporary-Argument]` names a variant
  without a payload, which the checker admitted and both runtimes then
  failed on (`coby` `diag.borrow-of-temporary`, `cobc` an internal error).
- 1.3.0 — `CHG-0122` (D-0103): `[Ref-Form-Temporary-Part-Argument]`: a
  field, element or slice of a temporary may be borrowed as an argument.
- 1.2.0 — `CHG-0100` (D-0084): `[Ref-Form-Temporary-Argument]` admits an
  operator expression, `e?` and a number or `bool` literal, the literal
  typed as the expected referent.
- 1.1.0 — `CHG-0086` (D-0073): `[Ref-Form-Temporary-Argument]`.

- 1.0.1 — `[Ref-Form-Temporary]` states that a projection through a
  reference-typed call result (`&f().x`, `[Field-Access-Auto-Deref]`) is
  not a projection of a temporary; editorial, it follows from spec/16.
- 1.0.0 — Rewritten per D-0018/D-0019 (`spec/AUDIT-2.md` B-16, B-19):
  `[Ref-Deref-Place]` makes `*e` a place expression so reborrow,
  assignment, and projection through a reference are all defined;
  `[Ref-Identity-Eq]` compares object and target and is `==` on refs;
  `[Ref-Form-Not-Place]` rejects borrowing a temporary. All entities
  `ACCEPTED`.
- 0.2.0 and earlier — superseded.
