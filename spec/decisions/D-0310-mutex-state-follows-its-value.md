# D-0310 — A mutex's state belongs to its place, and goes with its value

Status: ACCEPTED (2026-11-06, under D-0281's delegation: the owner handed the cost model to the assistant "to
manage autonomously, with authorisation given to make all necessary decisions to push CobaltC in the direction it
needs to go"; a repair found by the fuzz campaign after Update 168); implemented by `CHG-0411`
Kind: Decision (`spec/02-schema.md`); a repair of `state.sync`'s key
Governed by: `CobaltC_Master_Instructions.md`
Depends on: D-0016, D-0019 (relocation), D-0204 (supervised threads; `[Lock-Poisoned]`), D-0215 (`[Guard-Drop]`
leaves a poisoned mutex poisoned), D-0253 (`wake-count`)
Affects: `spec/04` (`state.sync`, `sync-state`, `synchronized`), `spec/07` (`[Relocate-In]`, `[Relocate-Out]`),
`spec/18` (`[Fault-Contain]`'s `Σ_1`), `spec/19` (`[Mutex-New]`, `[Lock]`, `[Lock-Reentrant]`, `[Lock-Poisoned]`,
`[Guard-Drop]`), `spec/03` (`inv.concurrency-validity`); `spec/conformance.md` (one row); `coby`, `cbrt`, `cobc`, `cbc`

## 1. What was wrong

`state.sync` was keyed by an object's identity, and `[Lock]` read `sync(of(a_m))`, the state of the object the
reference to the mutex reaches. `[Mutex-New]` sets `sync(o) := unlocked` on the temporary it makes. That is right
while the mutex stays a whole object: a binding adopts the temporary (`[Store-Binding-Temp]`) or takes it over
(`[Store-Binding-Place-Transfer]`), and `o` goes on. It goes wrong once the mutex is a part of another object:

1. **A mutex stored into a field loses its state.** `[Store-Sub-Relocate]` copies the cells by `[Relocate-In]`,
   which re-keys obligations, authorities and holders under the destination, then ends `o_src`; `sync(o_src)` ends
   with it. `of(&c.m)` is `c`, whose `sync` was never set, so `sync-state(c, Σ) = thread-local` and `[Lock]`'s
   premise `= unlocked` does not hold: by the letter, a mutex in a field (std's `Channel` is one) cannot be locked.
   `[Relocate-Out]` (a mutex in a payload, matched out) is the same in reverse.
2. **Two mutexes in one object would share one state**, both keyed by the object.
3. **The implementations each filled the gap their own way, and disagreed.** `coby` keys by place (object and
   path), so a poisoned mutex moved into a field comes out unpoisoned: its entry stays with the ended source.
   `cbrt` keys by address, so a poisoned mutex moved to a new binding loses its poison, and a mutex made later at
   the same address inherits it. The campaign found the last: two blocks each making a mutex and lending it to a
   thread that faults holding it; in the `gcc` build the second block's mutex took the first one's stack slot,
   and the second thread's `lock` faulted `diag.mutex-poisoned` before running (seeds 19 and 23).

## 2. Decision

**The state of a mutex is keyed by its place, and goes with its value.**

- `state.sync : Place ⇀ SyncState`, a place being `(o, p)`: the sub-range at path `p` of object `o` (`p = ε`: the
  whole object). `place(a, Σ)` is `(of(a,Σ), p)` with `target(a,Σ) = sub-range(of(a,Σ), p, Σ)`.
- `[Mutex-New]` sets `sync((o, ε)) := unlocked`. `[Lock]`, `[Lock-Reentrant]`, `[Lock-Poisoned]` read and set
  `sync(m)` for `m = place(a_m, Σ1)`; `[Guard-Drop]` for the place whose `inner` its lock path targets;
  `[Fault-Contain]`'s `Σ_1` poisons every place `ℓ` holds. An atomic's wake count (§2a) stays at `(o, ε)`,
  `o = of(a)`, as its cell does (`atomic-cell(o)`); the waiting primitives are wait loops (D-0260), so no program
  can observe where a count is kept.
- `[Relocate-In]` re-keys `sync((o_src, q))` to `sync((o, path·q))`, and `[Relocate-Out]` re-keys
  `sync((o, path·q))` to `sync((o_new, q))`, as both re-key obligations. `[Object-End]` drops the entries of `o`.

Poison says the interior may be half-changed; the interior goes wherever the value goes, so the poison does too.
A mutex made by `Mutex::new` is always unlocked, wherever its storage lies.

Alternatives not taken: keeping the identity key and having relocation drop the state (the letter, filled in),
which unpoisons a half-changed interior by moving it and leaves a field's mutex unlockable; and a fault on moving
a poisoned mutex, a new diagnostic for no gain.

## 3. Implementation

- **`cbrt`** keeps a mutex's poison in the mutex's own state cells, which `[Repr-Mutex]` (`spec/06` §4) leaves to
  the implementation and `Mutex::new` zeroes; the bytes travel with every move and copy of the value. A contained
  fault marks the cells of each mutex its thread holds (each is alive: its guard borrows it); `lock` reads them,
  under the book's lock as before. The book's `poisoned` set is gone. The cells' place is the mutex's address plus
  its `inner`'s size rounded up to 8 (`struct { τ inner; uint64_t state; }`), from the type descriptor.
- **`cobc` and `cbc`**: `cb_lock_bare` takes the mutex's type id (`lower_bare_guard`); the channel natives pass 0,
  as no fault can happen while they hold the lock. `cb_lock`'s pathless form passes on the id it already takes
  (`cbrt.h`).
- **`coby`** keeps poison in the mutex's value (`Value::Struct`'s second element), so it goes with every move.

## 4. Conformance

`conf.mutex-poison-follows-the-value`: a poisoned mutex moved into a binding, then into a field, still poisons
each `lock`; a mutex made after the poisoned one has ended, in the next block, locks.
