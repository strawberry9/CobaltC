# CHG-0415 — A literal where a shared slice is declared is a slice of static data; a place rooted at a constant is borrowed from it

Status: implemented (2026-11-07), D-0314
Affects: `spec/12` 1.29.0 (`rule.type.coerce-slice`: `[Coerce-Slice-Literal]`, `[Coerce-Slice-Literal-Generic-Disagree]`);
`spec/09` 1.9.0 (`[Ref-Form-Const-Place]`); `spec/17` 2.20.0 (§1a); `spec/22` 2.61.0 (§1, `byte-literal`); `spec/08`
1.8.1 (§2's prose); `spec/conformance.md` 3.298.0; `coby` (`consts::places`, `typecheck::coerce_text`, the array
literals' expected element type, `crosses_shared`, `views::slice_literal`, `lib.rs`); `cbc` (`consts::places`,
`consts::Folded`, `coerce_text`, `check_array_lit`, `check_array_repeat`, `crosses_shared`, `rw_expr`,
`slice_literal_disagreed`); `cobc` through `coby`'s front end

## What changes

1. **`consts`** (both front ends): every borrow of a place rooted at a literal constant (`&K[lo..hi]`, `&K[i]`,
   `&K.f`, …) has its root written `(*&K)`. In `cbc` the set of such constants is kept with the folded literals
   (`Folded`) for the bodies of `std` parsed later.
2. **The static pass**: an array literal or repeat checked where a `slice<τ, shared>` is expected takes `τ` as its
   elements' expected type; `coerce_text` records `Rewrite::SliceLiteral` for one written out as literals of a plain
   `τ`, with its array type (`slice_literals`), and a node two instantiations type apart is
   `[Coerce-Slice-Literal-Generic-Disagree]`.
3. **The rewrite** (`views.rs`, `cbc`'s `rw_expr`): the literal moves into a root constant `__slice_literal_H` (`H` the
   64-bit FNV-1a hash of the type's text and a canonical text of the value; equal literals share one), and the node
   becomes `&(*&K)[0..$]`; the program is checked again. `cobc` lowers the constant as any literal constant borrowed:
   one `static const` object.
4. **`crosses_shared`** (both front ends) looks through `*&x` (a shared crossing) and `*&mut x` (what `x` is).

## Conformance

`conf.coerce-slice-literal`, `conf.coerce-slice-literal-exclusive-rejected`, `conf.coerce-slice-literal-not-literal-rejected`,
`conf.coerce-slice-literal-generic-disagree-rejected`, `conf.const-place-borrow`, `conf.const-place-exclusive-rejected`,
`conf.borrow-exceeds-source-through-shared-borrow`. Each passes under `coby`, `cobc` (gcc and clang) and `cbc`; `cbc`
and `cobc` write the same C for the positive ones, with and without `--all-checks`.
