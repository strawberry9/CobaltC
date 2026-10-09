# D-0200 — Compile-time discharge: far more proven at compile time, far less tracked at run time

Status: ACCEPTED (2026-10-09; the owner: "One of the things that will make CobaltC unusable is its runtime costs. Rust, as I understand it, does a great deal more work at compile time. CobaltC needs to do this as well. … I need you to focus on the significant project of getting the compiler to do far more work at compile time, and far less work at runtime")
Kind: Decision (`spec/02-schema.md`)
Governed by: `CobaltC_Master_Instructions.md` §1, §6, §17
Depends on: `spec/14` §6 (Discharge), D-0018 (no check counts an unheld path), D-0107, D-0111, D-0188, D-0191, D-0196, D-0199
Affects: `impl/cobc/src/lower.rs`; `impl/cbrt/src/lib.rs`; `impl/cbrt/include/cbrt.h`; `spec/conformance.md`; `CHG-0228`

## Problem

A compiled program that works on text or containers spends about 90% of its instructions in the runtime that tracks
`spec/04`'s dynamic state: objects, paths, holders, statement and block scopes. Only about 3% are its own code
(callgrind on csvstat, extsort and wordpar, `stress/perf_oct9`). The cost is spread over about twenty kinds of event,
none above 10%. Making events cheaper gives tens of percent at best (D-0199, and an inline fast-path attempt that was
reverted). The gap to Python (12 to 34 times) closes only if the compiler proves most events unnecessary and does not
emit them.

`spec/14` §6 already permits this: a check the compiler has proven is not performed. Until now cobc proved checks
away only in special cases:

- plain-data locals (Stage 3);
- `String` locals lent only to readers and to a fixed list of builders (D-0196);
- pure direct and pure view parameters (D-0191);
- confined vectors and dual bodies (D-0188, D-0189).

## Decision

The compiler proves, per function and across calls, which values and references can never take part in a dynamic
fault, and gives them no runtime existence. What remains tracked at run time is only what the proofs cannot cover.
No rule, diagnostic or observable behavior changes: every program faults where and as it did.

The proofs are conservative and syntactic over the program and `std` (whose source is CobaltC). Any construct a proof
does not recognise falls back to today's tracked lowering. A proof's result is token 0 (`cbrt`: no path; every check
through it is the no-op the proof makes it), or no object at all.

### Phase 1 — lent parameters and objectless locals (this change)

1. **Writer parameters** (`Gen::writer_param`). An exclusive reference parameter that the body only reads and writes
   through (a resource assigned through it is excluded), and lends on whole or by field to reader or writer
   parameters, one at a time. No path derived from it outlives the call, and no two exist at once. This is the
   counterpart of D-0196's readers.
2. **Objectless locals of any container type** (`Gen::objectless_type`). Covered: `String`, `Vec`, `Queue`,
   `HashMap`, `HashSet`, and plain structs of these, over plain data. Such a local whose every use lends it to a
   reader or writer parameter, or moves it away, gets no object. Its borrows are token 0, its block owns nothing,
   and its end is `cb_drop_local`.
3. **Moves of objectless locals** anywhere (a by-value argument, a literal's part, `let y = x`, `return`). A C flag
   marks the local moved. A use after the move faults `diag.stale-binding`, as the binding's object would have, and
   the drop is skipped. The moved value gets an object where it leaves.

### Phase 2 — proofs across calls, held references, results without objects (CHG-0228)

Each proof below removes runtime events where it applies and changes nothing a program can observe. The same
faults are raised at the same points.

1. **Held references to elements.** `ref<T, m> r = &x[i]…` (or `&mut`) into a local vector (`Gen::elem_ref_lets`).
   The proof needs four things: `x` is a local, `x` is named nowhere else in the rest of the block, `r` is used only
   as a reader or writer, and `x`'s borrow and each element's access are checked where `r` is formed. Then `r` needs
   no path: token 0.
2. **Held shared `Vec` references read only** (`held_vec_reader`), and `foreach` element aliases over an exclusive
   holder. While the shared path is held, any conflicting access faults at that access. So the reads through it
   need only the bounds check.
3. **Quiet container locals** (`Gen::quiet_locals`). A local container is never moved and never held exclusively
   in the function's own code. Its exclusive borrows are arguments to writer parameters, or element references with
   no path. Then its shared checks cannot fail and are not made.
4. **Readers and writers, extended.**
   - Aliases a reader derives from its parameter (`let a = &r.f`, `foreach` over `&r.f`) are followed.
   - `String::as_view(r)` handed to a pure view counts as a read.
   - `from_view(view(r, …))` is a read of `r`.
   - A fused text loop over a view parameter is a pure use.
   - A writer's compound assignment and a `match (&r.f)` on it are writes and reads through the parameter.
   - `HashMap`/`HashSet::insert` take their container as a writer.
5. **Read-only closures are pure.** A closure capturing nothing, or capturing only what it never writes, whose
   reference parameters are shared and only read, is lowered with bare addresses and marked pure. `sorted_order` and
   `PriorityQueue` then call it natively. A `fn` given by value is dropped where the body would drop it.
6. **Results with no object.**
   - `__raw` forms now exist for `std`'s non-generic functions and for natives.
   - The last `match` of a `__raw` form collects its arms' values with no object (a raw sink).
   - Copies (`clone`, `from_str`, substrings) are made with no object where they go straight into such a sink, a
     `String` field of a literal, a `Vec::push`, a `HashSet::insert` or a `Channel::send`.
   - Objectless match binders: a payload of an objectless type, taken by value.
   - `replace(&mut x, String::new())` pushed whole (`take`).
7. **Checks through token 0 folded** (`fold_zero_tokens`). A check through a token known to be 0 is a no-op in
   `cbrt`, so it is removed before scopes are resolved. A scope left with nothing to record then goes as well.
8. **Natives** that do the body's checks in its order, the rest directly:
   - `Channel::send`/`recv`: `cb_lock_bare`, `cb_unlock_bare`, `cb_unlock_signal`;
   - `File::write`, `File::write_text`, `read_line`, `File::read_line`;
   - `Result::unwrap_or`, `Vec::sorted_order`;
   - `*Option::unwrap(HashMap::get(…))` reads the value in place.

   Objectless `String`/`Vec` locals are freed inline (`cb_drop_plain_buf`).

Withdrawn as unsound, both under D-0107:

- *Unstored parameters for several "flat" references*: two such parameters may overlap. D-0107 faults at the first
  access through either, and that access counts only held paths.
- *An unminted view beside any exclusive parameter*: kept only for exclusive referents made of scalars, where no
  view's text can lie.

Measured (`stress/perf_oct9/bench.sh`, cobc + gcc, best of 3; Python 3 alongside):

| program | before D-0199 | after Phase 2 | Python |
|---|---|---|---|
| csvstat | 3502 ms | 246 ms | 231 ms |
| extsort | 11422 ms | 996 ms | 174 ms |
| wordpar | 5102 ms | 542 ms | 167 ms |

What remains is not compile-time discharge, and each part is the owner's to decide:

- `File` writes are unbuffered: one system call per `write_text`, half of extsort's remaining time. Buffering them
  would be a `std` decision like D-0178's for standard output.
- `cbrt` takes one lock for its whole state on every entry while a second thread runs. wordpar's channel stage
  contends on it.

### Later phases (each its own change record, each measured)

Phase 2 covers the element borrows, by-value transfers and scopes this list began with. Still open:

- **Parameter bindings.** A lent parameter's binding object is made unless the `unstored` rule applies (one
  reference parameter, or all shared). Going further needs to know which parameters can overlap. D-0107 faults
  that overlap at the first access in the callee, so it cannot be judged at the call.
- **Exclusive checks on locals.** Shared checks on quiet locals are gone (Phase 2). An exclusive check also
  fails on a held shared path, so removing it needs a dataflow analysis of which paths are held where.

## Verification

Each phase is checked by parity between `coby` and `cobc` (gcc and clang) over the whole conformance suite, by the
round-8 programs against Python's output, and by new conformance cases at the proofs' edges: a use after a
conditional move, aliasing through two lent arguments, and writes of resources through a lent parameter.
