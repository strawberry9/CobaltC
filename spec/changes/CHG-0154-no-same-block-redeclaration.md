# CHG-0154 — A local is not redeclared in its own block (`[Binding-Form-Redeclared]`)

Kind: Change record (`spec/02-schema.md` §2)
Status: ACCEPTED (2026-10-01; D-0131)
Governed by: `CobaltC_Master_Instructions.md` §13
Depends on: D-0131
Affects: `spec/05` (1.4.0), `spec/14` (1.20.0), `spec/17` (2.12.0), `spec/examples.md` (3.16.0), `spec/registry/diagnostics.md` (1.43.0), `spec/conformance.md` (3.121.0), the guide, `impl/src/parser.rs`, the corpus

## What changed

- **`spec/05` §1 `rule.value-object.binding-form`:** `[Binding-Form-Redeclared]` — a name is declared once in its declaration region (a block's statements with the parameters, captures, loop names or pattern binder governing it); a second declaration is `diag.duplicate-local`. The paragraph that let a declaration shadow a binding of the same frame is replaced: shadowing comes only from a nested block, or of an item.
- **`spec/14` §3:** a `for` header's declaration governs the body; the note that the body may shadow it is withdrawn.
- **`spec/17` §2:** a local constant is shadowed only from a nested block; with a variable of its name in its own block it is `diag.duplicate-local`.
- **`spec/examples.md`:** `ex.no-silent-resource-overwrite` no longer offers redeclaration after `drop`.
- **`spec/registry/diagnostics.md`:** new `diag.duplicate-local`.
- **Rows:** `conf.shadowing` rewritten (a nested block); `conf.drop-then-shadow-ok` becomes `conf.drop-then-redeclare-rejected`; new `conf.redeclared-local-in-block-rejected`, `conf.redeclared-param-rejected`, `conf.duplicate-param-rejected`, `conf.redeclared-foreach-name-rejected`, `conf.duplicate-destructure-binder-rejected`, and five file cases in `impl/conformance/05-value-object-semantics/`.
- **Implementations:** the parser keeps a declaration region per block (joined by the body its binders govern) and reports the second declaration, with the first's line, before any type is checked; both tools share it.
- **Corpus:** every program that redeclared a name in its own block now assigns, or uses a second name.

## Compatibility classification

Breaking: a program that redeclares a local in its own block, or a
parameter or loop or pattern name in the body it governs, is rejected.
The repair — an assignment or a new name — changes no meaning.
