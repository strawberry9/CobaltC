# D-0237 — A thread that takes over an exclusive path is not contained

Status: ACCEPTED (2026-10-15; delegated by the owner, as D-0235)
Kind: Decision (`spec/02-schema.md`); completes D-0204's containment promise
Governed by: `CobaltC_Master_Instructions.md` §8, §20, §21
Depends on: D-0204 (supervised tasks), D-0213 (`[Thread-Affine]`)
Affects: `spec/18` §1 (`contained(ℓ, Σ)`, prose), `spec/OVERVIEW.md` §7; `coby` and `cbrt` (`affine`);
`spec/conformance.md` (CHG-0286)

## Problem

D-0204 contains a spawned thread's fault — the program goes on — unless the thread was lent an exclusive
reference or locked a mutex that may hold one, because "what the failed thread was changing was its own, or
is poisoned". The fourth review (F-07) found a third way to hold another thread's data: `[Thread-Affine]`
lets a thread take over an exclusive path whose thread has ended, or one still in transit. Probed 2026-10-15:
a thread lent `&mut x` returns a `Vec` holding the lent path; `main` moves the vector to a second thread,
lent nothing, which writes `x` and then faults. Both tools contained the fault: `try_join` handed it over and
`main` printed `x` as the failed thread had left it.

## Options

(i) a thread that takes over an exclusive path by `[Thread-Affine]` is from then on neither contained nor
cancelled, as a lent thread is; (ii) take-over only from `transit`, so an ended thread's path is unusable
until `[Join]` re-lends it; (iii) leave it, documenting that containment can leave another thread's data
half-changed.

## Decision

(i). `contained(ℓ, Σ)` gains "ℓ has taken over no exclusive path by `[Thread-Affine]`". Both runtimes add
the thread to the set they keep for lent threads at the take-over (`coby`'s `lent_excl`, `cbrt`'s `lent`),
which both containment and cancellation already read.

## Rationale

(i) changes no program that does not take a path over in a spawned thread, keeps `[Thread-Affine]` as it is,
and states D-0204's own reason in the rule. (ii) would change `conf.exclusive-ref-lent-then-joined` and need a
re-lend in `[Join]`. (iii) contradicts the promise D-0204 makes. A thread receiving an exclusive reference on a
channel was already excluded (it locks the channel's mutex, whose interior may hold one), so in practice the
new clause bites only where the reference arrived inside storage the thread was given.

## Results

- `spec/18` §1; `spec/OVERVIEW.md` §7; `coby` (`threads.rs` `affine`), `cbrt` (`affine_slow`).
- Case: `conf.thread-taking-over-path-not-contained` (one outcome over 40 schedules).
