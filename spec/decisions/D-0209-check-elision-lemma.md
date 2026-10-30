# D-0209 — Check elision as a stated lemma, and the hole it found

Status: ACCEPTED (2026-10-11; the owner: "continue actioning the outstanding items in the Action plan" — WI-6)
Kind: Decision (`spec/02-schema.md`)
Governed by: `CobaltC_Master_Instructions.md` §12, §20
Depends on: D-0188, D-0189, D-0203, D-0205, D-0206
Affects: `spec/08` 1.3.0, `spec/conformance.md` 3.203.0, `spec/02` 1.0.71 (CHG-0247); `impl/cbrt` (`cb_direct_ok`,
`cb_frozen_ok`, `cb_paths_disjoint`, `cb_live_threads`), `impl/cobc` (`lower_fn_direct`, `--all-checks`),
`impl/battery.sh` stage 3

## Problem

The review (F-06; `ChatGPT_review/ACTION-PLAN-2026-10-11.md` WI-6) asked that D-0188's property — a path's
standing changes only in its own thread, so an implementation may check a function's reference arguments once
at entry — be stated as a lemma with explicit hypotheses and a case per transition kind, and that the elision
be tested differentially. Writing the cases out found a transition the prose had not covered: `[Release]`
(a `Vec`'s `grow`, `pop`, `clear`; a map's rehash) ends every reclaimed object over the released cells and asks
no `solitary`. A reference to an element is on the element's own object; an exclusive borrow of the vector is on
the vector's; they do not clash. So another thread holding `&mut v` can release the object a held element
reference points at, and the reference goes stale — a diagnosed outcome in the rules, but one a body that
checked the reference only at its entry never sees.

`cobc` had exactly that hole, in two of its elided forms: a function whose reference parameters are all shared
references to plain data (D-0188's *direct* parameters) was compiled as one body with no checks and no entry
test; a frozen parameter (CHG-0237) was tested at entry against overlapping exclusive paths only. Both read the
freed cell (`stress/elision_oct11/`, local: garbage differing by C compiler) where `coby` reported
`diag.stale-binding`. Slices and whole-vector references were not affected: their paths are on the vector's
object and the growing thread's borrow clashes with them.

## Decision

1. **`[Standing-Local]`** (`spec/08` §4): the property as a lemma with six hypotheses — not lock-derived, not
   pending, valid and clash-free, a slice's range free of live objects, **(H5) the path's object is fresh
   storage or its thread is the only live one**, and the holding thread leaves it alone — and the step kinds
   of another thread listed with the rule that keeps the standing under each. H5 is the new one: a fresh-storage
   object ends only with its frame or by `[Destroy]`, which `solitary` refuses; an object over raw storage can
   be released by whoever holds its container, which no clash prevents; and a thread alone at the path's
   arrival cannot gain such a competitor during the call, since a thread it spawns receives only what it
   passes.
2. **`cobc`**: the runtime's entry tests refuse a path over a reclaimed object unless `cb_live_threads == 1`
   (`cb_frozen_ok`, `cb_paths_disjoint`'s single path, the new `cb_direct_ok`); a function with direct
   parameters gets `NAME__dir` (as before) and `NAME__chk` (every access checked through its token), chosen by
   `NAME` on an inline read of `cb_live_threads` — one load and a branch in a single-threaded program — or on
   `cb_direct_ok`; a frozen function's checked twin is fully checked and its dispatcher tests its direct
   parameters too. No accepted program changes; a single-threaded program's code runs as before.
3. **`cobc --all-checks`** (`COBALTC_ALL_CHECKS=1`): no proof about a function's reference parameters is used
   — no dual, field-dual, frozen, direct or confined-parameter body, no bare call, no pure-direct predicate; what
   a program proves of its own locals is kept. The object cache keys on it. Battery stage 3 runs the file suite
   again under it, with one C compiler: the elided and the fully checked forms must both be what `coby` does.
4. Two conformance cases, `conf.direct-param-element-grown-by-other-thread` and
   `conf.frozen-param-element-grown-by-other-thread`, deterministic (the holder joins the pusher before it
   reads). The lemma's other hypotheses are exercised by existing rows: H1 `conf.thread-cancel-lock`,
   `conf.lock-through-exclusive-ref`; H2 `conf.state-and-input-arguments-disjoint` and the three D-0188 rows;
   H4 `conf.slice-read-beside-held-element`, `conf.slice-write-over-held-shared-element`; a thread ending with
   its argument held, `conf.spawn-exclusive-read-rejected`, `conf.handle-sweep-waits-for-running-thread`; a
   closure capturing a path on another thread, `conf.spawn-closure-capturing-reference`; cancellation and
   unwind in another thread, `conf.thread-cancel-waits`, `conf.fault-with-blocked-thread`.

Not done: a mechanised proof (no proof assistant in the project; the lemma's case list is the shape one would
take), and a differential run under `--explore`: the file suite under `--all-checks` is the differential test,
and the thread cases in it are deterministic by construction.

## Results

Both cases: `coby`, `cobc` with GCC and with Clang report `diag.stale-binding` at the read. Before the fix
`cobc` printed `-1644705586 1001` (GCC) and another value (Clang) for the direct case. The file suite and the
rows under `--all-checks` agree with `coby` on every case.

**The cost of the dispatcher, and the call-site rule.** Re-timing after the change (`stress/perfguide`,
`bench`, instructions counted by `perf`) showed the dispatcher's load, compare, branch and the token array's
store costing eight to eleven instructions per call of a direct-parameter function — half the body of a
small one: the guide's E10 (`by_ref(&big)`, fifty million calls) +50%, E12 (`step(&mut v[i])`) +89%,
`bench/records` +38%, `bench/tree` +15%. The lemma itself says where the test is redundant: when every direct
argument is a borrow rooted at a local the caller owns (`&x`, `&mut x.f`, `&mut v[i]`, never through a
reference), the object is the caller's own storage, which no other thread can reach — a reference to it
held elsewhere is shared, which cannot release it, or exclusive, which the borrow just formed would have
clashed with — so `cb_direct_ok` would pass and the checked twin could fault nowhere the direct one does not.
`cobc` now calls the direct twin itself at such a call (`direct_local_call`, the twin kept under the
`FROZEN_LINES` fallback as a forwarder), and a program function's dispatcher is `always_inline`, so a
recursion through it pays one call per level, not two. E10, E12 and `records` are back at their counts
before this decision; `tree` keeps +13% (its recursion passes `Box::get_mut(b)`, not a local, through the
dispatcher). Two rows: `conf.direct-call-owned-locals` (the shapes) and `conf.direct-call-lent-vec-element`
(the borrow that guards them). On the way, the frozen entry test (CHG-0237, `cb_frozen_ok`) was found to
build the normalised path eagerly — 45% of E3's `by_ref(&text)` round trip — and builds it now only when an
exclusive path is there to compare with: E3 variant b 666 → 392 million instructions. The remaining gap of
that variant to its figure before CHG-0237 (0.028 s; 0.060 s now) is the frozen entry test itself, two hash
lookups under the runtime lock per call, which pays for itself only in a body larger than `String::len`'s;
noted for the owner, not changed here.
The final round (every example of `stress/perfguide`, the `bench` workloads, GCC and Clang) is at the Update-99
counts or below except `tree`. The examples whose counts sit six to ten per cent above an older results file (E2,
E4, E5 b, E6 b, E9 b, E11) already had them with the Update-95 and Update-96 compilers: a creep across the editions
since `bench/RESULTS.md` was written on 2026-10-08 (`hashcount` 321, 329, 332, 335 million instructions with
Updates 84, 95, 96 and 100), which this edition did not add to. The guide's figures were refreshed from the
final run.
