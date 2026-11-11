# CHG-0411 — A mutex's state belongs to its place, and goes with its value

Status: implemented (2026-11-06), D-0310
Affects: `spec/04` 1.13.0 (`state.sync`, `place`, `sync-state`, `synchronized`); `spec/07` 1.4.0 (`[Relocate-In]`,
`[Relocate-Out]`); `spec/18` 1.10.0 (`[Fault-Contain]`); `spec/19` 1.21.0 (`[Mutex-New]`, `[Lock]`,
`[Lock-Reentrant]`, `[Lock-Poisoned]`, `[Guard-Drop]`); `spec/03` 1.6.5; `spec/conformance.md` 3.295.0; `coby`
(`mutex_poisoned`, `poison_mutex`, the mutex's encoding); `cbrt` (`claim_mutex`, `state_cells`, `poison_cells`,
`cb_lock_bare`, `cb_lock_pathless`); `cobc` and `cbc` (`lower_bare_guard`, the channel natives); `cbrt.h`

## What changes

1. **`state.sync` is keyed by place** (`(o, path)`), and `[Lock]` and its kin name the mutex's place
   `place(a_m, Σ)`. `[Relocate-In]` and `[Relocate-Out]` re-key a mutex's state with its value. Before, the key was
   the object `of(a_m)`, so a mutex in a field had no state of its own by the letter (`thread-local`, and `[Lock]`
   had no step), and two mutexes in one object would have shared one.
2. **`cbrt`** keeps a mutex's poison in the mutex's own state cells (`state_cells`: the address plus `inner`'s size
   rounded up to 8, from the type descriptor). A contained fault marks the cells of each mutex its thread holds
   (`poison_cells`); `claim_mutex` reads them under the book's lock; the book's `poisoned` set, keyed by address, is
   gone. Before, a poisoned mutex moved to another binding lost its poison, and a mutex made later at the same
   address (the next block's, in the same stack slot) inherited it: its first `lock` faulted
   `diag.mutex-poisoned`.
3. **`cb_lock_bare` takes the mutex's type id** (`cobc`'s and `cbc`'s `lower_bare_guard`); the channel natives pass
   0, as nothing can fault while they hold the lock. `cbrt.h`'s pathless `cb_lock` passes on the id it takes to
   `cb_lock_pathless`.
4. **`coby`** keeps the poison in the mutex's value, beside `inner` (`mutex_poisoned`, `poison_mutex`), and in the
   state cells of its encoding, so a reclaimed object's mutex carries it too. Before, it was a set keyed by place,
   which a poisoned mutex moved into a field left behind.

## Found by

The fuzz campaign after Update 168 (seeds 19 and 23, `stress/verify/fuzz/findings/s19_p139`, `s23_p172`): the gcc
build faulted `diag.mutex-poisoned` where `coby` and the ASan build ran the second thread.

## Conformance

- `conf.mutex-poison-follows-the-value`: a poisoned mutex moved into a binding, then into a field, poisons each
  later `lock`; one poisoned through a `guard<i64>`, then a mutex made in the next block, which locks. Before, each
  of `coby` and `cobc` failed a different part of it (`coby` the field, `cobc` the binding and, under clang, the
  next block's mutex).
