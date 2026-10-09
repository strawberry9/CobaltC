# D-0022 — Aliasing Conflicts Stay Checked at Use, Not When a Reference Becomes Held

Status: ACCEPTED (2026-09-23, owner-delegated)
Kind: Decision (`spec/02-schema.md`)
Governed by: `CobaltC_Master_Instructions.md` §9, §12, §17
Depends on: D-0004, D-0018, rule.alias.borrow, rule.control.flow-analysis, rule.conc.spawn, rule.trust.rawptr, `spec/21-standard-library-semantics.md` §1
Affects: none — this record keeps D-0018 unchanged and records why

## Problem

D-0018 checks `clash` at every *access* and counts only *held* paths
(paths some live object's storage holds). A path formed as a temporary
is not held, so a second borrow formed in the same expression does not
clash with it; the conflict surfaces at the first access that finds
both held. While speeding up `cobc` (`impl/COBC-PLAN.md` §10, Stage 3)
the question arose whether conflicts should instead be checked when a
reference *becomes held* — stored in a binding, a parameter, or an
aggregate — which would let an implementation prove accesses through a
reference parameter free of conflicts and skip their checks. This
record decides it.

Observed behaviour today (`coby` and `cobc` agree):

    fn f(ref<i32, exclusive> a, ref<i32, shared> b) : i32 { *a = 1; *b }
    i32 x = 0; f(&mut x, &x);              -- diag.aliasing-conflict (dynamic), inside f at `*a = 1`

and likewise for `f(&x, &mut x)`, `f(&mut x, &mut x)`, `f(&mut x.a, &x)`,
and across threads:

    fn writer(ref<i32, exclusive> a) : i32 { … *a = i; … }
    fn g(ref<i32, shared> r, handle<i32> h) : i32 { join(h) }
    i32 x = 0; g(&x, spawn(writer, &mut x));   -- diag.aliasing-conflict inside writer

## Constraints

- D-0018's premise: a conflict is a property of two paths both able to
  act, checked where one acts; it needs no lifetime annotations.
- No explicit lifetimes (`feat.explicit-lifetime-parameters`, rejected
  permanently by `CHG-0022`).
- `spec/conformance.md` pins the phase and, through the file suite, the
  location of existing faults.

## Candidate mechanisms

1. **Keep D-0018: check at use.** Overlapping reference arguments are
   admitted at the call and caught at the first conflicting access
   inside the callee. **Selected.**
2. **Check at hold time.** `store` of a reference into a binding,
   parameter or aggregate re-checks `clash` against held paths; `f(&mut
   x, &x)` would fault at the call. This would make an access through a
   reference parameter provably free of conflicts (the caller's hold
   proved it), so its `clash` check could be skipped.
3. **Check both.** Hold-time checks plus D-0018's use-time checks.

## Selected design

Candidate 1: D-0018 unchanged. No rule changes.

## Rejected alternatives

2 and 3. The gain they were considered for does not materialize:
- **Validity is not provable either way.** A referent can end during a
  callee that runs as a thread without any scope ending:

      auto h = spawn(reader, Vec::index_shared(&v, 0));   -- reader reads *r in a loop
      Vec::push(&mut v, 8); …                              -- reallocation releases the element

  is `diag.stale-binding` inside `reader`. An access through a
  reference parameter therefore keeps its temporal-validity check under
  any aliasing rule; only the `clash` half could be skipped.
- **The cost falls on programs.** Candidate 2 moves every fault of the
  shapes above from the callee's access to the call site, changing
  where existing programs report, and adds a check to every store of a
  reference — the common case — to remove one from accesses through
  reference parameters.
- **Measured payoff is small.** On `12_collatz.cb` the remaining cost
  is dominated by what `Vec::index_shared` does per element (a frame,
  the parameter's hold, a reclaimed element object and a fresh borrow),
  none of which a hold-time check removes.

Two neighbouring speed proposals were declined in the same review
(`impl/COBC-PLAN.md` §10): refining `escaped` after calls that cannot
retain a reference (measured: the affected call sites carry a
negligible share of the checks), and a cheaper model of element
identity for `Vec` (the reclaimed element objects are what make a
reference held across a reallocating `push` stale — `conf.e2e-vec-
realloc-stale-ref` — rather than silently dangling).

## Semantic rationale

Under D-0018 a conflict is reported where it becomes *actionable*: the
first access that would observe the aliasing. Two references formed in
one call's argument list conflict only once the callee uses one of them
against the other; reporting it there names the access that matters.

## Usability

A programmer passing `&mut x, &x` sees the fault at the line inside
the callee that writes through the exclusive reference, with the rule
and both paths' provenance. A call-site report would be earlier but
would name no access; the current report names the one that conflicts.

## Explainability

Unchanged: `diag.aliasing-conflict` (dynamic) with `[Write-Conflict]`/
`[Read-Conflict]` provenance.

## Implementation-feasibility

Unchanged. An implementation may still skip any run-time check it can
prove cannot fail — that changes no outcome and needs no rule change
(`rule.control.flow-analysis` constrains which programs are *rejected*,
not which passing checks are performed). This record only establishes
that accesses through reference parameters are not such checks.

## Compatibility impact

None.

## Prior-art status

Rust rejects `f(&mut x, &x)` statically through lifetimes and two-phase
borrows; CobaltC has neither by design (D-0011, `CHG-0022`), and
D-0018's use-time check is its equivalent enforcement point.

## Invariant traceability

`inv.alias-validity` unchanged: checked at every access (D-0018).

## Revisit conditions

- A demonstrated program family where call-site reporting would be
  materially clearer to programmers than the callee-access report.
- The specification gaining a way to prove a referent outlives a
  callee (so the validity half becomes provable too); then candidate 2's
  performance case should be re-measured.
