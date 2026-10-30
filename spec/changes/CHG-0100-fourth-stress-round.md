# CHG-0100 — Fourth stress round: more temporaries as arguments, `Vec::from_slice` and `Vec::truncate`, `Rc`'s count, and fixes

Kind: Change record (`spec/02-schema.md` §2)
Status: ACCEPTED (2026-09-28, stress round; D-0084, D-0085 and item 15's tightening confirmed by the owner)
Governed by: `CobaltC_Master_Instructions.md` §8, §9, §12, §19, §21
Depends on: D-0084, D-0085
Affects: rule.ref.form, rule.fn.generic-call, rule.stdlib.vec, rule.stdlib.rc, the implementations

## Problem / motivation

A round of real programs (SHA-256, a JSON parser and printer, big
integers, a regular-expression engine, a Huffman coder, a Lisp, CSV,
generic containers, a directory tool, a bytecode compiler and machine,
Markdown, data-parallel slices, statistics, closures, a log-structured
key-value store, order processing, graph algorithms) found D-0084's
and D-0085's frictions and these faults:

1. **`std`'s `Rc::clone` and `Rc::drop` borrowed the whole box**
   exclusively to change the count, which clashed with a live `Rc::get`
   reference into the value through another handle (`cobc`
   `diag.aliasing-conflict`, `coby` `diag.destroy-while-aliased`).
2. **cobc** lowered an `if`'s then-branch from a copy of it, so the
   checker's records, kept per expression, were not found there: a view
   `&s[a..b]` of a `String` inside an `if` stopped with an internal
   error.
3. **cbrt** ended a reference handed out as a block's result when the
   block's local it was copied from ended, if no statement owned its
   path (one formed long before, read out of a buffer's cells):
   `foreach (r in v)` over a `Vec` of references faulted
   `diag.stale-binding` (`Vec::take_raw`'s `unsafe` block).
4. **cbrt** looked down the whole frame stack for the nearest statement
   scope at every object it made: a recursion through calls made as
   whole statements pushes frames only, so a recursion 96,000 deep
   (Tarjan's algorithm) spent 70% of its time there (143 s; now 31 s).
5. **cbrt** gave a destructor the caller's queue of values in transit:
   `return Err(sprintf(…))` from inside a `foreach` still holding
   `StringView`s sent the returned `String`, then destroyed the rest of
   the loop's `Vec`, whose destructor's own calls took the returned value
   as theirs and stopped the program. A destructor now runs with an empty
   queue, and the caller's is restored after it.
6. **cobc** read a `fn` value held in a field, an element or `*r` before
   calling it (`(t.run)()`), which for a box whose closure owns a
   resource is a read of a resource (`diag.read-of-resource`); it is
   called in place, as a binding holding one already was.
7. **coby:**
   - a thread's result was typed from the value alone, which does not
     say which struct it is, so `[join(t)]` gave `void` elements and a
     field read faulted `diag.type-mismatch`; it is typed by the
     callee's declared result type (a function's, or a closure's);
   - the payload of `f()?` when it was a struct or an array, likewise
     (`foreach (x in list_dir(d)?)`);
   - `(t.less)(a, b)` called through a reference to a struct holding a
     `fn` value, and `(*f)(x)` with `f` referring into a `HashMap`'s
     value, took the callee to be the whole object;
   - layouts were recomputed at every place read, and every aliasing
     check copied every live reference in the program: now layouts are
     cached and a check looks only at references to its own target.
8. **The checker ignored type arguments a function does not take**: a
   program's own non-generic `parse` hides `std::parse` (D-0024), and
   `parse<i64>(&s)` quietly called the program's. `[Generic-Call-Arity]`
   (`spec/15`) rejects them; the message points at `std::parse<…>(…)`.
9. **The checker took `Option::unwrap(HashMap::get_mut(…)).items` for
   part of a temporary**: it read only a function's declared result type,
   and `unwrap`'s is a generic `T`. A reference-typed result is now seen
   through `unwrap`/`expect`, as `[Ref-Form-Temporary]` states.
10. **A literal *expression* next to a typed operand was computed as an
    `i32`** (only a bare literal took the other operand's type):
    `k & ((1 << 40) - 1)` with `k : u64`, as an argument or in `x |= 1 << n`,
    faulted `diag.shift-amount-out-of-range` in both tools, and the checker
    rejected `((1 << 40) - 1) & k`. `rule.type.expected` makes the other
    operand's type the expected type there, and D-0037 carries it into
    literal expressions; now all three do (a shift's left literal takes
    the context's type, not the amount's).
11. **cbrt kept a reference slot a place no longer had**: overwriting
    `Some(r)` with `None` (by assignment, by `replace`, or in a channel's
    ring) left `r`'s path counted as held, so destroying its referent later
    faulted `diag.destroy-while-aliased`. The runtime now drops the
    destination's old slots whenever a value holding references is stored.
12. **cobc's compile time**: every emitted location counted the lines of
    the whole `std` prelude again; 800 locals summed in one expression took
    95 s to compile, 120 nested `if`s 15 s (now 2.8 s and 1.2 s).
13. **coby typed an array literal's later elements without the first's
    type**: `[0: u64, 999999999999]` faulted `diag.literal-out-of-range`.
14. **The checker let `x.f` through on an enum or a primitive value**:
    `Vec::pop(&mut v).unwrap()` (method-call syntax, which CobaltC does
    not have) reached coby as a dynamic `diag.type-mismatch` and crashed
    cobc. It is now a static `diag.type-mismatch` that names
    `Option::unwrap(x, ...)` when `T::f` is a function.
15. **A closure's borrow of its captures was checked only at run
    time**: `auto bump = [count]() : void { count += 1; }; count = 10;`
    faulted `diag.aliasing-conflict` when run. A binding holding a
    closure without `move` now carries `deriv` facts for its captured
    locals, as a spawn's handle does for its arguments (spec/14 1.13.0),
    so the write is refuted statically. A tightening: a program whose
    conflicting write was never reached is now rejected.
16. **`return` inside a closure's body** ended the function the closure
    was called from in coby (`main` returned the closure's `false`,
    printing nothing), and the checker typed its value by the enclosing
    function's result (`return 300` in a `: u32` closure inside
    `fn main() : u8` was `diag.literal-out-of-range`). A closure's call
    is a call of its body (`[Closure-Call]`): both now end that call and
    type the value by the closure's result.
17. **cobc crashed calling a closure through a reference**: `&g`, `g`
    an `auto` binding of a closure, passed where `ref<fn(…) : R, m>` is
    expected, handed over the closure's own storage where the callee
    reads a `fn` value. A closure binding the body borrows is now held
    as a `fn` value (`lower.rs`, `Stmt::Let`).
18. **cobc could not infer a type parameter that a later argument
    fixes**: `count(Vec::new(), label)` stopped at `Vec::new`, and
    `spawn(run_chunk, v, f)` (a generic function spawned without type
    arguments) at `run_chunk`. Both now read the later arguments' types
    first (probed, so the evaluation order is unchanged), as coby does;
    only names and their fields, elements and borrows are probed (a
    literal would take its default type, a closure would be formed
    twice). A payload-less variant waits for the other arguments, as an
    unsuffixed literal does (`Option::unwrap_or(None, 7)`, which cobc's
    spec-row run had skipped).
19. **coby's memory grew with every borrow through a reference**: the
    ancestors recorded for a token (`token_ancestors`) were never
    dropped (~144 bytes each; 100k `HashMap::get` calls reached 98 MB).
    A token's ancestor set is now carried by the values holding the
    token and held weakly by the table, which drops dead entries as it
    doubles (7.5 MB for the same run).
20. **Diagnostics without a message or a line:** a move out of a field
   in a function's result expression had no location;
   `diag.overwrite-of-live-resource` and
   `diag.cannot-infer-type-parameter` now name the variable and type,
   or the function and its unfixed parameters. The static aliasing
   diagnostics name the reference that still holds the borrow (`first
   still borrows names`, `the foreach loop over v still borrows it`),
   and a use after a move says so. `diag.format-invalid` says which
   specifier is wrong and why; an unbound name or type that is a private
   item of another module says it is not exported; `T[N] name` in a
   constant, a field or a parameter, and `S { f: v }`, name the forms
   CobaltC uses; a built-in type's name used as a name (`u32 slice`,
   `fn f32(…)`) says it is reserved; `diag.unbounded-type-parameter`
   names the operation and suggests passing it in as a `fn`. `&f()[lo ..
   hi]` no longer says a temporary may be borrowed as a call's argument
   (a slice of one may not). `diag.literal-out-of-range` names the
   literal, the type it took and its range, and says when that type is
   the `i32` default (`to_float<f64>(18446744073709551615)`). A
   returned reference to a local, a resource moved out of a `Vec`
   element, a write through a shared reference, taking apart a struct
   with a destructor and a resource payload bound through a reference
   now say what happened and what to write instead; `[&x]` in a capture
   list says captures are bare names. A constant operation that
   overflows names its operands and the type's range; a read that may
   be uninitialized, a type that contains itself by value, a value on
   the left of `=` (a literal, a call, a constant) and `if (x = 6)`
   each say what is wrong and what to write. A value of the wrong type
   in an initializer, an argument or a comparison names the conversion
   (`widen`/`narrow`/`to_float`/`to_int`, `3.0`, `String::from_str`) or
   the borrow (`&mut a`) that fits; `+` on text names `String::append`
   and `sprintf`; a variant that carries a value written bare says to
   write `A(value)`; a `std` type named without `import std;` says so; a
   format argument names what its specifier takes; a function returning
   a reference with zero or several reference parameters is named, at
   its line, with those parameters; a missing or twice-declared module
   file names the file.

## Decision

D-0084, and each tool brought to the specification.

## What changed

- **`spec/09` 1.2.0:** `[Ref-Form-Temporary-Argument]` (D-0084).
- **`spec/14` 1.13.0:** `deriv` facts for a closure without `move`.
- **`spec/15` 1.9.0:** `[Generic-Call-Arity]`.
- **`spec/21` 3.34.0:** `Rc::clone` and `Rc::drop` borrow `count` only.
  **3.35.0:** `Vec::from_slice`, `Vec::truncate` (D-0085).
- **Implementations:** as listed above (`coby`: `src/typecheck.rs`
  `is_temp_value`, `check_store_operand`, the two messages;
  `src/interp.rs` `eval_temp_borrow`, `call_closure`'s `return`, spawn's `declared_ret`,
  `eval_propagate`, the callee's place, `struct_layouts`/`enum_layouts`,
  `scan_refs_to`, `check_field` on enums and primitives; `cobc`: `lower_branch_block`, `probe_block_type`, the
  borrowed literal's type, a `fn` callee in place; `cbrt`:
  `cb_result_ref`, `TState.scopes`, a destructor's own in-flight queue,
  `cb_copy_datum` dropping stale slots; `coby`/`cobc`/checker: literal
  expressions as operands; `prelude::line_count` computed once).
- **Guide:** §09's "A temporary as an argument" shows an operator's
  result and a literal.

## Compatibility classification

Loosening (D-0084) and additions (D-0085); `[Generic-Call-Arity]` and the closure `deriv` facts are tightenings (a program passing type arguments a function does not take, or writing a captured variable while the closure holding it exists, is rejected); the rest are fixes.

## Conformance changes

**Added:** `conf.temp-borrow-operator-argument`,
`conf.temp-borrow-literal-typed-by-parameter`,
`conf.rc-clone-while-value-borrowed`, `conf.vec-from-slice`,
`conf.vec-from-slice-resource-rejected`, `conf.vec-truncate`,
`conf.vec-truncate-beyond-length`,
`conf.generic-call-extra-type-arguments-rejected`,
`conf.field-through-unwrapped-reference-borrowed`,
`conf.field-of-enum-rejected`,
`conf.closure-capture-written-while-held-rejected`, and twenty-six `.cb` cases under
`impl/conformance/`.

**Changed:** `15/closure_exclusive_capture_blocks_owner_rejected.cb`
and `15/closure_shared_capture_blocks_owner_write_rejected.cb` expect
`diag.aliasing-conflict (static)` (was `(dynamic)`): item 15's `deriv`
facts refute them.

## Revisit conditions

None.
