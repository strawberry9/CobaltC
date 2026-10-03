# CobaltC Specification — Reading Guide

This directory is the normative definition of CobaltC (Master
Instructions §13). Every artifact is at version 1.0.0 or later (each
carries its own Version and Change Log; `changes/` records every
normative change since 1.0.0); every entity `ACCEPTED`. This file is a
map, not a normative artifact.

## Edition

**CobaltC specification edition 2026.100306.**

The specification as a whole is identified by its edition,
`YYYY.MMDDnn`: the date it was published (UTC) and a two-digit counter
for that day, starting at 01. Editions before 2026-09-30 were numbered
`YYYY.nnn`, a counter within the year; both forms sort in publication
order, as text and as numbers. A new edition is published whenever an update of the
repository changes anything in this directory; an update that changes
only the implementations, the guide or the examples keeps the edition.
Each artifact still carries its own `Version` and Change Log, and each
normative change its `CHG` record; the edition names the set of them
published together. A conformance claim names an edition
(`NAMING-POLICY.md`: "conforms to CobaltC 2026.100306").

| Edition | Published with | Contents |
|---|---|---|
| 2026.001 | Commit Update 22, 2026-09-27 | The first numbered edition: every artifact as of D-0065 and `CHG-0075` |
| 2026.002 | Commit Update 24, 2026-09-27 | Local constants; fields and elements of constants are constants (D-0066, `CHG-0076`) |
| 2026.003 | Commit Update 26, 2026-09-27 | `diag.static-assert-not-constant`; `const auto` and values in a `static_assert` message; function-local types; a slice borrows its range (D-0067–D-0070, `CHG-0077`–`CHG-0080`) |
| 2026.004 | Commit Update 27, 2026-09-27 | `Vec::sort`'s source compares keys in place (`spec/21` 3.29.0; no rule changed) |
| 2026.005 | Commit Update 28, 2026-09-27 | A declaration by two names (`int x = 10;` is `diag.unbound-name`; `T x = a;`); syntax errors are `diag.syntax-error` (`CHG-0081`, `CHG-0082`) |
| 2026.006 | Commit Update 29, 2026-09-27 | Static refutation through references, handles and literal-bound locals; the stress rounds' frictions and fixes (`[e; N]`, constant array lengths, `diag.stack-exhausted`, text order); adjacent string literals; literal arguments typed by the call; `overwrite`, closure result types, `unwrap`/`expect`, `Vec::clone`, `fault` names (D-0071–D-0083, `CHG-0083`–`CHG-0099`) |
| 2026.007 | Commit Update 30, 2026-09-28 | Temporaries as arguments; `Vec::from_slice`/`truncate`; shifts bind tighter than `&` `^` `\|`; a use of a maybe-moved binding is rejected statically; an `Option<ref>` result borrows its first reference argument; `fn` values own their closures; the built-in bounds `eq`, `ordered`, `number`, `integer`; `min`, `max`, `abs`, `pow`, `sqrt`, `floor`, `ceil`, `round`, `trunc`, `HashMap::clear` (D-0084–D-0091, `CHG-0100`–`CHG-0106`) |
| 2026.008 | Commit Update 31, 2026-09-28 | `read_line` — on standard input and on a `File` — consumes one `\r` immediately before the `\n` it consumes (D-0092, `CHG-0107`) |
| 2026.009 | Commit Update 32, 2026-09-29 | A generic item as a value is instantiated by explicit type arguments or the expected fn type, and a binding with neither is rejected (`[T-Item-Generic]`, `[T-Item-Value-Uninferable]`); conformance rows for a spawned thread's bare-reference result, now compiled by `cobc` (D-0093, D-0094, `CHG-0108`, `CHG-0109`) |
| 2026.010 | Commit Update 33, 2026-09-29 | Overwriting a live part of an object — a field, a literal-index array element, a place reached through a valid reference — is refuted statically; what follows a false trusted claim: the rules describe the execution up to that step and constrain nothing after it (D-0095, D-0096, `CHG-0110`, `CHG-0111`) |
| 2026.011 | Commit Update 34, 2026-09-29 | `foreach` over an integer range: `foreach (x in lo..hi)` and `foreach (i, x in lo..hi)` count from `lo` up to, not including, `hi`; two plain-number bounds count in `usize` (D-0097, `CHG-0112`) |
| 2026.093001 | Commit Update 35, 2026-09-30 | The round-6 stress findings: the implementations enforce typing, visibility and constant rules the text already stated, a borrowing closure cannot escape its captures, destruction at a scope end is `solitary`-checked, spec/17's array-length text follows D-0074 (`CHG-0113`–`CHG-0116`); and the round-6 decisions: trailing commas, `_ = e;` for a discarded `Result`, printf `*` widths and `%v` enum names, maths and ASCII functions, byte literals typed by context, parts of temporaries as arguments, destructuring with `..` and renaming, qualified constant lengths, enum codes, pending argument borrows, comparisons on `String`/`str`/`bool`, patterns through references, struct and enum keys, the first static error in source order, and a reference binding ending at its last use (D-0098–D-0111, `CHG-0117`–`CHG-0131`) |
| 2026.093002 | Commit Update 36, 2026-09-30 | Method-call syntax recorded as a deliberate absence, the owner's final decision (`CHG-0132`, `spec/22` §5); §5's restrictions and absences brought up to date with D-0053, D-0090, D-0103 and D-0104 (`spec/22` 2.38.2; no rule changed) |
| 2026.093003 | Commit Update 37, 2026-09-30 | The findings on the examples' idioms and the extras: `Vec` helpers, views straight from a `String` and map lookups by `str`, `Option::ok_or`, `if (pattern = e)` and `while (pattern = e)`, the `clone` bound, the `std` errors as text, `bitstruct`, integers as bytes and CRC-32, a reproducible `Rng`, `read_volatile`/`write_volatile`, `checked_narrow`, bit counting and rotation, `sleep_ms`, `Weak<T>`, array lengths as constant expressions, and text literal patterns (D-0112–D-0127, `CHG-0133`–`CHG-0150`) |
| 2026.100101 | Commit Update 38, 2026-10-01 | The characters of a text: `String::chars` and `StringView::chars`, one view per character, and why no allocation-free `foreach` form exists (D-0128, `CHG-0151`) |
| 2026.100102 | Commit Update 39, 2026-10-01 | Room in advance: `Vec::reserve` and `String::reserve` (D-0129, `CHG-0152`); `?` on an `Option` in a function returning an `Option` (D-0130, `CHG-0153`); a local is declared once in its block — parameters and loop and pattern names count as declared in the block they govern — `diag.duplicate-local` (D-0131, `CHG-0154`); `spec/22` §1 notes the formatter `cobfmt` |
| 2026.100201 | Commit Update 42, 2026-10-02 | A `bitstruct` may be declared in a function, as a local type with its derived `bits` and `from_bits` (D-0132, `CHG-0155`) |
| 2026.100202 | Commit Update 43, 2026-10-02 | A local holding references and nothing to destroy (a map lookup's `Option<ref<…>>`, a struct of references) ends after its last use, as a reference does (D-0133, `CHG-0156`) |
| 2026.100203 | Commit Update 44, 2026-10-02 | The reading list notes that `std`'s bodies are normative for what they do, not how, and that `cobc` realizes the most-used ones natively (`21` §0); no rule changed |
| 2026.100204 | Commit Update 46, 2026-10-02 | D-0093's informative implementation note records that `cobc` now compiles a generic item passed as an argument whose type arguments the rest of the call fixes (`twice(id, 10)`); no rule changed |
| 2026.100301 | Commit Update 47, 2026-10-03 | `std` aligned with one design (D-0134, `CHG-0157`–`CHG-0161`): text a function only reads is a `StringView` (paths, needles, `File::write_text`), `StringView::of` and `[Str-Literal-View]`; `Rng::new`, `Result::map_err`, `String::parse`, `HashSet::key_at`, `stdout_write`/`stdin_read`; `remove` keeps order and `swap_remove` does not; `ReadError` folded into `FileError`; `HashSet::clear`; `spec/21` 4.0.0 with its tables grouped by type. Breaking (source) |
| 2026.100302 | Commit Update 48, 2026-10-03 | A `String` a call returns may be viewed as a call's argument (D-0135, `CHG-0162`): `[View-Form-Temporary-Argument]` extends D-0103 to `&f()[lo .. hi]`, so a computed path needs no binding; `spec/21` 4.1.0, `spec/09` 1.5.0. Additive |
| 2026.100303 | Commit Update 52, 2026-10-03 | `std` in submodules, still reached by `import std;` (D-0136): re-export, `export import p;` (`CHG-0163`, `spec/17` 2.14.0, `spec/22` 2.45.0); `std` divided into `std::core`, `collections`, `text`, `io`, `sys`, `memory`, `sync`, `random` and `math`, each re-exported by its root, and the floating-point functions moved from the intrinsics to `std::math` (`CHG-0164`, `spec/21` 4.2.0, `spec/06` 1.16.0). Breaking (source) only for a floating-point call without `import std;` |
| 2026.100304 | Commit Update 55, 2026-10-03 | Two collections in `std::collections` (D-0137, `CHG-0165`, `spec/21` 4.3.0 §2i, `spec/conformance.md` 3.127.0): `Queue<T>`, taken from and added to at either end in amortized constant time (`rule.stdlib.queue`), and `PriorityQueue<T>`, least first by the key types' order (`new`) or the caller's (`new_by`), equal elements first in, first out (`rule.stdlib.priority-queue`, `[Priority-Queue-Not-Key]`). Additive |
| 2026.100305 | Commit Update 57, 2026-10-03 | The set operations: `HashSet::union`, `HashSet::intersection` and `HashSet::difference`, each a new set of copies of the keys in the first set's order, `union` adding the second's new keys after it (D-0138, `CHG-0166`, `spec/21` 4.4.0 `[Set-Ops]`, `spec/conformance.md` 3.128.0). Additive |
| 2026.100306 | Commit Update 58, 2026-10-03 | Three text helpers (D-0139, `CHG-0167`, `spec/21` 4.5.0 §2h, `spec/conformance.md` 3.129.0): `contains` (`[Contains]`, whether `find` is `Some`), `replace` (`[Replace]`, a new `String` with every occurrence replaced, left to right without overlap; an empty `from` occurs nowhere) and `join` (`[Join]`, the inverse of `split`, for a slice of views or of `String`s), each on `StringView` and on `String`. |

**Licence.** Copyright © 2026 strawberry9. This specification — every
file in this directory — is licensed under the Creative Commons
Attribution-NoDerivatives 4.0 International License (CC BY-ND 4.0): it
may be copied and distributed unchanged, including commercially, with
attribution, but not modified
(https://creativecommons.org/licenses/by-nd/4.0/). Anyone may implement
the language it describes (`LICENSE-IMPLEMENTATION` at the repository's
root); the name is governed by `NAMING-POLICY.md` there.

## What is normative

Rules written in CFN (`01-metalanguage.md`) inside `03`–`21`, the
state definition in `04`, the grammar in `22`, the registries, and
the conformance suite. Prose explains; where prose and a rule differ,
the rule wins. Decision records (`decisions/`) are rationale, not
rules.

## Reading order for an implementer

1. `22-surface-syntax.md` — the grammar and the correspondence table
   from each production to its rule.
2. `04-abstract-state.md` — `Σ`, every component, every predicate.
3. `12-type-system.md` — static typing; then `06` §7 for
   representation and the implementation-defined parameters you must
   choose and document.
4. `13` → `05` → `14` → `15` — evaluation: results, `store`, statement
   and block exit, calls and `main`.
5. `08`, `07`, `16`, `09`, `10`, `11` — aliasing, resources and
   destructors, aggregates, references, escape checking, `let`.
6. `14` §6 — `rule.control.flow-analysis`: the exact static/dynamic
   boundary; your implementation must reject exactly the refuted
   instances.
7. `17`, `18`, `19`, `20`, `21` — modules, faults, threads, raw
   pointers and FFI, and the standard library module `std`: its intrinsics and the library
   types written in CobaltC itself (`Vec`, `String`, `HashMap`, `Rc`, `Box` and the rest),
   whose bodies are normative for what they do, not how: `cobc` realizes the most-used
   ones natively (`21` §0).
8. `conformance.md` — every case with its derivation; `registry/
   diagnostics.md` — every diagnostic you must be able to report.

## What you must decide and document

Listed in `22` §5: `AddrWidth`, byte order, enum discriminant width,
struct padding, `dangling<T>()`, `fn` value encoding, mutex cells,
the reporting form of termination outcomes, calling convention/ABI.
`IMPLEMENTATION-NOTES.md` (non-normative) is the consolidated form of
this list, alongside every point of permitted nondeterminism and every
`unsafe` trust condition, each with its governing rule.

## What the language does not provide

`22` §5's "deliberate absences" and "restrictions". Do not add them
in an implementation; propose them through a `D-XXXX` decision.

## Provenance

`AUDIT.md` (first audit), `AUDIT-2.md` (second audit, rules-level),
`AUDIT-STATUS.md` (closure of both, and the 2026-09-20 consistency
pass recorded as `CHG-0009`, and the per-case derivation of the
conformance suite recorded as `CHG-0010`, and the end-to-end program
derivations recorded as `CHG-0011`). D-0015 was superseded by D-0018;
D-0018 and D-0019 are the decisions behind the 1.0.0 rewrite.
