# CHG-0025 — `str` Text Values, `"…"`/`b"…"` Literals, and a Safe `print`

Kind: Change record (`spec/02-schema.md` §2)
Status: ACCEPTED
Governed by: `CobaltC_Master_Instructions.md` §1, §18, §21
Depends on: D-0020, rule.stdlib.prelude, rule.stdlib.string, rule.agg.array-construct, rule.type.is-resource, rule.type.eq, rule.type.typing, rule.type.expected, rule.arith.sizeof, rule.arith.represent, rule.arith.literal, rule.trust.extern-call, inv.string.utf8-validity, term.value, term.conformance
Affects: type.str, rule.stdlib.str, rule.stdlib.string, rule.stdlib.prelude, inv.str.utf8-validity, inv.string.utf8-validity, rule.type.is-resource, rule.type.eq, rule.type.typing, rule.type.expected, rule.arith.sizeof, rule.arith.represent, rule.arith.literal, rule.agg.array-construct, feat.str-literal

## Problem / motivation

`spec/22` 2.8.1 declared "there is no string or character literal"
and listed it under §5's deliberate absences. Text was a `u8` array
literal, and emitting it required a raw pointer and `unsafe`
(`ex.extern-write`). The human owner reconsidered the absence on
readability grounds — a byte array is not how a person thinks of
`"hello"` — and, from the seven candidate denotations D-0020 records,
directed option 6: a copyable, non-resource text value type, plus a
`b"…"` byte-array literal for the byte path, plus a prelude `print`
holding the only `unsafe`.

## Decision

D-0020. A string literal denotes a **value** of a new primitive type
`str` — a well-formed UTF-8 byte sequence, `is-resource = false`,
copied by `[Read]`, never a reference and never a resource. It is not
`ref`-typed, so `[Call-Multi-Ref-Return-Rejected]`/`CHG-0022` are
untouched; it is not `String`-typed, so no constant is a resource.

## What changed

**`spec/22` 2.9.0** (§1, §2, §3, §4, §5): `str-literal` and
`byte-literal` productions with their escape sets and ill-formedness
conditions; `str` added to `type-name` and `type`; both literals added
to `primary`; the correspondence table routes them; §5's absence
narrowed to character literals, a string view, and slicing. The
sentence "there is no string or character literal" withdrawn.

**`spec/21` 2.9.0** (§0, §2, new §2a): `type.str` and `rule.stdlib.str`
— `[Str-Literal]`, `[Str-Len]`, `[Str-Byte]`,
`[Str-Byte-Out-Of-Bounds]`, `[Str-Ptr]`, and the ordinary function
`print`; §0's table gains `str_len`/`str_byte`/`str_ptr`; §2 gains
`String::from_str` and no longer calls `from_utf8` the only
constructor.

**`spec/03` 1.2.0**: `inv.str.utf8-validity` added (`discharge:
static`, by the grammar); `inv.string.utf8-validity`'s establishment
names both constructors.

**`spec/12` 1.7.0**: `str` in `rule.type.is-resource`'s plain row;
`[Eq-Str]`; `[T-Lit-Str]`; `rule.type.expected` states that neither
literal takes an expected type.

**`spec/06` 1.4.0**: `⟪b0..b_{n-1}⟫` in the `Value` grammar;
`[Sizeof-Str]`, `[Repr-Str]` (`outcome: impl-defined`);
`rule.arith.literal` scoped to numeric literals.

**`spec/16` 1.3.1** (non-normative): `[Array-Construct]` notes the
`byte-literal` sugar.

**`spec/registry/features.md` 1.4.0**: `feat.str-literal` `ACCEPTED`
with the full §18 field set. **`spec/registry/diagnostics.md` 1.4.1**
(non-normative): `diag.index-out-of-bounds` names
`[Str-Byte-Out-Of-Bounds]`. **`spec/02` 1.0.2** (non-normative): "in
use" ranges updated.

**`spec/examples.md` 3.11.0**: `ex.str-literal`, `ex.e2e-hello-print`.
**`spec/conformance.md` 3.13.0**: eleven `conf.str-*`/`conf.byte-literal-array`/
`conf.string-from-str`/`conf.print-observed` rows in §11 and
`conf.e2e-hello-print` in §13.

## Rule changes

New: `rule.stdlib.str` (five labels). Extended: `rule.type.eq`
(`[Eq-Str]`), `rule.type.typing` (`[T-Lit-Str]`), `rule.type.is-resource`
(one row), `rule.arith.sizeof` (`[Sizeof-Str]`), `rule.arith.represent`
(`[Repr-Str]`, `Value` grammar), `rule.stdlib.string`
(`String::from_str`), `rule.stdlib.prelude` (three intrinsic rows).
Restated without change of meaning: `rule.arith.literal` (numeric
only), `rule.type.expected` (literals it does not reach),
`rule.agg.array-construct` (sugar note). No existing rule's conclusion
on any previously well-formed program changes.

## Affected invariants

`inv.str.utf8-validity` introduced; `inv.string.utf8-validity`'s
establishment field gains `from_str`, its proposition unchanged.
`inv.spatial-validity` is relied on by `[Str-Ptr]`'s side-condition
exactly as by `[Rawptr-Of]`. No invariant is weakened.

## Dependency impact

`rule.stdlib.string` gains `inv.str.utf8-validity`, `rule.stdlib.str`;
`rule.arith.sizeof`/`represent` gain `type.str`; `feat.str-literal`
cites D-0020 and this record.

## Compatibility classification

Additive, with one reservation: `str` becomes a `type-name` and can
no longer be an identifier. A corpus-wide search (`spec/`,
`impl/conformance`, `impl/cobaltc_examples`, the prelude) found no
use of `str` as an identifier, so no existing program changes
meaning. `"` and `b"` were previously not tokens at all, so no
previously grammatical program contained them.

## Migration implications

None required. A program that spelled text as a `u8` array may
replace it with `b"…"` (identical type and value) or, where a `str`
suffices, with `"…"` and `print`.

## Example changes

`ex.str-literal`, `ex.e2e-hello-print` added. `ex.extern-write` is
kept as the canonical `[Extern-Call]` example; `print`'s body is that
example's shape with `str_ptr`/`str_len` in place of the array.

## Conformance changes

Twelve rows added (`spec/conformance.md` 3.13.0). The reference
interpreter's suite adds the corresponding `.cb` cases under
`impl/conformance/12-type-system`, `21-standard-library-semantics`,
and `22-surface-syntax`, including four lexical ill-formedness cases
(`parse-error`) the row format cannot express.

## Future implementation implications

A compiler places literal bytes in read-only program data and passes
a `str` as an address/length pair by value (`[Sizeof-Str]`); an
interpreter interns each distinct byte sequence on first use. Which
address `str_ptr` yields, and whether equal literals share one, is
implementation-defined and must be documented (`spec/22` §5's list
gains this item by reference to `[Str-Ptr]`).

## Prior-art status

Rust's `&'static str` (address, length, `Copy`, UTF-8 by
construction) via lifetimes; C's string literal (an array of static
storage duration). See D-0020.

## Revisit conditions

D-0020's: `str` indexing/slicing syntax, a borrowed text view over
`String`, character literals, a fallible `print`, and the cost of
`String::from_str`'s copy.
