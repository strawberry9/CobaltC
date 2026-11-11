# CHG-0153 — `?` on an `Option` (`[Propagate-Option]`)

Kind: Change record (`spec/02-schema.md` §2)
Status: ACCEPTED (2026-10-01; D-0130)
Governed by: `CobaltC_Master_Instructions.md` §13
Depends on: D-0130
Affects: `spec/18` (1.5.0), `spec/registry/diagnostics.md`, `spec/conformance.md` (3.120.0), the guide §21, `impl/src/typecheck.rs`

## What changed

- **`spec/18` §2 `rule.fail.propagate`:** `[Propagate-Option]` — `?` on an `Option<τ>` in a function returning an `Option` is `match (o) { Some(v) : v, None : return None }`, with `[Propagate]`'s temporary rule (D-0060); `[Propagate-Option-Mismatch]` — anywhere else, `diag.propagate-outside-fallible-context`; the prose names `Option::ok_or(o, e)?` as the bridge into a `Result`-returning function.
- **`spec/registry/diagnostics.md`:** `diag.propagate-outside-fallible-context` lists `[Propagate-Option-Mismatch]` and its repair.
- **Implementations:** the static pass accepts `?` on an `Option` only in an `Option`-returning function (both tools already ran it so), rejects it elsewhere with the diagnostic and a message naming `Option::ok_or`, and rejects `?` on a number, `bool` or `str` as `diag.type-mismatch` (`[Propagate]`'s premise).
- **Rows:** `conf.propagate-option`, `conf.propagate-option-in-result-rejected`.

## Compatibility classification

Additive: a form the specification rejected is accepted, with the
meaning both implementations gave it. A `?` on an `Option` outside an
`Option`-returning function, which the implementations had let through,
is now rejected as the specification always required.
