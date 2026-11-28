# CHG-0481 — One file by default, the partly fixed reports, and cobmodel's explanation

Status: implemented (2026-11-24); Update 209
Affects: `spec/09` 1.10.0, `spec/10` 1.9.0, `spec/14` 1.53.0, `spec/20` 1.23.0, `spec/21` 4.90.0, `spec/22` 2.69.0,
`spec/IMPLEMENTATION-NOTES.md`, `spec/OVERVIEW.md` 1.3.5, `spec/registry/diagnostics.md` 1.75.0, `spec/conformance.md`
3.326.0; `impl/cobc` (`driver.cb`, `driverbase.cb`, `report.cb`, `build_info.cb` generated), `impl/build.sh`,
`impl/STATUS.md`, `impl/front` (`typecheck.cb`, `parser.cb`, `ast.cb`, `dump.cb`, `prelude.cb` generated),
`impl/back` (`gen.cb`, `layout.cb`, `native_io.cb`, `passes.cb`, `header.cb` generated), `impl/cbrt` (`rt_threads.c`,
`rt_threads.h`, `rt_lock.c`, `rt_objs.c`, `rt_objs.h`, `rt_refs.c`, `rt_init.c`, `rt.h`, `include/cbrt.h`,
`tools/exports.txt`), `impl/std/database.cb`, `impl/tools/gen_saslprep.py`, `impl/model` (`src/compare.cb`,
`src/statics.cb`, `src/main.cb`, `README.md`, `sample/hand-w03.cb`, `sample/jump-p001150.cb`); the cases under
`impl/conformance` (06, 18, 19, 20, 21), `impl/tests/cases/cobc`, `impl/tests/programs`, `impl/tests/rows`; `book/`
(ch13, ch14, ch16, appendix A; examples `ch10/text_types.cb`, `ch13/lend`, `ch14/variadic`), `htmlguide/`
(`content_a.py`, `content_b.py`, `content_c.py`, `content_intro.py`, `content_perf.py`, `content_runtime.py`;
`index.html` and the appendices generated); `CARD.md`; D-0338.

## Why

The owner asked for three things in one push with one gate at the end: `cobc` compiles a program's C as one file
with no link-time optimisation by default, a new `--lto` opting in to the former default and `--one-unit` removed;
the partly fixed reports addressed where it makes sense, 311, 315 and 337 from Update 208's round (CHG-0480, "What is
left") and 059, 065, 073, 108, 109, 114, 148, 183, 209, 228 and 229 from Update 207's (each report's "Status after
Update 207"); and the guide and the book explaining why `cobmodel` exists, when to use it and how. The owner
delegated the decision on 315's lock redesign ("I let you make the decision on whether a new design is a better
option for CobaltC"); it was chosen and implemented (D-0338 §2.2). Five agents did the work, by area (the driver and
build time, the checker, the runtime, `std` and C interop, `cobmodel`'s documentation). D-0338 has the choices; this
record has every item.

## What changes

The number is the report's.

**The driver:**

| # | Cause | Now |
|---|---|---|
| — | D-0277's default compiled a large program's C in parts with link-time optimisation; `--one-unit` (CHG-0470) gave one file without it, which `impl/build.sh` used for the compiler (the parts made it 6% slower, 9% larger) | one file with no link-time optimisation is the default (`build_exe`: one part unless `want_lto()`); `--lto` gives the parts and `-flto`/ThinLTO (`driverbase.cb` `set_want_lto`/`want_lto`); usage and `--help` say so (D-0338 §2.1) |
| — | `--one-unit` would have become a no-op | refused as an unknown option, status 2: "the program's C is compiled as one file by default; --lto builds it in parts with link-time optimisation" (`unknown_option`) |
| — | the executable cache's key marked the one-file form (`one unit`) | head `cobc build cache 2`, an `lto` line under `--lto`; Update 208's entries never match |
| — | `impl/build.sh` asked for `--one-unit` | unchanged logic: a compiler whose `--help` names `--one-unit` (stage 1, an installed pre-209 compiler) is given it, the new compiler nothing; comments rewritten |
| 073 | a `match` on text lowered to one `if (cb_bytes_eq(…))` per arm, which GCC's passes take superlinearly (16,000 arms 179 s; 136 s for the function alone) | more than 64 arms, every arm before the last a distinct text literal: two function-local `static const` tables (the literals sorted by `cb_bytes_cmp`, their arm indices), a binary search into the arm's index, then the arms as before (`gen.cb` `text_match_table`, from `lower_match_scalar`); 1,000/4,000/16,000 arms 0.78/1.81/8.42 s with GCC, Clang 8,000 arms 37.5 → 10.4 s; 4,000 arms, 10^6 lookups 33.6 → 0.12 s (D-0338 §2.8) |
| 059 | a large program's first build after an edit compiles all of its C | documented: the guide's "How long a build takes, and `--lto`" gives the costs, what the cache saves and does not, and `--lto`/`-O 0` for the edit loop; an object cache of `std`'s C is left for the owner (D-0277, D-0338 §3) |

**The checker:**

| # | Cause | Now |
|---|---|---|
| 108 | `part_through_ref` did not count a slice as a reference, so `ps[0].name` through `slice<Person, shared>` was moved out and faulted | a slice base counts as a `ref` does: `diag.move-out-of-field`, "`ps[0].name` is reached through a reference …" (D-0338 §2.12) |
| 109 | `closure_into_outer_container` looked only at closure literals, and an element store's escape check (`place_of`) did not follow a `Vec` element | `Body.closure_borrows` follows a non-`move` closure binding (through `auto d = c;`); `Vec::push`/`insert`, `HashMap::insert`, `Queue::push_*` and a store into a field or element refuse it, `diag.destroy-while-aliased`, as the literal (D-0338 §2.12) |
| 114 | D-0336's end after a condition covered `if` only, so a reference last read by a `match` scrutinee was kept through the arms | for an integer or `bool` scrutinee, a binding last mentioned there and in no arm ends once it is read (`let_cond_end`, `end_after_scrutinee`, `Facts.cond_ends`; `gen.cb` `lower_match` reads the scrutinee into a temporary and ends those bindings); default and `--all-checks` agree (`spec/14` `[Ref-Binding-Last-Use]`, D-0338 §2.5) |
| 183 | `pw_expr` classified a `match` scrutinee mentioning the parameter as kept, so `if (Some(rest) = StringView::strip_prefix(path, "x."))` kept `path` | the scrutinee keeps `q` only where its binders are kept, each followed through its arm as a binding from `q` is, every part read through it kept (`spec/10` `[Result-Source-Probe-Only]`, D-0338 §2.6) |
| 229 | `value_shape` had no shape for a block, so `&{String::copy("abcd")}` where a `StringView` is expected was "not sliced in this form" | a block or `unsafe` block declaring no binding has its tail's shape (`block_value_shape`); outside an argument the `diag.borrow-of-non-place` text says such a borrow is allowed only as a call's argument and to bind the value first (`spec/09`, `spec/21`, D-0338 §2.7) |
| 311 | `overlap3` took any non-literal range as "may overlap", so `&mut s[0..mid]` beside `&mut s[mid..n]` was tracked and each `s[i]` a check shared between threads | new path step `PSpan` for a range with a literal or *stable* local bound (`stable_bound_names`); two spans of which one ends where the other starts by the same key do not overlap (`spec/14` §6, D-0338 §2.4); the report's program untracked, 32.0e9 → 0.43e9 instructions at depth 2; three negatives (bound written between the borrows, rebound in a block, declared in a loop) still fault |

**The runtime:**

| # | Cause | Now |
|---|---|---|
| 315 | an uncontended lock took its stripe's mutex, a holder-record insert/remove and a waiters lookup, and each guard's end entered the runtime under its global lock: 62–73 ns a lock, four threads on the locked map 0.5–3.6 s | the lock is the mutex's state cells (`rt_threads.h` `MX_*`: holder id, waited-for bit, poison): one compare-and-swap each way uncontended; a waiter sets the bit under the book and a release that sees it wakes through the book; each thread's held list in its own state for poisoning (`TState::held`), cleared under the book when a guard ends on another thread (`release_moved`); the deadlock finder and `[Lock-Reentrant]` read the cells; a guard's descriptor carries the state-cell offset (`layout.cb`), `Channel`'s native send and receive pass the mutex's type (`native_io.cb`); a decided program's guard ends without the runtime's lock (`cb_drop_local` → `guard_release`); stripe lock records and maps removed. Lock-only loop 1.006e9 → 0.428e9 instructions, 30 ns a lock; locked map one thread 178 → 62 ms, four threads 482–716 → 116–134 ms (D-0338 §2.2) |
| 337 | a long `Rc` chain recursed `Rc::drop` → overwrite → `drop_in_place` → `Rc::drop` until the stack was exhausted (one report since Update 208) | `std::Rc`'s descriptor is marked (`owner` 2, `TypeInfo::chain_drop`); an `Rc` reached in tail position while an `Rc` destructor runs waits on a per-thread list, run in a flat loop when that destructor returns (`chain_drain`), in the nested order; decided form only, not during the unwind, cleared by a contained fault (D-0338 §2.10) |
| 065 | `-a`, `a / b`, `a % b` and shifts faulted through plain `cb_fault`: "arithmetic overflow", no values | new export `cb_fault_op` (`rt_init.c`, `cbrt.h`, `exports.txt`, `header.cb` regenerated, `passes.cb`'s name lists), called in the fault branch only with the temporaries the check holds (`gen.cb` `fault_op_call`): "-(-2147483648) is outside `i32` (…)", "7 / 0: the divisor is zero", "1 << 40: the shift amount 40 is not below 32, the width of `i32`"; instructions unchanged on the hot path (`spec/IMPLEMENTATION-NOTES.md` row) |
| 148 | a call of a `fn` value through a shared reference ran `cb_fn_shared` at every call, which looked up the type even of a closure capturing nothing | the generated call checks only a box whose closure bit says it captures (`call_fn_place`); `cb_fn_shared` returns at once for a capture-free closure and for a heap box known to hold no reference; `--report-tracking` lists the checks left, in decided programs too (`report.cb` `fn_checks`); the report's `fn_value_through_ref` 1.205e9 → 0.775e9 instructions (GCC; 0.705e9 Clang), a `fn` field called through a reference as fast as through a parameter (62 ms, GCC) |

**`std` and C interop:**

| # | Cause | Now |
|---|---|---|
| 228 | an `extern` declaration was a fixed C prototype unless its name was in `gen.cb`'s table (D-0336 §2.20), so `strfmon`, glibc's `error` and the program's own variadic C were called with the fixed convention (x86-64: `%al` unset, doubles misplaced) | `unsafe extern fn name(τ1 p1, …, τn pn, ...) : τr;` declares a variadic C function (`ast.cb` `ExternDecl.variadic`, `parser.cb`, `typecheck.cb`: "at least N" arguments, the rest typed by their own text and each an `FfiType` other than `void`; `dump.cb`; `gen.cb`: `extern R cbx_name(fixed…, ...) __asm__("name")`); `(...)` alone and `..., x` are syntax errors with their own messages; the table kept and extended (`strfmon`, `strfmon_l`, `error`, `error_at_line`, `semctl`, `mremap`, `ptrace`) (`spec/20` `[Extern-Call-Variadic]`, `[Extern-Call-Variadic-Arg]`; `spec/22` §3; `diag.extern-non-ffi-type`; D-0338 §2.3) |
| 209 | `saslprep` followed RFC 4013's letter where PostgreSQL's SASLprep reads it differently: 8 of 18 test passwords prepared differently from the server's | C.1.2 before B.1, the checks on the mapped text before NFKC, an empty result refused, ASCII as given, Corrigendum #4's decompositions (`database.cb`; `gen_saslprep.py` and its tables; unit tests "saslprep: the RFC's examples" and "as PostgreSQL's"); 0 differences against the local PostgreSQL 14 under GCC and Clang, a wrong password refused (`spec/21` `[Pg-Connect]`, D-0338 §2.9) |

**`cobmodel`:**

| # | Cause | Now |
|---|---|---|
| — | since report 274 `cobc --explore` shows a later outcome from its first differing line, behind `(lines 1-N as the first outcome's)` and `(as the first outcome's)`, and cuts with `... N more line(s)`; `compare.cb`'s `explored()` read the marks as program output (19 false OUTSIDE over the sample's 82 multi-outcome programs) | `explored()` keeps the first outcome's lines and rebuilds a later outcome's prefix from its mark (`mark_of`, `add_line`); cut lines dropped; the earlier form reads as before |
| — | the model's statics lacked report 313's row (D-0337 §2.9), so `hand-w03.cb` and `jump-p001150.cb` disagreed with `cobc --check` | `check_access_refute`'s `¬clash` row refutes a `?` fact held by a `handle` or `Vec<handle<void>>` declared outside the innermost open loop (`Cx::thread_holder`, `held_across_rounds`); the two samples' headers say `diag.aliasing-conflict (static)` |
| — | `cobmodel --help` printed the usage with status 2; the usage omitted `trace`, `enum`'s random options and `compare --report` | `--help` and `-h` print it on standard output, status 0; the usage is complete (`main.cb`) |

**Documentation:**

| Where | Now |
|---|---|
| guide §19 | new "Every schedule, from the rules: `cobmodel`" (`s19-cobmodel`): why it exists, a question-to-command table, `lend.cb` under `cobc --explore 8`, `cobmodel check`, `--explore 300`, the fix and `--witness`; "When to use it, and when not"; "Using it on a directory, in tests and in CI" (`compare` and its flags, OUTSIDE/unobserved/truncated/header-stale, `// MODEL:` lines, `statics`, `certify`, the gate's unit, `enum`); "The guarantee" links it; `counter.cb`'s `printf` (a real newline in the generator) fixed |
| guide introduction | `cobmodel` among the tools; new "How long a build takes, and `--lto`" with the measurements (D-0338 §2.1) |
| guide, other | `content_perf.py`: which ranges are told apart (311), the lock's cost (315); `content_c.py`: the lock block's cost, variadic `extern` declarations (228); `content_b.py`: a named closure stored in a container refused (109); `content_a.py`: a block's value borrowed as an argument (229); `content_runtime.py`: `cb_fault_op` in appendix B (065) |
| book | ch13 "Every schedule at once: `cobmodel`" with the tested example `ch13/lend.cb` (`--explore 8`); ch14 variadic C functions with `ch14/variadic.cb`; ch16 the build forms and `cobmodel`; appendix A the `...` form and `cobmodel check`; `ch10/text_types.cb`'s comment brought to the chapter's text (`embedded.py` failed on it at `90757a5`) |
| spec | `spec/OVERVIEW.md` §11 describes the model as it is (D-0326, its fragment, `rule.control.frame-references`, eight programs under `--pre-d0236`, the gate's unit, `cobc --explore 8` under GCC and Clang); `impl/model/README.md` the same count |
| `impl/STATUS.md` | the build forms |

**Test expectations:**

| Case | Now |
|---|---|
| `impl/tests/cases/cobc/driver_one_unit.case` | replaced by `driver_lto.case`: no `-flto` by default, `-flto` with `--lto`, `--one-unit` status 2 and its message |
| new cases | `match_text_many_arms.case` (073), `checker_partly_fixed_207.case` (16 programs: 108, 109, 114, 183, 229, 311), `runtime_update209.case` (065, 315, 337), `extern_variadic_strfmon.case` (228); conformance `extern_variadic_ellipsis_ok`, `extern_variadic_own_c_ok` (with `extern_code/varsum.c`), `extern_variadic_reference_arg_rejected`, with rows `conf.extern-variadic-ellipsis`, `conf.extern-variadic-own-c`, `conf.extern-variadic-arg` |
| 065's fault texts | re-recorded: conformance `div_by_zero_dynamic`, `div_overflow_min_neg_one`, `negation_min_value_overflow_dynamic`, `shift_amount_out_of_range`, `shift_width_edges`, `checked_fault_unwinds_and_terminates`, `fault_unwind_destructor_fault_first_wins`, `thread_taking_over_path_not_contained_rejected`, `failed_handle_in_main_unwind_rejected`, `rng_below_zero_faults`, `vec_predicates_ok_then_fault`; six programs and five rows under `impl/tests` |
| `runtime_reports_249_streams_and_faults.case` | the `Rc` chain is now `built dropped` with no fault; under `--all-checks` still one `diag.stack-exhausted` (337) |
| left stale by Update 208 | `driver_frame_held_aggregates.case` expects 291's static `diag.reference-escapes-scope`; `dead_locations_set_where_read.case` and `loop_vec_header_copies.case` take 252's carets |

**Found by the gate (Updates 207 and 208 were pushed without one):** `cobfmt` printed a file's later syntax errors (D-0323)
without its `does not parse` prefix, so the format unit took a refused conformance case for an unformatted one: it now
reports the first; `impl/tools/cobdoc.py` attached a test after a private item to the exported item before it (report
305 changed `cobc --doc` only), so the guide's std reference showed examples that cannot compile: it follows 305 now;
`impl/tools/validate_c.py` did not classify the fault entry points of report 065 (`cb_fault_arith`, `_c`, `_r`,
`cb_fault_index`, `cb_fault_narrow`, `cb_fault_op`, `cb_fault_range`, `cb_fault_to_int`) or `cb_call_self_fn` and
`cb_fn_moved_out`; 75 programs of `impl/tests/programs` were not in `cobfmt`'s form (a statement after a block's `}` on
its own line) and are now, with the 42 expectations whose locations moved; 13 more expectations, the book's example
outputs and the chapters' copies of them, and one case of `diagnostic_forms_bugs194.case` carried the fault texts and
carets of 065, 252, 254, 258 and 229. `stress/round6/kvlog.cb` matched `FileError` without the variants Updates 207 and
208 added and now has a `_` arm. The perf guide's `e01_confined` (+14.9% GCC, +23.8% Clang instructions), `e12_elements`
(+11-17%) and `e05_take_front` (+4-6%) rose in Update 207 with 065's fault values: a checked `+` keeps its operand in a
register for the fault path, one more instruction per element (a `mov` before the `add`); time is unchanged (0.455 s
both), as for `sieve` in Update 207; the baseline is the new count (`stress/gate/store/baselines/perfguide.json`, the
old one kept as `.before-update209`). `impl/tests/checks/store_env.py` names `COBALTC_CACHE` and `COBALTC_CACHE_DIR` (the build cache on or off,
and where it is) as changing no result.

## What is left

- After an edit a large program's C is compiled again whole, on one core by default (059): an object cache of
  `std`'s C is the cache D-0277 removed, left for the owner (D-0338 §3).
- Clang's passes over a large text `match`'s arms stay superlinear (073).
- `match` scrutinees other than integers and `bool` keep the old end of a reference (114).
- A capturing closure called through a shared reference is checked at each call (148, D-0218).
- A variadic C function declared without `...` and not in the table is a fixed prototype (228); deprecating the
  table form is the owner's.
- A block declaring bindings before a `String` tail, where a view is expected, keeps the old message (229).
- Other computed range bounds may overlap (311).
- Under `--all-checks` an `Rc` chain still nests; `Arc` chains and user types too (337).
- SIMD lanes and atomic `fetch_add` overflow name no values (065).
- `cobmodel`: a program outside the fragment is often refused with the parser's message in the model's own token
  form, and `--witness` prints the machine's debug forms; `check` exits 0 whatever it lists, and `compare`'s
  header-stale is advisory (documented); 19 sample headers are header-stale, as at the gate of 2026-11-21.

## Verification

- Each report's reproduction under GCC and Clang (209's against the local PostgreSQL 14, with a temporary
  `scram-sha-256` line in `pg_hba.conf`, restored after); each area's new cases and its existing case files under both
  compilers; edited conformance cases through `impl/conformance/run.py --only` under both; 228's two positive cases
  cross-compiled for Windows and run under wine; 315's lock, the locked map, the `Rc` chain, poisoning, deadlock,
  reentrancy and a moved guard under wine.
- `perf stat -e instructions` before and after for 315, 337, 148, 065's hot path (unchanged), 073 and 311.
- `decided_check` after each area's last build: identical to Update 208's base (502 decided, 11 undecided, 255
  rejected; no line differs).
- `cobmodel` over the full sample (315 programs) under the gate's command: 0 outside, 0 truncated, 6 unobserved, 19
  header-stale (the earlier 19); `statics` against `cobc --check` on all 315: 0 differ.
- `impl/tools/pregate.py` in each area's tree.
- The gate: green (run id 20261124_055844, GCC and Clang; conformance 1518 passed and 5 skipped, the programs 683
  passed, `cbtest rows` 2228 passed and 9 skipped, 0 failed anywhere; `model` unit green, `validate_c` 0 with a
  finding, `replay_model` and `replay_runtime` 0 REFUSED and 0 MISSED). Runs 20261124_011355 (nine units) and
  20261124_040457 (`records`: `store_env`) were red; their fixes are above.
