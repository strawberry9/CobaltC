# CobaltC Concurrency

Status: normative artifact
Version: 1.11.2
Conforms to: `spec/02-schema.md` (Kind: Type `type.handle`,
`type.mutex`, `type.guard`; Kind: Rule, `rule.conc.*`)
Governed by: `CobaltC_Master_Instructions.md` §17 (Aliasing and
Concurrent Access), §23
Realizes: D-0003, D-0004, D-0008, D-0016, D-0017, D-0018, D-0019

## Purpose

Threads, join handles, and the one synchronization-mediated access
mode (`mutex`/`guard`). Cross-thread aliasing needs no new check:
`clash` (`spec/04` §2) is thread-independent and evaluated at every
access in every thread, so a write in one thread while another
thread holds a conflicting reference is `diag.aliasing-conflict` at
whichever access comes second — regardless of timing. What this
artifact adds is the ability to *share* mutable state deliberately,
through a mutex whose interior is reachable only by a serialized lock.
The same property makes a path's standing local to its thread: once a
path (not lock-derived) is valid and clash-free, only its own thread's
steps can end it or make it clash, apart from a projection whose own
access faults in the other thread (`spec/08` §4, D-0188). An
implementation may rely on this to check a function's reference
arguments once, at entry, instead of at each access.
`spawn` and `join` are prelude intrinsics (`spec/21` §0), not
dedicated surface grammar (`spec/22`) — ordinary calls to `rule.conc.
spawn`/`rule.conc.join`, ever since `CHG-0008` reclassified them
alongside `drop`/`sizeof`/`allocate`. Nothing below this line changed
as a result: their reduction subjects were already call-shaped text.

## 1. Threads

### `type.handle`
**Status:** ACCEPTED

`handle<τ>`: `is-resource = true`; its built-in destructor
(`[Handle-Destructor]`, `rule.conc.join`) joins the thread. `sizeof = AddrWidth/8` (`spec/06` §7).

### `rule.conc.spawn`
**Status:** ACCEPTED

    [Spawn]
        ⟨e_f, Σ⟩ →* ⟨r_f, Σ_0⟩;  callable(type-of(r_f), (τ1..τn) -> τr);  r_f is a fn value or a move closure
        ⟨e_i, Σ_{i-1}⟩ →* ⟨r_i, Σ_i⟩ left to right, e_i in place position iff is-resource(τi)
        ℓ' fresh Thread;  f' fresh Frame
        ⟨establish(handle<τr>), Σ_n⟩ →^ℓ ⟨o_h, Σ'⟩                 -- rule.value-object.object-establish (resource)
        Σ'' = Σ'[ frame-stack(ℓ') := [f'], temp-scope-stack(ℓ') := [],
                  threads(ℓ') := { expr: body(r_f), handle: o_h, value: None },
                  init(o_h) := valid, storage(extent(o_h)) := represent(handle<τr>, ℓ') ]
        ⟨store(binding(p_i), r_i), Σ''_{i-1}⟩ →^{ℓ'} ⟨(), Σ''_i⟩   for i = 1..n   -- rule.fn.bind-param in ℓ';
                                                                  -- rule.resauth.transfer's ℓ2 ≠ ℓ case moves authority to ℓ'
        ────────────────────────────────────────────
        ⟨spawn(e_f, e1..en), Σ⟩ →^ℓ ⟨temp o_h, Σ''_n⟩

    [Thread-Body-Done]
        Σ.threads(ℓ').expr has reduced to r_b in result form; ⟨block'(f', r_b), Σ⟩ →^{ℓ'} ⟨r_b, Σ1⟩   -- parameter frame exit
        ────────────────────────────────────────────
        Σ1[ threads(ℓ').value := r_b ];  ℓ' takes no further steps

    [Thread-Step]   outcome: unspecified { any interleaving consistent with each thread's own →^ℓ order }
        ⟨Σ.threads(ℓ).expr, Σ⟩ →^ℓ ⟨e', Σ'⟩   for any live ℓ
        ────────────────────────────────────────────
        the program steps to Σ'[ threads(ℓ).expr := e' ]

A thread is a call (`rule.fn.call`) running in its own frame and
scope stacks under a fresh label; arguments are bound exactly as for a
call (resources move, references are values). They are bound by the
`[Spawn]` step itself, so a reference passed is held by the new thread
from `spawn` on, whether or not that thread has taken a step yet, until
the thread finishes. The handle is a
temporary resource that the caller stores. Steps of different threads
interleave arbitrarily at the granularity of `→`; each step is
atomic with respect to `Σ` (sequentially consistent). A closure passed
to `spawn` must be a `move` closure (`diag.spawn-borrow-closure`,
static): a borrow-capturing closure would hold references into the
spawning frame with nothing but dynamic checks between them and that
frame's exit.

**Depends on:** D-0016, D-0019, rule.fn.call, rule.fn.bind-param,
rule.value-object.object-establish, rule.resauth.transfer,
rule.fn.closure
**Affects:** state.threads, state.frame-stack, state.temp-scope-stack,
state.objects

### `rule.conc.join`
**Status:** ACCEPTED

    [Join]
        ⟨e, Σ⟩ →* ⟨place a, Σ1⟩ with of(a) = o_h : handle<τ>, e in place position;  ℓ' = value-at(handle<τ>, …)
        Σ1.threads(ℓ').value = r_b ∉ {None, taken}            -- blocks (no step) until the thread is done
        Σ1' = Σ1[ threads(ℓ').value := taken,                  -- the joiner claims the result (state.threads)
                  authority(ℓ, destroy, o) := {consumed: false}, authority(ℓ', destroy, o).consumed := true
                      for o ∈ objs-in(r_b) with (ℓ', destroy, o) ∈ dom(Σ1.authority) ]
                                                                -- re-keys the result's own top-level authority
                                                                -- (if any) from the worker to the joiner, exactly
                                                                -- as rule.resauth.transfer's ℓ2 ≠ ℓ case does for
                                                                -- rule.conc.spawn's argument-passing direction
        ⟨destroy(a), Σ1'⟩ →^ℓ ⟨(), Σ2⟩                         -- rule.resauth.destroy; [Handle-Destructor] finds the
                                                              -- result taken and discards nothing
        ────────────────────────────────────────────
        ⟨join(e), Σ⟩ →^ℓ ⟨r_b, Σ2[ objects(o).temp-scope := current-scope(ℓ,Σ2) for o ∈ objs-in(r_b),
                                    access-paths(a').temp-scope := current-scope(ℓ,Σ2) for a' ∈ refs-in(r_b) with held-by = ∅ ]⟩

    [Handle-Destructor]   -- the built-in destructor of handle<τ>, run by rule.resauth.destroy when a handle
                          -- is dropped or swept without join; ℓ is the thread running this destructor
                          -- (the one destroying the handle — possibly, but not necessarily, ℓ')
        Σ.threads(ℓ').value ≠ None                           -- blocks until done
        ────────────────────────────────────────────
        if Σ.threads(ℓ').value = r_b ≠ taken:  authority(ℓ, destroy, o) := {consumed: false},
                                               authority(ℓ', destroy, o).consumed := true
                                                   for o ∈ objs-in(r_b) with (ℓ', destroy, o) ∈ dom(Σ.authority)
                                                   -- re-keyed to the destroying thread first, exactly as [Join]
                                               — then r_b is discarded: objs-in(r_b) destroyed/ended as temporaries
                                                 of the current statement scope, and threads(ℓ').value := taken
        if Σ.threads(ℓ').value = taken:        nothing ([Join] already claimed r_b)

`join` consumes the handle (it is `destroy`), waits for the thread,
and hands its result to the joiner's statement as a temporary or
value — re-keying the result's own top-level destroy authority to the
joiner's thread first if the result is a resource, since authority is
granted to whichever thread established the object
(`rule.value-object.object-establish`), which for a value returned out
of a spawned thread's body is the worker, not the joiner. Destroying a
handle any other way — `drop(h)` or automatic sweep at block exit —
also waits for the thread, re-keys the same way, and discards the
result: a thread can never outlive the frame that owns its handle,
which is what lets `rule.temporal.ref-escape` and the dynamic checks
treat references passed to threads like references passed to any
other call. Waiting is blocking, not failure: the absence of a step
is not `↛`; a wait that can never end is `[Deadlock]` (§4).

    [Join-Failed]   -- D-0204
        Σ1.threads(ℓ').value = failed(d)                        -- its fault contained (spec/18 [Fault-Contain])
        ────────────────────────────────────────────
        ⟨join(e), Σ⟩ ↛ d                                         -- raised again in the joiner, at d's own location;
                                                                 -- [Handle-Destructor] likewise, unless the destroying
                                                                 -- thread is unwinding, or cancelled ℓ' and d is
                                                                 -- diag.thread-cancelled (then the failure is discarded)

A failed thread's failure is raised again wherever its handle is joined or
ends, so a program that never asks for failures as values fails as a whole,
one scope at a time. `std::try_join(h) : Result<R, ThreadFailure>`
(`spec/21` §3c) asks: it waits as `join` does and gives the failure as a
value, taking it, so the handle raises nothing. `std::is_finished(&h)` tells
whether the thread has ended -- with a result or a failure -- without
waiting: how a supervisor finds which threads to take. A `ThreadFailure`
gives the report the program would have printed (`text`) and the
diagnostic's name (`diagnostic`).

**Depends on:** rule.resauth.destroy, D-0008, D-0016, D-0019, D-0204
**Affects:** state.threads, state.objects

### `rule.conc.cancel`
**Status:** ACCEPTED

    [Cancel]   -- D-0204; cancel is a prelude intrinsic (spec/21 §0)
        ⟨e, Σ⟩ →* ⟨v, Σ1⟩,  v = a_h : ref<handle<τ>, shared>;  ℓ' = the thread a_h's handle names
        ────────────────────────────────────────────
        ⟨cancel(e), Σ⟩ →^ℓ ⟨(), Σ1[ cancelled(ℓ') := true ]⟩

    [Cancelled-Wait]   disposition: checked
        cancelled(ℓ') ∧ contained(ℓ', Σ) ∧ ℓ' is waiting, or begins to, in [Join], [Lock], a channel's
        [Send] or [Recv], `sleep`, or a socket's wait (spec/21 §2l), and is not unwinding a contained fault
        ────────────────────────────────────────────
        that wait ↛ diag.thread-cancelled   in ℓ'                 -- contained like any fault (spec/18)

A thread is asked to stop, and does at its next wait: the fault unwinds it
(its destructors run, its own threads are cancelled in turn), and its result
is the failure. A thread not waiting runs on until it waits or ends -- a
cancelled thread that never waits finishes its work. A thread lent an
exclusive reference is never cancelled (`contained` is false): stopping it
would leave the data it was changing half-changed. Cancelling a thread that
has ended changes nothing.

**Depends on:** rule.conc.spawn, rule.conc.join, rule.fail.fault-unwind, D-0204
**Affects:** state.threads

## 2. Mutex

### `type.mutex`, `type.guard`
**Status:** ACCEPTED

`mutex<τ>`: `is-resource = true`; a struct-like object whose layout is
one field, `inner` of type `τ`, plus `sizeof(usize)` of implementation state
(`outcome: impl-defined` for the extra cells' meaning; they are never
read by any rule). Its built-in destructor destroys `inner`
(`destroy-composite`). `guard<τ>`: `is-resource = true`; values are
lock-path tokens; built-in destructor `[Guard-Drop]`.

### `rule.conc.lock`
**Status:** ACCEPTED

    [Mutex-New]                                               -- the construction spec/21 §0's `Mutex::new` names
        ⟨e, Σ⟩ →* ⟨r, Σ1⟩, e in place position iff is-resource(τ)
        ⟨establish(mutex<τ>), Σ1⟩ →^ℓ ⟨o, Σ2⟩;  ⟨store(sub(o, inner, τ), r), Σ2⟩ →^ℓ ⟨(), Σ3⟩
        ────────────────────────────────────────────
        ⟨Mutex::new(e), Σ⟩ →^ℓ ⟨temp o, Σ3[ init(o) := valid, sync(o) := unlocked ]⟩

    [Lock]
        ⟨e, Σ⟩ →* ⟨v, Σ1⟩,  v = a_m : ref<mutex<τ>, shared>;  o = of(a_m)
        sync-state(o, Σ1) = unlocked                          -- blocks (no step) while locked-by(ℓ'), ℓ' ≠ ℓ
        a_g ∉ dom(Σ1.access-paths)
        ────────────────────────────────────────────
        ⟨lock(e), Σ⟩ →^ℓ
            ⟨temp o_g, Σ2⟩
            where Σ2 = Σ1 with: sync(o) := locked-by(ℓ);
                  access-paths(a_g) := { target: sub-range(o, inner), of: o, type: τ, mode: exclusive,
                                         thread: ℓ, valid: true, formed-at: this-event,
                                         frame: current-frame(ℓ,Σ1), temp-scope: current-scope(ℓ,Σ1), base: a_m, held-by: ∅ };
                                       -- base := a_m: the locking reference and its ancestors (the mutex's
                                       -- owning binding) are ancestors of the lock path, not competitors
                  o_g established as a guard<τ> resource object (object-establish) with
                    storage represent(guard<τ>, a_g), init valid, and a_g.held-by := {o_g}

    [Lock-Reentrant]   disposition: checked   sync-state(o, Σ1) = locked-by(ℓ)   ⟨lock(e), Σ⟩ ↛ diag.mutex-reentrant-lock

    [Lock-Poisoned]   disposition: checked   -- D-0204
        sync-state(o, Σ1) = poisoned(ℓ')                      -- ℓ' held the lock when its fault was contained
        ────────────────────────────────────────────
        ⟨lock(e), Σ⟩ ↛ diag.mutex-poisoned

    [Guard-Deref]
        ⟨e, Σ⟩ →* ⟨place a_gd, Σ1⟩ or ⟨temp o_g, Σ1⟩,  of type guard<τ>     -- e in place position (spec/13 §1): a guard is a
                                                                            -- resource and is never read as a value
        a_g = value-at(guard<τ>, Σ1.storage, target(a_gd,Σ1))  (resp. extent(o_g,Σ1))    -- spec/06 [Repr-Guard]
        ────────────────────────────────────────────
        ⟨*e, Σ⟩ →^ℓ ⟨place a_g, Σ1⟩                                          -- a place expression; spec/09 §2

    -- `g.f` and `g[i]` are `(*g).f` and `(*g)[i]`: spec/16 [Guard-Auto-Deref] (D-0064)

    [Guard-Drop]   -- built-in destructor of guard<τ>, run by rule.resauth.destroy
        ────────────────────────────────────────────
        Σ[ sync(o) := unlocked, access-paths(a_g).valid := false ]

    lock-derived(a, Σ)        ≝  some path in ancestors(a, Σ) was formed by [Lock]
    sync-exempt(a, m, a', Σ)  ≝  (lock-derived(a) ∧ ¬lock-derived(a') ∧ mode(a') = shared)
                                ∨ (lock-derived(a') ∧ ¬lock-derived(a) ∧ m = shared)

`lock` takes a shared reference to a mutex — which any number of
threads may hold — and yields a guard owning an exclusive root path
into the mutex's interior; the interior is reachable through no other
path (a mutex has no projections), and at most one lock path is live
at a time (`[Lock]` blocks otherwise), so accesses through it can
never race. `sync-exempt` is the one extension to `clash`
(`spec/04` §2): a lock-derived path and a shared path that is not
lock-derived do not conflict with each other — whether the shared path
is to the mutex or to an aggregate around it (a struct with a `mutex`
field, shared between threads by reference; `CHG-0072`), it can reach
nothing inside the mutex, which only a lock reaches. Two lock-derived
paths are checked as any two paths are. The mutex's serialization is what
`synchronized(a1, a2, o, Σ)` (`inv.concurrency-validity`) means
beyond D-0004's baseline. Everything else is unchanged: destroying or
moving a mutex while a guard is live is `diag.destroy-while-aliased`/
`diag.move-while-aliased` (`solitary` fails); a guard is released by
`drop` or at its owner's block exit (D-0008, RAII); locking a mutex
the same thread already holds is a fault, not a deadlock.

Sharing a mutex between threads is by `ref<mutex<τ>, shared>`; a
thread holding such a reference cannot outlive the mutex's frame
(§1 `[Handle-Destructor]`). Atomics and a weak memory model are not
provided (`spec/22` §5).

**Depends on:** D-0004, D-0008, D-0018, inv.concurrency-validity,
rule.alias.borrow, rule.resauth.destroy,
rule.value-object.object-establish
**Affects:** state.sync, state.access-paths, state.objects

## 3. Channels

### `rule.conc.channel`
**Status:** ACCEPTED

`Channel<T>` (D-0063) is a resource type of `std`, written in CobaltC
over `mutex` (`spec/21` §3c, `rule.stdlib.channel`): a queue of at most
`cap` values of type `T`, shared between threads by
`ref<Channel<T>, shared>` as a mutex is. Its state is `(q, closed)`: the
values in it, oldest first, and whether it is closed.

    [Channel-New]
        ⟨Channel::new(cap), Σ⟩ → ⟨temp o, Σ'⟩, o a Channel<T> with q = [], closed = false,
        holding at most cap values                                             when cap ≥ 1
    [Channel-Zero-Capacity]   disposition: checked   cap = 0   ⟨Channel::new(cap), Σ⟩ ↛ diag.channel-zero-capacity

    [Send]
        closed = false, |q| < cap:   ⟨Channel::send(ch, v), Σ⟩ → ⟨Ok(()), Σ'⟩ with q := q · [v]   (v moved in)
        closed = true:               ⟨Channel::send(ch, v), Σ⟩ → ⟨Err(v), Σ⟩                     (v given back)
        closed = false, |q| = cap:   no step: blocks until another thread changes the channel

    [Recv]
        q = [v] · q':                ⟨Channel::recv(ch), Σ⟩ → ⟨Some(v), Σ'⟩ with q := q'          (v moved out)
        q = [], closed = true:       ⟨Channel::recv(ch), Σ⟩ → ⟨None, Σ⟩
        q = [], closed = false:      no step: blocks until another thread changes the channel

    [Close]
        ⟨Channel::close(ch), Σ⟩ → ⟨(), Σ'⟩ with closed := true                 (already closed: unchanged)

    [Channel-Destroy]   -- its destructor
        the values of q are destroyed, oldest first; then the channel's own state

    [Channel-Deadlock]   disposition: checked
        [Send] or [Recv] blocks in the main thread while no spawned thread is running
        ────────────────────────────────────────────
        ↛ diag.channel-deadlock

Each step is taken under the channel's lock (`[Lock]`), so the steps of
different threads on one channel are serialized. Blocking is the
absence of a step, as for `[Lock]` and `[Join]`: not failure, and a
waiting thread's step is taken once another thread's `send`, `recv` or
`close` makes one possible. When several threads wait, which one
proceeds is `outcome: unspecified`. A value sent is received at most
once, in the order it was sent relative to the other values of that
channel. The main thread waiting alone is `[Channel-Deadlock]`; any other wait
that can never end is `[Deadlock]` (§4, D-0204).

**Depends on:** D-0063, rule.conc.lock, rule.conc.spawn, rule.conc.join,
rule.resauth.destroy, rule.stdlib.channel
**Affects:** the channel's state

## 4. The guarantee

### `rule.conc.guarantee`
**Status:** ACCEPTED

What every program without `unsafe` (`spec/20`) can rely on when it runs
threads, gathered from the rules above. Each clause is a consequence of
them; the rule adds none, but states what an implementation must not
depart from, whatever it proves in advance or runs in parallel.

    [Conc-Interleaving]   outcome: unspecified { which interleaving }
        every execution of the program is a sequence of steps of its threads, each step
        one `→^ℓ` of `[Thread-Step]`, atomic with respect to Σ: sequentially consistent

    [Conc-No-Race]
        a1 an access of thread ℓ1 and a2 an access of thread ℓ2 ≠ ℓ1 to overlapping cells of one
        object, at least one of them a write, neither ordered after the other by `[Join]`,
        `[Lock]`/`[Guard-Drop]` of one mutex, or `[Send]`/`[Recv]` of one value
        ────────────────────────────────────────────
        the later of the two is `↛ diag.aliasing-conflict` (or a stale or moved-from diagnostic
        when the earlier ended the object), whatever the timing: there is no data race

    [Conc-Defined]
        every step of every interleaving is defined: an access that is not permitted is a
        diagnostic, never undefined behaviour

    [Conc-Checks-Omitted]   -- an implementation obligation
        an implementation may leave out a check (a `discharge: static` proof, `spec/03`) only where
        it cannot fail in any interleaving of any thread's steps

    [Deadlock]   disposition: checked   -- D-0204
        thread ℓ waits ([Join], [Lock], a channel) and either
          (a) its wait closes a cycle: following each wait to the thread it waits on -- a joined thread,
              a mutex's holder -- comes back to ℓ, no thread on the way being cancelled; or
          (b) every live thread waits on another thread, a mutex or a channel, none on the clock, a
              socket or input, and none can end its wait -- and ℓ is one member of a cycle as in (a), if
              there is one; otherwise ℓ is each thread at the end of a chain of waits (one waiting on a
              channel, not on a thread or a mutex), whose failures then reach the threads waiting on it
              through [Join-Failed] and [Lock-Poisoned]
        ────────────────────────────────────────────
        that wait ↛ diag.deadlock   in ℓ                   -- contained like any fault (spec/18)

`[Conc-No-Race]` holds because a path's standing is local to its thread
(Purpose): what one thread holds, another thread's access finds in
`clash`, and a reference passed to `spawn` is held from the spawn
(§1). Which of two permitted outcomes a run shows can depend on timing:
a thread that has already finished holds nothing, so an access that
would conflict with it while it ran is permitted after. Both tools show
only permitted outcomes; they may differ in which (`coby` runs threads
one at a time). Deadlock is not ruled out, but it is diagnosed:
`[Deadlock]` faults a thread whose wait closes a cycle, and, when every
thread waits and none can go on, one of a cycle or the ends of the chains
of waits -- so the outcome does not depend on timing; the main thread
waiting alone on a channel remains `[Channel-Deadlock]`. Because the
fault is contained, a deadlocked pair of handlers in a server is reported
and the server goes on.

What is not decided: a wait on a channel while another thread is in a
call outside the program (a socket, input, the clock). That thread may
send once its call returns, and which threads can reach a channel is not
known at run time, so such a wait is never a deadlock, however long it
lasts. A program that must not wait forever there says how long it will
wait (`Channel::recv_timeout_ms`), or a supervisor `cancel`s it;
`--explore` (both tools) finds orders that leave a thread waiting.

**Depends on:** inv.concurrency-validity, rule.conc.spawn,
rule.conc.join, rule.conc.lock, rule.conc.channel, D-0203
**Affects:** none (a statement of what the rules above establish)

## Change Log

- 1.11.2 — `CHG-0245` (D-0207): `rule.conc.cancel` depends on
  `rule.fail.fault-unwind`, where `[Fault-Contain]` is stated, not on a
  `rule.fail.fault-contain` that does not exist. No rule changed.
- 1.11.1 — `CHG-0234` (D-0204): `[Deadlock]` (b) without a cycle faults the
  ends of the chains of waits, so its outcome does not depend on timing; §4
  says which waits are not decided.

- 1.11.0 — `CHG-0233` (D-0204): `[Join-Failed]`; `[Handle-Destructor]`
  raises a failure nobody took, and during a contained fault's unwind
  cancels its thread first; `rule.conc.cancel` (`[Cancel]`,
  `[Cancelled-Wait]`); `[Lock-Poisoned]`; `[Deadlock]` in §4.

- 1.10.0 — `CHG-0232` (D-0203): §4 (new) `rule.conc.guarantee` states
  what threads can rely on (`[Conc-Interleaving]`, `[Conc-No-Race]`,
  `[Conc-Defined]`) and what an implementation may omit
  (`[Conc-Checks-Omitted]`). §1 says that `[Spawn]` binds the parameters
  itself, so a reference passed is held from the spawn. No other rule
  changed.

- 1.9.0 — `CHG-0216` (D-0188): Purpose notes that a path's standing is
  local to its thread (`spec/08` §4). No rule changed.

- 1.8.1 — `[Handle-Destructor]` under `[Fault-Unwind]` (`spec/18` §1): no wait for the
  thread, which takes no further steps. Clarification; `conf.fault-with-live-thread`.

- 1.8.0 — `CHG-0074` (D-0064): `[Guard-Deref]` notes `spec/16`
  `[Guard-Auto-Deref]`.
- 1.7.0 — `CHG-0073` (D-0063): §3 (new) `rule.conc.channel`.
- 1.6.0 — `CHG-0072` (D-0063): `sync-exempt` exempts a lock-derived
  path from every shared path that is not lock-derived, not only from
  shared references of type `mutex<_>`; a struct holding a mutex can be
  shared between threads by reference.

- 1.5.0 — `CHG-0015`: `[Join]`/`[Handle-Destructor]` re-key a
  resource-typed thread result's top-level destroy authority from the
  worker thread to the destroying thread. `authority` is keyed by
  `Performer` (`spec/04`), granted to whichever thread established the
  object (the worker, for a value returned out of a spawned body); with
  nothing to transfer it, a joiner or an automatic handle sweep could
  never actually destroy a resource-typed thread result —
  `[Destroy-No-Authority]` would fire on the very first attempt. No
  existing conformance case exercised a resource-typed thread result
  (`conf.spawn-join-value` and every other `spec/19` case return
  `i32`), so the fix changes no existing outcome; found deriving
  `conf.spawn-join-resource-result`. `objs-in(r_b)` for a plain value
  is empty, so both added clauses are vacuous whenever the result is
  not a resource.
- 1.4.0 — `CHG-0010`: `[Guard-Deref]` restated over a place or
  temporary guard result (its operand is a place position, `spec/13`
  1.5.0); the 1.3.0 form read the guard as a value, which
  `[Read-Resource-Rejected]` forbids.
- 1.3.0 — `CHG-0009`: `[Join]` claims the thread's result
  (`threads(ℓ').value := taken`) before destroying the handle, and
  `[Handle-Destructor]` discards a result only if it is still
  unclaimed. As written in 1.2.0, `[Join]` invoked `destroy`, whose
  `[Handle-Destructor]` discarded `r_b`'s temporaries — the very
  objects `[Join]` then returned — while the rule's own comment called
  the destructor "a no-op". Non-normative in the same pass:
  `[Mutex-New]` moved under the `rule.conc.lock` heading that
  `spec/21` §0 cites for it; `type.handle`'s destructor citation
  corrected from `[Join]` to `[Handle-Destructor]`.
- 1.2.0 — `CHG-0008`: noted that `spawn`/`join` are now classified as
  prelude intrinsics (`spec/21` §0) rather than dedicated `spec/22`
  grammar. No rule in this file changed — `[Spawn]`/`[Join]`'s
  reduction subjects were already call-shaped text, unaffected by the
  reclassification.
- 1.1.0 — `CHG-0001`: mutex layout description rephrased in prose to
  avoid a colon-bracket shape that could be misread as surface
  struct-literal syntax; no rule semantics changed.
- 1.0.0 — Rewritten per D-0018/D-0019 (`spec/AUDIT-2.md` B-13, B-17):
  per-thread stacks; `[Spawn]` yields a temporary handle and binds
  parameters via `store` in the new thread; `[Join]` and the handle
  destructor both wait, so no thread outlives its handle's frame;
  `[Lock]` produces a guard resource owning a lock path, with
  `sync-exempt` as the sole extension to `clash`; reentrant lock
  diagnosed. All entities `ACCEPTED`.
- 0.3.0 and earlier — superseded.
