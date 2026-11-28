# CHG-0462 — The static pass holds two borrows the run's record holds; `&f()?` as a view

Status: implemented (2026-11-16); Update 195
Affects: `impl/front/typecheck.cb`, `impl/tests/cbtest/main.cb`, `impl/tests/checks/store_env.py` (new),
`impl/tools/gate.py`, `impl/conformance/` (five cases), `spec/conformance.md` 3.315.0

## Why

The release profile, run for the first time since 2026-11-08, and a report from the owner's own
program:

- **`fixpoint` was red.** The compiler written out in the bootstrap form and run with every check faulted
  (`diag.aliasing-conflict`) on any program with an `assert` of two scalars: Update 188's `assert_values` was
  called on an expression inside the arm of a `match (&mut e.kind)` whose binder into that expression was still
  in scope. The default form compiled and ran it. Reduced, the default form and every check disagree on two
  shapes, and the interpreter's last build agrees with every check on both:

      match (&mut e.kind) { Kind::Count(n) : { *n += 1; r = total(e); }, … }     // n not mentioned again
      ref<i32, exclusive> x = &mut e.n;  i32 r = total(e);  *x += 1;             // total takes ref<E, shared>

  The specification has both as conflicts. `spec/14` §6 gives a match-by-reference binder its fact "in the
  arm", and `[Ref-Binding-Last-Use]` ends `let` bindings only; `spec/12` §2 (D-0049) passes an exclusive argument
  for a shared parameter "as the shared reborrow `&*e`", a borrow of the whole referent. The static pass ended
  the binder at its last use when the whole reference was given to a call, and judged the second call by the
  fields the function reads; having found no conflict it decided the function, so the default form kept no
  record and ran what the run's record refuses. Written out (`total(&*e)`), or with the binder used after the
  call, or with a local scrutinee, the static pass already refused both.
- **`all-checks` was green in no time**, and had run nothing: `cbtest` keeps a case's result under a key that
  did not take `COBALTC_ALL_CHECKS`, so the results the default form had left answered for it.
- **`show(&dir()?)`**, `dir() : Result<String, String>`, `show(StringView)`, was rejected: "`String` is not
  sliced" (found writing `cbmail`, on Update 191). `[Coerce-Text]` has no exception for `?`; the same borrow of
  a call's `String` is accepted without the `?`, and with it for a `Vec`.

## What changes

- **A binder of a match by reference holds to the end of its arm** where the whole reference is given to a call
  (`Body::arm_binders`, the call's argument check): `diag.aliasing-conflict`, static, with the message a binder
  used after the call already gave. `if (pattern = &mut e)` and `while (pattern = …)` are matches by reference.
- **An exclusive reference given where a shared one is taken is a shared borrow of the whole referent** in the
  call's argument check, beside what the function's summary reads through it: a live exclusive holder of a part
  conflicts, as with `&*e` written out.
- Both refuse programs the default form ran. Each such program faulted with every check and under the
  interpreter, when the call was reached: no program whose run was right is refused. Nothing in the tree was such
  a program but the compiler's own `rw_parts`, which now calls `assert_values` after the match.
- **`value_shape` knows `e?`**: the payload of `e`'s `Option` or `Result`, as it knew `unwrap` and `expect`, so
  `&f()?` where a view is declared is the view of the `String` the `?` gives.
- **`cbtest`'s store key takes the settings that change a run** (`RUN_SETTINGS`: `COBALTC_ALL_CHECKS`,
  `COBALTC_VERIFY`, `COBALTC_RECORD`, the bootstrap form's, the runner, the schedule seed and the others the
  compiler and the runtime read); `impl/tests/checks/store_env.py`, in the gate's `records` unit, holds the list
  to the names the sources read.
- **Cases**: `conf.match-binder-holds-at-call`, `conf.if-pattern-binder-holds-at-call`,
  `conf.exclusive-given-as-shared-borrows-whole` (rejected), `conf.call-after-borrows-end` (what stays accepted),
  `conf.coerce-text-of-propagated-call`.

No rule changes: the specification already said both; the implementation did not.

## Checked

- **The shapes**, each in the default form, with every check, and under the interpreter's last build
  (`rust-reference`): before, the default form ran seven reduced programs that the other two faulted
  (`diag.aliasing-conflict` at the call or at the access inside it); now the static pass refuses all seven, and
  the forms beside them that all three accepted are still accepted and print the same (`conf.call-after-borrows-end`).
- **Nothing that exists is refused**: the C and the diagnostics the compiler writes are the same before and
  after, byte for byte, for 3 106 programs of the tree (the suite, the cases from `tests/conformance.rs`, the
  showcases, the book, the examples, the bench, the guide's examples, the tools) and 5 500 local stress programs;
  the compiler's own source and the five tools pass `--check`.
- **`&f()?`**: the owner's line compiles and runs under GCC and Clang, with every check too.
- **The store key**: with a store and a toolchain key set, a run with `COBALTC_ALL_CHECKS=1` after a default run
  of the same case compiles and runs it again and keeps its own result.
- **The gate profile** on the commit with this Update's three changes (`ae6ae00`, run `20261116_095541`): its 15
  units green, in 87 minutes.
- **The release profile's units** on the same commit, on the owner's word of 2026-11-16: their first run since
  2026-11-08, so the first on the tools in CobaltC and the runtime in C. Green: `slow-cases` (the suite's 1 425
  under GCC and under Clang), `runtime` (54 minutes), `verify`, `wine` (1 409 of 1 411, the two 300-thread cases
  the unit allows) and `proofs-kernels`. Four are not, none from this Update's changes; the first three are
  `impl/STATUS.md`'s known gaps, for Update 196:
  - `all-checks`, on its first run that compiled anything (the store key, above): 1 of 1 425 fails. With every
    check on, a binding declared with no value of a type with a destructor (`Vec<String> v;`) that meets its
    scope's end, or a contained fault, before its first assignment has the destructor run on it, and the
    destructor's first read faults `diag.use-of-uninitialized` (`conf.unassigned-let-takes-first-value`). The
    default form is right, as the interpreter was. Not from the move off Rust: the last Rust compiler writes
    the same C, and that C linked with the Rust runtime faults the same way.
  - `programs`: the stress programs, the nine fuzzers, the local TLS handshakes, the crypto sweep and the 35
    interoperation checks pass; the sanitizer step reports a leak in `net/echo50.cb`, the formatting state of
    each thread that ends (42 400 bytes over 20 threads), in the runtime in C since Update 193.
  - `fixpoint`, at the bootstrap level since Update 194 (the default form's fixed point is the gate's
    `bootstrap` unit, green): its first run faulted in the compiler's own `assert` rewrite 30 minutes into
    stage 2, which is how the first shape above was found. On this commit it was stopped by hand: stage 2, the
    compiler compiling itself with every check on, takes longer than the unit's 150 minutes. A run by hand on
    the source with the `assert` rewrite repaired (this change's first part, before the others) reached the
    fixed point: stage 2 is stage 1, 101 079 370 bytes at level 10, its stage 2 taking 5 h 42 min beside the
    gate's units. The unit's limit is eight hours now (`impl/tools/gate.py`); the unit itself has not yet run
    to its end on the final source.
  - `external` (advisory): the eight DNS-over-TLS exchanges with public resolvers and the HTTP clients pass;
    the step that drives the HTTP server with `curl` read no port, because its script waited ten seconds for a
    server that `cobc --run` now takes longer to build (the server announced its port afterwards). The script
    waits longer now; no fault of the tools.
