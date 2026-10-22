# D-0244 — A consumed binding's own facts are forgotten

Status: ACCEPTED (2026-10-15; delegated by the owner, as D-0235)
Kind: Decision (`spec/02-schema.md`); repair of `rule.control.flow-analysis`'s consumption row, no tool change
Governed by: `CobaltC_Master_Instructions.md` §20 (adversarial review)
Depends on: D-0236 (forgetting), D-0071 (a handle's facts)
Affects: `spec/14` §6 (CHG-0296)

## Problem

A spawn's handle `t` holds facts (`deriv(t, x, exclusive)` for `spawn(f, &mut x)`, D-0071). Moving `t` into a
`Vec` or a struct — `Vec::push(&mut hs, t)`, `Job { .h = t }` — is the consumption row, which cleared the facts
*rooted in* `t` but not the facts `t` held. So after the move `deriv(t, x, exclusive)` stayed `T`, and
`x = x + 1` in the same block was rejected statically, though `t`'s thread might have finished. Both tools
checked it at run time. D-0236's "Forgetting" names "is moved" among the ways a holder's facts are forgotten,
but the consumption row did not apply it.

## Options

(i) The consumption row also forgets `deriv(x, ·, ·)` — the roots become escaped, and the access is checked;
(ii) keep the facts, and make both tools reject statically (rejects programs whose thread is done).

## Decision

(i), as D-0236 meant: what a moved binding held goes on with it, where the facts do not follow.

## Results

`conf.handle-moved-into-vec-then-referent-written` (dynamic: the thread waits on a channel holding the lent
path); the model now gives the dynamic outcomes both tools show.
