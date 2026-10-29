# CHG-0136 — A generic parameter's concrete type arguments are checked as written

Kind: Change record (`spec/02-schema.md` §2)
Status: ACCEPTED (2026-09-30; a checker conformance bug found while exercising D-0113)
Governed by: `CobaltC_Master_Instructions.md` §13, §19
Depends on: D-0006, D-0012, D-0013, rule.fn.generic-call, rule.type.typing `[T-Call]`
Affects: the shared front end (`impl/src/typecheck.rs`), `spec/conformance.md`

## Problem / motivation

`fn k<B>(P<String, B> p) : B` accepted a `P<u64, i32>` (both tools,
one front end), and the body then read the `u64` field as a `String`.
Likewise `fn f<V>(ref<HashMap<String, V>, shared> m)` took a
`HashMap<u64, i32>` and `fn h<V>(V x, ref<Vec<String>, shared> w)` a
`Vec<u64>`. A non-generic `fn g(ref<HashMap<String, i32>, shared> m)`
rejected the same argument, so the hole was only in inference: the
shape unifier that fixes a call's type arguments bound *every* bare
type name it met in a parameter type — `String`, or a user struct —
as if it were a type parameter, and the exactness check that follows
substituted `String` → `u64` before comparing. `[T-Call]` and D-0006
require the parameter type, with its type parameters fixed, to equal
the argument type exactly.

## What changed

- **Checker:** `unify_type_shape` binds only the names in the
  declaration's own type-parameter list; a concrete name binds nothing
  and is compared as written by the exactness check. Every call site
  (function calls, expected `fn` types, variant construction, struct
  literals) passes its declaration's parameters.
- **Rows:** `conf.generic-concrete-argument-checked`,
  `conf.map-get-str-non-string-key-rejected`.

## Compatibility classification

Conformance fix: programs that were wrongly accepted are rejected with
`diag.type-mismatch` (static), the message naming the argument and the
parameter type. Every valid program is accepted as before; the guide's
examples, the showcase and the round-6 programs run unchanged.
