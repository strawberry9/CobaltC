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
  (`[Thread-Affine]`'s first access), fault-contain (the unwind of a contained thread, its mutexes
  poisoned), temp-root, a call's parameter binding and the one-access settlement (pending paths, D-0107
  and D-0242), relocate-in, relocate-out and destroy-composite (sub-resources), reclaim and its
  re-attachment, release, the moves into and out of raw storage (spec/20 §2), and since 2026-11-07 an
  atomic's operation and its notify (`state.sync`'s wake count), a `match`'s test of its scrutinee, and a
  channel's receive (the received value's paths, lent into the channel, join the receiver's scope).
- `Cobalt/Invariants.lean` — `spec/23`'s auxiliary state lemmas as predicates on `σ`: (T1) a valid path's
  object is alive; (T2) `heldBy` is the set it is defined to be; (A) ancestors are well-founded; (Fresh)
  every id is below the counter; (R) every obligation is alive, has one authority thread and an owner;
  (C1, C2) a mutex is locked exactly through its one valid lock path, by that path's thread; (P) a
  pending path is held by no object; (RS) a sub-obligation's container is under obligation with the same
  authority thread. (R)'s owner has the fourth case of `inv.resource-authority`, a reclaimed object.
- `Cobalt/Preservation.lean` — the theorems: each predicate is preserved by every step, by cases on the
  step; `reachable σ → invariants σ`.
- `Cobalt/Corollaries.lean` — what an access gets in a reachable state, one lock path per mutex, a bound
  parameter's conflicts found at use, a sub-obligation owned through its container, and `clash`'s fuel
  adequacy.
- `Cobalt/Elision.lean` (D-0270) — the elision lemmas: `StepBy` (a step with its performer), and for a
  standing path of ℓ that another thread leaves alone, `standing_step`/`standing_run` (`[Standing-Local]`,
  `spec/08` §4: valid, the same object, projection and mode after any step of another thread), the
  conflict theorems (a standing path is a `clash` witness for every overlapping access in an unpermitted
  mode, so that access is refused by its own premise), and `unreached_stays` (D-0200: an object no valid
  path reaches is reached by no other thread's step).
- `Cobalt/Exec.lean` (D-0270) — the machine executable: `Act`, the decidable premises (`check`), the step
  performed (`apply`), and `exec_sound`: an admitted action is a `Step`.
- `Cobalt/Statics.lean` (D-0300, D-0299 step 3) — the static rules for a core of the fragment against the
  machine: the facts `deriv(r, x.π, m)` of `spec/14` §6 as an abstraction of the machine's state
  (`Describes`: every valid, non-pending path on an owner's object other than its root is a reference
  binding's, and a fact says so with its projection and mode); `proven_no_clash`, the discharge table's
  proven row (an access no fact meets in a mode it does not permit passes `¬clash`); a lemma for each
  transfer row the core has (`row_read`, `row_write`, `row_end` for a reference's last use,
  `row_borrow` and `row_borrow_field` for `&_m x` and `&_m x.f`, `describes_mono` for a merge or a loop's
  post-fixed point); and `run_sound`, which composes them over a run (`Run`) of the core's commands.
- `Cobalt/Statements.lean` (2026-11-07) — each result above restated with its full statement (an
  `example` the build checks against the theorem), and the axioms each rests on checked under
  `#guard_msgs` against `#print axioms`: `lake build` fails if a result comes to rest on an axiom other
  than those listed, `sorryAx` (an unfinished proof) included. Every one rests on Lean's own three at most
  (`propext`, `Classical.choice`, `Quot.sound`); `COVERAGE.md` has the table.
- `Replay.lean`, `lake exe replay TRACE` (D-0270) — a trace of the model checker (`cobmodel trace`) run
  through the executable machine; the identities mapped as the machine allocates them; the first
  refusal reported. `impl/tools/replay_model.py DIR` runs a directory of programs and tallies.
- `COVERAGE.md` — generated by `impl/tools/proofcover.py`: each rule row of `spec/23` with its Lean step
  or the reason it is outside the fragment, and the axioms of each result of `Statements.lean`; `--check`
  is a battery step and fails on an axiom other than Lean's three.

## What is proved, and what is not

Proved (2026-10-17/18): for the fragment with threads, pending paths, sub-resources, objects and their contents, bindings, shared and
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
through an exclusive path carry the affinity premise), and `[Fault-Contain]` (a contained thread's
mutexes poisoned, its lock paths and its objects ended, its result the failure; `contained` is "no
exclusive path lent to it or taken over by it"), the pending paths of D-0107 and D-0242 (`clash` does not
count a pending path; an argument's `[Borrow]` and every projection form one; `[Temp-Root]` roots a
temporary; `[Call]`'s `store(binding(p_i))` binds an argument's path to a fresh parameter object of the
parameter frame, which holds it, and the path is pending no longer; a projection's one access ends it,
`settle`), and sub-resources (`[Relocate-In]`: the source's references and its obligations, re-keyed
under the container's identity and the sub-path, with their authority threads, then the source's end;
`[Relocate-Out]`: a fresh temporary resource takes a sub-range's references and sub-obligations;
`[Destroy-Composite]`: a container's sub-obligations discharged), and raw storage (`[Reclaim]`: a fresh
reclaimed object, a resource under the reclaiming thread's authority, with its root path, or the
re-attachment of one, which takes its authority, CHG-0030; `[Release]`: reclaimed objects end with their
obligations unfulfilled, none foreign-held, D-0214; `[Rawptr-Move-In]`/`[Rawptr-Move-Out]`: a whole
object's references, obligation and sub-obligations move to a fresh reclaimed object, or a reclaimed
object's to a fresh temporary, and the source ends), every step preserves (T1) a valid path's object is alive, (T2) the `held-by` equality of `spec/04` §1,
(A) a path's base is an earlier path, (Fresh) every identity and token in use is below the counter, (R)
`inv.resource-authority` for the fragment: every object under a destruction obligation is alive, exactly
one thread holds its authority, and that thread owns it by an open frame, an open statement scope, its
finished result whose handle is still an obligation, or the object is reclaimed (the `unsafe` author's,
under `discharge: trusted`), and (C) `inv.concurrency-validity`'s state
lemma: a mutex locked by ℓ through `a` has `a` as its own exclusive lock path formed in ℓ (C1), and a
valid lock path is the lock its mutex is held through, by its thread (C2) — so at most one lock path of
a mutex is valid at a time (`one_lock_path`), (P) a pending path is held by no object (its exemption
from `clash` lasts from formation to binding or its one access, and no store intervenes: `write`'s
premise is that a stored reference was bound or accessed), and (RS) `inv.resource-authority`'s third
owner case for the fragment: a sub-obligation `(o, q)` has `q ≠ ε` and a container `o` itself under
obligation (an aggregate holding a resource is a resource, `spec/12` §3) with the same authority thread,
so that (R)'s owner of `o` is the sub-obligation's (`sub_obligation_owned`). So every reachable state
satisfies all nine (`reachable_invariants`), every `read`/`write` the machine admits reaches a live object
through a valid path and passes `¬clash` by the step's own premise, and once `[Call]` has bound an
argument's path, another valid non-pending overlapping path of its object in an unpermitted mode is a
`clash` witness for every access through it (`bound_parameter_clashes`: D-0022's conflict at first use is
not hidden by the exemption the arguments had while pending). `lake build` reports no `sorry`.

The third way of owning is what the model found (`CHG-0330`): `inv.resource-authority` as stated had
no case for a finished thread's result before its `[Join]`, so no state after `[Thread-Body-Done]`
satisfied (R); the rules keep those objects (the handle claims or discards them), so the proposition
was short of a case.

Premises added for the elision lemmas and the replay (D-0270, each the rule's own): `clash` counts no
lock-derived path on either side (`sync-exempt`); `[Destroy]` and `[Object-End]` require `solitary`, the
latter by the owner; `[Authority-Transfer]` forms the new binding's root path itself (`transferTo`) and
moves an obligation whose source path is alone; `[Binding-Form]` and `[Temp-Root]` are of a temporary of
the thread's current scope, within a statement; `[Relocate-In]` and `[Rawptr-Move-In]` move the thread's
own object; `noGuard` concerns live guards.

`[Object-End]`'s `Dead` — a path whose only holder ended, or which `[Write]` released after its forming
statement, is invalid — is the step `dead` since D-0300: performed by the path's own thread, with no live
holder, not pending and not its object's holder root; every invariant is preserved by it, and the elision
lemmas hold over it (another thread's `dead` cannot end a standing path, which is its own thread's). The
replay takes a trace's `inv` as that step where its premises hold, as a counted gap otherwise. One reading
the fragment does not yet take, which the replay applies as a counted gap (`rsc` in a trace) rather than
a step: `[Stmt-Exit]`'s result exemption — a statement result's unheld paths pass to the parent scope
(the fragment's `stmtExit` would end them). Closing it is the next milestone: `stmtExit` changes, and
`Preservation`'s lemmas about it follow.

Modelling choices to know about: a handle ends only by `[Join]` or `[Handle-Destructor]`, and a guard
only by `[Guard-Drop]`, each as its own step (`noHandle`/`noGuard` premises on the other ends; the
rules' "blocks until done" and "the destructor runs `[Guard-Drop]`" have the same effect); a result
holding another thread's handle is joined before it is dropped; a thread's result holds only resources
the thread has authority over (the rules' arguments arrive by transfer, which re-keys authority); the
fragment has no `sync-exempt` in `clash` (a lock path's accesses are not modelled against the mutex's
shared paths), and `contained` leaves out the interior types of a thread's mutexes (the fragment's
mutexes hold no references); `state.sync` is keyed by identity, not by place (D-0310): the fragment's mutexes are whole objects, `[Mutex-New]`'s, whose place is `(o, ε)`, and a relocation does not carry a mutex's state (the rules re-key it with the value), so a mutex moved into a field, and two mutexes in one object, are outside what it models; a projection's one access and the end of its path are two steps (the
access, then `settle`), not one; `[Relocate-In]` requires the target sub-range to hold no obligation and
the source to be no handle or guard (a thread's handle is an object identity in the model, which a
relocation would turn into a sub-path), and `[Relocate-Out]` moves a sub-range that is itself a resource
(a plain sub-range's move is an establish and a write); the fragment keeps no slots apart, so the
references a relocation moves are any sublist of the container's contents (a weaker premise), and it
states no prefix-closure of sub-obligation keys (the rules give one: every prefix of a sub-resource's
sub-path is a resource sub-range); `[Join]` and `[Handle-Destructor]` require that the finished thread
holds no reclaimed obligation (the rules leave such an obligation to the `unsafe` author with no thread
behind it, which (R)'s "one authority thread" cannot state once the thread's record is gone), and the
fragment has no addresses, so `[Reclaim]`'s `reclaimed-at` lookup is the choice between its two steps
and `[Release]`'s range is the set of objects it ends.

`Cobalt/Corollaries.lean` states what the invariants give an access in a reachable state (a temporally
valid path reaches a live object; `held-by` is exactly the live holders; every obligation is owned) and
states a reclaimed obligation's owner, and proves `clash`'s fuel adequate: under (A) the ancestor walk from `a` needs at most `a` steps, so the
walk with fuel `nextId` is the whole chain and more fuel changes nothing.

The fragment now has every item D-0265 named for it and raw storage besides, and since 2026-11-07
`[Match]` (its test of the scrutinee a step with `[Read]`'s premises; the rest of the rule, `[Temp-Root]`,
`[Relocate-Out]` of the payload, its binding and the scrutinee's end, the steps they are),
`[Atomic-Op]` and `[Atomic-Notify]` (an atomic's `sync` is unlocked or a wake count, never a lock state,
so (C1) and (C2) are kept), and `[Recv]` (the value leaves the channel's queue by `[Relocate-Out]`; its
references' paths, lent into the channel by `[Thread-Lend]` and so of no thread, join the receiver's
scope). Calls as a whole (`[Call]`'s frame is `[Block-Enter]`, its binding is a step, its body is the
fragment's other steps) are compositions, and foreign calls remain outside (below).

The static rules that discharge the machine's premises before the run — `rule.control.flow-analysis` and,
since D-0282, `rule.control.frame-references` (`spec/14` §6), on which D-0281's decided programs rely to
keep no record of their references — are mechanised for a core since D-0300 (`Cobalt/Statics.lean`, above):
owners, references borrowed from them or from their fields, reads, writes, a reference's end, merges and
loops' post-fixed points, one thread. Outside that core they are not: reborrows through a reference (facts
rooted at `*q`), copies of references (inherited facts), calls (the call-site rule and its summaries),
threads (scoped lending), containers' elements and the block's-end rule are checked by the model checker's
statics (`impl/model/src/statics.rs`, `frames.rs`) against its exploration of every interleaving
(`model.proven-but-false`, `model.decided-but-false`), by a second reading over the whole language for a
core of it (`impl/tools/factcheck.py`, D-0300), and by testing.

## Coverage of `spec/23`'s rules

Of the 70 rule labels `spec/23` names, the fragment has 39 as steps: `[Object-Establish-Plain]`,
`[Object-Establish-Resource]`, `[Binding-Form]`/`[Store-Binding-Temp]`, `[Borrow]`, `[Field-Access]`,
`[Index-Checked]`, `[Temp-Root]`, `[Read]`, `[Write]`, `[Destroy]`, `[Destroy-Composite]`, `[Object-End]`,
`[Authority-Transfer]`, `[Relocate-In]`, `[Relocate-Out]`, `[Call]` (its parameter binding; its frame
is `[Block-Enter]`), `[Block-Enter]`, `[Block-Exit]`, `[Stmt-Enter]`, `[Stmt-Exit]`, `[Spawn]`,
`[Thread-Body-Done]`, `[Join]`, `[Handle-Destructor]`, `[Mutex-New]` (an establish), `[Lock]`,
`[Guard-Drop]`, `[Thread-Lend]`, `[Thread-Affine]`, `[Fault-Contain]`, `[Reclaim]`, `[Release]`,
`[Rawptr-Move-In]`, `[Rawptr-Move-Out]`, `[Match]`, `[Atomic-Op]`, `[Atomic-Notify]`, `[Recv]`. Outside it: the refusals (`[Read-Stale]`, `[Write-Conflict]`,
`[Lock-Poisoned]`, … — a refusal is not a step, and the fragment's steps carry the premises they check),
the value constructions and stores that only move references between slots (`[Struct-Construct]`,
`[Store-Sub-Value]`, `[Store-Result-Place-Release]`, `[Rawptr-Write]`, …, which the fragment's `write`
stands for), allocation itself (`[Allocate]`/`[Deallocate]`: cells, which the fragment has not), and
foreign calls (`[Extern-Call]`, `[Trust-Transition]`).

## The replay against the model (D-0270)

    cobmodel trace prog.cb --out prog.tr        # one run, as the fragment's actions
    lake exe replay prog.tr                     # admitted whole, stopped outside, or REFUSED
    python3 impl/tools/replay_model.py impl/model/sample --check

Over the model checker's 315 sample programs (2026-10-30, edition 2026.103001): 199 runs replay whole, 116
stop where the model leaves the fragment (vectors 65, channels 30, closures 12, arrays 4, jobs 3, atomics 1,
swaps 1), none refused; 6,728 actions admitted, 219 gaps applied. (2026-10-19: 178 whole, 137 outside, 7,181
actions; the difference is the samples D-0282's frame-reference rule now refutes before the run: a statically
rejected program has no run, and its empty trace replays whole.) A refusal is a finding: a model step the
fragment's rules do not admit, to be settled as a fragment gap, a model defect or a text defect. The executable machine
(`Exec.lean`'s `Act`) has not the four steps of 2026-11-07 (`[Match]`, the atomics, `[Recv]`): a trace still
stops where it reaches a channel or an atomic, as it did before they were steps of the relation.

## Building

    cd proofs && lake build                     # the library and the replay executable

`lake build` is a battery step (stage 1, with `proofcover --check` and the sample replay) since D-0270; the toolchain
`lean-toolchain` names.

Two kernels besides the one `lake build` runs check the development again (the release unit
`proofs-kernels` of `impl/gate.sh`, since 2026-11-07):

    cd proofs && lake env leanchecker Cobalt.State … Cobalt.Statements     # Lean's checker over the .olean files
    lake env leanexport Cobalt.State … Cobalt.Statements > export.ndjson
    nanoda_bin config.json                      # an independent kernel (Rust) over the export

nanoda is given Lean's three axioms and `Lean.trustCompiler` as permitted; the export declares core's
`sorryAx` whatever uses it, so it is not told to refuse an unpermitted axiom: which axioms each result rests
on is `Statements.lean`'s check, which the build makes and leanchecker replays. nanoda's part is the typing
of every declaration (60,601 on 2026-11-07, none refused).
