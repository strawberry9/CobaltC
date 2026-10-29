# D-0243 — A reference binding captured by a closure is escaped

Status: ACCEPTED (2026-10-15; delegated by the owner, as D-0235)
Kind: Decision (`spec/02-schema.md`); repair of `rule.control.flow-analysis`'s closure row, no tool change
Governed by: `CobaltC_Master_Instructions.md` §20 (adversarial review)
Depends on: D-0236 (forgetting), D-0239, D-0241 (`--as-licensed`), CHG-0100
Affects: `spec/14` §6 (CHG-0295)

## Problem

The closure row of the transfer table forms `deriv(c, x_i, m)` for each captured local *not of reference
type*. A capture of a reference binding `r` is a borrow of `r` itself (`[Closure-Form-Borrow]`, exclusive
when the body writes through `r`), and no fact described it. So in

    auto r = &mut x;
    auto c = [r]() { *r = *r + 1; };
    r = &mut y;
    c();

the write `r = &mut y` was *proven* clash-free — `escaped(r) = F` and no fact on `r` — while the closure's
exclusive borrow of `r` was live: `cobmodel check --as-licensed` reports `model.proven-but-false`. Both tools
checked the write and faulted (`diag.aliasing-conflict`, dynamic). The same was true of a closure without
`move` in any position other than a `let` or an assignment (an argument, a callee): its implicit borrows were
not "`&x…` in any other position", and marked nothing.

## Options

(i) `escaped(r) := T` for each captured reference binding, and `escaped(x_i) := T` for each capture of a
closure formed anywhere but a `let`/assignment — decides only between proven and unknown, as D-0236 does;
(ii) `deriv(c, r, m)` for reference captures too, which would reject statically what the tools check at run
time (a change of acceptance: a program whose `r` is reassigned after the last call of `c` would still be
rejected while `c` lives).

## Decision

(i). The row says so, and a row for a closure without `move` elsewhere. No acceptance changes.

## Results

`conf.ref-captured-by-closure-then-reassigned` (dynamic, both tools as before); the model's licensed check
reports no false proof over the sample and the new enumerations.
