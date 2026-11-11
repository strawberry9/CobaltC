# CobaltC Audit Closure Log

Status: living tracking artifact (Master Instructions §24)
Governed by: `spec/AUDIT.md` (first audit), `spec/AUDIT-2.md` (second audit)

## Purpose

One line per finding: id, status, and the artifact where the
acceptance criterion is met. `CLOSED` = demonstrably true in the cited
artifact. `SUPERSEDED` = the finding's subject was replaced.

## First audit (`spec/AUDIT.md`), as re-verified by `spec/AUDIT-2.md`

The earlier closure of A-01–A-07, A-09, A-11, A-22, A-28, A-30 was
found overstated (`spec/AUDIT-2.md` §4). All 31 findings are now
closed by the 1.0.0 rewrite:

| id | status | artifact |
|---|---|---|
| A-01, A-02, A-03 | CLOSED via D-0018 | `spec/05` `[Object-End]`, `spec/14` `[Stmt-Exit]`/`[Block-Exit]`, `spec/08` §3; `conf.destroy-after-borrow-scope-ends-ok`, `conf.sequential-exclusive-borrows-ok`, `conf.projection-write-while-borrowed-rejected` |
| A-04, A-05, A-24 | CLOSED via D-0016 + D-0019 | `spec/16` `[*-Construct]` (temporaries), `spec/15` `[Return]`/`[Call]`, `spec/19` `[Spawn]`; `conf.returned-resource-destroyed-at-caller-exit`, `conf.leak-free-by-construction` |
| A-06, A-27 | CLOSED via D-0019 | `spec/11` `[Let]`, `[Let-Uninit]`; `spec/05` `store` |
| A-07 | CLOSED | `spec/14` §6 `rule.control.flow-analysis` (facts, lattice, transfer functions, discharge table, outcome policy) |
| A-08 | CLOSED | `term.safe-program` |
| A-09 | CLOSED | every rule in `spec/05`–`spec/21` is stated over `Σ`; prose remains only as explanation |
| A-10 | CLOSED | `outcome:` on `[Object-Establish-*]`, `[Sizeof-Addr]`, `[Repr-*]`, `[Layout-*]`, `[Thread-Step]`, `[Allocate]` |
| A-11 | CLOSED | `spec/06` §7 `rule.arith.represent`; `spec/16` §1 `rule.agg.layout` |
| A-12 | CLOSED | D-0017 |
| A-13 | CLOSED | `spec/06` §3, `spec/13` §2 |
| A-14 | CLOSED | every rule/type/term/invariant/state/diag/ex/conf/feat carries Status; promotion pass complete (all `ACCEPTED`) |
| A-15 | CLOSED | `spec/02` §4 grep prints only ids (verified 2026-09-20) |
| A-16 | CLOSED | `spec/registry/diagnostics.md` 1.0.0 |
| A-17–A-21, A-23, A-25, A-26, A-29 | CLOSED | `spec/examples.md` 2.0.0, `spec/conformance.md` 2.0.0, `spec/21` §0–§1 |
| A-22 | CLOSED | `spec/15` §6 (`callable`, capture-mode scan, self-borrow call) |
| A-28 | CLOSED | `conf.generic-struct-monomorphize-resource` derived |
| A-30 | CLOSED | suites regenerated with a derivation per case |
| A-31 | CLOSED | `spec/22` §5 |
| A-32, A-33 | CLOSED via D-0018 | `conf.reference-in-field-survives-statement`, `conf.disjoint-field-borrows-ok` |

## Second audit (`spec/AUDIT-2.md`)

| id | status | artifact |
|---|---|---|
| B-01 | CLOSED | D-0018; `spec/04` `clash`; `spec/05` `[Read-Conflict]`/`[Write-Conflict]`; `conf.projection-write-while-borrowed-rejected` |
| B-02 | CLOSED | D-0018; `spec/08` `[Borrow]` (no suspension); `conf.two-shared-borrows-ok`, `conf.owner-read-while-shared-ok` |
| B-03 | CLOSED | D-0018; `spec/14` `[Stmt-Exit]` (`Unheld`), `spec/05` `[Object-End]` (`Dead`); `conf.sequential-exclusive-borrows-ok` |
| B-04 | CLOSED | `spec/04` record shape; every forming rule sets `frame`, `temp-scope`, `base`, `held-by` (`spec/05`, `08`, `16`, `19`, `20`, `07`) |
| B-05 | CLOSED | D-0019; `spec/05` `[Read]`/`[Write]` over `target(a)`, `type(a)`; `[Write-Partial-Init]`; `conf.partial-init-rejected` |
| B-06 | CLOSED | D-0019; `spec/15` §2/§4, `spec/14` §1a/§2; `conf.returned-resource-destroyed-at-caller-exit`, `conf.early-return-destroys-locals`, `conf.unstored-temporary-destroyed-at-stmt-end` |
| B-07 | CLOSED | D-0019; `spec/05` `rule.value-object.store`; `spec/13` §1; `conf.let-copy`, `conf.leak-free-by-construction` |
| B-08 | CLOSED | D-0018; `spec/15` §6; `conf.closure-borrow-capture`, `conf.reference-in-field-survives-statement` |
| B-09 | CLOSED | `spec/14` §4 `[While-*]`; `conf.while-body-frame-per-iteration` |
| B-10 | CLOSED | `spec/14` §6; outcomes marked static/dynamic throughout `spec/conformance.md` |
| B-11 | CLOSED | `spec/05` `[Object-Establish-*]` allocation; `spec/06` §7 |
| B-12 | CLOSED | `spec/04` `Performer ≝ Thread` |
| B-13 | CLOSED | `spec/04` per-thread stacks; `spec/19` |
| B-14 | CLOSED | `conf.double-destroy-rejected`, `conf.transfer-invalidates-source`; registry entry states the ordering |
| B-15 | CLOSED | `spec/08` `[Borrow-Exceeds-Source]`; `conf.exclusive-from-shared-rejected` |
| B-16 | CLOSED | `spec/09` `[Ref-Identity-Eq]` |
| B-17 | CLOSED | `spec/07` `[Relocate-*]`/`[Destroy-Composite]`, `spec/15` §3/§6, `spec/16` `[Match]` |
| B-18 | CLOSED | `spec/15` §7 `rule.fn.program`; `spec/18` §1 |
| B-19 | CLOSED | Status on every entity; §4 grep clean; promotion to `ACCEPTED` |
| B-20 | CLOSED | `spec/22` 1.0.0; `spec/21` §0 prelude |
| B-21, B-23, B-25 | CLOSED | stale text removed by the rewrites; D-0015 `REMOVED` |
| B-22 | CLOSED | `spec/06` `[Shr-Checked]`, `spec/14` `[If-No-Else]` |
| B-24 | CLOSED | `spec/05` `[Object-Establish-*]` carry `outcome`, no `disposition` |
| B-26 | CLOSED | owner reviewed, declined to expand candidate sets (`CHG-0024`); D-0006–D-0009 each gain a `Candidate-set review` section |

## Found and fixed during closure (not in either audit)

- A shadowed binding's object was never swept because ownership was
  keyed on the name map; `AccessPathRecord.frame` restored and
  `owned-by-frame` keyed on the holder path's frame (`spec/04`).
- A binding's own root path would have been invalidated by the `let`
  statement's exit; owner paths are exempt from `[Stmt-Exit]`'s
  `Unheld` (`spec/14` §1a).
- Two exclusive borrows of disjoint fields would have been statically
  rejected because the analysis tracked only the root binding;
  `deriv` now carries the projection path (`spec/14` §6).
- A plain temporary in an operator position had no read rule;
  `[Temp-To-RValue]` added (`spec/13` §1a).
- Three conformance fragments contradicted the rules on derivation and
  were corrected as fragments (`spec/conformance.md` change log); no
  rule was weakened to fit a case.

## Third pass: consistency check (2026-09-20, after `CHG-0008`)

Every artifact read in full and checked against every other; findings
and their fixes are recorded in `spec/changes/CHG-0009-consistency-
audit-fixes.md`. Normative findings, all CLOSED by `CHG-0009`:

| id | finding | artifact |
|---|---|---|
| C-01 | `[T-Deref]`/`[T-Assign]` did not type `*p`, `p + n`, `*g`; `[T-Spawn]` rejected `move` closures | `spec/12` 1.4.0 |
| C-02 | no evaluation context for a raw-pointer write's value | `spec/13` 1.4.0 |
| C-03 | `[Join]` destroyed the result it returned (via `[Handle-Destructor]`) | `spec/19` 1.3.0, `spec/04` 1.1.0 |
| C-04 | `Rc::drop` faulted `diag.destroy-while-aliased` on every last drop | `spec/21` 2.6.0 |
| C-05 | `[Reclaim]`'s trusted condition excluded its own re-attachment premise | `spec/20` 1.3.0 |
| C-06 | `conf.reclaim-two-paths-clash` reclaimed a live local's cells | `spec/conformance.md` 3.4.0 |
| C-07 | disambiguation (4) made `i32 x = 5;` an expression statement | `spec/22` 2.8.0 |

Non-normative slips fixed in the same pass are itemised in `CHG-0009`
§"Hygiene" and in each artifact's Change Log. Not changed, by
`spec/02-schema.md` §6: decision records (immutable; their fragments
stay in pre-2.0.0 syntax, as `CHG-0001` states) and the two audit
files above.

## Fourth pass: per-case derivation of `spec/conformance.md` (2026-09-20)

All 112 rows derived step by step against the rules as of `CHG-0009`.
No expected outcome was wrong; fourteen derivations rested on rules
that were missing, inapplicable, or contradicted elsewhere. Recorded
and closed in `spec/changes/CHG-0010-conformance-derivation-fixes.md`:

| id | finding | artifact |
|---|---|---|
| D-01 | no expected type for shift amounts/indices; none through `if`/`match`/blocks | `spec/12` 1.5.0, `spec/06` 1.2.0 |
| D-02 | `{ return …; }` typed `unit`; no typing for `Name{f} = e` | `spec/12` 1.5.0 |
| D-03 | `narrow` undefined across signedness | `spec/06` 1.2.0 |
| D-04 | `sizeof(mutex)` contradicted its layout; no `represent` for handle/guard/mutex | `spec/06` 1.2.0 |
| D-05 | FA did not mark `match (x)`, `Name{f} = x`, `*p = x` as consuming (soundness); no `¬live-resource-at` row | `spec/14` 1.3.0 |
| D-06 | `e?` had no rule on `Err` | `spec/18` 1.2.0 |
| D-07 | `*g` read a guard as a value (`[Read-Resource-Rejected]`) | `spec/19` 1.4.0, `spec/13` 1.5.0 |
| D-08 | built-in destructors unreachable from `destructor(τ)` | `spec/07` 1.2.0 |
| D-09 | raw-access trusted conditions false for every local | `spec/20` 1.4.0 |

The derivation notes for rows that needed no change are not kept as
an artifact (Master Instructions §24); the rewritten derivations in
`spec/conformance.md` 3.5.0 are the record.

## Fifth pass: end-to-end program derivations (2026-09-20)

Four whole programs (`spec/examples.md` `ex.e2e-*`) derived from
`[Program]` to `[Terminate-Ok]`; derivations in `spec/conformance.md`
§13. Seven rule gaps found, all closed by
`spec/changes/CHG-0011-end-to-end-derivation-fixes.md`:

| id | finding | artifact |
|---|---|---|
| E-01 | `current-scope` undefined for a freshly spawned thread | `spec/04` 1.2.0 |
| E-02 | `Vec::pop` of a resource had no rule after `grow`; `[Release]`'s condition false for `grow` | `spec/20` 1.5.0 |
| E-03 | reclaimed resources violated `inv.resource-authority`'s owner clause | `spec/03` 1.1.0 |
| E-04 | `[Match]` never said which rule pushes the arm frame | `spec/16` 1.3.0 |
| E-05 | `widen<i32>(b : u8)` had no rule | `spec/06` 1.3.0 |
| E-06 | expected type did not pass through unary operators | `spec/12` 1.6.0 |
| E-07 | programs and derivations recorded | `examples.md` 3.5.0, `conformance.md` 3.6.0 |

## Sixth pass: revisit-condition programs (2026-09-20)

`CHG-0011`'s "Revisit conditions" named four programs not yet derived.
All four now done, recorded in
`spec/changes/CHG-0012-vec-realloc-stale-ref-e2e.md` and
`spec/changes/CHG-0013-remaining-revisit-condition-programs.md`.

| id | finding | artifact |
|---|---|---|
| F-01 | (no gap) `ex.e2e-vec-realloc-stale-ref`/`conf.e2e-vec-realloc-stale-ref` derived: an element reference reached through a reference parameter and held across a reallocating `push` is invisible to `rule.control.flow-analysis` and `rule.temporal.ref-escape` alike (both track only a plain binding's own projections), so the program is accepted statically and `[Release]` → `[Object-End]` → `[Read-Stale]` catches it dynamically, exactly as `spec/21` §1 already states. First pass since `spec/AUDIT.md` to find no rule gap. | `spec/examples.md` 3.6.0, `spec/conformance.md` 3.7.0 |
| F-02 | (no gap) `ex.e2e-mutex-vec-resource-interior`/`conf.e2e-mutex-vec-resource-interior` derived: a resource-typed mutex interior is tracked as a `destroy-composite` obligation (unlike `ex.e2e-threads-mutex`'s plain `i32` interior, whose obligation set is empty); block exit reaches `Vec::drop` through the same "temporary exclusive root into the sub-range" mechanism `conf.e2e-rc-resource-payload` already uses. | `spec/examples.md` 3.7.0, `spec/conformance.md` 3.8.0 |
| F-03 | (no gap) `ex.e2e-closure-move-rc`/`conf.e2e-closure-move-rc` derived: a `move` closure's captured field is destroyed by the same `destroy-composite` mechanism as any struct field; calling the closure twice consumes nothing (only transient shared sub-borrows), and `drop`ping it reaches `Rc::drop` exactly as `conf.e2e-rc-resource-payload`'s last drop. | `spec/examples.md` 3.7.0, `spec/conformance.md` 3.8.0 |
| F-04 | (no gap) `ex.e2e-thread-fault-abandons-guard`/`conf.e2e-thread-fault-abandons-guard` derived: `[Fault-Unwind]` unwinds only the faulting thread; a guard/mutex/handle held by another live thread is abandoned exactly as `spec/18` §1's prose states, and the program's observable outcome (termination diagnostic, `extern`-call sequence) stays fully deterministic despite the interleaving nondeterminism, because the abandoned steps never touch either. | `spec/examples.md` 3.7.0, `spec/conformance.md` 3.8.0 |

Four consecutive derivations with no rule gap — the first "clean"
stretch since `spec/AUDIT.md`. Read as evidence that `CHG-0009`–
`CHG-0011` closed the load-bearing gaps in the reclaim/destroy-
composite/fault machinery, not as evidence that no further gaps exist
elsewhere in the corpus not yet derived this way.

## Seventh pass: `spec/21` library, statement by statement (2026-09-20)

Every `unsafe` block in `Vec`, `Rc` (`String` has none)
written down with its trusted condition and why it holds. Twelve
blocks total:

| Site | Rule(s) | Trusted condition (one sentence) | Holds because |
|---|---|---|---|
| `Vec::push`'s write | `[Rawptr-Write]`/`[Rawptr-Move-In]` | The target cells are in bounds and not reached by any live path at all (`CHG-0019` tightened `[Rawptr-Write]` to match `[Rawptr-Move-In]`'s condition; see F-05 below for why). | Index `len` was never previously established as an element (elements are only ever created at indices `< len` by an earlier push), and `Vec::pop` now `release`s any element it popped, so a reused slot is never still claimed. |
| `Vec::pop`'s read | `[Rawptr-Read]`/`[Rawptr-Move-Out]` | The cells hold a valid `τ` not reached by another object's live path in exclusive mode (Read), or claimed by no other live object and not tracked elsewhere (Move-Out). | Index `len` (post-decrement) was the last element `push` wrote; nothing else writes it between push and this pop. |
| `Vec::index_shared`/`index_exclusive`'s `reclaim` | `[Reclaim]` | The cells hold a valid `τ` claimed by no live fresh-storage object (a reclaimed object there is re-attached, not re-established). | `i < v.len` is checked dynamically just above; the cells are inside `v`'s own raw allocation, never a fresh-storage object's extent. |
| `Vec::grow`'s `copy_raw` | `[Copy-Raw]` | Both ranges are in bounds; no live object overlaps the destination. | The destination is the address range `allocate` just returned — freshly allocated, disjoint from everything live. |
| `Vec::grow`'s `deallocate` | `[Deallocate]` | `(p, n, align)` were returned by one `allocate`, not yet deallocated. | `(v.ptr, v.cap·sizeof(T), alignof(T))` is exactly what the previous `grow` (or `Vec::new`'s absence of one) allocated; a `Vec` never deallocates its buffer except here and in `drop`, never twice. |
| `Vec::drop`'s loop `reclaim` | `[Reclaim]` | As `index_shared`'s. | Same: `mut_i < self.len`. |
| `Vec::drop`'s `deallocate` | `[Deallocate]` | As `grow`'s. | This is the one and only deallocation of a `Vec` that reaches `drop` (single-use, `[Destroy]`'s authority is consumed). |
| `Rc::new`'s write | `[Rawptr-Write]`/`[Rawptr-Move-In]` | As `Vec::push`'s write. | Freshly allocated cells; nothing has reclaimed them yet. |
| `Rc::clone`'s `reclaim` + write | `[Reclaim]` then ordinary `[Write]` (not raw) | `reclaim`'s condition as above; the subsequent `b.count = …` is a checked field write through `b`, not a second raw access. | The box was established (fresh, resource `T`) or is being reclaimed for the first time (plain `T`) by this very call; `[Write]`'s own `¬clash` is *checked*, not trusted, once past `reclaim`. |
| `Rc::get`'s `reclaim` | `[Reclaim]` | As `clone`'s. | Same. |
| `Rc::drop`'s first block (`reclaim` + count) | `[Reclaim]` + ordinary `[Write]` | As `clone`'s. | Same. |
| `Rc::drop`'s second block (`drop` + `deallocate`) | `[Reclaim]` (inside `drop`'s argument) + `[Destroy]`/`[Destroy-Plain]` + `[Deallocate]` | `reclaim` re-attaches the same box (still alive); `deallocate` matches `Rc::new`'s one allocation, not yet freed. | `last` is true exactly once per box (`count` monotonically decreases, checked `== 0`); the counting block's borrow `b` already ended, so `solitary` holds for `[Destroy]`/`[Destroy-Plain]`. |

### F-05 · A live `Vec::index_shared` reference is silently overwritten by an unrelated `pop`+`push` of the same slot — **CLOSED by `CHG-0019`**

**Resolution (2026-09-20, human owner decided):** direction 2 below,
selected. `Vec::pop` now `release`s the popped slot regardless of
element type (`spec/21` 2.7.0); `[Rawptr-Read]`/`[Rawptr-Write]`'s
trusted conditions no longer carry the `reclaimed-at(addrs)` exemption
that made the hazard below "trusted" (`spec/20` 1.6.0). Re-derived as
`conf.e2e-vec-pop-push-reuse-stale-ref`: the reproduction below now
faults `diag.stale-binding` (dynamic) at `*r`, exactly like the
reallocation case `CHG-0012` already closed, instead of silently
reading `99`. The reproduction, "why nothing caught it," and the three
candidate directions are kept below as the record of the analysis that
led to the decision.

**Reproduction (well-formed, statically accepted):**

    fn hazard(ref<Vec<i32>, exclusive> v) : i32
    {
        auto r = Vec::index_shared(&*v, 2);   // r: shared ref into element 2, value 30
        Vec::pop(v);                          // len 3→2; [Rawptr-Read] returns 30, o_2 left alive (plain T)
        Vec::push(v, 99);                     // len 2→3; [Rawptr-Write] at the same address as o_2
        *r                                     // reads 99, not 30 — no diagnostic, static or dynamic
    }
    fn main()
    {
        Vec<i32> v = Vec::new();
        Vec::push(&mut v, 10);
        Vec::push(&mut v, 20);
        Vec::push(&mut v, 30);
        hazard(&mut v);
    }

**Why nothing catches it.** As in `conf.e2e-vec-realloc-stale-ref`,
reaching the Vec through a reference parameter puts `r` outside
`rule.control.flow-analysis`'s `deriv`/`escaped` tracking, so nothing
is rejected statically (the same-function shape *would* be rejected,
by the same mechanism `conf.vec-ref-then-push-rejected` uses). Unlike
that case, the dynamic path does not save it either: `Vec::pop`'s
`[Rawptr-Read]` (taken because `i32` is plain) does not end the
reclaimed object `o_2` that `index_shared` established — reading
doesn't own anything, correctly, for ordinary values. `o_2` survives,
still holding `r`. `Vec::push`'s `[Rawptr-Write]` then lands on the
same cells; its trusted condition — "no live derived path of an
object *other than* `reclaimed-at(addrs)` reaches them" — explicitly
excludes `o_2`'s own paths from consideration, so the write is
"trusted" regardless of `r`. Nothing in `write`'s conclusion ends or
invalidates `o_2` or `r`. `*r` then reads the new value through
`[Read]`, whose premises (`temporally-valid`, `alive`, `¬clash`) are
all satisfied — `r` is not stale, and `clash` never sees the raw write
at all (raw pointers have no access-path entry). The result: a live
`ref<i32, shared>` observes a value change with no relationship to
anything `r`'s own aliasing rules track — a violation of
`inv.alias-validity`'s guarantee in spirit, via a hole in the raw
boundary rather than in the checked one D-0018 governs.

**Why `[Rawptr-Move-In]`/`[Rawptr-Move-Out]` don't have the same hole.**
For a *resource* element type, `Vec::pop` uses `[Rawptr-Move-Out]`,
which `end-object`s the source unconditionally — so a live reference
into a popped resource element is already caught as
`diag.stale-binding` at its next use, dynamically, exactly like
`CHG-0012`'s finding. `[Rawptr-Move-In]`'s condition ("not part of any
live object *at all*", no exemption) would then correctly refuse to
call itself trusted if something tried to reuse that address while a
reference were still live — but nothing can be, because the prior
Move-Out already ended it. The hole is specific to the *plain*-element
path, where a "read" correctly does not consume anything, but the
following raw write's trusted condition was worded as if reusing a
reclaimed address is always safe.

**Candidate directions (not chosen — needs a human decision):**
1. Tighten `[Rawptr-Write]` to drop the `reclaimed-at(addrs)`
   exemption entirely (matching `[Rawptr-Move-In]`'s strictness). This
   does not, by itself, fix anything observable — it only makes
   `Vec::push`'s own `unsafe` assertion honestly false for this input,
   which is progress (the documentation would stop claiming something
   untrue) but leaves the silent corruption reachable, since a
   `trusted` condition being false has no runtime effect.
2. Change `Vec::pop`'s plain-element path to also end the reclaimed
   object at the popped slot (unifying it with the resource path's
   behavior), which would need either a raw-boundary rule that reads
   *and* ends a plain reclaimed object (no such rule exists — only
   `[Rawptr-Read]`, which must not consume anything by design, and
   `[Rawptr-Move-Out]`, restricted to `is-resource(τ)`), or a library-
   level change to `Vec::pop`/`index_shared`'s contract.
3. Decide this is an accepted trust boundary — `Vec::index_shared`'s
   reference is documented as invalid after *any* length-changing
   operation on the same `Vec` (not just a reallocating one), and the
   corpus's existing "re-index after a reallocation" repair advice
   (`diag.stale-binding`'s registry entry) is widened to say so — but
   then the `unsafe` block's *trusted* (unchecked) status, rather than
   *checked*, means this failure mode is unlike every other
   `diag.stale-binding` case, which is at least dynamically caught.

This finding was originally recorded (not patched) here per
`CobaltC_Master_Instructions.md` §3/§20's instruction to write up
rather than choose among genuinely different design directions
unilaterally. The human owner subsequently directed a decision; see
"Resolution" above and `CHG-0019`.

## Eighth pass: prose-stated algorithms confirmed total (2026-09-20)

Four algorithms stated as prose/CFN hybrids, checked
for well-definedness against every stack/path shape the corpus's own
constructs can produce.

| id | finding | artifact |
|---|---|---|
| F-06 | (fixed, `CHG-0015`) `[Join]`/`[Handle-Destructor]` never re-keyed a resource-typed thread result's top-level destroy authority from the worker thread to the joiner/destroyer. `authority` is `Performer`-keyed (`spec/04`), granted to whichever thread's `establish` produced the object — the worker, for a value returned out of a spawned body. Without a transfer, `drop`ping a joined resource, or letting an unjoined handle's resource result be swept, would hit `[Destroy-No-Authority]` on its very first attempt — a spawn/join pair can never actually clean up a resource-typed result. No existing case exercised this (every prior `spec/19` case returns `i32`); fixed in `spec/19` 1.5.0, demonstrated by `conf.spawn-join-resource-result`. | `spec/19` 1.5.0, `spec/conformance.md` 3.10.0 |
| — | `[Destroy-Composite]`'s "longest path first, ties by reverse declaration/index order" (`spec/07` §4) had never been exercised by two sibling resource fields at the same struct depth (every existing case has a single obligation, or array-indexed obligations already totally ordered numerically). Read as a lexicographic total order (primary key: length descending; ties broken by declaration/index position at the first diverging component) it is well-defined for every shape the corpus's resource-bearing structs can produce; confirmed by `conf.destroy-composite-multi-field-order`. No two readings differ *observably* here (the order is unobservable without a destructor with external side effects, which none in this corpus has). | `spec/conformance.md` 3.9.0 |
| — | `unwind-to` (`spec/14` §2): the only frame shape whose scope-tracking is non-uniform is a parameter frame with no statement scope of its own (`current-scope = ⊥`, fixed by `CHG-0011` E-01) — `unwind-to`'s `t ≠ ⊥` guard already skips straight to `[Block-Exit]` for exactly this shape. `[Match]`'s `f_arm` (`CHG-0011` E-04) and `loop-body`'s frame are both ordinary block frames (each opens exactly one statement scope for its own trailing/current statement at a time), so neither introduces a new case. Already exercised end to end by `conf.e2e-propagate-chain`'s derivation (`return` folding `f_arm`'s scope/frame and `loop-body`'s scope/frame, "each pair matched by `scope-frame`"). No gap. | `spec/conformance.md` (existing `conf.e2e-propagate-chain`, `CHG-0011`) |
| — | `rule.control.flow-analysis`'s consuming-use transfer row (`CHG-0010` D-05) cross-checked against every construct that calls `end-object`/`transfer(`/`relocate-in(`/`[Rawptr-Move-In]` on a *named whole binding*: local-declaration/field/element/payload initializers, arguments (ordinary calls, `spawn`, `Mutex::new` — all call-shaped, the row is syntactic and doesn't special-case the callee), `return`, `move` capture, `match` scrutinee, destructure source, raw write — all listed. The one shape not listed, `x = y;` with `y` resource-typed, does not need a row: `spec/13`'s place/value-position table never puts a plain assignment's source in a resource-legal position, so `y` there is read as a value and hits `[Read-Resource-Rejected]` (`disposition: rejected`, unconditional) before flow analysis is ever consulted. Closed list confirmed. | — |

No item in this pass needed a design escalation; one
(`F-06`) needed a rule completion, applied directly (a mechanical
analogy to `rule.resauth.transfer`'s existing `rule.conc.spawn`
direction, not a design choice among alternatives — unlike `F-05`).

## Ninth pass: static/dynamic boundary usability review (2026-09-20)

`CHG-0016`. Two documented usability behaviors assessed as intended trades, not
gaps — no sharper `rule.control.flow-analysis` rule proposed. Each
gained a confirming paragraph in its normative artifact (`spec/14` §6,
`spec/10` §3) rather than a `D-XXXX` decision, since nothing about
which programs are accepted or rejected changes.

## Tenth pass: open design items prepared for the owner (2026-09-20)

Three open design items (`feat.explicit-lifetime-parameters`
expanded, `feat.minimal-io-extern-surface`,
`feat.consolidate-join-typing`) prepared with full §18 fields in
`spec/registry/features.md` 1.1.0 (`CHG-0017`); none decided.

The fourth, `spec/AUDIT-2.md` B-26 ("every decision converges on the
same design as one well-known prior-art language; candidate sets in
D-0006–D-0009 are thin"), checked against the actual records:
`D-0006` (type identity/conversion) considered 4 named candidates;
`D-0007` (evaluation order), `D-0008` (scope-end destruction), and
`D-0009` (failure channels) each considered exactly 2 — the selected
design and one alternative, not the "multiple conceptually distinct
candidates" Master Instructions §11 asks for on foundational problems.
**Assessment for the owner:** five verification passes since (this one
included) have found defects exclusively in *rule completeness*
(a premise that couldn't be discharged, a case a rule didn't cover) —
never a case where the *selected mechanism itself* (checked integers,
left-to-right evaluation, automatic scope-end destruction via `drop`,
`Result`/`?` for recoverable failure vs. a fatal `checked` fault for
violations) produced a wrong or awkward answer under adversarial
derivation. That is evidence the selections have held up, not proof a
wider search would have changed them — thin candidate sets are a
process gap regardless of outcome. Revisiting `D-0007`–`D-0009`
purely to backfill alternative candidates for entities already
`ACCEPTED` and load-bearing throughout the corpus would cost real
effort for a rationale-quality improvement with no expected semantic
change; recorded here as `RECORDED (open)` — not scheduled — pending
the owner's own priority call, consistent with `spec/AUDIT-2.md`'s
original disposition ("not a defect").

## Eleventh pass: implementation-feasibility analysis finalized (2026-09-20)

`CHG-0018`: `spec/IMPLEMENTATION-NOTES.md` added,
consolidating every `outcome: impl-defined` choice, every `outcome:
unspecified` point, and every `discharge: trusted` condition with its
governing rule, per Master Instructions §20. The design agent's entire
work queue is now complete. At the time of this pass, one item
remained genuinely open for the human owner: `F-05` (a raw-pointer
trust-boundary soundness question); `IMPLEMENTATION-NOTES.md`
recommended resolving it before authorizing a reference
implementation. **F-05 is now closed** — the owner decided it
immediately after this pass; see `CHG-0019`. The four
`spec/registry/features.md` entries under `UNDER_INVESTIGATION`
(tenth pass) remain open, by design (feature decisions, not soundness
questions).

## Twelfth pass: F-05 decided and closed (2026-09-20)

The human owner directed a decision on F-05 rather than leaving it
open. Selected: `spec/AUDIT-STATUS.md`'s own candidate direction 2
(`Vec::pop` releases its slot for every element type, not only
resources). Applied as `CHG-0019`: `spec/21` 2.7.0 (`Vec::pop`),
`spec/20` 1.6.0 (`[Rawptr-Read]`/`[Rawptr-Write]` tightened to match),
`conf.e2e-vec-pop-push-reuse-stale-ref` derived to confirm the
original reproduction now faults `diag.stale-binding` dynamically
instead of silently reading the wrong value. No existing conformance
row's outcome changed (re-verified: no existing row writes to an
address with a still-live reclaimed object). `spec/IMPLEMENTATION-
NOTES.md`'s recommendation is satisfied — nothing outstanding blocks
authorizing a reference implementation on soundness grounds.

## Thirteenth pass: `feat.minimal-io-extern-surface` decided (2026-09-20)

The human owner directed a decision rather than leaving this open
design item further. Selected: option (a) from the tenth pass's
prepared field set — one minimal `extern fn` declared in the prelude.
Applied as `CHG-0020`: `spec/21` 2.8.0 adds `extern fn write(rawptr<u8>
buf, usize len) : isize;` to §0's prelude table, resolved by the
already-existing `rule.trust.extern-call` `[Extern-Call]` with no new
rule; `spec/registry/features.md` 1.2.0 moves
`feat.minimal-io-extern-surface` `UNDER_INVESTIGATION → ACCEPTED`. The
other three items from the tenth pass (`feat.explicit-lifetime-
parameters`, `feat.consolidate-join-typing`, B-26) remain open, by
design (feature/process decisions, not soundness questions).

## Fourteenth pass: remaining three open items decided (2026-09-20)

The human owner directed a decision on all three remaining tenth-pass
items, delegating the choice itself to the design agent (reserving
only the separate reference-implementation authorization, `Master
Instructions` §1, which stays undecided).

- `feat.explicit-lifetime-parameters` → `REJECTED` (`CHG-0022`):
  option (c) (the existing `[Call-Multi-Ref-Return-Rejected]`
  boundary) retained permanently; (a) named lifetime parameters and
  (b) unconditional zero-parameter acceptance both declined — neither
  is forced by any construct in the corpus, and Master Instructions
  §9/§7 counsel against building either mechanism pre-emptively. No
  rule changed.
- `feat.consolidate-join-typing` → `REJECTED` (`CHG-0023`): the
  redundancy question is settled, not merely assessed — `join` is
  never a `Σ.items` entry (`state.items`'s `Item` grammar has no
  intrinsic alternative), so `[T-Item]`/`[T-Call]` can never reach it
  regardless of arity, and `[T-Join]` is the sole source of its type,
  exactly as `[T-Spawn]` is for `spawn`. `spec/04` 1.3.0 and `spec/12`
  1.6.1 each gain a clarifying paragraph; no rule changed.
- B-26 (thin `D-0006`–`D-0009` candidate sets) → `CLOSED` (`CHG-0024`):
  reviewed, not expanded. Each rejected alternative in `D-0007`–`D-0009`
  was eliminated by direct appeal to an already-`ACCEPTED` doctrine
  section (§19, §9), not by an unconsidered gap; `D-0006` already had
  4 candidates. Five verification passes have found rule-completeness
  defects but never a wrong-*mechanism* defect in any of the four.
  Each of `D-0006`–`D-0009` gains a `Candidate-set review` section
  recording this conclusion; no decision's selected design changed.

No `feat.*` entry remains `UNDER_INVESTIGATION`; no `AUDIT-2.md`
finding remains open except the reference-implementation authorization
itself, which `Master Instructions` §1 reserves to the human owner and
which this pass does not touch.

## Mechanical checks (last run 2026-09-20, twelfth pass)

What is checked:

- Every `diag.*` referenced in a normative artifact is defined in
  `spec/registry/diagnostics.md`; every defined diagnostic is
  referenced by at least one rule.
- Every `rule.*` id referenced has a `### rule.…` header with Status.
- Every `[Label]` cited in a normative artifact is defined by a rule,
  except labels a Change Log names as removed.
- Every `Depends on:`/`Affects:` value in `spec/00`–`spec/22`, the
  registries, `examples.md`, and `conformance.md` is a list of entity
  ids under `spec/02` §4's corrected extraction command (696 ids as of
  the fifth pass; the 1.0.0 command, which matched only plain
  single-line labels, had seen 57 and reported "clean" — the A-15
  closure above was vacuous until the fifth pass).
- Every `ex.*` cited by the registry exists; every `conf.*` cited by
  an example exists; every `CHG-XXXX`/`D-XXXX` cited exists.
- No implementation code exists under `spec/`.

Result, re-run clean after every batch in the sixth through twelfth
passes (`CHG-0012`–`CHG-0019`): zero non-conforming tokens from the
dependency-line command. The only non-clean output from the
`rule.*`/`diag.*` header/definition checks is four pre-existing false
positives, unchanged since before this session and not touched by it:
`rule.ref.deref-*` (a historical Change Log citation of a retired
name, `spec/08`), `rule.resauth.leak` (a named, retired rule discussed
in prose, `spec/07` §5), and `diag.d`/`diag.x` (metavariable
placeholders in `spec/21` §0's `fault(d)` table row and
`spec/conformance.md`'s own header conventions, never real diagnostic
ids). A handful of identifiers this session's own new prose
accidentally line-wrapped mid-token (invisible in rendered Markdown as
an inserted space) were found by an ad hoc script and fixed in place
as they were introduced; none reached this recorded run.

`TODO.md` (the design agent's now-fully-checked-off work queue,
non-normative per its own header) was removed once every item was
resolved; the roughly two dozen citations of it across
`spec/AUDIT-STATUS.md`, `spec/10`, `spec/14`,
`spec/registry/features.md`, and six `spec/changes/*.md` records were
reworded in place to stand on their own, with no loss of the
substantive history each was recording. Re-run of every check above
afterward: still clean.
