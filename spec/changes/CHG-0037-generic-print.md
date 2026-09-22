# CHG-0037 — `print<T>(T x)`

Kind: Change record (`spec/02-schema.md` §2)
Status: ACCEPTED (2026-09-24, owner-chosen)
Governed by: `CobaltC_Master_Instructions.md` §1, §19, §21
Depends on: D-0028, rule.stdlib.str, rule.type.kind, rule.stdlib.prelude
Affects: rule.stdlib.str, rule.stdlib.print (new), rule.type.kind, rule.stdlib.prelude, diag.type-mismatch, the §11 cases listed below

## Problem / motivation

D-0028 has the details. In short, `print` took only `str`, and a
generic `print` cannot be written in CobaltC.

## Decision

D-0028: `print<T>(T x)`, a natively realized function of `std`, for
`str`, integers, floats, `bool` and `ref<String, shared>`.

## What changed

- **`spec/21` 3.3.0:**
  - §2a: `print`'s body and prose move to a new rule,
    `rule.stdlib.print` (`[Print]`, `[Print-Not-Printable]`,
    `[Print-Int]`, `[Print-Float]`);
  - §0's table lists `print<T>`.
- **`spec/12` 1.9.0:** `rule.type.kind`'s note on intrinsics covers
  `print`.
- **`spec/registry/diagnostics.md` 1.10.0:** `diag.type-mismatch` names
  `[Print-Not-Printable]`.
- **`spec/conformance.md` 3.22.0:** eight §11 cases.
- **Implementations:**
  - `std`'s source replaces `print`'s body with two private functions,
    `print_str` and `print_string`, which the native `print` calls for
    its `str` and `String` cases;
  - the resolver enters `std::print` as it enters `std::map_err`;
  - the checker types it, `coby` and `cobc` realize it, and `cbrt`
    gains `cb_print_int`, `cb_print_f64`, `cb_print_f32` and
    `cb_print_bool`;
  - the float formatter is one function, in `coby` (`value.rs`) and in
    `cbrt`.

## Affected entities

`rule.stdlib.print` (new); `rule.stdlib.str` (no longer defines
`print`); `rule.type.kind` (text).

## Previous semantics

    export fn print(str s)   -- writes s's bytes

## New semantics

    [Print]                T printable    ⟨print(x), Σ⟩ → ⟨(), Σ⟩, writing text(x)
    [Print-Not-Printable]  T not printable, or not one argument    diag.type-mismatch (static)

with `text` as `rule.stdlib.print` defines it.

## Affected invariants

`inv.str.utf8-validity`: output stays valid UTF-8 (D-0028).

## Dependency impact

`rule.stdlib.print` depends on `rule.stdlib.string` and
`rule.type.kind`.

## Compatibility classification

Extension. `print` of a `str` is unchanged, and so is a program that
declares its own `print`.

## Migration implications

None. The showcases' and benchmarks' `out.cb` modules keep working;
their comments no longer say that `std` prints only `str`.

## Example changes

None in `spec/examples.md`.

## Conformance changes

**Added:**
- `conf.print-int`
- `conf.print-float`
- `conf.print-string-ref`
- `conf.print-string-by-value-rejected`
- `conf.print-not-printable`
- `conf.print-generic-ok`
- `conf.print-generic-not-printable`
- `conf.print-output`, whose exact output `impl/tests/print_output.rs`
  checks.

## Future implementation implications

- **Float digits.** Rust's `{:e}` gives the shortest digits but breaks
  exact ties away from zero; `[Print-Float]` requires ties to even.
  Both implementations therefore take the digit count from `{:e}` and
  the digits from `{:.*e}` at that count (correctly rounded, ties to
  even) whenever those read back as the value.
- **`f32`** chooses its digits in `f32`: `print(0.1: f32)` is `0.1`,
  not the `f64` expansion of the `f32` value.

## Prior-art status

See D-0028.

## Revisit conditions

See D-0028.
