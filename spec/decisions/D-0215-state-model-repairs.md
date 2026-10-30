# D-0215 — State-model repairs: a poisoned mutex through an unwind, the schema's missing constructors, the scope of the UTF-8 invariant, a reference held twice in one object

Status: ACCEPTED (2026-10-11; the owner: "action the plan in full autonomously and using the recommendations as written" — `ChatGPT_review/ACTION-PLAN-2026-10-11b.md` WI-4)
Kind: Decision (`spec/02-schema.md`)
Governed by: `CobaltC_Master_Instructions.md` §6, §12, §15, §16, §20
Depends on: D-0018, D-0204, D-0210
Affects: `spec/19` 1.13.0, `spec/04` 1.5.0, `spec/03` 1.6.0, `spec/05` 1.6.0, `spec/conformance.md` 3.207.0 (CHG-0253);
`impl/conformance/08-alias-validity/reference_held_twice_in_one_object_ok.cb`

## Problem

The second independent review (`ChatGPT_review/CobaltC_Independent_Language_Design_Review_2026-10-11.md`)
found four places where the text of the abstract machine says less than, or other than, the machine both
tools run:

1. **F-02.** `[Fault-Contain]` (`spec/18`) poisons every mutex the failing thread holds (`Σ_1`) and then unwinds
   it; the unwind runs the guard's destructor `[Guard-Drop]`, which set `sync(o) := unlocked` unconditionally.
   Read literally, the poison was erased before anyone could see it, and `[Lock-Poisoned]` could never fire
   after a guard was destroyed — yet `conf.mutex-poisoned`, `conf.thread-cancelled-holding-guard` and
   `conf.mutex-poisoned-second-waiter` derive exactly that fault. `state.sync` also declared only
   `unlocked | locked-by(Thread)`, and `state.threads.value` only `Result | taken | None`, although the rules
   write `poisoned(ℓ)` and `failed(d)`.
2. **F-05.** `inv.string.utf8-validity` quantified over every live `String` object, and `[Let-Uninit]`
   (`String s;`) establishes a live `String` that holds no value yet.
3. **F-06.** `held-by` is a set of holder objects. `[Write]` released a holder `o` from every reference that
   the *written part* no longer held, and invalidated the reference when `o` had been its only holder. A
   struct holding one reference in two fields, one of them overwritten, therefore lost a reference it still
   held in the other field.

## Options

- F-02: (a) make `[Guard-Drop]` conditional on the mutex being `locked-by` a thread; (b) give the unwind a mode that skips the
  guard's destructor. (b) leaves the guard's lock path valid and needs a second destructor semantics; (a) is
  one premise.
- F-06: (a) make `held-by` a derived predicate over storage contents and remove every assignment to it from
  the rules; (b) keep the field, state the equality it must keep with the derived set, and correct the one
  rule that broke it. (a) touches eleven rules in seven files, all of which re-key or set the field correctly
  today; only `[Write]` miscounted. The action plan recommended (a); (b) gives the same machine with one rule
  changed, and is what was done — the definition the review asked for is now in `spec/04` as the field's
  meaning, so a rule that fails to keep it is a defect of that rule.

## Decision

1. `[Guard-Drop]` unlocks a mutex that is `locked-by` a thread and leaves a `poisoned` one as it is
   (`spec/19` §2, and a paragraph after the lock rules). `state.sync` gains `poisoned(Thread)`,
   `state.threads.value` gains `failed(Diagnostic)` (`spec/04`).
2. `inv.string.utf8-validity` speaks of initialized `String` objects (`init-state(o,Σ) = valid`). The other
   invariants of `spec/03` were read for the same question and need no change (CHG-0253 lists why).
3. `held-by(a)` is, in every state, the set of live objects whose stored contents contain `a`, every copy
   counting (`spec/04`); `[Write]` computes `kept` — every reference `o` holds after the write — and releases
   `o` from, and possibly invalidates, only the references in `old \ kept` (`spec/05`).

No accepted program changes outcome; no diagnostic is added.

## Results

Both tools already behaved as the corrected text says. `conf.mutex-poisoned`'s case locks after the faulting
thread's guard was destroyed and faults `diag.mutex-poisoned` under `coby` and `cobc` (both runtimes keep a
poisoned state the guard's release does not overwrite). For F-06 the probe
`stress/review2_oct11/f06b.cb` (`t.b = t.a; t.a = &y; *t.b`) printed `7 8` under `coby` and under `cobc`
with GCC and Clang before this decision: neither tool maintains a holder set by difference — `coby` finds the
live references to an object by scanning each holder's stored value (`distinct_refs_to`), and `cbrt` keeps a
reference valid while any stored copy of its token exists. The new case
`conf.reference-held-twice-in-one-object` pins the shape over struct fields and array elements. An "object
end" twin was considered and not added: once the last holder of a reference ends, no safe program can reach
the reference to observe that it was invalidated, which is the point of the rule.
