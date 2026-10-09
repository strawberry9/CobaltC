# CHG-0038 — Standard input: `read` and `read_line`

Kind: Change record (`spec/02-schema.md` §2)
Status: ACCEPTED (2026-09-25, owner-chosen)
Governed by: `CobaltC_Master_Instructions.md` §1, §19, §21
Depends on: D-0029, rule.trust.extern-call, rule.stdlib.string, rule.stdlib.prelude
Affects: rule.stdlib.read (new), rule.stdlib.prelude, the §11 cases listed below

## Problem / motivation

D-0029 has the details. In short, `std` could not read input, and a
program reading standard input through its own `extern` needed
`unsafe` and its own buffer, line and UTF-8 handling.

## Decision

D-0029: an `extern` `read` in `std`, and `read_line`, written in
CobaltC on it, returning `Result<Option<String>, ReadError>`.

## What changed

- **`spec/21` 3.4.0:**
  - §2b (new): `rule.stdlib.read` (`[Read]`, `read`, `read_line`);
  - §0: `ReadError` in the type list, `read` and `read_line` in the
    table.
- **`spec/conformance.md` 3.24.0:** five §11 cases.
- **`spec/02-schema.md` 1.0.14:** §5's "in use" ranges.
- **Implementations:**
  - `std`'s source gains `ReadError`, `read` and `read_line`;
  - `cobc` compiles `std::read` to `cb_read_in`, a new `cbrt` function
    reading the runtime's buffered standard input (flushing standard
    output first);
  - `coby` does not provide standard input: a call of `std::read`
    stops the program with `unsupported: reading input needs cobc;
    coby does not read standard input`, exit status 3;
  - the conformance runners read a new `stdin-hex:` header (below).

## Affected entities

`rule.stdlib.read` (new); `rule.stdlib.prelude` (its type list and
table).

## Previous semantics

None: `std` had no input.

## New semantics

    [Read]   ⟨read(p, n), Σ⟩ → ⟨k : isize, Σ[ storage(p+j) := the next input byte b_j for j < k ]⟩,
             k = 0 at the end of input, −1 when the input cannot be read

and `read_line` as `rule.stdlib.read`'s body gives it. Whether an
implementation provides standard input is implementation-defined.

## Affected invariants

None changed. `inv.trust-transition` and `inv.string.utf8-validity`
hold for `read_line`'s result (D-0029).

## Dependency impact

`rule.stdlib.read` depends on `rule.trust.extern-call`,
`rule.stdlib.vec` and `rule.stdlib.string`.

## Compatibility classification

Extension. A program's own `read`, `read_line` or `ReadError` takes
precedence over `std`'s (D-0024).

## Migration implications

None.

## Example changes

None in `spec/examples.md`.

## Conformance changes

**Added:**
- `conf.read-line-typed`
- `conf.read-outside-unsafe-rejected`
- `conf.read-line-lines`
- `conf.read-line-end-of-input`
- `conf.read-line-invalid-utf8`

The last three are file cases that read standard input. Their
`stdin-hex:` header gives the bytes to feed them. An implementation
that does not provide standard input must refuse them; the runners
check that `coby` does, and run them under `cobc` with that input,
comparing the output with `expect-stdout-hex:` (there is no interpreter
output to compare with). `read_line`'s `Err(Io)`, which no header can
produce, is checked by `impl/cobc/tests/read_input.rs`.

## Future implementation implications

- `read_line` makes one `read` call per byte. `cobc`'s runtime serves
  them from a buffer; 20,000 lines of 60 bytes take about 1.2 s, the
  per-byte cost of `std`'s CobaltC loop.

## Prior-art status

See D-0029.

## Revisit conditions

See D-0029.
