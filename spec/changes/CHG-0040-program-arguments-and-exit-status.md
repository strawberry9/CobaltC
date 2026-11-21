# CHG-0040 — Program arguments and exit status

Kind: Change record (`spec/02-schema.md` §2)
Status: ACCEPTED (2026-09-25, owner-chosen)
Governed by: `CobaltC_Master_Instructions.md` §1, §19, §21
Depends on: D-0031, rule.fn.program, rule.trust.extern-call, rule.stdlib.string, rule.stdlib.prelude
Affects: rule.fn.program, rule.stdlib.args (new), rule.stdlib.prelude, diag.no-main, diag.index-out-of-bounds, the §11 and §14 cases listed below

## Problem / motivation

D-0031 has the details. In short, a program could not read its
arguments or set its exit status, and `main`'s signature was stated
but not checked.

## Decision

D-0031: `fn main() : u8` gives the exit status; `std` gains an
`extern` `arg_bytes`, and `arg_count` and `arg`, written in CobaltC on
it. Argument 0 is the first argument after the program.

## What changed

- **`spec/15` 1.4.0:** `rule.fn.program` allows `fn main() : u8`;
  `[Terminate-Ok]` gives `terminate(ok(s), Σ)`; `[Program-No-Main]`
  rejects a root `main` with parameters or another return type; an
  implementation reports `ok(s)` as exit status `s`; the arguments are
  part of the environment.
- **`spec/21` 3.5.0:**
  - §2c (new): `rule.stdlib.args` (`[Arg-Bytes]`, `arg_bytes`,
    `arg_count`, `arg`);
  - §0: the three in the table; the scope paragraph names them.
- **`spec/22` 2.11.1:** §3's note on `main`.
- **`spec/registry/diagnostics.md` 1.11.0:** `diag.no-main`,
  `diag.index-out-of-bounds`.
- **`spec/conformance.md` 3.26.0:** ten cases; the outcome
  `ok, exit status n`; the `args:` header.
- **`spec/02-schema.md` 1.0.16:** §5's "in use" ranges.
- **Implementations:**
  - `std`'s source gains `arg_bytes`, `arg_count` and `arg`;
  - the shared front end checks `main`'s signature;
  - `coby FILE ARG…` passes `ARG…` to the program and exits with
    `main`'s status; `std::arg_bytes` is realized inside the
    interpreter;
  - `cobc` compiles `std::arg_bytes` to `cb_arg_bytes`, a new `cbrt`
    function reading the `argv` the generated C `main` passes to the
    runtime, and exits with `main`'s value; `cobc --run FILE ARG…`
    passes `ARG…` to the program;
  - the conformance runners read an `args:` header and the expectation
    `exit N`.

## Affected entities

`rule.fn.program`; `rule.stdlib.args` (new); `rule.stdlib.prelude`
(its table); `diag.no-main`; `diag.index-out-of-bounds`.

## Previous semantics

`terminate(ok, Σ)`, reported as exit status 0. No arguments. A root
`main` with parameters or a return type was accepted by both
implementations, against `rule.fn.program`'s text.

## New semantics

    [Terminate-Ok]   ⟨main(), Σ_0⟩ →* ⟨v, Σ⟩  ⇒  terminate(ok(s), Σ),  s = v for `fn main() : u8`, else 0

    [Arg-Bytes]   ⟨arg_bytes(i, p, n), Σ⟩ → ⟨|A_i|, Σ[ storage(p+j) := A_i[j] for j < min(n, |A_i|) ]⟩ when i < c,
                                             ⟨−1, Σ⟩ otherwise

and `arg_count` and `arg` as `rule.stdlib.args`'s bodies give them.

## Affected invariants

None changed. `inv.trust-transition` and `inv.string.utf8-validity`
hold for `arg`'s result (D-0031).

## Dependency impact

`rule.stdlib.args` depends on `rule.trust.extern-call`,
`rule.stdlib.vec`, `rule.stdlib.string` and `rule.fn.program`.
`rule.fn.program` gains D-0031.

## Compatibility classification

Extension, except that a root `main` with parameters or a return type
other than `void` or `u8` is now rejected, as `rule.fn.program` already
said. No program in the repository has one. A program's own `arg`,
`arg_count` or `arg_bytes` takes precedence over `std`'s (D-0024).

## Migration implications

None.

## Example changes

None in `spec/examples.md`.

## Conformance changes

**Added:**
- `conf.arg-typed`
- `conf.arg-bytes-outside-unsafe-rejected`
- `conf.arg-count-none`
- `conf.arg-out-of-range`
- `conf.args-read`
- `conf.arg-invalid-utf8`
- `conf.main-exit-status`
- `conf.main-exit-status-after-destructors`
- `conf.main-with-parameters-rejected`
- `conf.main-returns-i32-rejected`

`conf.args-read` and `conf.arg-invalid-utf8` are file cases whose
`args:` header gives their arguments, separated by spaces, with `\xHH`
for any byte and `""` for an empty argument. Both implementations run
them, and the compiled runners compare the output with the
interpreter's.

## Future implementation implications

None.

## Prior-art status

See D-0031.

## Revisit conditions

See D-0031.
