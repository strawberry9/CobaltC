# CobaltC file-based conformance suite

This is a permanent, file-based conformance suite for both CobaltC
implementations in `impl/`, the reference interpreter `coby` and the
native compiler `cobc`, for ongoing use — not a one-off check. It is a
separate, complementary artifact to `impl/tests/conformance.rs` (cases
embedded as inline Rust string literals, interpreter only): every
case here is a real, standalone `.cb` source file on
disk, individually readable, individually runnable
(`coby some_case.cb` works outside `cargo test` exactly as it does
inside it), and directly traceable to the spec artifact it exercises
by its directory name. Neither suite replaces the other — this one
trades the other's speed and literal-Rust-assertion convenience for
direct spec traceability and standalone inspectability.

## Running it

```
cd impl
cargo test --release --test cb_conformance_suite -- --nocapture
```

The runner (`impl/tests/cb_conformance_suite.rs`) discovers every
`.cb` file under this directory, parses its header, runs it through
the real built binary, and asserts the actual outcome matches what the
header declares — with a failure message naming the exact file and
the expected-vs-observed mismatch. With `--nocapture` it prints how
many cases there are, positive and negative, and how many fixture files
it skipped.

The same cases, compiled and run by `cobc --run`:

```
cd impl
cargo build --release -p cbrt -p cobc
cargo test --release -p cobc --test compiled_suite
```

`impl/cobc/tests/compiled_suite.rs` asserts the same header outcome,
and that the compiled program's standard output and error output (the
diagnostic, location included) are exactly `coby`'s. It compiles every
case twice, with GCC and with Clang (`COBC_TEST_CCS` chooses others),
and both must pass.

## File format

Every `.cb` file starts with a required header comment block:

```
// CONFORMANCE-CASE
// spec: 06-arithmetic
// facility: rule.arith.checked [Arith-Checked-Overflow]
// kind: negative
// expect: diag.arith-overflow (dynamic)
//
// A one-to-three sentence human-readable description of exactly what
// this case demonstrates and why.

fn add(i32 a, i32 b) : i32
{
    a + b
}

fn main()
{
    add(2147483647, 1);
}
```

- `spec:` — the directory it lives in (makes the file self-contained
  if copied elsewhere).
- `facility:` — the real `rule.*`/`[Label]`/`diag.*`/`D-XXXX`/
  `CHG-XXXX`/`feat.*` id(s) this case exercises, always checkable
  against the actual spec/registry files.
- `kind:` — `positive` (must be accepted / run to the stated outcome)
  or `negative` (must be rejected).
- `expect:` — one of:
  - `ok` — runs to completion successfully. Assertions are embedded in
    the program's own logic via the corpus-wide idiom
    `if (condition_that_should_hold_is_false) { 1 / (1 - 1); }`,
    forcing a div-by-zero fault if real behavior doesn't match.
  - `diag.<name> (static)` / `diag.<name> (dynamic)` — the exact
    diagnostic and phase a real run of the file actually produces.
    Every one of these was set by running the file through the real
    binary first, never guessed.
  - `parse-error` — for grammar-level failures caught before semantic
    analysis runs at all.
  - `exit N` — runs to completion with exit status `N`, from
    `fn main() : u8` (CHG-0040).
- `expect-stdout-hex:` (optional) — lowercase hex, asserting the
  literal bytes a `write()`-exercising case produces, not just success.
- `cobc-only:` (optional) — why only `cobc` can run the case: it links
  the program's own C code (`spec/20` `rule.trust.extern-code`). Under
  `coby` the case must be refused (exit 3, `unsupported: …`); `cobc`
  runs it and checks `expect:` and `expect-stdout-hex:`, since there is
  no interpreter output to compare with (CHG-0039).
- `stdin-hex:` (optional) — lowercase hex, the bytes fed to the case as
  standard input, by every runner and to both implementations; empty for
  empty input (CHG-0038, D-0050). A case without it gets no input.
- `args:` (optional) — the program's arguments (`spec/21`
  `rule.stdlib.args`), separated by spaces: `\xHH` stands for any byte
  and `""` for an empty argument; `$TMP` is a new, empty directory for
  each run, for a case that writes files. Every runner passes them,
  after the file, to `coby` and to `cobc --run` (CHG-0040, CHG-0043).

Every case runs with its own directory as the working directory, so a
relative path names a file beside it, such as the fixtures in
`21-standard-library-semantics/file_fixtures/` (CHG-0043).

**Formatting**: every file's CobaltC code uses Allman brace style
(every opening brace on its own line — `spec/22` §1's own stated
convention) and 4-space indentation throughout, per the human owner's
explicit requirement.

## Coverage map

Cases in every spec-governed directory (`06-arithmetic` through
`22-surface-syntax`), plus the fixture files of `17-modules/fixtures/`
(modules that the `17-modules` cases load; not cases themselves). The
test runs report the counts (see "Running it"). Per-directory
counts and content are visible directly in each directory; this section
records what's *not* covered and exactly why, rather than leaving gaps
implicit.

### Genuinely out of scope (documented interpreter limits, `impl/STATUS.md`)

- (`u128`'s upper half was listed here; fixed -- `06-arithmetic/
  u128_full_range_ok.cb`.)
- (Fine-grained thread interleaving was listed here; since the GIL
  rewrite -- `impl/STATUS.md`, "Blocking at any depth" -- threads yield
  between statements, and `19-concurrency/threads_interleave_under_
  mutex_ok.cb` relies on it.)
- (The implicit-tail literal gap was listed here; fixed --
  `12-type-system/tail_literal_takes_expected_type_ok.cb`.)

### Fixed since this pass

- **`[Borrow-Exceeds-Source]` / `[Write-Not-Exclusive]`**
  (`diag.borrow-exceeds-source`, `diag.write-through-shared`) — was: a
  function receiving `ref<i32, shared>` could do `auto rm = &mut *r;
  *rm = 99;` and successfully mutate the caller's value through what
  should have been a read-only reference, a real soundness hole in
  mode-monotonicity enforcement. Fixed by `crosses_shared_ref`
  (`src/interp.rs`), computed fresh from the expression's own structure
  at each borrow/write rather than cached — a first attempt using a
  persistent (obj, path) table was itself a real bug, since the same
  storage can legitimately be reached through both an exclusive and a
  shared reference at different points in one program (see
  `impl/STATUS.md`'s own account of finding and fixing that). Covered
  by `08-alias-validity/borrow_exceeds_source_write_through_shared_
  rejected.cb` (the reborrow shape) and three regression tests in
  `impl/tests/conformance.rs` (reborrow, direct write, auto-deref field
  write, plus a false-positive regression guard for the table-based
  first attempt).
- **D-0006's "no implicit conversion between distinct nominal types"**
  — was: an `i32` binding passed where an `i64` parameter is declared
  was silently accepted, the value round-tripping as if implicitly
  widened. Fixed by `check_arg_exact_types` (`src/typecheck.rs`) — a
  concrete (non-generic-parameter) parameter's type must exactly match
  its argument's own checked type, `[T-Call]`'s `Γ ⊢ ei : τi`. Static
  only (`diag.type-mismatch`'s own registered Phase; no dynamic
  evaluator re-check was added). A bare integer literal is exempt
  (`rule.arith.literal` lets it take on whatever concrete type context
  requires — found the hard way, via a false positive on
  `Vec::push(&mut vec_of_u8, 226)`, T resolved from the *other*
  argument only *after* the literal had already defaulted to `i32`).
  Covered by `12-type-system/call_argument_width_mismatch_rejected.cb`
  and four regression tests in `impl/tests/conformance.rs`.
- **`diag.use-of-uninitialized`** — was: `[Let-Uninit]` (`τ x;`, no
  initializer) entirely unimplemented; reading such a binding hit
  `diag.type-mismatch` by accident (the placeholder value failing
  arithmetic), not the registered diagnostic. Fixed with a simple
  per-object `uninit_bindings` set (`src/interp.rs`), checked on read
  (`result_to_value`) and cleared on the first write reaching that
  object (`eval_assign_place`) — not a full definite-assignment
  dataflow analysis (a struct declared uninitialized and only
  partially field-written before a read of an *untouched* field is not
  separately caught), a narrower but still-sound scope matching this
  pass's own conservative-first-write approach elsewhere. Covered by
  `11-initialization/use_of_uninitialized_binding_rejected.cb` and two
  regression tests in `impl/tests/conformance.rs`.
- **`diag.break-outside-loop`** — was: `break` outside a loop fell
  through to a generic internal "unexpected control flow at top level"
  message, not the registered diagnostic. Fixed with a simple
  `while_depth: u32` counter on the static pass's `Body`
  (`src/typecheck.rs`), mirroring `unsafe_depth`'s own existing
  pattern. **`diag.return-outside-fn` checked and confirmed not a
  bug**: `return` outside a function body is a parse error in this
  grammar ("expected item, found Return") — it never reaches semantic
  analysis at all, so there is nothing for a separate check to catch.
  Covered by `14-control-flow/break_outside_loop_rejected.cb` and two
  regression tests in `impl/tests/conformance.rs`.
- **`diag.propagate-outside-fallible-context`** — was: `?` in a
  function whose return type doesn't match `Result<_, E>` (same `E`)
  was silently accepted. Fixed in `ExprKind::Propagate`'s own checker
  (`src/typecheck.rs`): once the propagated expression's own `Result<T,
  E>` is known, the enclosing function's declared return type must be
  `Result<_, E>` with the *same* `E` — `[Propagate-Err-Mismatch]`'s
  exact requirement, checked only once `E` is actually known (never
  against an unresolved generic shape). Covered by `18-error-failure-
  semantics/propagate_outside_fallible_context_rejected.cb` and three
  regression tests in `impl/tests/conformance.rs` (wrong return type,
  mismatched error type specifically, and the matching case still
  accepted).
- **`diag.spawn-borrow-closure`** — was: `spawn` accepted a borrowing
  (non-`move`) closure without rejection, which could let a spawned
  thread outlive data it only borrowed. Fixed by adding `is_move: bool`
  to `ClosureInfo` (`src/interp.rs`, set at closure creation, never
  previously persisted) and checking it in `spawn`'s own closure-
  resolution arm. Found and fixed alongside a genuine, separate
  host-crash bug in the same code path: an argument-count mismatch
  between `spawn`'s trailing arguments and the closure's own parameter
  count was a Rust-level panic (`index out of bounds`), not a CobaltC
  diagnostic — now `diag.type-mismatch`. Also found, while writing this
  fix's own test: `expect_true`'s block-as-if-condition wrapping nests
  `join` deep inside an expression tree, which genuinely hangs — not a
  new bug, the already-documented resume-mechanism limitation
  (`impl/STATUS.md`'s "How blocking works, and its one real
  limitation"); the test was rewritten to the top-level-statement shape
  every other spawn/join test already uses, not worked around by
  changing the interpreter. Covered by `19-concurrency/spawn_borrow_
  closure_rejected.cb` and three regression tests in `impl/tests/
  conformance.rs`.
- **`diag.extern-non-ffi-type`** — was: an `extern fn` declared with a
  resource-typed (non-`FfiType`) parameter was silently accepted.
  Fixed by checking every declared extern's parameter and return types
  against `FfiType`'s exact membership (`is_ffi_type`,
  `src/typecheck.rs`) unconditionally in `check_program`, before the
  worklist even starts — `[Extern-Non-Ffi-Type]` is `disposition:
  rejected` on the declaration itself, so an extern that happens to
  never be called still needs checking; the existing per-call-site
  worklist has no call site to hang that check on for an unused
  extern. Covered by `20-trust-boundaries/extern_non_ffi_type_
  rejected.cb` and two regression tests in `impl/tests/conformance.rs`.

### Real gaps found *during this pass*, not previously documented

Empty — every item originally recorded in this section has since
been fixed (see "Fixed since this pass" above). One earlier entry here,
about `diag.borrow-of-temporary` versus `diag.borrow-of-non-place` for
`&make()`, was resolved the other way once the static pass implemented
`[Ref-Form-Not-Place]` per spec/13 §1: a bare call result is not a
place expression whatever its type, so `&make()` is
`diag.borrow-of-non-place` for a resource-returning `make` too, and
`diag.borrow-of-temporary` is reserved for a *projection* of a call
result or aggregate literal (`09-identity-origin-extent/
borrow_of_temporary_projection_rejected.cb`).

Building the suite and discovering real gaps (seven of them genuine,
one a false alarm on closer inspection) along the way is exactly what
a conformance suite is for.

## The spec's own table, as a test

`tests/spec_rows.rs` runs every row of `spec/conformance.md` as a
program and checks both the outcome and the phase the row states.
The file suite here is the readable, per-case form of that same
coverage; the row test is the mechanical guarantee that the master
document and the interpreter agree, and it fails naming the row when
they do not. `tests/conf_coverage.rs` separately guarantees that every
`conf.*` id is referenced by some test.
