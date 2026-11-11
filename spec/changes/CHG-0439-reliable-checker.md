# CHG-0439 — The checker's open findings, and the gaps a second probing found

Status: implemented (2026-11-12), D-0319, D-0296; Update 183
Affects: `spec/10` 1.7.0 (`[Call-Multi-Ref-Return-Rejected]`, `[Call-Elided-Lifetime]`, `[Result-Source-Probe-Only]`), `spec/14` 1.46.0 (§6),
`spec/15` 1.21.1 (§6), `spec/16` 1.34.0 (`[Pattern-Type]`), `spec/registry/diagnostics.md` 1.67.0,
`spec/OVERVIEW.md` 1.3.3, `spec/registry/features.md` 1.10.0, `spec/conformance.md` 3.308.0; `impl/src` (`ast.rs`, `parser.rs`, `derive.rs`,
`modres.rs`, `typecheck.rs`, `interp/calls.rs`), `impl/model/src/statics.rs`, `impl/cbrt/src` (`lib.rs`, `replay.rs`), `impl/cobc/src/lower/` (`exprs.rs`, `user_calls.rs`); their twins in `impl/selfhost` (`front/ast.cb`,
`parser.cb`, `derive.cb`, `modres.cb`, `typecheck.cb`, `tier.cb`, `back/gen.cb`); seven `std` signatures, two `std`
bodies; `stress/checker_gaps/`; the guide and the book

## Why

The owner, 2026-11-12, after Update 182: "fix the five open checker items and also continue focusing extensively
on furthering the correctness of the static checker, and fixing all issues that arise autonomously. I need a
really reliable static checker that programmers can rely on to not accept incorrect code."

CHG-0438 left five findings open. A second probing (about 200 programs, `stress/checker_gaps/`) looked where the
first had not: references through `fn` values, block expressions, closures and calls whose results hold
references; copies of exclusive references in aggregates; moves inside closures. As before, the worst findings
were programs the static pass passed with nothing left to the run, so `cobc` built them *decided* (D-0284) and the
compiled program read a dead stack slot where `coby` faulted.

## What changes

**A result that holds references names its sources** (D-0319, `spec/10` 1.7.0). With two or more reference
parameters, a result that holds references without being one (`Option<ref<T, shared>>`, `Vec<StringView>`, a
struct with a reference field, a closure) needs a `from` clause, as a reference result does
(`diag.lifetime-elision-ambiguous`). Its call was taken to hold what the first reference argument reaches, and a
clause that was written was not read: `from b` on an `Option<ref<…>>` still borrowed `a`, and a decided program
read `b`'s dead slot. A call's facts are now the named arguments'; and such a result, from one parameter or a
clause, stored past its source's block is `diag.reference-escapes-scope` before the run (it was the run's).
`std` names its sources: `StringView::strip_prefix`/`strip_suffix` `from v`, `Json::get`/`lookup` `from j`,
`Toml::get`/`lookup` `from t`, `http_date_words` `from text`; `Json::lookup` and `Toml::lookup` walk their path
by position (a local `Vec` of its parts kept it). The showcase's `Map::get` and five functions of `impl/selfhost`
take a clause too.

**`[Result-Source-Probe-Only]`** reads the type arguments a call writes (`StringView::parse<usize>(q)` holds no
reference), and the parameter of a call that may receive references does not keep the argument it is given
(`Vec::push(starts, i)`).

**An exclusive reference copied into an aggregate** (F09). `H { .m = m }` (or `[m]`) is an exclusive borrow of
`*m`, as `n = m` is: `*m = 5` while `h` is used is `diag.aliasing-conflict`. And a struct's one reference field
holds what the struct surely holds, so `two(m, h.m)` is refused: it passed one exclusive reference twice, which
both tools ran (at run time the two arguments are one reference). Two bindings whose last mention is one
statement end together (`spec/14` `[Ref-Binding-Last-Use]`: one passed over is judged again once a later one
has ended; `h` held `*r`, so `r` waited for `h` and outlived it). The executable model (`impl/model`) reads
both as the tools do.

**A call through a `fn` value or a closure** holds what its reference arguments reach and what the closure holds.
It was taken for static data: `r = f(&y)` past `y`'s block was decided, and the compiled program read the slot.
**A block expression** that gives its own binding (`r = { ref<i32, shared> t = &y; t }`) was taken for static
data, and **a closure** stored past what it captured (`g = move [p]() …`, `p` into an inner block) was not checked.
A block expression's own bindings are now read through their initializers: one that is itself a reference
(`ref<…> t = &y`, `auto t = Box::get(p)`) stands for what it borrowed, so the first store is
`diag.destroy-while-aliased` before the run, and `{ ref t = p; if (…) { Some(&t[0]) } else { None } }` holds
what `p` does. A binding the block writes after its initializer (`t = &y`, `h.p = &y`, `Vec::push(&mut v, &y)`,
a closure capturing it), one with no initializer, or a destructured part, holds what is not known once the block
has ended: storing it past the block is the run's, as is the closure's store (a tail that borrows a place of the
block itself, `{ i32 w = …; &w }`, is `diag.reference-escapes-scope`). An `if`'s `then` block is read the same way (its tail was looked up outside it, and a `Vec` it
filled was taken for static data).

**A `Vec` of references** (and any value that may hold references in its cells) stored in a binding that
outlives what it reaches. `r = v`, with `v` a `Vec` that `&y` was pushed into, did not give `r` what `v` held
(a store followed a holder's facts only for a reference or a struct with a reference field), so `y`'s end
beside a live `r` passed; it is now `diag.destroy-while-aliased`. And a stored `Vec<ref<…>>` was not compared
with the blocks it may reach: `r = mk(&y)` past `y`'s block is the run's. Both were decided and read the slot.

**Two run-time bugs the probing met** (`[Call-Self-Tail]`, D-0309; both tools were wrong on valid programs):
`coby` copied a view argument of a self tail call into a temporary the ending frame's statement never ended, so the
text it viewed could not be destroyed later (`diag.destroy-while-aliased` at `main`'s end); and `cobc`, where a
frame records paths, ended a reference argument formed in the ending frame (`root_is(Box::get(x), name)`) with its
statement before the body ran again (`diag.stale-binding`). A place argument is read without the temporary, and a
reference argument travels in flight across the frame's end (`cb_send_ref` before the data, `cb_recv_ref` after
the unwind), in both back ends; the two calls read no location, so a store of it before them is dropped as dead (`bench/tree.cb` under Clang ran 12% more instructions while it was kept). A `cbc` built by this Update's `cobc` met the second while checking a closure. The runtime's replay trace (D-0299 step 4) forgets an object's storage when it ends: a struct literal's temporary where an inlined callee's parameter had been was placed in that ended object under GCC, so the traces of one program differed between the C compilers.

**Closures.** Without a written result type a closure's type is its body's (`spec/15` §6), and each `return` and
`?` in the body is checked against it (F21: `?` in a body giving `()` returned `None`; `return 1` beside a `str`
tail was accepted). A capture without `move` is a borrow while the body runs: moving it, or a part of it, out, or
destroying it there, is `diag.move-while-aliased` / `diag.destroy-while-aliased` (both tools faulted, under
different diagnostics).

**Patterns.** A variant written with its qualifier is that enum's (`spec/16` 1.34.0): `B::X :` in a `match` on an
`A` is `diag.type-mismatch`, and `Foo::Q`, which names nothing, is `diag.unbound-name` (it bound every value as
`Q`). The parser keeps the qualifier on the chain (`Option::Some(v)`), `modres` resolves it in its module and
keeps it beside the arm (`quals`), and the checker compares it with the enum matched at that level. An arm after a
struct pattern, or after any arm of a scrutinee with no variants or literals, is `diag.unreachable-arm` (F15).

**Generics.** A literal shift amount on a `T: integer` takes the shifted operand's type (F19: `a >> 3` was
refused with empty type names); a `T` amount is refused naming `T`.

## Not changed

- A reference stored through one parameter into another (`stash(&mut h, &inner)`) stays the run's: a signature
  says what the result may refer to, not what is stored where (spec/10 §2). The guide's comparison with Rust uses
  that shape now; `h = keep(&inner)` is refused before the run.
- A `Vec<StringView>` from a call may hold none of its sources (D-0290), so `drop(s)` while it is used is the run's.
- The odd open-type forms CHG-0438 listed (`[None, None]`, `foreach (x in [])`, …): unchanged. So is a closure with
  no written result type whose type comes from a `return` or an open variant tail (`[]() { return 5; }`): `cobc`
  stops on it with an internal error, as before (no wrong program is accepted; writing the type works).

## Measured

Every probe's verdict is the same under `coby`, `cobc` (GCC, Clang) and `cbc`; the probing's DIFF rows (`coby`
faulted, the compiled program printed) are gone. The conformance suites and the decided sweep (no program lost its
status) and the gate.
