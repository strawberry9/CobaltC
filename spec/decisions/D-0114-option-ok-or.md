# D-0114 — `Option::ok_or`

Status: ACCEPTED (2026-09-30, the owner: "proceed with your recommendation" — the findings report's F2, candidate 1, after weighing "programmers will now need to know 2 patterns")
Kind: Decision (`spec/02-schema.md`)
Governed by: `CobaltC_Master_Instructions.md` §1, §8 (item 10), §9
Depends on: D-0033 (`unwrap_or`), D-0060 (`?` ends a temporary), rule.fail.propagate
Affects: `spec/21` §0, `spec/conformance.md`, the guide §18/§21, `impl/src/prelude.rs`

## Problem

`?` applies to a `Result` only. A function returning a `Result` that
calls an `Option`-returning helper (a parser's `peek`, a map lookup,
`Vec::pop`) writes the same five lines to turn `None` into its error:

    u8 c = match (peek(p))
    {
        Some(c) : c,
        None : return Err(fail(p, "expected a value")),
    };

36 sites in the corpus, and in every one the `None` arm only returns
an error.

## Candidate mechanisms

1. **`Option::ok_or<T, E>(Option<T> o, E e) : Result<T, E>`** in `std`,
   the `Option` as a `Result` so `?` applies:
   `u8 c = Option::ok_or(peek(p), fail(p, "…"))?;`. Selected.
2. **`?` on an `Option`** inside an `Option`-returning function: 8 sites;
   a rule for a rare case.
3. **A refutable binding with an `else` block** (`Some(c) = peek(p) else
   { return … };`): covers `break`/`continue` and error mapping too, but
   is a grammar change; left for a later decision.
4. **Leave the `match`.** One pattern per intent; five lines each time.

The owner's concern was 4's: a second spelling for what `match` says.
Decided on the precedent of `unwrap_or` (D-0033), which is the same
family — "the value, or a default" — and on the test that a `std` helper
earns its place when it names one very common intent so the reader need
not read the arms to learn that `None` merely returns an error.

## Selected design

    export fn Option::ok_or<T, E>(Option<T> o, E e) : Result<T, E>
    {
        match (o) { Some(v) : Ok(v), None : Err(e) }
    }

`e` is evaluated before the call whichever variant `o` holds (strict
left-to-right, D-0007); on `Some` it is unused and destroyed at the
statement's end like any temporary. `Option::ok_or(o, e)?` returns
`Err(e)` from the enclosing function on `None` (`[Propagate]`), whose
error type must be `E` (D-0006; `map_err` first otherwise). The name is
the one programmers will search for.

## Compatibility impact

Additive; a program's own `Option::ok_or` shadows it (D-0024).

## Revisit conditions

If the refutable binding (candidate 3) is ever adopted, `ok_or` stays:
it is the form for "this error", the binding the form for "do this".
