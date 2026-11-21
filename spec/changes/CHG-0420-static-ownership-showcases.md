# CHG-0420 — The static ownership plan's last target: no runtime tracking left in any decided showcase

Status: implemented (2026-11-09), D-0315
Affects: `spec/conformance.md` 3.302.0 (five rows); `cbrt` (`cb_notes_drop`, `cb_join_plain`; the handle destructor ends a result with no object; `cbrt.h`); `cobc`
(`lower/`: `analysis_strings` — `noted_type`, `objectless_type_in`, the candidate walk; `stmts`, `exprs`, `places`,
`intrinsics`, `fresh`, `flights`, `frames`, `closures`, `functions`, `matching`, `operators`, `passes`, `program`,
`user_calls`, `values`, `vec_calls`; `lower.rs`) and its twins in `cbc` (`back/`: `gen`, `state`, `natives`, `fresh3`,
`flights`, `passes`, `strings4`, `vec_scan`, `header`); `coby` (`interp/intrinsics_threads.rs`, `interp/destroy.rs`);
`tools/validate_c.py`. No rule changes. No program's outcome changes under `cobc`; under `coby`, the two
programs of the last item below now end as the rules (and `cobc`) say.

## What changes

`CHG-0419` left 217 lines of `--report-tracking` in 36 of the 84 decided showcases. All are gone. Each item below
holds in a decided program only, in both toolchains.

1. **`std`'s resource types are noted values** (`File`, sockets, HTTP values, processes). `noted_type` no longer
   leaves out `std::` types whose destructors are their own.
2. **More runtime calls record nothing in a scope.** The decided list now includes `cb_exit`, `cb_os_random`,
   `cb_path_op`, `cb_proc_op`, `cb_net_op`, `cb_task_op`, `cb_release`, `cb_reallocate_plain`, `cb_fn_shared` and
   `cb_fn_mark_pure`. It also includes the thread calls (`cb_env_alloc`, `cb_spawn`, `cb_lend`, `cb_lend_fn`,
   `cb_join_plain`, `cb_join_status`, `cb_cancel`, `cb_sleep_ns`) and the lock calls (`cb_lock`, `cb_lock_pathless`,
   `cb_lock_bare`, `cb_unlock_bare`). A spawn's argument block and trampoline (`cb_env_N`, `cb_thr_N`) are names,
   not calls.
3. **Closures.**
   - A closure with no captures is a `static cb_fnbox`.
   - A closure given only to `fn` parameters that do not escape is boxed on the stack.
     `fn_param_noescape` decides this, transitively.
   - A closure type is judged by its captures, and a noted value may hold `fn` parts.
   - Closure bodies return raw.
   - The candidate walk reads a call through a candidate, and a candidate passed to a `fn` parameter.
4. **Pattern binders.** A `match` arm's binder or an `if`-pattern's binder whose type has a destructor of its own is
   noted, with a sub-frame after it (`noted_arm_binder`). An arm whose body yields its binders, or a literal, takes
   the scrutinee with no object.
5. **Statement temporaries whose type has a destructor of its own** are noted.
   - Each is registered in the table and ends where its statement ends.
   - A value read in part (`make().f`) works the same way.
   - Where it is taken into an argument, a field, an element or a spawn's block, its registration leaves first:
     `cb_notes_drop`, which removes one registration out of order.
   - An argument or field with no object waits as such a temporary only if a later argument could fault
     (`noted_pend`; `harmless` decides). This replaces `CHG-0419`'s "last argument only".
6. **`let` arming.**
   - A `let` with no declared type arms its value by the value's probed type.
   - An intrinsic's result (`lock`, `spawn`, `Mutex::new`) is armed by the declared type, or else by the probe.
   - `String::copy` and the empty constructors of `std`'s containers are armed.
   - A `let` with no value is a candidate: bound with no object, its moved flag set until the first assignment.
   - An array literal is armed, its elements with no object.
7. **Threads, mutexes, guards.**
   - A handle and a guard are noted.
   - A mutex is judged by its interior, an array by its elements.
   - `spawn` takes its by-value arguments with no object. It gives its handle with none where its consumer takes it
     last.
   - A thread result whose end frees memory alone comes back as its bytes. `join` takes it with `cb_join_plain`,
     which stamps nothing.
   - `join` of a handle with no object reads its moved flag.
   - `lock` gives a guard with no object where its consumer takes it last. A guard temporary (`*lock(&m)`) is a
     noted statement temporary.
   - `Mutex::new` takes its value with no object.
   - The candidate walk follows `spawn`, `join`, `try_join`, `lock` and `*x`.
8. **Smaller cases.**
   - A borrow through fields and indexes of an objectless root (`objectless_root_through`).
   - `Channel::send` of a fresh `String`, its result discarded.
   - No `cb_end_binding` of token 0.
9. **`coby`.**
   - **`spawn`'s arguments are the statement's temporaries until every argument is evaluated, as a call's are
     (`CHG-0339`).** Before, a fault in a later argument left the values built for, or moved into, the earlier
     arguments undestroyed: `conf.fault-ends-pending-spawn-argument`.
   - **A binding of `[Let-Uninit]` that a fault leaves before its first write ends with nothing in it.** Before,
     coby ran the type's destructor on the placeholder. For a `Vec` that destructor faulted mid-unwind and left its
     reference live, so the thread that contained the fault later faulted `diag.aliasing-conflict` on an unrelated
     value: `conf.unassigned-let-takes-first-value`.

   `cobc` already did both.
10. **Step 8, as the plan says: the design on paper, one program prototyped, then the decision.** The prototype is a
    loop of 50 million calls, each binding two locals with a destructor of their own; the cycles per call are:

    | variant | per call |
    |---|---|
    | as generated | 284 |
    | without the tables | 207 |
    | with the destructor called directly | 79 |
    | that, with the registrations as inline stores | 13 |
    | a cleanup attribute with `-fexceptions` | 2 |

    So the cost lies in two runtime calls, not in having a table: the generic end of a value, and a registration made
    by a call. Both can be removed portably. What O3 would add beyond that, about 11 cycles a call, needs:
    - `-fexceptions` on every compiler;
    - a forced unwind with a personality of its own on 64-bit Windows;
    - a cleanup for every frame push;
    - a new failure for a destructor that faults while unwinding.

    O3 is not adopted. The direct drop and the inline registration are follow-up work for the noted values.

## Measured

Measured on this source, with `cobc`, `cbc` and `cobfmt-cb` all built from it:

- **Report.** `--report-tracking` lists nothing for any of the 84 decided showcases, the ten programs of the speed
  test, or the nine benchmarks. No decided lowering falls back to the first lowering (`cobc -v`). `cbc` itself is a
  decided program with nothing listed; the `cobc` before `CHG-0420`'s last fix (item 6, the declared type) fell back
  to the first lowering on it.
- **Speed test** (`timeit.py`, five runs). `cbc` takes 0.27–1.47 times `cobc`'s time per program, 1.26 times over all
  ten (`CHG-0419`: 1.28). The largest ratios are the 13,000-line program (1.47), the type inferencer (1.45) and the
  3,900-line program (1.37).
- **Formatter** (`timefmt.py`). `cobfmt-cb` takes 0.03–0.66 times `cobfmt`'s time on every file, with identical text.
- **Runtime share.** The runtime is 9.8% of `cbc`'s cycles on the Lisp interpreter (`CHG-0419`: 10.8%): 5.9% in
  `cbrt`, 3.9% in `cbrt.h`'s helpers.
- **Correctness.** The 90 showcases print the same under `coby` and `cobc`, with gcc and with clang. `cbc`'s C is
  identical to `cobc`'s on the showcases, the speed test, the benchmarks and the new cases (116 files). The conformance
  cases and the speed test's programs behave alike under the sanitizers. One leak the sanitizers found is fixed in
  the runtime: an unjoined handle whose thread's result came back with no object now ends that result by its
  type's drop (the array of handles in `conf.thread-values-cross-without-objects`).

## Conformance

- `conf.noted-payload-binders-end-in-order`
- `conf.noted-temporaries-end-with-statement`
- `conf.thread-values-cross-without-objects`
- `conf.unassigned-let-takes-first-value`
- `conf.fault-ends-pending-spawn-argument`

Each gives the same results under `coby` and `cobc` (gcc and clang) and under the sanitizers. The first four are
decided programs, and their report lists nothing.

## Not done

**A program that spawns a value whose type has a destructor of its own is not decided.** The static pass reports
"a thread is lent a reference no fact describes". Two more forms keep the run-time record:
- an array of handles to threads that were lent references;
- `join(spawn(f, &m, …))` with the handle never held by the frame.

None of the showcases does any of these.
