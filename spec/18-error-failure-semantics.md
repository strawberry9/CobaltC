# CobaltC Error and Failure Semantics

Status: normative artifact
Version: 1.5.1
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

## 1a. Assertions

### `rule.fail.assert`
**Status:** ACCEPTED

`assert` (D-0065) is a function of `std` taking a condition and, as
`static_assert` does, an optional message — here a format and its
arguments, as `printf`'s (`spec/21` §2f):

    assert(c)                ≡  if (!(c)) { assert_fail(); }
    assert(c, f, a1, …, an)  ≡  if (!(c)) { assert_fail(format(f, a1, …, an)); }

    [Assert]
        ⟨c, Σ⟩ →* ⟨true, Σ1⟩
        ────────────────────────────────────────────
        ⟨assert(c, f, a1, …, an), Σ⟩ → ⟨(), Σ1⟩          -- a1 … an are not evaluated

    [Assert-Fail]   disposition: checked
        ⟨c, Σ⟩ →* ⟨false, Σ1⟩;  ⟨format(f, a1, …, an), Σ1⟩ →* ⟨text, Σ2⟩
        ────────────────────────────────────────────
        ⟨assert(c, f, a1, …, an), Σ⟩ ↛ diag.assert-failed, reporting text   -- then [Fault-Unwind];
                                                                            -- assert(c): no text

    [Assert-Ill-Formed]   disposition: rejected
        there is no condition; c is not of type bool; or f and a1 … an fail
        [Format-Invalid] or [Format-Arg-Mismatch] (spec/21 §2f)
        ────────────────────────────────────────────
        ill-formed; diag.type-mismatch (diag.format-invalid for the format)

The condition is evaluated exactly once, always: there is no mode in
which assertions are left out. The message is formatted, and its
arguments evaluated, only when the condition is false. A failed
assertion is a fault like any other (`[Fault-Unwind]`): destructors run,
the program ends reporting `diag.assert-failed` with the call's location
and the message. `static_assert` (`spec/17` §1b) is the same check made
before the program runs, for a constant condition.

**Depends on:** D-0065, rule.fail.fault-unwind, rule.stdlib.format

## 2. `Result` propagation

### `rule.fail.propagate`
**Status:** ACCEPTED

    e?  ≡  propagate(e)                                  -- spec/22 §2

    [Propagate]
        ⟨e, Σ⟩ →* ⟨r, Σ1⟩,  r : Result<τ, E>                -- either variant: the match below decides
        ────────────────────────────────────────────
        ⟨propagate(e), Σ⟩ → ⟨match (r) { Ok(v) : v, Err(err) : return Err(err) }, Σ1⟩
            where a temporary r ends (rule.value-object.object-end) as soon as the arm has bound
            its payload, rather than at [Stmt-Exit] (D-0060)

    [Propagate-Err-Mismatch]   disposition: rejected
        the enclosing function's return type is not Result<_, E'> with E' = E
        ────────────────────────────────────────────
        ill-formed; diag.propagate-outside-fallible-context

    [Propagate-Option]                                   -- D-0130
        ⟨e, Σ⟩ →* ⟨o, Σ1⟩,  o : Option<τ>,  the enclosing function returns Option<τ'>
        ────────────────────────────────────────────
        ⟨propagate(e), Σ⟩ → ⟨match (o) { Some(v) : v, None : return None }, Σ1⟩
            where a temporary o ends as soon as the arm has bound its payload (as above)

    [Propagate-Option-Mismatch]   disposition: rejected
        o : Option<τ>, and the enclosing function's return type is not an Option
        ────────────────────────────────────────────
        ill-formed; diag.propagate-outside-fallible-context

`e?` is sugar for the `match` shown (`rule.agg.match`), which is why
it consumes a resource-bearing `Result` exactly as a `match` does and
why the early return reuses `rule.fn.return` unchanged. One difference
from a written `match` (D-0060): when `r` is a temporary it ends at the
`?`, not with its statement, whatever it owns. Its payload has been
taken, and nothing can name the rest; so `match (peek(p)?)` holds only
the reference `?` produced, not a copy of it inside the `Result` too. A
`Result` that is a place (`x?`) is read as before and stays. The error
type must match the enclosing function's declared error type exactly
(D-0006); convert first with `Result::map_err` (`spec/21` §0).

`?` on an `Option` (D-0130) gives the payload of `Some`, or returns
`None` from a function that itself returns an `Option`: the common case
of checked arithmetic (`checked_add(start, n)?`) and of a chain of
lookups in an `Option`-returning helper. Nothing converts between the
two: in a function returning a `Result`, `Option::ok_or(o, e)?` names the
error to return (D-0114), and a `Result`'s `?` in an `Option`-returning
function is `[Propagate-Err-Mismatch]` as before.

**Depends on:** rule.agg.match, rule.fn.return, D-0006, D-0009, D-0060, D-0130

## Change Log

- 1.5.1 — `CHG-0158` (D-0134): `Result::map_err` (was `map_err`).
- 1.5.0 — `CHG-0153` (D-0130): `[Propagate-Option]` and
  `[Propagate-Option-Mismatch]`: `?` on an `Option` in a function
  returning an `Option`.
- 1.4.0 — `CHG-0075` (D-0065): §1a (new) `rule.fail.assert`.
- 1.3.0 — `CHG-0068` (D-0060): `[Propagate]` ends a temporary operand at
  the `?`.
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
