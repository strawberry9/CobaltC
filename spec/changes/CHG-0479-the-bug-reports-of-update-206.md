# CHG-0479 — The bug reports of Update 206

Status: implemented (2026-11-22); Update 207
Affects: `spec/05` 1.9.0, `spec/06` 1.23.0, `spec/10` 1.8.0, `spec/13` 1.8.0, `spec/14` 1.51.0, `spec/18` 1.12.0, `spec/20`
1.22.0, `spec/21` 4.88.0, `spec/22` 2.67.0, `spec/registry/diagnostics.md` 1.73.0, `spec/conformance.md` 3.324.0,
`spec/IMPLEMENTATION-NOTES.md`; `impl/front` (`typecheck.cb`, `parser.cb`, `lexer.cb`, `modres.cb`, `hints.cb`,
`each.cb`, `check.cb`), `impl/back` (`gen.cb`, `state.cb`, `passes.cb`, `stack_checks.cb`, `cnames.cb`, `libheader.cb`,
`native_misc.cb`, `native_text.cb`, `native_vec.cb`, `vec3.cb`, `std_values.cb` and `header.cb` generated),
`impl/cobc` (`driver.cb`, `certificate.cb`, `diags.cb`, `frontend.cb`, `main.cb`, `summary.cb`, `build_info.cb`
generated), `impl/cobfmt` (`format.cb`, `width.cb`), `impl/cbrt` (`rt_init.c`, `rt_threads.c`, `rt_lock.c`,
`rt_refs.c`, `rt_objs.c`, `outbuf.c`, `fmt.c`, `plat_posix.c`, `plat_win.c`, `procio.c`, `netio.c`, `fileio.c`,
`rt.h`, `io.h`, `plat_io.h`, `cbrt.h`, `exports.txt`, `diag_table.h` generated), `impl/std` (`collections.cb`,
`core.cb`, `memory.cb`, `sync.cb`, `std.cb`, `ffi.cb`, `text.cb`, `math.cb`, `time.cb`, `env.cb`, `fs.cb`,
`process.cb`, `compress.cb`, `crypto/digest.cb`, `crypto/kdf.cb`, `encoding/json.cb`, `encoding/toml.cb`,
`http.cb`, `http/client.cb`, `http/server.cb`, `tls.cb`, `x509.cb`, `database.cb`, `database/postgres.cb`,
`intrinsics.md`), `impl/front/prelude.cb` (generated), `impl/tools/gen_saslprep.py` (new, checked by `impl/tools/pregate.py`); the cases under
`impl/conformance`, `impl/tests/cases`, `impl/tests/programs`, `impl/cobfmt/tests.py`; `book/`, `htmlguide/`;
`CARD.md`; D-0336.

## Why

An agent testing Update 206 (`28818f0`) filed 194 reports (046–239), each with a reproduction (`private/BUGS/`,
outside the repository). The owner asked for every one to be resolved in one push, with focused tests for what
each changed and one bench run at the end, in place of the gate's full profiles. Five agents did the work, by
area. D-0336 has the choices that changed a rule or added to `std`; this record has every report.

## What changes

The number is the report's.

**The checker left to the run, or to nobody, what it can refuse:**

| # | Cause | Now |
|---|---|---|
| 104 | an `extern fn` call's arguments were neither counted nor checked, so a wrong one was truncated or gave invalid C | the count is checked and each argument is checked exactly (`check_arg_exact_types`) |
| 105 | `is_bare_int_literal` ignored a suffix, so `300: u16` adapted to a `u8` parameter and wrapped | only an unsuffixed (or `byte`) literal adapts |
| 108 | a resource part reached through a reference and moved by value faulted at the run | refused statically, with the `clone(&p.name)`, reference and `replace` forms |
| 109 | a non-`move` closure stored in a container that outlives one of its captures faulted at the run (`diag.stale-binding` at the push) | refused statically, naming the capture and the container, with the `move [i](…)` form (D-0336 §2.9) |
| 110 | two fields of one name were accepted | `diag.duplicate-item`, "`Point` has two fields named `x`" |
| 114 | the run held a reference binding to the end of its last statement, the static pass only to its last use, so a binding last used in an `if`'s condition was decided and faulted under `--all-checks` | a binding whose last use is an `if`'s condition ends after the condition, and one whose last use is an assignment's value ends once the value is made, before the place is written (a place of plain data: a local, or fields of a local with at most one index by a literal or a local, not named in the value), in the checker and the run alike; a bounds fault of the place still comes first (D-0007) (`spec/14` `[Ref-Binding-Last-Use]`, D-0336 §2.7) |
| 136 | `if (Some(n) = HashMap::get(&*lock(m), k))` was refused while the same `match` was accepted, and the `match` let a binding made from `n` outlive the guard | a binder's view of a scrutinee temporary is free in its arm; a binding made from it and used after the `match` or `if` ends is refused (D-0336 §2.9); the two forms agree |
| 180 | a scrutinee calling with a closure literal kept the closure's capture borrows over the arms, so `--all-checks` reported a false move | the scrutinee is evaluated in its own statement scope |
| 080 | a `fn` value stored in a `Vec` or `HashMap` was read by copy when its element ended, so the closure's borrow of its captures outlived the container (a false `diag.aliasing-conflict`) | a `fn` value read through a raw pointer moves out, and a reclaimed element holding `fn` values ends in place (`cb_fn_moved_out`) |
| 234 | the constant check of `to_int` was wrong at both ends | it compares exactly with −2^(bits−1) and 2^bits; the message shows `u128`'s real maximum; `diag.narrowing-overflow`'s repair says to test a float's range first |

**The checker refused correct programs:**

| # | Cause | Now |
|---|---|---|
| 047 | `%s` and `%v` did not read through a `ref<StringView, _>` | they do, as for numbers and text (D-0042) |
| 049 | a generic body's `foreach` element is untyped there, and `clone` refused it | an untyped local argument is left to the instantiation's check |
| 052 | a reference binder of a `match` arm or `if (P = &x)` lasted to the arm's end | it ends after its last use in the arm's block, in the checker and the run (D-0336 §2.7) |
| 056 | a local's known value was folded into a branch its guard excludes, giving a static refusal | not folded inside the branches of an `if` whose condition names it, nor in the right operand of `&&`/`||` (D-0336 §2.8) |
| 075 | a shared view copied out of a field through `ref<S, exclusive>` borrowed the whole struct exclusively | it holds a shared fact on that field |
| 082 | `can_receive` recursed into a self-recursive type to its depth limit and answered "may receive" | a type met again inside itself adds nothing |
| 092 | `n.parent = None` on an `Option<Weak<T>>` field was refused as an overwrite of a live resource | `Weak` and `ArcWeak` are quiet for any `T` (`spec/05`, D-0336 §2.10) |
| 113 | a `probe<K>` argument counted as kept by a `from` result | a probe parameter is never kept |
| 135 | `spawn(f, a…)` checked the arguments without `f`'s parameter types | each argument is checked with its parameter's type expected |
| 161 | a `Vec` of views or shared references was not quiet, so reassigning it was refused | shared references and slices are quiet (D-0336 §2.10) |
| 162 | `count > 5` with `count: ref<u32, shared>` was refused | the literal takes the number's type, so `[Cmp-Through-Shared-Ref]` applies (D-0336 §2.5) |
| 181 | `Option::ok_or(o, "msg")?` in a `Result<_, String>` function was refused | `?` on a call gives its deferred literal the function's error type (D-0336 §2.13) |
| 183 | a `from` clause refused a `foreach` over a probe-only parameter | a value made from the parameter whose initializer would keep it is followed as that value through the block; a binding *assigned* such a value later stays refused (following it would mean every later use, including an enclosing loop's next round), and the message names the declared form, `StringView rest = &path[…];` |
| 185 | a `..` pattern's hidden binders counted as naming a private field | the visibility check skips them |
| 190 | `HashMap::entry(&mut m, &Red)` was refused where `get` accepted it | a temporary may be borrowed for a call whose reference result cannot reach it (D-0336 §2.13) |
| 229 | `&unsafe { … }` or `&{ … }` as a call argument was refused | a block's value is a temporary that can be borrowed as an argument |

**Accepted programs that `cobc` or the C compiler could not build:**

| # | Now |
|---|---|
| 074 | the element-reference `let` (D-0200) lowers its index with the vector's length for `$`; an `EDollar` with no length is a named compiler assertion |
| 079 | a closure argument no longer fixes a bare type parameter before the receiver does, and is boxed when the parameter's type after substitution is a `fn` type |
| 106 | a diverging `then` branch takes the other branch's type, so a missing `return` after it is refused |
| 107 | a variant called with the wrong number of values is refused ("`Quit` carries no value: write `Quit`, without `()`") |
| 112 | a field read through a reference to a reference in a closure is read through both |
| 142 | a payload-less variant written as a value has its enum's type, so `widen`/`narrow` of it is refused |
| 144 | an `auto`-bound closure moves its object into the binding |
| 145 | the thunk of a `fn` value whose parameter is `void` takes a `cb_unit` |
| 159 | `String::append` of an enum is refused with the `appendf("%v")` form (D-0336 §2.6) |
| 210 | an array literal of unit variants has the enum's element type, so the slice coercion is made |

**The language's rules (D-0336):**

| # | Now |
|---|---|
| 055 | `f(a < b, c > d)`: `identifier '<' types '>'` is type arguments unless the token after `>` begins an operand (`spec/22` disambiguation (1)) |
| 127 | `m::Type name = …;` is a declaration whatever `m::Type` names (`spec/22` disambiguation (4)); an unknown type is reported by name, with "did you mean `std::String`" |
| 081 | `resource enum` is gone from the grammar and every message; a `drop` on an enum, which never ran, is `diag.destructor-on-plain-type` |
| 182 | a range whose bounds are both byte literals is a `u8` range (`spec/14` rule.control.foreach-range) |
| 068 | new warning `diag.comparison-constant`: an unsigned value compared with `0` so that the result is fixed; the message gives the count-down form |
| 125 | new warning `diag.no-effect`: a statement that is only a comparison, a name, a field or a literal |
| 046 | `%v` stays as D-0100 has it; the book no longer says it prints any value, and the refusal says what to print instead |
| 086 | `HashMap::contains_ref`, `HashMap::get_ref`, `HashSet::contains_ref`, `Vec::contains_ref` look up by a key held as a reference |
| 098 | `StringView` is `eq` and `ordered` by its text, so `Vec::contains`, `sort` and `index_of` take a `Vec<StringView>`; it is still not a key type |

**Diagnostics:**

| # | Now |
|---|---|
| 050 | "did you mean" prefers a name equal but for case, a function whose name begins the misspelling, a module's name before `::`, a field's nearest name, and an exported type of a module not imported |
| 053 | a float exponent of `pow` names `powf(x, y)`; every float-to-integer hint says `to_int` drops the fraction |
| 058 | `==` on a `Vec` (or a struct holding one) says what `==` compares and names `Vec::eq(&a, &b)` or a field-by-field comparison |
| 060 | the end of the file inside a block is reported at the `{` never closed, never at "unknown location"; a `fn` at column 1 inside a block asks whether a `}` is missing above it |
| 061 | `'hello world'`: "text is written in double quotes"; a `match` arm without its comma is told each arm ends with `,` |
| 062 | `diag.stale-binding` says where the binding was moved and names `clone(&x)` |
| 063 | an integer division where a float is declared gives the converted operands (`to_float<f64>(total) / to_float<f64>(count)`) |
| 064 | `P { x = 1 }`, `P { 1, 2 }` and `P { x, y }` are given the `P { .x = x, .y = y }` form |
| 067 | `?` in `main` names the `fn run() : Result<void, E>` form; `main` stays `void` or `u8` (D-0336 §2.6) |
| 069 | an index of another integer type: "an index is a `usize`: `widen<usize>(i)`"; mixed operands name the conversion that keeps every value |
| 071 | an empty slice argument is told how to write it (`array<PgValue, 0> none = [];`, `&none`); `PgConnection::query`'s documentation shows it |
| 072 | the "not a reference" message shows the parameter as the argument's own type makes it |
| 084 | `String::append(&mut out, &s)` with `s` already a reference: "write it without `&`" |
| 085 | the invalid-escape message lists the text escapes, says `\xNN` is a byte literal's and that `'` needs none |
| 087 | `&id` where `id` is wanted: "pass the value itself"; a call's reference result is told to take `*` |
| 088 | a `foreach` binder passed as a `StringView`: "pass its value, `*w`" |
| 089 | `foreach` over a `Queue` or `PriorityQueue` names the `while (Some(x) = Queue::pop_front(&mut q))` form |
| 090 | an error in one instantiation of a generic function names it and the call that makes it |
| 091 | `T: eq + clone` says a parameter has one bound and which one has both |
| 097 | a discarded `Result` is placed at the call, a declaration's mismatch at its initializer, a module resolution error at its expression |
| 099 | the `if`/`else` mismatch names both types; branches of `String` and `str` name `String::copy(…)` |
| 101 | `&if …` as an argument says it is not allowed even there and shows binding the value first |
| 115 | `foreach (w in &f(…))` with `f` unbound reports `diag.unbound-name` first |
| 116 | `diag.unbounded-type-parameter`'s repair names the bound the operator needs |
| 117 | a `Result` function given an `Option` (or the reverse) is a `diag.type-mismatch` naming the matching function |
| 118 | a `mutex` used as its value names `*lock(&hits) += 1`; an `Atomic` names its functions |
| 119 | `*s` of a reference parameter names `clone(s)`, not "end the reference" |
| 120 | `cobc --check` of a file with `unittest` blocks and no `main` checks it as `--unittest` does; otherwise a missing `main` is reported last, after the other errors |
| 121 | an unsigned subtraction below zero names a check first, a signed type, `checked_sub` and `saturating_sub` |
| 122 | a closure's signature mismatch shows both signatures |
| 123 | two integer arguments of each other's parameter type are reported as swapped, before any conversion |
| 124 | an integer literal in float arithmetic: "write the literal as a float: `2.0`" |
| 128 | `w: f64`, `pub f64 x;`, `3 ** 3`, `v[-1]`, `range(3)`, `i32 lo, hi;` and `Rect(w, h)` as a pattern are each given the CobaltC form |
| 129 | a message that gives the form to write has no repair line; the arithmetic repair is kept for numeric messages; `m["a"]` names `HashMap::get`; a non-`bool` condition names the test |
| 130 | reference messages name the program's binding, and an assignment to a `ref<T, exclusive>` says to write through it |
| 151 | several syntax errors from `cobfmt` or `--check-syntax` are printed one per line with their columns |
| 154 | `cobc --check DIR` with no `.cb` file says so, status 2 |
| 163 | the `eq`/`ordered` message says which types qualify and names the field that does not |
| 164 | `--doc narrow` and `to_int` name `diag.narrowing-overflow`; `to_float` says it converts `f64` to `f32` too |
| 165 | an implicit enum code past its type is one error at the variant ("would take the code after 255, which does not fit `u8`") |
| 184 | `4 \| 6 :` and `b'0'..=b'9' :` say there are no or- or range patterns and give the one-arm `if` form |
| 186 | a syntax error in an item inside a module ends that item only, as at the top level |
| 187 | `import … as`, `import m::{a, b}` and `import m::*` are given the one-name form |
| 188 | `export import` in a library root checked alone says the root has no name there and how to check such a library (D-0336 §2.6) |
| 189 | an initializer whose `i32` comes only from unsuffixed literals is told to type one of them (`0: usize`), not to `narrow` |
| 191 | a `void` block initializer: "a block's value is its last expression, written without `;`"; the next statement is still checked |
| 192 | `0 <= x < 10`: "comparisons do not chain: write `0 <= x && x < 10`" |
| 193 | an array or `Vec` passed by name to a slice parameter: "lend it: `&a`" |
| 231 | every extern signature error is reported, with the bodies' errors |

**Time and memory of checks, lowering and the run:**

| # | Cause | Now |
|---|---|---|
| 073 | a `match` on text or scalars was one `else if` nested as deep as its arms (192 s to build at 16,000 arms) | over 64 arms it is a flat run of tests with one label after |
| 093 | the decided lowering kept frames and objects for an arm whose tail is an `if` rebuilding moved parts | a raw sink passes into a tail block; no frame, statement scope or object left; 10M rebuilds 0.10 s (was 4.3–5.7 s; the control 0.095 s) |
| 131 | arithmetic in a `spawn` argument counted as a possible reference and kept the program's records | operators, conversions and their `if`/block values are never references; 2.9 s → 75 ms |
| 174 | `TomlReader::basic`'s `if` argument beside the reader kept every caller's records | `arg_holds` looks into branches and tails, and `std` binds those arguments first; 8.3 s → 73 ms |
| 148 | a closure's `self` counted as a runtime call, and `cb_fn_shared` walked the box at each call | `cb_clself` and `cb_lf` are data names; a box's flags are cached; through a reference 319 → 114–132 ms (indexed 81), a capturing closure 609 → 260–272 ms (non-capturing 221–229) |
| 149 | the zero-sized-type anchoring searched every unit for every captureless closure's type, and `elide_record_chains` rescanned per chain | a set lookup per `struct`, bounds and counts made once; 400/800/1,600 closures 1.43/2.76/5.46 s (was 1.31/4.06/16+) |
| 150 | `check_if` and `&&` copied the whole flow per branch | an undo log while branches are checked, joining only the names changed; 7,009/14,009/28,009 lines 1.07/1.93/3.77 s (was 1.6/4.9/17.3) |
| 155 | each copy of a closure copied its whole chain of captured closures | a copy that nothing could tell from the original shares the box (D-0336 §2.17); 500/1,000/2,000 wrappers 1/3/5 ms (was 254/1,096/4,965), 32,000 in 102 ms |
| 048 | `Vec::sort` of floats went through the generic sort | sorted natively by `[Sort-Float]`'s key; 1M `f64` 185 ms beside `i64` 111–155 ms (was 270 beside 70) |
| 100 | `StringView::replace` scanned naively | each occurrence found with the native `find`; an absent needle 124 ms (was 209), a repetitive one 424 ms (was 5,320) |
| 146 | `BigUint::parse` took one digit a step | nine digits a step in place, and `text` divides in place by 10^9; 80,000 digits parse in 40 ms (was 2,188), `text` 314 ms (was 639) |
| 147 | `crc32` was computed bit by bit | slicing by 8 over a constant table; 20 MB in 58 ms (was 161), `gunzip` 200 ms (was 273) |
| 219 | a TOML table's members were found by a linear search | a table of eight or more members is indexed; 8,000 keys 8 ms (was 16 s) |
| 132 | separate mutexes and channels were serialised through one runtime lock | the mutexes held and the event counts are in 64 striped locks; the book is taken only when a thread waits on the key; four threads each on its own mutex 1,658 → 160–226 ms (one thread 100), on its own channel 2,479 → 291–366 ms (one thread 135) |
| 236 | each joined thread left its record, so `spawn`/`join` slowed down linearly | `join` removes it; batches flat at about 0.22 s, memory flat |
| 141 | a C thread calling a `--lib` library left about 768 bytes | its whole runtime state is freed when it ends; 2.4 MB flat over 20,000 threads |
| 238 | each failure caught with `try_join` was kept to the program's end | `ThreadFailure` is a resource whose `drop` frees the record (D-0336 §2.16); 40,000 handled failures 2.7 MB flat (was 15.9 MB) |

**The runtime:**

| # | Now |
|---|---|
| 065 | an index, range, `+ - *` overflow, `narrow` and `to_int` fault gives its values ("index 5 is out of bounds for length 3", "250 + 10 is outside `u8` (0 to 255)"); new exports `cb_fault_index`, `cb_fault_range`, `cb_fault_arith`, `cb_fault_arith_r`, `cb_fault_arith_c`, `cb_fault_narrow`, `cb_fault_to_int`. The fault of `+` and `-` is given the wrapped result and the second operand, and the runtime rebuilds the first (modulo 2^bits); of `*` by an odd literal, the wrapped product and the literal (the other operand is the product times the literal's inverse modulo 2^bits); of another `*` up to 64 bits, the product at double width; so the hot path keeps no copy of an operand the operation overwrites. (Passing both operands, as first written, cost 10–33% more instructions in `bench`, `collatz`, `records` and `sieve_fn`: the bench over the merged tree found it.) |
| 066 | `Result::unwrap` and `expect` name the error ("`Result::unwrap` found `Err`: NotFound", "reading the config: NotFound"), through the `std`-only intrinsic `unwrap_failed_with` (D-0336 §2.18) |
| 070 | a fault that poisons a mutex is recorded with it; `diag.mutex-poisoned` names it ("a thread failed while holding this lock: `diag.deadlock` at FILE:LINE") |
| 169 | `diag.channel-deadlock` names a failed, unjoined thread's fault |
| 102 | a write to standard output refused because the pipe's reader has gone ends the program as `exit(1)` does (`[Print-Reader-Gone]`, D-0336 §2.14); `forever \| head -3` ends at once |
| 167 | standard streams closed at start are opened on the null device, so `printf` cannot write into the program's own file; a write to a standard output closed at start is `[Print-Failure]` |
| 171 | the `[Print-Failure]` line follows a fault's report too |
| 134 | a handle destroyed on the thread it names makes the thread self-owned; at the end of `main` the runtime waits for every thread still running (D-0336 §2.16) |
| 166 | a stack-exhaustion unwind lifts the floor first, so its destructors run, and a thread's reaches `try_join` |
| 170 | a destructor that faults during the program's unwind does not stop it; the faults are reported in order (D-0336 §2.15) |
| 126 | `%u` prints the value's bits unsigned in its width, as C |
| 235 | `#` adds no prefix to 0, and a NaN with its sign bit prints `-nan`, as C |
| 232 | `allocate` with an alignment that is not a power of two gives `Err` |
| 230 | the null-pointer faults name the function; `diag.null-pointer` says "where an address was required" |

**`cobc` and `cobfmt`:**

| # | Now |
|---|---|
| 059 | an executable build cache keyed by the generated C and everything else the build reads (D-0336 §2.19); `http_min.cb` 35 s the first time, 5.0 s the second (machine loaded) |
| 137 | `--explore`'s replay line carries `--all-checks`, `--bootstrap-form`, the file and the program's arguments, shell-quoted |
| 138 | `--emit-c -o` refuses the program's own files and `.cb` names |
| 139 | `--certificate FILE` is checked without emptying anything and written through a temporary file renamed over `FILE` |
| 152 | `--lib` on a file with no `main` and no entry point says what a library needs; the header's link line names an archive not called `libNAME.a` as `-l:NAME` |
| 153 | not a defect: the test's label is printed when it starts so that a slow test shows which it is; the book now says so |
| 094, 143, 160, 179 | `cobfmt` spaces a unary operator after `=` and binary operators consistently; `--width` leaves a `foreach`'s bindings and a capture list whole; a one-line body with a block comment keeps its line comment after the `}`; a closure in a struct-literal field is indented at the field |

**C interop:**

| # | Now |
|---|---|
| 140 | documented: a C program links one CobaltC library (each carries its program's start-up and type table for the one runtime); the header's contrary sentence is gone |
| 225 | the header defines `cb_i128`/`cb_u128` when a signature uses them |
| 226 | a `rawptr<array<T, N>>` parameter is written `T *` |
| 227 | a struct an entry point takes through `rawptr` is defined in full where C can declare it |
| 228 | a C library function with a variable argument list is declared with its fixed parameters and `...`; one of the program's own C is called through a fixed wrapper (`spec/20` rule.trust.extern-call) |
| 233 | the header can be included from C++ (`extern "C"`, `COBALT_STATIC_ASSERT`, `COBALT_ALIGNOF`) |

**Security (HTTP, TLS, X.509, password hashes):**

| # | Now |
|---|---|
| 077 | every header `respond` writes is checked (a name a token, a value with no control byte but tab, no DEL); a bad one is `Err(BadMessage)` with nothing written |
| 078 | the client checks the method, headers and target before any connection is taken; `Url::parse` refuses a byte at or below 0x20 or 0x7F and a host that is neither a reg-name nor a bracketed IPv6 address |
| 156 | `Url::parse` refuses an empty host and a port that is not 1–5 digits up to 65535 |
| 176 | `respond` drops a handler's `Transfer-Encoding` |
| 177 | no `100 Continue` for a declared length over `max_body`; the body is `TooLarge` at once |
| 178 | a status outside 100–599 is `Err(BadMessage)`, nothing written |
| 194 | the reader drops consumed bytes before each read, and trailers are read as fields within the 64 KiB head limit; the server stays at 2,044 kB after 100 MB of pipelined input |
| 195 | an HTTP/1.0 message with `Transfer-Encoding` is `BadMessage`, on the server and the client |
| 196 | chunked framing is strict: CRLF, 1–8 hex digits from the first byte, extensions without control bytes |
| 197 | a method that is not a token is refused (400) |
| 198 | a `Host` value is empty or a uri-host with an optional port |
| 199 | a redirect's `Location` is resolved by RFC 3986 §5.2 |
| 200 | `Connection` is read as a comma list over all its fields; `close` anywhere closes |
| 201 | `100 Continue` only to HTTP/1.1 |
| 202 | at most 16 interim responses; the 17th is `BadMessage` (D-0336 §2.23) |
| 203 | `form_decode` and `Url::query_pairs` split at `&` only (D-0336 §2.23) |
| 211 | the TLS server checks the chosen key share's length for its group, in both ClientHellos; alert 47 and `HandshakeFailed`, not a dead process |
| 216 | the TLS client answers a `CertificateRequest` with an empty `Certificate`; alert 116 has its own text |
| 237 | `HttpServer::accept` no longer does the TLS handshake; the connection's first `request` or `respond` does (D-0336 §2.23); a client behind an idle connection is answered in 0.1 s (was 29.7 s) |
| 212 | name constraints (dNSName, iPAddress) are parsed and enforced over every name below the issuer and the host; a critical constraint of another form makes the certificate unusable; a violation is `NotAuthority` |
| 213 | `verify_chain` searches every candidate issuer depth first, within 100 signature checks |
| 214 | the trust anchor's path length and an issuer's EKU are enforced (`NotAuthority`, `WrongUsage`) |
| 215 | `verify_password` gives `false` for a stored hash with `t` above 2^32−1 or `m` above 1,048,576 KiB, and `argon2id` faults `kdf_parameters` (not `narrowing-overflow`) above RFC 9106's 32-bit bounds |

**PostgreSQL:**

| # | Now |
|---|---|
| 204 | a decoded NUL in the URL's user, password or database is `BadConnectionUrl`, as libpq has it |
| 205 | a `client_encoding` other than UTF8 ends the statement with the new `PgError::Unsupported` and closes the connection (D-0336 §2.25) |
| 206 | a reply that times out sends a `CancelRequest` and waits once more, so the outcome is the server's and the connection stays usable; new `PgConnection::set_timeout_ms` and `TlsStream::set_read_timeout_ms`/`set_write_timeout_ms` (D-0336 §2.25); the report's insert is now "canceling statement due to user request (57014)", 0 rows stored |
| 207 | an `ErrorResponse` before the stream ends is given as `Err(Server(e))` (FATAL 57P01 and the like) and the connection is closed |
| 208 | `COPY` in either direction is `Unsupported`, the connection stays usable |
| 209 | SCRAM uses `saslprep(password)` (new, RFC 4013 over Unicode 3.2, tables generated by `impl/tools/gen_saslprep.py`), the raw password when it gives `None` |

**The rest of `std`:**

| # | Now |
|---|---|
| 076 | `copy_file` onto itself is `Err(SameFile)` before the target is truncated (new variant, D-0336 §2.21) |
| 095 | `NotEmpty`, `LinkLoop` (new), `IsDirectory` for `copy_file` of a directory, `NotFound` for a `Command`'s missing working directory |
| 096 | new `read_link`; a link loop is `LinkLoop` |
| 103 | new `list_dir_bytes` beside `list_dir` |
| 224 | new `File::create_private` and `make_temp_dir` |
| 222 | `write_bytes` writes in one call and reports the failure for data under 8 KiB too |
| 168 | `fsync`'s EINVAL, EROFS and ENOTSUP mean nothing to sync, so closing `/dev/null`, a pipe or a FIFO succeeds |
| 223 | an environment name empty or holding `=` is refused, as one holding NUL |
| 239 | a `Child` dropped without `wait` is reaped (after `kill`, at once; otherwise at the next spawn or drop) |
| 172 | documented: `Child::wait` does not drain the child's pipes |
| 173 | documented: `kill_tree` does not reach a process that left the tree (D-0336 §3) |
| 175 | after one SIGINT under `watch_interrupts`, a socket wait returns early only for an interrupt not yet seen by `interrupt_requested()` |
| 111 | `Rc`'s strong handles together hold one weak count, so the last `Weak` living in the value no longer frees the box under `Rc::drop` |
| 133 | a cancelled `Barrier` wait takes its arrival back |
| 083 | documented: `[Retain]` was right, the documentation of `retain`/`dedup` said otherwise |
| 051 | `ParseError::text(Invalid)` is "invalid syntax"; new `ParseError::describe(&e, text)` places it (D-0336 §2.22) |
| 054 | `Args::finish` names a declared option written in another form, with the form to write |
| 057 | new `DateTime::to_iso_local` |
| 157 | JSON literals and HTTP dates report the documented error position |
| 158 | `utf8_error_at` gives the start of the bad sequence, the end of the valid prefix |
| 217 | `from_iso` and `unix_ms_from_iso` give `OutOfRange` for a moment whose seconds do not fit an `i64` |
| 218 | each part of a TOML header or dotted key counts against the 512-level limit |
| 220 | a gzip member is decoded apart from the one before it, and zero bytes after the last member are the end |
| 221 | `json_number` writes negative zero as `-0.0` |

## What was found along the way

- **The stage-1 compiler's decided lowering asserted on the merged source.** Building the merged tree, stage 1 (compiled
  by the `28818f0` compiler) stopped at "an objectless local moved away without a flag" (`gen.cb` `read_place`) on
  `HashSet::contains(&x.inert_c, &vals[i].c)` in `gen::lower_array_lit`; G2 had met the same assertion earlier on a
  `foreach` in `early_end_plan_with`. The cause (G2): the second, decided lowering made new `Gen` ids from 1<<48 again
  while `Facts.temp_borrows` still held ids the first had made, so a synthesized `&vals` could carry such an id and be
  lowered as a temporary, a by-value read of `vals`. The new compiler continues the first lowering's ids
  (`second.synth_id = synth_id`), and `lower_borrow` never takes a path to a bound local for a temporary. Because stage
  1 is still built by `28818f0`, `lower_array_lit` looks the set up through a helper (`is_inert_c`), which that compiler
  lowers correctly. Turning the decided lowering off for stage 1 was tried and undone: the old first lowering gives
  invalid C for `typecheck`'s `expanding_type`. No reduced case was made: it needs the two lowerings' id streams to
  collide.
- **A view outlived a temporary guard through `match` (soundness, present at `28818f0`).** `match
  (HashMap::get(&*lock(m), k)) { Some(n) : { kept = n; }, None : {} }` followed by `StringView::len(kept)` was accepted
  (not decided) and ran in the default form and under `--all-checks` without a fault, though the guard had been
  released: the run does not end views through a guard when it unlocks. It is closed statically with 136 (above):
  a binding made from the binder of a scrutinee temporary's view is refused when it is used after the `match` ends. The run still does not end views through a guard when it unlocks; the static rule is what refuses it.
- G2's 052 change in the back end first searched enclosing scopes for any ended name, and a name with no binding in
  the block hit an outer one (invalid C in `typecheck`'s `expanding_type__frz`); arm binders now end through their own
  list (`Facts.binder_ends`).
- 170: guarding a destructor during a contained thread's unwind resumed generated drop code with scopes out of step
  (`conf.rc-hidden-in-callback-list`: "statement pop closed a frame"), so that case keeps the earlier behaviour.
- 238: `showcase/tier13/deadlock_found.cb` iterated `[try_join(t1), try_join(t2)]` by value, which a resource
  `ThreadFailure` refuses; it takes each `Result` through a helper, and is decided again with the same output.

## What is left

- `spec/21`'s "The `std` source" listings were already out of step with `impl/std` (documentation lines stripped,
  older text) and no tool keeps them; only the rule texts and one listing line were updated.
- `BigUint::text` is still quadratic (146), near Python's time on the reported size.
- `StringView::find`'s worst case is still `memchr` and `memcmp`, not Two-Way (100).
- Under Clang, `bench/sieve`'s inner loop keeps its bounds and overflow checks once an arithmetic fault is given
  values (+12.5% instructions against Update 206; the time is unchanged, 1.78 s, the loop being bound by memory, and
  still fewer instructions than checked Rust's); every form tried (both operands, a site record, values in
  thread-locals, `cold`) costs the same, only a fault with no values does not (065).
- `kill_tree` cannot reach a double-forked daemon (173), a documented limit: a process group would take the child out
  of the terminal's foreground group.
- Division, negation, shifts and the native map and channel checks still report the summary without values (065).
- A destructor that faults during a contained thread's unwind still ends the program with the first fault (170).
- A reference's last use in an assignment whose place can hold a reference, or is reached through more than one index,
  still lasts to the end of the statement at the run (114); the static pass then refuses or decides as before.
- An `EPIPE` on `Child::write_input` stays `Err(Io)` (095; documented in `[Child-Streams]`).
- 175 has no case (it needs a signal and a server); the reproduction is its check.

## Checked

- Each report's reproduction behaves as its "how to verify" asks, under GCC and Clang (the PostgreSQL ones against the
  local server, 216 against `openssl s_server`).
- New conformance cases and rows (`spec/conformance.md` 3.324.0), new cases under `impl/tests/cases` and four new
  `cobfmt` tests; each group ran its new cases and the existing cases of its area under both compilers.
- The static rejections' and faults' recorded messages re-recorded where the text changed, each diff reviewed.
- `decided_check` against the base after each group's last build: no program lost DECIDED; three gained it
  (`showcase/tier2/parallel_computation.cb`, `showcase/tier3/parallel_sort.cb`, a `flow_d0071` case whose negative is
  now refused again by using the binder after the write); after G2's third round 502 decided, 11 undecided, 255
  rejected, the same on the merged tree.
- After the push, the bootstrap workflow's full conformance run found four cases the focused runs had not reached
  (unchanged files whose outcome the reports changed): `conf.result-source-kept-bound` and `-captured` (183 now
  follows a copy and a closure, so each case gives the result a kept form), and the recorded output of
  `format_in_place_ok` (126) and `thread_is_finished_ok` (065); fixed, and the whole conformance suite run under
  GCC and Clang before the second push.
- On the merged tree, built once: the 169 conformance cases added or changed since `28818f0` and the 185 runs of
  `impl/tests/cases`, under GCC and Clang; the 178 rows of `spec/conformance.md` whose text or program changed, under
  both; every report's `repro/run.sh` (no internal error, assertion or crash; five need the reporter's machine: paths,
  a PostgreSQL role, Python's `tomllib`).
- `impl/tools/pregate.py` in each group's tree.
- `bench/run.sh` over the merged tree: the first run found 065's cost (above), fixed; the run on the final tree has
  every build agree, and against Update 206's instruction counts GCC is within 1% on every workload and Clang within 1%
  or lower (`collatz` -25%, `bench` -7%, `sieve_fn` -3%) except `sieve` (+12.5%, above); `bench/RESULTS.md` is that
  run.
