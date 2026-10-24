# CHG-0041 — Text conversions, and a program's own variants first

Kind: Change record (`spec/02-schema.md` §2)
Status: ACCEPTED (2026-09-25, owner-chosen; the `String` additions and the variant rule owner-delegated)
Governed by: `CobaltC_Master_Instructions.md` §1, §19, §21
Depends on: D-0032, rule.stdlib.print, rule.stdlib.string, rule.module.resolve
Affects: rule.stdlib.text (new), rule.stdlib.prelude, rule.module.resolve, diag.type-mismatch, diag.ambiguous-name, the §9, §11 and §14 cases listed below

## Problem / motivation

D-0032 has the details. In short, a program could not turn text into a
number, nor build text from numbers, and `std`'s new `ParseError::Empty`
made programs' own `Empty` variants ambiguous.

## Decision

D-0032: `String::append<T>` for `print`'s types and `parse<T>` for
integers and floats, both realized natively; `ParseError`;
`String::new` and `String::as_bytes`; and a program's own enum variants
found before imported ones.

## What changed

- **`spec/21` 3.6.0:**
  - §2d (new): `rule.stdlib.text` (`String::new`, `String::as_bytes`,
    `[Append]`, `[Append-Not-Printable]`, `[Parse]`,
    `[Parse-Not-Numeric]`, `number(T)`, `value(T)`);
  - §0: `ParseError` in the type list; `String::append` and `parse` in
    the table; `parse` in the scope paragraph.
- **`spec/17` 2.1.0:** `[Resolve-Unqualified]` clause (4) looks among
  the enums of the module, else of the nearest enclosing module that
  has the variant; new clause (4b) looks among imported enums only when
  (4) finds none; `[Resolve-Ambiguous]` covers (4b).
- **`spec/registry/diagnostics.md` 1.12.0:** `diag.type-mismatch`,
  `diag.ambiguous-name`.
- **`spec/conformance.md` 3.27.0:** thirteen cases.
- **`spec/02-schema.md` 1.0.17:** §5's "in use" ranges.
- **Implementations:**
  - `std`'s source gains `ParseError`, `String::new`,
    `String::as_bytes`, `String::append` and `parse`, with private
    helpers and four intrinsics only `std` may call (`append_native`,
    `text_write`, `parse_check`, `parse_value`);
  - `impl/src/numtext.rs` scans and converts numbers for both tools
    (`cbrt` includes the same file);
  - the shared front end checks `append`'s and `parse`'s `T` at the
    call, and resolves variants by the new clauses; a bare variant name
    that more than one enum has is rewritten `E::V`;
  - fixed on the way: a qualified `E::V` whose `V` another enum also
    has was dispatched by the bare name; a generic associated function
    of a non-generic type could not infer its type parameter (and a
    generic struct literal with a non-generic field left its type
    unknown).

## Affected entities

`rule.stdlib.text` (new); `rule.stdlib.prelude`; `rule.module.resolve`;
`diag.type-mismatch`; `diag.ambiguous-name`.

## Previous semantics

No conversion between text and numbers, and no way to add to a
`String`. Clause (4) counted every visible enum alike, own and
imported.

## New semantics

    [Append]   String::append(s, x) pushes text(x) (rule.stdlib.print) onto (*s).bytes
    [Parse]    parse<T>(s) = Err(Empty) | Err(Invalid(k)) | Err(OutOfRange) | Ok(value(T)), as §2d gives

and clauses (4) and (4b) of `[Resolve-Unqualified]` as `spec/17` gives
them.

## Affected invariants

None changed. `inv.string.utf8-validity` holds for `append`'s result:
it adds ASCII or a `str`'s or `String`'s bytes.

## Dependency impact

`rule.stdlib.text` depends on `rule.stdlib.print`,
`rule.stdlib.string`, `rule.stdlib.vec`, `rule.type.kind` and
`rule.arith.represent`.

## Compatibility classification

Extension. The resolution change only resolves names that were
ambiguous before; no accepted program changes meaning. A program's own
`parse` or `ParseError` takes precedence over `std`'s (D-0024), and its
own `Empty`, `Invalid` or `OutOfRange` over `ParseError`'s.

## Migration implications

None.

## Example changes

None in `spec/examples.md`.

## Conformance changes

**Added:**
- `conf.string-append-text`
- `conf.string-append-not-printable`
- `conf.string-append-self-conflict`
- `conf.string-as-bytes`
- `conf.parse-int`
- `conf.parse-invalid-offset`
- `conf.parse-out-of-range`
- `conf.parse-not-numeric`
- `conf.text-round-trip` (a file case)
- `conf.own-variant-wins`
- `conf.qualified-variant-shared-name`
- `conf.imported-variants-ambiguous`
- `conf.generic-assoc-fn-inferred`

## Future implementation implications

- `append` of a number goes through a 64-byte buffer and one
  `Vec::push` per byte, as `std`'s CobaltC does elsewhere.

## Prior-art status

See D-0032.

## Revisit conditions

See D-0032.
