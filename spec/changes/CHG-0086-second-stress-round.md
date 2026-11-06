# CHG-0086 — Second stress round: library additions, temporaries as arguments, and fixes

Kind: Change record (`spec/02-schema.md` §2)
Status: ACCEPTED (2026-09-28, owner-delegated)
Governed by: `CobaltC_Master_Instructions.md` §8, §9, §12, §19, §21
Depends on: D-0073
Affects: rule.ref.form, `[T-Closure]`, `spec/21` §0, the implementations

## Problem / motivation

D-0073's frictions, and these implementation faults found by the same
programs:

1. **`&f()[lo .. hi]` was rejected** as `diag.borrow-of-non-place`, for
   `f` returning a reference, while `&f()[i]` and `&(*f())[lo .. hi]`
   were accepted.
2. **`cbrt` leaked paths.** A path whose last holder let go outside any
   statement scope stayed on its object's list. A loop borrowing into
   one array (`h[k] += 1` with `k` read from a `Vec`) made every check
   scan all earlier paths: quadratic time, 7.8 s for 20,000 records.
3. **Closures:**
   - **coby:** a closure passed directly as a `fn(…)` argument, or kept
     in a struct field, a `Vec` or a `Box`, could not be called
     (`diag.type-mismatch`).
   - **coby:** a generic `T` fixed only by a `fn(…) : T` argument was not
     inferred, so `map_all(&es, get)` gave `Vec` elements of 0.
   - **The checker** treated a closure's type as unknown, so a closure
     whose body gave a `bool` was accepted as a `fn(u32) : u32` and gave
     a `bool` (coby) or 0 (cobc) where a `u32` was declared.
   - **cobc** could not infer a generic call's type argument from a
     closure literal.

## Decision

D-0073, and each tool brought to the specification.

## What changed

- **`spec/09` 1.1.0:** `[Ref-Form-Temporary-Argument]`.
- **`spec/12` 1.13.0:** `[T-Closure]` with an expected `fn` type.
- **`spec/21` 3.31.0:** the new `std` functions.
- **`spec/conformance.md` 3.69.0:** the cases below.
- **`spec/02-schema.md` 1.0.59:** §5's "in use" ranges.
- **Implementations:**
  - **The checker:** temporary borrows as arguments (`temp_borrows`);
    closure types (`fn(P…) : R`, checked against the expected type,
    recorded in `closure_types`); `fn` types unified in inference; the
    slice base through a reference-returning call; the messages for
    `==` on `String`, a variant with two payloads, and a temporary
    borrowed elsewhere.
  - **`coby`:** a temporary borrow is a statement temporary borrowed
    from its root. A closure bound at a declared `fn` type keeps its
    identity. A closure stored as a value is a callable box
    (`Value::Closure`), kept aside in raw storage. A closure's result
    type is used when its body runs. Type parameters are inferred from
    function and closure signatures.
  - **`cobc`:** a temporary borrow through `cb_temp_path`; a closure
    argument unified by its signature and boxed.
  - **`cbrt`:** a path no holder, scope or flight keeps is retired when
    its last holder ends; returned and borrowed references in flight
    hold their paths.
  - **`std`:** `Vec::insert`/`remove`, `Option::is_some`/`is_none`,
    `Result::is_ok`/`is_err`, `String::eq`/`eq_str`.

## Compatibility classification

Extension, and fixes. The static closure check rejects programs whose
closures give another type than their `fn` type. Those programs ran
wrongly before: a `bool` where a `u32` was declared.

## Conformance changes

**Added:**
- `conf.temp-borrow-argument`, `conf.temp-borrow-binding-rejected`;
- `conf.closure-result-checked`, `conf.closure-in-vec-callable`,
  `conf.closure-generic-inferred`;
- `conf.slice-through-returned-ref`;
- `conf.vec-insert-remove`, `conf.option-result-predicates`,
  `conf.string-eq`, `conf.string-eq-operator-rejected`.

## Revisit conditions

- A `fn` value whose closure owns a resource: `coby` moves it, `cobc`
  faults when copying it (`diag.read-of-resource`). Whether such a value
  is copyable, movable or rejected is open; `spec/15` says a function
  value is plain and copyable.
- A boxed closure is not destroyed with the value that holds it
  (`coby`).
