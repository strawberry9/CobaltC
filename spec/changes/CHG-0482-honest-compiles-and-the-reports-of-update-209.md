# CHG-0482 — Honest compiles, the D-0338 review, and the reports of Update 209

Status: implemented (2026-11-24); Update 210
Affects: `spec/17` 2.23.0, `spec/20` 1.24.0, `spec/21` 4.91.0, `spec/registry/diagnostics.md` 1.76.0; `impl/cobc`
(`driver.cb`, `frontend.cb`), `impl/front` (`typecheck.cb`, `items.cb`, `prelude.cb` generated), `impl/back`
(`gen.cb`, `passes.cb`), `impl/cbrt` (`rt_objs.c`, `rt_init.c`, `include/cbrt.h`, `tools/exports.txt`), `impl/std`
(`http.cb`, `http/server.cb`, `http/client.cb`), `impl/model/src/parse.cb`, `impl/tests/checks/store_env.py`, the
cases under `impl/tests/cases/cobc`; `htmlguide/` (`content_intro.py`, `content_c.py`, `content_runtime.py`; the
pages generated); D-0339.

## Why

The owner's review of D-0338 kept its twelve decisions and asked for four changes (D-0339 §2.1–§2.3): remove the
executable cache and make every build an honest compile, make the variadic table a refusal, and destroy `Rc` chains
iteratively under `--all-checks` too. The fifth round of reports (350–451, on Update 209) was grouped by priority;
the owner asked for the first two groups (memory unsafety, remote crashes, crashes of `cobc`, silent wrong results,
faults left to the run) in the same change, with one gate at the end and no subagents.

## What changes

| Item | Was | Now |
|---|---|---|
| D-0336 §2.19, 393 | `cobc` kept the executables it built (`COBALTC_CACHE`, `COBALTC_CACHE_DIR`); a rebuilt `.o`/`.a` was not seen | no build cache (`exe_cache_*` removed from `driver.cb`, the case `driver_build_cache.case` and the two names in `store_env.py` with it); every build compiles |
| 228 (D-0336 §2.20) | a listed variadic C library function declared without `...` was declared to C with `...` | refused, `diag.extern-variadic-without-ellipsis` (`[Extern-Variadic-Without-Ellipsis]`); the table moved to `typecheck.cb` (`c_variadic_fixed`) |
| 337 (D-0338 §2.10) | under `--all-checks` a long `Rc` chain still nested and exhausted the stack | a field's `Rc` in tail position of an `Rc` destructor waits for it in both forms; the value `Rc::drop`'s `overwrite` consumes is in tail position (`cb_consume`) |
| 392 | a struct field of type `Vec<StringView>` held no loan: the default build read freed memory | a `rawptr<T>` field holds what `T` holds (`ref_info_add`): the program is tracked, and the write faults in both forms |
| 407 | `&b[0]` / `&b.text` of a parameter `from a` leaves out was a probe | a kept use, `diag.result-source-kept` (`pw_place_of`) |
| 439 | `drop(v[i])`, `drop(r.f)` skipped the move check | checked as a move: `diag.move-out-of-field` |
| 408 | `Ok(x)` beside a `fn` argument was not compared with the `T` the other arguments fix | a variant literal argument is checked after the others, as a bare literal |
| 409, 410, 411 | `cobc` asserted "cannot infer type arguments" | `error[diag.type-annotation-needed]` naming the function, the line and what to write (`cannot_infer`) |
| 440, 441, 442 | `DAYS[1] = 29`, `f().x = 1`, `None = o`, `HITS = …`, `drop(HITS)` passed the check (no effect, or a `cobc` assertion) | refused (`assign_root_problem`; `diag.static-assigned`, `[Static-Not-Assigned]`) |
| 443 | `&less` of a function item passed the check and `cobc` asserted | refused with the repair to pass `less` |
| 438 | a non-ASCII byte in a path made `cobc` fault (`push_ascii`); `cobmodel` too on a string | an octal escape in the C (`c_escape`); `cobmodel`'s `debug_str` keeps the character |
| 358 | `1 << s` took `s`'s type (`u32`) | the literal is the expected type, else `i32`, in the checker and the lowering |
| 350, 380 | `spawn(f, x);` as a statement was silent; a thread's `Result` was lost | `diag.handle-discarded` (warning); `diag.result-discarded` when the thread gives a `Result` |
| 374 | `exit(0)` in a test ended the run with status 0 and no summary | the runner calls `cb_unittest_runner`; the test is FAILED, the reason printed, status 1 |
| 356 | `v[0] = Res { … }` passed the check and faulted every run | refused statically, as a field's overwrite |
| 423 | 17 hex digits of chunk size faulted the server or client with an overflow | `BadMessage` |
| 424 | a second `respond` for one request desynced keep-alive | `Err(BadMessage)`, nothing written |
| 425 | a Latin-1 header byte made the message malformed | the line read as ISO-8859-1 |
| 430 | an empty line before a request line lost the request | ignored once |
| 431 | the client never saw an early 413 | the closing connection drains up to 64 MiB after a failed request |
| 432 | a slow sender held a connection by keeping each read inside the timeout | the whole head within `timeout_ms` of its first byte |
| new cases | — | `reports_update210.case`; `extern_variadic_c_library_call.case` and `extern_variadic_strfmon.case` rewritten for the refusal |

## What is left

- 384: left as the run's (`conf.literal-expression-overflow-dynamic`, D-0037); 395 and 414 (through a binding) left as the run's (D-0284, `conf.result-through-if-checked-at-run`).
- 381, 382, 390, 394, 412, 413, 418: accepted by `--check`, caught by the run-time record (safe).
- 392's programs are tracked and fault, rather than refused statically.
- 409–411: `--check` still accepts them; the lowering now stops with a clear error.
- 380's unjoined handle ending at its scope's end; 399 (`never` in signatures) for the owner; reports of priority
  3–5.

## Verification

The first gate (`20261124_133551`) was red: the fold of 384 contradicted D-0037 and slowed a deep expression; the
395/414 check contradicted D-0284 (it was withdrawn); two cases recorded the old behaviour (the `--all-checks` `Rc`
chain); `validate_c.py`'s table lacked `cb_unittest_runner`. Each was fixed and the gate run again. Each report's repro (`private/BUGS/NNN-*/repro/run.sh`) rerun on the new build; `reports_update210.case`, the
variadic cases and `runtime_update209.case` under GCC and Clang; one gate-profile run before the push.
