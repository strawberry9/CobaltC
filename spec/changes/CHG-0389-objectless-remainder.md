# CHG-0389 — The objectless remainder: a decided program's values keep no object where no check could read it

Kind: Change record (`spec/02-schema.md` §2)
Status: ACCEPTED (2026-10-29)
Governed by: `CobaltC_Master_Instructions.md` §21
Depends on: D-0294, D-0286, D-0285, D-0281
Affects: `impl/cobc/src/lower/analysis_strings.rs`, `analysis_refs.rs`, `frames.rs`, `exprs.rs`, `matching.rs`,
`values.rs`, `closures.rs`, `stmts.rs`, `flights.rs`, `functions.rs`, `passes.rs`, `impl/cobc/src/main.rs`

## What changed

In `cobc`'s objectless analysis (`impl/cobc/src/lower/analysis_strings.rs`):

1. `objectless_type_in` answers a type met again on the walk yes (was no); the arm for a struct of the program's
   takes only names that are structs, so an enum of the program's reaches D-0286's enum arm (it never did);
   `std::Box<T>` over an objectless `T` is objectless in a decided program; `decided_plain` (a value with no
   resource and no `fn` in it, references included) counts as plain data there.
2. In a decided program the walk admits of a candidate: assignment to it (`id | 4`, `Fx::objectless_binding`
   requires a quiet type) and to a field of it (recorded as `=f`; plain data or a quiet type); the candidate as the
   value assigned to a place (moved out), as `x?`, as a nested block's or a `match` arm's tail (moved out), as
   `&x[a..b]` and `&x[i]` (seen), and as a `match` scrutinee (seen; moved when an arm binds a payload).
3. `COBC_DEBUG_W`: the callee of a rejected call argument (`W bad x in Call(f)`), the reason `assigned`, and the
   marker `W pass decided` (`main.rs`) where the second lowering's trace begins.

In the lowering:

4. `lower_assign` (`exprs.rs`): a write into an objectless local, whole or a field, destroys the old value with the
   write (`cb_write_quiet(0, …)` for a quiet type; any other resource faults `diag.overwrite-of-live-resource` if
   live, as through a direct parameter), skipped and then cleared when the whole binding's moved flag is set; a
   field write on a moved-away binding is stale.
5. `lower_match` (`matching.rs`): an objectless binding is matched in place (stale if moved away); an arm that
   moves a resource payload out sets the binding's moved flag (`bind_match_arm`).
6. `lower_slice_of_as` (`values.rs`): a slice of an objectless local in a decided program is the bounds check and
   token 0.
7. `lower_block_body` (`stmts.rs`) and `return` (`exprs.rs`): in a decided program a literal, or `Box::new` of a
   value (`box_new_expr`, `flights.rs`), that is the result is built with no object; `lower_box_new`
   (`closures.rs`) takes a call's result for the box with none (`arm_inert`).
8. `unstored_ref_params` (`analysis_refs.rs`): in a decided program the count and modes of the reference
   parameters do not gate the rule.
9. `lower_fn_frozen` (`functions.rs`): in a decided program the frozen body is the function, with no twin and no
   dispatcher.
10. `elide_record_chains` (`passes.rs`), run in a decided program before `resolve_scopes` on every body
    (`dumped_text_passes`) and on every `std` body's text (`ground_native`, `flights.rs`): an object's straight
    run of record-only statements (`cb_new`; `cb_bind` then `cb_take`; `cb_move_to`; `cb_result`; ended by
    `cb_forget_obj`, `cb_absorb` or `cb_raw_move_in`), its ids named nowhere else, no branch, no call that could
    fault, no `cb_frame_pop` and no `cb_stmt_pop` the object could still belong to between, is removed. (Its first
    form let any statement stand between: a fault in a call there no longer destroyed the value, and two
    programs whose destructor prints lost a line — `fault_ends_pending_argument`, `stress/d0194/q3` — in the
    1,500-program gate.)

## Verification

- `cargo +1.77.2 check --workspace --tests`; `cargo test --release -p cbrt`.
- `stress/d0281/objectless/dgate.sh` (decided build against the pathless-only build) over `s300.list` and
  `s1500.list`.
- The three-way differential (`gate3.sh`, `coby`, light under `COBALTC_RECORD=check`, full) over the 996 programs
  that keep paths (`kept146.list`), GCC.
- The release suites (`stress/suites/runs/u149`).
- Measurements in `CHG-0389`'s changelog entry and `stress/d0281/phase3/BENCH.md` (`runs/objl`).
