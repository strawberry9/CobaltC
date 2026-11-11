# D-0242 — A path formed for one access is pending until it

Status: ACCEPTED (2026-10-15; delegated by the owner, as D-0235)
Kind: Decision (`spec/02-schema.md`); clarification of `spec/16` and `spec/08` §4, no tool change
Governed by: `CobaltC_Master_Instructions.md` §20 (adversarial review)
Depends on: D-0007 (evaluation order), D-0107 (pending), D-0219 (the model), D-0238
Affects: `spec/16` §2 ("One access"), `[Index-Vec]`, `[Slice-Index]`; `spec/04` §2 (`clash`'s `pending`);
`spec/08` §1, §4 (CHG-0294)

## Problem

Extending the model to slices and to D-0238's `[Index-Vec]` (D-0246) showed three programs both tools run
faulting by the text's letter:

- `v[0] = v[0] + 1` and `s[0] = s[0] + 1`: an assignment evaluates its target place first (D-0007), and the
  target's element borrow (`[Index-Vec]`'s `a_k`, exclusive) was live while the value's element borrow
  (shared) was formed: they meet (`index(0)` and `index(0)`), and `¬clash` fails.
- `h.r = h.r` and `p.x = p.x + 1`: the same with a field projection, live from `[Field-Access]` until its one
  access, the write, after the value's read of the same field. (Before this round: no enumerated program
  wrote a field from itself.)
- `Vec::push(&mut v, v[0])`: D-0238 made `a_k` "a temporary of the statement", so the element borrow of the
  argument `v[0]` was still live when `push` wrote `v.len` through the vector, and met it. This one D-0238
  introduced; before it, `v[k]` was the call `*Vec::index_shared(&v, k)`, whose argument borrow ended with the
  call.

## Options

(i) A path formed for one access — a projection's, `[Index-Vec]`'s `a_k` and the element place's,
`[Slice-Index]`'s element place's — is *pending* (D-0107's term: `clash` does not count it) until that
access, which ends it. (ii) Evaluate an assignment's value before its target place (changes D-0007's order,
and with it which of two faults a program reports). (iii) End `a_k` at once after its check, and treat the
assignment target separately. (i) is one rule for every case, says what `spec/08` §4 already says of a
projection ("formed unchecked … it exists only for its access"), and is what both tools do: their element
and field checks are made at the access, and nothing of them stays.

## Decision

(i). Each such path keeps its formation check (`[Index-Vec]`'s `¬clash(a_k, m)`, `[Slice-Form]`'s), is
pending until the access through it — a read, a write, or the borrow formed from it, which keeps it only as a
base — and that access ends it, and the paths it was formed through for the same place. Soundness is
unchanged: every access is still checked, at the moment it is made, against every live path that is not
pending; what a pending path stands for is checked at its own access.

## Results

- `conf.vec-element-assigned-from-itself`, `conf.vec-push-own-element`, `conf.field-assigned-from-itself`: run
  in both tools, as before; the model, which faulted on all three, now agrees.
- The model's sample comparison is unchanged by the model's adoption of the rule (0 outcomes outside the set).
