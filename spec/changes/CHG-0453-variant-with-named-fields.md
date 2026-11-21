# CHG-0453 — A variant with named fields; a struct pattern through a reference (D-0324)

Status: implemented (2026-11-14), D-0324, D-0296; Update 189
Affects: `impl/src/ast.rs` (`VariantDecl::fields`, `shown_type_name`), `impl/src/parser.rs` (`parse_enum`),
`impl/src/modres.rs` (`Resolver::field_variants`, `fields_variant`, `fields_variant_arm`, the struct literal
in `walk_expr_untagged`, the destructuring statement), `impl/src/typecheck.rs` (`check_call_in`'s
construction, `check_match`'s chain, `pattern_missing`, `is_fields_struct`, the `Destructure` arm,
`Rewrite::PatternFields`, `Body::derived`), `impl/src/views.rs` (`blk`); their twins in
`impl/selfhost/front/ast.cb`, `parser.cb`, `modres.cb`, `typecheck.cb` (`check_variant_call`,
`enum_variant_has_fields`, `check_destructure`, `field_borrow_let`, `rw_block`, `pattern_fields_of`,
`is_fields_struct`, `last_name_seg`); `spec/22` (2.65.0), `spec/16` (1.35.0), `spec/12` (1.32.0);
`impl/conformance/16-aggregates/` (ten cases; `struct_pattern_through_ref_rejected.cb` became
`struct_pattern_through_ref_ok.cb`); `spec/conformance.md` (3.312.0)

## What changes

- The parser reads `V { τ f; … }` in an enum and hoists the struct `E$V` beside it (the enum's type
  parameters; every field exported; exported with the enum); the variant's payload is `E$V<T…>`. Refused in an
  enum with codes and in one declared in a function.
- `shown_type_name` (both toolchains) prints a `$` part that begins with a letter as `::` and that part, so
  every message names `E$V` as `E::V`; a local type's `N$3` is still `N`.
- The resolver rewrites `E::V { .f = … }` to `E::V(E$V { .f = … })` and an arm whose struct pattern names a
  variant with fields to `E::V(p)` taking `p` apart as `E$V`; bare `V { … }` and the statement
  `E::V { … } = e;` are refused there.
- The checker refuses `E::V(x)` made or matched with one binder (not in a derived function, whose `clone`
  works on the hidden payload), shows a missing arm `E::V { .. }`, and binds a struct pattern reached through
  a reference by `auto x = &p.f;` (`&mut p.f`): it checks those declarations in the round that finds them
  (kept with the program, so what the check records about them stays theirs) and records
  `Rewrite::PatternFields`, which `views` applies before the next round.
- The missing-field message of a struct literal names the fields (CHG-0452), so a variant's reads
  "a literal of `Shape::Rect` gives every field; `h` is not given".

## Checked

The ten cases under coby, cobc with gcc and clang (`compiled_suite`), and cbc's C the same as cobc's
(`compare_c.sh`) and its reports the same as coby's (`compare_reports.sh`); the decided sweep (CHG-0450's
section of the log, `stress/ease189/LOG.md`).
