# proofs — the mechanised fragment (plan A3, D-0265)

A Lean 4 development, outside the Rust workspace, that states and proves for a fragment of CobaltC what
`spec/23-preservation.md` argues in prose for the whole language: the invariants of `spec/03` hold of every
reachable state of the fragment's abstract machine.

The fragment is `impl/model`'s (D-0219): the corner where the reviews found their soundness holes — a
reference carried across a boundary. Its machine here is written from `spec/04`'s state and `spec/05`,
`08`, `10`, `14`'s rules, not from the model checker's code, so that where the two agree the agreement
means something; the model checker's enumerator supplies the programs the two are compared on.

## Layout

- `lakefile.lean`, `lean-toolchain` — the Lake project (Lean 4 stable; `elan` installs it).
- `Cobalt/State.lean` — `σ` for the fragment: objects, access paths (mode, validity, ancestors, the
  objects holding the path), frames, the current thread's scope stack; the predicates of `spec/04` §2
  (`alive`, `temporallyValid`, `overlap`, `permitted`, `clash`).
- `Cobalt/Step.lean` — the step relation, each step performed by a running thread: establish, bind,
  borrow, project, read, write, destroy, plain end, authority transfer, block and statement enter/exit,
  spawn, thread-body-done, join, handle-destructor, lock, guard-drop, lend (a path in transit), take
  (`[Thread-Affine]`'s first access).
- `Cobalt/Invariants.lean` — `spec/23`'s auxiliary state lemmas as predicates on `σ`: (T1) a valid path's
  object is alive; (T2) `heldBy` is the set it is defined to be; (A) ancestors are well-founded; (Fresh)
  every id is below the counter; (R) every obligation is alive, has one authority thread and an owner;
  (C1, C2) a mutex is locked exactly through its one valid lock path, by that path's thread.
- `Cobalt/Preservation.lean` — the theorems: each predicate is preserved by every step, by cases on the
  step; `reachable σ → invariants σ`.
- `Cobalt/Corollaries.lean` — what an access gets in a reachable state, one lock path per mutex, and
  `clash`'s fuel adequacy.

## What is proved, and what is not

Proved (2026-10-17/18): for the fragment with threads, objects and their contents, bindings, shared and
exclusive borrows, projections (`[Field-Access]`/`[Index-Checked]`), reads, writes (`[Write]` through
any path: the written slot's references leave the object's contents and the value's join them, D-0215;
the fragment keeps no slots apart, so it admits any sublist as the slot's, a weaker premise), `[Destroy]`
of a resource and the end of a plain object, `[Authority-Transfer]` (within or across threads),
`[Block-Enter]`/`[Block-Exit]` and `[Stmt-Enter]`/`[Stmt-Exit]` over each thread's own stacks (the exits
ending the frame's owned objects, the scope's temporaries and its unheld paths), `[Spawn]`,
`[Thread-Body-Done]`, `[Join]` and `[Handle-Destructor]`, `[Lock]` (the lock path and its guard
established, the mutex locked) and `[Guard-Drop]` (the guard and its path end, the mutex unlocked),
`[Thread-Lend]` (an exclusive path borrowed into transit) and `[Thread-Affine]`'s taking (the first
access makes a transit or finished thread's path the accessing thread's; `read`, `write` and `borrow`
through an exclusive path carry the affinity premise), every step preserves (T1) a valid path's object is alive, (T2) the `held-by` equality of `spec/04` §1,
(A) a path's base is an earlier path, (Fresh) every identity and token in use is below the counter, (R)
`inv.resource-authority` for the fragment: every object under a destruction obligation is alive, exactly
one thread holds its authority, and that thread owns it by an open frame, an open statement scope, or
its finished result whose handle is still an obligation, and (C) `inv.concurrency-validity`'s state
lemma: a mutex locked by ℓ through `a` has `a` as its own exclusive lock path formed in ℓ (C1), and a
valid lock path is the lock its mutex is held through, by its thread (C2) — so at most one lock path of
a mutex is valid at a time (`one_lock_path`). So every reachable state satisfies all seven
(`reachable_invariants`), and every `read`/`write` the machine admits reaches a live object through a
valid path and passes `¬clash` by the step's own premise. `lake build` reports no `sorry`.

The third way of owning is what the model found (`CHG-0330`): `inv.resource-authority` as stated had
no case for a finished thread's result before its `[Join]`, so no state after `[Thread-Body-Done]`
satisfied (R); the rules keep those objects (the handle claims or discards them), so the proposition
was short of a case.

Modelling choices to know about: a handle ends only by `[Join]` or `[Handle-Destructor]`, and a guard
only by `[Guard-Drop]`, each as its own step (`noHandle`/`noGuard` premises on the other ends; the
rules' "blocks until done" and "the destructor runs `[Guard-Drop]`" have the same effect); a result
holding another thread's handle is joined before it is dropped; a thread's result holds only resources
the thread has authority over (the rules' arguments arrive by transfer, which re-keys authority); the
fragment has no `poisoned` state (`[Fault-Contain]` is outside it) and no `sync-exempt` in `clash` (a
lock path's accesses are not modelled against the mutex's shared paths).

`Cobalt/Corollaries.lean` states what the invariants give an access in a reachable state (a temporally
valid path reaches a live object; `held-by` is exactly the live holders; every obligation is owned) and
proves `clash`'s fuel adequate: under (A) the ancestor walk from `a` needs at most `a` steps, so the
walk with fuel `nextId` is the whole chain and more fuel changes nothing.

Not yet in the fragment: `[Fault-Contain]` and poisoning, sub-resources (`[Destroy-Composite]`),
pending call arguments (D-0107). In that order.

## Building

    cd proofs && lake build

`lake` is not part of the battery: the proofs are checked by whoever changes them, with the toolchain
`lean-toolchain` names.
