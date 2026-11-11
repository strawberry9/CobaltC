# CHG-0438 — Gaps in the static checker, and the decided programs they made unsafe

Status: implemented (2026-11-12), D-0296; Update 182
Affects: `spec/11` §3 (`[Definite-Assignment-Rejected]`), `spec/12` §1 (`[Type-Arity]`), `spec/conformance.md`;
`impl/src` (`typecheck.rs`, `modres.rs`, `value.rs`, `interp/`); their twins in `impl/selfhost/front`
(`typecheck.cb`, `modres.cb`); `stress/checker_gaps/`

## Why

The owner, 2026-11-12, after `i32 x = None;` was found accepted: "fix the None bug too, look for other problems
with the static checker also, and fix them as well". About 600 small programs probed the checker
(`stress/checker_gaps/`, the minimal ones under `probes/`). Each finding below was a program `--check` accepted
that `coby` then faulted on, or that `cobc` stopped on with an internal error, or that the two ran differently.

The worst were not type errors. Where the static pass found nothing left to the run, `cobc` built the program
*decided* (D-0284): no record of its references. Four shapes the checker passed by deferring them to the run's
check (spec/10 §2, "unknown" referent) were also not reported to the run, so the compiled program read a dead
stack slot or freed heap where `coby` faulted. A gap in the checker was a memory error in a decided program.

## What changes

**Left to the run, and said so** (`rule.temporal.ref-escape`, spec/10 §2; D-0284). A result that is a reference,
a slice, a view, an `Option` of one or a closure, whose borrow no visible `&` shows (`r` set by an `if`, held in an
`Option`, copied through a binding), and which may refer to a binding of this frame, is reported to the run; so
is such a reference stored into a binding that outlives one of its possible referents. What "may refer" follows:
a fact's root that is a local, or `*r` of a local reference whose own facts reach one, or a local that is itself a
reference or stores references, through its facts. A reference read out of a parameter's container (tier D) is
not this frame's and stays decided. A part of a `foreach` over a splitter's call (D-0287) reaches the text the
loop's holder views: that holder, a `Parts`, is copied as a view is (`$each_len($d)` was taken to move it, and
the parts then reached nothing known), and a stored reference's check compares the blocks of the bindings its
facts reach, so `kept = w` inside a block whose text ends first is reported (the compiled program had printed
freed memory). Swept over the 186 programs of the showcases, the benchmarks,
`stress/vsrust`, the book's examples and `impl/cobaltc_examples`: the same 162 decided as before, the same 6 not.

**A returned closure borrows its reference captures' bindings.** `[p]() : i32 { *p }` returned from a function
whose parameter is `p` returned a borrow of `p` itself, which ends with the call: `diag.reference-escapes-scope`.
`move [p]` copies the reference, as before.

**Definite assignment** (`spec/11` 1.5.0). A borrow of a binding with no value, and a closure's capture of one, is
`diag.use-of-uninitialized`. The right operand of `&&` and `||` runs on some paths, so its writes hold on those
alone (`true || { x = 1; true }` left `x` without a value).

**Types.**
- A variant is a value of its enum (`[T-Enum]`): where a known type is expected that is not that enum, `None`,
  `Ok(3)` or `B::Z` is `diag.type-mismatch`. A payload-less variant, or one whose payload fixed only some type
  arguments, had no type, and every comparison skipped it (`i32 x = None;`).
- A variant's qualifier names its enum: `Foo::Y`, `Result::Some` and `Option::X` are `diag.unbound-name` (the
  variant was taken by its last name alone).
- `[Type-Arity]` (`spec/12` 1.31.0): `Option<i32, i32>` and a bare `Vec` are `diag.type-mismatch`.
- An integer literal is not a float as an argument either (`half(3)`, through a `fn` value too), as it is not
  anywhere else (D-0006).
- A struct literal of no struct is `diag.unbound-name`.
- `[Generic-Call-Uninferable]`: a value whose type argument nothing fixes, where nothing later can (a statement,
  a `printf` argument, a `match`'s scrutinee, a binding without a declared type whose value has a partly open
  type, a generic call whose untyped arguments are all such values): `diag.cannot-infer-type-parameter`. A
  variant compared with a value of another type (`1 == None`) is `diag.type-mismatch`.

**`?`** returns from the closure around it, so it is checked against the closure's result, and `None?` or
`Ok(3)?` against the enclosing result's kind (`diag.propagate-outside-fallible-context`).

**`printf`**: `%v` of an enum that holds a resource (`Option<String>`) by value is `diag.read-of-resource`; `&o`
prints it.

**`coby`**: a `fn` value of a generic function's instance (`sq<u8>`, or `sq` where `fn(u8) : u8` is declared)
keeps its type arguments, so its body runs at them (`id<u8>(255) + 1` printed `256`). A fault while a nested
block's objects end (`{ … }` as an expression) is located at the block, as `cobc` locates it: `coby` took the line
it last evaluated, so a block whose last statement is `u = t;` put the fault on that line, and the compiled suite
found the two runs' messages apart.

## Not changed

(`CHG-0439` fixed the first, second, third, fifth and sixth of these; the open-type forms stay.)

- An exclusive reference copied into a struct literal (`H { .m = m }`) and then used beside `m` is still accepted;
  both tools run it alike. Recorded in `stress/checker_gaps/LOG.md` as open.
- A pattern's qualifier (`B::X :` in a `match` on an `A`) is not checked: the parser keeps only the variant's
  name. Both tools match by that name alike. Open.
- An arm after an irrefutable struct pattern is not reported unreachable. Open.
- A closure that borrows a local, stored through a call (`Vec::push(&mut fs, [n]() …)`), stays the run's (spec/10
  §2, by design).
- Some open type arguments `coby` runs and `cobc` cannot type, all odd code: a generic struct literal with an open
  field bound with `auto`, `foreach (x in [])`, `[None, None]`, `drop(None)`, `Mutex::new(None)`, `{ None }`, and
  two literals compared (`None == None`, `Some(1) == None`). Open.
- `?` in a closure whose result type is not written (`[o]() { i32 v = o?; … }`, its body a statement) is not
  checked against the result the body gives: `coby` returns `None` from it, `cobc` cannot type it. Open.
- `a >> 3` on a `T: integer` is rejected, with empty type names in the message: a valid program refused. Open.

## Measured

Every finding's program gives the new verdict under `coby`, `cobc` (GCC, Clang) and `cbc`. The decided sweep
above. The conformance suites and the gate.
