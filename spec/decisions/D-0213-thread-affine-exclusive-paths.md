# D-0213 — An exclusive path belongs to one thread; references are lent at a thread boundary

Status: ACCEPTED (2026-10-11; the owner: "action the plan in full autonomously and using the recommendations as written" — `ChatGPT_review/ACTION-PLAN-2026-10-11b.md` WI-1, Q1)
Kind: Decision (`spec/02-schema.md`)
Governed by: `CobaltC_Master_Instructions.md` §6, §8 (1–2), §12, §17 (aliasing and concurrent access), §20
Depends on: D-0004, D-0018, D-0071, D-0111, D-0204, D-0209
Affects: `spec/08` 1.4.0, `spec/04` 1.6.0, `spec/19` 1.14.0, `spec/14` 1.24.0, `spec/21` 4.55.0, `spec/registry/diagnostics.md`
1.51.0, `spec/conformance.md` 3.208.0 (CHG-0251); `impl/src/interp.rs`, `impl/src/typecheck.rs`, `impl/cbrt/src/lib.rs`,
`impl/cbrt/include/cbrt.h`, `impl/cobc/src/lower.rs`, `impl/std/sync.cb`; seven cases in `impl/conformance/19-concurrency/`;
`stress/review2_oct11/` (local: the probes, the exploration logs)

## Problem

The second independent review (F-01) found that one exclusive reference can be used by two threads with no
second path for `clash` to see. A reference is freely copyable (`type.ref`) and every copy carries the same
access path; `[Spawn]` bound a reference argument as a value, so `spawn(w, r); spawn(w, r);` gave both threads
the path `r` holds, and each thread's access excluded that path as its own ancestor. The probes
(`stress/review2_oct11/`) showed it in both tools, and wider than the review said:

| Probe | Shape | Before (`cobc`, GCC) |
|---|---|---|
| `f01.cb` | `r` passed to two threads | 2000 of 2000, no diagnostic (the threads interleaved little) |
| `f01b.cb` | `r` passed to a thread, the spawner writes `*r` | no diagnostic |
| `f01c.cb` | `r` copied into two structs moved to two threads | 1694 of 2000: lost updates |
| `f01d.cb` | `r` sent through a `Channel<ref<i32, exclusive>>`, the sender writes on | 194812 of 200000 |
| `m02.cb` | two threads lent `&h` (shared), `h` holding `&mut x`, both writing `*h.inner` | 181890 of 200000 |
| `m03.cb` | the spawner keeps `q`, lends a struct holding `q` by `&mut` | 183989 of 200000 |

`[Conc-No-Race]` claims every such pair of accesses faults. The last two shapes do not pass a reference to
`spawn` at all: an exclusive reference stored in storage another thread reaches is the same hole.

## Options

1. **Lend at the boundary, and make exclusive paths thread-affine** (the plan's recommendation, extended):
   `spawn` and `Channel::send` replace each exclusive reference they hand over by a path borrowed from it, which
   the receiving thread takes over at its first access; every other exclusive path is accessed only by the
   thread that formed it. No syntax changes.
2. **A static restriction**: a reference reaches `spawn` or `Channel::send` only as a syntactic borrow. Simpler,
   but it forbids the `Channel<ref<…>>` and struct-of-references shapes D-0204 admits, and it does not touch the
   two shapes in which no reference is passed (`m02`, `m03`).

## Decision

Option 1.

1. **`[Thread-Affine]`** (`spec/08` `rule.alias.thread-affine`): an access through an exclusive path, or a
   `[Borrow]` from one, by thread `ℓ` requires that the path be `ℓ`'s — `thread-of(a) = ℓ` — or in transit, or
   of a thread that has ended (then `ℓ` takes it over). Otherwise `diag.aliasing-conflict`. Shared paths are not
   restricted. `AccessPathRecord.thread` (`spec/04`) already existed; it now says whose a path is, and may be
   `transit`.
2. **`[Thread-Lend]`** (`spec/19` §1): every exclusive path in a value — however deep in it, in a moved
   object's contents, among a `move` closure's captures — is replaced by a path borrowed from it (`[Borrow]`'s
   premises checked), marked `transit`, held where the old one was. `[Spawn]` lends its arguments and captures;
   `[Send]` lends the value it carries (`std`'s body calls a new std-only intrinsic, `lend_places`,
   `spec/21` `[Lend-Places]`). The sender's copies stay valid and clash with the lent path while it lives; a
   second lend of the same reference while the first lives is a second exclusive borrow and faults.
3. **Flow analysis** (`spec/14`): a reference binding passed whole to `spawn` is a borrow of its referent held
   by the handle until `join`, so the plain shapes — lending `r` twice, writing `*r` before the join — are
   rejected statically.
4. What an argument reaches only through storage it owns (a `Vec`'s elements) is not lent: an exclusive
   reference there stays the spawner's, and the thread's access through it faults while the spawner runs. The
   same holds for one in a `mutex` another thread locks, or in a struct a thread is lent a reference to (`m02`,
   `m03`): conservative, and deterministic — the access faults whatever the schedule. A program hands a thread
   exclusive access to something by passing the reference to `spawn` or sending it through a channel.

## Results

- Both tools implement it. `coby` numbers an exclusive token a spawned thread forms with that thread in its
  high bits and keeps a small map for lent and taken-over tokens (`Interp::affine`, `lend_value`); `cbrt` keeps
  the thread in each path record (atomic, so a read-only entry can take a path over), checks it in
  `check_access` only while another thread runs, and provides `cb_lend`/`cb_lend_fn` over the slots of a range;
  `cobc` lends a thread's argument block and closure before `cb_spawn`, and lowers `lend_places`. The D-0209
  direct dispatcher (`cb_direct_ok`) now falls back to the checked body for an exclusive path that is not this
  thread's: its unchecked body would have skipped the check.
- Every probe now faults: `f01`, `f01b` statically; `f01c` at the second `spawn`; `f01d` at the sender's write;
  `m02`, `m03` in the threads, contained. The 105 cases of `08-alias-validity` and `19-concurrency` passed under
  both tools unchanged; `thread_channel_exclusive_not_contained_rejected.cb` (a reference sent and used by the
  receiver alone) is unaffected.
- Seven cases (`conf.exclusive-ref-*`), each run under `cobc --explore 40` with one outcome. Two of the first
  drafts had two outcomes: a thread that has ended holds nothing (`spec/19` §4), and a reference binding ends at
  its last use (D-0111), so a later conflicting access may pass; the cases now keep the holder alive across the
  conflicting step.
- **Open, for the owner (not changed here).** `spec/08` says a shared reference "can never be a source of
  mutation" (Master Instructions §17). The rules let a function given `ref<S, shared>` write through an exclusive
  reference stored in `S` (`*s.inner = 1`): the place `s.inner` is read through the shared path, and the write
  goes through the value read (`stress/review2_oct11/m01.cb`, accepted by both tools). D-0213 makes the
  cross-thread form fault; in one thread it is sequential and unsound nowhere, but it contradicts the stated
  interface guarantee. Making the deref of a reference read through a shared path shared would close it, and
  changes which programs are accepted.
