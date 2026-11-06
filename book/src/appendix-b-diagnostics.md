# Appendix B: Diagnostics you will meet

Every diagnostic has a stable name. *Static* means the program is rejected
before it runs; *dynamic* means a check failed while it ran and the program
stopped; *both* means the same rule is applied statically where the text
shows the problem and dynamically where only the values can. `coby --explain
prog.cb` prints the full registry entry for any of them, and the
specification's registry under `spec/registry/` lists them all. These are
the ones a newcomer sees most.

| Diagnostic | When | What it means, and the usual fix |
|---|---|---|
| `diag.syntax-error` | static | The text is not a program. The message names what was expected. |
| `diag.unbound-name` | static | Nothing has this name. Check the spelling, declare it, or `import std;`. |
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
| `diag.lifetime-elision-ambiguous` | static | A function with two reference parameters returns a reference. Return an owned value. |
| `diag.read-of-resource` | static | A resource where a copy would be made, such as `==` on two vectors. Borrow it, or use `slice_eq`. |
| `diag.move-out-of-field` | static | A resource field moved out of a struct that lives on. Move the whole struct, or destructure it. |
| `diag.overwrite-of-live-resource` | both | An assignment over a resource whose destruction matters. `overwrite`, `replace`, or `drop` first. |
| `diag.non-exhaustive-match` | static | A variant has no arm. Add it, or `_`. |
| `diag.unreachable-arm` | static | An arm no value can reach, such as a repeated literal. |
| `diag.result-discarded` | static | A `Result` computed and not used. Handle it, or `_ = e;`. |
| `diag.propagate-outside-fallible-context` | static | `?` in a function that does not return a matching `Result` or `Option`. |
| `diag.unwrap-failed` | dynamic | `unwrap` or `expect` on `None` or `Err`. |
| `diag.assert-failed` | dynamic | An `assert` whose condition was false. |
| `diag.static-assert-failed` | static | A `static_assert` whose condition was false. |
| `diag.capture-list-mismatch` | static | A closure's brackets do not list exactly the outer names its body uses. |
| `diag.unbounded-type-parameter` | static | An operator on a `T` with no bound that gives it. Add `: ordered`, `: number`, or pass the operation in. |
| `diag.spawn-borrow-closure` | static | A closure that borrows locals passed to `spawn`. Use `move`. |
| `diag.mutex-reentrant-lock` | dynamic | A thread locked a mutex it already holds. |
| `diag.mutex-poisoned` | dynamic | A thread faulted while holding this mutex. |
| `diag.deadlock`, `diag.channel-deadlock` | dynamic | A wait that nothing can end. |
| `diag.thread-cancelled` | dynamic | The thread was asked to stop; its waiter sees it through `try_join`. |
| `diag.trusted-outside-unsafe` | static | A raw-pointer or extern operation outside an `unsafe` block. |
| `diag.extern-non-ffi-type` | static | An `extern fn` with a type C cannot take. Use scalars, raw pointers and plain structs. |
| `diag.recursive-type` | static | A struct that contains itself directly. Put a `Box` between. |
| `diag.no-main` | static | No `fn main()` at the root, or one with parameters or the wrong return type. |

A message beginning `internal error in coby:` or `cobc:` is a bug in the
tool, not in your program. It exits with status 4; please report it with
the program that caused it.
