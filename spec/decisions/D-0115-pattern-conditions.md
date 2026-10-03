# D-0115 — `if (pattern = e)` and `while (pattern = e)`

Status: ACCEPTED (2026-09-30, the owner: "proceed with F3 as D-0115" — the findings report's F3)
Kind: Decision (`spec/02-schema.md`)
Governed by: `CobaltC_Master_Instructions.md` §1, §8, §9, §10
Depends on: rule.agg.match (D-0056, D-0057, D-0058, D-0109), D-0049 (a match consumes only what it moves), rule.control.if, rule.control.while, D-0099 (a discarded `Result`)
Affects: `spec/22` §2 (2.39.0), `spec/14` §3–§4 (1.19.0), `spec/conformance.md`, the guide §14 and §22, the shared parser

## Problem

A `match` that cares about one case only, and a loop whose body begins
with a five-line `match`:

    match (HashMap::get(&ids, &key)) { Some(i) : return *i, None : {}, }

    while (true)
    {
        String line = match (read_line())
        {
            Ok(Some(l)) : l,
            Ok(None) : break,
            Err(_) : return 1,
        };
        …
    }

144 empty arms (`None : {}`, `_ : {}`) and 49 verbatim copies of the
input loop in the corpus. The reader must read the arm that does
nothing to learn that it does nothing.

## Candidate mechanisms

1. **Leave it to `match`.** Exhaustive and explicit; five lines each.
2. **`if (pattern = e)` and `while (pattern = e)`** — a condition that
   is a pattern test, desugared to the `match` the programmer would
   have written (`[If-Pattern]`, `[While-Pattern]`, `spec/14`). No new
   token: `=` and the pattern language exist, and `pattern '=' expr` is
   the shape of the destructuring statement. Selected.
3. **A contextual word, `if (e is Some(x))`.** One more word to
   learn; reads well but says the same thing as 2.
4. **`match` with the empty arm optional.** Loses exhaustiveness, the
   one property `match` exists for.

## Selected design

- Grammar: `cond ::= expr | pattern '=' expr` for `if` and `while`.
  The pattern's top must be a variant with a payload pattern
  (`Some(x)`, `Ok(Some(l))`), a qualified variant (`m::V`), or a
  literal; the parser decides by tokens (`spec/22` disambiguation (7)).
  A bare name stays the assignment it is (`if (x = e)` is rejected by
  `[T-If]` as today, with the message pointing at `==`); a
  payload-less variant like `None` is tested by `match`, `is_none` or
  `==`.
- Meaning: `if (p = e) b1 else b2 ≡ match (e) { p : b1, _ : b2 }` and
  `while (p = e) b ≡ while (true) { match (e) { p : b, _ : break } }`.
  Everything follows from `rule.agg.match`: binder scope (the block
  only), what `e` consumes, nested and literal patterns, the value of
  an `if` used as an expression, `else if` chains.
- The `while` form ends the loop on every value `p` does not match. For
  a `Result` that includes `Err`: the form is the written decision to
  stop on anything else, as a `_ : break` arm is, and a loop that must
  tell `Err` apart writes the `match`. (`[Propagate]`'s error is not
  discarded silently in the D-0099 sense: it is matched, by the exit.)
- Implementation: the shared parser builds the `match`; the checker,
  `coby` and `cobc` see only constructs they have. Both tools therefore
  cannot disagree about it.

## Compatibility impact

Additive: `Some(x) = e` in a condition was a syntax-level assignment
to a non-place, rejected; nothing that ran changes meaning.

## Revisit conditions

If a payload-less variant test (`if (None = o)`) is wanted often, the
parser could admit a qualified or bare variant name once `modres` has
resolved it; declined now for the ambiguity with assignment.
