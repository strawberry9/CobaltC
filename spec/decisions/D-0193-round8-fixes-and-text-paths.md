# D-0193 — Round-8 findings: closures capturing closures, closure names, text and queue paths, `swap_remove`, `Response::text`

Status: ACCEPTED (2026-10-09; the owner: "continue writing real world programs that stress and test the language and its libraries … Resolve all issues yourself where you can")
Kind: Decision (`spec/02-schema.md`)
Governed by: `CobaltC_Master_Instructions.md` §1, §6, §9, §17
Depends on: D-0089, D-0137, D-0173, D-0191, D-0192
Affects: `impl/src/interp.rs`; `impl/cobc/src/lower.rs`; `impl/cbrt/src/lib.rs`; `impl/std/collections.cb`, `impl/std/http.cb`; `spec/21` (4.46.0); `spec/conformance.md` (3.186.0); `CHG-0221`

## Problem

The round-8 programs (`stress/round8`: a search index, JSON with compression, a parallel word count, an event
simulation, an HTTP key-value service, a query engine) found:

1. **cobc:** closures were numbered per function, so two functions' closures could share a C name
   (`conflicting types for 'cl_7'`): any program could fail to compile.
2. **coby:** a closure captured by another closure lost its capture types (they were recorded per object, and the
   capturing closure or a call's view of it is a new object): calling it was a run-time `diag.type-mismatch`. A
   `move` capture of a `fn` value whose closure owns a resource copied it (its destructor ran early) or, once moved,
   never destroyed it, because the capture's type, `fn(…)`, says nothing about what the closure owns.
3. **cbrt:** copying a closure that captured a closure shared the inner box between the copies; when one ended, the
   other's slot was empty (`diag.stale-binding` at its next call). Ownership (D-0089) was not seen through nesting.
4. **Speed:** text loops spent microseconds a word in `std`'s byte-at-a-time bodies (`to_ascii_lower`,
   `split_whitespace`, `trim`), `foreach` over a `Vec` value paid the object protocol per element (`Drain::take`),
   and `PriorityQueue` 4–12 µs an operation; several threads using the runtime slept and woke in the kernel for each
   short critical section.
5. **Frictions:** no `Vec::swap_remove` (`HashMap` and `HashSet` have one); a `Response`'s body as text needed
   `String::from_utf8(replace(&mut r.body, Vec::new()))`.

## Decisions

1. Closure C names carry a hash of the C function being emitted, beside the per-function number: unique in the
   program, and each function's text still independent of the rest (`cache.rs`).
2. **coby:** capture types are also kept per closure literal and used where an object has none; a closure capturing
   (by `move`) a value that holds a resource-owning closure owns one itself; a resource-owning closure object captured
   by `move` moves as a handle to itself, ending when that handle is dropped.
3. **cbrt:** `cb_fn_copy` copies the closures a closure captures (each copy owns its own); `fn_owns_resource` looks
   through captured `fn` values.
4. **Natives (cobc), each making the body's checks in its order:** `StringView::split_whitespace`, `trim`,
   `trim_start`, `trim_end`; `String::to_ascii_lower`/`upper`; `Drain::take` for plain elements and `StringView`s;
   `PriorityQueue::push`/`pop` for plain elements (the same comparisons in the same order; a `less` whose closure may
   keep its arguments, or a borrowed element, takes the prelude's body). `rotate_left`/`rotate_right` are emitted
   at the operand's width (`cb_rotl32`, …), which C compilers make one instruction of. The runtime lock spins (with a yield every
   256 tries) before it sleeps.
5. **std:** `Vec::swap_remove<T>(ref<Vec<T>, exclusive> v, usize i) : T`,
   `Response::text(ref<Response, shared> r) : Result<String, Utf8Error>` and `StringView::char_count` /
   `String::char_count` (`len` counts bytes; counting characters built a `Vec` of views), each naming one common
   intent (§9).
6. **Reader slices (cobc):** a local shared slice the function never assigns, borrows, captures or drops reads its
   elements unchecked, as a pure slice parameter does (its path cannot end while it is live, `spec/08` §4).
   `StringView::chars` is native, as `split_whitespace` is. `StringView::parse` parses in place (no `String` copy),
   and `parse<T>` of an integer `T` is native in `cobc` (1.2M numbers 4.0 → 1.3 s).

## Results

`wordpar` (1.4 MB, one worker) 10.6 → 5.0 s, eight workers 24.5 → 8.9 s; `to_ascii_lower` of 31k lines 428 → 44 ms;
`foreach` over a split line's words 1,241 → 613 ms; `PriorityQueue` 4.2 → 0.6 µs an operation (with a comparator
11.7 → 0.6); `eventsim` (200k customers) 13.3 → 2.0 s; `httpkv` with eight clients 5.4 → 3.7 s; SHA-256 64 → 78 MB/s (gcc), 87 (clang).

## Not decided here

`HttpClient` keeps one connection per request (D-0173); keep-alive would be a client change of its own. Threads that
all use the runtime heavily remain slower together than one alone: that needs per-thread runtime state.
