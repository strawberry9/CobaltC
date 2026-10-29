# D-0235 — `[Swap-Places]` and `[Lend-Places]` write their places by `[Write]`

Status: ACCEPTED (2026-10-15; the owner delegated the fourth review's findings: "close off all remaining work
yourself, autonomously, making the best decisions for CobaltC")
Kind: Decision (`spec/02-schema.md`); a soundness repair (`spec/STABILITY.md` §2)
Governed by: `CobaltC_Master_Instructions.md` §8, §12, §17, §21
Depends on: D-0048 (`swap`), D-0213 (`[Thread-Affine]`, `[Lend-Places]`), D-0218, D-0222
Affects: `spec/21` §3b, `spec/08` §2–§3, `spec/OVERVIEW.md` §4; `impl/std/core.cb` (documentation); `coby`
(`swap_places`, `lend_places`); `cobc` (the same intrinsics); `spec/conformance.md` (CHG-0284)

## Problem

The fourth independent review (2026-10-15, F-02) found that `[Swap-Places]`, the std-only intrinsic beneath
`swap`, `replace`, `overwrite` and `Vec::swap`'s general body, wrote its two places in its conclusion with no
`¬clash`, no `[Thread-Affine]` and no reduction to `[Write]`, although `spec/08` §3 said every intrinsic
reduces to `[Read]`/`[Write]`. Run on 2026-10-15, both tools did as the rule said:

    auto r = &mut x;
    auto s = &*r;          // a shared reborrow of *r, live
    swap(r, &mut y);       // r passed on whole: no borrow formed
    printf("%v\n", *s);    // printed 2 under coby and cobc (GCC, Clang)

and an exclusive reference stored in a `Vec` moved to another thread was written through by `swap` while its
own thread ran. `replace(r, 9)` gave the same. D-0222's probe of "every element-writing body" had missed these
three functions because they do not go through `[Index-Vec]`. `[Lend-Places]` had the same shape, harmless
only because `Channel::send` always hands it `&mut v` of its own parameter.

## Options

(i) `[Swap-Places]` takes `[Write]`'s premises for each place (the same place twice excepted, as D-0022's
`swap(&mut a, &mut a)` requires), and so does `[Lend-Places]`; (ii) `swap`'s body borrows `&mut *a` and `&mut
*b` before the intrinsic, leaving the intrinsic as it was; (iii) restrict `swap` and `replace` to borrows
formed at the call. (ii) repairs the three bodies but leaves the intrinsic's rule an exception to `spec/08` §3,
which is the claim the safety argument rests on; (iii) rejects ordinary programs that pass a reference on.

## Decision

(i). `[Swap-Places]`, for distinct places, requires of each reference what `[Write]` requires of a write
through it — the object alive, the path temporally valid, `¬clash(a', exclusive)`, `affine(a', ℓ)` — with
faults `[Swap-Places-Stale]` and `[Swap-Places-Conflict]`, and keeps `held-by` as `[Write]` does. The same
place twice is no access. `[Lend-Places]` takes the same premises for its place. `spec/08` §3 names both as
reducing to `[Write]`, and `[Thread-Affine]`'s list of rules includes them. Both tools check each place as an
exclusive write: `coby` by `[Thread-Affine]` and its write-conflict test, `cobc` by `cb_write` on each token
inside the distinct-places branch (and before `cb_lend`).

## Rationale

A soundness repair by `spec/STABILITY.md` §2: the invariant is `inv.alias-validity` with D-0218's promise that
what a shared reference reaches does not change while it lives, and `[Conc-No-Race]`; the unsafe step is an
exchange through a reference that has a live conflicting path or belongs to a running thread; the programs
newly faulted are exactly those that make that step. The check is the one `*a = v` makes, so `swap(r, …)`
and `*r = …` now agree. A call whose arguments are borrows formed for it pays two checks that cannot fail.
`cobc`'s native `Vec::swap` for plain elements is unchanged: it already falls back to the checked body
whenever a path lives on either element, and otherwise makes the exclusive check of the vector the body's
element borrows make.

## Results

- `spec/21` §3b: `[Swap-Places]`, `[Swap-Places-Stale]`, `[Swap-Places-Conflict]`, `[Lend-Places]`; `spec/08`
  §2–§3; `spec/OVERVIEW.md` §4; `swap`'s documentation in `impl/std/core.cb`.
- Cases: `conf.swap-passed-ref-beside-shared-reborrow`, `conf.replace-passed-ref-beside-shared-reborrow`,
  `conf.swap-through-another-threads-ref` (one outcome over 40 schedules), `conf.swap-forms` (the forms that
  still exchange: locals, a place with itself, two fields, `replace` with no other path, `Vec::swap` of
  `String`s, `replace` of an `Option<String>`).
- Cost, user-space instructions against Update 108's `cobc` (one run each, GCC / Clang): every `bench/`
  workload unchanged (0.00%); a loop doing nothing but `swap(&mut a, &mut b)` and `replace(&mut a, i)` 20
  million times +7.4% / +7.5% (about 13,000 instructions an iteration before, the two calls' borrows and
  frames; the four added write checks about 980).
