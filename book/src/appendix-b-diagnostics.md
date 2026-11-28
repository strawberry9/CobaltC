# Appendix B: Diagnostics you will meet

Every diagnostic has a stable name. *Static* means the program is rejected
before it runs; *dynamic* means a check failed while it ran and the program
stopped; *both* means the same rule is applied statically where the text
shows the problem and dynamically where only the values can. `cobc --explain
--run prog.cb` prints the full registry entry for any of them, and the
specification's registry under `spec/registry/` lists them all. These are
the ones a newcomer sees most.

A rejected program gets each error that stands on its own, one for each
statement of a function's body (a statement that uses a local a failed one
declared or left unknown is not checked, and a note says so), then a count.
When the text is a habit from C, Rust
or another language (`v.len()`, `let x`, `x as u64`, `println!`, `i32 a[10];`),
the message says how CobaltC writes it, and a misspelt name gets the nearest
one that exists. A failed `assert` of a comparison shows both values:
`assertion failed: 9 == 10`.

| Diagnostic | When | What it means, and the usual fix |
|---|---|---|
| `diag.syntax-error` | static | The text is not a program. The message names what was expected. |
| `diag.unbound-name` | static | Nothing has this name. Check the spelling (the message names the nearest name), declare it, or `import std;`. `cobc --doc Vec` lists a type's functions. |
| `diag.ambiguous-name` | static | Two imports supply the name. Qualify it, `m::name`. |
| `diag.name-not-visible` | static | The item or field is private to its module. Export it, or go through the module's functions. |
| `diag.type-mismatch` | static | The types differ and nothing converts. Convert with `widen`, `narrow` or `to_float`, or fix the type. Also a `printf` specifier that does not match its argument. |
| `diag.cannot-infer-type-parameter` | static | `auto v = Vec::new();` and the like. Write the type on the left. |
| `diag.use-of-uninitialized` | static | A variable is read on some path before it is written. Assign on every path. |
| `diag.duplicate-local` | static | A name declared twice in one block. Assign to it, or use another name. |
| `diag.literal-out-of-range` | static | A literal does not fit its type, `256: u8`. |
| `diag.arith-overflow` | both | `+`, `-` or `*` left the type's range. Widen, or use `wrapping_`, `saturating_` or `checked_`. |
| `diag.div-by-zero` | both | Division or remainder by zero. Test first, or `checked_div`. |
| `diag.narrowing-overflow` | both | `narrow` to a type the value does not fit. |
| `diag.index-out-of-bounds` | both | An index at or past the length. |
| `diag.stale-binding` | both | A name used after its value moved or was dropped, or a reference used after its target ended. Use the new owner, or give the name a new value. |
| `diag.aliasing-conflict` | both | An access while a conflicting reference is live: a write under a reader, a second reference beside an exclusive one, a push while an element reference lives, a spawner using what it lent a thread. End the earlier borrow first. |
| `diag.write-through-shared` | static | A write through `ref<T, shared>`. Take `ref<T, exclusive>`, or put the value in a mutex. |
| `diag.move-while-aliased`, `diag.destroy-while-aliased` | both | A resource moved or dropped while a reference to it lives. |
| `diag.reference-escapes-scope` | static | A reference to a local returned or stored outside its block. Return the value. |
| `diag.lifetime-elision-ambiguous` | static | The checker cannot tell what a function's result points into: it returns a reference or a view and takes no reference parameter, or it returns a reference, a view or a value holding them (`Option<ref<T, shared>>`, `Vec<StringView>`) and takes two or more with no `from` clause. Name the parameters the result may come from (`from a`, or `from a, b`), or return an owned value. |
| `diag.read-of-resource` | static | A resource where a copy would be made, such as `==` on two vectors. Borrow it, or use `slice_eq`. |
| `diag.text-join` | static | `+` with text, as Java, Python and C++ join strings. Append with `String::append(&mut s, t)`, or build the text with `sprintf("%s%s", a, b)`. |
| `diag.move-out-of-field` | static | A resource field moved out of a struct that lives on. Move the whole struct, or destructure it. |
| `diag.overwrite-of-live-resource` | both | An assignment over a resource whose destruction matters. `overwrite`, `replace`, or `drop` first. |
| `diag.non-exhaustive-match` | static | A variant has no arm. Add it, or `_`. |
| `diag.unreachable-arm` | static | An arm no value can reach, such as a repeated literal. |
| `diag.result-discarded` | both | A `Result` computed and not used, or a `handle<Result<…>>` that may end without `join`; at run time, a handle in a container that ended unjoined after its thread gave `Err`. Handle it, `join` the handle, or `_ = e;`. |
| `diag.propagate-outside-fallible-context` | static | `?` in a function that does not return a matching `Result` or `Option`. |
| `diag.unwrap-failed` | dynamic | `unwrap` or `expect` on `None` or `Err`. |
| `diag.assert-failed` | dynamic | An `assert` whose condition was false. |
| `diag.static-assert-failed` | static | A `static_assert` whose condition was false. |
| `diag.capture-list-mismatch` | static | A closure's brackets do not list exactly the outer names its body uses. |
| `diag.unbounded-type-parameter` | static | An operator on a `T` with no bound that gives it. Add `: ordered`, `: number`, or pass the operation in. |
| `diag.spawn-borrow-closure` | static | A closure that borrows locals passed to `spawn` (one capturing nothing, `[]`, is accepted). Use `move`. |
| `diag.not-thread-safe` | both | An `Rc`, or a value holding one, given to `spawn`; at run time, an `Rc` used in a thread other than its maker's. Use an `Arc`. |
| `diag.mutex-reentrant-lock` | dynamic | A thread locked a mutex it already holds. |
| `diag.mutex-poisoned` | dynamic | A thread faulted while holding this mutex. |
| `diag.deadlock`, `diag.channel-deadlock` | dynamic | A wait that nothing can end. |
| `diag.thread-cancelled` | dynamic | The thread was asked to stop; its waiter sees it through `try_join`. |
| `diag.trusted-outside-unsafe` | static | A raw-pointer or extern operation outside an `unsafe` block. |
| `diag.extern-non-ffi-type` | static | An `extern fn` with a type C cannot take. Use scalars, raw pointers and plain structs. |
| `diag.recursive-type` | static | A struct that contains itself directly. Put a `Box` between. |
| `diag.no-main` | static | No `fn main()` at the root, or one with parameters or the wrong return type. A file with `unittest` blocks and no `main` is checked as `--unittest` checks it. |
| `diag.no-effect` | warning | A statement whose value is its only effect, such as `count == count + 1;` (meant `=`) or `x;`. The program runs; assign, or write `_ = …;`. |
| `diag.handle-discarded` | warning | `_ = spawn(…);`, or `spawn(…);` as a statement: the handle ends at once, so the statement waits for the thread. Keep the handle, or push it onto a `Vec` of handles, and `join` it later. |
| `diag.extern-variadic-without-ellipsis` | static | A C library function with a variable argument list (`snprintf`, `open`, `ioctl`, …) declared without `...`. Declare its fixed parameters and then `...`, as C does. |
| `diag.static-assigned` | static | A `static` assigned or passed to `drop`. It lives as long as the program; change it through what it holds (`Atomic::store(&S, x)`). |
| `diag.type-annotation-needed` | static | The compiler cannot take a generic call's or a variant's type arguments from what is written. Write the type where the value is declared (`Result<u8, str> r = …;`), or on the variant (`Ok<u8, str>(…)`). |
| `diag.thread-affine-moved` | static | A guard would be taken to another thread (passed to `spawn`, or captured by a spawned closure). A guard stays on the thread that locked it: pass the mutex (`&m`) and call `lock(m)` in the thread. |
| `diag.never-returns` | static | A function declared `: never` can return: a `return`, or a path that reaches the end of its body. End every path with `exit(…)` or another `: never` call. |
| `diag.reference-identity` | warning | `x == v` of two references to a `T: eq` compares where they refer, not the values. Write `*x == *v` for the values. |
| `diag.comparison-constant` | warning | An unsigned value compared with `0` so that the answer never changes, `i >= 0`. Count down with `while (i > 0) { i -= 1; … }`; for `a - b >= 0`, compare `a >= b`. |
| `diag.match-arm-break` | warning | `break;` ending the block of a `match` arm for a literal value, after its work, inside a loop, written as in a C `switch`: it leaves the loop. Delete it, since an arm never runs on into the next; to leave the loop, write `break` without `;` as the block's value. |

A message beginning `internal error in cobc:` is a bug in the
tool, not in your program. It exits with status 4; please report it with
the program that caused it.
