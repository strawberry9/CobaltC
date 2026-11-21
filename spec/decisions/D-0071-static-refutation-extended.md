# D-0071 — Static refutation through references, pattern binders, and thread handles

Status: ACCEPTED (2026-09-27, owner-chosen)
Kind: Decision (`spec/02-schema.md`)
Governed by: `CobaltC_Master_Instructions.md` §8 (items 1, 10), §12, §17
(Aliasing and Concurrent Access)
Depends on: D-0018, D-0022, D-0046, D-0064, rule.control.flow-analysis
Affects: rule.control.flow-analysis (`spec/14`), `spec/conformance.md`

## Problem

`rule.control.flow-analysis` refutes an access when a visibly formed
reference to an overlapping place, of a mode that does not permit it,
is certainly live. Four shapes where the conflict is just as certain
were left to the dynamic check, because the analysis did not see the
reference that makes them conflict:

    auto r1 = &mut x; auto r2 = &*r1; *r1 = 2;              -- a reborrow through r1
    auto v = &(*g).v; g.v = …;                               -- a borrow through a guard
    match (&o) { Some(n) : { o = None; … }, … }              -- a pattern binder
    auto h = spawn(w, &x); x = 2; join(h)                    -- a thread's parameter

Each is `diag.aliasing-conflict` when that point runs. The last one is
worse: its outcome is unspecified, `{ ok, diag.aliasing-conflict }`,
depending on whether the thread has finished by then. A program with a
bug of this kind can pass its tests and fail on another machine.

## Constraints

- A program that runs correctly today stays accepted. The spawn case
  is the one exception, and only by removing the `ok` from an outcome
  that the specification already leaves unspecified (see Compatibility
  impact).
- The analysis stays intraprocedural and syntactic (spec/14 Scope),
  with no new syntax and no annotations.
- Every conforming implementation rejects the same programs.
- D-0022 is unchanged. Two overlapping borrows in one argument list
  (`f(&mut x, &x)`) are still reported at the callee's first
  conflicting use.

## Candidate mechanisms

1. **Extend `deriv` to the three missing holders:**
   - references reached through a tracked reference or guard binding;
   - the binders of a match by reference;
   - the handle a `spawn` returns.

   **Selected.**
2. Refute overlapping borrows in one argument list (Swift's law of
   exclusivity). This would reject programs that run today, and it
   reverses part of D-0022. Declined by the owner.
3. Treat a non-literal index expression as overlapping itself when the
   index is not written in between. Already covered: a non-literal index
   overlaps every index (spec/14 `deriv`), so `&mut a[i]` followed by
   `a[i] = …` is refuted today.
4. Change nothing, and report possible conflicts only as warnings.

## Selected design

Candidate 1. `rule.control.flow-analysis` changes as follows.

- **Referent roots.** For a tracked binding `q` of reference or guard
  type, the place it refers to is a root of its own, written `*q`. The
  following are places with root `*q`:
  - `*q`;
  - `(*q).f` and `q.f` (auto-deref, D-0064);
  - `q[i]`;
  - their projections.

  So `&_m` of such a place records `deriv(r, *q.π, m)`. An access at
  such a place has its `¬clash` refuted exactly as an access rooted at
  a binding is. `*q` has no `valid` or `init` fact; those stay with `q`.

  Every fact rooted at `*q` becomes `F` when:
  - `q` is assigned (its referent changes);
  - `q` is moved (a guard);
  - `q`'s block ends.
- **Match binders.** In each arm of `match (&_m e)`, and of `match (q)`
  for a tracked reference binding `q`, the arm's binder `b` holds
  `deriv(b, e's place, m)` (respectively `deriv(b, *q, m)`) for the
  arm. The binder's block exit clears it, as for any binding.
- **Thread handles.** `auto h = spawn(f, a1, …, an)` and
  `h = spawn(…)` set `deriv(h, x.π, m)` for every argument `ai` that is
  itself `deriv`-forming. That means:
  - a visible borrow;
  - a slice;
  - a D-0011-elided call on one.

  The facts hold until:
  - `join(h)`, where the thread has finished and its parameters have
    ended;
  - a move of `h`, which takes them out of the analysis' sight;
  - `h`'s block exit.

  `spawn(…, &_m x …)` still sets `escaped(x) := T`.
- **Discharge.** An access whose root is `*q` is refuted like any other
  when an overlapping, non-permitted `deriv` fact is `T`. It is never
  *proven*: the caller may hold other paths to the same object (D-0022's
  `f(&mut x, &x)`), which a single body cannot see. A temporary root is
  unchanged (never `T`, never `F`).

## Rejected alternatives

- **2:** it trades D-0022's report-at-use for a report at the call, and
  it rejects callees that never use both arguments. The owner kept
  D-0022.
- **3:** already covered by the existing overlap rule.
- **4:** keeps late detection for conflicts that are certain. Warnings
  remain available to implementations for *possible* conflicts, as
  spec/14 already allows.

## Semantic rationale

Each new fact records a reference the dynamic semantics already
holds:
- a reborrow is a path held by `r2` that descends from the path `q`
  holds (D-0018);
- a match-by-reference binder holds a reference into the scrutinee for
  its arm (D-0046);
- a spawned thread's parameter objects hold their arguments until the
  thread finishes (`rule.conc.spawn`, `rule.conc.join`).

A refuted access is therefore one that faults whenever it runs, or, for
a handle, whenever the thread has not yet finished. For a handle, the
program's result would otherwise depend on thread timing.

## Usability

    fn bump(ref<Counter, exclusive> c)
    {
        auto n = &mut c.hits;
        c.hits = 0;          // diag.aliasing-conflict (static): `n` borrows c.hits
        *n = *n + 1;
    }

    auto h = spawn(sum, &data);
    Vec::push(&mut data, 9);     // diag.aliasing-conflict (static): the thread reads data until join(h)
    i64 s = join(h);

## Implementation-feasibility

This is a change to the shared checker (`impl/src/typecheck.rs`, used by
both `coby` and `cobc`):
- `deref_place_of` gives the `*q` places;
- `spawn_facts_of`, `match_referent` and `join` clear the handle's
  facts;
- `flow_consume` drops what a moved binding held.

Neither the runtime nor code generation changes.

## Compatibility impact

- **Reborrows, guards and match binders:** a program rejected
  dynamically at a certain conflict is now rejected statically, at the
  same line.
- **Spawn:** a program whose outcome was `unspecified { ok,
  diag.aliasing-conflict }` is now rejected statically. The `ok` was
  never guaranteed: it happened only when the thread finished first.

No program whose outcome was `ok` on every execution changes.

## Prior-art status

- **Rust:** the borrow checker tracks reborrows and match bindings
  statically. `thread::scope` ties a scoped thread's borrows to the
  scope, which is checked through lifetimes.
- **Swift:** static exclusivity enforcement for local variables and
  `inout` parameters, with dynamic enforcement elsewhere.

## Revisit conditions

- Retention summaries for calls that store a reference argument
  (`conf.vec-holds-exclusive-ref-conflict` stays dynamic).
- Warnings for possible conflicts (a `?` fact), should they become
  normative.
