# CobaltC Concurrency

Status: normative artifact
Version: 1.5.0
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
call (resources move, references are values). The handle is a
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
is not `↛`. Deadlock is out of scope (`spec/22` §5).

**Depends on:** rule.resauth.destroy, D-0008, D-0016, D-0019
**Affects:** state.threads, state.objects

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

    [Guard-Deref]
        ⟨e, Σ⟩ →* ⟨place a_gd, Σ1⟩ or ⟨temp o_g, Σ1⟩,  of type guard<τ>     -- e in place position (spec/13 §1): a guard is a
                                                                            -- resource and is never read as a value
        a_g = value-at(guard<τ>, Σ1.storage, target(a_gd,Σ1))  (resp. extent(o_g,Σ1))    -- spec/06 [Repr-Guard]
        ────────────────────────────────────────────
        ⟨*e, Σ⟩ →^ℓ ⟨place a_g, Σ1⟩                                          -- a place expression; spec/09 §2

    [Guard-Drop]   -- built-in destructor of guard<τ>, run by rule.resauth.destroy
        ────────────────────────────────────────────
        Σ[ sync(o) := unlocked, access-paths(a_g).valid := false ]

    lock-derived(a, Σ)        ≝  some path in ancestors(a, Σ) was formed by [Lock]
    sync-exempt(a, m, a', Σ)  ≝  (lock-derived(a) ∧ type(a') = mutex<_> ∧ mode(a') = shared)
                                ∨ (lock-derived(a') ∧ type(a) = mutex<_> ∧ m = shared)

`lock` takes a shared reference to a mutex — which any number of
threads may hold — and yields a guard owning an exclusive root path
into the mutex's interior; the interior is reachable through no other
path (a mutex has no projections), and at most one lock path is live
at a time (`[Lock]` blocks otherwise), so accesses through it can
never race. `sync-exempt` is the one extension to `clash`
(`spec/04` §2): a lock path and the shared mutex references do not
conflict with each other — the mutex's serialization is what
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

## Change Log

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
