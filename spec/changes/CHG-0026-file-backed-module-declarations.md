# CHG-0026 — File-Backed `module` Declarations

Kind: Change record (`spec/02-schema.md` §2)
Status: ACCEPTED (2026-09-22)
Governed by: `CobaltC_Master_Instructions.md` §1, §18, §21
Depends on: D-0021, term.program, term.module, rule.fn.program, rule.module.resolve, rule.module.visibility, rule.module.use, term.conformance
Affects: rule.module.file, feat.module-file, diag.module-file-not-found, diag.module-cycle, diag.module-file-duplicate

## Problem / motivation

Under `spec/22` 2.9.0 the only spelling of a module is the inline
`module m { … }`, so every item of a program lives in one source
text; the reference interpreter takes one file. The human owner asked
how a module in a different file is imported, and — after considering
a file clause on `import` — directed a bodiless `module` declaration
naming the file, with no `from` keyword and no `file(…)` wrapper. D-0021
records the derivation.

## Decision

D-0021. `vis module m "p";` in file `F` declares module `m` exactly as
`vis module m { … }` would, with the body taken from the file `p`
names, resolved against `F`'s directory. The declaring site names the
module; the file's top level is anonymous. `import` is untouched.

## What changed

**`spec/22` 2.10.0** (§3, §5): `module-decl` gains the alternative
`str-literal ';'`; §5's "linking" absence gains a sentence saying a
file-backed declaration is source assembly into one program, not
linking; the impl-defined list gains the path forms beyond the
required core and how a diagnostic location names its file.

**`spec/17` 1.4.0** (§1, new §5): §1's "every … `module` declaration
in module `M` adds `M::name`" notes that both forms count; §5 adds
`rule.module.file` — `[Module-File]` (the equivalence),
`[Module-File-Missing]`, `[Module-File-Cycle]`,
`[Module-File-Duplicate]` — and the path-resolution requirement.

**`spec/registry/diagnostics.md` 1.5.0**: three new entries,
`diag.module-file-not-found`, `diag.module-cycle`,
`diag.module-file-duplicate`, all static.

**`spec/registry/features.md` 1.5.0**: `feat.module-file` added.

**Not touched, deliberately:** `spec/00` (`term.program` and
`term.module` hold as written — a program is still a finite set of
items in one root module, and a module is still a named container of
items); `spec/15` (`rule.fn.program` is unchanged: `Σ_0.items` is "the
program's declarations" regardless of how many files spell them);
`spec/04` (no `Σ` component); `spec/03` (no invariant);
`spec/conformance.md` and `spec/examples.md` (see "Conformance
changes").

## Affected entities

New: `rule.module.file`, `feat.module-file`, and the three
diagnostics. Changed in text only, not in meaning:
`rule.module.resolve` (§1's enumerating sentence). Unchanged in text
and meaning: `rule.module.visibility`, `rule.module.use`,
`rule.fn.program`, `term.program`, `term.module`.

## Previous semantics

`module-decl ::= vis 'module' identifier '{' item* '}'`. A string
literal after `module identifier` did not parse. Every item of a
program was written in one source text.

## New semantics

`module-decl ::= vis 'module' identifier ( '{' item* '}' | str-literal ';' )`.

    [Module-File]
        `vis module m "p";` occurs in module M in file F
        p, resolved against F's directory, names a file whose contents parse as item* I
        ────────────────────────────────────────────
        the declaration is `vis module m { I }` in M

with three `disposition: rejected` companions for a missing file, a
cycle, and a duplicate. A conforming implementation must accept a
`/`-separated relative path with no `..` segment and no leading `/`;
other forms are impl-defined.

## Affected invariants

None. Modules introduce no invariant of their own (`spec/17`'s
Purpose; D-0017), and this change is an equivalence over syntax: every
rule that governs the inline form governs the file form through the
equivalence, without being touched.

## Dependency impact

`rule.module.file` depends on `rule.module.resolve`,
`rule.module.visibility`, and `term.module`; nothing gains a
dependency *on* it. `feat.module-file` and the three diagnostics cite
it. No existing `Depends on`/`Affects` line changes.

## Compatibility classification

Additive, source-compatible. No keyword is added; no identifier
becomes reserved; no program accepted under 2.9.0 changes meaning or
acceptance.

## Migration implications

None required. A program that was hand-concatenated from several files
may, optionally, be split back: each fragment becomes a file and a
`module m "…";` declaration, after which references into it gain the
`m::` prefix or an `import`.

## Example changes

None to `spec/examples.md` in this record: its fragments are derived
in `spec/conformance.md`, whose rows were single-file fragments (see
next). D-0021's Usability section carries the two-file example; the
runnable copy is `impl/cobaltc_examples/13_modules.cb` +
`13_geometry.cb`. (Non-normative note, 2026-09-22: `CHG-0029` later
added `ex.module-file` and homed the cases below in
`spec/conformance.md` §14. D-0021's example constructs `Rect` from
the root with private fields — permissible only before `CHG-0027`
made `[Field-Not-Visible]` a rule; a Decision is never edited once
`ACCEPTED`, so `ex.module-file` and the runnable copy, which write
`export f64 w;`, are the forms to copy.)

## Conformance changes

`spec/conformance.md`'s rows are single fragments assembled into one
program by `impl/tests/spec_rows.rs`; a file-backed module case is by
definition two files, so — as a structural exception of the same kind
as the table's single-line fragment convention — these cases live in
the file-based suite, `impl/conformance/17-modules/`, with fixture
files beside the entry case. The following cases are required
(`term.conformance`), named here so the rows have ids:

| Case | Setup | Outcome |
|---|---|---|
| `conf.module-file-ok` | `main.cb` declares `module geo "./geometry.cb";`; `geometry.cb` exports `fn area`; `main` calls `geo::area(…)` | `ok` |
| `conf.module-file-import-ok` | as above with `import geo::area;` and an unqualified call | `ok` |
| `conf.module-file-type-ok` | `geometry.cb` exports `struct Rect`; `main.cb` writes `geo::Rect r = …;` and, after `import geo::Rect;`, `Rect r2 = …;` | `ok` (the `spec/22` §2 rule-4 pre-scan sees the file's types) |
| `conf.module-file-private-rejected` | `geometry.cb` has a private `fn`; `main` calls it qualified | ✗ `diag.name-not-visible` (static) |
| `conf.module-file-nested-ok` | `lib/a.cb` declares `module b "./b.cb";` and `lib/b.cb` exists; `main.cb` declares `module a "./lib/a.cb";` and calls `a::b::f()` | `ok` (the inner path resolves against `lib/`) |
| `conf.module-file-missing-rejected` | `module m "./absent.cb";` | ✗ `diag.module-file-not-found` (static) |
| `conf.module-file-cycle-rejected` | `a.cb` declares `module b "./b.cb";`, `b.cb` declares `module a "./a.cb";`, `main.cb` declares `module a "./a.cb";` | ✗ `diag.module-cycle` (static) |
| `conf.module-file-duplicate-rejected` | `main.cb` declares `module x "./m.cb";` and `module y "./m.cb";` | ✗ `diag.module-file-duplicate` (static) |
| `conf.module-file-main-is-ordinary-ok` | `geometry.cb` declares an `export fn main() : i32 { 7 }`; `main.cb` has its own `fn main()` calling `geo::main()` | `ok` |

The file-based runner marks a `.cb` file as a fixture — the body of a
module some case declares, not an entry, so it is not run on its own —
by a first line `// CONFORMANCE-FIXTURE`; the fixtures live under
`impl/conformance/17-modules/fixtures/`. (Non-normative note,
2026-09-22: `CHG-0029` gave these ids their `spec/conformance.md`
home, §14, whose rows name the case files; the paragraph above
describes the state this record left.)

## Future implementation implications

For `coby`: a pre-parse pass that finds `[export] module IDENT "…" ;`,
reads the file relative to the current file's directory, recurses
with a load stack (cycle) and a seen-set (duplicate), and replaces the
declaration with `module IDENT { … }` before lexing the combined text
— the mechanism `impl/src/lib.rs` already uses to place the prelude
ahead of the program, and required for the same reason (the parser's
declaration-statement pre-scan, `impl/src/parser.rs` `Parser::new`,
must see every file's `struct`/`enum` names). `main.rs` passes the
entry file's directory in; string-only entry points resolve against
the working directory. The one piece of new machinery is a table from
combined line ranges to (file, line) so `resolve_user_line` and
`diagnostics::render` can name the file. `parser.rs`, `modres.rs`,
`build_items`, `typecheck.rs`, and `interp.rs` are untouched. A
compiler does the same at its front end.

## Prior-art status

See D-0021: Rust's bodiless `mod foo;` / `#[path]` has the selected
shape; C's `#include` and C++20's build-system-mapped named modules
are the rejected candidates 4 and 3.

## Revisit conditions

D-0021's: a conventional `module m;` mapping; naming one file twice;
a search-path or package mechanism; `..`/absolute paths in the
conformance core. A further syntax change layered on this one is a
new Change record, not an edit to this one.
