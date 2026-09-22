# D-0046 — `match` Through a Reference

Status: ACCEPTED (2026-09-26, owner-chosen: option B)
Kind: Decision (`spec/02-schema.md`)
Governed by: `CobaltC_Master_Instructions.md` §1, §9, §17
Depends on: D-0042, D-0045, rule.agg.match
Affects: rule.agg.match (`[Match-By-Ref]`)

## Problem

A `match` bound payloads by value only. A plain payload was copied; a
resource payload (a `Box`, `Vec`, `String`) could be bound only from a
whole owned enum, since binding it through `*r` would move it out of
something borrowed (`[Match-Move-Through-Ref]`). So an enum reached
through a reference — every node of a `Box` tree after the root, an
`Option<String>` field — could not have its payload inspected. A
reference-typed scrutinee type-checked but faulted at run time in both
implementations.

## Candidate mechanisms

A. **Inferred** (Rust's match ergonomics): matching through a
reference binds a reference to a resource payload, a copy of a plain
one. B. **Written**: a scrutinee that is a reference (`match (&e)`,
`match (&mut e)`, a reference-typed expression) binds every payload as
a reference of its mode. **Selected.** C. No change: trees stay in
arenas.

## Selected design

`[Match-By-Ref]`: the arm is chosen by the referent's variant; a binder
is `ref<τi, m>` to the payload, derived from the scrutinee's reference
(its borrow counts as the scrutinee's); nothing is consumed. `match
(e)` and `match (*r)` are unchanged.

## Rejected alternatives

- **A:** what a binder is would depend on whether its payload type is a
  resource, which the reader cannot see at the `match`; and it would
  infer what `foreach` writes (D-0042).
- **C:** leaves `Box` (D-0045) usable for lists and fields but not for
  the trees it is most often used for.

## Semantic rationale

A binder is a reference formed from the scrutinee's place by one
projection, exactly as `&(*r).f` is for a field; every borrow rule
applies to it unchanged (a write through a shared one is
`diag.write-through-shared`; replacing the enum while a payload
reference lives is `diag.aliasing-conflict`).

## Usability

    fn eval(ref<Expr, shared> e) : i64
    {
        match (e)
        {
            Num(n) : *n,
            Neg(x) : -eval(Box::get(x)),
            Add(p) :
            {
                auto pair = Box::get(p);
                eval(&pair.left) + eval(&pair.right)
            },
        }
    }

## Explainability

"`match (&e)` looks at `e` without taking it apart."

## Implementation-feasibility

The typechecker types binders as references; `coby` binds a reference
into the payload (and now resolves a payload step of a path to the
active variant's type and offset, also for an enum on the heap);
`cobc` borrows the payload from the scrutinee's token with a
`CB_PAYLOAD` projection.

## Compatibility impact

Extension: a reference-typed scrutinee faulted before in both tools;
`conformance/12-type-system/type_mismatch_rejected.cb`, which asserted
that fault, now matches a reference to an `i32`.

## Prior-art status

Rust `match &e` (binding modes), OCaml/Haskell (values, no ownership).

## Invariant traceability

`inv.alias-validity`: a binder is an ordinary derived reference.

## Revisit conditions

Nested patterns.
