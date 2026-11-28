# CHG-0401 — Attributes: `[[deprecated("…")]]` and `[[removed("…")]]` (D-0303)

Kind: Change record (`spec/02-schema.md` §2)
Status: ACCEPTED (2026-11-02)
Governed by: `CobaltC_Master_Instructions.md` §21
Depends on: D-0303, D-0220, D-0230
Affects: `spec/22-surface-syntax.md` (2.58.0: `attr`, `tombstone`, §5), `spec/17-modules.md` (2.18.0:
`rule.module.retired`), `spec/registry/diagnostics.md` (1.62.0), `spec/registry/features.md` (1.6.0),
`spec/STABILITY.md` (1.1.0), `spec/examples.md` (3.21.0), `spec/conformance.md`; `impl/src/{ast,parser,modres,
retired,typecheck,derive,astdump,lib,main}.rs`, `impl/cobc/src/main.rs`; `impl/selfhost/front/{ast,parser,dump,
modres,typecheck,items,check,derive}.cb`, `impl/selfhost/cbc/{driver,build_info}.cb`; `impl/tests/common/mod.rs`,
`impl/tests/{cb_conformance_suite,spec_rows}.rs`, `impl/cobc/tests/{oracle/mod,compiled_suite,compiled_spec_rows}.rs`;
the new cases under `impl/conformance/17-modules/` and `impl/conformance/22-surface-syntax/`

## What changed

1. **Grammar.** `[[deprecated("m")]]` before an item, a field, a bit field or a variant; `[[removed("m")]]` before a
   tombstone, the item's head and `;`. Misuses are `diag.syntax-error`, each with a message saying what is allowed.
2. **The rule.** `rule.module.retired`: a name's standing; `[Tombstone-Declares]`; `[Use-Deprecated]`
   (`diag.deprecated-use`, a warning under `--allow-deprecated`); `[Use-Removed]` (`diag.removed-use`); the uses that
   do not count (inside a retired declaration, a derived function, the `unittest` right after a retired item).
3. **Tools.** Both toolchains parse the attributes, keep each declaration's standing, check every resolved path, an
   `import`'s target, a path through a tombstone, a field and a variant in a pattern, drop tombstones after name
   resolution, and print the warnings once the program is accepted. `--allow-deprecated` on `coby`, `cobc` and `cbc`
   (before the file, before or after `--check`), and `COBALTC_ALLOW_DEPRECATED`. The AST dumps show a standing only
   when it is not live. A prelude function's text for the self-hosted checker includes its attribute.
4. **Harnesses.** A case's `// flags:` header gives the tool flags before its path.
5. **Policy.** `STABILITY.md` §3: during a window the old form is `[[deprecated]]`, rejected unless the flag; §3a, the
   lifecycle; §5.
6. **Conformance.** `conf.deprecated-use-rejected`, `conf.deprecated-use-allowed`, `conf.deprecated-type-path`,
   `conf.deprecated-module`, `conf.deprecated-field`, `conf.deprecated-variant`, `conf.deprecated-own-body-exempt`,
   `conf.deprecated-unittest-exempt`, `conf.removed-use-rejected`, `conf.removed-use-not-allowed`,
   `conf.tombstone-reserves-name`, `conf.tombstone-type-path`, `conf.attribute-unknown-rejected`,
   `conf.attribute-no-message-rejected`, `conf.tombstone-with-body-rejected`, `conf.removed-field-rejected`,
   `conf.attribute-on-import-rejected`.

## Not as the proposal wrote it

- The exemption is "a use inside a retired declaration", not "inside the item's own body": a deprecated type's own
  associated functions name the type (D-0303's record says why).
- No `= note: declared at …` line.

## Verification

- Under `coby`, and `cobc --run` with GCC and with Clang: every new case gives its stated outcome, the same output
  and the same diagnostic; `coby --dump-ast` and `cbparse` print the same tree for each, and `cobfmt` and
  `cobfmt-cb` the same text.
- `cargo test --release --no-fail-fast` over the workspace: every target passes (`conf_coverage` first). The one
  failure of the first run was `driver`'s Windows row, whose `libcbrt.a` for `x86_64-pc-windows-gnu` was older than
  the runtime's sources; rebuilt with Rust 1.77.2, the target passes.
- From this Update's source: the suites (`suites.sh`: both conformance suites under both tools, the compiled suite
  with GCC and Clang and under `--all-checks`, the model sample), `compare_c.sh` (1172 same, 0 DIFF) and
  `compare_parser.sh` (19636 same, 0 different). The proposal's other §9 gates run once, from the final source of
  `CHG-0405`'s Update, which is the one published; the evidence tier and its run are in `spec/MANIFEST.md`.
