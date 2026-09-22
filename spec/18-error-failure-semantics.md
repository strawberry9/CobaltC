# CobaltC Error and Failure Semantics

Status: normative artifact
Version: 1.2.0
Conforms to: `spec/02-schema.md` (Kind: Rule, `rule.fail.*`)
Governed by: `CobaltC_Master_Instructions.md` §17, §23
Realizes: D-0008, D-0009

## 1. Fault termination

### `rule.fail.fault-unwind`
**Status:** ACCEPTED

    [Fault-Unwind]
        ⟨e, Σ⟩ ↛ d   in thread ℓ                        -- any rule's diagnostic judgment
        f_0 = the bottom frame of Σ.frame-stack(ℓ)
        Σ' = unwind-to(f_0, (), Σ)                       -- rule.control.unwind, thread ℓ only
        ────────────────────────────────────────────
        ⟨program, Σ⟩ ↛↛ terminate(d, Σ')

A `checked` fault is fatal (D-0009). The faulting thread unwinds every
statement scope and frame it has open — running every destructor
D-0008 would run on ordinary exit, so that resources with effects
outside the process (files, locks, OS handles) are released — and the
program terminates reporting `d`. Other threads take no further
steps; their frames are not unwound (their resources are abandoned —
the process is ending, and a second unwind could itself fault).
`↛↛` is never a premise of any rule (`spec/01` §2.5a): there is no
catching.

**Depends on:** D-0008, D-0009, rule.control.unwind

## 2. `Result` propagation

### `rule.fail.propagate`
**Status:** ACCEPTED

    e?  ≡  propagate(e)                                  -- spec/22 §2

    [Propagate]
        ⟨e, Σ⟩ →* ⟨r, Σ1⟩,  r : Result<τ, E>                -- either variant: the match below decides
        ────────────────────────────────────────────
        ⟨propagate(e), Σ⟩ → ⟨match (r) { Ok(v) : v, Err(err) : return Err(err) }, Σ1⟩

    [Propagate-Err-Mismatch]   disposition: rejected
        the enclosing function's return type is not Result<_, E'> with E' = E
        ────────────────────────────────────────────
        ill-formed; diag.propagate-outside-fallible-context

`e?` is sugar for the `match` shown (`rule.agg.match`), which is why
it consumes a resource-bearing `Result` exactly as a `match` does and
why the early return reuses `rule.fn.return` unchanged. The error
type must match the enclosing function's declared error type exactly
(D-0006); convert first with `map_err` (`spec/21` §0).

**Depends on:** rule.agg.match, rule.fn.return, D-0006, D-0009

## Change Log

- 1.2.0 — `CHG-0010`: `[Propagate-Ok]` renamed `[Propagate]` and its
  `discriminant = Ok` premise dropped — as written, no rule reduced
  `e?` on an `Err` value, so `conf.propagate-err` had no derivation;
  the desugaring's `match` already handles both variants.
- 1.1.0 — `[Propagate-Ok]`'s desugaring target re-spelled: mandatory
  parentheses around the `match` scrutinee (`CHG-0003`, missed for
  this file in that pass) and `:` instead of `=>` for the arms
  (`CHG-0005`). No rule semantics changed — `?`'s desugaring into a
  `match` is unaffected.
- 1.0.0 — Rewritten (`spec/AUDIT-2.md` B-13, B-17, B-20):
  `[Fault-Unwind]` per thread over `rule.control.unwind`; `?` given
  surface syntax and defined as a `match` desugaring; `map_err` moved
  to the library (`spec/21` §0) as an ordinary generic function. All
  entities `ACCEPTED`.
- 0.3.0 and earlier — superseded.
