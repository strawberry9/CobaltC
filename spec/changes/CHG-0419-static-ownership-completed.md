# CHG-0419 — The static ownership plan completed: locals with destructors of their own in tables, a location stored only for a callee that may read it, the cost contract

Status: implemented (2026-11-09), D-0315
Affects: `spec/14` 1.44.0 (§6 "The cost contract": what a decided program's value costs at its end);
`spec/conformance.md` 3.301.0 (six rows); `cbrt` (`lib.rs`: an entry's live bit cleared as the unwind ends it);
`cobc` (`lower/`: `analysis_strings` — `noted_type`, the walk's unused `let`s; `frames` — `objectless_binding`,
`bind_value`, `bind_param`, `objectless_bound`, `pop_frame`, `push_sub_frame`, `end_diverged`, `notes_finish`;
`passes` — `reads_location_in`, the decided scope list; `stack_checks` — `quiet_call_locations`; `stmts`,
`functions`, `closures`, `exprs`, `places`, `native_text`, `syntax`, `program`; `report.rs`) and its twins in `cbc`
(`back/`: `gen`, `state`, `natives`, `native_text`, `passes`, `stack_checks`, `strings4`, `vec_scan`, `direct`,
`refs4`, `layout`; `front/typecheck`; `cbc/report`); `impl/cobc/tests/cost_rows.rs`. No rule changes; no program's
outcome, fault, location or output changes.

## What changes

1. **Noted values** (the plan's step 4, its second part). In a decided program, a `let`'s binder or a by-value
   parameter whose type has a destructor of its own — the program's `resource` structs and enums, over parts that free
   memory alone or are such values (`noted_type`) — is bound with no runtime object where the candidate walk admits
   it. Its end is the body's own code: its drop where its scope ends or a jump leaves it, under its moved flag where
   it may have been moved away. It has an entry in the function's table, registered on its own where the value is
   bound (`cb_notes_push(&cb_nt[k], 1u)`, at the frame stack's depth there) and popped by its drop; the runtime clears
   an entry's live bit as it ends it. A sub-frame opened after the value's statement (`push_sub_frame`) holds the
   objects bound after it and closes with the frame below it, so `[Block-Exit]`'s order holds between noted values
   and the objects the analysis still keeps, at a block's end and in every unwind (`CHG-0418`'s fold). An unused
   `let` is a candidate; a plain temporary used as a place is a place with token 0; `place = x;` of a memory-only
   type reads `x` with no object; `cb_fn_item` records nothing in a scope. A noted local moved into a call's
   argument goes over with no object only as the last argument; before a later one it takes its object as the
   statement's temporary it then is, which a fault in the later argument ends (found by the gate:
   `conf.fault-ends-pending-argument` and `conf.exit-ends-pending-argument` had lost the moved value's destructor).
2. **A location stored only where a callee may read it** (step 6, `spec/14` §6's last sentence). A function is
   *quiet* when no line of its body may read the executing statement's location (a fault without a location of its
   own, a call into the runtime, a stack check it keeps) and none stores one, its calls of quiet functions aside,
   decided over the program's functions to a fixed point; `prune_stack_checks` decides it from its own scan, and a
   body in which a store is followed by a call of a quiet function is analysed again with those calls read as
   nothing (`dead_locations_flow`). D-0309's round had tried a program-wide pass and dropped it ("no fewer cycles" on
   `records`): measured again, with every store removed by hand, `records`, `sieve`, `bench` and `strings` moved
   within noise (−1.8% to +0.6% of cycles), the interpreter of `stress/vsrust` 6.7% fewer cycles and `cbc` 4.7%, so
   the reason stood for the benchmarks and not for an interpreter. The pass is kept.
3. **In a decided program `deallocate` and `copy_raw` record nothing in a scope** (`cb_deallocate`,
   `cb_deallocate_plain`, `cb_copy_raw` in the decided list): a destructor that frees raw memory keeps no statement
   scope.
4. **`--report-tracking` counts what it missed.** A function's own frame (written before its first position) is
   counted at its first line; a closure's body (`cl_…`) begins a function as `f_…` does; a `#line` back to the
   generated C ends the program's position, so `cb_program`'s root frame is no line's (it had been charged to the
   last function's last line).
5. **`cbc`'s own work.** It reads a struct's field where the declaration is (`struct_field`), tests a declaration's
   presence without copying it and a function's head in place (`fn_head`), checks each function once (the type
   checker's facts are read where they are, not copied), visits each unit's definitions only (`prune_stack_checks`),
   and builds the statement-scope name sets once per program. Tried and dropped: a declaration without its body for a
   call (a call's direct form reads the body: the C changed), a declaration shared through `Rc` (in `cbc`, which keeps
   paths, every reference into it was checked at its end: 36% more instructions), and the body fetched only past the
   cheap tests (most calls pass them: 4% more).
6. **The cost contract** (step 7): `spec/14` §6 states what a decided program's value costs at its end where the
   analysis follows it, and that the frames and statement scopes left are those of values that keep a run-time object,
   which `--report-tracking` lists. `impl/cobc/tests/cost_rows.rs` checks that the report lists nothing for seven of
   the conformance cases of the forms D-0315 follows and for the nine benchmarks.
7. **Step 8 decided on its measurement**: the tables' registrations are 0.05–0.08% of `cbc`'s instructions on the
   speed test's programs; the generated drop glue is 5.4–6.0% (`cobfmt-cb`: 2.9%), but that is the values' freeing,
   which a `cleanup` attribute would run all the same. Nothing a cleanup attribute could remove reaches the plan's 5%;
   no design of it is written.

## Measured

On the owner's speed test (`timeit.py`, five runs, cobc and cbc from this source but for the gate's fix above, which changes no program of the speed test's C and `cbc`'s instructions by under 0.2%): `cbc` takes 0.29–1.49 times
`cobc`'s time per program, 1.28 times over all ten; the largest ratios are the type inferencer (1.49), the
13,000-line program (1.47) and the 3,900-line one (1.38). Instructions on `--emit-c`: 1.10 times `cobc`'s on the
Lisp interpreter, 1.27 on the type inferencer, 1.18 and 1.20 on the large programs. `cobfmt-cb` takes 0.03–0.70 times
`cobfmt`'s time on every file (`timefmt.py`, both its cases; the same text). The runtime is 10.8% of `cbc`'s cycles on
the Lisp interpreter (6.1% in `cbrt`, 4.7% in `cbrt.h`'s helpers); 71% before D-0315. Step 6: the interpreter of
`stress/vsrust` 4.2% fewer cycles; `cobc`'s own work up 0.8–1.4% of instructions for the pass, `cbc`'s within 1% either
way (its C gains what the pass costs).

`--report-tracking`: nothing for the ten programs of the speed test and the nine benchmarks; nothing for 48 of the
84 decided showcases; 217 lines in the other 36. About 115 of those are values the runtime owns by design (D-0315's
§3.5: threads' handles, mutexes, guards and channels, closures' boxes, `Rc` and `Weak`); 38 are `std`'s resource
types with destructors (`File`, sockets, HTTP clients and servers, processes), not yet admitted as noted values; the
rest are the frames and statement scopes of the bodies that hold them, a few aggregate literals holding values with
destructors, and raw memory. The proposal's target of no line for the showcases is not met.

## Conformance

`conf.noted-locals-end-in-order`, `conf.drop-flag-moved-in-one-branch`, `conf.drop-flag-moved-in-first-iteration`,
`conf.unwind-order-mixed-tables-and-frames`, `conf.drop-flag-partial-move-then-propagate`,
`conf.contained-fault-runs-table-destructors`: locals with destructors of their own, ending at a block's end, a
`return`, a `?`, after a move on one path or in one iteration, and in the unwinds of uncontained and contained faults
across bodies with tables and bodies with frames; alike under `coby` and `cobc` (gcc, clang) and the sanitizers
(ASan, UBSan, LSan). The partial move still keeps runtime objects (a destructure of a value with a destructor of its
own is a form the analysis does not yet follow); its outcome is the case's. Step 6 changed the C of 219 of the 1,305
conformance cases; each, built with and without it under gcc and clang, printed the same stdout and stderr (fault
locations included) and ended with the same status.
