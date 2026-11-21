# D-0292 — Light references: a decided function's references carry no record, in any program

Status: ACCEPTED (2026-10-29, under D-0281's delegation: the owner handed the cost model to the assistant "to
manage autonomously, with authorisation given to make all necessary decisions", and asked for all of D-0281 to be
completed without further prompting). D-0281 phase 3, steps 0–3 of `stress/d0281/phase3/DESIGN.md` §5.
Kind: Decision (`spec/02-schema.md`)
Governed by: `CobaltC_Master_Instructions.md` §9, §12
Depends on: D-0018, D-0111, D-0202, D-0213, D-0281, D-0282, D-0283, D-0284, D-0290, D-0291
Affects: `spec/14-control-flow.md` 1.41.0 (§6 the cost contract); `spec/conformance.md` 3.283.0;
`spec/IMPLEMENTATION-NOTES.md` (the runtime contract); `impl/cbrt/src/lib.rs`, `impl/cbrt/include/cbrt.h`,
`impl/cobc/src/lower/*.rs`, `impl/src/typecheck.rs`, `impl/src/interp.rs`; `CHG-0387`

## Problem

A decided program (D-0284) runs with no record of references. A program that is not decided — one function
the static pass left a conflict in, one reference kept in data it does not follow, one handle lent where no
binding holds it — kept, until this decision, the whole record of `spec/08` for *every* reference in *every*
function: each `&x`, `&s.f`, `&*p` minted a path on its owner, stamped it into the statement's scope and retired
it at the statement's end, whether or not anything could ever clash with it. Of the 13,397 programs of the
soundness sweep, 996 keep paths (D-0291): every program with a `Vec<StringView>` or a struct of references
returned from a function, every program a thread handle leaves, every program with a point check. In such a
program the path records are a smaller cost than the objects of values and temporaries (the objectless track,
D-0285/D-0286, and phase 3 steps 6–7), but they are a cost paid for references the static pass had already
decided, and they stood in the way of lowering a decided function of a tracked program as a decided program's
function is lowered. A decided program pays too: its bindings minted root paths (`cb_bind` is no macro).

## Decision

1. **Two kinds of reference at run time.** A reference the static pass gave facts to
   (`rule.control.frame-references`), formed in a function it left nothing to the run, is *light*: it is checked
   where it is formed and recorded nowhere, and no access through it, and not its end, touches the runtime. A
   reference without facts — stored into data, read out of data, derived from one of those, lent to a thread, a
   lock's guard, or formed in or arriving in a function whose conflicts the static pass left to the run — is
   *recorded*: a path on its owner, as `spec/08` describes.
2. **Tokens.** A reference carries a token (`cb_ref.tok`): `0`, no record and no owner known (as before: a proven
   reference, a static's); `LIGHT | ROOT | obj`, the owner's own binding, valid while it is the object's current
   root; `LIGHT | obj`, a light reference into `obj`; otherwise a path id, a recorded reference. Two bits of the
   path id's generation name the kind (bit 53 `LIGHT`, bit 54 `ROOT`; the generation keeps 21 bits), and no table
   resolves a light token. An owner's binding mints no root path.
3. **Formation.** From the owner's binding, in a decided function: the owner must be the binding's (else
   `diag.stale-binding`); when holders exist on the object, the new reference's place, given explicitly by the
   lowering, is checked against them (`[Borrow]`'s `¬clash`); the result is light. From a light reference: the
   object must be alive; nothing else is checked — a holder that overlapped the base would have met it at its
   own formation or at the base's (DESIGN.md §2.1's induction over holders' formation order). From a recorded
   reference: as before.
4. **Accesses.** Through a light reference nothing is checked (a resource target written through it is
   `diag.overwrite-of-live-resource`, as through token 0). Through the owner's binding: the holders, as before,
   which costs nothing when there are none. Through a recorded reference: as before.
5. **Holders are made where a reference is stored.** `[Write]` of a reference value into any slot (a field, an
   element, a closure's capture, a thread's block, a binding's own object) is given the referent's address and
   type: a light reference stored there becomes a path on its object with the projection of its place, read from
   the owner's type (`proj_of_addr`: fields by offset, elements by size, an enum's payload by the tag in memory),
   held by the slot alone. A recorded reference is stored as before. A slot never holds a light token.
6. **Recording frames.** A function the static pass left a conflict in (`Items::tracking_needed`), one with a
   point check (D-0283), every function of a library or of an `--all-checks` program, pushes a *recording* frame:
   every reference it forms is recorded, as before this decision, and every reference that arrives in it — a
   parameter, a call's result — is recorded on arrival with its place (`cb_rec_ref`; a parameter held by the frame
   for the call, a result stamped into its statement). A block's frame records what its function's does. A
   function whose body the static pass decided pushes a light frame.
7. **What stays recorded in every frame.** An element reference (`&v[i]`, `Vec::index_*`), whose object is the
   element's and whose lifetime the record ends; a slice or view (`&v[a .. b]`, `String::view`), which lives in
   its value's slot; a lock's guard; what `[Thread-Lend]` lends; a temporary's path. The block objects of phase 3
   step 7 are what would make element references light.
8. **The oracle.** `COBALTC_RECORD=full` records every root and every reference as before this decision, in the
   same binary; `COBALTC_RECORD=check` runs light and aborts where a light reference reaches an entry of a
   recording frame unrecorded, or a place the runtime cannot reconstruct. The phase-3 gate (`gate3.sh`) builds
   each program that keeps paths once and runs it light, full and under `coby`: the three must agree.
9. **Diagnostics are unchanged.** A clash between a light reference and a holder is reported at the later of the
   two, where the record reported it too: at the light reference's formation, or at the holder's formation or
   access. No diagnostic id or position moves.

## Why these, and not the alternatives

- Recording nothing for light references, rather than a cheaper record, is what makes D-0281 §14's tier-S lines
  hold inside a tracked program: form, copy, pass, return and end cost no runtime state. The induction of
  DESIGN.md §2.1 needs every formation of a light reference to check the owner's holders and every access
  through the owner's own binding to check them; it does not need light references in any table.
- The frame, not the token, says whether a borrow records: the lowering's borrow calls keep their arguments, and
  a function's natives (`std` bodies `cobc` emits) need no second form; a decided callee under a tracked caller
  forms light references, a tracked callee under a decided caller records what arrives.
- A light reference carries no projection, so the places that need one get it from the lowering, which has it
  (the store, the arrival, `drop(*r)`, a lock, a slice's container); every borrow through a light reference
  needs none (3).
- Element references and slices stay recorded because their lifetimes are what the record ends (an ephemeral
  element object lives while its paths do) and because a slice's path lives in its value's slot in any case;
  making them light is the work of phase 3 steps 6–7 (frame shadows, block objects), not of this step.
- Per-function lowering of values (no objects for a decided function's values in a tracked program, D-0285 and
  D-0286 per function) is not in this decision: it needs the calling convention of decided and tracked functions
  reconciled (`stress/d0281/phase3/PERFN.md`), and this decision is its precondition — a decided function's
  references are now light in any program.

## Consequences

- `impl/cbrt/src/lib.rs`: the token kinds; `make_root`; `PathView`/`view` (a token read the same way whatever its
  kind); `frame_rec`, `cb_frame_push_fn`, `cb_frame_push_rec`; `light_borrow`; `cb_hold_ref`, `cb_rec_ref`,
  `proj_of_addr`; `cb_borrow_range` and `cb_destroy_via` given the place, `cb_lock` the mutex's type; the
  `COBALTC_RECORD` switch and `COBALTC_RT_STATS` counters (roots light and recorded, light references, paths
  minted, holders at stores, arrivals recorded, marked-owner checks, projections from addresses).
- `impl/cobc`: `Fx::tracked` (a body in `tracking_needed`, with a point check — `Items::point_check_fns`, new — a
  library's or an `--all-checks` program's); the function frame marker (`R`/`G`); `cb_hold_ref` at the stores of
  reference values; `cb_rec_ref` at parameters and reference results of recording bodies; `-v` says "lowered
  with holders (D-0292)" for a program that keeps paths.
- Gates: the 167 dynamic conformance cases under GCC and Clang in check mode, the three-way differential over the
  996 programs that keep paths (`stress/d0281/phase3/gate3.sh`), the release suites.
- Measurements (`stress/d0281/phase3/BENCH.md`, instructions retired, GCC `-O2`): the micro programs `b` (a
  tracked program's 200k string appends), `d0`/`d` (200k views built and read), `h` (a reference stored in a
  `Vec` a million times): see `CHG-0387`. A decided program gains only its root paths (the self-hosted compiler
  `cbc`, decided since D-0291, −6%); the decided controls (`csvstat`, `extsort`, `wordpar`) are unchanged.
- Two conformance rows (`conf.light-ref-recorded-on-arrival`, `conf.light-ref-recorded-on-arrival-written`): a
  decided `main`'s references into fields of one struct arrive in a tracked function and are recorded with their
  places, so disjoint fields do not clash and the element write the values decide does.
- Next (D-0281 phase 3): step 4, the token-kind tests inline in `cbrt.h`; step 6, frame shadows for values that
  hold references; step 7, block objects, which make element references light; then per-function lowering of
  values (`PERFN.md`), the single port of the self-hosted compiler, and its measurement.
