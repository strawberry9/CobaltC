# CHG-0344 — Key types satisfy `eq` and `ordered`; `[Cmp-Key]` (D-0271)

Kind: Change record (`spec/02-schema.md` §2)
Status: ACCEPTED (2026-10-19, with D-0271)
Governed by: `CobaltC_Master_Instructions.md` §8, §12
Depends on: D-0271, D-0110, D-0108, D-0090
Affects: `spec/06` (1.20.0), `spec/12` (1.25.0), `spec/21` (4.71.0), `spec/22` (2.56.0), `spec/conformance.md`
(3.253.0), `spec/02` §5 (1.0.124); `impl/src/typecheck.rs`, `impl/src/interp.rs`, `impl/src/interp/borrows.rs`;
`impl/cobc/src/lower/calls.rs`, `equality.rs`, `operators.rs`; `impl/std/collections.cb` (a comment);
`impl/conformance/12-type-system/` (three files, one amended); the guide (§12, §21, §22's list); the book
(chapter 11)

## What changed

- `spec/06` `rule.arith.cmp`: `[Cmp-Key]` — `<` on a struct or enum key type is `[Sort]`'s order; the
  paragraph on `String` operands covers an operand of a key type that holds one (borrowed, not read).
- `spec/12` `rule.type.bound`: the `eq` and `ordered` rows admit every key type; the closing paragraph says
  a user type satisfies them when it is a key type and never `number` or `integer`.
- `spec/22` §5 restriction 5 rewritten accordingly; `spec/21` `rule.stdlib.hashmap`'s key types exclude a
  struct or enum declared `resource` (a clarification of D-0110); `rule.stdlib.sort` notes that `[Sort]`'s
  order is `<`'s; the hash-table listing's comment names struct and enum key types (stale since D-0110).
- Conformance: eight cases added (`conf.cmp-struct-key` … `conf.bound-ordered-float-field-rejected`);
  `conf.bound-unsatisfied-rejected` and `conf.bound-unsatisfied` use `Option<f64>`;
  `conf.vec-contains-not-eq-rejected` a struct with an `f64` field.
- Implementations: the static pass's `bound_holds` and the `KeyCmp` marker; `coby`'s evaluation by
  `key_cmp`; `cobc`'s lowering to `cb_kc_…`/`cb_eq_…`, and `cb_eq_…` comparing a `String` part by its bytes.

## What changed for programs

A struct or enum of key types can be compared with `<`, `<=`, `>`, `>=` (field by field, as `Vec::sort`
orders it), passed where a `T: eq` or `T: ordered` is wanted (`Vec::contains`, `index_of`, `dedup`, `max`,
`min`, a program's own generics), and compared with `==` when it holds a `String`. Nothing accepted before
changed meaning. A type with a float field remains outside all of this, and the message names the field.
