# CHG-0150 — Text literal patterns

Kind: Change record (`spec/02-schema.md` §2)
Status: ACCEPTED (2026-09-30; D-0127)
Governed by: `CobaltC_Master_Instructions.md` §13
Depends on: D-0127
Affects: `spec/16` (1.21.0), `spec/22` (2.43.0), `spec/conformance.md` (3.118.0), `spec/decisions/D-0057` (amended), the guide §16, `impl/src/parser.rs`, `impl/src/typecheck.rs`, `impl/src/interp.rs`, `impl/cobc/src/lower.rs`

## What changed

- **`spec/16` `rule.agg.match`:** `lit` gains `string-literal`; `[Pattern-Type]` admits a text level (`str`, `String`, `StringView`) and types a text literal as of every text type and no other; `[Match-Non-Exhaustive]` treats a text level as an integer level (covered by `_` or a binder only); prose.
- **`spec/22`:** `pattern` gains `string-literal`; §2 (7) lists a text literal among the tokens that start a `pattern '=' expr` condition; §5 restriction 2.
- **Parser:** a `Str` token ends a pattern; `try_condition_pattern` starts on one.
- **Checker:** text scrutinee levels; the through-reference case (D-0109) for text; a literal against a type that is not an enum, integer, `bool` or text is now rejected statically with a message (it was a dynamic `diag.type-mismatch` for a float); `literal_key` for text.
- **coby:** the literal path compares a text scrutinee's bytes (`str`, `String`, `StringView`; a place, a temporary or a value); a binder arm takes the whole value (`take_whole`); a text literal below a variant compares the payload's bytes.
- **cobc:** a `String` place scrutinee is matched in place (as a resource enum is), a binder arm moves it out with `cb_take`; text arms compare with `cb_bytes_eq`; a literal through a reference payload crosses for text as for integers.
- **Rows:** `conf.match-text-str`, `conf.match-text-literals`, `conf.match-text-non-exhaustive-rejected`, `conf.match-text-literal-type-rejected`, `conf.match-text-on-integer-rejected`, `conf.match-text-reference-rejected`, `conf.match-text-duplicate-rejected`, `conf.if-pattern-text`.

## Compatibility classification

Additive: every program accepted before is accepted with the same
meaning; a text literal in pattern position was a syntax error.
