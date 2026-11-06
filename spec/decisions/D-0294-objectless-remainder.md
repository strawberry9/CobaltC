# D-0294 — The objectless remainder: a decided program's values keep no object where no check could read it

Status: ACCEPTED (2026-10-29, under D-0281's delegation: the owner handed the cost model to the assistant "to
manage autonomously, with authorisation given to make all necessary decisions", and asked for all of D-0281 to be
completed without further prompting). D-0281 §24 (as amended at Update 148): the objectless remainder, D-0286's
option C, before phase 3 steps 6–7.
Kind: Decision (`spec/02-schema.md`)
Governed by: `CobaltC_Master_Instructions.md` §9, §12
Depends on: D-0281, D-0285, D-0286, D-0293
Affects: `impl/cobc/src/lower/analysis_strings.rs`, `analysis_refs.rs`, `frames.rs`, `exprs.rs`, `matching.rs`,
`values.rs`, `closures.rs`, `stmts.rs`, `flights.rs`, `functions.rs`, `passes.rs`, `impl/cobc/src/main.rs`;
`CHG-0389`

## Problem

After D-0293 the self-hosted compiler `cbc`, a decided program, spent 60% of its 6.86 G instructions in the
runtime's object machinery (`end_identity_in`, `detach`, `cb_stmt_pop`, `new_obj`, `make_pending`, the frame
push and pop, `cb_bind`, `cb_move_to`) although no path is ever checked in it: every resource value that is not
an *objectless local* (D-0196, D-0200, D-0286) still gets an object, bound in a frame or stamped in a statement
scope, moved from temporary to binding, and ended one by one. Its C made 24,261 objects and only 2,357 objectless
locals. The objectless analysis's trace (`COBC_DEBUG_W`, Update 148) said why its 2,362 resource bindings kept
objects: 1,578 for their type (the recursive `ast::Type`, `ast::Expr`, `state::V`, and every `Option`, `Vec` or
`HashMap` of them: `objectless_type` answered no to a type that holds itself, and — a slip since D-0286 — to every
enum of the program's, because the arm for a program's *struct* took every name that was not `std`'s), 248 for
being assigned to, 139 for being matched on, and the rest for being sliced, moved into a place, propagated with
`?`, or being a nested block's tail. And beyond the bindings, the objects of temporaries: a struct or variant
literal that is the function's result got an object and forgot it in the next statement; a `std` body's result
was bound, taken back, moved and forgotten in five calls; every reference parameter a function held in a local
got an object nothing read.

## Decision

In a decided program (`Gen::decided`, D-0285: no path exists, every path entry of the runtime is a constant), the
compiler places no object where no check could read it:

1. **Recursive types are objectless.** `objectless_type` answers a type met again on its walk *yes*
   (coinductively: the whole's answer is its other parts'); a program's enum reaches D-0286's enum rule; a `Box`
   over such a value is such a value (the runtime ends it natively from the type descriptor); and a value with no
   resource and no `fn` in it counts as plain data (`decided_plain`: a reference inside a value is tracked by
   nothing there).
2. **The uses an objectless local may have grow to what the lowering can do without an object**: assigned whole
   (a quiet type: the old value is destroyed by the write, `cb_write_quiet`, unless the binding's moved flag says
   it was moved away, and the flag is cleared); a field written (plain data, or a quiet type destroyed the same
   way; a moved-away binding is stale); moved into a place (`p.f = x`), propagated (`x?`), a nested block's or a
   `match` arm's tail (moved out, as a call's argument is); sliced (`&x[a..b]`: the bounds, and token 0) and its
   element borrowed (`&x[i]`: the vector's address, token 0); matched in place (`match (x)`: the discriminant is
   read where it lies, and an arm that moves a payload out sets the binding's moved flag, as `cb_take` would mark
   its object).
3. **Results and arguments built in place leave with no object.** A struct or variant literal, or `Box::new` of a
   value, that is a function's result (a block's tail or `return`) is built with no object; `Box::new`'s argument
   takes a call's result with none.
4. **A record nothing reads is not kept.** A text pass over each decided body and each `std` body's text
   (`elide_record_chains`) removes an object's whole life when it is one straight run of statements — made,
   perhaps bound and taken back, moved, promoted to the enclosing scope and ended (forgotten, absorbed, or moved
   into raw memory) — with no branch, no call that could fault (a destructor the fault would run on the value is
   observable), no frame's end, a scope's end only once the object was promoted past it or is bound, and its ids
   named nowhere else: every one of those statements is a record no check reads, and the object's end comes
   before any scope's end or fault that could destroy the value.
5. **Every reference parameter is unstored.** The count and the modes of the references into a function decided
   whether its parameters needed no slot (`unstored_ref_params`: one, or all shared); in a decided program no
   check reads a slot, so every reference parameter the body never assigns, borrows, captures or shadows is
   unstored (no object, no slot).
6. **One body per function.** The frozen twins (CHG-0237: `NAME__frz` and `NAME__chk`, chosen at entry by
   `cb_frozen_ok`) are one body in a decided program, where the test is the constant 1.

Nothing changes in a program that keeps paths beyond rule 1's first clause (a recursive type over objectless parts
is objectless there too, with the same uses as before) and the enum repair, which reaches only decided programs
(D-0286's rule is theirs).

## Why these, and not the alternatives

- The rules are the lowering's, not the runtime's: a decided program's runtime entries are already constants, and
  what remained was the objects the compiler asked for. Removing the asking, case by case where the lowering can
  drop or move a value itself, is D-0286's option C as §24 names it.
- The text pass (rule 4) exists because the same chains arise from many lowering sites (results of `std` bodies
  whose fixed text binds and takes back, literals absorbed into results, temporaries promoted and forgotten) and
  a single conservative rule — straight-line, every mention accounted for, ended before any scope that could
  destroy it — covers them all; the lowering changes (rule 3) take the commonest at the source so the C is
  smaller too.
- Rule 2's `match` keeps D-0049's meaning: a binding matched by value stays where it is unless the arm taken moves
  its payload out, and then it is moved from; the objectless form says so with the flag the binding already has
  for its other moves.
- The frozen twins' dispatcher cannot choose the checked body in a decided program; emitting both doubled the C of
  every function with a frozen parameter (and would have more so once rule 5 made them common).

## Consequences

- Over the self-hosted compiler's decided lowering: of its resource bindings 165 keep objects (were 2,362); its
  C makes 13,724 objects (were 24,261) and binds 2,862 in frames (were 8,637); `cbc --emit-c` of
  `showcase/tier6/anagrams.cb` retires 3.51 G instructions (was 6.86 G: 0.51); `cbparse` 0.48, `cobfmt` 0.55, the
  decided showcases 0.71–0.94, the controls 0.92–0.99, the tracked micro programs unchanged. Its profile is now `cb_stmt_pop`
  8.5%, `cb_frame_pop` 6.3%, `drop_in_place_tail` 5.7% (destruction itself), `end_identity_in` 5.5%,
  `make_pending` 5.0%, `detach` 4.4%: the statement scopes and frames that remain for the objects that remain
  (binders of `match` arms over non-objectless payloads, locals the analysis still rejects, temporaries of
  arguments) — the next round of the same work, after the tracked programs' turn.
- Measured in full in `stress/d0281/phase3/BENCH.md` (`runs/objl`), `CHG-0389`.
- Gates: `stress/d0281/objectless/dgate.sh` (the decided build against the pathless-only build) over the
  300-program and the 1,500-program samples; the three-way differential over the 996 programs that keep paths
  (rule 1's first clause reaches them); the release suites.
- `COBC_DEBUG_W` names the callee of a rejected call argument and marks where the decided lowering's trace
  begins (`W pass decided`); `objectless_binding`'s reasons gain `assigned`.
