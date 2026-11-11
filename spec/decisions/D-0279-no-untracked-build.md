# D-0279 — No untracked build: every binary the tools write is a conforming execution

Status: ACCEPTED (2026-10-25, the owner's decision: "on D-0279 I expect option A is the one to choose?", agreed:
option A, no untracked form). Drafted PROPOSED 2026-10-24 at the owner's request during P4 ("Draft option 2 as a
D-record for me to decide, and in the meantime keep doing option 1"); figures as of Update 134.
Kind: Decision (`spec/02-schema.md`)
Governed by: `CobaltC_Master_Instructions.md` §9, §12 (the guarantee)
Depends on: D-0276 (`cbc` replaces `cobc`), D-0274 (the bootstrap form), D-0205/CHG-0237 (frozen
bodies), D-0188 (dual bodies), D-0199/D-0200 (compile-time discharge)
Affects (option C, recommended): `impl/cobc/src/lower/` and `impl/selfhost/back/` (a lowering form),
`impl/selfhost/build.sh` (the release build), `impl/selfhost/compare_c.sh` and `fixpoint.sh` (its checks),
`impl/STATUS.md`, `README.md`; not `spec/`'s rules

## Problem

P4's measurements (`stress/p4/RESULTS.txt`, after Update 134, 2026-10-24) put `cbc` at 15–43 times `cobc`'s
time (32–56 times its instructions) on ordinary inputs, against D-0276's working bound of 10. P4's first
round (Update 134) cut `cbc`'s instructions by 19–61%; it also made `cobc` faster, so on the larger inputs the time
ratios did not fall (dns_over_http went from 36 to 43 times):

| input | `cobc` | `cbc`, Update 133 | `cbc`, Update 134 | ratio now (time, instructions) |
|---|---|---|---|---|
| `showcase/tier3/tinyos.cb` | 0.6 s | 22.9 s | 9.0 s | 15×, 32× |
| `showcase/tier12/dns_over_http.cb` | 1.8 s | 100 s | 76.6 s | 43×, 56× |
| `impl/selfhost/cobfmt/main.cb` | 6.1 s | 293 s | 243.8 s | 40×, 49× |

On tinyos, 86.0% of `cbc`'s samples are in the runtime (`cbrt`) and 5.5% in the C library; its own
generated code is 8.2%. At Update 133, that 700-line input made about 330 million runtime calls (statement
scopes 29 M each way, `move_to` 22 M, `read` 20 M, new objects 15 M, frames 14.5 M each way, …), at about 250
instructions each, all told; P4's first round removed many of them but not the pattern. The runtime is
already tuned per event (arenas, pools, short lists; the first P4 changes found 6%); the cost is the number of
events. That number falls only as the compiler proves more accesses (static elision, P4's option 1), and each
such proof is written twice (`cobc`, `cbc`) and must leave every fault where it was. A further two to three
times from it would still leave `cbc` at 15–20 times `cobc` on the larger inputs.

The upper bound of removing tracking altogether: what remains is the generated code and the C library, about
14% of today's samples on tinyos, so about 7 times faster than now. That is about 2 times `cobc` on tinyos and,
if the larger inputs' shares are alike, about 6–7 times `cobc` on them (the rest is `cbc`'s own algorithms and
the C it writes, P4's third strand).

## What "untracked" would mean

A lowering form in which the compiler writes no call into `cbrt`'s object and path bookkeeping: no statement
scopes or frames as runtime entries, no objects for values, no paths for borrows, no `read`/`write`/`borrow`
checks of `[Read-Conflict]` and `[Write-Conflict]` or of temporal validity (`diag.aliasing-conflict`, `diag.stale-binding`,
`diag.move-while-aliased`, …). Destruction is placed statically (the compiler already does this for objectless
locals, D-0199/D-0200). What stays, because it costs little and is not bookkeeping: index bounds,
arithmetic overflow, division by zero, `Option::expect`/`assert`, UTF-8 validation, every static check
(the program is still checked in full before any C is written), and the runtime's non-tracking services
(allocation, I/O, threads, formatting).

What is lost, for a binary built so: CobaltC's guarantee (`term.safe-client`, `spec/00`) holds only on a
conforming implementation, and the dynamic discharge of `spec/03`'s alias and temporal invariants is part of
conformance. An untracked binary is therefore **not a conforming execution**. A program that would have
faulted with an aliasing or staleness diagnostic instead does whatever the C does: reads stale memory, writes
through a dangling pointer. For `cbc` that means a compiler bug that today stops with a located fault could
instead produce wrong C or crash.

Containment (D-0204) also rests on tracking (which exclusive paths a thread was lent): an untracked
program would treat every fault as fatal.

## Options

A. **No untracked form.** Speed comes from static elision only (P4 option 1), plus `cbc`'s algorithms.
   - For: one form, every binary conforming; the guarantee is never conditional on how a program was built.
   - Against: `cbc` stays well above the 10x bound for the foreseeable future (estimate: 15–20x on the larger
     inputs after a long series of elisions; tinyos is at 15x already); the slow compiler is the one the project builds with.

B. **A public option** (`cobc --untracked`, `cbc --untracked`) for any program, documented as producing a
   non-conforming binary, like C compilers' options that drop checks.
   - For: every user can trade the guarantee for speed where they judge it safe (a program already tested
     under the checked build).
   - Against: makes the guarantee optional in the language's own tools; a second semantics users will rely
     on; every future feature must be lowered in both forms, in both compilers; the strongest argument for
     the language (defined failure) gets an "unless" in the README.

C. **The toolchain's own release build only** (recommended). The untracked form exists as an internal
   lowering form (an environment switch like D-0274's bootstrap form, not a documented option), used by
   `impl/selfhost/build.sh` to build the `cbc` (and `cobfmt-cb`, `cbparse`) that people run. The checked
   build stays the one every suite, battery and fixed point runs, and gates each release:
   - the untracked `cbc` must write the same C, byte for byte, as the checked `cbc` on the whole corpus
     (`compare_c.sh` against it) and reach the fixed point;
   - a fault the checked `cbc` reports on any input is a bug to fix before release, as now;
   - `cbc --version`/`-v` says which form it is; `CBC_CHECKED=1` (or `build.sh CHECKED=1`) builds the checked
     one, and a user who hits a crash is told to retry with it.
   - For: the speed where it matters (the project's own compiler) without offering the trade to programs;
     the checked build remains the reference and the evidence; the scope can widen to B later if wanted.
   - Against: the compiler people run is not a conforming CobaltC execution of its own source, so a
     latent aliasing bug in `cbc` would show as wrong output rather than a fault; the corpus comparison
     and the fixed point bound that risk but do not remove it. The lowering form is real work in both
     compilers (estimate: weeks), and every later lowering change has a third form to keep.

D. **As C, but in `cbc` only.** `cobc` keeps two forms; only `cbc` learns the untracked one (it is the
   compiler that needs the speed).
   - For: half the implementation work.
   - Against: `compare_c.sh` cannot compare the untracked C with anything but itself across versions;
     the form has no second implementation, against the project's practice since D-0276.

## Decision

Option A. No compiler writes a binary without the tracking that `spec/03`'s alias and temporal invariants
require: not as a public option (B, ruled out: the guarantee is never conditional on how a program was built),
not as the toolchain's own release build (C), not in `cbc` alone (D). Reasons, as of 2026-10-25:

- D-0281 attacks the cost at its source: references held by frames carry no run-time state by rule, which
  covers most of what `cbc` pays for; the untracked form was a way around tracking the compiler could not
  remove, and D-0281 removes it instead.
- D-0278 freezes the CobaltC toolchain between D-0281's phase boundaries; a third lowering form in both
  compilers, carried through every D-0281 revision, would be work done against rules about to change.
- A release compiler built with the guarantee off says the language's checks are too costly to live with,
  the criticism D-0281 exists to answer; and a latent aliasing bug in an untracked `cbc` would surface as
  wrong C instead of a located fault.

Option C may be proposed again only with measurements, if `cbc` remains well above D-0276's working bound
(ten times `cobc`) after D-0281's phase 3. Option B is closed.

## Recommendation (as drafted, 2026-10-24, superseded by the decision)

C, after a measurement: before the lowering form is written, a prototype (a `cbrt` whose bookkeeping entries
return at once, linked into today's `cbc`, run on the corpus with checked-build output compared) confirms the
about-7x bound and that nothing but bookkeeping is skipped. If the prototype shows less than about 4x, A is the
better course and this draft is withdrawn.

P4's option 1 (static elision) continues either way: it speeds up every compiled program, not only `cbc`.

## Questions for the owner

1. A, B, C or D?
2. For C or D: the prototype first (recommended), or go straight to the lowering form?
3. For C or D: is "the release `cbc` is built untracked, the checked `cbc` is the reference" acceptable
   wording for the README and the guide's introduction, or should the untracked build stay out of the
   published tree (built only locally)?
