# D-0333 — The reports Update 203 left open

Status: ACCEPTED (2026-11-21). Decided for the owner: after Update 203 the owner asked that the next push close every
report of `private/BUGS` that Update 203 had left open or answered in part, "so no next push is needed", and left every
choice to the implementation. Implemented by `CHG-0476`
Kind: Decision (`spec/02-schema.md`)
Depends on: D-0332 (the reports), D-0290 (frame-held aggregates), D-0134 (`remove`), D-0116 (`clone`), D-0204 (stack
exhaustion), D-0257 (entry points), D-0331 (the map's entries)
Affects: `spec/14` §6, `spec/16` §4a, `spec/17`, `spec/19`, `spec/21` (`rule.stdlib.rc`, `rule.stdlib.hashmap`),
`spec/registry/diagnostics.md`, `spec/IMPLEMENTATION-NOTES.md`

## 1. Decisions

1. **An `Rc` is checked at run time as well (031).** Each `Rc`'s box records the thread that made it, and every
   operation of `Rc` and `Weak` compares it with the running thread and faults, `diag.not-thread-safe` (now phase
   *both*), on a mismatch. The static refusal of `spawn` stays, as the earlier and plainer error; the run-time check
   catches the `Rc` the types cannot show, such as one captured by a closure that reaches `spawn` as a plain `fn`
   value. `Arc` is not affected. The cost is one word a box and one load, and a call while the program has had one
   thread, where the thread's number is not asked for.
2. **`remove` keeps its order and is fast in any order (045).** The removed entry's place is left empty, its key
   destroyed and its value returned at the call as before, and the empty places are closed up together: when they
   outnumber the entries, when the table grows, at `clear`, at the end of a consuming loop and at the map's end. Until
   then the places are counted in a tree, so `key_at`, `value_at` and `foreach` find the i-th entry in logarithmic
   time, and in constant time when there are none. Closing them at the next positional call was not possible: those
   calls take the map shared, and moving entries under a live `get` would leave its reference pointing at other
   memory. The map has a destructor of its own for the places.
3. **A callee's stores are summarised (014).** A callee given an argument exclusively is judged by the parts of what
   the argument refers to only where a summary of its body says it can store a reference into such a part where
   another argument reaches (a direct store, or through a call it makes; calls through `fn` values, closures and C
   are taken to store anything). The summaries are settled by a fixed point over the program's bodies, so order and
   recursion do not matter. An argument given shared, and a reference into a part of what an argument's referent
   itself keeps, are judged by their parts as in Update 203. A program that is not decided keeps its records and
   faults as with every check on.
4. **An array length is any constant (013).** `sizeof`, `min_value`, `max_value`, `widen` and `narrow` of the integer
   types and `sizeof` of `f32`, `f64`, `bool` and of a plain struct declared in the file may be used in the length of an
   `array<T, N>`, directly or through a `const`, and a failure is reported where the length is read. After the push,
   `sizeof` and `alignof` of any type the program lays out (generics' instances, `align(N)`, references, containers,
   enums, types of other modules) may be used too: such a length is computed once the program's items are known, the
   text parsed again with it until the lengths settle. A type parameter may not (one length serves every instance), nor
   a length that depends on the size of a type holding it; the message says so.
5. **A deep chain is cloned with a loop (022).** A type that links to itself once through `Box<Self>` or
   `Option<Box<Self>>` has a derived `clone` that recurses for the first 1,024 links and copies the rest with a loop.
   A tree of two children per node, and a struct owning something on both sides of its link, still recurse, to the
   depth of their height. In the bootstrap form the destruction of a chain whose links were borrowed (by a walk or a
   clone) no longer recurses.
6. **A foreign C thread may call an entry (001).** A program with entry points makes the runtime ready for threads it
   did not start: such a thread is registered on its first call and released when it ends, and the runtime's locking
   stays on for the whole run (a thread arriving while `main` runs unlocked would race with it).
7. **The generated C nests as the source does (020).** A last pass on each function's C removes the braces of bare
   blocks that name nothing, so 200 nested `if`s, loops and blocks compile with Clang's default depth.
8. **Windows and the cleanup (032, 037).** On Windows a stack overflow on a registered thread is contained as
   `diag.stack-exhausted` (a vectored exception handler, a guarantee of room for it, the guard page made again), the
   program `cobc --run` starts ends with `cobc` (a job object) and has `COBALTC_CPU_LIMIT`, and a console event removes
   the directories. A thread's signal stack is freed when it ends. On Unix `cobc` removes its directories from the
   moment they exist, a program run under a runner and the C compiler `cobc` started are sent the signal, and
   `--explore` runs are covered. The Windows parts were tested under Wine only.
9. **Messages that fit (029).** An integer literal beyond `u128` has its own diagnostic and repair line, and the
   commonest static diagnostics (`diag.type-mismatch`, `diag.syntax-error`, `diag.borrow-of-non-place`,
   `diag.name-not-visible`, `diag.unbound-name`) have repair lines that name what to write.

## 2. Consequences

- A program that used an `Rc` from a thread other than its maker's through a hidden path now faults where it raced.
- The compiler's own check of its source takes about 1.3% more instructions (the indexes that make 021 and 044 scale).
- A `HashMap` has a destructor, and a loop over it a hidden binding for the places; a map with no empty place costs a
  comparison more in `foreach`, `key_at` and `value_at`.
- A program with entry points keeps the runtime's locking on.

## 3. Verification

See `CHG-0476`.
