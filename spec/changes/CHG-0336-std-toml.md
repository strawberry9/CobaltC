# CHG-0336 — `std::encoding::toml`: TOML 1.0 read into a `Toml` tree and written back; `Json::lookup`

Kind: Change record (`spec/02-schema.md` §2)
Status: ACCEPTED (2026-10-18, D-0268: the owner's acceptance)
Governed by: `CobaltC_Master_Instructions.md` §20, §21
Depends on: D-0268; `spec/21` §2r `rule.stdlib.json`, §2d `ParseError`; `spec/06` `[Checked-*]`
Affects: `spec/21` §0, §2r, §2r′ (4.70.0); `spec/conformance.md` (3.246.0); `spec/02` §5 (1.0.117); `impl/std/encoding/toml.cb`, `impl/std/encoding/json.cb`, `impl/std/encoding.cb`, `impl/src/prelude.rs`; `htmlguide` §21

## What changed

- **The rule.** `spec/21` §2r′ `rule.stdlib.toml`: `Toml::parse(text)` reads a TOML 1.0 document into a
  `Toml` tree (`[Toml-Parse]`, with `[Toml-Tables]` for what headers, dotted keys and inline tables may
  define and what they close); `Toml::text(&t)` writes a table as a document, its plain members first
  and its sub-tables and arrays of tables under headers, and any other value in the form it takes
  after `=` (`[Toml-Text]`); `Toml::get`, `at`, `lookup` and the `as_` functions read a tree
  (`[Toml-Get]`); `Toml::set` and `push` build one (`[Toml-Set]`); the errors are `ParseError`'s
  (`[Toml-Error]`). `TomlDateTime` with `TomlDateKind` carries the four date-time forms and
  `TomlDateTime::text` writes one. §2r `[Json-Get]` gains `Json::lookup`, the same dotted path over
  a `Json` tree.
- **The module.** `impl/std/encoding/toml.cb`, declared `export module toml "encoding/toml.cb";` in
  `encoding.cb` and re-exported; embedded by `prelude.rs`; §0's module map, item table and signature
  table list it. Written in CobaltC over `std::text`; no `unsafe`.
- **Verification.** `conf.toml-parse`, `conf.toml-tables`, `conf.toml-text`, `conf.toml-tree`,
  `conf.toml-errors` (`impl/conformance/21-standard-library-semantics/toml_ok.cb`), one output under
  both tools and both C compilers; six documented `unittest` blocks (`--unittest-std`); the toml-test
  corpus for TOML 1.0.0 (709 files) whole under `coby` and a `cobc`-built decoder with GCC and Clang,
  from the git-ignored `stress/toml_test/` (not a battery step).

## What changed for programs

Additive. A program that declares its own `Toml`, `TomlMember`, `TomlDateTime` or `TomlDateKind`, or a
variant of one of the new names, beside `import std;` keeps its own (`[Resolve-Unqualified]` clause 1;
`--warn-shadow` names the shadowing). `Json::lookup` is a new function; no existing call changes.
