# CHG-0480 — The bug reports of Update 207

Status: implemented (2026-11-23); Update 208
Affects: `spec/05` 1.10.0, `spec/06` 1.24.0, `spec/12` 1.33.0, `spec/14` 1.52.0, `spec/15` 1.23.0, `spec/16` 1.40.0, `spec/18`
1.13.0, `spec/19` 1.24.0, `spec/21` 4.89.0, `spec/22` 2.68.0, `spec/OVERVIEW.md` 1.3.4, `spec/registry/diagnostics.md` 1.74.0,
`spec/conformance.md` 3.325.0; `impl/front` (`typecheck.cb`, `parser.cb`, `lexer.cb`, `modres.cb`, `hints.cb`,
`prelude.cb` generated), `impl/back` (`gen.cb`), `impl/cobc` (`driver.cb`, `doc.cb`, `summary.cb`, `host.cb`, `host.c`,
`build_info.cb` generated), `impl/cobfmt` (`width.cb`), `impl/cbrt` (`rt_init.c`, `rt_threads.c`, `rt_lock.c`,
`rt_refs.c`, `rt_objs.c`, `outbuf.c`, `fmt.c`, `diagnostics.c`, `fileio.c`, `procio.c`, `netio.c`, `plat_posix.c`,
`plat_win.c`, `plat_io.h`, `io.h`, `rt.h`, `diag_table.h` generated), `impl/std` (`collections.cb`, `std.cb`, `text.cb`,
`math.cb`, `builtins.md`, `stats.cb`, `time.cb`, `env.cb`, `fs.cb`, `io.cb`, `process.cb`, `encoding/hex.cb`,
`encoding/json.cb`, `encoding/toml.cb`, `http/client.cb`, `http/server.cb`, `database/postgres.cb`),
`impl/tools/cobdoc.py`, `impl/tests/checks/doc_matches_cobdoc.py`, `impl/tests/foreign_forms/expected.txt`; the cases
under `impl/conformance`, `impl/tests/cases`, `impl/tests/programs`, `impl/tests/rows`; `book/` (ch02, ch04, ch08,
ch11, ch13, appendices A and B), `htmlguide/` (`content_a.py`, `content_b.py`, `content_c.py`, `content_intro.py`,
`content_perf.py`; `index.html` and `Appendix_06.html` generated); `CARD.md`; D-0337.

## Why

An agent testing Update 207 (`156c0e8`) filed 110 reports (240–349), each with a reproduction (`private/BUGS/`,
outside the repository). The owner asked for every one to be resolved in one push, with focused tests for what each
changed and one bench run at the end, in place of the gate's full profiles. Five agents did the work, by area
(diagnostics, the static pass, generics and code generation, the runtime and tools, `std`). D-0337 has the choices
that changed a rule, added a warning or added to `std`; this record has every report.

## What changes

The number is the report's.

**The checker left to the run what it can refuse:**

| # | Cause | Now |
|---|---|---|
| 291 | `[Ref-Escape-Rejected]` for a call's result looked only at results that are references or hold one by `holds_ref` (a `Vec<StringView>` is neither), so a function returning a `Vec` of views of a local `String` passed `--check` and faulted `diag.destroy-while-aliased` on every run | a call whose result may hold references refers to its sources, and a returned value keeping references whose facts reach an owner of the frame is `diag.reference-escapes-scope` (`check_holder_escape_to_caller`; `std` bodies and raw-pointer locals skipped); conformance `vec_of_refs_result_outlives_argument_checked_at_run` is now refused statically (D-0337 §2.9) |
| 313 | a `?` fact held by a thread's handle (or a `Vec` of them), lent on an earlier round of the loop, was left to the run, whose outcome depended on whether that thread had ended (`--explore 20`: 17 faults, 3 successes) | refuted statically when the holder is declared outside a loop the access is in and neither path has a computed index or range, saying it depends on timing (`spec/14` §6 `¬clash`); a handle lending on some paths only stays the run's; conformance `ref_lent_in_loop_handle_kept_written_rejected` is now refused statically (D-0337 §2.9) |
| 318 | the move-out-through-reference refusals (report 108) knew `ref` bindings only, so `Vec<String> batch = *g;` and `String n = g.name;` through a lock guard faulted at the run | refused statically, naming `replace(&mut *g, Vec::new())`, `replace(&mut g.name, v)` and `clone(&*g)` (D-0337 §2.9) |
| 345 | the generic pass skipped `[T-Bound-Literal]`, so `T s = 0;` with an unbounded `T` ran for `i32` and was refused only at a float instantiation | refused in the body, naming the `number` (or `integer`) bound (D-0337 §2.6) |

**The default build and `--all-checks` disagreed:**

| # | Cause | Now |
|---|---|---|
| 265 | the run ended a block's quiet references and reference holders only at the frame's pop, after the text they view (declared later, destroyed first), so a view declared before its text faulted `diag.destroy-while-aliased` under `--all-checks` | `early_end_plan_with` also ends a binding last mentioned by the block's last statement after it, and holders of references whose end frees memory alone (`Vec<StringView>`, `Vec<ref<String, shared>>`) end after the block's last statement |
| 266 | copies of a shareable `move` closure called at once from two threads faulted `diag.aliasing-conflict` in the default build: `lower_closure_body` typed `self` exclusive, and the runtime's `cb_call_self_fn` light path borrowed the box exclusively whatever `fn_shareable` said | `self` is shared for a closure that writes no capture and whose captures own no resource and hold no reference or `fn` (D-0336 §2.17), and the light path takes a shareable closure's `self` shared (`rt_refs.c`) |
| 268 | report 114's end after an `if` condition looked only at the first condition, so a reference last used in an `else if` condition was kept to the statement's end at the run | in an `else if` chain a binding named only in conditions ends at the start of each then-branch before the last condition naming it, and after that condition; `chain_end_ok` checks that no later branch names it |
| 290 | `slice_len(&v)` lowered by forming a slice (a shared borrow with an object) that lived to the statement's end, so `v[slice_len(&v) - 1] = 9;` faulted under `--all-checks` | `slice_len` of a `Vec` or array reads its length directly with a shared borrow check, an array named whole as its constant length |
| 319 | `cb_exit` folded the exiting thread's frames with the solitary check while `v` was lent to a running thread, so `exit(64)` faulted `diag.destroy-while-aliased` under `--all-checks` | during an `exit`'s unwind with threads running, a value another thread references is left as it is (`spec/15` `[Terminate-Exit]`, D-0337 §2.14); both forms exit 64 |

**The checker refused correct programs:**

| # | Cause | Now |
|---|---|---|
| 242 | `probe<StringView>` is `ref<StringView, shared>`, and a `str` did not fit, so `Vec::contains(&views, "fig")` was refused (caret on `Vec`, the parameter's name as the argument's) | a `str` there is `&StringView::of(t)`, borrowed for the call (`Rewrite::ProbeViewRef`), outside generic bodies (`spec/12` rule.type.probe, D-0337 §2.8) |
| 267 | a shared view copied out of a field through a reference parameter (`StringView r = c.rest`) took a fact on the field (report 075's rule), so `c.rest = &r[1..$];` conflicted | the copy refers to what `*c` keeps (`**c`, D-0290) when the referent keeps no exclusive reference |
| 273 | report 056's guard against folding a local's known value covered `if`, `&&`, `\|\|` and `while`, not `match`, so `_ : total / count` was `diag.div-by-zero` | `check_match` guards its arms with the scrutinee's locals |
| 280 | the lazy `foreach` rewrite (D-0287) turned `StringView::split(line, sep)` into `Parts::split(StringView, str)`, refusing a `StringView` separator | the rewrite is made only for a separator written as a text literal; another one makes the `Vec` (`spec/14` rule.control.foreach-parts, D-0337 §2.13) |
| 288 | the marker pass judged `Vec<T>` not quiet, so `s.items = Vec::new();` in generic code was refused even with `T: plain` | a type naming a type parameter is judged in each instantiation, as a bare `T` was (`spec/05` quiet(τ)); an instantiation with a destructor type is still refused |
| 292 | a binder's borrow ended after the arm's last top-level statement that mentions it, so "copy, then advance" was refused inside a nested `if` | a binding last mentioned by an `if` that one branch names ends inside that branch after its last mention there, and at the start of the other (`nest_into_branches`, `Facts.nested_ends`; D-0337 §2.10) |
| 293 | `&NAMES` of a `const array<str, N>` was not static data, so it was a temporary, which `slice_len`'s argument never allowed (`count(&NAMES)` passed only inside `printf`, through a leaked flag) | a const array literal of text literals is static data when borrowed, and `slice_len(&NAMES)` gives its constant length |
| 307 | `f(&Vec::new())` at a slice parameter: the `[Coerce-Slice]` rewrite lost the expected `Vec<τ>` in round two | the temporary call gets the expected `Vec<τ>`, and the inferred type argument is written on the call (`Rewrite::CallTypeArgs`) |
| 320 | `spawn([]() { … })` was refused: "a spawned thread needs a `move` closure" | a closure literal with an empty capture list is accepted (`spec/19` `rule.conc.spawn`, D-0337 §2.11) |
| 338 | `[T-Bound-Literal]` was not applied to call arguments in generic bodies: `max(x, 0)`, `Vec::push(v, 0)`, `Vec::filled(n, 0)` were refused at `T = f64` | a deferred literal whose parameter the other arguments or the expected result fix as `T` takes the `number` marker (`bound_literal_hint`), so each instantiation types it at `T` (`spec/12`, D-0337 §2.6) |
| 339 | `o == Some("a")` with `o : Option<String>` took the `[Cmp-Key]` branch, which checked both operands with no expected type | a variant or struct literal operand there is checked against the place's type; with the literal on the left the key place is borrowed, not read |
| 340 | the declared type did not reach a literal nested in a generic call's argument (`Box<Option<u64>> c = Box::new(Some(3));`) | a variant or struct literal argument whose parameter the other arguments leave open takes it from the expected result |
| 341 | report 049's untyped-`clone` exemption covered `clone(&x)` only | also `&x.f`, `&x[i]`, `&(*x).f` rooted at a local; the instantiations still check |

**Accepted programs that `cobc` or the C compiler could not build:**

| # | Cause | Now |
|---|---|---|
| 264 | `view_ops` is keyed by expression id, shared by every instantiation of a generic body, so the `StringView` instantiation's `VEq` was lowered in the `u32` and `String` ones (invalid C, or a `cobc` assertion) | in an instantiated body the view operation is re-derived from the operands' types (`instance_view_op`) |
| 336 | the same, for `<` and `>` (`VLess`) | the same fix |
| 282 | `key_less` lowered a `str` probe by value with the plain `<` (`Vec::binary_search` on a `Vec<str>`: invalid C) | compared by bytes (`lower_key_bytes`) |
| 281 | after resolution a type was known by its bare name, so a program's `struct K` and `std`'s type parameter `K` were one type (a `cobc` assertion, or an error inside `<std>/collections.cb`) | a type parameter named like a program's top-level type is renamed in its declaration (`K'`) (`spec/12` rule.type.kind, D-0337 §2.7) |
| 289 | `lower_place` of `*a` on a direct reference binding called `auto_deref`, which reads through every level, so `*a` of `ref<ref<T>>` was `**a` | one level |
| 335 | a generic function passed by name to a generic callee's `fn` parameter was probed with no expected type, and `lower_fn_item` asserted inside the probe | the probe fails, and the argument is lowered at its parameter's type |

**The language's rules (D-0337):**

| # | Cause | Now |
|---|---|---|
| 257 | the grammar said `(arm ',')+`, but the parser accepted a `match` without a comma after its last arm, as 43 files of the repository write | the grammar follows the compiler, `arm (',' arm)* ','?` (`spec/16` §4, `spec/22`); book and guide say the last comma may be left out (§2.4) |
| 258 | a C `switch`'s `break;` at the end of a `match` arm left the enclosing loop silently | new warning `diag.match-arm-break` for a bare literal arm that does work and ends with `break;` (`spec/14`, §2.1) |
| 259 | `0755` was silently the decimal 755 (C: 493) | a decimal literal of two or more digits starting with `0` is ill-formed; the message names `755` and `0o755` (`spec/22`; D-0035 superseded; §2.2) |
| 260 | `"n=" + n` was `diag.read-of-resource`, and `--explain` described copying a resource; `str + str` had no join hint | new `diag.text-join`, whose `--explain` describes joining text (`spec/06`, §2.5) |
| 302 | `_ = spawn(f, x);` joined the thread on that line, silently | new warning `diag.handle-discarded` (`spec/19` type.handle, §2.1) |
| 329 | a `#!/usr/bin/env -S cobc --run` first line was refused as a preprocessor line | a `#!` first line is a comment (`spec/22`); elsewhere `#` is refused, saying a `#!` line is allowed only first (§2.3) |
| 346 | `x == v` between two `ref<T, shared>` compared identity, silently, in a `T: eq` function | new warning `diag.reference-identity`, naming `*x == *v` and `rawptr_of(x) == rawptr_of(v)` (`spec/12`, §2.1) |

**Diagnostics:**

| # | Cause | Now |
|---|---|---|
| 241 | a probe mismatch in a generic body got advice for one instantiation (`*k`, or "view it: `&x`"), and following one broke the other | every instantiation names `HashMap::contains_ref(…, k)` (or the callee's `_ref` sibling) |
| 243 | `import model;` in a file checked alone said "names nothing" and advised `import std;` | "a file checked or tested alone is the program's root": check the program that mounts it (`cobc --check main.cb`) |
| 244 | a `void` body in a `Result<void, E>` function or `main : u8` was told to drop a `;` | "end the body with `Ok(())`" / "with `0`"; the `;` hint only after a value |
| 245 | `u64 n = width * height;` (`u32`) was advised `widen<u64>(…)` of the product, which still overflows | the operands are widened: `widen<u64>(width) * widen<u64>(height)`, saying why |
| 246 | method calls on text got no hint or the wrong function, with a placeholder `x` | `s.len()` → `StringView::len(&s)`, `s.push_str(t)` → `String::append(&mut s, t)`, `s.equals(t)` → `s == t`, `s.bytes()` → `String::as_bytes(&s)`, with the receiver's own text |
| 247 | when a loop's condition and its body both failed, only the body's error was shown | the condition's (earlier) error is reported (D-0323); a signed counter compared with a length is told to be `usize` from the start |
| 248 | a plain value where an `Option` or `Result` is wanted got no wrapping hint | "wrap it, `Some(i)`" (or `Ok`, `Err`) at a declaration, a `return` and a body's last value |
| 249 | `?` on a plain value or on a `Result` in an `Option` function gave generic messages | "`?` takes a `Result` or an `Option`, and this is a `usize`"; "turn the `Result` into an `Option` first, `Result::ok(r)?`" |
| 250 | `printf(&name)`, `{}` with arguments and `%s` of an integer got unrelated repairs | `printf("%s", &name)`; "write `%v` for each `{}`"; "a number prints with `%d` (or `%v`)" |
| 251 | the body-result mismatch's caret was on the last token, with no note for an `if` without `else` | it points at the body's last expression; "this `if` has no `else`"; a `match` says what each arm must give |
| 253 | advice in a generic body's instantiation named the concrete type (`String::clone(&xs[0])`), which does not compile there | the generic forms (`clone(&xs[0])`, "the bound `T: clone` gives `clone`"), and following the old advice is explained |
| 254 | the index fault's repair named checked accessors that do not exist; `Vec::get`, `is_empty`, `last`, `v.first()` got only "nothing named" | each is given the CobaltC form; the repair names checking first or `Vec::pop` (registry; `diag_table.h` regenerated) |
| 255 | a read while a writing closure lived was told to "use `shared` on both sides" | "the closure `inc` writes `count`, and borrows it exclusively for as long as `inc` exists": put it in its own block, or `drop(inc);` |
| 256 | `p.x = 1;` on an unassigned struct was reported as a read | "a part is set only in a value that exists; a struct is given its value whole", with the literal form |
| 261 | more day-one habits got bare or wrong messages | `for (x in &v)`, `n--`/`++n`, C's literal suffixes, a `fn` inside a `struct`, `Vec<i32> v = [1, 2, 3]`, `T::clone(x)`, Rust's `Copy` (`plain`, now in the bound list), a collection used after `foreach (n in names)`, `*HashMap::get(…)` and `??` each get the CobaltC form |
| 269 | `diag.comparison-constant` fired in a generic body because one instantiation is unsigned | not given for an operand of a type parameter's type in an instantiation (`spec/06`) |
| 270 | the warning for `stock - ordered >= 0` printed "`the value`" as code and gave count-down advice | names the expression and `stock >= ordered` |
| 271 | the chained-comparison message (report 192) invented operand names | it uses the operands' own text (`r.lo <= x && x < r.hi`), `…` for one it cannot show |
| 272 | a missing `;` before a line starting with a number type got the literal-suffix hint | that hint only when the type name is on the literal's line and not followed by a name |
| 276 | the near-miss listing suggested a `--doc --find` with every word, which finds nothing | one `--doc --find WORD` per part of the name that a listed item has |
| 283 | a missing `from` clause on a function with two reference parameters was reported at its caller, with advice that does not fix it | `[Call-Multi-Ref-Return-Rejected]` is checked before the bodies, so it is reported at the function (D-0337 §2.20) |
| 294 | `if (Savings { rate_bp } = k)` got struct-literal advice leading to another error | "a variant with fields is written with its enum's name, in a pattern too: `Kind::Savings { rate_bp }`" (D-0324) |
| 295 | `&e.hour` for a value parameter was told to write `*` (`*f(…)`), caret on the callee | "pass the value itself: `e.hour`, without `&`", caret on the argument |
| 297 | an integer literal to an `f64` parameter was always told "`3.0`", caret on the callee | the literal's own digits (`7.0`), caret on the argument |
| 305 | `--doc` attached a private function's unit test to the exported item before it | only when no private item came between (`summary.cb`) |
| 306 | the message advised `PgRow::clone`, which `--doc` said does not exist, and named the argument's type wrongly | it names `ref<PgRow, shared>`, and `--doc` documents a derived `T::clone` (D-0116) |
| 308 | `b""` got the generic repair | names `&Vec::new<u8>()` and `Vec::new<u8>()` |
| 343 | `==` on a struct of `String` and `f64` said it held "a `Vec`, a `Box`, a hash table, a handle" | it names the real reason: a struct holding text compares as a key type (D-0271), and `price` is `f64`; the rule is unchanged |
| 344 | a moved `move` closure was advised `clone(&fmt)`, which a function value refuses | names writing it without `move`, a new closure for each use, or using it before the move |
| 347 | `o == None` on an `Option` of a function, and `f == g`, said "the types do not match" | "a function value has no `==` (`[Eq-Fn]`)", naming `Option::is_none` or `match` |
| 348 | a closure capture conflict named a field the closure does not use (`*s.items`) | "captures `s` whole …: a closure captures whole variables", with `auto min = s.min;` and `[min]` |
| 349 | `T::speak(x)` got "nothing named" and `import` advice | "`T` is a type parameter, which has no functions of its own (there are no traits)": pass a `fn` parameter; `T::clone` → `clone(x)`, `T::eq`/`cmp` → `==`/`<` |

**The runtime: faults, threads and standard streams:**

| # | Cause | Now |
|---|---|---|
| 252 | a run-time fault's caret was at the statement's start, and an unsigned `0 - 1` was advised `wrapping_` | the caret is under the operation when the fault names it and the line has it once (`diagnostics.c` `op_column`); the count-down note `while (i > 0) { i -= 1; … }` (registry) |
| 279 | `fmt.c`'s `fast()` took `%u`'s sign from `is_signed` when another specifier made the format general | `%u` is never signed there |
| 323 | the flush before a read of standard input or before `eprintf` ignored a broken pipe, and a `printf` after it only filled the buffer, so a filter piped into `head` never ended | `outbuf_write` refuses once the reader has gone, `outbuf_flush` reports it with nothing pending, and those flushes go through `[Print-Reader-Gone]` (D-0337 §2.17) |
| 317 | `cb_terminate_ok` waited for running threads before writing standard output's buffer | the buffer is written before the wait (`[Print-Buffer]`) |
| 316 | a thread blocked forever at the end of `main` hung the program and lost `main`'s output | the output is written (317); the wait stays (D-0336 §2.16, D-0337 §2.14); `spec/15` `[Terminate-Ok]` says so |
| 312 | `join` consumed the handle before waiting, so when `cancel` of a supervisor ended that wait nothing cancelled the child, and `main` waited out its sleep (default build) | a thread being joined is marked (`TState.joining`), and a contained fault cancels and waits for it (`spec/18` §1); 0.10 s in both forms |
| 277 | a destructor faulting during a contained thread's unwind ended the program and hid the thread's own fault | both are reported, the thread's first (`CUNWIND_FIRST`); an identical repeat is reported once (`spec/18` §1, D-0337 §2.16) |
| 321 | `main` waiting alone on a semaphore nobody releases was "these threads would wait for each other forever" | a lone waiter at the end of a chain of waits is "`main` waits here forever: no other thread can end this wait", decided when the deadlock is found; a cycle keeps the old text |
| 332 | a thread blocked in `read_line` could not be cancelled, so `main`'s end waited for the input to end | a read of standard input that waits is a cancellation point (`os_stdin_wait`, POSIX `poll` and Windows; `spec/19`, D-0337 §2.14); the program ends 0.30 s after start |
| 331 | under `watch_interrupts` a program blocked in `read_line` could not be stopped by signals | an interrupt arriving while the previous one is unseen ends the program as without `watch_interrupts` (SIGINT: 130) (D-0337 §2.15) |
| 300 | a worker blocked in a socket read was not woken by SIGTERM once `main` had seen the flag (29 s shutdown) | a socket wait is cut short by an interrupt that arrives during it (`netio.c` `at_start`); 0.105 s |
| 337 | a stack exhausted in a destructor (a long `Rc` chain) unwound and ended again values under destruction (`0 - 1` of a count; 174,762 reports under `--all-checks`) | the program ends with the one `diag.stack-exhausted`, files and standard output flushed, no destructor (`DROPS_RUNNING`, `fault_without_unwind`; `spec/18` §1, D-0337 §2.16) |

**Time:**

| # | Cause | Now |
|---|---|---|
| 324 | standard output was flushed before every `read_line`, even with the next line buffered (100,001 writes for 100,000 lines) | flushed before a read only when it waits for the OS, or always when standard output is a terminal (`[Print-Buffer]`); 97 writes; 3M lines 6.08 s → 0.51 s (Python 1.50 s) |
| 315 | an uncontended `lock` took the stripe twice for the reentrancy check and the claim; contended claims all waited in the book | the check and the claim share one hold (`claim_checked`), the state cells' offset is cached per thread, a contended claim spins 64 rounds under the stripe first; `lock_only` 1.228e9 → 1.006e9 instructions (−18%), four threads 6.8 s → 0.6–3.1 s (noisy machine); C's 10 ns not reached (D-0337 §3) |
| 301 | `Json::set` and `get` scan an object's members (80,000 keys: 17.9 s to build) | new `Json::push_member` and `Json::key_index` (D-0337 §2.19); 80,000 keys 43 + 79 ms, linear |
| 311 | divide-and-conquer `spawn` on `&mut s[0..mid]` and `&mut s[mid..n]` was 20–30× slower than sequential: the ranges' bounds are not constant, so the slice is tracked and every `s[i]` checked | documented limit: the guide's performance chapter names `slice_parts(s, 2)` (decided) and `--report-tracking` (D-0337 §3) |

**`cobc` and `cobfmt`:**

| # | Cause | Now |
|---|---|---|
| 262 | the executable cache hashed only an `extern` C file's own bytes, so a header edit or a changed `CPATH` served the old binary | each C file as `cc <flags> -E` preprocesses it, and the compiler's environment, are in the key; a relative `COBALTC_CACHE_DIR` is resolved (D-0337 §2.18) |
| 263 | the compiler's identity was its `--version` line, so an edited `--cc` wrapper was not seen | the key holds the compiler file's bytes; `-march=native` is not cached |
| 274 | `--explore` cut each outcome to 8 stdout and 3 stderr lines with no marker | "… N more lines"; a later outcome shows its lines from the first that differs, or "(as the first outcome's)"; help and guide say so |
| 275 | the replay line dropped `--unittest` | it carries `--unittest`, `--unittest-std` and `--unittest-filter TEXT` |
| 314 | `--explore` hung forever when one schedule never ended | each run has a wall-clock limit, max(10 s, 10 × the slowest run that ended); a watchdog kills a run past it (`cobc_host_kill`), shown with its replay seed (D-0337 §2.18) |
| 309 | `cobfmt --width` broke a short call and left a long `match` on one line | a line holding a `match` is broken one arm per line first (`break_match`) |
| 278 | the book said `cobfmt` puts every brace on its own line; `spec/15` still said "no other thread is live" at `[Terminate-Ok]` | documented: book ch02 describes the one-line forms and `--width`; `[Terminate-Ok]` and `[Terminate-Exit]` describe threads still running, a faulting destructor and a value lent to another thread |

**`std`:**

| # | Cause | Now |
|---|---|---|
| 240 | `from_iso` and `unix_ms_from_iso` read only `…Z`, so `to_iso_local`'s text did not read back | `T`/`t`/space, `Z`/`z`, `±HH:MM`/`±HH`/`±HHMM` converted to UTC, no zone as written (`[From-Iso]`, D-0337 §2.19) |
| 304 | PostgreSQL's `timestamptz` text could not be read, and `PgRow` had no timestamp getter | read as 240; new `PgRow::get_datetime` (`[Pg-Cell]`) |
| 284 | `--doc powf` said `pow` is exact for an integer exponent, which is false for a float base | documented: exact for integer types, `powf` for accuracy (D-0337 §2.20) |
| 285 | `median` and `percentile` sorted NaNs last and used them | NaNs passed over first, as `minimum`/`maximum` do (`[Stats-Median]`, `[Stats-Percentile]`) |
| 286 | `String::truncate` faulted past the end while `Vec::truncate` does nothing | it leaves the text as it is; inside a character it still faults (D-0085 revisited, D-0337 §2.19) |
| 287 | `from_hex("")` was `Err(Empty)`, so an empty blob did not round-trip | `Ok` of no bytes; `postgres.cb`'s special case removed (`[Hex]`) |
| 296 | `--doc PgValue` never named its variants | `--doc` lists an enum's variants and a struct's exported fields (`summary.cb`, `cobdoc.py`, checked by `doc_matches_cobdoc.py`) |
| 298 | an idle keep-alive connection's timeout was `Err`, so the documented 400 answered the client's next request | `Ok(None)` and the connection closes (`next_arrives`); a timeout inside a request stays `Err` (`[Http-Request]`) |
| 299 | `send` kept a request's own `Transfer-Encoding` beside its `Content-Length` and an unchunked body | `Transfer-Encoding`, `Content-Length`, `Host` and `Connection` of the client or request are never copied (`is_framing`, `[Http-Send]`) |
| 303 | `PgValue::Float(2.0)` was sent as `2.0`, which an integer column refuses | a whole value within ±2^53 is sent as `2` (`-0` for −0.0) (`[Pg-Query]`, D-0337 §2.19) |
| 310 | `max_redirects = 0` gave `Err(TooManyRedirects)` for a 302 | 0 follows none and gives the 3xx (`[Http-Redirect]`, D-0337 §2.19) |
| 322 | the guide pointed to a statics section that does not exist | the threads section gives `static Atomic<u64> HITS = 0;` with a runnable example |
| 325 | `Args::undeclared` sliced `&item[0..2]` on `-é` (`diag.not-char-boundary`) | `unknown option -é` |
| 326 | EXDEV, ENAMETOOLONG, E2BIG, EBADF and a NUL in a name were all `FileError::Io` | new `CrossDevice`, `NameTooLong`, `InvalidName`, `NotOpenForThat`, `ArgsTooLong` from new runtime codes −12…−16 (POSIX and Windows); `read_line` of a directory stays `Io`, documented (D-0337 §2.19) |
| 327 | `copy_file` refused a pipe source with `Io` | a source neither regular file nor directory is read to its end (`copy_stream`, `[Copy-File]`) |
| 328 | a TOML duplicate key or table, or month 13, was "unexpected 'n'" at correct text | new `Toml::describe_error` ("name is defined twice", "is not a date or time"); no new `ParseError` variant (`[Toml-Error]`) |
| 330 | `Args` accepted two options of one short name; `-o=x` was advised `-o =x` | a second declaration faults `diag.assert-failed` through a `requires`; `-o=x` is advised `-o x` (`[Args-Flag]`, `[Args-Finish]`) |
| 333 | `Args` could not parse git-style subcommands | new `Args::command(&mut a, "-C --config")` (`[Args-Command]`, D-0337 §2.19) |
| 334 | `File::read_line` drops the `\n`, so a partial last line looked complete | new `File::read_line_with_end` |
| 342 | generic code could not call `entry`, `get_mut` or `remove` with a key of type `K` | new `HashMap::entry_owned`, `HashMap::get_mut_ref`, `HashMap::remove_ref`, `HashSet::remove_ref`; `probe_key` is a probing intrinsic; conformance `update_by_key_held_ok` (D-0337 §2.12) |

## What was found along the way

- **The `match` grammar and `std`.** The first build refused a `match` without a comma after its last arm; that
  build's compiler, copied to `impl/target/prev` by `build.sh`, then refused `std`'s and `gen.cb`'s one-line matches
  as stage 1, which is how the 43 files were found (257). Later builds of that group used the Update 207 compiler as
  stage 1's compiler. With that group's builds, `cobc --check-syntax` over the 2,991 `.cb` files of the repository
  refuses the same 135 files (the ill-formed cases and module fragments) as the Update 207 compiler, and `cobc
  --check` refuses no file that Update 207 accepts (the static pass's new refusals, 291, 313 and 318, came from
  another group and change two conformance cases, above).
- **266 had two causes in two places.** The generated code typed every closure's `self` exclusive, and, found with a
  debugger, the runtime's light call path borrowed the box exclusively whatever the closure's flags said; with the
  runtime half alone one run in three still faulted, and with the generated half alone it still faulted.
- 291's first version refused `Rc::new`'s `Rc { .ptr = bp }` (conformance chapter 21 found it); the holder check
  skips `std` bodies and raw-pointer locals.
- 283's earlier check first ran for a function returning a reference and taking none, changing the outcome of
  `ref_escape_plain_param_return_rejected`; it is limited to functions with two or more reference parameters.
- 258's warning was narrowed three times on the repository's own code (D-0337 §2.1); no `.cb` file gets it.
- 277: the first form printed a destructor's fault twice when it was the thread's own (`rc_hidden_in_callback_list`);
  an identical repeat is reported once. 321: the lone-waiter text is chosen when the deadlock is found, so "each
  other" stays for `atomic_wait_each_other` and `deadlock_main_end_of_chain`.
- `exclusive_ref_lent_twice_dynamic_rejected` once reported its fault in the thread (line 19) instead of at the
  `spawn` (line 25) under Clang, a timing difference; ten reruns under each compiler gave line 25.
- Ten inline rows' recorded faults (`conf_abs_min_overflows`, `conf_compound_assign_overflow`,
  `conf_dollar_empty_overflow`, `conf_i32_add_overflow_dynamic`, `conf_index_out_of_bounds_dynamic`,
  `conf_limits_overflow_dynamic`, `conf_narrow_overflow`, `conf_slice_bounds_dynamic`, `conf_str_byte_out_of_bounds`,
  `conf_vec_index_out_of_bounds`) and `impl/tests/programs/conf_narrow_overflow.expect` still have the fault texts
  without values from before Update 207's report 065: the Update 207 compiler gives the new texts too.

## What is left

- An uncontended `lock` is still far from C's 10 ns (315): a fast path in the mutex's own cells needs the deadlock
  finder and poisoning to read holders from there, a record redesign.
- A long `Rc` chain still exhausts the stack when destroyed (337); its end is one report now, not an iterative drop.
- The static pass cannot prove `&mut s[0..mid]` and `&mut s[mid..n]` disjoint (311): symbolic range bounds are a
  possible static-pass item.
- `read_line` from a standard input that is a directory stays `Io` (326): its lowering is native (`native_io.cb`).
- A destructor that faults during a contained thread's unwind still ends the program (277).
- A thread that never ends keeps the program running (316, chosen).
- 314 has no case (it needs at least 10 s per hanging run); the reproduction is its check.
- The ten inline rows and the program case above are to be re-recorded.

## Verification

- Each report's reproduction behaves as its "how to verify" asks, under GCC and Clang (the PostgreSQL ones against
  the local server); each group added focused cases (`impl/tests/cases/tools/diagnostic_forms_bugs249.case`,
  `impl/tests/cases/cobc/checker_reports_249_static.case`, `generics_reports_249.case`,
  `runtime_reports_249_streams_and_faults.case`, `runtime_reports_249_threads.case`,
  `driver_explore_outcomes_shown_apart.case`, `driver_build_cache.case` extended; conformance
  `std_reports_249_ok`, `args_command_ok`, `args_declared_twice_fault`, `http_reports_249_ok`, `fs_reports_249_ok`,
  `update_by_key_held_ok`, with new rows in `spec/conformance.md`) and ran them, with the existing cases and
  conformance chapters of its area, under both compilers.
- Recorded messages re-recorded where the text changed (conformance and program `.expect` files, 13 inline rows,
  `impl/tests/foreign_forms/expected.txt`), with GCC and Clang agreeing, each diff read.
- `decided_check` after each group's last build: identical to the Update 207 base (502 decided, 11 undecided, 255
  rejected; no program changed state).
- `impl/tools/pregate.py` in each group's tree.
- On the merged tree (three integration builds): the full conformance suite under GCC and Clang, 1514 of 1515 passed on
  each, the one failure `file_printf_ok` expecting `Io` where the runtime now gives `NotOpenForThat` (326), its case
  brought to that; `cbtest rows` 2225 passed, 0 failed (GCC and Clang) after re-recording 39 conformance and 13 inline
  rows whose fault texts and carets changed (252, 254); the new and changed case files of every group, 20 passed;
  `decided_check` identical to the Update 207 base; the repros of 266, 319 and 326, whose fixes span two groups.
- `bench/run.sh` once, over the final tree: instructions within 0.01% of Update 207's on every workload under both
  compilers; times within the machine's noise (−2.5% to +4.2%).
