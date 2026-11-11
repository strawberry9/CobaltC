# D-0268 — `std::encoding::toml`: TOML 1.0 read into a value tree and written back

Status: ACCEPTED (2026-10-18, the owner: "accept the recommendations, do it after std::stats")
Kind: Decision (`spec/02-schema.md`)
Governed by: `CobaltC_Master_Instructions.md` §6, §9, §15, §20
Depends on: D-0182 (`std::json`, the value-tree pattern), D-0197 (`std::encoding`), D-0223 (the frozen map admits a module for a new subject), D-0226 (text is UTF-8 bytes), D-0229, D-0230
Affects: `spec/21` §0, §2r′ (4.70.0); `impl/std/encoding/toml.cb`, `impl/std/encoding.cb`, `impl/src/prelude.rs`; `CHG-0336`

## Problem

`std::encoding` read JSON and nothing else. Configuration files are TOML far more often than JSON
(Cargo, Python's packaging, Hugo, many services), and "read a configuration file" is one very
common intent (Master Instructions §9): a program that needed one wrote a line-by-line reader that
took `key = value` and little else. The owner asked whether `std::encoding::toml` should be added.

## Options

1. **Coverage.** (a) A subset: key/value pairs, tables and strings, the part most files use.
   (b) TOML 1.0 whole: tables and arrays of tables, dotted keys, inline tables, the four string
   forms, integers in four bases with underscores, floats with `inf` and `nan`, the four date-time
   forms, and the format's refusals (a key or table defined twice, a value added to a closed
   table). A subset reads some files and silently misreads others; the public test suite exists
   only for the whole.
2. **The value model.** (a) Reuse `Json`. (b) A `Toml` enum of its own: `TomlString`,
   `TomlInteger(i64)` apart from `TomlFloat(f64)`, since TOML tells them apart and an `i64` is exact
   where an `f64` is not; `TomlBool`; `TomlDateTime` for the four date-time forms, which JSON has
   no value for; `TomlArray`; `TomlTable` with ordered members, as `JsonObject`. Variant names
   carry `Toml`, as `std`'s bare variant names are unique.
3. **Date-times.** (a) `std::time`'s `DateTime` (no fraction, no offset, no local forms).
   (b) A `TomlDateTime` struct of year, month, day, hour, minute, second, nanoseconds, the offset in
   minutes, and a `TomlDateKind` for the form; independent of `std::time` (a conversion later if
   asked). The format's four forms are the format's, not the clock's.
4. **Writing.** (a) A reader only. (b) `Toml::text` writing one canonical document: a table's plain
   members first, then its sub-tables under headers and arrays of tables under `[[…]]`; comments not
   kept; a layout-preserving editor declined (that is a document model, not a value tree).
5. **Reading a tree.** `get`, `at` and the `as_` functions as `Json`'s, and `lookup(&t, "a.b.0")`
   for a dotted path, with an element by its index where the value is an array; `Json` gains the
   same `lookup` for parity.
6. **Errors.** `ParseError` as every reader in `std` uses: `Invalid(i)` at the byte that does not
   fit or at the key the format refuses, `OutOfRange` for an integer beyond `i64`, a float beyond
   `f64` or nesting past 512; `Empty` never, since an empty document is an empty table.
7. **Verification.** `unittest` blocks, a conformance case, and the public toml-test corpus run
   offline from the git-ignored `stress/` (downloaded as untrusted data into its own directory) as a
   round check, not a battery step.

## Decision

(b) in each, and 5–7 as stated. `std::encoding::toml` (`impl/std/encoding/toml.cb`), declared in
`encoding.cb` and re-exported, so `import std;` sees `Toml`, `TomlMember`, `TomlDateTime`,
`TomlDateKind` and the functions. Written in CobaltC over `std::text`; no `unsafe`, so the trusted
table is unchanged. The reader builds the document as nodes that remember how each table was made
(by a header, implicitly on the way to a deeper header, by a dotted key, as an inline table, as an
array of tables), which is what the format's rules about redefinition turn on, and converts them to
the `Toml` tree at the end.

## Results

`spec/21` §2r′ `rule.stdlib.toml` with `[Toml-Parse]`, `[Toml-Tables]`, `[Toml-Text]`, `[Toml-Get]`,
`[Toml-Set]` and `[Toml-Error]`, the source transcribed; §0's module map, item table and signature
table; `[Json-Get]` gains `Json::lookup`. `CHG-0336`. Cases `conf.toml-parse`, `conf.toml-tables`,
`conf.toml-text`, `conf.toml-tree`, `conf.toml-errors` (`toml_ok.cb`), one output under `coby` and
`cobc` with GCC and with Clang. The toml-test corpus for TOML 1.0.0 (709 files: 208 valid with their
expected trees, 501 invalid) passes whole under `coby` and under a `cobc`-built decoder with both C
compilers (`stress/toml_test/run.py`). Five documented `unittest` blocks. The guide's §21 gains a
passage. Additive.

## Revisit conditions

TOML 1.1 when it is final (`\e`, `\x`, seconds optional, newlines in inline tables): a second reader
mode or a new edition of the rule. A conversion between `TomlDateTime` and `std::time::DateTime` if a
program asks for it.
