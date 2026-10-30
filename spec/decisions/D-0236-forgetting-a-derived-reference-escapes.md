# D-0236 — The flow analysis's forgotten facts mark their root escaped

Status: ACCEPTED (2026-10-15; delegated by the owner, as D-0235)
Kind: Decision (`spec/02-schema.md`); a soundness repair of the normative static text (`spec/STABILITY.md` §2)
Governed by: `CobaltC_Master_Instructions.md` §8, §12, §20, §21
Depends on: D-0018, D-0111, D-0219 (finding 8), D-0221
Affects: `spec/14` §1 (`[Ref-Binding-Last-Use]` prose), §6 (`escaped`, the transfer table, "Forgetting"),
`spec/OVERVIEW.md` §5; `spec/conformance.md` (CHG-0285). No tool changes.

## Problem

The fourth review (F-01) found that `rule.control.flow-analysis` could prove a false `¬clash`. When a
reference binding `r` ends — by `[Ref-Binding-Last-Use]`, at block exit, or when it is reassigned or moved —
the table sets its `deriv(r, x.π, m)` facts to `F`. But the path `r` held may have been copied where the
analysis does not look: a struct field later reached through `&h`, a `Vec`'s element, a spawned thread's
parameter whose handle is pushed into a container. There it stays valid and held (`spec/04` `held-by`), and
`clash` is true of the next write to `x`, while the discharge table calls that write *proven*, "no runtime
check". The review gave two programs; 2026-10-15 probing found the same shape at block exit and on
reassignment:

    i32 x = 0;
    Vec<ref<i32, exclusive>> v = Vec::new();
    {
        auto r = &mut x;
        Vec::push(&mut v, r);      // r's path now also in v
    }                              // block exit: deriv(r, x) := F
    x = 5;                         // proven by the table; clash is true: v[0] holds the path

Neither tool exploits it: `coby` checks every access, and `cobc` omits checks only on a local the function
never borrows. Both fault at `x = 5` in all four programs. The defect is what the text licenses: a conforming
implementation that omits the checks the table proves would let a shared reference see a change, or two
threads write `x`.

## Options

(i) every row that forgets a `deriv(r, x.π, m)` fact because `r` ended, was reassigned or moved also sets
`escaped(x)`; (ii) the same, but only when `r`'s value was copied somewhere untracked since it was bound (a
by-value argument, an aggregate field, a push, a spawn argument); (iii) end a binding early only when no copy
of its value was made. (ii) keeps a few more proofs at the price of a second syntactic test whose own gaps
would be the next finding; (iii) withdraws D-0111's early end from ordinary programs.

## Decision

(i). "Forgetting" is stated once in `spec/14` §6 and cited by the rows that clear a binding's facts (block
exit, `[Ref-Binding-Last-Use]`, `r = &_m x.π` and `r = e`, a moved reference binding). The rows that clear
facts because their subject is gone — `join(h)` and the consumption or `drop` of `x` — do not forget in this
sense; a fact rooted at `*q` needs nothing, since an access there is never proven.

## Rationale

`escaped` decides only between *proven* and *unknown*; it changes no acceptance, so no program changes
outcome, and the dynamic check — which both tools always made here — decides. What the text loses is proofs
of accesses after a reference into the same object has ended, which no implementation uses today (`cobc`'s
proof is for locals never borrowed at all). Master Instructions §8 puts invariant preservation before
performance potential, and §9's economy favours one sentence over a second analysis. D-0221's argument for
finding 8 ("a deferred end is later than a last use, never earlier") was about the binding; the hazard was
the token, and this rule is what makes that argument sufficient.

## Results

- `spec/14` §6: the `escaped` definition, five rows, the "Forgetting" paragraph; §1's prose after
  `[Ref-Binding-Last-Use]`; `spec/OVERVIEW.md` §5.
- Cases (each a dynamic fault both tools already gave): `conf.ref-copied-into-struct-then-binding-ended`
  (the review's Program A), `conf.ref-copied-into-vec-then-block-exit`,
  `conf.ref-copied-into-struct-then-reassigned`. The review's Program B (a thread's handle pushed into a
  `Vec`) has two outcomes by schedule and is a probe in the model's sample instead (D-0241).
- The model's static half computes `escaped` with forgetting, and its `--as-licensed` mode reports any access
  the table proves while `clash` is true (D-0241); over the sample and the enumeration it reports none.
