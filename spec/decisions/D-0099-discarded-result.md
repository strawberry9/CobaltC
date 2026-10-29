# D-0099 — A discarded `Result` is an error; `_ = e;` discards on purpose

Status: ACCEPTED (2026-09-30, owner-delegated: "go with _ = e; and proceed with all your choices", "proceed with the _ = e; regardless of how many files are affected. Just change them to work.")
Kind: Decision (`spec/02-schema.md`)
Governed by: `CobaltC_Master_Instructions.md` §1, §2, §9
Depends on: rule.expr.seq, `spec/21` `Result`
Affects: spec/13 §3 (`[Stmt-Result-Discarded]`, `[Discard]`), spec/22 `statement`, `spec/registry/diagnostics.md`

## Problem

`write_file(&p, &t);` compiled silently: the `Result` telling whether the
write happened was thrown away — C's classic unchecked return value. The
functions most often called for their effect return `Result<void, E>`
(`write_file`, `File::write`, `File::close`, `remove_file`, `rename`,
`Channel::send`), exactly the ones whose errors get forgotten. The rest
of CobaltC refuses silent loss (a live resource cannot be overwritten, a
moved value cannot be read); a lost error was the exception.

## Candidate mechanisms

1. **A discarded `Result` is a static error; `_ = e;` discards on
   purpose.** Selected. Zig makes discarding any non-void value an error
   with the same `_ = e;` opt-out.
2. **A warning.** CobaltC has no warnings: a diagnostic is an error.
3. **`drop(e);` as the opt-out.** No new form, but `drop` means "destroy
   this resource now", and most `Result`s own nothing.
4. **Every discarded value an error** (Zig). Would reject discarding
   `HashMap::insert`'s old value, `Vec::pop`'s element and the like,
   which is ordinary and loses nothing that signals failure.

Prior art: Rust (`#[must_use]` on `Result`, a warning, `let _ =`), Swift
(unused results warn unless `@discardableResult`), C++17 `[[nodiscard]]`,
GCC `warn_unused_result`, Go (external `errcheck`), Zig (error, `_ =`).

## Selected design

- `[Stmt-Result-Discarded]`: an expression statement, or a block-like
  statement, whose static type is `Result<τ, ε>` is ill-formed
  (`diag.result-discarded`, static). The check uses the declared type: a
  generic `T` that is a `Result` at one instantiation is not flagged.
- `[Discard]`: `_ = e;` is `{ auto $d = e; }` — `e` is evaluated and its
  value ends at once. `_` already names "no binding" in a pattern; as the
  target of `=` it names "no place".

## Compatibility impact

Programs that dropped a `Result` stop compiling until they handle it or
write `_ =`. Every such site in the repository (std, showcase, examples,
the conformance suite, the stress programs, the guide) is changed with
this record.

## Revisit conditions

If a `must_use` marking for other types is wanted (an `Option` from a
lookup, a handle), the same rule extends by type.
