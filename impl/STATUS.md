# CobaltC reference interpreter — status

This is a reference interpreter for CobaltC (per `spec/IMPLEMENTATION-NOTES.md`'s
own framing: an interpreter that executes the specification's CFN rules
directly over an abstract state, not an optimizing native compiler),
built per the human owner's authorization to move from the design phase
to an implementation. It targets the platform it was built and tested
on: **Ubuntu 22.04.5 LTS, x86_64, kernel 6.8**.

## Build and run

```
cd impl
cargo build --release          # binary: target/release/coby
cargo test                     # every suite: inline conformance, the .cb file suite, every spec row
./target/release/coby path/to/program.cb
```

`coby <file>` parses, loads the prelude (`std`'s CobaltC source in
`std/*.cb`, embedded by `src/prelude.rs`; `spec/21` transcribed), runs the static pass
(`src/typecheck.rs`) over the whole reachable program, and — only if
that pass accepts it — evaluates `fn main()`. Exit code 0 on `ok`
termination; 1 with a full, spec-grounded diagnostic on stderr
otherwise (rejected before running, or faulted at run time — see
"Diagnostic messages" below for what that actually looks like now).

## What's implemented and working

- **Spec-grounded diagnostic messages** (`src/diagnostics.rs`, new this
  pass) — see "Diagnostic messages" below.
- **A real static pass ahead of execution** (`src/typecheck.rs`) — see
  "The static pass" below for what it covers and its own honestly-
  scoped limits.
- **Real module visibility enforcement** (`src/modres.rs`, new this
  pass) — see "Module visibility" below.
- **File-backed module declarations** (`src/loader.rs`, 2026-09-22):
  `module m "./file.cb";` per `spec/17` §5 / `CHG-0026`, with
  diagnostics that name the file they lie in — see "File-backed
  modules" below.
- **Full lexer and parser** for `spec/22`'s surface grammar: literals
  (including type-suffixed and default-typed, and `CHG-0025`'s `"…"`
  str-literal and `b"…"` byte-literal), all operators with
  correct precedence/associativity, structs/enums/arrays, `if`/`while`/
  `match`/`break`/`continue`, functions (including associated functions
  `Type::name` and generics), closures (`move`/borrow capture syntax),
  `unsafe` blocks, `extern fn`, nested `module { }` declarations and
  `import` (fully resolved and visibility-checked — see "Module
  visibility" below), and the disambiguations spec/22 §2 calls out by
  name (struct-literal vs. statement, array-literal vs. closure,
  nested-generic `>>` splitting, single-field destructuring).
- **Arithmetic** (`spec/06`): all integer types with correct bitwidths,
  checked add/sub/mul/div/rem/neg/shift with the documented overflow
  diagnostics, wrapping/saturating/checked_* families, all the named
  conversions (`widen`/`narrow`/`narrow_wrapping`/`reinterpret`/
  `to_float`/`to_int`), IEEE float arithmetic and comparison (including
  NaN behavior), bitwise ops.
- **Aggregates**: structs, arrays (with dynamic bounds checking),
  enums/`match` (exhaustiveness is checked *both* statically, when the
  scrutinee's enum is concretely known, and dynamically as a backstop:
  `diag.non-exhaustive-match`), single-field destructuring, generic
  struct/enum monomorphization via a real (if ad hoc) type-parameter-
  inference pass described below.
- **Functions, generics, closures**: recursion, both capture modes with
  correct capture-mode derivation (a syntactic scan for writes/moves/
  `drop`, per `spec/15` §6), move-captured resources correctly owned by
  the closure object and destroyed with it (a real bug in this exact
  path — a move closure called twice used to read already-destroyed
  data — was found and fixed this pass; see "Bugs found and fixed").
- **Resource authority and destruction** (`spec/07`, D-0008): automatic
  reverse-declaration-order destruction at block exit and at statement
  exit for unstored temporaries; `drop`; move semantics that invalidate
  the source binding; composite (nested-field/array-element/enum-
  payload) destruction, including through `Vec`/`Rc`/user structs
  nested arbitrarily deep; the diagnostics for double-destroy,
  destroy/move-while-aliased, and overwrite-of-a-live-resource.
- **Aliasing** (`spec/08`, D-0018): a real dynamic `clash` check — not
  a stub — implemented by scanning every live `ref`/`guard` value for
  overlapping-target, incompatible-mode occurrences, using per-borrow
  *access-path tokens* (distinct from which object currently holds a
  copy of the value) so passing a reference by argument or storing it
  in a struct field doesn't spuriously conflict with itself, and
  `sync-exempt` (a mutex guard vs. a shared reference to its own mutex,
  `spec/19` §2). This is the language's actual soundness mechanism per
  `spec/08` §3's own statement ("the invariant's guarantee rests on the
  use-time checks... not... by construction") — it is not a lesser
  substitute for a missing static pass, it is *the* mechanism the
  static pass is layered on top of in the spec. **A real, unfixed bug
  in this mechanism was found this pass — see "Known bugs" below.**
- **Trust boundaries** (`spec/20`): a real byte-addressable arena
  backs `allocate`/`deallocate`/raw pointer arithmetic/read/write;
  `reclaim` with correct re-attachment (repeated `reclaim` at the same
  address returns the same object identity, so two `reclaim`d paths to
  one address correctly alias each other); `[Rawptr-Move-In]`/
  `[Rawptr-Move-Out]` for resource types crossing the raw boundary;
  `[Unsafe-Rejected]` enforced *both* statically (ahead of execution,
  as of this pass — `src/typecheck.rs`) and dynamically (a lexical
  nesting-depth counter, as before).
- **The actual `spec/21` prelude, `Vec<T>`, `String`, `Rc<T>`
  source** — transcribed verbatim into `src/prelude.rs` (since D-0136,
  `std/*.cb`, one file per submodule) and run through
  this interpreter's own parser/evaluator, not reimplemented natively
  in Rust. `Vec::push`/`pop`/`index_shared`/`index_exclusive`/`grow`/
  `drop`, `String::from_utf8` (full UTF-8 validator) /`from_str`/`len`/
  `into_bytes`, `print`, `Rc::new`/`clone`/`get`/`drop` (reference counting) all
  work as specified, including growth/reallocation and nested
  `Vec<Vec<T>>`/`Vec<Rc<T>>`-shaped resource cleanup.
- **`CHG-0020`'s `write` extern**, bound to standard output through
  `std::io::Write::write_all` on the arena's real bytes (no `libc`, no
  platform-specific code, so the crate builds unchanged on Windows) —
  `ex.extern-write`'s program actually writes to stdout. The `isize`
  claim is the byte count on success and `-1` on an I/O error; a
  short write is never reported, since `write_all` loops.
- **Concurrency primitives** (functionally, not physically — see
  below): `spawn`/`join`, `Mutex::new`/`lock` with reentrant-lock
  detection and guard-release-on-drop.
- **Failure semantics**: checked faults unwind (destructors run) and
  terminate the whole program with the diagnostic name;
  `Result`/`?` propagation desugars correctly, preserving the
  `Result<T,E>` type through an early `Err` return.

## Diagnostic messages (`src/diagnostics.rs`, new this pass)

Before this pass, every rejection/fault just printed the bare
`diag.*` id (e.g. `terminate: diag.aliasing-conflict`) — accurate but
undecipherable without already knowing `spec/registry/diagnostics.md`
by heart. `coby <file>` now prints something like:

```
error: diag.aliasing-conflict (dynamic)
  at line 12

  rule:      [Borrow-Denied], [Read-Conflict], [Write-Conflict], [Match-Conflict]
  invariant: inv.alias-validity, inv.concurrency-validity
  required:  ¬clash(a, m)
  observed:  witness(a, m, Σ) — a live non-ancestor path with overlapping target and incompatible mode

  repair: end the conflicting reference (let its holder go out of scope) before this access, or use `shared` on both sides

  see: spec/examples.md -> ex.borrow-conflict, ex.projection-conflict
       spec/registry/diagnostics.md
```

**Fidelity, not invention.** `src/diagnostics.rs` embeds
`spec/registry/diagnostics.md` verbatim at compile time
(`include_str!`, not a hand-transcribed copy that could silently drift
on a future spec edit) and parses its own regular bullet-list format
(one bullet per `diag.*` id, or a few sharing one bullet) into a real
table of Phase/Rule/Invariant/Required/Observed/Provenance/Repair/
Related fields. Every word of explanation shown to a user traces back
to that file; nothing is paraphrased into friendlier-sounding invented
prose. A `parse_registry` unit test suite (7 tests) checks the parser
itself — including a case whose Repair text has an internal period
inside a parenthetical, to prove the position-based field-slicing
doesn't truncate early — and one test,
`every_diag_id_this_interpreter_emits_is_in_the_registry`, cross-checks
`interp.rs`'s own `ALL_EMITTED_DIAGS` (every `diag.*` id this
interpreter's source can actually construct, hand-derived by grep, one
dynamically-built id included) against the parsed registry, so this
interpreter can never silently start emitting an unregistered/made-up
diagnostic name without a test failing.

**Source locations.** `Expr` already carried a `line: usize`
(`src/ast.rs`) but nothing threaded it out to a fault. Rather than
changing ~115 individual `Flow::Fault(...)`/`Err("diag....)`
construction sites' signatures across `interp.rs`/`typecheck.rs` (a
much larger, higher-risk refactor), the fix is three thin wrapper
functions — `Interp::eval`, `Interp::exec_stmt`, `Interp::
exec_block_body` in `interp.rs`; `Body::check_expr`, `Body::check_stmt`
in `typecheck.rs` — each renaming its existing body to an `_inner`
function and tagging the *first* untagged fault that passes through
with the current line (`"diag.xxx@42"`), via a `current_line` field
updated as each function runs. An already-tagged fault passes through
unchanged, so the innermost (most precise) attachment point always
wins. This reaches every fault reachable from `eval()`'s own call tree
(the overwhelming majority) plus the few raised directly in
post-evaluation bookkeeping (`destroy_object`, `store_binding`,
block-exit's destruction sweep) via the same-statement fallback line —
not a fully general per-AST-node span, but real, mostly-precise
locations where there were none at all before.

**The prelude-offset bug this caught.** `build_interp` parses one
combined `"{prelude}\n{user src}"` token stream (deliberately — the
disambiguation prescan needs to see the prelude's own struct/enum
names), so every `Expr::line` is a line number *in that combined
text*, not the user's own file, until `prelude::line_count()`'s offset
is subtracted back out (`resolve_user_line`, `lib.rs`). The first
version of that offset (`PRELUDE_SRC.lines().count()`) was off by
exactly one line for *every single diagnostic*, silently: `PRELUDE_SRC`
already ends in its own trailing `\n`, and `format!("{}\n{}", ...)`
adds one *more*, inserting a real blank line the naive `.lines()`
count doesn't account for. Caught by manually testing a real 4-line
program before declaring this done (it reported line 4 for an error
actually on line 3) — fixed by counting `\n` characters directly
(mirroring the real `format!` join byte-for-byte) instead of trusting
`.lines()`'s own line-counting semantics, and now covered by
`lib.rs`'s `located_diag_tests` module, which asserts against known
correct line numbers in short literal programs rather than
re-deriving the same offset arithmetic the implementation uses (which
would let a shared error in the formula cancel out invisibly).

**Public API compatibility.** `run_source`/`check_source_static`/
`run_source_phased` (what `tests/conformance.rs`'s ~106 assertions
compare against) are completely unchanged in observable behavior —
still bare `"diag.xxx"` strings, no `@line` suffix ever visible to
them. The line-tagging lives entirely in a parallel `_raw` internal
API (`run_source_raw`, `check_source_static_raw`,
`run_source_phased_located`/`OutcomeLocated`) that only `main.rs`
uses; the public API strips any tag via `strip_location` before
returning. This was a deliberate design choice specifically to avoid
touching the existing test suite's ~15 assertion call sites at all,
not an oversight — see `lib.rs`'s own doc comments on
`check_source_static_raw`.

**Honest gap.** Not every fault carries a precise location — anything
raised from a helper called before the checker/evaluator's
`current_line`-tracking wrappers ever ran (vanishingly rare in
practice, since almost everything routes through `eval()`/
`check_expr`) reports "at: unknown location" rather than a guessed
line. A diagnostic whose id has no `spec/registry/diagnostics.md`
entry (should never happen, per the cross-check test above, but
handled defensively) renders an honest "no entry for `diag.x` ..."
note instead of fabricated prose.

## The static pass (`src/typecheck.rs`, new this pass)

A whole-program, ahead-of-execution pass, independent from the dynamic
evaluator (a deliberate design choice — two separately-derived
implementations of "what type does this expression have" is a
cross-check property, not duplication). It covers:

- **Type-checking** per a real subset of spec/12's `[T-*]` rules —
  arithmetic, comparison, assignment, `if`/`match`/block result types
  (including `[T-Block]`'s "a block whose last statement is a
  `never`-typed expression statement is itself `never`" clause),
  field/struct-literal/enum-variant construction, pointer-offset
  arithmetic (`[T-Rawptr-Offset]`, a distinct rule from ordinary
  arithmetic) — with **exact** nominal type equality (D-0006: `i32`
  and `i64` are never compatible), unlike the dynamic evaluator, which
  compares raw integer payloads regardless of the declared width/
  signedness tag.
- **Literal range/defaulting** (`rule.arith.literal`) and **literal-
  operand constant-fold overflow refutation** (`rule.control.flow-
  analysis` §6: `2147483647: i32 + 1: i32` is rejected *before* the
  program runs, not merely faulted at run time — an actual static/
  dynamic phase distinction, not just a diagnostic-name match).
- **`[Unsafe-Rejected]`**, statically: rawptr deref, pointer-offset
  arithmetic, `extern fn` calls, and `reclaim`/`release`/`copy_raw`/
  `deallocate` all require a lexically enclosing `unsafe { }`, checked
  ahead of execution (not only when the code path actually runs, as
  the dynamic-only counter did before).
- **Closure capture-list mismatch** (`diag.capture-list-mismatch`,
  spec/15 §6): a real free-variable scan compared against the
  declared capture list.
- **Match exhaustiveness**, statically, when the scrutinee's enum is
  concretely known.
- **`rule.control.flow-analysis` (spec/14 §6), in full** — see the
  dated section "rule.control.flow-analysis implemented" below. This
  replaced the earlier flat, single-block `deriv` check.

**Generics** are checked per *concrete instantiation*, monomorphization-
style: a worklist of `(function, type-args)` pairs starting from
`main`, growing as calls (to generic **and** non-generic functions
alike — every reachable function's body is checked, not just generic
ones) are discovered with statically-resolvable type arguments. An
instantiation whose type argument cannot be pinned down statically
(e.g. `Vec::new::<T>()` with no `let`-annotation or other expected-type
context to fix `T`) is simply not checked — the dynamic evaluator
remains the backstop for it, as for everything else this pass cannot
confidently resolve.

**The governing rule throughout is conservatism**: `Ok(None)` ("unknown,
don't check") is the answer whenever this pass isn't sure, never a
guess. This was verified empirically, not just asserted: every
`expect_ok`/`expect_true`(`_with`) fragment in `tests/conformance.rs`
(all 95 of them) is asserted, permanently, not to be a static-pass
false positive — that assertion is what caught and fixed every real
bug listed below during development.

**What it still does *not* cover** (the dynamic evaluator remains the
sole enforcement): cross-declared-type comparisons unless a param/
return/field type makes both sides concretely known to this pass;
and nothing else in the registry's `Phase: static` set -- see "The
remaining static rules" below.

## Module visibility (`src/modres.rs`, new this pass)

`spec/17-modules.md` is fully implemented, not stubbed: a real module
tree (arbitrary `module { }` nesting), `export`/private visibility
(`rule.module.visibility`), name resolution in the actual specified
order (`rule.module.resolve`'s `[Resolve-Unqualified]`/
`[Resolve-Qualified]`), and `import` (`rule.module.use`) actually
affecting resolution rather than parsing to a no-op. Run once, as a
static pass immediately after parsing and before `build_items`/
typecheck/evaluation ever see the program — `rule.module.visibility`
itself says this rule is "purely lexical; always `disposition:
rejected`", so unlike almost everything else in this interpreter,
there is deliberately no dynamic fallback for it.

Design: rather than threading "which module is this in" through every
existing lookup call site in `interp.rs`/`typecheck.rs` (~40 of them),
this pass walks the whole AST once and rewrites every item reference
(calls, struct literals, type names, `import` targets) in place to its
resolved, fully-qualified key (`M::name`, matching `rule.module.
resolve` §1 exactly) — so every existing lookup keeps working
unmodified, keyed the same way it always was, for the entire (flat,
single-module) existing corpus. A reference this pass cannot
confidently resolve is left untouched rather than force-rejected, so
the pre-existing `diag.unbound-name` checks remain the authority for
"does this name exist at all"; this pass only ever *adds* the two
checks nothing else in the pipeline could ever perform:
`diag.name-not-visible` and `diag.ambiguous-name`
(`[Resolve-Not-Visible]`/`[Resolve-Ambiguous]`).

One deliberate reading decision, beyond the literal rule text: the
CFN rule for `[Resolve-Qualified]` states `visible(qk, name, M)` only
for the *final* segment of a qualified path. Read completely
literally, that would let a qualified reference reach an exported item
through a *private* intermediate module, which contradicts `rule.
module.visibility`'s own prose ("An export item inside a private
module is reachable only where that module is"). This implementation
checks visibility on *every* hop of a qualified path, not just the
last, to actually match that stated guarantee
(`conf_module_export_inside_private_module_not_visible` demonstrates
the case where these two readings diverge).

**Known, deliberate limitations** (both since lifted -- see "Verification pass: every spec row, outcome and phase" below; kept here as the record of what this pass left):
- The surface grammar only allows a single bare identifier in *type*
  position (`Type::Named` holds one `String`, never a `::`-path —
  confirmed against `parse_type`), so a struct/enum in a nested module
  can only be named as a field/parameter/return type from within that
  module, an ancestor of it, or via `import` — never spelled
  `a::b::Foo` in type position, since the parser has nowhere to put
  the extra segments. Qualified paths work fully in value position
  (calls, struct literals), where the grammar does allow them.
- Two different enums both visible from the same reference site and
  sharing a variant name are correctly detected as ambiguous *for a
  bare variant reference* (`diag.ambiguous-name`, clause (4)), but
  actual variant *dispatch* at runtime still goes through the
  pre-existing single flat `enum_of_variant` table keyed by bare
  variant name (unchanged by this pass) — a qualified `Enum::Variant`
  reference has its `Enum` segment fully resolved and visibility-
  checked, but the trailing `Variant` segment dispatches through that
  same flat table. Not exercised by any test in this corpus (no two
  enums share a variant name), and not newly introduced — the flat
  table predates this pass.

Seven new tests (`conf_module_*`) cover: a private item rejected as
not visible from outside its module; an exported item correctly
reachable; a private item correctly usable from its own sub-module
(clause (3) carries no visibility premise at all); an exported item
behind a private intermediate module still rejected; `import` making
an otherwise-unbound name resolve; an `import` of a non-visible target
rejected at the `import` itself; and ambiguous imports.

## File-backed modules (`src/loader.rs`, 2026-09-22)

`rule.module.file` (`spec/17` §5, `CHG-0026`/D-0021): `[export] module
m "p";` is a module whose body is the file `p` names. Implemented as
the equivalence the rule states, literally: a pre-parse pass finds the
declaration in the token stream, reads the file relative to the
declaring file's directory, expands it recursively, and splices its
text in as `[export] module m { … }` before the program is lexed as a
whole. This is the same mechanism `lib.rs` already uses to place the
prelude ahead of the program, and it is required for the same reason:
the parser's declaration-statement pre-scan (`Parser::new`, spec/22 §2
rule 4) collects `struct`/`enum` names from the one token stream it is
given, so `Rect r = …` in the entry file only parses as a declaration
if the file declaring `Rect` is in that stream. Consequently
`parser.rs`, `modres.rs`, `build_items`, `typecheck.rs`, and
`interp.rs` are untouched — they see an inline module.

The three static rejections are raised by the loader: a path naming no
readable file (`diag.module-file-not-found`), a file whose body is
being assembled to reach the declaration (`diag.module-cycle`; the
entry file itself is on that stack, so a file naming it closes a
cycle), and one file named by two declarations
(`diag.module-file-duplicate`). Identity is by canonical path.

**Locations.** Every pass still tags a diagnostic with one line number
in the combined `prelude + expanded program` text. The loader builds a
`SourceMap` from each expanded line back to (file, line);
`resolve_user_location` applies it after the prelude offset, and
`render` now prints `at <file>:<line>`. `main.rs` passes the entry
file's path (`run_program_phased_located`); the string-only entry
points (`run_source*`, `check_source_static*`) resolve paths against
the working directory and name the program `<input>`. Both passes run
over one expansion, so a file is read once per run.

The resolver's own diagnostics (`diag.name-not-visible`,
`diag.ambiguous-name`, and its `diag.unbound-name` for an `import`
target) are tagged too, as of the same date: with the referencing
expression's line (innermost expression wins, as in `typecheck.rs`),
a declaration statement's initializer line for its type annotation,
or the `import` declaration's own line (`Item::Import` now carries
one). A visibility failure in a field, parameter, or return *type*
still has no line — `Type` carries none — and renders as "unknown
location".

**Implementation-defined choices** (`spec/22` §5 list; documented here
as the spec requires): beyond the required `/`-separated relative core
with no `..` and no leading `/`, this implementation also accepts `..`
segments, a leading `./` (stripped for display), and an absolute path,
all through `std::path` on the host platform; a resolved path denotes
the file `fs::canonicalize` yields. A diagnostic location names the
file by the path as written, joined onto the declaring file's
directory.

**Tests.** `src/loader.rs` unit tests: verbatim pass-through when no
declaration is present; a diagnostic inside a loaded file reports that
file and its own line; entry-file lines after a splice keep their own
numbers; a missing file reports the declaring line; a file naming the
entry file is a cycle. `conformance/17-modules/file_module_*.cb`: the
nine cases `CHG-0026` names, with their bodies under
`conformance/17-modules/fixtures/` marked `// CONFORMANCE-FIXTURE`
(the runner skips those). `cobaltc_examples/13_modules.cb` +
`13_geometry.cb` is the runnable two-file example.

## Consistency pass after file-backed modules (2026-09-22)

Three read-only audits (spec-internal, implementation-vs-spec,
guide-vs-spec) were run after `CHG-0026`; everything they found is
closed here or in `CHG-0027`–`CHG-0029`:

- **Field visibility enforced** (`CHG-0027`, `[Field-Not-Visible]`):
  `typecheck.rs` now knows the module of the function it is checking
  (`Body::module`, from the item's qualified key) and rejects a
  private field named by `e.f`, a struct literal, or a destructuring
  from outside the struct's module — `diag.name-not-visible`, static.
  Before this the check did not exist anywhere, and
  `fixtures/geometry.cb` (now `export f64 w;`) relied on the gap.
- **`diag.no-main`** (`CHG-0028`): `check_program` rejects a program
  with no non-generic root `main` with the new id instead of
  `diag.unbound-name`; the evaluator's own fallback message says the
  same.
- **Qualified unbound names are static** (`CHG-0029`): a `Path` with
  more than one segment that is neither an item nor an
  `Enum::Variant` is `diag.unbound-name` in the static pass, matching
  the registry's Phase for `[Resolve-Unbound]`.
  A call's callee is now resolved as a name before the "unmodeled
  intrinsic" fallthrough (`check_call`), which is where an unknown
  callee used to slip through to run time; `Mutex::new` joins
  `INTRINSIC_NAMES` as the one qualified intrinsic. The two case
  headers that expected `(dynamic)` —
  `conformance/17-modules/unbound_name_rejected.cb` and
  `conformance/22-surface-syntax/unbound_name_parse_survives_but_resolve_fails.cb`
  — were wrong and are corrected.
- **Syntax errors carry file:line:col**: a lexical or grammatical
  error's `line:col` in the combined text is mapped through the
  loader's `SourceMap` (`relocate_syntax_error`) and `main.rs` prints
  it as `error: parse error at <file>:<line>:<col>: …` — previously
  the line was a combined-text line with no file.
- **`ALL_EMITTED_DIAGS`** gained the three loader diagnostics and
  `diag.no-main`. The registry test still checks only emitted ⊆
  registry; the reverse direction is not tested (a registry entry a
  future pass never emits would go unnoticed) — noted, not fixed.
- **Spec-row router**: `tests/spec_rows.rs` runs a
  `spec/conformance.md` §14 row whose fragment is a `.cb` path as a
  whole program with the file's own directory, so multi-file cases
  are checked mechanically like every other row.
- **Type-position visibility** (a private struct named in a field,
  parameter, or return type) still reports "unknown location" — `Type`
  carries no line — unchanged.
- **Examples are now tested** (`tests/examples_run.rs`): every
  `cobaltc_examples/*.cb` with a root `fn main` must exit 0 through the
  real binary. Added after `CHG-0027` broke `13_modules.cb`
  (`13_geometry.cb`'s fields were private) and nothing caught it —
  the examples had only ever been verified by hand.

## `cobc`: the native compiler, Stage 1 / M1 (2026-09-22)

`impl/` is now a Cargo workspace: `coby` (this interpreter, unchanged
in behaviour), `cobc` (the compiler, `impl/cobc/`), and `cbrt` (the
runtime staticlib every compiled program links, `impl/cbrt/`). The
plan is `impl/COBC-PLAN.md`; this section records what M1 delivered.

```
cargo build --release --workspace
./target/release/cobc --run program.cb      # compile to a temp dir and run: a drop-in for coby
./target/release/cobc -o prog program.cb    # keep the executable (add --keep-c for the C)
```

**Pipeline.** `cobc` runs `coby`'s front end (`lib::analyze`: loader,
parser, resolver, static pass) and prints a static rejection exactly
as `coby` does; then `cobc/src/lower.rs` walks the checked item table
once, computing every expression's type and emitting gnu11 C in
three-address form (every intermediate is a named C local); `cc -O2
-ffp-contract=off` (`-O`/`-march=` override) compiles that against `libcbrt.a` found beside the `cobc` binary. A
construct M1 does not compile yet exits 3 with `unsupported: …`, which
the runners count as skipped, never passed.

**Compiled today:** every integer type including `i128`/`u128`
(checked `+ - * / %`, shifts, negation, bit operators, all conversion
intrinsics and the wrapping/saturating/checked families), floats,
`bool`, `str` literals with `str_len`/`str_byte`/`str_ptr`, `print`
and the `write` extern, arrays with bounds checks, structs and enums
(generic instantiations monomorphized, layouts asserted against the
spec's `[Layout-*]` at C compile time), `match`, `if`/`while`/`break`/
`continue`/`return`, blocks as values, user functions and generics
inferred from arguments and expected type, `rawptr_of(&x)`,
`reinterpret_ptr`, `dangling`, `fault`. Diagnostics: static ones from
the front end; dynamic ones from `cbrt::cb_fault`, rendered through
the same `diagnostics.rs` (shared by `#[path]`), with `file:line`
from the loader's source map passed as constants.

**Not yet (each exits 3):** references and everything that needs the
runtime's objects — resources, `drop`, moves, borrows, `?` (M2/M3);
closures and fn values (M3); raw storage `allocate`/`reclaim`/
`release`/`copy_raw`, hence `Vec`/`String`/`Rc` (M4); concurrency
(Stage 2). `cbrt` therefore holds no object table yet: frames and
statement scopes are counted, faults print and exit.

**Results** (`cobc/tests/compiled_suite.rs`, which runs the whole file
suite and the examples through `cobc --run`): 99 cases pass, 51
skipped, 0 disagree; 3 examples run natively. The M1 gate — every
case in `06`, `12`, `13`, `14` that uses no reference or resource, 27
of them — passes.

**Deviation from the plan:** the "complete typer" (plan D4) is not a
separate pass; it is the lowering walk itself, which computes each
type as it emits (the same expected-type propagation `typecheck` and
the evaluator use). One pass, no map keyed by expression address.

**Bugs found by differential testing in M1:** `saturating_add` on
`u128` used `int_min_max`'s bounds, which are not the 128-bit bit
patterns (`bounds()` in `lower.rs` now is); a qualified variant path
whose enum segment reaches its enum through an `import` is left
unrewritten by `modres` and dispatches by bare variant name, as the
evaluator does.

**Implementation-defined choices for `cobc`** (`spec/22` §5), where
they differ from `coby`'s table above: a `str` literal's bytes live
in the executable's read-only data (`str_ptr` returns that address);
`dangling<T>()` is address 1; layouts are C's, asserted equal to the
spec's. A `fn` value's image is the address of its callable box.
A `handle<τ>` is the thread's id; a `guard<τ>` holds the address of
the mutex's `inner` (`[Repr-Guard]`); a mutex's state cells are zero
and never read (lock state is kept by the runtime, keyed by the
mutex's address). An `extern fn` is a C prototype under a private
name bound to the declared symbol with an assembler label, resolved by
the system linker against libc and libm (`-lm`); any FfiType signature,
pointers included; a symbol the linker cannot find is `unsupported: no
C function …`, exit 3, as in `coby`. Threads run in parallel (Stage 3 T3); every access
to memory two threads can reach is checked under the runtime lock, so
each such step is atomic, as `[Thread-Step]` requires.

### M2: objects, access paths, moves, destruction (2026-09-22)

`cbrt` now holds `spec/04`'s dynamic Σ over real addresses: an object
table (address, type, alive, init, resource, owning frame or statement
scope, root path), an access-path table (target, projection, mode,
valid, ancestors, occurrence count), and a slot table mapping the
address of every stored reference to its token. A path takes part in
`clash`/`solitary` only while its occurrence count is positive — held
by some live object's storage — which is exactly what `interp.rs`'s
`scan_refs` computes by walking values; the check functions are that
code over these tables. Frames and statement scopes are one LIFO
stack: a frame pop ends its owned objects last-established-first
(`[Block-Exit]`), a scope pop ends its temporaries (`[Stmt-Exit]`),
and `cb_fault` folds the whole stack the same way before exiting
(`[Fault-Unwind]`), calling generated destructors through the type
table. A second fault during the unwind exits with the first
diagnostic, unreported.

In generated code: every binding is a C local *and* a runtime object
with a root token (`cb_new`/`cb_bind`); every place access names its
path — the binding's root, or the token of the reference it goes
through — and calls `cb_read`/`cb_write` before the C load or store,
so `[Read]`/`[Write]`'s checks run (stale binding, uninitialized,
clash, overwrite of a live resource). References are `cb_ref`
(pointer + token) in registers and a bare pointer in memory, the token
in the slot table (`cb_store_ref`/`cb_load_ref`; `cb_copy_datum` for
aggregates holding references, driven by the type descriptors).
Resources always travel with their object id: `cb_take` moves out of
a binding (solitary check, root invalidated), `cb_move_to`/`cb_bind`
into a binding, `cb_absorb` into a container field/element/payload,
`cb_send`/`cb_recv` across a call in argument order (an in-flight
FIFO; reference data in plain aggregates travels the same way), and a
block's tail re-stamps its object outward (`cb_result`). `drop(x)` is
`cb_destroy`; `drop(*r)` is `cb_destroy_via` (never solitary while
owned). `match` on a resource enum relocates the payload into the
binder (`cb_end_moved_out` on the scrutinee) or consumes it; a
single-field destructuring relocates the field out the same way.

**Results:** 115 file cases pass, 35 skipped, 0 disagree — and the
compiled suite now compares stdout byte-for-byte with `coby` on every
case and example, not just the outcome. The M2 gate (`07`–`11`): 30
of 35 cases pass; the five skipped all need raw storage (`Vec`),
which is M4. `04_ownership.cb` runs natively with the right
destruction order.

**Differential finds in M2:** `r == r` on a resource must report
`diag.read-of-resource` before either operand is moved (the operand's
resource-ness is now decided by a side-effect-free probe); a `match`
on a non-enum value and `==` on references (`[Eq-Ref]`, by address)
were unhandled.

**Known limits, deliberate:** `[Write]` marks an object initialized
on any write, whole or partial (the interpreter's own conservative
reading); the overwrite check treats every initialized resource
target as live (as `coby` does). Per-thread destroy authority is not
tracked; since `CHG-0030` no program can reach `[Destroy-No-Authority]`
before an earlier check (`STATUS.md` "Stage 2").

### M4: raw storage — `Vec`, `String`, `Rc` compiled as user code (2026-09-22)

The prelude's `Vec`/`String`/`Rc` are CobaltC source and now compile
like any user code; what they needed was `spec/20` §2's raw-storage
story in the runtime. A **reclaimed object** is one at a raw address,
owned by no frame (`storage-kind = reclaimed`): `cb_reclaim(addr, ty)`
attaches one there or re-attaches the existing one, and returns its
root token, so `reclaim<T>(p)` is an ordinary place — borrowable
(`&reclaim<T>(p).value`), destroyable (`drop(reclaim<T>(p))`),
readable. `*p = v` with a resource `v` is `[Rawptr-Move-In]`
(`cb_raw_move_in`: the object moves to the raw address and becomes
reclaimed; a previous identity there ends); `auto x = *p` of a
resource is `[Rawptr-Move-Out]` (`cb_raw_move_out` ends the
reclaimed identity, the value becomes a fresh temporary). `release`
and `deallocate` end every reclaimed object *starting* in the range
(as `coby` does); `allocate` is `std::alloc` with the layout recorded
for `deallocate`, `allocate(0)` is address 1; `copy_raw` moves bytes
and the reference slots inside them, never identities — so after
`Vec::grow` the old buffer's objects end at the `deallocate` and the
new buffer's elements are fresh on the next `reclaim`, exactly the
derivation `conf.e2e-vec-nested-realloc` writes out. Raw reads and
writes of plain values are unchecked C, as `trusted-unchecked` says.

**A performance cliff found by `12_collatz.cb`:** every
`&reclaim<T>(…)` borrow minted a token that was never retired, and
ended objects stayed in the tables, so each aliasing check scanned an
ever-growing list. The spec's own `[Stmt-Exit]` is the fix and is now
implemented: a token records the statement scope it was formed in;
at that scope's exit an *unheld* token (no slot holds it) is retired,
a held one lives on with its holder; a reference leaving a function
travels the in-flight channel like an object (`cb_send_ref`/
`cb_recv_ref`) and a block's reference result is re-stamped outward
(`cb_result_ref`); an ended object's tokens and record are dropped
from the tables; a `while` condition gets a scope of its own per
iteration. Tables are now bounded by what is live.

**Results:** 132 file cases pass, 18 skipped, 0 disagree; 10 of 13
examples run natively with output identical to `coby`'s. Every
remaining skip is closures/fn values/`?` (M3, 9 cases) or concurrency
(Stage 2, 9 cases). With `cargo build --release`, the compiled
`12_collatz.cb` runs in 1.2 s against the interpreter's 2.4 s — with
every dynamic check still performed; Stage 3 elision has not begun.

### M3: closures, fn values, `?` (2026-09-22)

**Closures** are what `spec/15` §6 says they are: a capture struct
plus a call operation. A literal lowers to a C struct `s_closureN`
whose fields are the captures — a `ref<τ, m>` per borrow capture,
with `m` from the same syntactic scan as the interpreter's
`closure_body_writes` (`body_writes` in `lower.rs`), or the moved
value per `move` capture — built exactly like a struct literal
(borrows formed, resources absorbed, reference data stored), always
registered as an object because a call borrows it. The body becomes
`ret cl_N(cb_ref p_self, params…)`, lowered with a scope in which
each captured name is a `CaptureRef(i)` (the place `*self.f_i`,
through the token in that slot) or `CaptureVal(i)` (the place
`self.f_i`, projected from `self`'s token) — the resolver's
`[Closure-Call]` rewrite done in the lowering's place model rather
than in the AST. Calling a closure binding borrows it exclusively
(`cb_borrow` on its root) and calls the body with that reference.
A nested closure capturing a captured name reborrows through the
outer capture, with no special case.

**fn values.** `sizeof(fn) = 8` (`[Sizeof-Addr]`), and a `fn`-typed
parameter accepts a closure (`map_in_place(&mut v, shift)` in
`08_closures.cb`), so a `fn` value is an 8-byte handle to a
*callable box* (`cb_fnbox`): one interned box per fn item
(`cb_fn_item`, so `[Eq-Fn]` is handle equality), or a heap box that
owns a moved-in closure object (`cb_fn_box`: the object relocates
into the box, keeping its identity, reference data, and obligations).
A call through a handle goes through a per-signature thunk
(`cb_call_<sig>`) that dispatches on the box: a closure is called
through a fresh exclusive borrow of the boxed object, an item
directly. A `fn`-typed binding, parameter, or field *owns* its box:
ending it destroys the closure inside (`drop_in_place` on kind
`CB_K_FN`) and frees the box; `[Read]` of a `fn` value (`cb_fn_copy`)
clones a non-resource closure box — its own object and reference
data — and faults `diag.read-of-resource` for a resource-owning one,
which is `coby`'s verdict for reading that closure's binding.
Coercion happens wherever a value meets an expected `fn` type
(`lower_expr` wraps `lower_expr_inner`), so arguments, initializers,
fields, results, and returns all take it without special cases.

**`?`** is `rule.fail.propagate`'s own desugaring, lowered as the
synthesized `match (e) { Ok(v) : v, Err(err) : return Err(err) }`;
**`map_err(r, f)`** relocates `r`'s payload into a fresh `Result<T,
E2>` (E2 from `f`'s type), calling `f` through the thunk or, for a
closure argument, an exclusive borrow of it.

**Results:** 141 file cases pass, 9 skipped, 0 disagree; 12 of 13
examples run natively with `coby`'s exact output (`08_closures.cb`
included). Every remaining skip is concurrency (Stage 2). Stage 1's
finish line — every non-concurrency case, spec row, and example
identical to `coby` — is reached for the file suite and the
examples; a compiled variant of `spec_rows.rs` (plan §4) is still to
be written.

**Deviation from `coby`, deliberate:** a closure owning a resource
that is passed *as a `fn` value* and then read back is
`diag.read-of-resource` here, where the interpreter would move the
closure object (its binding has the closure's own type there). No
case does this; `spec/15` §6 says such a value is passed by move.
**Known limit:** a plain struct holding a `fn`-typed field is copied
by value without cloning the boxes inside (two owners of one box);
no program in the corpus stores fn values in structs.

### M5: every spec row compiled — Stage 1 complete (2026-09-22)

The row assembly that `tests/spec_rows.rs` used privately now lives in
`tests/common/mod.rs`, shared by that runner and by
`cobc/tests/compiled_spec_rows.rs`, so the interpreter and the
compiler check *the same programs* against *the same expectations*:
every row of `spec/conformance.md` is written to a file, run as
`cobc --run`, and compared on outcome, phase, and — for agreeing rows
— stdout against `coby`. Modules and file-backed modules needed no
compiler work: the front end (loader, resolver) is shared, so
`17-modules` and `22-surface-syntax` passed as soon as their
constructs did.

**Results:** 143 spec rows pass, 10 skipped (all concurrency), 0
disagree; 141 file cases pass, 9 skipped (concurrency); 12 of 13
examples native (`09_threads.cb` is Stage 2). That is Stage 1's
finish line (`COBC-PLAN.md` §0) for every non-concurrency case, row,
and example.

**Differential find in M5:** a block or branch whose value is a
resource handed back an object-id local declared *inside* its C
braces; the first consumer after the block was `Vec::pop` at a
resource element type (`conf.vec-pop-resource`), which no file case
had exercised. Sinks now declare the id holder beside the result
temporary (`sink_obj_var`), the pattern `match` already used.

### After M5: two gaps the suites did not cover (2026-09-23)

Both were found by running the HTML guide's examples through both
tools; neither had a case in the file suite or the spec rows.

- **`[Eq-Struct]` in `cobc`.** `==`/`!=` on a struct, array or enum
  was `unsupported: binary operator on an aggregate`. `Gen::eq_fn`
  now writes one `cb_eq_…` function per aggregate type (fields in
  order, elements in a loop, tag then payload), calling the functions
  for its parts, which are written first; `Gen::eq_expr` compares a
  scalar in place (a reference in memory is its bare address,
  `[Eq-Ref]`). Case: `12-type-system/aggregate_eq_ok.cb`.
- **Float literals next to `f32`** (`rule.arith.literal`, both tools).
  An unsuffixed float literal took `f64` whatever the other operand
  was: `a * 2.0` with `a : f32` faulted `diag.type-mismatch` at run
  time in `coby`, and `1.0 < a` was rejected statically by the shared
  typechecker. The expected-type threading in `check_binary`,
  `Interp::eval_binary` and `cobc`'s `is_bare_literal` covered integer
  literals only; it now covers float literals too. Case:
  `06-arithmetic/float_literal_takes_f32_from_operand_ok.cb`.

**Results:** 143 file cases pass, 9 skipped (concurrency), 0
disagree; spec rows and examples unchanged.

**Stage 1 status.** Done: M1–M5. Concurrency followed as Stage 2
(M6–M8, below). Not done, by design: check elision (Stage 3: each
elision a `CHG` to `rule.control.flow-analysis`). Performance
without any elision, release build: `12_collatz.cb` 1.2 s compiled
vs 2.4 s interpreted.

### Stage 3: speed — T0 and T1 (2026-09-23)

`COBC-PLAN.md` §10. `12_collatz.cb`: 0.58 s → 0.155 s (2.97 G → 0.59 G
instructions), `coby` 2.3 s; outcomes unchanged — every suite, the 201
guide programs (identical to `coby`), and 20 repeated runs of each
concurrency program. **T0** (runtime only): the runtime's maps use a
multiplicative hash of their integer keys instead of SipHash, which had
been 54% of all instructions; `cb_read`/`cb_write` check the path in
place instead of copying its projection and ancestors. **T1** (checks
the existing `rule.control.flow-analysis` already proves, which
`spec/14` §6 says are not performed): plain locals that are never
borrowed, captured or dropped have no runtime object and no checks;
statement scopes and block frames whose code forms no temporary or
path are not pushed. The rest of the cost is accesses through
references, which the current analysis never proves; widening it is
T2, one `CHG` per decision. A further runtime pass (id lists reused
from a pool, `cb_borrow`'s check in place) brought it to 0.14 s
(0.52 G). T2 candidates 1–2 (proving accesses through reference
parameters) were shown unsound by counterexample and set aside;
`COBC-PLAN.md` §10 records why.

**T3: parallel threads** (2026-09-23). Stage 2's GIL is gone. The
runtime's shared state is guarded by a reentrant lock held only for
the duration of each runtime call (a standard mutex, which spins
briefly before sleeping) and released in full while a thread blocks;
generated code between runtime calls runs unlocked, so work on
unchecked locals runs truly in parallel. Locking switches on at the
first `spawn`: a program that never spawns takes no lock. Each
thread's frame/scope stack and in-flight queue are its own (the main
thread's in a plain static, a spawned thread's behind a thread-local
pointer), and the executing statement's location is a C thread-local
in the generated program, set inline by `cb_at`. Blocked threads wait
on a key (the thread they join, the mutex they want) and are woken
only by a signal on that key. Four threads each running a 30 M-step
arithmetic loop: 1.23 s under the GIL, 0.39 s now (3.2×, four cores,
no added CPU time). Single-threaded code pays about 5 instructions per
runtime call for the lock-mode check (`12_collatz.cb` 0.14 s → 0.15 s).

**Two older bugs this exposed**, both fixed. (1) Binding a value that
holds a reference copied the reference's slot to the new address and
then re-homed the object's slots onto the same address without
releasing the overwritten one, so the path was counted as held forever:
a correct program could then fault `diag.aliasing-conflict` (a
`resource struct` holding `&y`, ended, then `y = 2` — `coby` `ok`,
`cobc` a conflict; new case
`08-alias-validity/resource_holding_ref_ends_releases_borrow_ok.cb`).
Every guard hit it, so a mutex locked in a loop accumulated dead lock
paths and every check on it slowed down with each iteration.
(2) Invalidated paths that nothing holds are now removed at once (a
later use is `diag.stale-binding` either way).

**Results:** all suites in release and debug builds (the cross-thread
assertion on objects' stack indices never fires), the 201 guide
programs identical to `coby`, each concurrency program 100 times, a
four-thread lock-contention program (80,000 locked increments) 20
times — always the exact count — and the spec's interleaving-dependent
`conf.cross-thread-write-conflict` shape 200 times: always one of its
two permitted outcomes (with real parallelism `cobc` reports the
conflict; `coby`'s GIL lets the thread finish first).

**T1, reference-holding locals** (2026-09-23): a local holding a
reference (or plain data with one inside) that the function never
borrows itself — `&*r` borrows the referent, not `r` — keeps its runtime
object, whose slots make its references held, but accesses to the
variable are not checked: a reference is never consumed, and nothing
else reaches the variable's storage. About 2% of `12_collatz.cb`'s
instructions. **Stage 3 is complete**: the remaining proposals were decided
(delegated by the owner) and declined — `COBC-PLAN.md` §10 "Stage 3
status" and `spec/decisions/D-0022`.

**`Vec` element access realized natively** (2026-09-23,
`spec/decisions/D-0023`): the prelude's own `Vec::index_*` (every `T`)
and `Vec::push`/`Vec::drop` (plain, non-zero-sized `T`) compile to
native bodies with the same outcomes (`spec/21` §0), and plain element
objects end with their last derived path. `12_collatz.cb` 0.15 s →
0.07 s; a 1,000,000-element `bool` sieve 16.2 s / 581 MB → 3.1 s / 3 MB.
A program's own function named `Vec::index_shared` is still compiled as
written. `coby` is unchanged.

**`[Item-Duplicate]`** (2026-09-23, `CHG-0032`): both tools reject a
second declaration of any qualified name, the prelude's included
(`diag.duplicate-item`, static, at the later declaration), in
`lib.rs` `check_duplicate_items` before name resolution. Declarations
now carry their line (`FnDecl`/`ExternDecl`/`StructDecl`/`EnumDecl`
`.line`, `Item::Module`'s fourth field). Previously the last
declaration silently won, even over the prelude's `Vec::drop`.

**The module `std`** (2026-09-23, D-0024 / `CHG-0033`): the prelude
source (`src/prelude.rs`) is wrapped in `export module std { … }`, so
its items are `std::Vec`, `std::print`, …, and `Vec`/`String`/`Rc`
fields are private to it. `modres.rs` resolves every `import` once
(`import_targets`) and adds `[Resolve-Unqualified]` clause 2b (a module
import's exported items) and imported enums to clause 4;
`[Assoc-Fn-Foreign-Type]` is checked in `Resolver::build`. `std::map_err`
stays a native intrinsic in both tools, entered by the resolver as
`std`'s exported item. The names both tools build by spelling
(`Option`, `Result`, `AllocError`, `Vec::…`) are `std::`-qualified. Also fixed:
destructors of structs declared in a module (the signature check
compared a qualified type with the bare name; `coby`'s lookup used the
bare name). The test harnesses begin every inline program and table
fragment with `import std;`.

**Cheaper runtime bookkeeping** (2026-09-23, `COBC-PLAN.md` §10):
- **Arenas.** `cbrt` keeps paths and objects in generation-checked
  arenas (`Arena`: an id is `generation << 32 | slot + 1`, so a stale
  token still finds nothing).
- **Path lists.** Each object keeps its own path list.
- **Ephemeral objects.** Ephemeral element objects live in a hash-map
  index beside the ordered one, have no root path, and are borrowed
  directly.
- **`Vec::index_*` fast path.** A call whose first argument is written
  `&place`/`&mut place` and whose index is stateless calls a `…_pre`
  body after `cb_borrow_check` (the borrow's check and the body's read
  check, no token).

The 1,000,000-element sieve went from 3.12 s to 0.95 s.

**More checks left out where they cannot fail** (2026-09-24,
`COBC-PLAN.md` §10 "Checks proven by `cc`, emptier scopes"):
- **Bounds check inline.** `cb_index_check` is a `static inline` in
  `cbrt.h` (the runtime export is gone), so `cc` drops it where a loop
  guard already proves the index.
- **Scopes settled per function.** `lower.rs` emits scope and frame
  markers and `resolve_scopes` keeps only those some runtime call
  records into; a frame around one busy statement, a branch tail, and
  a function's own frame no longer count as busy. This replaces
  `elide_scope`.
- **Conversions in a fast-path index.** `widen`, `narrow_wrapping`,
  `reinterpret`, and a `narrow` that cannot fail count as stateless in
  a `Vec::index_*` index (`int_range_within`).

`12_collatz.cb`: 246 M → 134 M instructions, 0.042 s → 0.024 s.
Four new cases: `14-control-flow/fault_unwind_through_scope_free_blocks_dynamic.cb`,
`16-aggregates/array_index_past_loop_guard_dynamic.cb`, and two
`21-standard-library-semantics/vec_index_through_conversion*` cases.

**Element accesses fused; `Vec::len` native** (2026-09-24, `COBC-PLAN.md`
§10): `*Vec::index_*(…)` read, or assigned a stateless value, at once,
for a plain element type, compiles to the native body inline plus
`cb_elem_access`. That call does nothing where no live object lies over
the element, and otherwise makes the borrow's and the access's checks
and ends the path. `Vec::len` has a native body (one checked read), and
`Vec::len(&v)` reads in place after `cb_borrow_check`. `in_prelude` also
recognizes a body that is only a tail expression. `12_collatz.cb`:
134 M → 54 M instructions (0.025 s → 0.011 s); `prime_sieve.cb`:
408 M → 118 M. Six new cases in `21-standard-library-semantics/`
(`vec_element_*`, `vec_len_*`).

**Confined local `Vec`s** (2026-09-24, `COBC-PLAN.md` §10): a local
`Vec<T>` (plain, non-zero-sized `T`), initialized where it is declared,
and used only through `Vec::push`/`Vec::len`/`*Vec::index_*` as its
`&x` argument (at most moved out as the function's result), is proven
never to have a path other than its root. So its borrows, the checked
reads and writes in those bodies, and `cb_elem_access` are all left
out. This is `Gen::confined_vecs` plus `Fx::bind_confined`, and the
check-free `Vec::push` is a `static inline` `…_nc`. Also: a
`cb_at` that is overwritten in straight-line code before anything can
read it is dropped (`drop_dead_locations`). The benchmark in
`bench/` (`bench.cb`: sieve of 10,000,000 + Collatz below 1,000,000): 3.65 s
→ 0.63 s (Rust 0.35 s, 0.66 s with overflow checks). Three new cases
(`vec_index_computed_by_pushes_ok`, `vec_local_write_past_end_dynamic`,
`vec_built_locally_and_returned_ok`).

**Assignment checks its write after the value** (2026-09-24, `coby`):
`eval_assign_place` made `[Write]`'s conflict and live-resource checks
before evaluating the value, and never re-checked that the target was
still alive. `*Vec::index_exclusive(&mut v, 0) = { Vec::push(&mut v, …);
… }`, whose pushes reallocate, then panicked with `stale object` in
`write_place`. spec/13 §1 evaluates the target place, then the value,
and `[Write]`'s premises hold at the write. So `coby` now evaluates the
value, then checks `alive` (`diag.stale-binding`), live-resource and
`clash`, then writes, as `cobc` always has. A fault the assignment
raises itself is now reported at the assignment's line, not at the
last line its value reached (`cobc` already did this). Case:
`21-standard-library-semantics/vec_element_write_stale_after_value_pushes_dynamic.cb`.
Also: six file cases cited `rule.alias.clash`, which no spec file
defines (`clash` is spec/08 §1's predicate). They now cite
`rule.alias.borrow` (checked when a borrow is formed) or
`inv.alias-validity` (checked when a path is used). Every `facility:`
id in the suite now resolves to the spec.

**Stricter comparison, and differential testing** (2026-09-24):
- **Error output compared.** The compiled suites (`compiled_suite`,
  `compiled_spec_rows`) now compare `cobc`'s stderr with `coby`'s as
  well as its stdout, so a diagnostic's location is checked too. The
  exception is `conf.cross-thread-write-conflict`, a race whose output
  depends on how the threads interleave.
- **`cobc/tests/differential.rs`.** Random, well-typed programs, run by
  both tools, must agree on exit status, stdout and stderr. The
  programs use pushes and element accesses (half of their vectors
  local-only, so confined), element references held in another vector
  across reallocation, reference parameters, moves, `Tracer` resources
  ending in nested blocks, and arithmetic that may overflow, divide by
  zero or narrow badly. 100 programs from seed 1 by default;
  `COBC_DIFF_N`/`COBC_DIFF_SEED` for longer runs, `COBC_DIFF_KEEP=<dir>`
  to keep them all. Disagreements are kept in
  `target/differential-failures/`. 4,200 programs agree. A run of 2,000:
  about half run to completion, and most of the rest fault at run time
  (bounds, overflow, division by zero, stale references, conflicts).
- **Found by it: `coby` reported many faults at "unknown location".**
  `eval()` tagged an untagged fault with `current_line`, not with the
  faulting expression's own line as its comment said. When an operator
  faulted after an operand's call had run (`Vec::len(&v) - 1` on an
  empty vector), `current_line` was a prelude line, which has no
  location. It now tags with the expression's own line, which also
  covers the assignment case above. `cobc` was already right. Case:
  `06-arithmetic/usize_underflow_after_call_dynamic.cb`.
- **The confined-`Vec` analysis** now treats the arguments of the
  conversions (`widen`, `narrow`, …) and of the integer arithmetic
  intrinsics as values, as `lower_intrinsic` lowers them. So a read
  inside `widen<i64>(*Vec::index_shared(&v, i))` no longer rules `v`
  out.

**Confined vectors passed to functions** (2026-09-24, `COBC-PLAN.md`
§10). A `ref<Vec<_>, _>` parameter that its function uses only as a
confined local may be used is *confining* (`Gen::confining_table`, a
greatest fixed point over the program's functions). A confined vector
passed to it, by a caller that passes it in no other argument of that
call, stays confined, and the call goes to a copy of the function
compiled for such arguments (`request_variant`, named by mask:
`f_mark_c1`), which makes no checks through them. `bench/sieve_fn.cb`
(the sieve in functions, then of 10,000,000): 3.87 s → 0.16 s, the
same as the local version, when measured that day (current figures:
`bench/RESULTS.md`). The differential tester's programs gained helpers that pass the
vector on (`fill_twice`), recurse (`sum_from`), return an element
reference (`first_ref`, not confining), and take two vectors
(`copy_first`), called with two confined vectors, two unrestricted ones,
or one vector twice (a conflict both tools report). Half their vectors
are passed only to confining helpers. 3,000 such programs agree. Cases:
`21-standard-library-semantics/vec_confined_through_helpers_ok.cb`,
`vec_passed_twice_to_helper_dynamic.cb`.

### Closing gaps found while updating the README (2026-09-23)

Checking the README's list of what `cobc` does not compile turned up
reachable gaps in both implementations; each now has a case.

- **`cobc`:** a generic function used as a value is instantiated from
  its explicit type arguments or the expected `fn` type
  (`fn(i32) : i32 f = id;`); `drop(e)` of a temporary destroys it at
  once, and `drop(*p)` of raw storage moves the value out and destroys
  it; a callee that is itself an expression is called — a `fn` value
  through its thunk, a closure temporary through an owner-like path of
  its own (`cb_temp_path`), ending with its statement
  (`15-function-semantics/generic_fn_item_as_value_ok.cb`,
  `07-resource-authority/drop_of_temporary_destroys_it_ok.cb`,
  `20-trust-boundaries/drop_of_raw_place_destroys_value_ok.cb`).
- **`coby`, two resource leaks in move closures.** A closure owning a
  resource was never treated as one — its type tag says "not a
  resource" — so destroying it never destroyed its captures: a captured
  value's destructor never ran. Each call also left the body's view of
  a move-captured value behind, so a reference inside that value stayed
  held for the rest of the run (a later access to the referent was a
  false `diag.aliasing-conflict`). A closure called straight from an
  expression now ends when the call does (the interpreter ends a
  temporary when its consumer is done with it; `cobc` ends it at the
  end of the statement, as `[Stmt-Exit]` says — they differ only if the
  rest of that statement could observe the destruction)
  (`15-function-semantics/move_closure_owning_resource_destroys_it_ok.cb`).
  A spawned move closure likewise ends with its thread, destroying what
  it owns before `join` returns; the thread body now releases the
  per-call views too (a debug-build assertion that none existed hung
  the interpreter's debug build on `thread_resource_authority_rekeyed_ok`)
  (`19-concurrency/spawned_move_closure_resource_destroyed_ok.cb`).
- **Both, `[Eq-Fn]`:** `==`/`!=` on a `fn` value, a closure, or an
  aggregate holding one was accepted; the spec makes it ill-typed. The
  typechecker now rejects it (`diag.type-mismatch`, static;
  `12-type-system/fn_value_eq_rejected.cb`, `closure_eq_rejected.cb`).

**References in raw storage** (investigated 2026-09-23). A value
holding a reference, written into raw cells and read back, lost the
reference in both implementations: `coby` stored a reference as eight
zero bytes with nothing behind them, and `cobc`'s `[Rawptr-Move-Out]`
copied the bytes but ended the reclaimed object's reference slots
instead of moving them to the destination. This broke every `Vec` whose
element type holds a reference (`Vec<ref<i32, shared>>`, a `Vec` of
structs with a reference field) as well as a resource holding one moved
through raw memory. Fixed: `coby` keeps such values in a side table by
cell address (`arena_side`), restored on read, carried by `copy_raw`,
dropped by `release`/`deallocate` and by a move out; `cobc`'s
`cb_raw_move_out` re-homes the slots before ending the identity. Also
fixed in `cobc`: a field access on a reference returned by a call
(`Vec::index_shared(&w, 0).r`) dereferenced the register-form `cb_ref`
as if it were a memory slot and produced C that did not compile.

**Resolved by `CHG-0031`** (owner's decision): a reference stored in
raw storage is held. By the letter of `spec/20` 1.7.0 it was not —
`[Rawptr-Write]` (a plain `Vec` element) made `reclaimed-at(addrs)` the
holder only "if it exists", and after a fresh `push` none did, so
`[Stmt-Exit]` invalidated the reference and `Vec<ref<…>>` was unusable.
Now the write establishes or re-attaches the reclaimed object over the
cells as holder, as `[Rawptr-Move-In]` did for resources. `coby` counts
every raw-stored reference as held and releases it when the object over
its cells ends (destroy, `release`, `deallocate`, a move out); `cbrt`'s
`cb_release` forgets a released range's slots even where no object was
reclaimed. Cases: `conf.vec-of-refs-usable`,
`conf.vec-holds-exclusive-ref-conflict`, `conf.vec-pop-releases-ref`
and four files in `20-trust-boundaries/`.

### Found while writing the showcases (2026-09-23)

`showcase/` (29 programs, each run under both implementations by
`showcase/run_all.sh`) exercised paths the suites had not:

- **`coby`: resources inside reclaimed objects were never destroyed.**
  A reclaimed object's record holds a placeholder; its value lives in
  its raw cells. `destroy_composite` read the record, so a struct with
  no destructor of its own but a resource field — `Rc`'s box, a `Vec`
  element — destroyed nothing: the value inside an `Rc` was never
  dropped. It now reads the cells
  (`21-standard-library-semantics/rc_last_owner_destroys_value_ok.cb`,
  `vec_element_resource_field_destroyed_ok.cb`).
- **`coby`: a temporary read through (`make().name`, `make()[i]`) was
  never ended.** Statements now end such temporaries, and closures
  called straight from an expression, when they finish
  (`[Stmt-Exit]`; `stmt_temps`) — which also moves the earlier
  closure-temporary fix from "after the call" to the statement's end,
  matching `cobc` (`14-control-flow/temporary_read_through_ends_at_statement_ok.cb`).
- **`coby`: nested array literals got no expected element type**, so
  `-1` inside `array<array<i64, 3>, 3>` was an `i32`
  (`16-aggregates/nested_array_literal_takes_expected_element_type_ok.cb`).
- **`cobc`: `?` on an `Option`** was an internal error; it desugars to
  `Some(v) : v, None : return None`
  (`18-error-failure-semantics/propagate_on_option_ok.cb`).

### Real `extern` calls (2026-09-23)

Until now an `extern fn` other than the prelude's `write` could not be
used: `cobc` answered `unsupported`, and `coby` silently returned zero
(a placeholder, never a real call). Both now call the C function named
by the declaration, from libc or libm, on x86-64 Linux:

- **`cobc`** emits a prototype under a private name bound to the
  symbol with a GCC assembler label (so a CobaltC signature never
  collides with the C headers' declaration of the same function) and
  links against libc and libm; every FfiType, pointers included.
- **`coby`** (`src/ffi.rs`) looks the symbol up at the call (`dlsym`,
  libm opened on first use) and calls it through one function-pointer
  type with six integer and eight float parameters: under the x86-64
  System V convention integer and vector registers are assigned
  independently, so that single type reaches any callee with at most
  six integer/`bool` and eight `f32`/`f64` arguments. An `f32` travels
  in the low half of its register; results are read back as an integer
  register, an `f64` or an `f32`. It refuses `rawptr` arguments and
  results — its memory is a simulated arena — with `unsupported: …`,
  exit 3, the convention `cobc` already used.
- **A missing symbol** is `unsupported: no C function `…` in libc or
  libm`, exit 3, from both (`cobc` recognises the linker's undefined
  reference).

Case: `20-trust-boundaries/extern_libc_scalar_calls_ok.cb` (`labs`,
`abs`, `sqrt`, `pow`, `hypotf`, `toupper`, `isalpha`, `fma`).

Also found by the Tier 4 showcases: **`coby` could not reach a field
through a raw pointer**, `(*p).f` — `*p` evaluated to a value, and a
field of a value was a type mismatch. `*p` is a place (`spec/13` §1); a
field access through it now re-attaches the cells as the reclaimed
object over them, as `reclaim<S>(p).f` does, so the field can be read
and written (`20-trust-boundaries/raw_place_field_access_ok.cb`).

**`cobc` does not compile**: nothing on this list remains. The last
entry, a generic function passed as an argument whose instantiation
the call fixes only through a literal or `None` (`twice(id, 10)` for
`twice<T>(fn(T) : T f, T x)`, exit 3 `unsupported`), compiles since
2026-10-02: the item waits for the other arguments, as a bare `None`
does, then takes the type they fixed
(`12-type-system/generic_item_value_inferred_from_call_ok.cb`). The two
entries before it were closed earlier: a thread whose result is a bare reference now
compiles (D-0094 / CHG-0109, 2026-09-29 — stored-form transport
through the thread block, `conf.spawn-ref-result`), and the
specification question about `auto f = id;` was answered by D-0093 /
CHG-0108 (`[T-Item-Value-Uninferable]`): both implementations now
reject it statically, and the instantiated forms are typed by
`[T-Item-Generic]`.

### Stage 2: concurrency — M6–M8 (2026-09-23)

(The GIL described here was replaced by Stage 3's T3, below: threads
now run in parallel. The rest of this section stands.)

`spawn`, `join`, `handle<τ>`, `Mutex::new`, `lock` and `guard<τ>`
compile; `COBC-PLAN.md` §9 has the decisions (S1–S7).

**Runtime.** Every CobaltC thread is an OS thread. One global lock
(the GIL) is held by whichever thread runs generated code or the
runtime; a thread gives it up while blocked (`join`, `lock`, a
handle's destructor, woken by an event raised when a thread finishes
or a mutex unlocks) and, while more than one thread is live, at each
`cb_stmt_push`, handing it to a waiting thread before taking it back.
Each thread's frame/scope stack, in-flight queue and location are
parked while it does not hold the GIL; objects, paths and slots are
shared. A fault reports, unwinds the faulting thread's stack, and
exits holding the GIL, so no other thread takes a step (`spec/18`).

**`spawn`** stores the callee (a `fn` value; a `move` closure is
boxed) and the arguments in a heap block: a reference as a held slot,
so the spawning statement's end cannot retire it; a resource moved in
and detached (`cb_hold`); reference data with its slots. The new
thread runs a generated trampoline that sends the arguments exactly
as a caller would, calls through the callee's thunk, releases the
block's references, leaves the result at the block's start (with its
object, for a resource) and reports done. **`join`** waits, moves the
result (and its object identity) into the joining statement, frees
the block and marks it taken; consuming the handle then runs
`[Handle-Destructor]`, which finds nothing to discard. Dropping or
sweeping a handle waits and destroys an unclaimed result.

**`lock`** checks `[Lock-Reentrant]`, waits until the mutex is free,
and mints the lock path (`inner` of the mutex, exclusive, the locking
reference and its ancestors as ancestors); the guard is that path
stored like a reference, so `*g` lowers as `*r` does and the path
takes part in `clash` while the guard holds it. `[Guard-Drop]`
unlocks and invalidates the path. `sync-exempt` is the one change to
`clash`: paths record the mutex projection they are lock-derived
from.

**Fixed on the way:** `[Sizeof-Mutex]` in both tools. The spec gives
`mutex<τ>` the layout of `struct { τ inner; usize state; }`; both
computed `sizeof(τ) + 8` without the struct's rounding (12 bytes for
`mutex<i32>`, not 16). `cobc`'s layout assertion caught it the first
time a mutex type was emitted. The interpreter test
`e2e_mutex_inner_value_encodes_at_offset_zero` had encoded the old
size and now expects 16; `19-concurrency/mutex_sizeof_struct_layout_ok.cb`
pins the rule in both tools.

**Per-thread destroy authority** (`CHG-0030`, owner's decision
2026-09-23). Neither tool tracks which thread holds an object's destroy
authority. Checking whether that could matter found that the spec made
an ordinary program fault: a `Vec` of resources moved to a thread (or
returned through `join`) and dropped there was
`diag.no-destroy-authority`, because `[Reclaim]` re-attached each
element object with the authority of the thread that pushed it. The
spec now has re-attachment take the authority for the reclaiming
thread (`spec/20` 1.7.0). With that, every instance of
`[Destroy-No-Authority]` is reported first by another check (a moved
binding is stale; a destroy through a reference fails `solitary`; a
double destroy is stale), so not tracking it conforms. Cases:
`conf.thread-vec-of-resources-dropped`/`-returned` and
`19-concurrency/vec_of_resources_*_ok.cb`.

**Results:** 155 of 155 spec rows, 156 of 156 file cases, 13 of 13
examples, and all 201 programs extracted from the HTML guide produce
`coby`'s exact outcome and output. Every concurrency program gave the
same result in 30 repeated runs.

**`Mutex::new` typed as the generic call it is** (owner's decision,
2026-09-23). Removing the concurrency skip exposed
`12-type-system/generic_inference_from_context_ok.cb`'s line
`auto m = Mutex::new(Vec::new());`, which fixes `Vec`'s `T` nowhere:
`[Generic-Call-Uninferable]` (`spec/15` §5, D-0013) makes it
ill-formed, but the typechecker checked every intrinsic's arguments
with inference disabled, so it was accepted, and `cobc` could not lay
out a `Vec<?>`. The typechecker now types `Mutex::new<T>(T v)`
(`spec/21` §0) as a generic call: `T` from an explicit argument, the
expected `mutex<τ>`, or the argument, which is checked with inference
live. The interpreter passes the same `T` down when it evaluates the
argument, so `mutex<i64> m = Mutex::new(5000000000);` no longer faults
`diag.literal-out-of-range` there (it always passed the typechecker
and `cobc`), and `cobc` honours an explicit `Mutex::new<τ>`. The case
now uses `mutex<Vec<u8>> m = …` and `mutex<i64> big = …`; the untyped
form is `generic_call_uninferable_under_mutex_new_rejected.cb` and a
line of the inline test `static_generic_call_uninferable_rejected`.

### Negated literals and unary minus (2026-09-24, D-0025, `CHG-0034`)

`spec/06` now has `[Literal-Negated]`: a minus applied directly to an
integer literal is range-checked as one value, which both tools already
did for `-2147483648`. Checking them against the new clause found three
bugs, all in the shared checker (and `coby`'s evaluator for the first):
- **`i128`:** a negated literal was accepted only when it was the
  minimum, so `i128 x = -1;` was `diag.literal-out-of-range`.
- **Unsigned operands:** `-m` for `u8 m` passed the checker, against
  `[T-Neg]`. `coby` then produced an out-of-range `u8`, and `cobc`
  wrapped `-1` to 255 but faulted on `-0`. It is now
  `diag.type-mismatch`, statically, as is `-0: u8`.
- **Literal `min / -1`:** the literal-operand refutation reported every
  failure as `diag.arith-overflow`, so `-2147483648 / -1` was not
  `diag.div-overflow`. `min % -1` with literal operands was not refuted
  at all, and now is.

Cases: seven `conf.*` rows in `spec/conformance.md` §1, with inline
tests, and `06-arithmetic/negated_literal_min_ok.cb`,
`negated_literal_parenthesized_rejected.cb`,
`div_overflow_literals_static.cb` and
`12-type-system/neg_unsigned_rejected.cb`.

### Type limits and intrinsic operand typing (2026-09-24, D-0026, `CHG-0035`)

`min_value<T>()` and `max_value<T>()` give an integer type's bounds.
The checker types them (`T` explicit and an integer type), `coby`
reads them from `int_min_max`, with `u128`'s maximum as its bit
pattern, and `cobc` emits the constant through `bounds` and `c_int`.

The same change makes the conversions' and alternative operations'
operand types a static check (`[T-Convert]`, `[T-Alt]`,
`check_integer_intrinsic_operands`), at each instantiation. Before it,
`checked_add(1.0, 2.0)` or `widen<i64>(1.5)` faulted
`diag.type-mismatch` at run time in `coby` and was an internal error
in `cobc`, the same through a generic body's `T`, and
`wrapping_add(1: i32, 2: i64)` was accepted by both. An unsuffixed
literal operand still takes the other operand's type.

Cases: eleven `conf.*` rows in `spec/conformance.md` §1, with inline
tests, and `06-arithmetic/type_limits_ok.cb`,
`12-type-system/limits_generic_not_integer_rejected.cb` and
`alt_generic_non_integer_rejected.cb`.

### Conversion directions, call arity, intrinsic names (2026-09-24, D-0027, `CHG-0036`)

Three gaps found while adding the limits:
- **Conversion directions.** `widen` outside `[Widen]`'s containment was
  accepted: `widen<u64>(-5)` faulted in `coby` and gave
  18446744073709551611 in `cobc`. The checker now enforces `[Widen]`'s
  containment and `[Reinterpret-Sign]`'s width and signedness. D-0027
  lets `narrow` and `narrow_wrapping` take any integer pair, so
  `narrow<u64>(n : usize)` is portable. Three example programs changed
  four `widen`s to `narrow`.
- **`[T-Call]`.** Neither tool checked argument counts, and a call of a
  non-callable value faulted in `coby` and was an internal error in
  `cobc`. The checker now rejects both, for functions, `extern`
  functions, `fn` values and closures (a closure's parameter count is
  followed through `auto d = c;`). `coby` checks a closure call's count
  as well, instead of indexing past the arguments.
- **Intrinsic names.** `coby` looked intrinsics up before the program's
  own names, so `fn widen()` panicked it. The checker and `coby` now
  look a callee up as a local binding, then an item, then an intrinsic,
  as `cobc` did (`spec/21` §0).

Cases: nine `conf.*` rows in `spec/conformance.md` §1, with inline
tests, and `06-arithmetic/widen_not_contained_rejected.cb`,
`narrow_any_integer_pair_ok.cb`,
`15-function-semantics/call_arity_rejected.cb` and
`21-standard-library-semantics/item_named_like_intrinsic_ok.cb`.

### Generic `print` (2026-09-24, D-0028, `CHG-0037`)

`print<T>(T x)` prints `str`, every integer type, `f32`/`f64`, `bool`
and `ref<String, shared>`. Like `map_err` it has no CobaltC body: the
resolver enters `std::print`, the checker types it (anything else is
`diag.type-mismatch`, per instantiation for a generic `T`), and each
implementation realizes it. Its `str` and `String` cases call two
private `std` functions, `print_str` (the old body) and `print_string`
(`write` over the `String`'s bytes); `cobc` emits a call to those, or
to `cbrt`'s `cb_print_int`, `cb_print_f64`, `cb_print_f32` and
`cb_print_bool`.

Float text is one function, `format_float`, identical in `value.rs`
and `cbrt`: Rust's `{:e}` for the shortest digit count, then `{:.*e}`
at that count so that exact ties round to even, as `[Print-Float]`
requires (`{:e}` alone breaks them away from zero, e.g.
`161305489493093.125` → `…093.13`). Checked on 9,000 `f64` and `f32`
values against Python's shortest representations: no difference, and
`coby` and `cobc` agree on all of them.

`impl/tests/print_output.rs` checks `print_output_ok.cb`'s exact
output; the compiled suite compares `cobc`'s with it. Cases: eight
`conf.print-*` rows in `spec/conformance.md` §11, with inline tests.

### Found while writing the tier-3 showcases (2026-09-24)

Six new showcases (`hash_map`, `big_integers`, `shortest_paths`,
`sudoku`, `crc_rle`, `parallel_sort`) exercised paths the suites had
not. `cobc` ran all of them correctly; `coby` and the shared checker did
not:
- **`[T-Alt]` literal operands.** The checker range-checked an
  unsuffixed literal operand of `wrapping_*`/`saturating_*`/`checked_*`
  as `i32`, so `wrapping_mul(h, 1099511628211)` was
  `diag.literal-out-of-range`, and `wrapping_add(x : u8, 300)` was
  accepted. The literal now takes the other operand's type, in the
  checker and in `coby`'s evaluation, and these operations have a
  result type (`τ`, or `Option<τ>`) so a literal beside a nested call
  is typed too.
- **A temporary `match` scrutinee** that holds a reference was never
  ended, so its reference stayed live and destroying the referent later
  failed `diag.destroy-while-aliased`. It now ends with its statement,
  and a block's trailing expression now ends the temporaries it made
  (`rule.control.stmt`), keeping those its result refers into.
- **`return v;` from a nested block** destroyed `v`'s resource while
  unwinding the frames it left (`diag.stale-binding` at the caller).
  Frame pops on the way out now keep the returned temporary.
- **A call returning a plain enum or struct held in a local** came back
  as an untyped value, and `match` on it failed `diag.type-mismatch`.
  It now comes back as a temporary of the declared return type.

Cases: `14-control-flow/match_temporary_ref_scrutinee_ends_ok.cb`,
`15-function-semantics/return_values_from_nested_blocks_ok.cb`,
`06-arithmetic/alt_literal_takes_operand_type_ok.cb`,
`alt_literal_out_of_range_rejected.cb`; each fails on the previous
`coby`.

**Enum literals and expected types** (fixed the same day). An expected
type never reached a generic enum literal's payload: `Option<u64> x =
Some(5000000000);` was `diag.literal-out-of-range`, and so was the same
literal in an assignment, a return, an argument, a field, a nested
literal and an `==` operand. The checker and `coby` now take the enum's
unwritten type arguments from the expected type; `==`/`!=` passes each
operand's type to an enum literal on the other side (in all three
tools: `cobc` computed `Some(5000000000) == x` with an i32 payload); a
generic call's argument expects its parameter's type once the
arguments to its left fix it; and `coby` keeps a declaration's written
type on the object it binds (`Option<u64> x = None;` was an
`Option<?>`, so a later `x = Some(…)` had nothing to go by). Case:
`12-type-system/enum_literal_takes_expected_type_ok.cb`
(`conf.enum-literal-expected-type`).

### `cobc` compile time: exponential probing (2026-09-25)

`showcase/tier3/tinyos.cb` took 92 s to compile (the executable ran in
0.02 s). `cobc` learns a branch's type by lowering it into a discarded
buffer (a probe), then lowers it again for real. `lower_if` probed both
branches, `lower_match` every arm, and `lower_block` its body, so each
level of nesting doubled the work: a 14-deep `else if` chain took 3.9 s,
and every extra level twice as long. Now:
- `lower_if` probes the else branch only when the then branch never
  finishes (otherwise the result type is the then branch's);
- `probe_type` and `lower_block`'s probe are cached per function body,
  by expression (or block) and expected type. The cache is keyed by
  address, so expressions `cobc` synthesizes (a `then` block's
  expression, `?`'s match, closure captures) are kept alive for the
  body's lifetime, and a `then` block's expression is made once;
- `lower_match` stops probing arms once the result type is known.

TinyOS compiles in under 3 s; nested `if`, `match` and `else if`
shapes compile in linear time. Cases: `14-control-flow/
if_else_chain_deep_ok.cb`, `if_nested_deep_ok.cb`,
`match_nested_deep_ok.cb`, each of which would take minutes to hours
under the old lowering.

### Branch-free checked `if`: investigated, not done (2026-09-25)

With hardware counters (`bench/README.md`), `collatz`'s gap to
unchecked Rust turned out to be ~40 million mispredictions of its
even/odd `if`, not the checks' instructions. Findings:
- GCC 11 (`cc` here) keeps that branch even in unchecked code
  (`bench/collatz_wrapping.cb`: 0.458 s; the same generated C under
  Clang: 0.24 s, like Rust). `cb_at` stores in the branches are not the
  cause: removing them changed nothing.
- Hand-transforming `cobc`'s checked output to compute both sides with
  their overflow results, select without a branch, and fault only if the
  selected side overflowed (the overflow test for `* 3` as a comparison
  against `UINT64_MAX / 3`, not `__builtin_mul_overflow`, which puts a
  128-bit multiply on the loop's critical path) gave 0.355 s under
  Clang, but nothing under GCC: GCC re-branches the ternary (0.63 s),
  ignores `__builtin_expect_with_probability`, a masked select is slower
  (0.55 s), and a forced `cmov` in inline assembly gives 0.456 s against
  0.47 s. Clang as `cobc`'s C compiler loses 50% on the sieves.

So the transformation is not implemented. Revisit if `cobc` gets a
back end other than GCC 11, or GCC's if-conversion improves.

### `cobc --cc COMPILER` (2026-09-25)

`cobc` runs `cc` by default; `--cc COMPILER` (or `--cc=COMPILER`) runs
another C compiler, a name on PATH or a path, with the same options.
Checked with Clang: every conformance case, example and showcase (291
programs) compiled with `--cc clang` behaves exactly as under `coby`.

That check found one portability bug in the generated C. `to_int`'s
range test wrote an integer type's minimum as a bare decimal literal:
`(double)-9223372036854775808` is a minus applied to a literal that
fits no signed C type, which GCC made 128-bit (so it worked) and Clang
made unsigned, so the bound became +2^63 and every `to_int<i64>` faulted
`diag.narrowing-overflow` (`06-arithmetic/float_literal_takes_f32_from_
operand_ok.cb`, `showcase/tier4/ffi_math.cb`). The bounds are now typed
constants (`c_int`), as the test's other half already was.

Since then every compiled test runs under both compilers: the three
`cobc` suites (`oracle::c_compilers`, default `gcc,clang`, each
compiler in its own thread, overridable with `COBC_TEST_CCS`), the
showcase runner and the guide's example checker (`COBC_CCS`). A
compiler that is not installed fails the suites rather than being
skipped.

**Output path that is a directory** (2026-09-25). `cobc tinyos.cb`
defaulted its executable to `tinyos`, the directory holding TinyOS's
modules, so the linker failed with `cannot open output file tinyos: Is a
directory`, after a stray `tinyos.c` had been written. `cobc` now checks
first and says to choose a name with `-o`; nothing is written.
`cobc/tests/driver.rs` covers it.

### Standard input: `read` and `read_line` (2026-09-25, D-0029, `CHG-0038`)

`std` gains `ReadError`, the `extern` `read(rawptr<u8> buf, usize len)
: isize` (standard input's next bytes; 0 at its end, −1 on an error)
and `read_line() : Result<Option<String>, ReadError>`, written in
CobaltC over `read`. `read_line` reads one byte per call into a byte
it `allocate`s (an extern may write only outside live objects), stops
at `\n` or the end of input, and validates the line with
`String::from_utf8`.

- **`cobc`** compiles `std::read` to `cb_read_in` (`cbrt`), as it
  compiles `std::write` to `cb_write_out`. `cb_read_in` flushes standard
  output, so a prompt appears before the program waits, and reads Rust's
  buffered stdin, so a byte per call is no system call per byte. 20,000
  lines of 60 bytes take 1.2 s, the cost of `std`'s CobaltC loop.
- **`coby`** reads input too since D-0050 (below); it first refused it
  (exit 3). A program's own `extern fn read` is not `std`'s and is
  handled as before.

Conformance cases that read standard input carry a `stdin-hex:` header
(`impl/conformance/README.md`). `coby`'s runners require the refusal;
the compiled runners feed the input and compare the output with
`expect-stdout-hex:`, since there is no interpreter output to compare
with. Cases: `21-standard-library-semantics/read_line_lines_ok.cb`,
`read_line_end_of_input_ok.cb`, `read_line_invalid_utf8_ok.cb`, and two
spec rows (`conf.read-line-typed`, `conf.read-outside-unsafe-rejected`).
`cobc/tests/read_input.rs` checks `Err(Io)`, with a directory as
standard input.

### The program's own C code: `extern "…";` (2026-09-25, D-0030, `CHG-0039`)

`extern "…";` is an item naming foreign code. The parser reads it
(`Item::ExternCode`; an `export` before it is a parse error) and the
item table keeps each string with its line (`Items::extern_code`).

- **`cobc`** (`cobc/src/link.rs`) resolves every declaration before
  writing any C. A string beginning `./`, `../` or `/` is a file,
  relative to the declaring file (found through the loader's source
  map, `coby::locate`): a `.c` file is compiled on its own (the
  program's `-O`/`-march`, the C compiler's default warnings, shown); a
  `.o` or `.a` is passed as it is; a `.so` by absolute path with
  `-Wl,-rpath,<its directory>`, so the executable runs from any
  directory. Any other string is a library name, passed as one
  `-lNAME` argument; a name with characters `cc -l` would not take, or
  a leading `-`, is refused. The results go after the generated C,
  before `-lcbrt`. Each file (canonical path) and name is linked once.
  A missing file, another kind of file, a malformed string, or a
  library the linker cannot find (`cannot find -lNAME`) is reported as
  `cobc: extern "…": …` at the declaration's `file:line`, exit 2; a
  missing function is `unsupported: no C function `f` in libc, libm or
  the program's C code`, exit 3.
- **`coby`** accepts and ignores the declaration (the owner's
  decision). Calling a function only that code provides stops as
  before, exit 3, and the message now names the declared code, "which
  only cobc links".

Conformance cases only `cobc` can run are now marked `cobc-only:
<reason>` (`impl/conformance/README.md`); the `read_line` cases carry
it too, in place of `stdin-hex:` alone. Cases:
`20-trust-boundaries/extern_code_c_file_ok.cb` (a C file named by the
case and, relative to itself, by a module in `extern_code/`),
`extern_code_library_name_ok.cb` (`extern "m";`, run by both),
`extern_code_export_rejected.cb`. `cobc/tests/extern_code.rs` checks
`.o`, `.a` and `.so` files, a library name, a module-relative path,
duplicates, a `.so` executable run from `/`, and every error.

### Program arguments and exit status (2026-09-25, D-0031, `CHG-0040`)

`fn main() : u8` sets the exit status; `fn main()` still exits 0. The
shared front end now checks `main`'s signature (`typecheck.rs`): a
root `main` with parameters or another return type is `diag.no-main`,
located at that `main`. Before this, both tools accepted
`fn main(i32 x)` and `fn main() : i32`.

`std` gains the `extern` `arg_bytes(usize i, rawptr<u8> buf, usize
len) : isize` (copies up to `len` bytes of argument `i`, returns its
whole length, or −1 when there is none), and `arg_count() : usize` and
`arg(usize i) : Result<String, Utf8Error>`, written in CobaltC over it.
Argument 0 is the first after the program: the program's name is left
out, since it differs between the tools.

- **`coby FILE ARG…`** passes `ARG…` (read with `args_os` and
  `as_encoded_bytes`, so on Unix bytes that are not UTF-8 arrive as
  they are; on Windows an argument arrives as UTF-8, or as WTF-8 that
  `arg` rejects if it has an unpaired surrogate; no platform-specific
  code, so coby still builds on Windows) and exits with `main`'s
  status. `Interp::prog_args` holds them; `call_extern` realizes
  `std::arg_bytes` by writing the arena. `run_main` returns the status;
  `Outcome::Ok` and `OutcomeLocated::Ok` carry it;
  `run_program_with_args` is the entry point `main.rs` uses.
- **`cobc`**: the generated C `main` takes `argc`/`argv` and hands them
  to `cb_set_args` (`cbrt`, copied once, without `argv[0]`);
  `std::arg_bytes` compiles to `cb_arg_bytes`; `cb_terminate_ok` takes
  the status, `main`'s value for `fn main() : u8`. `cobc --run FILE
  ARG…` passes `ARG…` to the program, so options go before the file;
  a second file name is now a usage error rather than replacing the
  first.

The file-case runners read an `args:` header (spaces between
arguments, `\xHH` for a byte, `""` for an empty one;
`tests/common/mod.rs` `case_args`) and pass it to both tools; an `ok`
run with a nonzero status and nothing on stderr is `exit N`. The
spec-row runners' `Exp::Ok` carries the status, from an outcome
reading `ok, exit status n`. On Windows, which cannot pass an
argument that is not UTF-8, the file suite skips
`arg_invalid_utf8_ok.cb` and says so; the spec-row runner still runs
it, handing the bytes to the interpreter directly. Cases:
`21-standard-library-semantics/args_read_ok.cb`,
`arg_invalid_utf8_ok.cb`, and eight spec rows (`conf.arg-*`,
`conf.main-*`), each also a test in `tests/conformance.rs`. The guide checker (`gen.py`) gained `args=` and
`status=`.

### Text conversions, and own variants first (2026-09-25, D-0032, `CHG-0041`)

`std` gains `ParseError { Empty, Invalid(usize), OutOfRange }`,
`String::new`, `String::as_bytes`, `String::append<T>` (for `print`'s
types: adds `text(x)`) and `parse<T>` (integers, `f32`, `f64`; strict,
correctly rounded).

- **`String::append`** has a body in `std` that calls the std-only
  intrinsic `append_native`, which each tool dispatches as it does
  `print`: a `str` to `String::append_str`, a `ref<String, shared>` to
  `String::append_string` (both CobaltC in `std`), anything else to
  `String::append_text<T>`, which `allocate`s 64 bytes, has the
  std-only `text_write` format `x` into them (`coby`: `number_text`, the
  function `print` now uses too; `cobc`: `cb_text_int`/`_f64`/`_f32`/
  `_bool` in `cbrt`), and pushes them.
- **`parse`** is CobaltC in `std` over the std-only `parse_check<T>`
  (-1, -2 empty, -3 out of range, else the invalid offset) and
  `parse_value<T>`, reading the `String`'s buffer. Both tools scan with
  `src/numtext.rs`; `cbrt` includes the same file (`#[path]`), so
  neither can drift from the other. Floats are converted by Rust's
  correctly rounded `str::parse` once the scanner has accepted the text.
- The four intrinsics are `diag.unbound-name` outside `std`
  (`typecheck.rs` `STD_ONLY_INTRINSICS`); `text_write`, `parse_check`
  and `parse_value` need `unsafe`. `check_user_call` checks `T` of
  `std::String::append` and `std::parse` at the call
  (`[Append-Not-Printable]`, `[Parse-Not-Numeric]`), so a bad `T` is
  reported in the program, not inside `std`.
- **Variants** (`spec/17` clauses (4), (4b)): `modres.rs` looks for a
  bare variant in the module's enums, then outward to the nearest
  module that has one, and only then among imported enums. A variant
  name that more than one enum of the program has is rewritten to
  `E::V`, since execution dispatches a bare variant through the flat
  `enum_of_variant` table. Without "nearest", `std`'s own `Empty` in
  `parse` collided with a root enum's `Empty`, the root enclosing `std`.
- **Fixed on the way:** `resolve_qualified` did not accept a variant as
  the last segment, so `ParseError::Empty` stayed unresolved and ran as
  whichever enum the flat table held for `Empty`. `check_user_call`
  (and the enum-construct and struct-literal inference) compared
  `m.len()` with the number of type parameters, but `unify_type_shape`
  also enters non-generic type names such as `String`, so
  `fn W::put<T>(ref<W, exclusive> w, T x)` could not infer `T`.
- The spec-row assembler (`tests/common/mod.rs`) ends an `import`
  item at its `;`, as it does `extern`.

Cases: `21-standard-library-semantics/text_append_parse_ok.cb` (every
integer type's limits, floats in each of `print`'s forms, each
`ParseError`), and twelve spec rows, each also a test in
`tests/conformance.rs`. `numtext.rs` has its own unit tests.

### C calls on Windows (2026-09-25)

`coby` calls C functions only on x86-64 Linux (`src/ffi.rs`); elsewhere
it refuses them, `unsupported: extern fn `f`: coby calls C functions
only on x86-64 Linux`, exit 3. `extern_code_library_name_ok.cb` and
`extern_libc_scalar_calls_ok.cb` therefore failed
`cb_conformance_suite` on Windows (the runner stopped at the first).
The file suite and `tests/spec_rows.rs` now skip a case that gets that
refusal, where `common::COBY_CALLS_C` is false, and print which; on
x86-64 Linux the refusal cannot happen and the cases must pass.

### Showcase Tier 5, and what it changed (2026-09-25, D-0033, `CHG-0042`)

`showcase/tier5/` holds four command-line tools (`stats`, `calc`,
`convert`, `table`), each run several times from its `.args` file by
`run_all.sh` (one run per line; `(none)` is a run without arguments;
the `.out` records every run's command, output and exit status). They
read no standard input, per `showcase/SHOWCASE_INSTRUCTIONS.md`.

Writing them led to D-0033:

- **Assigning to a moved-from or dropped binding** (`spec/11`
  `[Assign-Reestablish]`). typecheck: a single-name assignment target
  skips the stale lookup (`Body::reinit_target`) and sets
  `valid := T` after the write. `coby`: frames record each binding's
  type (`Frame::types`); `eval_assign` asks after the right side
  whether the binding's value has gone (`binding_gone`: moved, or its
  object ended) and if so `reinit_binding` stores a new object and
  moves the binding into the frame that declared it. So `x = f(x)`,
  accepted statically before but faulting at run time, now works.
  `cobc`: `assigned_names` finds the names a function assigns as a
  whole; each binding of one records `cb_frame_here()`; the assignment
  calls `cb_rebind(root, &c, ty, frame, loc)` before `cb_write`, which
  returns the root while its object is alive and otherwise binds a
  fresh uninitialized object in that frame (path tokens are
  generational, so a stale root is never mistaken for a reused one).
  A live resource is still never overwritten.
- **`b'x'`**: the lexer gives `Tok::Int(v, Some("u8"))`.
- **Float exponents**: the lexer; typecheck now range-checks float
  literals (`1e400`, `1e39: f32` are `diag.literal-out-of-range`; both
  tools had let them be infinite). `coby` also left an unsuffixed float
  literal that its context types `f32` as an `f64` value
  (`f32 y = 16777217.0` printed `16777217.0`); `eval_expected` now
  rounds it. Known: a literal is lexed to `f64` and then rounded to
  `f32`, so a pathological literal could be double-rounded.
- **`Option::unwrap_or`, `Result::unwrap_or`**: `std` source.

The Tier 5 programs were then simplified with all four; their output is
unchanged. Cases: fourteen spec rows (one replacing
`conf.reassign-after-drop-rejected`), each also a test in
`tests/conformance.rs`, and four file cases in `22-surface-syntax`.

Also noticed, and fixed next (below): a fault raised inside `std`
(`arg(0)` with no arguments) was reported at "unknown location".

### Faults inside `std` report the calling line (2026-09-25)

A fault raised in `std`'s code (`arg` out of range, `Vec::index_*`'s
bounds check, `fault(alloc_failure)`, ...) used to print "at: unknown
location" in both tools. Now it is reported at the program's own line
that called into `std`. `coby` tags a fault with a line only if the
line is the program's (`user_line`: past the prelude), so an untagged
fault from `std` reaches the program's calling expression and takes
its line. `cobc` sets the thread's location (`cb_at`) only at the
program's own lines (`Fx::at`), including at each call into `std`
(`lower_user_call`), and no longer resets it to "no location" inside
`std`'s bodies or the native `Vec` code inlined for them; the runtime
reports a fault without a location of its own at `cb_at`. No
diagnostic id or phase changes. `lib.rs`
`fault_inside_std_reports_the_calling_line`.

### Files: `read_file` and `write_file` (2026-09-25, D-0034, `CHG-0043`)

`std` gains `FileError { NotFound, Denied, Io, Utf8(Utf8Error) }`,
`read_file(ref<String, shared>) : Result<String, FileError>` and
`write_file(ref<String, shared>, ref<String, shared>) : Result<void,
FileError>`, written in CobaltC over two externs private to `std`:
`file_read(path, path_len, buf, cap) : isize` (copies up to `cap`
bytes, returns the file's length; `read_file` starts with a 4 KiB
buffer and reads again with exactly the length if the file is longer)
and `file_write(path, path_len, buf, len) : isize`. `src/fileio.rs`
does the access (`std::fs`, errors mapped to -1/-2/-3) for `coby`
(`call_extern`) and, through `#[path]`, for `cbrt`
(`cb_file_read`/`cb_file_write`). Both tools read and write files, on
Windows too (`coby`). `read_line` now writes `ReadError::Io` and
`ReadError::Utf8`, since `FileError` shares those variant names.

- `cobc`: a parameter of type `void` was emitted as `void p_x`, which
  C rejects (`Result::unwrap_or(write_file(…), ())` instantiates
  `T = void`); it is now `cb_unit` (`param_ctype`).
- Runners: every file case runs with its own directory as the working
  directory (`common::case_dir`), and `$TMP` in `args:` is a fresh
  directory per run (`common::fresh_dir`), substituted after splitting
  the header. `tests/spec_rows.rs` sets the process's directory around
  each file row. The guide checker runs each example in its `check/`
  directory.
- Cases: `21-standard-library-semantics/files_round_trip_ok.cb` (with
  `file_fixtures/not_utf8.txt`) and four spec rows, each also a test in
  `tests/conformance.rs`; `fileio.rs` unit-tests `Denied` and a
  directory (`Io`) on Unix, skipping `Denied` when run as root.
- Showcase: `tier5/wc.cb` reads `tier5/wc_data/`.

### Radix literals, compound assignment, `for`, `const` (2026-09-25, D-0035, D-0036, `CHG-0044`)

- **Lexer:** `0x`/`0o`/`0b` integer literals (a suffix as for decimal;
  an empty or bad digit is a lex error); tokens `+= -= *= /= %= &= |=
  ^= <<= >>=`; keywords `for`, `const`. The numeric suffix scan is
  `lex_suffix`.
- **Parser:** `p op= e` becomes `p = p op e` when `p` has no call
  (evaluating it again is the same place) and `{ auto __compoundN =
  &mut p; *__compoundN = *__compoundN op e; }` when it has one.
  `for (init; cond; step) body` becomes a block holding `init` and
  `ExprKind::While(cond, body, Some(step))` (the third field is new);
  a missing `cond` is `true`. `const τ N = e;` is a `FnDecl` with no
  parameters, body `e`, `is_const: true`.
- **The step:** `coby` runs it after the body and after `continue` as
  a statement (`eval_while`); typecheck checks it on the join of the
  body's end and every `continue` (`check_while`); `cobc` puts a label
  before it and lowers a `for`'s `continue` to `goto` it (`loops` now
  holds that label).
- **Constants:** `modres` rewrites a path resolving to a constant to a
  call of it (`Resolver::consts`), so locals still shadow it.
  `consts.rs` (after `modres`) rejects cycles (`diag.const-not-
  constant`) and folds integer, float, `bool` and `str` constants with
  the checked arithmetic of `spec/06`, typing literals as a declaration
  would (a bare literal takes the other operand's type, else the
  default), and rewrites each use to a suffixed literal; an overflow,
  division by zero or bad shift is reported at the constant. Other
  constants stay calls. typecheck rejects a resource type or a
  non-constant initializer (`const_expr`). An assignment whose left
  side is not a place is now a static `diag.type-mismatch` (it was a
  run-time one).
- Two bare literals in an operator ignored the declared type
  (`u64 x = 1024 * 1024;` was a type mismatch); changed next (D-0037).

### Literal expressions take the expected type (2026-09-25, D-0037, `CHG-0045`)

`ast::is_literal_expr`: an unsuffixed numeric literal, or `-`, `~`,
parentheses or an arithmetic/bitwise operator over literal expressions
only (a shift's amount aside). typecheck (`check_binary`, now given the
expected type) checks such an operator's operands with an expected
number type; `coby`'s `eval_expected` evaluates them with it (and a
negation or `~` of one); `consts.rs` types one with the constant's
type. `cobc` already lowered an arithmetic operator's operands with its
expected type, so it needed nothing: the front end had rejected these
programs before it saw them. The random differential test (seed 92)
found `coby`'s new path leaving an overflow in such an expression
without its line; it is tagged now (`tag_at`), with
`06-arithmetic/literal_expression_overflow_dynamic.cb` guarding it.

`for (…);`, `while (…);` and `if (…);` (C's empty-body trap) were
already parse errors, a body being a block; the parser now names the
mistake (`no_empty_body`: "`;` after `for (…)`: a `for` body is a
block, so `for (…);` is not an empty loop; remove the `;`"), with a
file case for each in `22-surface-syntax`.

### Formatted output: `printf`, `String::appendf` (2026-09-25, D-0038, `CHG-0046`)

`impl/src/fmt.rs` parses a format (`%[- 0 + space #][width][.prec]conv`,
conversions `d i u x X o b f F e E g G s`, `%%`; a flag its conversion
does not take, or a width/precision above 4096, is an error) and
renders it with C's text (unit tests against C's own output; `%#o`
writes `0o`, and widths count characters). `modres` registers
`std::printf` and `std::String::appendf` and rewrites each call:
`printf(f, a…)` → `std::print($fmt(f, a…))`, `String::appendf(s, f,
a…)` → `std::String::append(s, $fmt(f, a…))`. `$fmt` cannot be written
in a program (`$` is not an identifier character). typecheck checks it
(`f` a literal that parses: `diag.format-invalid`; one argument of the
specifier's kind each: `diag.type-mismatch`; a type parameter at each
instantiation) and types it `str`. `coby` renders to a `Value::Str`
(a `&String` argument's bytes read from the arena, `string_bytes`);
`cobc` builds a `cb_fmt_arg` array and calls `cb_format`, which renders
into a per-thread buffer that the following `print`/`append` copies at
once (nothing else can hold a `$fmt` `str`). Case:
`21-standard-library-semantics/printf_output_ok.cb` (numeric lines
compared with C's `printf`). The Tier 5 `wc` now writes its columns
with `printf`.

### One way to write output: `printf`, `%v`, `sprintf` (2026-09-25, D-0039, `CHG-0047`)

`print` is no longer exported from `std` (`print(x)` is
`diag.unbound-name`, `std::print` `diag.name-not-visible`); it remains
what `printf` is rewritten to. `fmt.rs` gains `%v` (any printable type,
width and `-` only), rendered through `format_float`, which moved there
from `value.rs` and cbrt so all three share one implementation; cbrt's
`cb_format` takes the same kind. `modres` rewrites `sprintf(f, a…)` →
`std::String::from_str($fmt(f, a…))`. Every program in the repository
was migrated (`print(x)` → `printf("%v", x)`, a literal → `printf` of
it with `%` doubled), then adjacent `printf` statements whose arguments
cannot print were merged into one; showcase and guide outputs are
unchanged. `ffi_math` writes its table with `%.6f`. The hand-written number and
hex printers (`put_byte`/`digits`/`print_*` in `cobaltc_examples`, the
showcases' hex helpers, the guide tour's `out` module, `bench/out.cb`)
were then replaced by `printf` specifiers, outputs unchanged; what
remains writes raw bytes on purpose (`hex_printer`'s ASCII column, the
guide's "Printing bytes") or feeds tinyos's simulated process output.

### Standard error: `eprintf` (2026-09-25, D-0040, `CHG-0048`)

`std` gains a private extern `write_err` and helper `eprint_str`;
`modres` rewrites `eprintf(f, a…)` to `std::eprint_str($fmt(f, a…))`.
`coby` writes `write_err`'s bytes to its standard error, `cobc` calls
cbrt's `cb_write_err`. Case: `eprintf_stderr_ok.cb` (standard error
checked by `tests/print_output.rs`; `compiled_suite` compares `cobc`'s
with `coby`'s). The Tier 5 tools write usage and error messages with
`eprintf`. `std::exit` was reconsidered and not adopted (D-0031 stands).

### Hash tables: `HashMap<K, V>`, `HashSet<K>` (2026-09-25, D-0041, `CHG-0049`)

`std` gains `HashMap` and `HashSet`, written in CobaltC (src/prelude.rs):
keys and values in two `Vec`s in insertion order, an open-addressing
table of entry indices (linear probing, at most three-quarters full,
doubling, backward-shift deletion). `remove` moves the last entry into
the gap (constant time); `remove_ordered` keeps the order (linear). Two
std-only intrinsics do what depends on `K`: `key_hash` (FNV-1a over the
key's bytes) and `key_eq`; `coby` reads the bytes from the value, `cobc`
emits `cb_hash_bytes`/`cb_bytes_eq` (inline in `cbrt.h`). The front end
rejects a `K` that is not an integer, `bool`, `str` or `String` at each
call of a `HashMap`/`HashSet` function. Private helpers `Vec::replace`
and `Vec::remove_at`. Checked against a Python model over 4000 random
operations with `String` keys (coby, cobc gcc and clang agree). A first
version re-placed every entry on removal and rotated the whole entry
list: 333 removals from a 1000-entry map took 3½ minutes in `coby`;
backward-shift deletion and the swap made the same run 1.4 s. Case:
`hashmap_ok.cb` (output pinned).

### `foreach` (2026-09-25, D-0042, `CHG-0050`)

`foreach (x in c)`, `in &c`, `in &mut c`, and `(k, x in …)`, over `Vec`,
arrays, `HashSet` and `HashMap`. The parser writes it as a counted `for`
over hidden `$`-bindings and seven `$each_*` intrinsics; `src/each.rs`
expands each intrinsic for the collection's type, and the typechecker
(checking the expansion with `std`'s field visibility), `coby` (from the
hidden binding's object type) and `cobc` (`probe_type`) all use it. A
consuming loop holds the collection in a `std`-private holder (`Drain`,
`HashDrain`, `SetDrain` over `Vec::take_raw`) whose destructor destroys
the elements not yet taken. A reference written without `&` loops as
its borrow. `printf`'s `$fmt` formats a reference to a number, `bool` or
`str` as the value in all three. Case: `14-control-flow/foreach_ok.cb`.

The examples, showcases and guide then moved their element-visiting
loops to `foreach` (outputs unchanged); loops that need the index for
more than a position (sorts, grids, two vectors in step, run-length
scans) and the benchmarks stay as they were. Doing so exposed a `cobc`
bug: `probe_type` caches by expression address, and the expressions
`foreach`, `printf`'s dereference and a key's bytes synthesize were
temporaries, so a later one could reuse an address and get a stale
type (intermittently, `internal error: print of an unprintable type`);
they are now kept alive in `kept`, as the other synthesized expressions
already were.

### Digit separators (2026-09-26, D-0043, `CHG-0051`)

The shared lexer's `digit_run` reads the digits of every numeric literal
(integer, fraction, exponent, and after `0x`/`0o`/`0b`), allowing a `_`
only between two digits and dropping it; anywhere else it is a lexical
error, "a `_` in a number goes between two digits". `parse<T>` is
unchanged and still rejects `_`. Showcases use it for long constants
(and hex for the CRC and FNV constants).

### Showcase Tier 6 and two `coby` faults (2026-09-26)

Tier 6 (`showcase/tier6`: `wordfreq`, `anagrams`, `inventory`,
`build_order`) exercises `HashMap`, `HashSet`, `foreach` and the
`printf` family on files. It found two faults in `coby` (`cobc` was
right in both):

- `crosses_shared_ref`, `[Borrow-Exceeds-Source]`'s dynamic check,
  peeked at `&mut *e` by evaluating `e`, then `eval_borrow` evaluated
  it again: a call in `e` ran twice (`*HashMap::entry(&mut m, k, 0) +=
  1` moved `k` twice). An expression with a call is no longer peeked;
  `*f(…)` is judged by `f`'s declared return type, the rest by the
  static check. Case: `conf.reborrow-call-evaluated-once`.
- `exec_stmt` and a block's trailing expression ended a statement's
  temporaries only when it finished normally; one left by `return`,
  `break` or `continue` kept them, so a temporary `Option<ref<…>>`
  scrutinee kept its borrow and a later destroy of the referent faulted
  (`diag.destroy-while-aliased`). `end_temps_on_exit` ends them, keeping
  what a `return` carries out. Case:
  `conf.return-ends-statement-temporaries`.

The rough edges it met are listed in `showcase/README.md`.

### The gaps Tier 6 met (2026-09-26, D-0044, `CHG-0052`)

`std` gains `String::clone`, `String::push_ascii` (faults
`diag.not-ascii` above 127), `String::clear` and `Vec::clear`.
`Stmt::Destructure` holds every field (parser, `modres`, typecheck,
`coby`, `cobc`); the typechecker requires each field named once and
refuses a struct with its own destructor (`diag.move-out-of-field`).
`coby` destroyed the destructured container with `destroy_object`,
which ran a field's own destructor a second time; it now ends it with
`end_moved_out`, as `cobc`'s `cb_end_moved_out`. `foreach (i, k, v in
m)` gives a map entry's position (`src/each.rs` refuses three names for
anything else). The Tier 6 programs use all of it.

### `Box<T>`; recursive types rejected (2026-09-26, D-0045, `CHG-0053`)

A struct containing itself by value (`struct Node { Option<Node> next;
}`) overflowed both tools' stacks while computing its layout.
`typecheck::recursive_type` now runs first in `check_program`: a walk
over fields, payloads, array elements and `mutex` values (type
arguments substituted) that stops at `ref`, `rawptr`, `fn`, `handle` and
`guard`, rejecting a type that reaches itself with
`diag.recursive-type`. `Box<T>` in the prelude (`Rc` without the count;
a `full` flag lets `into_inner` move the value out). A `match` through a
reference on an enum with a resource payload is still
`diag.move-out-of-field`; that is the next decision.

### `match` through a reference (2026-09-26, D-0046, `CHG-0054`)

A scrutinee that is a reference (`match (&e)`, `match (&mut e)`, a
reference-typed expression) binds each payload as a reference of its
mode; before, it type-checked and then faulted in both tools. Typecheck
types the binders; `coby`'s `eval_match_by_ref` forms the payload
reference with the scrutinee's token as ancestor; `cobc`'s
`lower_match` borrows it with a `CB_PAYLOAD` projection. Making it work
for an enum inside a `Box` needed `coby`'s paths to understand a
payload step: `type_at` returned the enum's type for it, and
`path_offset` (arena memory) put the payload at offset 0 with the
enum's type; both now read the active variant and use its payload type
and the enum layout's payload offset.

### Slices, `$`, indexing a `Vec` (2026-09-26, D-0047, `CHG-0055`)

Tokens `..` and `$`, type-name `slice`. The parser allows a range
`[lo .. hi]` only as `&`'s operand (`ExprKind::SliceOf`) and `$` only
inside index brackets (`ExprKind::Dollar`). A slice is a borrow of its
source plus a start and a length: in `coby` a value `{ ref to source,
start, len }`, so the existing scan for live references keeps the
source borrowed; in `cobc` a struct `{ void *src; T *data; uint64_t
len; }` whose `src` slot holds the `cb_borrow` token (its type
descriptor lists that slot). `x[i]` on a `Vec` checks the `Vec`'s own
path in the mode of the access (`coby`: a `write_ctx` flag set for an
assignment's target and `&mut`'s operand; `cobc`: the `Vec`'s base and
path on the element place, so `cb_read`/`cb_write` decide); on a slice,
through the slice's token. `$` is a stack of lengths in both. The
typechecker treats a slice as a reference for escape and elision
(`is_borrow_ty`), rejects moving a resource out of a `Vec` or slice
element, and checks literal array bounds. `foreach` over slices via
`src/each.rs`. `split_at` is left for range-aware borrows (D-0047).

### `swap`, `replace`, `Vec::swap`, `slice_swap` (2026-09-26, D-0048, `CHG-0056`)

Four `std` functions over the std-only intrinsic `swap_places`, which
exchanges two places' contents (`coby`: read and write both places;
`cobc`: exchange the bytes, and a reference slot's token for a value
holding references). `cobc`'s borrow of a `Vec` or slice element now
carries the element's index (`CB_INDEX`), so two different elements are
disjoint, as they already were in `coby`. `f(&mut x, &mut x)` is
admitted at the call and conflicts at the first access inside `f`, as
D-0022 decided (an earlier note here called it a gap; it is not).

### Syntax errors as diagnostics (2026-09-27, `CHG-0081`, `CHG-0082`)

`int x = 10;` was a syntax error ("expected Semi, found Ident") with no
phase, which read as a failure at run time. Now a statement opening
`identifier identifier` then `=` or `;` is a declaration (`T x = a;`
declares a type parameter's value too), and an unknown type is
`diag.unbound-name` (static) at the statement (`Stmt::Let::line`, so
`int x;` is located). Every lexical or grammatical error renders as
`diag.syntax-error` (static) at its file and line, the message giving
what was expected and found, as spelled (`Display for Tok`), and the
column (`render_located`, which splits a Windows path's drive colon
correctly). The harnesses read it as any diagnostic; the conformance
table has syntax rows (§15). Found on the way: `* 3` (a dereference of
an integer) passed the checker, then failed in `coby` at run time and
crashed `cobc`'s code generator; it is `diag.type-mismatch` (static).

### Performance, first round (2026-09-27)

Realistic workloads (strings in a `Vec` and a `HashMap`, sorting,
records through references, a binary tree) profiled with `perf`
found three costs growing faster than the input:

- `cbrt`'s `ALLOCS` was a list searched and shifted at every
  `cb_deallocate`: freeing many allocations (a `Vec<String>`) was
  quadratic. Now a `BTreeMap` by address. 200,000 strings built,
  counted, sorted and summed: 218 s → 17.6 s.
- `coby`'s `release`/`deallocate` scanned every reclaimed object for
  those in the freed range; `ReclaimedAddr` keeps an index by address.
- `Vec::sort` of a key type compares elements in place with the
  std-only intrinsic `key_less_at(v, i, j)` (`Vec::sorted_order_keys`),
  `v` borrowed shared for the whole sort, instead of borrowing two
  elements per comparison: sorting those strings 11.2 s → 5.0 s
  (`cobc`). `sort_by` still borrows, since the program's comparison
  takes references.

Also: `coby` read `COBALTC_TRACE` from the environment at every
expression; it is read once (`trace_on`). What remains is linear and
spread over the runtime's bookkeeping (about 13 µs per `HashMap::entry`,
2 µs per tree step, 1 µs per checked element access in `cobc`): the
next phase is eliding it where `cobc` can prove an access cannot fail.

### A slice borrows its range (2026-09-27, D-0070, `CHG-0080`)

`split_at` without a function: `&mut v[0 .. m]` and `&mut v[m .. $]`
live together. `coby`: `Proj::Range(lo, hi)` (never navigated; the
navigation functions skip it), `Value::Ref::range` set by a slice
(`scan_value` appends it to the reference's path), `overlap` meets a
range with an index inside it or an intersecting range; a slice is
checked over its range at formation; a `Vec` element is checked at its
index. The checker: `PElem::Range(Option<(lo, hi)>)`, literal ranges
compared, others left to the dynamic check (`spec/14`). `cbrt`:
`Proj::Range`, `cb_borrow_range` (composing a slice-of-a-slice's range
into an absolute one), and `check_access` normalizing `[…, Range(l, _),
Index(r)]` to `[…, Index(l + r)]` and comparing with `ranged_overlap`
when a range is involved (the fast path otherwise). Two threads can now
sort the halves of one `Vec` at once.

### Local types (2026-09-27, D-0069, `CHG-0079`)

`struct`/`enum` as statements, with `fn T::name` for them in the same
block. The parser, as for local constants: `parse_local_type` hoists
the declaration as `T$k` and binds `::T` in the scope (types and values
are separate keys); `lookup_type` renames `T` in types, in a path's
first segment before `::`, `<` or `{` (struct literals, `T::f`,
`T::V`), in qualified patterns and in destructuring. `parse_fn` parses a
function of a local type with the enclosing function's variables out
of scope (only constants and types kept) and its type parameters
replaced. A plain `fn` in a block, or one for a non-local type, is a
syntax error.

### `const auto`, formatted `static_assert` messages, and their diagnostics (2026-09-27, D-0067, D-0068, `CHG-0077`, `CHG-0078`)

`const auto N = e;` (module or local): `FnDecl::auto_type`; `consts`
(`resolve_auto`, run first) synthesizes the type from `e` over the
closed set of constant expressions (literals, operators with D-0037's
literal rule, other constants, the pure intrinsics, struct, array and
variant literals, fields, elements, a function's name), writes it into
the declaration, and rejects an undetermined one (`None`) with
`diag.type-mismatch`. `static_assert(c, "format", a…)`: each argument a
constant expression; the checker types `$fmt(format, a…)` and records it
(`message_fmt`); `check_static_asserts` formats it
(`Interp::eval_static_message`) only when the assertion fails.
`diag.static-assert-not-constant` (D-0067) replaces
`diag.const-not-constant` and `diag.type-mismatch` for a `static_assert`
whose condition (or a message argument) is not a constant, or whose
condition is not a `bool`; the condition is checked before the argument
count.

### Local constants (2026-09-27, D-0066, `CHG-0076`)

`const τ N = e;` as a statement. The parser keeps the scopes of the
function it parses (`scopes`: a name to its hidden constant, or `None`
for a local variable; blocks, parameters, closures, `match` binders,
`for` and `foreach` names), hoists each local constant into its module
as `N$k` (drained after the item that holds it), and renames each use of
`N` in scope, a `match` binder included (so it is a literal pattern). A
local variable or type parameter in the initializer is
`diag.const-not-constant`, raised from `parser::parse`. Everything after
the parser sees a module constant. The checker's `const_expr` takes a
field of a constant expression and an element at a constant index.

### `assert` (2026-09-27, D-0065, `CHG-0075`)

`assert(c)` and `assert(c, f, a…)`: `modres` rewrites the call to
`if (!(c)) { $assert_fail($fmt(f, a…)); }` (the message's arguments are
evaluated only on failure). `$assert_fail` faults `diag.assert-failed`
with the text as the diagnostic's message (`coby`: `DETAIL_SEP`, as
`static_assert`; `cbrt`: `cb_fault_msg`; both render it through
`diagnostics::with_message`). `coby`'s line tagging now tells a line
suffix from an `@` inside a message (`diagnostics::has_location`). The
guide's `check` helper (a deliberate division by zero) is gone: its
examples use `assert`, or `static_assert` for a constant condition.

Noticed, not changed: when a fault unwinds, `coby` runs the destructors
before printing the diagnostic and `cbrt` after; standard output is the
same, only its interleaving with standard error differs.

### Concurrency testing round (2026-09-27)

Twenty realistic threaded programs (pipelines, worker pools and a pool
resource, a bank of per-account mutexes in a `Vec`, a logger thread
owning a `File`, request/reply, threads spawning threads, resources in
`Option`/`Result` across threads, boxed trees through a channel, a
shared `HashMap`, 8×8 producers and consumers, `StringView`s lent to
threads, `?` leaving with a guard held, a worker faulting while others
wait, late deadlocks), each run twice under `coby` and 5–15 times
natively under GCC and Clang, all outputs required identical. `Rc`
shared across threads gives only the two permitted outcomes (`42`, or
`diag.aliasing-conflict` when the counts' accesses meet), never a
corrupted count.

`coby` bugs found and fixed (`cobc` was right in every case):

- `spawn`'s arguments were evaluated without their parameters' types:
  `spawn(t, 1)` with `fn t(u64 seed)` passed an `i32`, and arithmetic on
  it later faulted `diag.literal-out-of-range`. Now as a call's, for a
  `fn` and a `move` closure.
- A temporary guard (`*lock(&m)`) was never registered with its
  statement, so it was never destroyed: the mutex stayed locked (a false
  `diag.mutex-reentrant-lock` on the next lock) and a `Vec` of mutexes
  could not be destroyed.
- `join` returned a struct, enum or array result as an untyped value, so
  `match (join(h))` faulted `diag.type-mismatch`; it is a temporary of
  the thread's result type now (a resource one ends with the statement
  unless taken).
- A channel wait by the main thread checked `[Channel-Deadlock]` only as
  it began: if the last other thread ended during the wait, `coby` hung
  (`cbrt` too, in principle). Both re-check whenever the wait wakes.
- After a wait or a statement-boundary yield, the current line was
  whichever line another thread had last run, so a fault in a threaded
  program could name the wrong line; each thread's line is restored.

Friction met, for the owner: a request carrying `&reply` to a server
keeps that reference alive until the server's arm ends, so a client
that destroys its reply channel right after receiving the answer races
it (`diag.destroy-while-aliased`, timing-dependent; reply channels that
outlive the server avoid it). `StringView` has no bytes accessor. A
closure cannot declare its return type. Two temporary guards on one
mutex in one statement (`printf("%v %v", *lock(&m), *lock(&m))`) are a
reentrant lock, as specified.

### Access through a guard (2026-09-27, D-0064, `CHG-0074`)

`g.f` and `g[i]` are `(*g).f` and `(*g)[i]` (`[Guard-Auto-Deref]`). The
checker types a field or index through `guard<τ>` as through a
reference (`check_field`, `Index`, `static_place_type`); `cobc` lowers a
guard-typed base through the lock path (`guard_through`, split out of
`guard_place`). `coby` already ran it; before, the checker left `g.f`
untyped and `cobc` failed with an internal error.

### Channels (2026-09-27, D-0063, `CHG-0072`, `CHG-0073`)

`Channel<T>` in `std` CobaltC: a `mutex<ChannelState<T>>` around a ring
of `Option<T>` slots, and two event counts per channel (waiting senders,
waiting receivers) behind the std-private `event_op`. A state change
adds one to the count of those it may let proceed, under the channel's
lock; a waiter reads its count under the lock and, once the lock is
released, waits for it to differ (`coby`: `block_until` over counts in
`Shared`; `cbrt`: `cb_event_op` over `wait_until`/`signal`, keyed
`id | 1 << 62`). `[Channel-Deadlock]` is detected when the main thread
waits with no spawned thread running. `cbrt`'s waiters now have a
condition variable per key, so a signal wakes only its own waiters.

Throughput (20,000 `u64`s, capacity 16, this 4-core VM): one producer
and one consumer, `cobc` 0.8 s, `coby` 13.8 s; four producers, `cobc`
3.5 s, dominated by contention on the runtime lock that every runtime
call takes. `coby` hands its GIL over at each statement while threads
are live (about 12 µs a statement here, as for a plain `mutex` loop), so
a message's forty-odd statements cost it about 0.7 ms.

Fixes the channel needed, or found on the way:

- `sync-exempt` (CHG-0072): a lock-derived path and a non-lock-derived
  shared path never clash. `coby` now decides lock-derivation by guard
  tokens and their descendants, and a guard's token records the locking
  reference as its ancestor (locking through a destructor's `self`
  clashed with it); `cbrt` compares lock origins.
- `coby` keyed its lock table by object, so two `mutex` fields of one
  struct were one lock; it is keyed by the mutex's place now.
- `coby`: an object retyped to a binding's written type
  (`Option<Noisy> x = None;`) or a call's declared return type kept the
  resource flag of its first type, so a resource stored in it later was
  never destroyed.
- The checker types `lock(e)` as `guard<τ>`; it was untyped, so
  `(*g).n + 1` took the literal's default type.

`showcase/tier3/pipeline.cb` runs a three-stage pipeline.

### Sorting and searching (2026-09-27, D-0062, `CHG-0070`)

`Vec::sort`, `sort_by`, `binary_search`, `binary_search_by` in `std`
CobaltC: a stable bottom-up merge sort of positions (`Vec::sorted_order`,
comparing through `Vec::index_shared` references), then each element
moved once into a new buffer (`Vec::apply_order`, `take_raw`). `key_less`
is a std-only intrinsic (integers by value, `bool`, text by bytes:
`cb_bytes_less` in `cobc`); the checker checks `sort`'s and
`binary_search`'s `T` as `HashMap`'s `K`. `cobc` sorts integers and
`bool`s natively (`native_vec_sort`: the checks on `v`, element objects
ended, then `cb_sort_plain`, Rust's stable slice sort): 200,000 `u64`s
7.0 s → 1.2 s. `sort_by` still makes an element reference per comparison
(about 2 µs each under `cobc`).

### Directories, the environment and the clocks (2026-09-27, D-0061, `CHG-0069`)

`make_dir`, `remove_file`, `remove_dir`, `rename`, `list_dir`,
`path_kind` (`PathKind`), `env_var`, `current_dir`, `monotonic_ns`,
`unix_seconds`: `std` source over three primitives, `fs_op` (one or two
paths), `fs_query` (an answer of any length, `file_read`'s copy-and-retry
protocol) and `clock_read`, all in `src/fileio.rs` for both tools (`coby`
in `call_extern`, `cobc` as `cb_fs_op`/`cb_fs_query`/`cb_clock_read`).
Names and values come back as the platform's bytes
(`into_encoded_bytes`, portable), checked by `String::from_utf8`.
`make_dir` answers `Ok(false)` for a directory already there rather than
adding a `FileError` variant, which would break exhaustive matches.

### The five friction points (2026-09-27, D-0058–D-0060, `CHG-0066`–`CHG-0068`)

- **A binder of the whole value, and constants, in patterns (D-0058).**
  The parser keeps a lone name (or a qualified path) as a binder
  candidate; `modres` makes it a constant's call (folded by `consts`), a
  variant, or leaves it a binder. The checker types a top-level binder as
  the scrutinee (the reference itself through `match (&e)`); `coby` takes
  the whole value (`take_whole`: moved when a resource) and needs no
  discriminant when the first arm matches everything; `cobc` binds the
  scrutinee's slot, and lowers a catch-all first arm on a scrutinee that
  is not an enum flat (`lower_match_scalar`; a `_` on a place leaves it
  unread).
- **`File::printf` (D-0059)**: `modres` rewrites it to
  `File::write_text(f, $fmt(…))`.
- **`?` consumes a temporary operand (D-0060)**: `coby`'s
  `eval_propagate`; `cobc` ends a `?` scrutinee temporary after its match.
- **Performance.** A byte benchmark (2 M elements) went from 6.6 s to
  1.1 s under `cobc`, the 1.7 MB stream stress test from 26 s to 4.6 s:
  - `foreach (x in &v)` over plain elements whose body only reads `*x`
    compiles each `*x` as the checked element read `(*$c)[$i]`
    (`Fx::foreach_alias`, `block_only_reads`): no element object and path
    per step (4.4 s → 0.17 s for 2 M elements). Any other use of `x`
    keeps the reference.
  - `Vec::clear` of plain elements is native (`native_vec_clear`): the
    checks `Vec::drop`'s native form makes, one `cb_release` over the
    range (1.07 s → 0.02 s).
  - `File::read` reads up to `max` from the file, not one 8 KiB fill.
  - A leak the 2026-09-27 `cb_send_datum` fix brought: a reference sent in
    a value (a slice argument, a returned `Option<ref>`) was detached from
    its statement and never stamped again, so it outlived every holder
    and each later check of its object walked it (quadratic).
    `cb_recv_datum` now stamps what it receives into the current
    statement, as `cb_recv_ref` does.
  - `cb_send_datum` also left the sender's slot (in a dead C temporary)
    holding what it sent, so a returned `Result<ref<…>, _>` stayed held
    until a later store happened to reuse the address — under GCC it did,
    under Clang it did not, and D-0060's case failed under Clang only. The
    slot is now given up as the value is sent.
  - The `foreach` alias expressions are kept to the function's end
    (`alias_kept`): cobc caches synthesized expressions by address, and a
    freed alias's address reused by a later expression hit a stale entry
    (`tinyos.cb` failed, nondeterministically, with "unresolved path").

### No runaway test programs (2026-09-27)

A compiled test program from an earlier session was found spinning at
100% CPU for about 12 hours: `cobc --run` had been killed (a tool
timeout) and its child, the compiled program, was left an orphan with
nothing to stop it. Now:

- `cobc --run` starts the program with `PR_SET_PDEATHSIG` (Linux): it is
  killed when `cobc` ends, however `cobc` ends. `COBALTC_CPU_LIMIT=<s>`
  gives it an `RLIMIT_CPU` (reported as such when hit).
- `cobc`'s test oracle runs `coby`, `cobc` and compiled programs with an
  `RLIMIT_CPU` and death-with-parent (`oracle::limited`,
  `CPU_LIMIT_SECS`); `coby`'s tests run `coby` through
  `common::output_limited`, which kills a process after `RUN_LIMIT_SECS`
  (portable: `Child::kill`).
- `showcase/run_all.sh`: `ulimit -t`, `COBALTC_CPU_LIMIT`, and `timeout -k`
  on every run (`LIMIT`, default 600 s); `htmlguide/generator/gen.py`: a
  CPU limit on every example it runs; `bench/run.sh`: `ulimit -t`.

Checked: killing `cobc --run` with `SIGKILL` takes its program with it;
an endless loop under `COBALTC_CPU_LIMIT=1` stops after a second; a
showcase that never ends fails `run_all.sh` in seconds with nothing left
running.

### Real-world testing of the 2026-09-26 features (2026-09-27)

Fourteen larger programs (an archiver, a streaming stress test over a
generated 1.7 MB file, a bytecode VM with parallel runs, `File` in every
container and move, `File`'s edge semantics and error paths, a JSON
reader, a literal/nested-pattern torture test, 18 static rejections, a
program shadowing 18 intrinsics with a library file, an append-only
key-value store, a `mutex<File>` shared by threads, CSV statistics, a
BMP editor, a symbolic simplifier), each run under `coby` and `cobc`
with GCC and Clang, outputs and every written file compared, and checked
against independent Python computations. Fixed, each with a conformance
case unless noted:

- `cobc`: `t[i]` on a `ref<Vec<…>>` parameter compiled confined (the
  argument a local Vec that never escapes) was an internal error
  ("unresolved path"): `vec_index_call` now takes a confined parameter's
  type from `confined_params` (`16-aggregates/vec_index_through_ref_param_ok.cb`).
- `coby`: `match (Vec::pop(&mut v))` with a struct element: `Some(x)`
  inside the body built `Option<void>` and the call returned that
  temporary as it was; a call's temporary of the declared return type's
  name now takes the concrete declared type
  (`16-aggregates/match_generic_call_struct_payload_ok.cb`).
- `cbrt`: `Reclaimed::range` scanned every ephemeral entry (a HashMap)
  on each `release`, so `Vec::clear` of a Vec whose elements had been
  indexed was quadratic (minutes for 70,000 bytes); the ephemeral map is
  ordered now. No conformance case (a timing); the sieve benchmarks are
  unchanged.
- `coby`: `e?` ended a `Result` that owns nothing at once; `[Propagate]`
  is a `match` that copies out of it, so a temporary one ends with its
  statement and a reference it holds is live until then, as `cobc` had it
  (`18-error-failure-semantics/propagate_reference_lives_to_statement_end_rejected.cb`).
- `cobc`/`cbrt`: `return Some(&m.v);` — a returned value holding a
  reference formed in the `return` statement — ended that reference with
  the statement (`cb_send_datum` did not detach its tokens as
  `cb_send_ref` does), so the caller's use was stale
  (`15-function-semantics/return_value_holding_new_reference_ok.cb`).
- Static pass: `*g` through a guard checked `g` in value position and
  rejected it as reading a resource (`i32 w = *g;`, a tail `*g`)
  (`19-concurrency/guard_deref_value_read_ok.cb`).
- Static pass: an array literal of elements it cannot type (`[join(a),
  join(b)]`) was typed `array<i32, N>`; it takes the expected element
  type (`19-concurrency/join_in_array_literal_ok.cb`).

Friction met, for the owner (all five then resolved, below: D-0058–D-0060
and the performance pass):

- A `match` arm cannot bind the whole value at the top level
  (`n : Err(Fields(n))`, `other : other`): written three times, each time
  replaced by a `let` before the match or `_` and the scrutinee.
- Named constants are not patterns: a VM's opcodes are bare literals
  with comments.
- Writing a string literal to a file takes a binding first
  (`File::write_str(&mut f, &String::from_str("…"))` borrows a
  temporary); an `fprintf`-like call is on D-0054's revisit list.
- `match (peek(p)?)` holds `peek`'s reference to the statement's end
  (above), so an arm cannot then change `p`; a peek returning a value,
  not a reference, is the way.
- `cobc`'s per-element checks cost about a microsecond each: 26 s for
  the 1.7 MB stress test (every byte pushed, hashed through `foreach`
  and validated several times); `coby` is far slower (a 98 KB scale
  was used for the three-way runs). `File::read` returns at most one
  8 KiB read-ahead fill per call.

### Literal patterns (2026-09-26, D-0057, `CHG-0065`)

`ast::Arm::lit` holds the literal expression (an `IntLit`, a negated
one, a `BoolLit`); the checker types it with the level's type expected
(so range, sign and suffix rules are the literal rules) and extends
coverage with `pattern_elems` (a literal is the element `=value`; a
`bool` level splits into `=true`/`=false`; an integer level is covered
only from above). `coby`: `lit_equals` at the end of `arm_matches`, and
a scalar path in `eval_match` for an integer or `bool` scrutinee.
`cobc`: `slot == literal` in an arm's condition, `lower_match_scalar`.
The lexer took `3 : x` as the suffixed literal `3: x`; a suffix is now
only a numeric type's name.

### Nested patterns (2026-09-26, D-0056, `CHG-0064`)

`ast::Arm` gains `nested` (the variants below the first; `chain()` is
all of them), so a pattern is a chain with its binder at the innermost
payload. The parser reads `V(V(…(x)))`; `modres` turns a binder that
names any enum's variant into a unit-variant level. The checker types
each chain (`variant_payload`; a level must be an enum, never a type
parameter), and `pattern_missing` decides both exhaustiveness (its
result, e.g. `Ok(None)`, is the diagnostic's message) and reachability
(`diag.unreachable-arm`, located at the arm). `coby`: the first arm
whose chain the value's variants begin (`arm_matches`), the binder
through k `Proj::Payload`s, moved out or borrowed as before. `cobc`:
`arm_levels` gives each level's variant index; an arm is a conjunction
of tag tests down `.u.vN` slots, its binder that slot (a `cb_borrow`
with k payload projections through a reference).

Tier 7 found a `coby` fault: `?` returned `Ok`'s payload as a bare
value, which `match` cannot read a variant from, so `match (f()?)`
faulted `diag.type-mismatch`. An enum payload is now a typed temporary
(`eval_propagate`); `18-error-failure-semantics/propagate_result_matched_ok.cb`.

### Items named as intrinsics stay in their module (2026-09-26, D-0055, `CHG-0063`)

A program's root `fn reinterpret` (or `sizeof`, `allocate`, …) broke
`std`: `std`'s code found it through `[Resolve-Unqualified]` (3), and a
root item's key was the bare name, the same string as the intrinsic's.
`modres.rs` now skips clause (3) for an intrinsic's name
(`is_intrinsic_name`) and `qualify` keys a root item of such a name
`name$`. Nested and file-backed modules reach the intrinsic too; a root
item of such a name cannot be imported, so a nested module no longer
reaches it (`conf.item-intrinsic-nested-rejected`).

### `File` (2026-09-26, D-0054, `CHG-0062`)

`std`'s `File` (a `resource struct` holding an index and an `open`
flag, written in CobaltC: `open`, `create`, `append`, `open_rw`, `read`,
`read_to_end`, `read_line`, `write`, `write_str`, `seek`, `len`,
`close`, the destructor) and `read_bytes`/`write_bytes`, over two
std-only primitives, `file_op` and `file_at`. `src/fileio.rs` holds the
table of open files for both tools (a `Mutex<Vec<Option<Entry>>>`; each
entry a `std::fs::File`, a read-ahead buffer and whether it was opened
for writing; a write or seek gives back the unread read-ahead first, so
`open_rw` writes land where reading stopped). `coby` moves bytes to and
from the arena in `call_extern`; `cobc` calls `cb_file_op`/`cb_file_at`.
`File::read` writes straight into the `Vec`'s spare capacity. `close`
calls `sync_all` on a file opened for writing (Rust's `File` cannot
report `close(2)`'s own error); the destructor does not. `std` avoids
`widen`/`narrow` (written before D-0055 below fixed why it had to).

### `StringView` (2026-09-26, D-0053, `CHG-0061`)

`std`'s `StringView` (a struct over `slice<u8, shared>`, written in
CobaltC with its functions: `String::view`, `sub`, `len`, `eq`,
`eq_str`, `find`, `starts_with`, `ends_with`, `trim*`, `split` returning
`Vec<StringView>`, `parse<T>`, `String::from_view`, and `append_view`
behind `String::append`). The static pass types `&s[lo .. hi]` on a
`String` place, `&v[lo .. hi]` on a view and `==`/`!=` with a view, and
records each in `Items::view_ops`; `coby` (`eval_view_op`) and `cobc`
(`lower_view_op`) evaluate the expression's own operands, with `$` the
length, and call `std`. A view prints through its slice's reference
(`coby`: `view_bytes`; `cobc`: a read check on the slice's source, then
its data and length). `is_borrow_ty` includes `StringView`, so elision,
escape and the flow analysis treat it as they treat a slice; and a
borrow reached through a slice or view binding now has that binding's
own origin (a slice of a parameter's slice returned from `std` was
rejected as escaping). `diag.not-char-boundary` is the one new
diagnostic.

### `static_assert` (2026-09-26, D-0052, `CHG-0060`)

`static_assert(c)` / `static_assert(c, "message")`, an intrinsic of
type unit. The static pass types it (a `bool` constant expression,
`const_expr`; a string-literal message) and records it with the body's
type arguments, once per instantiation (none while a generic body is
checked opaque). `lib.rs`'s `check_static_asserts` then computes each
with a fresh interpreter (`Interp::eval_static_assert`) before the
program runs or is compiled, for `coby` and `cobc` alike: false is
`diag.static-assert-failed` (static) with the message (carried after
`diagnostics::DETAIL_SEP` and rendered as `message:` below the
location); an overflow while computing it is that diagnostic,
statically, at the assertion. At run time the call does nothing in
either tool.

### Float conversions and limits (2026-09-26, D-0051, `CHG-0059`)

`widen<f64>(x : f32)` (exact), `to_float<τ>(x)` from a float (IEEE-754:
nearest, ties to even, an infinity beyond the range, NaN kept),
`reinterpret` between a float and an integer of its width, and
`min_value`/`max_value` of `f32` and `f64` (the largest finite value and
its negation). `coby`: Rust's `as` and `to_bits`/`from_bits`. `cobc`: C
casts between `float` and `double` (IEEE-754 under Annex F on gcc and
clang) and a union compound literal for the bits; the limits as hex
float literals. `widen<f32>` of an `f64`, `narrow` of a float, and
`reinterpret` between floats or across widths stay `diag.type-mismatch`.
`[Limits-Not-Integer]` is now `[Limits-Not-Number]`. The showcase
`float_anatomy` checks its raw-pointer read against `reinterpret<u64>`.
This closes the gap the second testing round found.

### Extensive testing, second round: fixes (2026-09-26)

Fourteen more programs (standard input under both tools, threads,
generics over slices, modules, `Rc`, big integers, destructor order,
fault paths) and 2,000 random programs (`COBC_DIFF_N=2000
COBC_DIFF_SEED=1000`: all agree). Fixed, each with a case under
`impl/conformance/`:

- **`cobc`: `v[i]` on a `Vec` was not `[Index-Vec]`'s call.** It
  borrowed the element as a projection of the `Vec` itself, so holding
  `&v[0]` made a later `Vec::push(&mut v, …)` `diag.aliasing-conflict`
  where `Vec::index_shared(&v, 0)` (and `coby`) let the push reallocate
  and the next use be `diag.stale-binding`. `v[i]` is now lowered as
  `*Vec::index_shared(&v, i)` / `*Vec::index_exclusive(&mut v, i)`
  (`vec_index_call`), with `$` the `Vec`'s length; confinement
  (`confined_vecs`) accepts `x[i]` as it accepts the call, so a local
  `Vec`'s indexing costs what the call costs (a plain local: as fast as
  before; a `Vec` in a struct field: the call's usual cost).
- **The checker: `auto r = &v[0];`** now records `r` as derived from `v`,
  as for `Vec::index_shared(&v, 0)`: a push while `r` lives is rejected
  statically.
- **`coby`: indexing a `slice<T, …>` in a generic function** read the
  element with the slice's written type (`T`), not the `Vec`'s: wrong
  values or `diag.type-mismatch`.
- **`coby`: a resource element first reached in a worker thread** kept
  that thread as its owner; the owner's `Vec::drop` then failed with
  `diag.no-destroy-authority`. Re-attaching a reclaimed resource now
  hands its authority to the reclaiming thread (`[Reclaim]`, CHG-0030).
- **`coby`: `spawn`'s arguments** were handed to the new thread as
  places, read or moved when it first ran -- after the spawning block
  could have ended them (a host panic, timing-dependent). They are taken
  at the `spawn`: a plain place read, a resource moved out (with the
  move's checks), a temporary handed over.
- **`coby`: deep recursion.** The interpreter runs on a thread with a
  4 GiB reserved stack (2 GiB per program thread; 256/64 MiB on 32-bit
  hosts): the default stack ended a program some 10,000 calls deep with
  a host crash. Name lookup stops at the call's own frames
  (`Frame::is_call`) and `bind` scans the statements' temporaries only
  for a temporary (`temp_index`): each call's cost no longer grows with
  the stack's depth (40,000 calls: 132 s before, 1.2 s after). Memory is
  still some 11 KB per active call.

- **`cobc`: a by-value `match` on a resource field, element or `*r`**
  (`match (h.spare) { None : …, _ : … }`) read the scrutinee as a value,
  a move out of a projection (an internal error). Any place of a resource
  enum type is now matched in place; only a whole binding can be moved
  from (the checker rejects moving a payload out of anything else).

Found, then closed by D-0051 (below): there was no conversion between
`f32` and `f64`.

### `coby` reads standard input (2026-09-26, D-0050, `CHG-0058`)

`std::read` in `coby` flushes standard output and reads up to `len`
bytes from `std::io::stdin().lock()` into the arena (0 at the end, −1
on an error), as `cbrt`'s `cb_read_in` does; only the Rust standard
library, so Windows builds and runs it too (checked with `cargo check
--target x86_64-pc-windows-msvc`). The GIL is released while the read
waits, so other threads run on (`tests/read_input.rs`). On a Windows
console (not a pipe), Rust decodes the console's UTF-16, so input that
is not valid Unicode is `Err(Io)` there. The `read_line_*` conformance
cases now run under both tools, fed their `stdin-hex:` bytes; `coby`'s
output is `cobc`'s oracle for them.

### Frictions resolved (2026-09-26, D-0049, `CHG-0057`)

Four relaxations from writing larger programs (owner-delegated):

- **Literal branches.** A branch of an `if`/`match` that is only a
  literal takes the type of the first branch that is not one. The
  checker records it for `coby` (`match_hints`, keyed by the `if`'s
  then-block); `cobc` probes that branch first.
- **Shared reborrow at calls.** `ref<τ, exclusive>` (`slice<τ,
  exclusive>`) is accepted for a function's `ref<τ, shared>`
  (`slice<τ, shared>`) parameter and passed as `&*e`, directly or
  through `spawn` (`coby`: `weaken_arg` at parameter binding; `cobc`:
  `weaken_arg` at the call). Not through `fn` values or closures.
  `cobc` now also ends a slice formed for a call (written
  `&e[i .. j]` in the argument, or reborrowed) when the call returns,
  as `coby` always did: `s[0] = total(&s[0 .. $])` used to conflict
  in `cobc` only.
- **Overwriting a value that owns nothing.** `live-resource-at` asks
  whether the value owns something (`owns`): a `None` of an
  `Option<Box<T>>` does not, so `*slot = Some(x)` is allowed. Static
  refutation only for types every value of which owns something
  (`always_owns`); otherwise `coby` (`value_owns`) and `cbrt`
  (`cb_owns`, with a new `owner` flag in the type descriptor) decide.
- **A match consumes only what it moves.** A by-value match on a
  resource enum consumes it only in an arm that binds a resource
  payload; `cobc` reads a binding scrutinee in place and moves out of
  it (`cb_take`) only in that arm. A temporary scrutinee no arm moves
  out of ends with its statement (after the arm, not before).

`Mutex::new(HashMap::new())` needs a declared type (`mutex<HashMap<K,
V>> m = …`), as any generic constructor does; the guide shows it.

### Integration testing of the 2026-09-26 features: fixes (2026-09-26)

Larger programs combining slices, `Box`, `HashMap`/`HashSet`,
`foreach`, match through a reference, destructuring, `swap`/`replace`,
threads, closures and `?` turned up these, all fixed without a
specification change (each has a case under `impl/conformance/`):

- **`cobc`: a match arm's temporaries outlived its C block.** A
  non-block arm body that builds a value holding a reference (a slice,
  or a struct with a reference field) inside `Some(...)` registered its
  temporaries in the enclosing statement, but their C storage was
  declared in the arm's block, which the C compiler reused for the
  match's result; ending the stale temporary at statement end released
  the result's borrow, and the next use was `diag.stale-binding`. An arm
  body now gets its own statement scope, as a block tail always had
  (`[Match]` says the arm body is statement-scoped).
- **`&f().x` for an `f` returning a reference** was rejected as
  `diag.borrow-of-temporary`. By `[Field-Access-Auto-Deref]` it is
  `&(*f()).x`, a place through that reference; the operand check now
  looks at the base's type (both tools).
- **Unknown type names** in a signature, field or payload
  (`ref<Mutex<i64>, shared>` — the type is `mutex`) were accepted
  silently, and `cobc` then failed with an internal error. They are
  `diag.unbound-name` (`[Resolve-Unbound]`).
- **`if`/`while` conditions and index expressions were not checked
  statically**: the result of checking them was discarded, so `if (5)`
  compiled under `cobc`, `while (nope)` was an internal error there, and
  a call in a condition had its argument types unchecked. They are now
  checked (`[T-If]`, `[T-While]`: `bool`; an index or slice bound:
  `usize`). `read_of_resource_via_comparison_rejected.cb` is now static.
- **A slice binding was not a borrow to the flow analysis**: moving its
  source while it lived was caught only at run time. It now records the
  same `deriv` fact as a reference binding.
- **`cobc` reported `diag.aliasing-conflict` for a move** of a binding
  held by an exclusive borrow; a move is `[Authority-Transfer]`, so it
  is `diag.move-while-aliased` (it no longer does a `[Read]` first).
- **`coby` reported a fault in a function's tail `Some(x)`** at the
  caller's line: a variant built with its expected type bypassed
  `eval`'s line tagging.

- **A variant's payload arity was not checked statically**:
  `ParseError::Invalid` (whose payload is a `usize`) written bare, or
  `Colour::Red(3)` for a payload-less `Red`, reached `coby` as a run-time
  fault and `cobc` as an internal error. Both are `diag.type-mismatch`
  (`[T-Enum]`: a bare `Vi` is `Vi(())`). A bare name that two enums
  declare with different arities is left to the usual resolution (own
  variants first, D-0032).
- **`coby`: `spawn` of a `move` closure bound to a name** ran the
  thread on the spawner's own closure object, which the spawning block's
  exit could end before the thread started (a host panic in the thread,
  timing-dependent; seen with closures spawned in a loop). The thread now
  gets its own object: a closure owning a resource moves in (its binding
  is moved from, as in `cobc`), any other is copied (the spawner may
  still call it, as in `cobc`).
- **`cobc`'s `swap(&mut a, &mut a)` faulted** (`diag.aliasing-conflict`,
  from full write checks on the two aliased parameters); `[Swap-Places]`
  has no conflict premise and leaves one place swapped with itself
  unchanged, as `coby` did. `cobc` now checks only that both references
  are still valid (`cb_valid`).

### Checks `cobc` no longer makes (2026-09-25)

Checks proven unable to fail (no specification change):

- **A loop counter's `+ 1`** (`loop_increment`): in `while (x < E)`
  (or a `for` with that condition), where `x` is an integer local with
  no run-time path (`unchecked`), the only write to `x` in the body and
  step is one `x = x + 1` (or `x += 1`) outside any nested loop, and
  nothing there declares another `x`, that `+` is emitted without an
  overflow check: `x` is unchanged since `x < E`, so `x + 1 <= max`.
- **`Vec::push`'s `len + 1`** in the native bodies: there `len < cap`
  (a full vector has just grown), and a capacity is bounded by the
  storage `allocate` gave.

`bench`: the sieve 5.13 → 4.72 billion instructions, `bench` 6.29 →
5.88 billion; `collatz` unchanged (its remaining checks can fail:
`3 * x + 1` can overflow even when `3 * x` does not). Time barely
moves: the sieve is bound by memory traffic, and the remaining gap to
Rust is two reloads per element store (a `uint8_t *` store may alias
the vector's own length and pointer in C) and the location stores
(`cb_at`), which are not checks. Cases:
`06-arithmetic/loop_increment_to_max_ok.cb` and two that must still
fault (`…_twice_…`, `…_shadowed_…`).

### Showcase modernisation: two `coby` faults (2026-10-01)

Rewriting every showcase onto the newer features found two faults in
`coby`; `cobc` was right both times.

- **A generic enum's variant as an argument took a defaulted type.**
  `None`, `Ok(5)` or `Some(None)` passed where the parameter is a type
  parameter was evaluated with no expected type, so its type argument
  was a default (`Option<i32>` for `None`), and that decided `T`. With
  `Vec` written over raw memory, `Vec::filled(3, None)` for a
  `Vec<Option<usize>>` then laid its elements out at `sizeof` of the
  wrong type, and they overlapped. Such an argument (a variant of an
  enum with type parameters, its payload free of effects) now waits,
  as an unsuffixed literal does (D-0079), until the other arguments and
  the expected type have fixed its parameter, and the final inference
  skips it (`is_context_typed_variant`, `inferred_skip` in
  `src/interp.rs`); `12-type-system/generic_variant_argument_typed_by_context_ok.cb`.
- **A slice out of `?` had no type.** `eval_propagate` made a typed
  temporary of an enum, struct or array payload but handed a slice back
  as a bare value, which indexing refused, so `need(b, at, n)?[0]`
  faulted `diag.type-mismatch`. A slice payload is now a typed
  temporary too; `16-aggregates/propagate_slice_index_ok.cb`.

A performance gap found while updating the benchmarks: `cobc` gives
`Vec::push`, `len`, indexing, `clear`, `sort` and `drop` native code
(`native_vec_index`), but compiles `Vec::filled` and `Vec::from_fn`
from their CobaltC bodies — a checked `clone(&x)`, or a closure call,
per element. Building `sieve`'s 100,000,000 flags with `Vec::filled`
took the run from about 2 s to 17 s, with `Vec::from_fn` to 55 s;
`v[i]`, `foreach` ranges, `for` and `+=` cost nothing. Native bodies for
the two (at least for plain element types) are the fix; until then
`bench/` builds its vectors with a `push` loop (`bench/README.md`).

Also: the file-handle unit tests (`fileio::handle_tests`) take turns
through a lock. The handle table is the process's and a closed slot is
reused, so a test's deliberate double close could land on another
test's file when they ran in parallel.

### `cobc` speed for the newer idioms (2026-10-01)

Each newer idiom was timed against the hand-written loop it replaces
(pairs of programs, kept outside the repository), and the gaps closed
where the checks they skipped were provably unobservable. Every change
was checked by the `cobc` and `cbrt` suites and by a differential set
of aliasing cases (held references, staleness, conflicts at calls,
comparators that print or fault) under `coby`, `cobc`+gcc and
`cobc`+clang.

The runtime (`cbrt`):

- An object's list of paths is indexed once it is long (`TokList`).
  Retiring one path was a scan of the list, which made
  `String::chars` and `String::split` quadratic.
- Stored references are kept by 4 KiB page, each page a short sorted
  list (`SlotMap`), not in one ordered tree of every slot.
- A closure called through a `fn` value: its `self` borrow is ended
  when the call returns (`cb_call_self` / `cb_call_done`), and a
  closure that captures nothing forms none (box kind 2).
- `cb_borrow_unminted` and `cb_borrow_range_unminted` make a borrow's
  checks without minting a path.

The compiler (`lower.rs`):

- **Direct reference parameters.** Some parameters need no stored path:
  - a sole reference parameter to plain data, or several shared ones;
  - a slice of plain elements;
  - a reference to a `Vec` of plain elements that is only handed on
    whole.

  Each must be used only as a projection base, as `*r`, or handed whole
  to a function with a plain result. It is never lent or stored.

  Such a parameter is bound as its bare address, and its accesses are
  unchecked. Nothing else in the function can reach that storage while
  the call holds its path.

  Closures that capture nothing get the same treatment, for `self` and
  for their parameters.
- **Pure direct parameters.** These are direct parameters that are
  handed on only to other pure ones. At the call:
  - `&p` becomes a check and a bare address.
  - A slice is neither sent nor received. This holds only when the
    function is never used as a `fn` value.
  - A slice formed for the call (`&b[lo..hi]`) is checked, not minted.
- **`foreach` over a confined `Vec`**, by `&` or `&mut`, reads and
  writes the elements in place.
- **`clone(&p)` of plain data** is a read of `p`.
- **Native bodies:**
  - `Vec::pop`;
  - `Vec::push` of reference data, received straight into the buffer;
  - `Vec::push(&mut x, e)` on a checked local whose `e` can move or end
    nothing: one check, and no statement scope;
  - `Vec::sort_by` with a closure literal that captures nothing, as a
    native merge sort. It makes the prelude's comparisons in the same
    order, since the closure may print or fault.

The numbers are in seconds, idiom against its hand-written loop:

| | before | after | loop |
|---|---|---|---|
| `Vec::sort_by` | 100 | 0.8 | 0.2 |
| `Rng::below` | 31.7 | 0.6 | ~0.1 |
| `while (Some(x) = Vec::pop(…))` | 22.9 | 1.1 | 1.2 |
| `foreach (x in &mut v)` | 21.2 | 0.5 | 0.5 |
| `Vec::from_fn` | 11.7 | 1.0 | 0.5 |
| `Vec::from_slice` | 10.8 | 0.7 | 0.2 |
| `read_le` | 8.8 | 2.3 | 0.1 |
| `Vec::filled` | 3.2 | 0.7 | 0.3 |
| `String::chars` | >300 | ~90 | 5 |
| `String::split` | >300 | 26 | 3 |

The `Rng::below` loop prints a different total from its idiom, so that
comparison is approximate.

Then the bench workloads (`bench/`), which had picked up a regression
from the first runtime changes (an object's two path lists widened it
past the size Rust copies inline, and one sorted list per page shifted
on every stack slot of a deep recursion):

- `TokList` is the size of a `Vec` again (its index boxed inside the
  enum); `SlotMap` pages are direct arrays of their 512 positions, with
  a one-page cache, emptied pages reused, and a range removed in one
  pass into a reused buffer.
- The `reclaimed` table's persistent half has a hash index beside its
  ordered map: every `Box::get` and element access looks an address up.
- Native bodies: `Vec::grow`, `Vec::reserve`, `Vec::sort` for text and
  struct or enum keys (the prelude's merge order, D-0110's comparison),
  `Box::get` / `Box::get_mut` (one `cb_elem_borrow`), and
  `String::append_string` (its per-byte checks are the first byte's).
- `Vec::push(&mut s.bytes, b)` through any checked place: one check,
  the body inline, `grow` handed a path formed for it (unstamped, ended
  at once). `str_byte` and `str_len` count as inert.
- A confined `Vec` may be moved into a struct built as the function's
  result (`String { .bytes = v }`), and passed to `Vec::reserve`.
- **Unstored reference parameters**: a function's sole reference
  parameter that it never assigns, borrows as a binding or captures
  uses the token it came with; it gets no object or slot. Its accesses
  stay checked.
- D-0129 (`Vec::reserve`, `String::reserve`): `std`'s builders reserve
  their final length.

Two experiments were reverted: memoising access checks by an epoch
(the epoch moved at nearly every statement, so it only added work),
and inline small vectors for paths' projections and ancestors (no net
gain once the path records' size is counted).

`bench/` against the pushed compiler (instructions, then seconds):
`strings` 20.9G → 6.9G, 4.70 → 2.0 s; `tree` 16.6G → 9.4G, 4.10 →
2.4 s; `hashcount` 10.7G → 6.3G, 2.40 → 1.5 s; `records` 7.3G → 1.7G,
1.30 → 0.3 s. The compute kernels are unchanged.

Still slow:
- Building a `StringView`, as `chars` and `split` do: a slice temporary
  and a struct temporary per view.
- `read_le`, which forms a slice per call.
- `key_eq` on a `HashMap`'s `String` keys still mints an element borrow
  per probe (an access to a resource element establishes a lasting
  object, so the plain-element shortcut does not apply).

Two compiler regressions from this round, found by the stress programs
once they were rewritten onto the newer idioms (both fixed, each with a
conformance case):

- A `foreach` holder recorded for the element alias under a C name from
  a probe pass: an inner loop's condition read an undeclared variable
  (nested `foreach (i, x in a)` over reference parameters,
  `14-control-flow/foreach_nested_over_params_ok.cb`).
- A pure direct parameter (passed a bare address, no path) handed on to
  a parameter of another mode: the reborrow needed the path it did not
  have (`15-function-semantics/ref_param_passed_on_shared_ok.cb`). Pure
  parameters are now handed on only in the same mode.

### `Vec::reserve` and `String::reserve` (D-0129, 2026-10-01)

`reserve(&mut v, n)` makes room for `n` more: afterwards the capacity is
at least the length plus `n`, grown once to the larger of `len + n` and
twice the capacity. Written in CobaltC in `std` for both tools; `cobc`
compiles it natively (as it does `grow`). `String::from_str`,
`String::clone` and `Vec::from_slice` reserve their length before
filling.

### `?` on an `Option` (D-0130, 2026-10-01)

Both tools had accepted `?` on an `Option` in a function returning an
`Option` (it returns `None` on `None`), which the specification did not
allow; five programs relied on it. The owner adopted it as
`[Propagate-Option]` (`spec/18`). The static pass now accepts it only
there: anywhere else it is `diag.propagate-outside-fallible-context`,
with a message naming `Option::ok_or(o, e)?`; `?` on a number, `bool` or
`str` is `diag.type-mismatch`. Not caught: an operand whose type the
static pass cannot determine (a parenthesised `if` whose branches are
`Some(…)` and `None`): `coby` then runs it, and `cobc` reports an internal error.

### No redeclaration in the same block (D-0131, 2026-10-01)

A name is declared once in its declaration region: a block's
statements, together with the function's parameters (and a closure's
captures), a `foreach`'s names, a `for` header's declarations, or the
pattern binder of an arm or an `if`/`while` pattern that govern it. A
second declaration there is `diag.duplicate-local` (static), with a
message saying how far above the first one is. A nested block may
still shadow, and a local may take an item's name.

- **Where.** The parser checks it (`declare_in_region`): blocks open a
  region, and the binder sites open one that the governed block joins
  (`join_at`). Both tools share the parser. cobfmt still formats such a
  program, since its structure is known.
- **The corpus.** Five redeclarations, in the stress programs `bst`,
  `functional` and `kvlog` (both sets) and in `const_local_ok.cb`, were
  repaired with `..`, a renamed destructuring binder, or an inner block.
  Every other program was already clean.

### The `strings` benchmark: 1.90 s to 0.73 s (2026-10-02)

`strings` (100,000 `String`s pushed, counted in a `HashMap`, sorted,
summed) was 26 times Rust's time; it is now about 10 times. What
changed, each with no change to what any program observes:

- **Native `std` bodies (`cobc`):**
  - `String::from_str` and `String::clone`: one allocation and a copy,
    the result object established and sent as the body's `return` does;
  - `HashMap::find` with `String` keys: hash and compare in place, with
    a key's element access checked only where an object already lies
    over it;
  - `Vec<String>::drop`: an element with no live object drops its bytes
    directly (`cb_live_at`); one with an object takes the body's path;
  - `Vec::push` of a resource holding no reference: the value received
    and moved into the buffer, without the body's bind and take;
  - `Vec::sort` of text: a stable merge over the elements themselves,
    comparing with an inline `memcmp` (`cb_bytes_lt`).
- **Runtime (`cbrt`):**
  - an element object established by a borrow is ephemeral for any type
    holding no reference or function (resources included): a borrow
    never hands out its root, and nothing moves out of a reference, so
    a later `[Reclaim]` makes one no program can tell from it;
  - `cb_raw_move_in` ends the moved-in object of such a type at once,
    instead of keeping it until `[Release]`;
  - the live-allocation table is a hash map.
- **Formatting (`fmt.rs`, both tools):** an integer under `%v`/`%d`/
  `%i`/`%u`, or valid UTF-8 text, with no flag, width or precision, is
  written straight into the output; a unit test checks it against the
  general path over thousands of values.

The semantics differential cases (`stress/perf/semantics`, with seven
new ones on `String` elements, moved-in values and map keys) agree
under `coby`, `cobc`+gcc and `cobc`+clang. What remains is mostly the
general cost of tracked objects: destroying the duplicate key each
`entry` call receives, frames and statement scopes per iteration, and
each `foreach` element's binding (`stress/perf/NOTES.md`).

### `records`, `tree`, `hashcount` (2026-10-02)

| Benchmark | Before | After | Rust |
|---|---|---|---|
| `records` | 0.31 s | 0.03 s | 0.006 s |
| `tree` | 2.30 s | 1.10 s | 0.026 s |
| `hashcount` | 1.50 s | 0.66 s | 0.041 s |
| `strings` | 1.90 s | 0.68 s | 0.072 s |

Each change keeps what every program observes (the semantics
differential cases in `stress/perf/semantics`, with new ones for each
change, agree under `coby`, `cobc`+gcc and `cobc`+clang):

- **`&v[i]` / `&mut v[i]` for a pure direct parameter, a plain element:**
  the vector's checked read, the bounds fault, and one `cb_elem_access`
  in the borrow's mode, in place of minting an element path and
  retiring it (`records`: one call per particle per step).
- **A `match` arm's reference binder used once, by the first thing the
  arm evaluates** (`Some(b) : insert(Box::get_mut(b), k)`), with only
  literals and locals beside it: no binding object; its uses go through
  the payload borrow's token (`unstored`).
- **`match (&mut p)` / `match (&p)` on an enum place:** the borrow's
  check and the tag's read through the place's base, no scrutinee path;
  a binder's path is borrowed from the base through the place's
  projection and the payload (the scrutinee's path would be unheld all
  its life, D-0018).
- **`p op= e` with a call in `p`** (D-0035's hidden reference), `e` a
  literal or a local: the hidden reference needs no binding object; the
  two statements are lowered as one.
- **Scope resolution:** `cb_owns` records nothing and `cb_absorb` needs
  only its statement, so a function or block whose only runtime calls
  are those needs no frame (`tree`'s `insert` has none now).
- **Runtime:** a path's projection and ancestor lists come from pools; a
  `Vec<T>` of plain `T` is destroyed directly (the type table marks it,
  `vec_plain`), without the destructor call's anonymous object and
  path; a struct's destruction no longer copies its field list.

Tried and reverted: keeping a box's referent object persistent (slower:
more tokens per object and a larger ordered index).

### `HashMap` (2026-10-02)

| Program (200,000 operations) | Before | After | Rust |
|---|---|---|---|
| `hashcount` (`String` keys, `entry`) | 0.65 s | 0.35 s | 0.041 s |
| the same with `u64` keys | 0.51 s | 0.15 s | |
| `get` of existing `String` keys | 0.86 s | 0.44 s | |
| `strings` (bench) | 0.64 s | 0.50 s | 0.072 s |

Why it was slow, and what changed (no change to what a program observes;
the semantics cases, now 54, and 1,100 random differential programs
agree under `coby`, `cobc`+gcc and `cobc`+clang):

- **Shared reference parameters handed on** (`get` → `index_of` →
  `find`) each got a binding object: `unstored_ref_params` took a sole
  reference parameter only. Any number now qualify when all are shared
  (two shared paths never clash, and none can be made exclusive).
- **Native `get`, `get_mut`, `contains`, `entry` and `insert`** for
  `String`, integer and `bool` keys (`entry`/`insert` for a plain value
  type), over one inline probe (`map_probe`), and a native `find` for
  integer and `bool` keys. `get`'s `Option<ref<V>>` is returned as
  reference data with no temporary object (`cb_elem_borrow_datum`,
  `cb_send_datum_none`).
- **A borrow of a fresh path in its own mode** (`&v[i]`, `&mut *f(…)`)
  is that path: no second path is minted.
- **A by-value match binder of reference type used once, first,** goes
  through the scrutinee's slot (no binding object).
- **Scope resolution:** `cb_fmt_arg` and string-literal arrays (`cb_sN`)
  are data, and the pure inline helpers record nothing, so a statement
  that formats no longer keeps every enclosing frame.
- **Runtime:** parsed format strings are cached per thread and rendered
  into one reused buffer; the allocation table hashes with the
  runtime's own hasher; an empty range skips release's and element
  drop's collection (`Reclaimed::any_in`).

**Deeper round (same day):** `hashcount` 0.35 → 0.09 s (Rust 0.041),
`u64` keys 0.15 → 0.02 s, `get` 0.44 → 0.39 s, `strings` 0.50 → 0.37 s.

- **No checks the native map bodies cannot fail.** After the first read
  of the map through its path, the probe and the field writes make no
  runtime check: one path held for the call; no object outlives a `std`
  call over a cell of the private `slots`; a key element can be held
  only shared, which a shared read never clashes with.
- **A fresh key passed untracked.** `entry`/`insert` with a key that is
  `String::from_str(…)` / `String::clone(…)` (so `sprintf`) and inert
  other arguments call `_raw` copies and `_rawk` bodies: the new string
  has no object (it would be established, sent, received, moved and
  destroyed or moved in, with nothing able to see it); a key not stored
  is freed directly.
- **The map's own path is not minted.** `&x` / `&mut x` of a local
  passed to a native map body (and `get`'s key) is checked as the
  borrow would be and passed with `x`'s root path.
- **`*HashMap::entry(…) op= e`** (plain value, `e` a literal or a
  local) calls the `_acc` body: the value's address after one exclusive
  element access with a write check, and the read and write through it
  need none.

**Runtime-level round (same day):** `strings` 0.37 → 0.26 s, `tree`
1.10 → 0.88 s, `hashcount` 0.09 s held.

- **`Vec::push` of a fresh `String`** (`sprintf(…)`, `from_str`, `clone`)
  passes it untracked to a `_raw` push, as the map keys are.
- **Reference locals held by their frame.** A local of reference type
  that the function never assigns, borrows as a binding or captures has
  no binding object: the frame holds its path (`cb_frame_hold`, a
  `HOLD`-marked entry in the frame's list, let go at the frame's end or
  at the binding's last use, `cb_frame_unhold`, D-0111), and its uses go
  through the path's token. Every `foreach (x in &v)` gains.
- **`Box::get(b)` / `Box::get_mut(b)` on a match binder used only there**
  is made inline, the box's pointer read through the scrutinee's base;
  no path is minted for the binder.
- **Implied checks dropped** (`elide_implied_checks`, a pass over each
  function's C): within a run of lines with no other runtime call and
  no join, a check through the same path under a projection an earlier
  check covered cannot fail (an exclusive check covers every later
  check under it, a read covers reads, and initialization is the
  object's), so it is not made.
- `cb_elem_borrow` needs only its statement scope.

**Direct recursion through `Box`es (same day; `stress/perf/tree-design.md`):**
`tree` 0.88 → 0.27 s (Rust 0.026). A struct reference parameter used only
as `r.f` (read), `r.f = e`, and `match (&r.f)` / `match (&mut r.f)` whose
binder goes only to `Box::get[_mut](b)` handed on (`box_rec_uses`) is a
*direct* parameter even though the struct holds resources: no path, no
checks through it. What stays: a write to a resource field checks the
value it overwrites (`cb_owns`, `diag.overwrite-of-live-resource`); the
`Box`'s contents handed to a direct parameter get one element access in
the borrow's mode (`cb_elem_access`, where a kept reference clashes, at
the call's location) and are passed as an address. A `match` on such a
field makes no check. Cases: `tree_direct_conflict`, `direct_overwrite_live`.
Then 0.27 → 0.21 s: a `Box` whose contents have no live object is
destroyed in place (no object established to be destroyed at once;
`box_teardown` cases: a 200,000-link chain in constant stack, destructor
order), and a statement scope takes its lists from the pool only when
something is first recorded in it.

Two frictions found on the way: `Option::is_none(o)` for an `Option`
value said only "nothing here fixes `T`" (it now names the argument
that is not a reference and says to pass `&`); and a map lookup's
`Option<ref<…>>` kept borrowing the map to its block's end (D-0133:
such a local now ends after its last use, as a reference does).

## `cobfmt`: the formatter (2026-10-01)

`impl/cobfmt/`, a third workspace member (`private/cobfmt-proposal.md`,
the owner's decisions in its §11). It lexes with `coby`'s lexer in a
mode that keeps comments and line breaks (`Lexer::tokenize_with_trivia`),
rebuilds the layout from the tokens, and writes each token exactly as
it was spelled:

- **Layout.**
  - Indentation is four spaces a block, and Allman braces are enforced
    for blocks and declarations. A struct literal's braces and a block
    used as a value after an operator stay where they are.
  - Continuation lines keep their offset from the line they continue.
  - Spacing follows rules where the tokens decide it, and is kept as
    written (none or one space) where they do not (`<`, `>`, a bound's
    `:`).
  - The `:` of a multi-line `match`'s arms, and of a multi-line
    `bitstruct`'s fields, is aligned, and groups of
    trailing comments keep their column.
  - Blank lines are normalised, and `\r\n` and a byte-order mark are
    kept.
- **Line breaks** are the author's, except a function's body, which is
  always Allman (`fn f() : i32 { 1 }` becomes four lines); `--width N` breaks over-long
  argument, parameter and array lists, one item per line.
- **Safety.** Before writing anything:
  - the result must lex to the same tokens and the same comments, and
    still parse (as `coby` parses it, with the prelude and the file's
    modules), or the file is left untouched (exit 3);
  - debug builds also check that formatting is idempotent;
  - the write is atomic: a temporary file, given the original's
    permissions, is renamed over it;
  - `--backup` writes `NAME.cb.format.bak`, then `.1`, `.2`, … .
- **Refused.** A file the language rejects as `diag.syntax-error` is
  reported and not formatted.
- **Tests.**
  - Unit tests cover each rule.
  - `tests/cli.rs` covers the command line, and requires the showcases,
    examples, benchmarks and conformance cases to be already formatted.
  - Outside the suites: a fuzz of 660 mangled variants of the corpus
    (all formatted, all idempotent), and a run of every changed probe
    program under `coby` before and after (identical).
- **The corpus.** It was formatted once when the owner chose aligned
  arms: 117 files and 31 guide examples, whitespace only.

## Conformance suite

`tests/conformance.rs` now has **106 tests, all 106 passing, 0
ignored** — the 105 from the module-visibility pass plus
`conf_cross_thread_write_conflict` (new this pass, and only now
meaningful now that `spawn` is real: it asserts one of the spec's own
two documented legitimate outcomes, `ok` or `diag.aliasing-conflict`,
across 20 repeated runs per test invocation, rather than a single
hardcoded expectation). Covers a substantial majority of
`spec/conformance.md`'s ~150 cases plus **all ten** of
`spec/examples.md`'s `ex.e2e-*` whole-program cases. Verified stable
across 10+ repeated full-suite runs (real thread scheduling makes this
worth checking explicitly, not just running once).

## Bugs found and fixed this pass

- **`clash`'s ancestor exemption was transient, not persistent**
  (`src/interp.rs`) — the bug this pass was specifically asked to fix.
  `&*v` inside a function taking `ref<Vec<i32>, exclusive> v` correctly
  excluded `v`'s own token at the *moment* it formed a new shared
  sub-borrow (token `T2`), but nothing recorded that `T2` permanently
  descends from `v`'s token — so a *later* access made through `T2`,
  one level removed (e.g. `Vec::index_shared`'s own internal `v.len`
  read, using `T2`), checked only `T2`'s own immediate exclude chain,
  never re-derived that `v`'s still-live exclusive borrow is an
  ancestor, and spuriously faulted `diag.aliasing-conflict`. Fixed with
  a new persistent `token_ancestors: HashMap<u64, HashSet<u64>>` on
  `Interp`: `record_token_ancestors` is called once, at the point a
  genuinely new `Ref`/`Guard` token is minted (`eval_borrow` and the
  by-reference closure-capture site), storing the transitive closure of
  the excludes chain active at formation time; `clash`/`solitary`
  consult it (via a new `excluded()` helper) in addition to the
  per-access exclude chain they already computed. `record_token_
  ancestors` is a no-op (nothing stored) for a token formed with no
  active excludes — e.g. `&x` on a plain local — so root borrows are
  unaffected.
  - Fixing this surfaced a **second, previously-masked bug**: with the
    spurious conflict gone, two of the three programs it had been
    blocking now ran far enough to dereference a reference into an
    already-`release`d/`deallocate`d reclaimed object — which
    `read_place`/`resolve_through_refs` handled by an internal
    `.expect("stale object")` **panic**, not the `diag.stale-binding`
    fault the spec's own `[Read-Stale]` requires. This was real and
    pre-existing, just unreachable until the first fix let execution
    get there. Fixed by making `resolve_through_refs` fallible (`Result
    <(u64, Vec<Proj>), Flow>`, its two call sites updated to `?`) and
    adding a `object_live()` check before every reference dereference
    (`eval_deref`'s `Ref`/`Guard` arm, and each loop iteration in
    `resolve_through_refs`) — an ended object's reference now faults
    `diag.stale-binding` at the point of use, as specified, instead of
    crashing the host interpreter.
  - All three previously-`#[ignore]`d tests (`ex_e2e_vec_realloc_
    stale_ref`, `ex_e2e_mutex_vec_resource_interior`,
    `ex_e2e_vec_pop_push_reuse_stale_ref`) now pass, un-ignored, with
    their originally-documented (spec-accurate) expected outcomes —
    none weakened or special-cased. Every existing aliasing/borrow
    conformance case was re-run and still correctly rejects real
    conflicts (98/98 total, 0 regressions).
- **Move-closure double-call bug** (`src/interp.rs`
  `bind_closure_captures`): a move-captured field's per-call snapshot
  was bound as *owned* by the call frame, so the *first* call's
  frame-exit wrongly ran its destructor early (e.g. dropped a captured
  `Rc`'s count to 0 and deallocated it after just one call); the
  *second* call then read already-freed memory. Found deriving
  `ex.e2e-closure-move-rc` (which calls a move closure twice — no
  prior test did). Fixed: bound as a non-owned alias instead; the
  closure's own field remains the sole owner for the closure's whole
  life.
- Several static-pass-only bugs found and fixed during development of
  `src/typecheck.rs` itself (pointer-offset arithmetic mistyped as
  ordinary same-type arithmetic; a capture-list free-variable scan
  that bounded by the captures themselves, so it flagged *every*
  correct closure as mismatched; generic type-parameter inference
  ordering that let one argument's hint poison a sibling generic call
  before it could be resolved from the *other* argument instead; a
  match arm's payload binder substituted through the wrong scope's
  type parameters (the enclosing function's, not the matched enum's
  own concrete instantiation)) — all caught by the `expect_ok`
  regression guard before being committed, not left in the tree.
- One real fixture bug in the *existing* test suite (`conf_match_
  wildcard_ok`) that the new whole-program type-checker surfaced:
  `main() : void` with a mismatched `i32`-typed tail expression, which
  the old dynamic-only evaluator never checked at all (it never
  checked a function body's type against its declared return type,
  full stop). Fixed by rewriting the arms as `void`-typed blocks.

## Bugs found and fixed in a later, separate pass (2026-09-21)

- **`encode`/`decode` (`src/interp.rs`) had no `Type::Array` arm at
  all.** `rawptr_of`/`arena_write` serializes a value into the raw byte
  arena via `encode`, and reading a rawptr does the inverse via
  `decode` — but their `match (ty, v)` only handled `Int`/`Bool`/`F32`/
  `F64`/`Rawptr`/`Ref`-ish/`Named` (struct); an array fell through to
  the generic `_ => vec![0u8; sizeof]` catch-all, silently writing (and
  reading back) all-zero bytes for *any* array value crossing a raw-
  pointer or `extern` boundary, regardless of its actual contents.
  Consequence: `CHG-0020`'s `write` — the interpreter's only real
  observable-I/O path, and every prior pass's own demonstration of it —
  never actually worked; `hello.cb` wrote six NUL bytes, not
  "hello\n". **This was not a new regression from any prior pass — it
  was present from the very first build** (verified via `git worktree`
  against `c976cfc`, the first implementation commit). No test ever
  caught it because every existing `write`-related test only asserted
  the call didn't fault (`expect_ok`), never checked the actual bytes;
  the human owner's own prior "verified" claim about `hello.cb`
  printing real text was an unrigorous visual check of terminal
  whitespace, not an actual byte inspection — corrected here.
  Fixed by adding the missing `(Type::Array(inner,_), Value::Array(_))`
  arms to both `encode` (per `[Repr-Array]`, `spec/06` §7: each
  element's `represent` placed at `i·sizeof(τ)`) and `decode`
  (its exact inverse). `sizeof`/`alignof` already handled `Type::Array`
  correctly (`spec/06` `[Sizeof-Array]`) — only the byte-level
  (de)serialization was missing, not the layout knowledge. New test
  `e2e_extern_write_produces_real_bytes_on_stdout`
  (`tests/conformance.rs`) spawns the real built binary
  (`CARGO_BIN_EXE_coby`) and asserts the literal captured stdout
  bytes, closing the coverage gap that let this ship four times over —
  every prior "verified" test run only checked that programs ran
  without faulting, never that raw-pointer/`extern` I/O produced
  *correct* content.
  - **Follow-up, checked and fixed the same day**: as suspected, the
    same `match (ty, v)` in `encode`/`decode` had no arm for
    `Value::Enum`/enum `Type::Named`, and — more fundamentally —
    `sizeof_ty`/`alignof_ty` never consulted `self.items.enums` at all
    (only `self.items.structs`), so `sizeof<Option<i32>>()` and any
    struct field of enum type silently fell back to `sizeof(ty)`'s
    generic `Type::Named => 0`. This is a deeper gap than the array
    one: not just missing byte-level (de)serialization, but missing
    *layout* for enums entirely, wherever byte layout is consulted
    (struct fields of enum type would have overlapped at offset 0).
    Fixed by adding `enum_layout` (mirroring `struct_layout`) computing
    exactly `[Layout-Enum]` (`spec/16` §1): discriminant width `DW = 4`
    (this implementation's already-documented choice, below);
    `payload-off = round_up(DW, max(alignof(τi)))`;
    `size = round_up(payload-off + max(sizeof(τi)), align)`;
    `align = max(DW, max(alignof(τi)))`. Wired into `sizeof_ty`/
    `alignof_ty` alongside `struct_layout`, and into `encode`/`decode`
    per `[Repr-Enum]` (`spec/06` §7): the `DW`-byte little-endian
    variant index at offset 0, the payload's own `encode`/`decode` at
    `payload-off`. Manually verified byte-for-byte before writing
    tests: `Some(65:i32)` → `00 00 00 00 41 00 00 00` (discriminant 0,
    payload at offset 4); `None` → `1` at offset 0, zeroed payload
    region; a struct `{ Option<i32> a; i32 b; }` → `a` at offset 0
    (size 8), `b` at offset 8 (size 4), 12 bytes total, none
    overlapping. Three new permanent regression tests assert these
    exact byte sequences via the same real-binary-plus-captured-stdout
    technique as the array fix's test, not just "didn't fault."
  - 122 tests passing (119 prior + 3 new), 0 regressions.

## Sanity pass over encode/decode's remaining `Type` combinations (2026-09-21)

After the `Type::Array`/`Type::Named`-enum fixes above, the human owner
asked for a systematic pass over every remaining `Type` variant's
`encode`/`decode` handling (and, it turned out, `sizeof_ty`/
`alignof_ty` too) rather than assuming the other 11 combinations were
fine by extrapolation. Method: enumerate every `Type` variant
(`src/ast.rs`), for each check whether it can actually be reached
through `rawptr_of`/a struct field/an array element in a real,
type-checker-accepted program, and where reachable, empirically test
it (build a real `.cb` program, run the real binary, inspect the
real bytes) rather than reason about it in the abstract. This found
two more real bugs — one of them more consequential than either of
the two fixed earlier the same day:

- **`sizeof_ty`/`alignof_ty`'s `Type::Array` (and `Type::Mutex`) arms
  delegated to the free `sizeof()`/`alignof()` functions in
  `src/value.rs`**, which recurse on the *element* type using
  themselves, not `self.sizeof_ty` — so they have no access to
  user-declared struct/enum layouts at all. Consequence:
  `sizeof<array<Foo,2>>()` for any struct/enum-typed `Foo` silently
  returned `0`, regardless of `Foo`'s real size — discovered because a
  manual nested-type test (`array<Foo,2>` where `Foo` has an
  `Option<i32>` field) produced an empty write with no error, traced
  to `sizeof` returning 0 for the length argument. This is more
  fundamental than the `encode`/`decode` gaps: it's a broken *size*
  computation, not just missing serialization of an already-correctly-
  sized value, and it would have silently corrupted the layout of
  *any* struct field whose type is `array<StructOrEnum, N>`, not only
  code that calls `sizeof` directly. Fixed by making `sizeof_ty`/
  `alignof_ty` recurse through `self.sizeof_ty`/`self.alignof_ty` for
  `Type::Array`/`Type::Mutex` instead of falling through to the
  self-blind free functions.
- **`Type::Mutex` had no `encode`/`decode` arm at all** (`mutex<T>` is
  represented at runtime as `Value::Struct(vec![inner])`, distinct
  from the `Type::Named`-keyed struct arm, so it fell to the zero-fill
  catch-all just like `Array`/enum did). Fixed per `[Repr-Mutex]`
  (`spec/06` §7): the inner value's own `encode`/`decode` at offset 0;
  the trailing state-cell bytes are left zeroed, a valid instance of
  that region's `outcome: impl-defined`/"never read by a rule"
  freedom, not a placeholder standing in for something real.

One more real (lower-severity) gap fixed while auditing: `Type::Fn`
also had no `encode` arm, so a struct field holding a named `fn` value
silently encoded as all zeros — meaning any two distinct functions
would produce identical raw byte images, violating `[Repr-Fn]`'s
"injective image" requirement, if anything ever compared `fn` values
by their raw bytes (nothing in this interpreter's ordinary execution
path does — `==`/`!=` and `spawn`'s callee resolution both operate
directly on `Value::FnVal`'s string, never via an arena round-trip).
Fixed the `encode` side only, with a deterministic hash of the
qualified name (real, non-zero, and — per `DefaultHasher`'s fixed
initial state — stable across runs of the same binary, not
per-process-randomized). **`decode` deliberately does not attempt to
reconstruct a callable `Value::FnVal` from these bytes** — the spec's
own text says a `fn`'s representation is "only consumed by ==/!= ...
and by `spawn`'s callee argument," never implying decode-then-call is
a meaningful operation, and reconstructing one would need a global
hash-to-name reverse registry this interpreter doesn't have and that
nothing in the corpus needs. Documented here rather than silently left
inconsistent.

**Checked and confirmed NOT bugs** (verified, not assumed by analogy):

- `Type::Void`/`Type::Never`: `sizeof = 0`, so the generic zero-fill
  catch-all produces (and consumes) an empty byte slice either way —
  correct by construction, not by luck.
- `Type::Ref`/`Type::Handle`/`Type::Guard`: `encode` writes an opaque
  zero token and `decode` has no arm for them (falls to `Value::Unit`)
  — deliberately not "fixed" to fully round-trip, because unlike an
  array/enum/mutex/fn value, a `ref`'s real meaning is an
  access-path-tracked binding this interpreter's aliasing machinery
  owns, not a fact recoverable from raw bytes at all; this is exactly
  why `ref` (unlike `rawptr`) is not in `FfiType` and reclaiming raw
  storage back into a real object goes through `reclaim`, never a
  direct decode of a stored reference. Treating this as a design
  boundary, not a gap, is a judgment call — recorded here explicitly
  as one, not silently assumed.
- `Type::Closure`: `sizeof = 0` in `value.rs`, unaudited by the two
  bugs above, but checked here: `spec/22`'s grammar has no surface
  spelling for a closure type at all (`type-name` only admits
  `int-type | f32 | f64 | bool | ref | rawptr | array | handle | mutex
  | guard`, plus a `Named` path) — a struct field or array element can
  therefore never be declared with closure type in any program the
  parser accepts, making `Type::Closure`'s zero `sizeof` structurally
  unreachable from `encode`/`decode`'s struct/array recursion. Not
  proven unreachable from every possible internal code path, but
  checked against the actual grammar constraint rather than assumed
  safe by pattern-matching on the other fixes.

Four new permanent regression tests
(`e2e_sizeof_array_of_struct_with_enum_field_is_correct`,
`e2e_array_of_struct_with_enum_field_encodes_correctly`,
`e2e_mutex_inner_value_encodes_at_offset_zero`,
`e2e_fn_value_encodes_as_nonzero_deterministic_bytes`), each manually
verified against a real run of the built binary before being written
down as an assertion, exactly like the two bugs found and fixed
earlier the same day. **126 tests passing (122 prior + 4 new), 0
regressions**, stable across repeated runs.

## Full silent-bug audit, area 1 continued + arithmetic (2026-09-21)

The human owner asked for "a full audit for any other silent bugs."
Continuing area 1 (encode/decode's remaining combinations were already
closed above) into arithmetic (`spec/06`) found a cluster of real bugs
in the *dynamic evaluator's* integer-literal handling — none caught by
any of the 126 previously-passing tests, all confirmed by building and
running a real `.cb` program before being touched, same
methodology as every fix above.

- **`i128` arithmetic panicked the host interpreter itself in debug
  builds.** `int_min_max`'s signed branch computed
  `(1i128 << (bw-1)) - 1` / `-(1i128 << (bw-1))` for `bw=128` — host
  `i128` overflow (the bit pattern `1i128 << 127` *is* `i128::MIN`, so
  negating it and subtracting 1 from it both overflow `i128`'s own
  range). This happened to wrap to the mathematically correct answer
  in `--release` (overflow-checks off by default) — confirmed by hand,
  not luck: `-i128::MIN` wraps to `i128::MIN` and `i128::MIN - 1` wraps
  to `i128::MAX`, both exactly right — but panicked for real in a debug
  build (`cargo build` + `target/debug/coby` on `i128 x = 100; i128
  y = x + 1;`: "attempt to subtract with overflow"). Fixed by using the
  real `i128::MIN`/`i128::MAX` constants directly for `bw >= 128`
  instead of computing them.
- **`u128`'s upper half (`2^127` to `2^128-1`) is rejected as
  out-of-range even though those are legal `u128` values — a real,
  confirmed, architectural limitation, NOT fixed.** `Value::Int`'s
  payload is `i128`, which cannot represent a `u128` value above
  `i128::MAX`. `u128 x = 170141183460469231731687303715884105728;`
  (exactly `2^127`) is rejected `diag.literal-out-of-range`. Fixing
  this for real would mean widening `Value::Int`'s payload type (to
  `u128`, or a dedicated wider representation), which touches every
  arithmetic op, comparison, and `encode`/`decode` call site — a real,
  separate project, not attempted here. `int_min_max` now returns the
  widest range the *current* payload can actually hold rather than a
  value that would itself overflow computing it, and this is locked in
  by a permanent test (`e2e_u128_upper_half_literal_known_representation_limit`)
  asserting the *current*, limited behavior, so a future fix has to
  deliberately update this test, not silently regress past it unnoticed.
- **A broad, previously-undetected gap: un-suffixed integer literals
  outside `i32`'s range were rejected in almost every context that
  should give them a type from the surrounding declaration**, because
  `eval_inner`'s own `IntLit` handling unconditionally defaulted to
  `i32` and `eval_expected` (the one function that *does* thread an
  expected type through, per its own doc comment) only special-cased
  `Call`/`StructLit`, never a bare literal. Confirmed broken in every
  one of: a `let` with a declared type (`i64 y = 5000000000;`), plain
  assignment (`y = 5000000000;`), a non-generic function call argument
  (`f(5000000000)` where `f(i64 x)`), an explicit `return` statement
  (`return 5000000000;`), and a binary-op right operand once the left
  operand's type is already known (`y == 5000000000`). Fixed by adding
  a shared `eval_int_lit_expected` helper implementing
  `rule.arith.literal`'s real priority (suffix, then a *concrete*
  expected type, then `i32`), and threading it into: `eval_expected`
  itself (fixes `let`/enum-variant/struct-field, which already routed
  through it); `eval_assign_place` and the raw-pointer-write arm of
  `eval_assign` (assignment); `call_user_fn_expected` for non-generic
  callees (call arguments — evaluated against each parameter's already-
  concrete type instead of plain `eval`, since no inference is needed);
  a new `return_ty_stack: Vec<Type>` pushed/popped around a function
  call's body execution, consulted only by an explicit `return`
  (deliberately *not* by implicit block-tail evaluation — see the new
  honest gap below, and the doc comment on `return_ty_stack` itself for
  why); and `eval_binary`, which now evaluates a bare-literal right
  operand against the left operand's already-evaluated concrete type
  (left-to-right, D-0007 — a literal on the *left* still has nothing to
  learn from yet, an honest, documented, one-directional limitation,
  not silently claimed to be symmetric).
  - **A second, independent bug in the *static* checker surfaced while
    fixing the `return` case**: `typecheck.rs`'s `ty_compat` was a bare
    `a == b`, with no `[T-Never]` case ("an expression of type never is
    accepted at any expected type") — so `fn g() : i64 { return
    5000000000; }` (a block whose only content is a bare `return`,
    correctly typed `Never` by `[T-Block]`'s own already-implemented
    clause) was rejected `diag.type-mismatch` comparing `Never` against
    `i64`, even after the dynamic-evaluator fix above. One caller
    (`check_match`, line ~933 pre-fix) had already worked around this
    ad hoc with its own `t == Type::Never ||` guard — a sign this was a
    known-but-uncentralized gap, not a one-off. Fixed by making
    `ty_compat` itself symmetric-`Never`-aware, which also correctly
    covers its five other call sites (branch merging, operand
    comparisons) that had no such workaround.
  - Nine new permanent regression tests, each confirmed against a real
    build (including one debug-build panic confirmed by hand, since
    `cargo test` doesn't build the debug profile), stable across
    repeated runs.
  - **A related "most negative literal" bug found immediately after**,
    same root cause family: `i32 x = -2147483648;` — the ordinary,
    idiomatic way to spell `i32::MIN` in every C-family language — was
    rejected `diag.literal-out-of-range`, statically *and* dynamically.
    The positive magnitude (`2147483648`, `2^31`) legitimately does not
    fit `i32`'s positive range, but the checker/evaluator were checking
    *that* magnitude's range before negating, instead of checking the
    negated value's range. Confirmed broken for `i32`/`i64`/`i128`'s
    own `MIN` (`i128`'s case needs its own care: the positive magnitude
    `2^127` doesn't fit `i128` *at all*, not even as an intermediate —
    `v as i128` for that exact value already bit-reinterprets to
    `i128::MIN`, so a plain `-(that)` would be the exact same host-
    overflow bug `int_min_max` had, computed via bit-pattern identity
    instead of arithmetic negation). Fixed in **four** places, both
    checker and evaluator (this is why it wasn't caught by the fixes
    above, which only ever touched one side at a time): `src/interp.
    rs`'s new `eval_neg_int_lit_expected` (mirroring `eval_int_lit_
    expected`, checking the negated value's range) wired into
    `eval_unary` (no expected type), `eval_expected` (let/assign/call-
    arg/return), and `eval_binary`'s right-operand case; `src/typecheck
    .rs`'s `check_unary`, which previously had **no `expected`
    parameter at all** (confirmed by reading its signature — an
    independent instance of the same "expected type doesn't reach
    here" shape, not merely a consequence of the evaluator-side bug),
    fixed by adding one and threading it from `check_expr`, plus the
    same negated-value range check.
  - **136 → 140 tests passing** (10 + 4 new), 0 regressions, stable
    across repeated runs. `cargo test --release` and `cargo build
    --release` both clean.

## Area 3: resource destruction ordering (2026-09-21)

`[Destroy-Composite]` (`spec/07`) orders same-length sibling paths by
"reverse declaration/index order" — mirroring D-0008's block-scope
rule (last-established, first-destroyed) at the composite level too.
**`destroy_composite_value`'s struct-field and array-element loops
were both forward (declaration/index order), not reverse.** Confirmed
with a real program: a resource struct's own destructor writes its
`tag` field's byte via `write`; a `Holder` with three such fields
(tags 1, 2, 3) emitted `01 02 03` (forward) before the fix, `03 02 01`
(correct) after. Same bug, same fix, for a fixed-size array of
resources. The pre-existing block-scope (D-0008) path was checked as a
negative control and was already correct (`03 02 01`) both before and
after — this is specifically a composite (struct-field/array-element)
bug, not a general destruction-order bug. Three new tests lock in both
fixes and the already-correct block-scope behavior. **143 tests
passing (140 prior + 3 new)**, 0 regressions.

## Known bugs (found, root-caused, NOT fixed — real gap, not a stub)

- (The tail-position literal entry formerly here is closed: see
  "Dynamic fidelity" below. `u128`'s upper half is likewise fixed.)
- (The `coby` panic writing through an element reference made stale
  by the assignment's own value is fixed: see "Assignment checks its
  write after the value" below.)

## Other known, deliberate scope cuts

**Concurrency is now real** (this pass). `spawn` starts a genuine
`std::thread`; every real CobaltC thread (main and every spawned one)
shares one `Interp` behind a real `Arc<Mutex<Interp>>`, so `[Thread-
Step]`'s interleaving is now actual OS scheduling, not a fixed program
order — `conf_cross_thread_write_conflict` (new this pass) runs the
spec's own racy example 20 times per test run and asserts only that
one of the two documented legitimate outcomes (`ok` or
`diag.aliasing-conflict`, `outcome: unspecified`) occurred each time,
which is now a meaningful assertion rather than something the old
synchronous simulation could not even exercise. `Mutex::new`/`lock`
use real mutual exclusion (`cobalt_locks: HashMap<mutex-obj, holding
thread>`) with real blocking, not a same-thread-only reentrant check.
Rc/Arc conversion (all shared AST nodes -- `FnDecl`/`ExternDecl`/
`StructDecl`/`EnumDecl` -- moved from `Rc` to `Arc`) made `Interp: Send`,
which is what makes sharing it across real `std::thread`s sound at all.

**How blocking works** (rewritten 2026-09-21; see "Blocking at any
depth: the GIL" below). `join`, `lock` and `[Handle-Destructor]` wait
*in place*, at whatever depth they are nested, by releasing a global
interpreter lock while they poll the thread-shared state, and every
thread yields that lock between statements while others are live. The
earlier unwind-and-retry mechanism (`Flow::WouldBlock`,
`Frame::resume_at`), which could resume only at a thread's outermost
statement list and made an unjoined handle's sweep non-blocking, is
gone.

## Mode-monotonicity fixed: a real soundness hole (2026-09-21)

The file-based conformance suite (`impl/conformance/`) found, while
building out `08-alias-validity/`, that `diag.borrow-exceeds-source`
and `diag.write-through-shared` were never constructed anywhere in
`src/interp.rs`. Concretely: a function receiving only `ref<i32,
shared> r` could do `auto rm = &mut *r; *rm = 99;` and successfully
mutate the caller's value through what should have been a read-only
reference — the shared/exclusive mode system's central guarantee,
silently unenforced. The human owner directed this (and seven other
gaps the suite found) be fixed, "don't stop until complete."

**Root cause.** `eval_borrow`'s own comment claimed mode-monotonicity
was "handled in `eval_deref`'s mode propagation via Field/Index" — but
`EvalResult::Place` carries no mode at all, and nothing anywhere
actually remembered "this place was reached through a shared
reference" by the time a `&mut` was requested on it. The comment
described an intended design that had never been implemented, not a
real check with a bug in it.

**First attempt, itself a real bug, caught before committing.** The
obvious fix — a persistent `HashMap<(u64, Vec<Proj>), Mode>` recording
the weakest mode ever seen crossing to a given place — regressed 5 of
137 existing tests immediately. Root cause: the same underlying
storage is legitimately reachable through both an exclusive and a
shared reference at *different points* in one program — e.g. borrow
`r1` exclusive, reborrow `r2 = &*r1` shared, read once through `r2`,
then keep writing through `r1`; or, exactly what `Vec::pop`/`Vec::push`
do internally, a transient shared reborrow (`Vec::index_shared`'s own
`v.len` read) followed by an unrelated later exclusive access to the
same Vec. A table keyed only by `(obj, path)` cannot distinguish "this
exact access, right now, is through a shared reference" from "some
earlier, already-ended access happened to be" — every fix attempt at
making the table "always overwrite with the current access's mode"
just moved the staleness window to a different pair of accesses,
because *some* code path always existed that reached the same address
without going through the one function updating the table.

**Actual fix**: abandoned the persistent table entirely.
`crosses_shared_ref` (`src/interp.rs`) determines mode-crossing fresh,
every time, from the borrow/write expression's own AST structure and
the *current* value of whatever it dereferences — `Deref` peeks the
inner value's mode directly (mirroring `eval_deref`'s own resource-safe
peek pattern, not `eval_to_value`, since a `guard<T>`'s interior may
legitimately be a resource); `Field`/`Index` check their base's current
value and recurse for longer chains (`r.a.b`). Nothing is cached
between calls. Wired in at the three points that matter: `eval_borrow`
(rejects `&mut` reborrow from a shared source, `diag.borrow-exceeds-
source`), `eval_assign`'s direct-deref-as-lhs case (`*r = v`,
`diag.write-through-shared`), and `eval_assign`'s ordinary auto-deref
field/index-as-lhs case (`r.field = v`, same diagnostic).

**Verified**: all three attack shapes (reborrow, direct write,
auto-deref field write) correctly rejected; all 137 pre-existing
`conformance.rs` cases plus all 70 `.cb` cases pass; the standard
library's own internal reborrowing (`Vec::index_shared`'s `v.len`
reads, `Vec::pop`/`Vec::push`'s field writes through exclusive
parameters, `lock`'s guard-deref) needed no changes — the fix is sound
against real, not just synthetic, reborrow patterns. Four new
regression tests in `impl/tests/conformance.rs` (the three attack
shapes plus a guard against the first attempt's exact false-positive
shape) and one new `.cb` case
(`08-alias-validity/borrow_exceeds_source_write_through_shared_
rejected.cb`).

## D-0006 enforced for call-argument widths (2026-09-21)

Also found while building the file-based conformance suite: an `i32`
binding passed where an `i64` parameter is declared was silently
accepted, the value round-tripping as if implicitly widened — D-0006
("no implicit conversion or subtyping anywhere") unenforced for
function-call arguments. Fixed by `check_arg_exact_types`
(`src/typecheck.rs`): every concrete (not still mentioning one of the
callee's own type parameters, after whatever substitution is known so
far) parameter type must exactly nominal-match its argument's own
checked type, `[T-Call]`'s `Γ ⊢ ei : τi`. Static only, matching
`diag.type-mismatch`'s own registered Phase — no dynamic-evaluator
re-check was added (the dynamic evaluator does not otherwise re-verify
a bound argument's value against its parameter's declared type
anywhere; adding one narrowly for this alone would be new, unguarded
mechanism this pass did not scope).

A bare integer literal is exempt: `rule.arith.literal` lets it take on
whatever concrete type context requires, and a first version of this
check did not account for the *timing* of generic inference — a
literal argument to a generic parameter (`Vec::push<T>`'s `x: T`) gets
hinted and defaults *before* `T` is known from a different argument, so
comparing its pre-inference default type (`i32`) against the
post-inference concrete type (`u8`, once `T` resolved) produced a false
positive on `Vec::push(&mut vec_of_u8, 226)` — caught immediately by
the existing 70-case `.cb` suite (`string_from_utf8_valid_and_
invalid.cb` regressed) before being committed. Four new regression
tests in `impl/tests/conformance.rs` (mismatch rejected, exact match
ok, bare-literal-still-widens ok including the `Vec::push` shape that
found the false positive, and a concrete parameter checked correctly
even inside an otherwise-unresolved generic call) and one new `.cb`
case (`12-type-system/call_argument_width_mismatch_rejected.cb`).

## diag.use-of-uninitialized implemented (2026-09-21)

Third gap from the same suite-building pass: `[Let-Uninit]` (`spec/11`,
`τ x;` with no initializer) was entirely unimplemented -- reading such
a binding hit `diag.type-mismatch` by accident (the placeholder
`Value::Unit` failing ordinary arithmetic), never the registered
`diag.use-of-uninitialized`. Fixed with a simple per-object
`uninit_bindings: HashSet<u64>` on `Interp`: `Stmt::Let`'s no-
initializer case establishes the object and marks its id; a read
(`result_to_value`'s `Place` arm, alongside the existing
`diag.read-of-resource` check it already had) faults if the id is
still marked; any write reaching the object (`eval_assign_place`)
clears the mark. Deliberately not a full definite-assignment dataflow
analysis -- a struct declared uninitialized and only partially
field-written before a read of an *untouched* field is not separately
caught, matching this pass's conservative-first-write scope elsewhere,
not a claim of full spec/11 coverage. Two new regression tests plus one
new `.cb` case.

## diag.break-outside-loop implemented; diag.return-outside-fn confirmed not a bug (2026-09-21)

Fourth/fifth items from the same list: `break` outside any `while`
fell through to a generic internal "unexpected control flow at top
level" message rather than `diag.break-outside-loop`. Fixed with a
`while_depth: u32` counter on `typecheck.rs`'s `Body`, incremented/
decremented around `ExprKind::While`'s body check and consulted at
`ExprKind::Break`/`Continue` — the same pattern `unsafe_depth` already
uses for its own ahead-of-execution `disposition: rejected` fact.
Not reset across a closure body boundary, matching `unsafe_depth`'s
own existing (not this pass's) precedent there — conservative in the
safe direction, never over-rejects.

Checked `diag.return-outside-fn` before attempting anything: `return`
outside a function body is a parse error in this grammar ("expected
item, found Return"), never reaching semantic analysis at all — there
is no reachable state for a separate check to catch. Confirmed, not
assumed, and left unimplemented on that basis, not silently skipped.

Two new regression tests plus one new `.cb` case.

## diag.propagate-outside-fallible-context implemented (2026-09-21)

Sixth item: `?` in a function whose return type doesn't match
`Result<_, E>` (same `E`) was silently accepted. Fixed in
`ExprKind::Propagate`'s own checker (`src/typecheck.rs`):
`[Propagate-Err-Mismatch]`'s exact requirement, checked once the
propagated expression's own `Result<T,E>` shape is known (never
against a still-unresolved generic type, matching this pass's
conservative-elsewhere approach). Three new regression tests plus one
new `.cb` case.

## diag.spawn-borrow-closure implemented; a real panic and a real test-shape hang found along the way (2026-09-21)

Seventh item: `spawn` accepted a borrowing (non-`move`) closure without
rejection -- `rule.conc.spawn` requires "a `fn` value or a `move`
closure." Fixed by adding `is_move: bool` to `ClosureInfo`
(`src/interp.rs`) — set at closure creation but never previously
persisted anywhere a later check could read it — and consulting it in
`spawn`'s own closure-resolution arm.

**A genuine host-crash bug, found and fixed alongside it**: `spawn`
with an argument count not matching the closure's own parameter count
indexed `argvals[i]` unconditionally, a Rust-level panic (`index out of
bounds`, exit code 101) rather than a CobaltC diagnostic. Now a bounds
check producing `diag.type-mismatch`.

**A real hang, found while writing this fix's own regression test, and
correctly diagnosed as not a new bug**: `expect_true`'s block-as-if-
condition wrapping (`if (!({ ...; join(h) == 6 }))`) nests `join` deep
inside an expression tree, which the already-documented resume-
mechanism limitation (above, "How blocking works, and its one real
limitation") cannot resume correctly — a genuine, reproducible hang
under `cargo test --test-threads=1`, but exercising a pre-existing,
already-scoped-and-accepted architectural gap, not something this pass
introduced. The interpreter was not changed for this; the test was
rewritten to the top-level-statement shape every other spawn/join test
in this corpus already uses (`if (join(h) != ..) { ... }` as a direct
statement of `main`, not wrapped through a block-valued condition).

Three new regression tests plus one new `.cb` case.

## diag.extern-non-ffi-type implemented (2026-09-21)

Eighth and last of the fixable items: an `extern fn` declared with a
resource-typed (non-`FfiType`) parameter was silently accepted. Fixed
by `is_ffi_type`/`check_program`'s new unconditional pass over every
declared extern (`src/typecheck.rs`), run once before the worklist
starts — `[Extern-Non-Ffi-Type]` is `disposition: rejected` on the
declaration itself (spec/20), so an extern that's declared but never
called must still be checked; the existing per-call-site machinery has
no call site to hang that check on for an unused one. Two new
regression tests plus one new `.cb` case.

The eighth and last item, `diag.borrow-of-temporary`, was re-checked
rather than left as recorded-but-not-fixed — and turned out to be a
false alarm, not a real gap. `spec/registry/diagnostics.md`'s own
shared entry for `diag.borrow-of-non-place`/`diag.borrow-of-temporary`
distinguishes "a value" from "a temporary"; this interpreter already
implements exactly that distinction correctly (`eval_borrow`'s
`EvalResult::Val` vs. `EvalResult::Temp` arms, `src/interp.rs`) — a
plain (non-resource) call result has no object identity and correctly
reports `diag.borrow-of-non-place`; a resource-returning call result
does have one and correctly reports `diag.borrow-of-temporary`,
verified with a real program (`Vec::new()`'s result borrowed directly).
The original finding tested only the plain-value shape and mistook its
correct diagnostic for imprecision. `impl/conformance/09-identity-
origin-extent/`'s existing case comment was corrected, and a new
sibling case added for the genuine-temporary shape.

**All eight items from the original list are now resolved**: six real
bugs fixed (one a genuine soundness hole), one confirmed structurally
unreachable given this grammar (`diag.return-outside-fn`), and one
confirmed to already be correct on closer inspection
(`diag.borrow-of-temporary`). Two further real bugs were found and
fixed incidentally while verifying these (a host-crash panic on
`spawn`'s argument-count mismatch; the transient-mode-cache false
positives during the soundness-hole fix's own first attempt).

## Per-access ancestor chains: a real soundness bug in the dynamic `clash` backstop fixed (2026-09-21)

**The bug.** `eval_deref`, `resolve_through_refs` and the `*r = e`
arm of `eval_assign` recorded the reference tokens they followed in a
persistent `place_excludes: HashMap<(obj, path), Vec<token>>`, keyed by
the *place* they reached, and every later `clash`/`solitary` check on
that place (by anyone, through any path) read those tokens back via
`excludes_for` and treated them as ancestors. Entries were never
cleared and each new entry was appended to the old one. Net effect:
**using a reference once permanently disabled it as a conflict witness
for its target.** `auto r = &x; x = 2;` was rejected, but `auto r =
&x; i32 a = *r; x = 2;` was accepted -- and a following `*r` read `2`
through a `ref<i32, shared>`. The same mechanism let the owner write
`p.b` after `*rb = 20` with `rb` still live, and let `drop(v)` find `v`
"solitary" after `Vec::len(r)` had auto-deref'd `r`. This is exactly
the pitfall the `crosses_shared_ref` comment already describes
("a persistent table keyed only by (obj, path) cannot distinguish
*this* access from some earlier one") -- the same design had survived
in the ancestor bookkeeping. Found while writing the human-readable
guide: the interpreter accepted a program the spec's own
`conf.reborrow-ok` derivation explicitly calls `[Read-Conflict]`.

**The fix.** `EvalResult::Place` now carries its own ancestor chain as
a third field, `Place(obj, path, Vec<token>)`: a bare binding carries
`[]`, `*r` carries `[r's token]`, `r.f`/`r[i]` carry every token
`resolve_through_refs` followed (plus whatever the base already
carried), and a reference *value* used as a projection base carries
its own token. Every consumer -- `result_to_value`, `into_result_form`,
`eval_borrow`, `eval_assign_place`, `eval_match`, `drop`, transfer
and move-out -- takes the chain from the `Place` it is acting on.
`place_excludes` and `excludes_for` are gone. The persistent
`token_ancestors` set (recorded once at *formation* of a new token) is
unchanged and still supplies the transitive part at `clash` time, so
`ancestor_exemption_reborrow_through_parameter_ok` keeps passing.
`[Store-Binding-Place-Copy]` (`store_binding`'s plain-place arm) now
copies through `result_to_value` -- i.e. through `[Read]` with its
clash and definite-assignment checks -- instead of a raw `read_place`
peek, which is what makes `auto r = &mut x; *r = 5; i32 y = x;` reject.

**Two existing file-based cases were wrong, not the fix.**
`08-alias-validity/disjoint_field_borrows_coexist_ok.cb` and
`09-identity-origin-extent/deref_place_read_and_write.cb` both read
through the owner while an exclusive borrow was still live in the same
block (`*ra = 10; ... if (p.a != 10)`; `*r = *r + 1; if (x != 6)`).
spec/conformance.md's `conf.disjoint-field-borrows-ok` reads `*r1 +
*r2` through the references, and `conf.reborrow-ok`'s derivation says
in so many words that "reading `x` itself here would be
`[Read-Conflict]`". Both files now check through the references inside
the block and through the owner only after it ends; their facility
lines cite the spec cases.

**Regression cases** (all run against the real binary; every negative
one previously ran to completion): `08-alias-validity/
shared_ref_used_then_owner_write_rejected.cb`,
`exclusive_field_borrow_used_then_owner_write_rejected.cb`,
`shared_ref_used_then_destroy_rejected.cb`,
`owner_read_while_exclusive_child_used_rejected.cb`, and the positive
`ref_used_then_scope_end_frees_owner_ok.cb`; plus nine tests at the end
of `tests/conformance.rs` (the same shapes, the two-shared-borrows
variant, a move variant, the shared-child-used-then-owner-read positive
case from `conf.owner-read-while-shared-ok`, and a guard that the
parameter-reborrow ancestor exemption is unaffected).

## Borrow-capture closures: captured names resolve through the capture reference (2026-09-21)

**The bug** (the reverse direction: valid code rejected). A closure
that writes a captured variable captures it by exclusive reference
(`[Closure-Form-Borrow]`), and inside the body that name means
`*self.f_i` (`[Closure-Call]`) -- an access *through* the capture
reference. `bind_closure_captures` instead bound the name straight to
the referent object (`bind_alias`), so the body's write to `total`
found the closure's own capture reference as a live, non-ancestor
exclusive path to the same cells and faulted `diag.aliasing-conflict`.
Every write through an exclusive capture was rejected; only read-only
captures (shared vs. shared, always permitted) happened to work.

**The fix.** For a borrow-capturing closure, each captured name is
bound (per call) to a fresh *capture cell*: an object holding a copy
of the capture reference, registered in `capture_cells`. `eval_path`
resolves such a name through the reference -- `Place(target, path,
[capture token])` -- so the access carries the capture token as its
ancestor, exactly as `*r` does for any reference; the cell is removed
when the call returns (`release_capture_cells`), so its copy of the
token cannot outlive the call and keep conflicting with the owner.
`crosses_shared_ref` treats a bare captured name as crossing a shared
reference when the capture is shared. A nested closure capturing a name
that is itself a capture reborrows through the cell (exclusive from
exclusive only; `diag.borrow-exceeds-source` otherwise). Move closures
are unchanged, except that a move-captured variable of *reference type*
is now bound as an owned view like every other moved value rather than
aliased to its referent. `closure_body_writes` now descends into nested
closure literals that capture the name, since the mode is derived from
the whole body text (the `x_i := *self.f_i` substitution reaches into
the nested literal too).

**Regression cases:** `15-function-semantics/
closure_exclusive_capture_write_ok.cb`,
`closure_exclusive_capture_blocks_owner_rejected.cb`,
`closure_shared_capture_blocks_owner_write_rejected.cb`,
`closure_nested_capture_reborrows_ok.cb`,
`closure_move_captured_reference_ok.cb`; and eight tests at the end of
`tests/conformance.rs`, including one that the capture cell is released
after the call.

## rule.control.flow-analysis implemented (2026-09-21)

`src/typecheck.rs` now implements spec/14 §6 as written, replacing the
flat single-block `deriv` check:

- **Facts and lattice.** Per function body, for every binding declared
  in it (parameters included): `valid(x)`, `init(x)`, and
  `deriv(r, x.π, m)` (keyed per reference binding `r`; an absent key is
  `F`), each in `{T, F, ?}` (`Tri`). `π` is a syntactic projection path
  (`PElem`: field names and literal-or-not indices) with the spec's
  overlap relation (`paths_overlap`). `escaped(x)` is deliberately not
  tracked: it only ever separates "proven" from "unknown", and this
  interpreter performs every dynamic check regardless (nothing is
  elided on a proof), so the distinction has no observable effect here.
- **Transfer functions** are applied in evaluation order as the type
  walker visits each construct: declarations, whole-object writes,
  visible borrows and D-0011-elided calls into a reference-typed
  binding (`deriv_facts_of`/`set_deriv`), every consuming use of a
  resource binding (`check_store_operand`: arguments, initializers,
  field/element/payload initializers, `return` and block results,
  `match` scrutinees, destructuring sources, assignment values, `move`
  captures), `drop`, and block exit (`exit_scope`: the block's bindings
  end; a shadowed same-named binding's facts are saved at declaration
  and restored, since facts belong to bindings, not names).
- **Control flow.** `if` and `match` run each branch/arm from the state
  after the condition/scrutinee and join pointwise (`Flow::join`; an
  unreachable state -- after `return`/`break`/`continue` -- is the
  identity). `while` (`check_while`) iterates the loop head to the
  least fixed point (entry ⊔ body-end ⊔ every `continue`), with
  refutations suppressed (`report = false`) while iterating, then walks
  the body once more from the fixed head with refutations on; the exit
  is the test's state ⊔ every `break`. This is equivalent to the spec's
  CFG dataflow because the language's control flow is structured.
- **Discharge.** At each reliance point the table's *refuted* row
  fires the rule's own diagnostic as a static rejection: name lookup
  (`valid = F` → `diag.stale-binding`), reads and projection writes
  (`init ≠ T` → `diag.use-of-uninitialized`; definite assignment
  rejects on unknown, as spec/11 §3 requires), reads/writes/borrows
  against overlapping `deriv` facts of incompatible mode
  (`diag.aliasing-conflict`), moves and destroys against any `deriv`
  fact (`diag.move-while-aliased`, `diag.destroy-while-aliased`),
  whole-object writes to a resource binding with `valid = T ∧ init = T`
  (`diag.overwrite-of-live-resource`), a resource place read in value
  position (`diag.read-of-resource`), a resource field in a store
  position (`diag.move-out-of-field`), and a literal index against a
  fixed array length (`diag.index-out-of-bounds`). An access rooted at
  `*r`, at an auto-dereferenced reference, or at a temporary has no
  place (`place_of` → `None`) and is never refuted, per the table's
  "never T / never F" row. *Proven* and *unknown* are not distinguished:
  both leave the existing run-time check in place.
- **Positions.** `check_expr_inner` takes a one-shot `place_pos` flag
  from its parent so the operand of `&`, the target of `=`, the base of
  `.f`/`[i]`, a `match` scrutinee and `drop`'s operand are not treated
  as reads; everything else that denotes a place is read
  (`[LValue-To-RValue]`).

**Effect on the suites.** Every program the inline suite marks `ok`
is still accepted by the static pass (the `expect_ok` guard), and the
prelude bodies (`Vec`, `String`, `Rc`) pass unchanged. Eleven file-
based cases moved from `(dynamic)` to `(static)` -- each one is a case
spec/conformance.md itself documents as static (transfer-invalidates-
source, double-destroy, reassign-after-drop, destroy/move-while-
borrowed, overwrite-live-resource, owner-write-while-shared,
owner-read-while-exclusive, projection-write-while-borrowed,
definite-assignment) -- and their headers now say so. Eight new
`14-control-flow/flow_*.cb` cases and seven inline tests pin the
phase in both directions, including the `unknown → dynamic` cases (a
move in one branch of an `if`; a move inside a loop; an access rooted
at a dereference) that the analysis must *not* refute.

**Not part of this** (still dynamic-only or absent, see "What it
still does not cover" above): `rule.temporal.ref-escape` and the
elision-ambiguity rule, `diag.cannot-infer-type-parameter`, and the
syntactic well-formedness rules listed there.

## rule.temporal.ref-escape and [Call-Multi-Ref-Return-Rejected] implemented (2026-09-21)

Neither `diag.reference-escapes-scope` nor `diag.lifetime-elision-
ambiguous` was emitted anywhere before this pass; spec/10's static
layer now exists in `src/typecheck.rs`:

- **Lexical facts.** Every block the walker enters gets an identity
  (`enter_scope`), with `block_parent` recording enclosure; each
  tracked binding records its `decl-block`; a reference-typed binding
  initialized by a visible borrow (or a D-0011-elided call on one)
  records `ref_origin`, the `(referent block, root binding)` that borrow
  named. `referent_of` computes `referent-block` per spec/10 §2: a
  place rooted at a non-reference binding names its decl-block; one
  rooted at `*r` or a projection through a reference-typed `r` names
  whatever `r`'s initializing borrow named, else `unknown`. Every
  borrow and every reference-returning call has its referent recorded
  *as it is walked* (`borrow_referents`, keyed by expression address),
  since a block's result is stored by the enclosing statement only
  after that block -- and its bindings -- have already ended.
- **Escapes.** A store by declaration or assignment into a binding
  whose decl-block strictly encloses the referent's block is rejected;
  the reference-valued parts of an aggregate literal (struct fields,
  array elements, enum payloads) and the results of a block/`if`/
  `match` escape with the value (`collect_stored_borrows`). The
  function result (`return e` and the body's trailing expression)
  escapes to the caller and is rejected unless the referent's root
  binding is a reference-typed parameter -- the `[Call-Elided-
  Lifetime]` exemption -- or its referent is unknown.
- **Declaration rule.** `check_program` rejects any declared function
  whose return type is a reference and which has zero or two-or-more
  reference-typed parameters, whether or not it is called -- checked
  *after* the bodies so that a body's own escape (`conf.reference-
  escape-rejected` names `diag.reference-escapes-scope` for the
  zero-parameter shape) is what gets reported.

Two file-based cases that documented these gaps now state the rules:
`10-temporal-validity/lifetime_elision_two_params_rejected.cb`
(renamed from `..._not_statically_rejected_but_dynamically_caught.cb`)
and `reference_escapes_scope_rejected.cb`; four more cases and four
inline tests added.

## The remaining static rules, and fn items as values (2026-09-21)

`src/typecheck.rs` now decides every `Phase: static` diagnostic in
`spec/registry/diagnostics.md` except `diag.return-outside-fn` (a parse
error in this grammar) and `diag.unbound-name`/`ambiguous-name`/
`name-not-visible` (the resolver's, `src/modres.rs`):

- **`diag.cannot-infer-type-parameter`** (`[Generic-Call-Uninferable]`):
  after the explicit list, the argument shapes and the expected type,
  a still-unfixed type parameter is a rejection -- but only when every
  argument's type is known to this pass and the call is not itself an
  argument whose parameter type still mentions the enclosing callee's
  unresolved parameters (`infer_poisoned`: `Vec::push(&mut v,
  Vec::new())`'s inner call has no expected type *yet*). An argument
  of a shape this pass does not type (a closure capture, a `guard`, a
  `join` result) leaves the call undecided rather than rejected.
- **`diag.unbounded-type-parameter`** (`rule.type.kind`): every generic
  body is checked once with each `T` mapped to an opaque marker `$T`
  (`marker_name`/`is_marker`; `has_unresolved_marker` now means what
  its name always said), in addition to its concrete instantiations;
  arithmetic, comparison, logic, unary operators, field access,
  indexing, dereference or a call on a value of marker type is
  rejected. A marker is otherwise "unknown" to every other check, and
  a valid type argument for unification.
- **`diag.borrow-of-non-place` / `diag.borrow-of-temporary`**
  (`[Ref-Form-Not-Place]`/`[Ref-Form-Temporary]`): the operand of `&`
  is classified syntactically per spec/13 §1 (`borrow_operand_kind`).
  A bare call result is *not a place*, whatever its type: `&make()` is
  `diag.borrow-of-non-place` for a resource-returning `make` too --
  which corrects an earlier reading recorded in `conformance/README.md`
  and `09-identity-origin-extent/borrow_of_resource_temporary_
  rejected.cb`; `diag.borrow-of-temporary` is a projection of a call
  result or aggregate literal (new `borrow_of_temporary_projection_
  rejected.cb`, `conf.borrow-of-temporary-rejected`).
- **`diag.borrow-exceeds-source` / `diag.write-through-shared`**
  (`[Borrow-Exceeds-Source]`/`[Write-Not-Exclusive]`): `crosses_shared`
  decides from static types (`static_place_type`, side-effect free)
  whether a place expression crosses a `ref<_, shared>` -- an explicit
  `*r`, or a field/index auto-deref, at any hop.
- **`diag.bad-destructor-signature` / `diag.direct-destructor-call`**
  (spec/07 §1): declaration-level check of every `fn T::drop`; a call
  to `T::drop` by name is rejected (`drop(x)` is the intrinsic).
- **`diag.spawn-borrow-closure`** (`rule.conc.spawn`): a non-`move`
  closure literal, or a local initialized from one (`closure_moves`),
  as `spawn`'s callee.
- **fn items are values of their `fn(...) : τ` type** (`[T-Item]`):
  `check_path` types a non-generic function or extern item as
  `Type::Fn`; the evaluator (`invoke_callable`, `eval_call`'s
  fallback) calls a binding or field holding a `FnVal` by name instead
  of treating every non-`Val` callee as a closure object. `apply(double,
  4)` and `fn(i32) : i32 g = double; g(5)` both work now.
- **A bare literal on the left of a binary operator** takes the right
  operand's type (`rule.type.expected`, "the other operand's determined
  type"): both passes check/evaluate the right operand first in that
  case -- a literal has no effects, so D-0007 is not observable. The
  inline test that pinned the old limitation now asserts the fix.

Four file-based cases moved to `(static)`; seven new cases and seven
inline tests added.

## Grammar: the disambiguations spec/22 states, as stated (2026-09-21)

- **`identifier '<'`** (`src/parser.rs`, `try_type_args`): the token
  sequence `identifier '<' type (',' type)* '>'` is a generic-argument
  list regardless of what follows, and anything else after `<` is a
  comparison (spec/22 §2 disambiguation (1)) -- decided by
  speculatively parsing a type list and backtracking (position and any
  `>>` split, via `undo_log`) if it fails. The previous heuristic
  scanned forward for any `>` before `;`, so `b < 48 || b > 57`,
  `(a < b) || (b > a)` and `a < b && b > a` were all parse errors.
- **Qualified type names** in type position (`geom::Point p`,
  `ref<geom::Point, shared>`): `parse_type` accepts a path; statement
  disambiguation (4) recognises a path ending in a declared type name
  followed by an identifier as a declaration (`qualified_type_ahead`);
  `modres::resolve_type` resolves the path with `resolve_qualified`.
- **Struct literals of module-declared types**: the resolver already
  rewrote the literal's path to the item key, but the evaluator and the
  static pass looked the struct up by the path's *last* segment; both
  now use the whole key, so `geom::Point { … }` from outside and
  `Point { … }` inside the module (or after `import geom::Point`) all
  construct.
- **`Vec<i32>::new()`**: a path may continue with `::` after a
  generic-argument list.
- **Named functions as values** and **a literal left of a typed
  operand**: see "The remaining static rules" above.

Three new file-based cases and two inline tests.

## Dynamic fidelity: raw pointers to locals, u128, tail typing, destroy authority, drop through a reference (2026-09-21)

- **`rawptr_of(&x)` addresses `x`'s own cells** (`[Rawptr-Of]`:
  `min(target(a))`). A fresh-storage object had no address, so
  `rawptr_of` wrote a *snapshot* to a fresh arena address; a raw write
  through `rawptr_of(&mut x)` was invisible to `x`. Now the first
  `rawptr_of` of a fresh object moves its whole value into the arena
  and records it as arena-backed (`reclaimed_addr`/`reclaimed_by_addr`,
  the same mechanism reclaimed objects use), so every later
  `read_place`/`write_place` and every raw access see one storage;
  `destroy_object` releases the mapping at `[Object-End]`. Only types
  the arena encodes faithfully (`arena_encodable`) are backed this
  way; a value holding references or handles keeps the snapshot.
- **`u128`'s upper half** is representable: `Value::Int(U128, _)`'s
  payload is the value's bit pattern (`value.rs` `is_u128`), with
  checked/wrapping/saturating arithmetic, division, comparison
  (`int_lt`), conversions (`narrow` in both directions, `to_float`,
  `to_int`) and literal range checks done in `u128`. `i128`'s own
  host-overflow cases now use checked host arithmetic too.
- **Tail-position expected types** (`rule.type.expected`): the
  expected type of a block reaches its trailing expression -- a
  function body's result (its return type), each `if` branch, each
  `match` arm, a block used as a value, parentheses
  (`exec_block_stmts`/`exec_block_body_expected`, `eval_if`,
  `eval_match`, `eval_expected`). `fn g() : i64 { 5000000000 }` no
  longer defaults the literal to i32; the "Known bugs" entry for it
  is closed.
- **Per-thread destroy authority** (`state.authority`):
  `Object.owner_thread` is granted at establishment to the current
  thread; `destroy_object` faults `diag.no-destroy-authority` from any
  other thread; a transfer from another thread's object is
  `diag.transfer-without-authority` -- except the one legitimate
  cross-thread case, binding a spawned thread's parameters
  (`binding_thread_params`), which re-keys; `join`'s claimed result is
  established in the joiner. Neither diagnostic is reachable from a
  well-formed program (every cross-thread route re-keys), which is the
  spec's intent; the tracking makes that a checked fact.
- **`drop(*r)`** is `[Destroy-Not-Solitary]`: a place reached through
  a reference is never solitary while the object's owner binding
  exists.

Seven file-based cases and five inline tests (one replacing the test
that pinned the u128 limitation).

## Blocking at any depth: the GIL (2026-09-21)

The `Arc<Mutex<Interp>>` held for a whole `run_body` attempt is
replaced by a global interpreter lock (`interp::Gil`: the `Interp` in
an `UnsafeCell`, a `Mutex<bool>` + `Condvar` token). Exactly one OS
thread evaluates at a time; the thread-shared parts of Σ that another
thread reads or writes while this one waits -- `thread_status`
(`state.threads`), `cobalt_locks` (`state.sync`), `program_fault` --
live in their own `Arc<Mutex<Shared>>` outside the GIL'd interpreter.

- **Waiting** (`Interp::block_until` → `gil_wait`): `[Join]`, `[Lock]`
  and `[Handle-Destructor]` release the GIL, poll `Shared` until their
  condition holds (or the program has terminated), and reacquire.
  `gil_wait` touches nothing but the GIL and `Shared`; `block_until`
  is `#[inline(never)]` and passes `&mut self` through
  `std::hint::black_box` on the way back so no interpreter state is
  assumed unchanged across the wait. This is the classic GIL
  discipline: the `&mut Interp` a waiting thread keeps on its stack is
  not used until it holds the GIL again.
- **Interleaving** (`yield_step`): while more than one CobaltC thread
  is live (`active_threads`), the evaluator yields the GIL after every
  statement, so `[Thread-Step]`'s "any interleaving" is real at
  statement granularity, not "whichever thread grabs the lock runs to
  completion". `conf_cross_thread_write_conflict` still admits both of
  its spec-permitted outcomes.
- **Termination** (`Flow::Terminated`): a waiting thread that observes
  `program_fault` unwinds its Rust stack without running any
  destructor (`pop_frame` is a no-op once `Interp::terminated` is
  set) -- spec/18's "other threads take no further steps; their frames
  are not unwound" -- and `run_main` reports the fault. A worker that
  faults records it and stops; `run_main` also reports a worker's fault
  that landed after main's last wait.
- **Handle sweeps** really wait now and discard (destroy) a resource
  result; a thread can no longer outlive the frame that owns its
  handle. **`return_ty_stack`** became per thread
  (`return_ty_by_thread`), since calls of different threads now
  interleave.
- Removed: `Flow::WouldBlock`, `BodyOutcome`, `RunOutcome`,
  `Frame::resume_at`, `Interp::self_shared`; `lib::run_source_raw`
  acquires the GIL, runs `main` once, and releases it.

Three file-based cases and three inline tests: a `join` nested in a
helper call and inside an expression, a handle sweep that must wait
for a worker mid-loop, and two threads interleaving fifty lock/unlock
cycles each. `spec/IMPLEMENTATION-NOTES.md`'s "no interleaving"
caveat in `conformance/README.md` no longer applies.

## Concurrency lock-bypass audit (2026-09-21)

A prior pass reported "no concurrency lock-bypass found (code review,
not test programs)" and flagged that as its weakest-verified claim.
The human owner asked for a more rigorous, empirical recheck. Two
independent things were actually checked, and they have different
answers:

**1. Is there a real data race in this interpreter's own Rust code**
(a memory-safety bug: unsynchronized concurrent access to the same
memory) — checked with `cargo +nightly build/test -Zbuild-std --target
x86_64-unknown-linux-gnu -Z sanitizer=thread` (installed for this
audit; not otherwise a project dependency), the actual industry-
standard tool for this, not code review. **Clean: zero race reports**
across the full 131-case conformance suite, plus purpose-built stress
programs beyond anything in the existing suite (10-way and 30-way real
contention on one `mutex<i32>`, verified to produce the exact correct
final count every run; 5 concurrent threads with one hitting a real
checked fault mid-run, verified to terminate correctly every run) —
all run repeatedly under ThreadSanitizer with no warnings. Grep
confirms why this is structurally likely to hold, not just lucky: the
only Rust `unsafe { }` blocks in the crate are the two `gil.interp()`
accesses to the interpreter behind the GIL (the `write` extern itself
is a safe `write_all` on a locally-owned, already-copied `Vec<u8>`,
never shared arena memory directly); there is no `static`,
`thread_local!`, second `Mutex`/`RwLock`, or atomic anywhere — every
piece of state reachable from more than one thread lives inside the
one `Interp` behind the one `Arc<Mutex<Interp>>`, and safe Rust's own
compiler-enforced aliasing rules do the rest. This claim is now
genuinely empirically verified, not merely code-reviewed.

**2. Is there real, fine-grained interleaving between threads at all**
— checked by trying to build a concrete duplicate-side-effect repro
predicted by the already-documented `exec_block_body` resumability gap
(a nested function call containing a side effect before a blocking
call, expected to redo that side effect on a blocking retry). The
repro did **not** reproduce, and tracing why surfaced a more
fundamental, previously-unstated characteristic: `shared.lock().unwrap()`
(`spec/19`'s spawn-thread retry loop) is held for the *entire*
duration of one `run_body` call, and nothing inside ordinary
(non-blocking) CobaltC execution ever releases it early — only
returning `WouldBlock` does. Consequence: a spawned thread doing
CPU-bound work with no `join`/`lock` calls of its own runs to
*complete* atomic exclusion of every other thread (including `main`)
for its entire duration; other threads cannot even begin executing
their own first statement until it finishes, let alone interleave
mid-statement with it. A repro built expecting real contention on a
mutex held by a busy-looping thread found *no* contention at all: by
the time the contending thread's own `run_body` call could even
acquire the giant lock, the busy-looping thread had already finished
and released the CobaltC-level mutex, because it had held the *Rust*
lock continuously the whole time and nothing preempted it.

This is **not a soundness bug** — `[Thread-Step]` (`spec/19`) leaves
interleaving granularity `outcome: unspecified`, and "one thread runs
to completion, then another" is a legitimate instance of that freedom,
not a violation of it; `conf_cross_thread_write_conflict`'s own "either
outcome is fine" framing already accommodates full serialization as
one legitimate case. But it is a real, previously-undocumented
*architectural characteristic* worth knowing before writing a
concurrency test against this interpreter: **two spawned threads
neither of which calls `join`/`lock` will never actually interleave —
one completes before the other is even scheduled a first instruction**,
because the coarse `Arc<Mutex<Interp>>` is the only yield point and it
is only ever released between statements at a `WouldBlock` boundary,
never mid-computation. Genuine interleaving is only observable across
`join`/`lock` call boundaries specifically (which is exactly what every
existing concurrency test in this corpus already exercises — none of
them depend on interleaving *within* a non-blocking stretch of code,
which is why none of them were affected by this). Fixing this to allow
real preemption mid-computation would need a cooperative-yield point
inserted periodically into the statement-execution loop even for
non-blocking code — a real, separate piece of architecture work, not
attempted here, and not clearly justified unless a future need
(e.g. wanting to actually exercise `conf.cross-thread-write-conflict`'s
non-serialized outcome) demonstrates it.

**A checked fault in any thread terminates the whole program**
(spec/18 `[Fault-Unwind]`), implemented via a shared `program_fault:
Option<String>` field (not `std::process::exit` -- this interpreter
may be one of many running in the same host process, e.g. the
conformance test binary, and an early design that called
`std::process::exit` from a faulting spawned thread's closure did
exactly that during development, killing the whole test binary
mid-suite). Every thread's own dispatch loop checks it and stops
taking further steps as soon as it next has the chance to, without
unwinding -- matching "other threads take no further steps; their
frames are not unwound." `ex_e2e_thread_fault_abandons_guard` verifies
this.

**Struct field layout and enum discriminant width are implementation
choices, documented here** (`spec/IMPLEMENTATION-NOTES.md` §1's own
required disclosure for a real implementation):

- `AddrWidth = 64`, little-endian (`x86_64`).
- Struct fields: natural alignment, declaration order, no reordering —
  matches System V AMD64 conventions closely enough for the FFI shapes
  this corpus uses, though no attempt was made to byte-for-byte match
  a real C compiler's struct-passing ABI beyond layout (calling
  convention itself is moot here since `extern fn` bodies are never
  compiled to native code — `write` is special-cased directly to a
  standard-output write, not called through a generic FFI trampoline).
- Enum discriminant width: fixed `u32` (4 bytes) always, rather than
  the smallest width that fits the variant count — the simpler choice,
  costing a few bytes of padding. This choice is now load-bearing, not
  just documented intent: `enum_layout` (added in the encode/decode
  fix above) actually implements it, and `e2e_enum_*` in
  `tests/conformance.rs` reads real enum representation bytes back
  through `rawptr`/`write` and asserts this exact width.
- `dangling<T>()` returns address `1` (a real allocation never starts
  at an address that low, since the bump allocator starts at
  `0x1000`); never dereferenced by prelude code, matching the spec's
  own disclaimer.

**`rawptr_of` on a stack local** (formerly a one-time snapshot) now
gives the object an arena address the first time and backs it there --
see "Dynamic fidelity" above.

## Implementation-defined choices (consolidated, 2026-09-21)

Every `outcome: impl-defined` row of `spec/IMPLEMENTATION-NOTES.md` §1,
with this interpreter's committed choice:

| Choice | This interpreter |
|---|---|
| `AddrWidth` | 64 (`value::ADDR_WIDTH`) |
| Byte order `BO` | little-endian (`value::LITTLE_ENDIAN`; every `encode`/`decode` is `to_le_bytes`/`from_le_bytes`) |
| Struct field offsets | declaration order, natural alignment, no reordering, trailing padding to the struct's alignment (`struct_layout`) |
| Enum discriminant width `DW` | 4 (`value::ENUM_DISCRIMINANT_WIDTH`; `enum_layout`, `[Repr-Enum]` image at offset 0) |
| `dangling<T>()` | address 1 (the bump allocator starts at `MAIN_ADDR_BASE`, far above it); never dereferenced by prelude code |
| `fn` value encoding | the 8-byte `DefaultHasher` image of the item's qualified name (`encode`, `[Repr-Fn]`); `==`/`!=` and `spawn` operate on `Value::FnVal` directly, never on the bytes |
| A mutex's extra state cells | a `usize` after `inner`, laid out as `struct { T inner; usize state; }` (`[Sizeof-Mutex]`; `value::mutex_size`), contents never read; the lock word itself is `Shared::cobalt_locks` |
| `handle<T>`'s encoding | `Value::Handle(thread key)` in the object model; if a handle is ever raw-encoded (`encode`) its image is 8 zero bytes -- not injective, observable only by a program that writes a handle-bearing aggregate through a raw pointer, itself outside `FfiType` |
| Termination-outcome / diagnostic reporting form | exit status 0 for `ok`, 1 otherwise; the diagnostic id, phase and the registry's own rule/invariant/required/observed/repair text on stderr (`src/diagnostics.rs`, `src/main.rs`); `lib::run_source` returns `Err(diag id)` |
| Calling convention, name mangling, linking | (2026-09-23) an `extern fn` names a C function by its declared identifier, looked up at the call in the running process (libc) and then in libm (loaded on first use); called through the x86-64 System V convention with at most six integer/`bool` and eight `f32`/`f64` arguments, no stack-passed or variadic callees (`src/ffi.rs`). A `rawptr` argument or result is refused (`unsupported:`, exit 3): the arena's addresses mean nothing to C. A missing symbol is `unsupported: no C function …`, exit 3. The prelude's `write` stays bound to standard output over the arena's real bytes. Other targets: every extern call is `unsupported` |
| File-backed module paths beyond the required core (`rule.module.file`, 2026-09-22) | `..` segments, a leading `./`, and absolute paths are accepted through `std::path` on the host; a resolved path denotes the file `fs::canonicalize` yields, which is also the identity used for cycle/duplicate detection (`src/loader.rs`). For display, a leading `./` is dropped and a path is joined onto the declaring file's directory only when that directory is not the working directory |
| A diagnostic location's file name | the path as written in the declaration, joined onto the declaring file's directory; the entry file by the path given on the command line; a program given as a string is `<input>` (`render` prints `at <file>:<line>`) |

`outcome: unspecified` points (fresh object addresses, thread
interleaving) are exactly that here: object identities are a counter,
addresses a bump allocator, and interleaving is the OS scheduler
through the GIL's statement-boundary yields.

## Conformance coverage is now mechanical (2026-09-21)

`tests/conf_coverage.rs` parses every `conf.*` id out of
`spec/conformance.md` and fails, naming it, if no test references it
(a `// covers: conf.x` comment or a `conf_x`/`ex_e2e_x` test name in
`tests/conformance.rs`, or a mention in a `.cb` case's header). The 19
cases that had none were transcribed or annotated in the same pass;
two of them needed rules this pass added: `[Match-Move-Through-Ref]`
(`diag.move-out-of-field`, static) and `[Destroy-Projection]`/
`[Store-Binding-Place-Transfer-Sub]` for a `move`-captured resource
dropped or moved inside its own closure body (`move_captures`). On the
way, a real bug in the closure free-variable scan was fixed: an item
called by its bare name inside a closure (`drop(v)`, `takes(v)`,
`sizeof<u8>()`, `Some(k)`) counted as a captured variable, so any such
closure was a false `diag.capture-list-mismatch`.

## Verification pass: every spec row, outcome and phase (2026-09-21)

Asked to verify that no known interpreter-versus-spec mismatches
remain, this pass did two mechanical cross-checks and fixed what they
found.

**Registry cross-check.** Every `diag.*` id in
`spec/registry/diagnostics.md` was grepped against the sources. All
are emitted except `diag.return-outside-fn`, which this grammar cannot
reach: a `return` outside a function body is a parse error before any
static rule runs. The strings emitted but not registered are the unit-
test fixtures in `diagnostics.rs`.

**Spec-row check, now a permanent test.** `tests/spec_rows.rs`
assembles every row of `spec/conformance.md` into a program the way
the document itself describes (items at the top level, statements in
`fn main()` with `bool c = true;` in scope, a trailing value bound to
a local; `ex.e2e-*` rows run the example verbatim; rows that lean on
an earlier row's context are completed from a small override table)
and compares the interpreter's outcome *and phase* with the row's
`(static)`/`(dynamic)`/`→ v` column. It checks 126 rows and passes.
Before it passed, it found five genuine mismatches, all fixed:

1. **Uncalled functions were never checked.** The static worklist was
   seeded from `main` and grew by call; `rule.fn.program` says every
   item must be well-typed whether or not it is called. The worklist
   now seeds every declared function (generic ones with `$T` markers).
2. **Later match arms were not typed by the first.** `rule.type.expected`
   makes the first arm's type the expected type of the rest, so
   `match (x) { A : 1, B : 2: u8 }` must be rejected and
   `match (x) { A : 255: u8, B : 256 }` must be a literal-out-of-range,
   not an `i32` arm. `check_match` now carries the first arm's type
   into the rest and records it in `Items::match_hints`, keyed by the
   match expression's address, so the evaluator applies the same
   expected type at run time (the two passes share one `Items`; see
   below).
3. **No static shift-amount fold.** `x << 40` with a literal amount
   out of range for the operand's width is `diag.shift-amount-out-of-
   range (static)` in the spec; it was only caught at run time.
4. **Unbound bare names were only a run-time fault.** `y` with no
   binding, variant, item or intrinsic of that name is now
   `diag.unbound-name` from the static pass. The first version of this
   check was far too eager: a `match` binder whose payload type was
   unknown (an `allocate` result) was tracked by the flow state but
   absent from `gamma`, so the prelude's own `Vec::grow` was rejected.
   Any flow-tracked name, and any closure capture, now counts as bound.
5. **Variant dispatch was flat.** `E::V` resolved its `V` through the
   single `enum_of_variant` table regardless of `E`. `variant_enum`
   (in both passes) resolves by the qualified prefix first and falls
   back to the flat table only for a bare name. This closes the second
   of the two "known, deliberate limitations" recorded above; the
   first (a `::`-path in type position) was closed by the grammar pass.

**One shared `Items`.** `build_interp` now runs `typecheck::check_
program` on the very item table the evaluator uses, so `run_source`
never executes a program the static pass rejects, and the typing facts
recorded for the evaluator (`match_hints`) refer to the evaluator's own
expression nodes. `check_source_static` still exists for callers that
want the static verdict alone.

**One test expectation corrected.** `e2e_dropping_rc_while_a_get_
reference_is_still_in_scope_is_correctly_rejected` expected the generic
`diag.aliasing-conflict`; the spec's own row for that shape
(`conf.destroy-while-borrowed-rejected`) says `[Destroy-Not-Solitary]`'s
`diag.destroy-while-aliased`, which is what the interpreter reports.

**What is left, by design.** `handle`'s raw encoding is not injective
(it is outside `FfiType`); only `write` is a callable extern;
`diag.no-destroy-authority` and `diag.transfer-without-authority` are
unreachable from a well-formed program (the static rules reject the
shapes that would reach them); the `r_is_temp` simplification in the
evaluator is harmless. None of these is a mismatch with the spec's
stated outcomes.

## Aliasing checks no longer scale with the object count (2026-09-21)

Running `cobaltc_examples/12_collatz.cb` (a 10,000-entry `Vec<u32>`
memo table) took 93 seconds and looked like a hang. Micro-benchmarks
isolated it: a plain loop and function calls cost about 5 microseconds
per iteration, but a `Vec::index_shared` cost grew linearly with the
Vec's length -- 200,000 indexes into a 10-element Vec took 5 seconds,
into a 10,000-element Vec more than 300.

The cause was `scan_refs`, which every `clash`/`solitary` check calls:
it walked *every* live object looking for `Ref`/`Guard` occurrences,
and every Vec element ever indexed is a live object (`reclaim` mints
one per address, kept for as long as the buffer). The interpreter now
keeps `ref_holders`, the set of objects whose value has ever held a
reference, maintained at the two places a value enters an object
(`new_object`, `write_place`) and cleared by the single `remove_object`
helper every removal now goes through. `scan_refs` walks only that set.
It is a superset (a holder is never removed while its object lives),
which is harmless: a stale member contributes no occurrences.

Result: the 10,000-element case runs in 5.4 seconds, the Collatz
example in 2.4. `perf_vec_index_cost_does_not_grow_with_vec_length`
guards against the regression with a generous bound. The remaining
~25 microseconds per indexed access is constant per access, mostly the
byte-addressed arena (`HashMap<u64, u8>`, one lookup per byte) and the
per-access decode; left as is.

## Files

- `src/lexer.rs`, `src/parser.rs`, `src/ast.rs` — front end.
- `src/value.rs` — runtime values, platform constants, checked
  arithmetic primitives.
- `src/typecheck.rs` — the static pass: whole-program type-checking,
  literal/flow-analysis-lite overflow refutation, the `deriv` alias
  check, static `[Unsafe-Rejected]`. Independent from `interp.rs`,
  sharing only `Items` (the parsed declaration tables).
- `src/modres.rs` — module visibility and name resolution (new this
  pass, `spec/17`): runs once, right after parsing, rewriting every
  item reference to its resolved fully-qualified key.
- `src/interp.rs` — the dynamic evaluator: the abstract-state model
  (objects, access-path-token-based aliasing, frames, the byte arena),
  expression/statement evaluation, prelude intrinsics, and generic
  type-parameter inference. This is intentionally one large file
  rather than split further, since the evaluator's pieces are heavily
  mutually recursive through `&mut self`.
- `src/prelude.rs` — the verbatim `spec/21` CobaltC source.
- `src/main.rs` — CLI entry point (runs the static pass, then, only if
  it accepts the program, the dynamic evaluator).
- `tests/conformance.rs` — the transcribed conformance suite, plus the
  permanent static-pass regression guard in its `expect_ok`/
  `expect_true`(`_with`) helpers.

## Full silent-bug audit: coverage summary (2026-09-21)

The human owner's "full audit" request covered seven prioritized areas.
Honest accounting of how far this pass actually got, not just what it
found — a real "checked and clean" is recorded distinctly from
"not reached at all":

1. **encode/decode's remaining `Type` combinations** — fully covered
   (this file's own "Sanity pass" section above), including this
   pass's continuation into nested cases (`array<Foo,2>` where `Foo`
   has an enum field; a struct field of enum type) and generic
   type-parameter substitution reaching `sizeof_ty`/`is_resource`
   correctly (tested directly: a generic struct with a resource type
   parameter nested inside `array<T,N>`, both as a plain generic
   struct and constructed from inside a generic *function* — both
   correctly move-tracked, not silently miscategorized as non-resource).
2. **Arithmetic** — covered in depth; five real bugs found and fixed
   (see the two dated sections above). Float NaN/`-0.0`/infinity
   comparison and division, and `wrapping_`/`saturating_add` across
   u8/i8/u16/u64/i64 boundaries, were additionally spot-checked
   directly against a real running program and found correct.
3. **Resource destruction ordering** — covered; one real bug found and
   fixed (struct-field/array-element order, above); block-scope order
   checked as a negative control and confirmed already correct.
4. **Generics** — now covered by deep, dedicated audit (2026-09-21,
   `e2e_deep_generic_nesting_*`/`e2e_four_level_*`/
   `e2e_generic_function_calling_*`/`e2e_multi_param_*`/
   `e2e_resource_type_param_*` in `tests/conformance.rs`), specifically
   closing the "not separately re-verified" gap this section used to
   record. `Vec<Rc<Vec<Option<i32>>>>` (triple nesting, mixing
   resource and non-resource generics), a 4-level homogeneous
   `Vec<Vec<Vec<Vec<i32>>>>`, a generic function calling another
   generic function with a *resource-typed* argument
   (`double_wrap<T>(T x)` calling `wrap<T>(T x)`), a multi-parameter
   user-defined generic struct nested inside itself
   (`Pair<i32, Pair<i32,i32>>`, with byte-level layout verified via
   `write()`), and destruction *order* (not just construction) for a
   resource type parameter inside a generic struct's array field —
   all built as real programs, run through the real binary, checked
   against real values/bytes, all correct. **Zero real bugs found in
   generic nesting itself.** One initially-suspected bug (chasing an
   aliasing-conflict fault through a `Rc::clone`→`Rc::get`→two-index
   chain) turned out, after reduction, to be this interpreter
   *correctly* rejecting a genuinely unsound test program (`drop`ping
   an `Rc` while a reference obtained via `Rc::get` from it was still
   a live, reachable binding, never explicitly scoped out first) — not
   a bug, confirmed by scoping the reference out fixing it, and now
   also a permanent regression test in its own right
   (`e2e_dropping_rc_while_a_get_reference_is_still_in_scope_is_correctly_rejected`)
   guarding the *correct* rejection, not just the nesting cases that
   should succeed.
5. **Concurrency host-level soundness** — originally a code review
   only, explicitly flagged as this pass's weakest-verified claim; redone
   empirically the same day (see "Concurrency lock-bypass audit" above):
   nightly Rust + ThreadSanitizer across the full suite plus purpose-built
   10-way/30-way mutex-contention and fault-during-concurrency stress
   tests, zero race reports. Separately, while hunting a predicted
   duplicate-side-effect bug from the already-documented `exec_block_body`
   resumability gap, found instead that the spawn retry loop's giant
   lock is held for an entire `run_body` call and never released
   mid-computation — a genuine, previously-unstated architectural
   characteristic (not a soundness bug; `spec/19`'s `[Thread-Step]`
   leaves interleaving granularity unspecified), documented there with
   the exact repro that revealed it.
6. **Standard library boundaries** — spot-checked: `Vec` growth
   crossing a capacity boundary (0→1 and the 4→8 doubling point) with
   resource elements, `String::from_utf8` at a valid 4-byte sequence, a
   truncated multi-byte sequence, and an overlong 2-byte encoding of an
   ASCII codepoint, and `Rc` clone/drop in non-LIFO order (clone twice,
   drop the middle one first, use the others after) — all correct, no
   bugs found. Not exhaustively covered.
7. **Diagnostic/registry consistency** — not separately re-verified
   this pass beyond the pre-existing `every_diag_id_this_interpreter_
   emits_is_in_the_registry` test (still passing); no new `diag.*`
   string literal was introduced by any fix in this pass, so nothing
   new needed checking against the registry.

No further silent bugs were found in areas 4–7 beyond what's listed —
but "checked and found nothing" carries less weight than "checked and
found something" for the areas given only a spot check, not a
systematic pass. Treat 4/6 as "probably fine, not proven," 5 as "sound
by code review," and 7 as "unchanged, still passing," distinctly from
areas 1–3's actual bug-hunting depth.

## `str` values, string/byte literals, and `print` (2026-09-22)

`CHG-0025` / D-0020 (`spec/21` §2a, `spec/22` 2.9.0) implemented end
to end.

**What landed.**
- `src/lexer.rs`: `Tok::Str`/`Tok::Bytes` from `lex_quoted`, with the
  two escape sets the grammar fixes (`\u{…}` only in `"…"`, `\xNN` only
  in `b"…"`); `str` joins `TYPE_NAMES`. Unterminated (newline or EOF
  before the closing quote), an escape outside the set, a non-ASCII
  character in `b"…"`, and `b""` are all `LexError`s — the program does
  not parse, matching `spec/22`'s "lexically ill-formed".
- `src/parser.rs`: `"…"` → `ExprKind::StrLit`; `b"…"` is desugared
  *in the parser* to `ExprKind::ArrayLit` of `u8`-suffixed `IntLit`s,
  so the checker and evaluator never see a byte literal at all —
  exactly `spec/16` §3's "concrete syntax only".
- `ast::Type::Str`, `value::Value::Str(Arc<[u8]>)`: a `str` is a plain
  `Value`, copied by cloning the handle; `is_resource_ty`/`is_resource`
  fall through to `false`, so every copy/move/destroy site treats it
  like an integer with no special case. `[T-Lit-Str]` ignores the
  expected type; `check_binary` rejects any operator but `==`/`!=` on
  a `str` operand statically (`diag.type-mismatch`, the `[T-Cmp]`
  numeric requirement).
- Intrinsics `str_len`/`str_byte`/`str_ptr` in `try_intrinsic`;
  `str_byte` faults `diag.index-out-of-bounds` (`[Str-Byte-Out-Of-
  Bounds]`). `String::from_str` and `print` are real prelude CobaltC
  (`src/prelude.rs`), not Rust.
- `Vec<str>` works: `arena_encodable(Str)` is true; `[Repr-Str]` is
  encoded as (address, length) and decoded by reading the bytes back.
  `arena_write` interns every `str` inside the value first (`encode`
  is `&self`).

**Implementation-defined choices (`[Sizeof-Str]`, `[Repr-Str]`,
`[Str-Ptr]`).** `sizeof(str) = 16`, `alignof(str) = 8` (two
`AddrWidth/8` words). A `str`'s bytes get an arena address on first
`str_ptr` or arena encoding, via `bump_alloc` (never reused, never
inside any object's extent); the same byte sequence always yields the
same address (`Interp::str_data`, keyed by content). `print` writes to
fd 1, like `write`.

**Harness change.** `tests/cb_conformance_suite.rs` maps a first
stderr line containing `lex error at` to `parse-error`, alongside
`parse error at`; the four new lexical ill-formedness cases under
`22-surface-syntax` are the first `.cb` cases to fail in the lexer
rather than the parser. Parse and lex errors still report their line
in the *combined* prelude+program text (pre-existing; `resolve_user_
line` only handles `diag.*@N` tags) — unchanged by this pass.

**Honest gaps.** None new. `if (s == 3)` with `s : str` is accepted
statically and never faults (`values_eq` on unlike values is simply
`false`): the same pre-existing laxity that accepts `if (b == x)` for
`b : bool`, `x : i32` — `if`-condition operands are not type-checked
by the static pass — not a `str` issue. No `str` indexing syntax,
slicing, character literal, or fallible `print`, per D-0020's revisit
conditions.

**Tests.** 14 new `.cb` cases (`12-type-system` ×3, `21-standard-
library-semantics` ×5, `22-surface-syntax` ×6), `ex_e2e_hello_print`
in `tests/conformance.rs`, and the twelve new `spec/conformance.md`
rows run by `tests/spec_rows.rs`. `01_hello.cb` and `02_fizzbuzz.cb`
now use `print`; `02`'s output is byte-identical to before.

## Suggested next steps, in priority order

1. (Done -- "Blocking at any depth: the GIL".)
2. (Done -- "rule.control.flow-analysis implemented" and "The remaining
   static rules"; what is left is by design intraprocedural, spec/14 §6
   "Scope".)
3. (Done -- see "Grammar: the disambiguations spec/22 states".)
4. Extend `token_ancestors`/`object_live` treatment to any other
   borrow-forming site added in the future (e.g. if `lock`'s `Guard`
   formation is ever changed to go through a ref-derived place rather
   than a direct argument evaluation) — the pattern is established,
   just apply it at the new site.

### `std` aligned with one design (D-0134, 2026-10-03)

The owner accepted every recommendation of `private/std-proposal.md`
(`CHG-0157`–`CHG-0161`). What changed in the implementation:

- **`StringView` for text a function only reads.** The path and name
  parameters of `read_file`, `write_file`, `read_bytes`, `write_bytes`,
  `File::open`/`create`/`append`/`open_rw`, `make_dir`, `remove_file`,
  `remove_dir`, `rename`, `list_dir`, `path_kind` and `env_var`,
  `write_file`'s text, and the needles of `find`/`starts_with`/`ends_with`
  are `StringView`s. `File::write_str` is `File::write_text(StringView)`;
  `File::printf` writes through the private `File::write_formatted`.
- **`StringView::of(str)`** over the std-only intrinsic `str_slice`.
  `coby`: once per text, a `Vec<u8>` object over the interned bytes that
  never ends (`Interp::str_views`). `cobc`: the slice borrows the object
  `cb_reclaim` establishes over the literal's static bytes (one per
  address). The type checker exempts it from
  `[Call-Multi-Ref-Return-Rejected]`.
- **`[Str-Literal-View]`** is `impl/src/views.rs`, a pass after `consts`:
  a `str` literal (or a `str` constant's use) as an argument whose
  parameter is declared `StringView`, a local's initializer declared
  `StringView`, or a struct literal's field declared `StringView`
  becomes `std::StringView::of(L)`. The type mismatch for a `String` or a
  `str` binding there names the repair (`&s[0..$]`, `StringView::of(t)`).
- **Renames:** `Rng::new`, `Result::map_err` (registered by `modres`,
  realized natively as before), `String::parse`, `HashSet::key_at`, the
  externs `stdout_write`/`stdin_read` and private `stderr_write`.
- **`remove` keeps order:** `HashMap::remove`/`HashSet::remove`/`remove_str`
  are the former order-keeping removal (`remove_slot` shared), and
  `swap_remove` the former constant-time one; `remove_ordered` is gone.
- **`ReadError` is gone;** `read_line` returns `FileError`.
  **`HashSet::clear`** added.
- **Found on the way:** `coby`'s `spawn` ran a deep copy of the callee's
  body, so the checker's per-expression records (keyed by address) were
  lost in the thread, and a `StringView` formed there (`&s[0..$]`) was a
  dynamic `diag.type-mismatch`. It now runs the item's own body
  (`SpawnBody`). Case: `view_formed_in_thread_ok.cb`.
- **Migration** of the repository's programs by
  `stress/std-align/migrate.py` (an old `remove` became `swap_remove`, so
  behaviour is preserved exactly), then by hand where a `ref<String>`
  parameter was passed on or a `match` on `read_line`'s error named only
  `Io` and `Utf8`.

### A `String` a call returns, viewed as an argument (D-0135, 2026-10-03)

D-0134 made paths `StringView`s, so `read_file(&build_path(…)[0..$])`
had to bind the path first: `[View-Form]` (D-0053) wanted a place, and
D-0103's argument exception covered only `[Slice-Form]`. `CHG-0162`
extends it to views (`[View-Form-Temporary-Argument]`):

- **Checker:** the `String` view branch of `SliceOf` takes `cur_temp_ok`
  as the slice branch does and records the expression in `temp_borrows`;
  a call's result is checked as a value, part of a temporary as a place,
  neither with `flow_borrow`.
- **`coby`:** `ViewOp::SliceString` forms the borrow with
  `eval_temp_borrow` for a whole temporary (an object of the statement);
  part of one goes through `eval_borrow` as before.
- **`cobc`:** `lower_view_op` sets `temp_root` around `lower_place`, so
  the temporary becomes an object of the statement (`cb_temp_path`), as
  for D-0103's slices.
- **Kept past the statement** (`StringView w = StringView::trim(&f()[0..$]);`):
  `diag.destroy-while-aliased` in both tools, at the statement.
- **Fault location at a statement's end (`cobc`):** a temporary ending
  with its statement while a kept reference holds it (D-0103's slices
  too) reported the last line run, often inside the callee, because
  `cb_stmt_pop` had no location; `coby` reports the statement. A
  statement scope now carries its statement's line, as a frame does
  (`cb_stmt_push_at`, the `S` marker's location), and `cb_stmt_pop` puts
  it in `dloc` while the temporaries end.
- **Benchmarks:** `strings`, `hashcount` and `tree` draw their keys with
  `std`'s `Rng` (`Rng::new`, `Rng::below`) instead of a hand-written
  generator, and their Rust twins with `bench/rng.rs`, a copy of
  `std::Rng` written the same way. The key sequences differ from the
  earlier generator's, so timings from here on are not directly
  comparable with the logs above dated before 2026-10-03.

### `cobc`: what a call through `&mut x` costs (2026-10-03)

`bench`'s switch to `std::Rng` showed `Rng::below` at 226 instructions a
draw under GCC, against Rust's 22. Measured on 10,000,000 draws
(`stress/rngperf/draw.cb`), three causes, three changes:

- **The borrow's check (60% of the time).** `Rng::below(&mut rng, n)`
  passes `rng` to a *pure direct* parameter (`Gen::pure_direct`: the
  callee uses it only as `r.f`/`*r`, hands it on only to the same, and
  mints no path), yet `rng` was *pinned* (`pinned_names`) for being
  borrowed at all, so it had a runtime object and every call ran
  `cb_borrow_unminted` → `check_access`. Such a borrow no longer pins:
  when the callee is named so that no local can be it (two or more
  segments, `Gen::pure_direct_ref_arg`) and no other argument of the
  call names `x` (`mentions`), nothing can hold a path to `x` but its
  root, so `x` is `unchecked` (spec/14 §6's discharge, as for a local
  never borrowed) and its address is the argument. A local borrowed any
  other way, captured, dropped, or named by another argument of the call
  stays pinned.
- **The stack check.** `cb_stack_check` compared the address of a local,
  which made GCC put a stack-protector canary in every function; it now
  compares `__builtin_frame_address(0)`. A function whose C calls nothing
  that could call back into the program (`calls_nothing`: only C
  keywords, builtins, `cb_at`, `cb_fault`) has no check: it cannot go
  deeper, and its caller's check and the floor's margin cover its frame.
- **Inlining.** Every generated function but `f_main` is `static`, so
  GCC inlines as Clang already did. `f_main` stays external: inlined
  into `cb_program`, which only C's `main` calls, GCC compiled the whole
  program as code run once, for size (a `div` for a `% 5000`).

Result: GCC 226 → 31 instructions a draw (0.33 → 0.048 s), Clang 22
(0.038 s), Rust 22 (0.037 s). Also fixed: `cbrt.h`'s comments on the
externs named them by their pre-D-0134 names.

### `cobc`: `records`, a `Vec` of structs updated through `&mut v[i]` (2026-10-03)

`bench/records` ran 13× Rust's instructions: 80% were run-time checks
(`check_access`, `cb_borrow_check`, `cb_elem_access`) on `step(&mut ps[i])`,
a call that adds four numbers. `ps` was not *confined* (`bind_confined`,
`scan_vec_uses`) for three of its uses; each is now a confined use:

- **`&x[i]` / `&mut x[i]` passed to a pure direct parameter**, no other
  argument naming `x`: the callee keeps no path and is handed the
  element's address alone (`elem_access_place`'s confined case). A call
  by a single name counts only if nothing in the function binds that
  name (`bound_names`: `let`s, destructured fields, match binders,
  closures' parameters, the function's parameters and a closure's
  captures), since a binding of it would be what is called. The same
  check now guards the confining-parameter rule, which had trusted a
  single name.
- **`x[i].f…` read as a value**: the element read, no path formed.
- **`foreach (p in ps)` over a reference parameter whose body reads `p`
  only as `*p` or `p.f…`** (`foreach_param_reads_only`; `block_only_derefs`
  now also accepts `p.f…`, rewritten over the element alias by
  `alias_field_chain` in both `lower_expr` and `lower_place`). In the
  variant for a confined argument, the loop's hidden holders `$e`/`$d`
  are bound as direct references standing for the parameter
  (`confined_holder`), and `lower_place` reaches a confined
  `*Vec::index_*(…)` through `elem_access_place`, as `lower_expr` did.

`records`: GCC 230M → 23.5M instructions (0.039 → 0.009 s), Clang 232M →
17.5M (0.005 s), Rust with overflow checks 17.4M (0.0045 s). Tested with
small programs only (owner: no full battery yet): `stress/recperf/edge`
(a local closure named like the callee, another argument reading the
`Vec`, a loop that writes elements, an element reference held across
the call, which still faults `diag.aliasing-conflict`), every bench
program's answer against Rust under both C compilers, and the `Rng`
round's edge cases.

### `cobc`: `tree`, a `Box` tree built and summed (2026-10-03)

`bench/tree` ran 31× Rust's instructions. Four changes, measured on it:

- **A scope per arm of a statement `match`** (`stmt_match`,
  `branch_scope`). A `match` that is a whole statement or a block's tail,
  its value unused, gives each arm its own statement scope: the arm is
  the statement's last evaluation, so its temporaries ending with the arm
  is their ending with the statement, in the same order. The descending
  arm (`Some(b) : insert(Box::get_mut(b), k)`) forms none, and its scope
  is dropped (`resolve_scopes`) instead of every level paying the
  building arm's `cb_stmt_push`/`cb_stmt_pop`. 954M → 697M.
- **`Box::new` native** (`lower_box_new`) for a `T` with no reference and
  no `fn` value: `Box::cells<T>()` allocated, the alloc fault, the value
  stored, its object (if any) ended by `cb_raw_move_in` as the `std` body
  ends it, `Box { .ptr, .full = true }`; `T` from the expected `Box<T>`
  when the call writes none. `cb_allocate` and `cb_raw_move_in` are
  classified for `resolve_scopes` (free; and the statement only, as
  `cb_absorb`), without which every enclosing scope and frame was kept.
  697M → 457M.
- **Inert literals without objects** (`inert_expr`, `objectless`,
  `inert_c`): a payload-less variant, or a struct literal of a struct
  without a destructor, not `resource`, whose fields are plain data or
  inert, owns nothing; built as a field, a payload or `Box::new`'s
  argument it gets no object (a fault in a later field leaves nothing of
  it to destroy). So does `Box::new(inert)` as a variant's payload, the
  last thing evaluated before the variant. 457M → 278M.
- **Assigning an inert value** (`inert_value`: inert, or a variant of an
  inert value or of `Box::new(inert)`) to a place that is not raw memory:
  no object; only the target's checks come between it and the store, and
  destroying it would run nothing. The building arm then forms nothing
  either. 278M → 212M.

`tree`: GCC 954M → 212M instructions (0.23 → 0.097 s), Clang 948M → 214M;
Rust with overflow checks 30M (0.024 s). What is left: the descent (each
level's `cb_elem_access` on the `Box`'s contents, which no static
reasoning here discharges), the teardown at exit, and `cb_allocate`'s and
`cb_deallocate`'s bookkeeping.

**Found and fixed on the way (also in the pushed Update 48):** a
temporary borrowed as the argument of a pure direct parameter
(`show(&mk(1))`) was evaluated twice -- the branch for `&place` lowered
it, found no path, and left it to the general path, which lowered it
again -- so its effects ran twice and it was destroyed twice. The branch
now takes only a place rooted in a binding (`is_place_expr`,
`root_bound`, as the slice branch above it). Tested with small programs
only (owner: no full battery yet): `stress/treeperf/edge` (a fault after
an inert field, destructor order with `Box`es, `Box::new` of `String`s,
of a moved local and of a struct with a reference, overwriting a live
`Some`, temporaries in arms, the double evaluation), the `Rng` and
`records` rounds' edge cases, and every bench program's answer against
Rust under both C compilers.

### `cbrt`: allocation, element checks and teardown (2026-10-03)

The next of `tree`'s costs, all in the runtime:

- **Allocation.** `cb_allocate`/`cb_deallocate` registered every block in
  a global map under a lock (a hash insert and remove per `Box`), only to
  recover its layout. A block aligned no more than the C allocator's
  guarantee (`MALLOC_ALIGN`: 16 on 64-bit) now comes from `malloc` and
  goes back to `free`; only an over-aligned one uses Rust's allocator and
  the map, and a count of those lets `free` skip the lock while there are
  none. `deallocate` of anything `allocate` did not return was a silent
  no-op and is now the C allocator's undefined behaviour: a false
  `[Deallocate]` claim, which `[Trusted-Claim-False]` leaves unconstrained.
- **Element checks.** `cb_reclaimed_entries`, the number of entries in
  `Reclaimed` (at least the live objects over raw cells), is kept by
  every change to it and read by an inline `cb_elem_access` in `cbrt.h`:
  while it is 0 no object lies over any element or `Box` content, and the
  call to the runtime (`cb_elem_access_slow`, which would return at once)
  is not made. `cb_release` and `box_drop` skip their lookups the same way.
- **Teardown.** A struct's destruction visits only its owning fields
  (`TypeInfo::owning`, computed once at `cb_init`), and an enum's no
  longer copies its list of payload types (an allocation per enum
  destroyed).

`tree`: GCC 212M → 106M instructions (0.097 → 0.058 s), Clang 214M →
110M; from 954M at the start of the `tree` round; Rust with overflow
checks 30M (0.024 s). `hashcount` 451M → 382M, `strings` 1007M → 952M
from the allocation change. Tested with small programs only (owner): all
the `Rng`, `records` and `tree` rounds' edge cases agree under `coby`,
`cobc`+gcc and `cobc`+clang, and every bench program's answer matches
Rust under both C compilers.

### `cobc`/`cbrt`: `strings`, a `Vec<String>` read in loops (2026-10-03)

`bench/strings` ran 3.9× Rust's instructions; about 40% was minting and
retiring a path per element in `foreach (w in &words)`, whose elements own
a buffer and so took no part in the plain-element aliasing of the earlier
rounds. Four changes:

- **Reader parameters** (`Gen::reader_param`, `reader_uses`): a shared
  reference the body only reads through (`p.f…`, `p[i]…`, `*p` as values)
  and hands on, whole or as `&p.f…`, to other readers, with a result
  holding no reference. Nothing derived from the reference is kept,
  returned, stored or written through.
- **Token 0 is a proven reference** (`cbrt`): `cb_read`, `cb_borrow_check`
  and `cb_borrow_unminted` through it return at once, `cb_borrow` from it
  gives 0, `cb_store_ref` of it leaves the slot empty (so `cb_load_ref`
  gives 0 back). Ids start at 1, so no real path is 0, and a check on 0
  used to be a stale-binding fault nothing relied on.
- **Reader loops** (`foreach_alias`): `foreach (x in &v)` over elements
  that own something but hold no reference, its body handing `x` only to
  reader parameters (a single-name callee only if nothing in the body
  binds the name) or reading its plain fields, aliases `x` to the element;
  `x` used whole is `(cb_ref){ &element, 0 }`. The holder `$c` holds `v`
  shared for the whole loop, so no exclusive path into it can exist and
  every shared read of an element is proven. No object, path, frame or
  scope per step: the element costs its bounds check.
- **Runtime:** `cb_format` keeps its copied arguments in a thread-local list
  and reuses each text argument's buffer (it allocated a list and a copy
  of each text per call); `cb_deallocate_plain` frees cells that never
  held a reference (a `String`'s bytes, buffers of `String`s or of plain
  elements) without the slot scan, taking `cb_deallocate`'s full path only
  while an object could lie over raw cells or an over-aligned allocation
  is live.

`strings`: GCC 952M → 438M instructions (0.218 → 0.123 s), Clang 960M →
451M; Rust with overflow checks 244M (0.070 s). `hashcount` 382M → 315M
(0.080 → 0.068 s). What is left in `strings` is spread: about 600,000
access checks on `words` and `counts` (two per push, two per map entry,
one per loop step, ~100 instructions each), and the sort's comparisons,
the allocations and the copies Rust makes too. Tested with small programs
only (owner): `stress/strperf/edge` (a non-reader that keeps the
reference, a local closure named like a reader, a push inside the loop,
`sprintf` of its own earlier results, duplicate map keys) and every
earlier round's edge cases agree under `coby`, `cobc`+gcc and `cobc`+clang;
every bench answer matches Rust under both C compilers.

### `cobc -v` (2026-10-03)

`-v`/`--verbose` prints, on standard error, each command `cobc` runs before
running it -- the C compiler on the generated C, `cc -c` on each C file an
`extern "…";` names (`link.rs`), and under `--run` the program -- as a
line beginning `+ `, each word quoted for a shell where needed (`show`);
and where the generated C is written and whether it is kept or removed
(`note`). Nothing else changes: without `-v` the output is as before.

### Smaller executables (2026-10-03)

A compiled hello world was 5.9 MB: 4.4 MB of it the debug information of
the Rust standard library and the runtime in `libcbrt.a`, most of the
rest standard-library code nothing calls. On Linux, `cobc` now links
with `-Wl,--gc-sections -Wl,--strip-debug` (`LINK_TRIM` in
`cobc/src/main.rs`): 0.8 MB under GCC and Clang, the symbol table kept for
profilers and debuggers (`strip` takes it to 0.6 MB). The generated C is
compiled without `-g`, so no debug information of the program's own is
lost. `cobc --help` and the guide's build-by-hand command say so.
Checked: every showcase identical under `coby`, `cobc`+gcc and `cobc`+clang,
the edge-case programs of the performance rounds, and a program linking
its own C file.
`cobc -s`/`--strip` links with `-Wl,--strip-all` in place of
`--strip-debug` (`link_trim`): no symbol table either, 0.6 MB for a hello
world. Nothing at run time reads symbols (faults report the compiled-in
`cb_at` locations), so the program behaves the same; only profilers and
debuggers lose the names. (`cc -s` beside `--strip-debug` did nothing:
the driver puts `-s` first and the linker keeps the last strip option.)

### A literal viewed again after an early return (`cbrt`, 2026-10-03)

`cobc` faulted `diag.stale-binding` in a common idiom: a function that
calls `StringView::find(a, "ti")` with the needle written as a literal,
called once where `find` returns at once (the needle longer than the
text) and then where it enters its loop. The literal goes through
`StringView::of`, whose `[Reclaim]` at the literal's static address
re-attaches the object the runtime already has there; the first call's
view had ended, and `retire_token` removed its paths but left the
object's `root` naming the dead token, which `cb_reclaim` handed back.
`retire_token` now clears `root` when it retires it, so the next
`[Reclaim]` mints a new root. `coby` was right throughout. Case
`21-standard-library-semantics/literal_view_reclaimed_again_ok.cb`
(fails on the old runtime under GCC and Clang).

### `std` in submodules; re-export (D-0136, 2026-10-03)

`export import p;` (`CHG-0163`) and `std` divided by subject
(`CHG-0164`): `std::core`, `collections`, `text`, `io`, `sys`, `memory`,
`sync`, `random`, `math`, each re-exported by `std`'s root.

- **Source:** `std`'s CobaltC moved from the one string `PRELUDE_SRC` to
  `std/*.cb`: `std.cb` is the root's body (the primitives private to
  `std`, and `export module core "core.cb";` … with the re-exports), the
  others one submodule each. `prelude::source()` embeds them
  (`include_str!`) and expands each submodule declaration in place, as
  the loader expands a program's, so the tools still need no files at
  run time and `line_count` is unchanged in kind. `cobfmt --check
  impl/std` passes.
- **Front end (`modres`):** an `import` carries its `export`; `reexport`
  enters, in each re-exporting module's item table, an alias (the
  original key, exported) for every exported entry of the target module,
  or for the one item named, repeated to a fixed point so chains and
  cycles settle; a name already there for a different key is
  `diag.duplicate-item` at the `export import`. A module hop of
  `[Resolve-Qualified]` now looks in the module's item table (aliases
  included) rather than at `M::seg` keys, and `m::V` finds the enum among
  the module's items.
- **Keys:** an item of a `std` submodule is keyed by its short path,
  `std::Vec::push` (`modres::qualify`), the proposal's Q3 (a): every
  `std::…` name written in `typecheck`, `interp`, `each`, `views` and
  `cobc`'s `lower` (about 400), the C names `cobc` emits, and every
  message stay as they were. `std`'s submodules share one privacy
  (`visible` treats a `std::m` declaring module as `std`), because
  `std::io` and `std::sys` use `Vec`'s and `StringView`'s private parts.
- **`std::math`:** the 14 floating-point functions are no longer
  intrinsics: `modres` enters them as natively realized items of
  `std::math` (keys `std::sqrt`, …), and `typecheck`, `interp` and
  `lower` match those keys where they matched the bare names. A
  program's own root `round` is keyed `round`, no longer `round$`.
- **Messages:** an unbound `sqrt`, `printf`, … names the import that
  brings it in.
- **Clause (2b) brings no submodule.** The battery found
  `showcase/tier3/tinyos.cb` broken: its nested module `cpu` imports
  `std` and names the program's root module `memory`, and (2b), which
  comes before (3), now found `std::memory` first. A module import now
  brings an imported module's exported items other than modules
  (`resolve_unqualified`); case `17-modules/module_import_brings_no_submodule_ok.cb`.

### `Queue<T>` and `PriorityQueue<T>` (D-0137, 2026-10-04)

`CHG-0165`: two collections in `std::collections` (`spec/21` §2i),
written in CobaltC in `std/collections.cb`; both tools run them as
written, with no native realization.

- **`Queue<T>`:** two `Vec`s as stacks (`head`: front part, front
  element last; `tail`: back part in order). An empty part is refilled
  from the other by `Queue::shift` (`copy_raw` per element, one
  `release` for the range): all elements until both ends have been
  taken from (`took_front`, `took_back`), half after, so alternating
  ends stays amortized O(1). `cobc` realizes `front`/`back` natively
  (`native_queue_peek`, element types as for the native `HashMap::get`)
  and `pop_front`/`pop_back` (`native_queue_op`, plain element types,
  as the native `Vec::pop`); the pushes stay the body's (its call of the
  native `Vec::push` measured faster than a native push).
- **`PriorityQueue<T>`:** a binary heap in `items: Vec<T>`, with each
  element's arrival number at the same position of `seqs: Vec<u64>`;
  ties leave first in, first out. Elements move by `copy_raw` +
  `release` through the buffer pointer (no `Vec::swap`); `pop` moves the
  hole to a leaf (one comparison per level), then the last element in
  and up. One comparison decides (the earlier arrival unless the later
  goes before it); `new` compares with `key_less_at` in place, `new_by`
  calls `less` on two element references. `[Priority-Queue-Not-Key]` is
  checked in `typecheck` at `PriorityQueue::new`'s call, as
  `Vec::sort`'s, which also covers a generic function's instantiation.
- **`cobc`:** `key_less_at` of a non-key element (the branch a `new_by`
  queue never takes) lowers to a run-time `diag.type-mismatch` instead
  of an internal error; before, a `new_by` queue of references or `Rc`s
  did not compile.
- **`Queue` speed under `cobc` (gcc -O2), 200,000 elements:**
  `push_back` 44 ms, `pop_front` 74 ms (the ring buffer: 100 ms, 140 ms;
  a program's own `Vec` + head index, stress/queues/q12.cb: 42 ms,
  68 ms); `front` about 1.0 µs per call, as `HashMap::get` (was 1.6).
  Benchmarks: stress/queues/q11.cb, q14.cb, q15.cb.
- **`PriorityQueue` speed under `cobc` (gcc -O2), 40,000 elements:** `PriorityQueue<u64>` push
  85 ms, pop 230 ms by key order (`new_by`: 125 ms, 570 ms); a heap a
  program writes over `Vec<u64>` with `Vec::swap` (stress/queues/q4.cb)
  pops in 650 ms. The first version (pairs in one `Vec`, `Vec::swap`,
  two `less` calls) took 1.2 s (2.0 s). `String` elements: push + pop
  510 ms (was 620). A struct key is ~10% slower than with pairs in one
  `Vec`. Benchmarks: stress/queues/q6.cb, q9.cb.
- **Cases:** `21-standard-library-semantics/queue_*.cb`,
  `priority_queue_*.cb`; rows `conf.queue-*`, `conf.priority-queue-*`.

### `HashSet::union`, `intersection`, `difference` (D-0138, 2026-10-03)

`CHG-0166`: three functions in `std/collections.cb`, written in
CobaltC over `key_at`, `contains`, `insert` and `clone` (`K: clone`);
both tools run them as written. New sets, arguments unchanged; the
first set's order, then (for `union`) the second's new keys. Case
`21-standard-library-semantics/hashset_set_operations_ok.cb`.

### `String::contains`, `replace`, `join` (D-0139, 2026-10-04)

`CHG-0167`: six functions in `std/text.cb`, written in CobaltC over
`find`/`at` and the private `append_view`/`append_string`; both tools
run them as written. `contains` is `find != None`; `replace` builds a
new `String` left to right without overlap (empty `from` copies);
`join` exists for a slice of views and for a slice of `String`s. Case
`21-standard-library-semantics/string_replace_join_contains_ok.cb`.

### `std::time`: `DateTime`, `Weekday`, `unix_ms` (D-0140, 2026-10-04)

`CHG-0168`: a new submodule, `std/time.cb`, written in CobaltC: the
proleptic Gregorian calendar in UTC by H. Hinnant's `civil_from_days` /
`days_from_civil` over `i64` (floor division written out, since `/`
truncates). `from_unix` is total over `i64`; `to_unix` checks each
multiplication and addition and counts back from the day after for
negative days, so `i64::MIN` round-trips; a hand-built invalid value is
`diag.invalid-datetime` (registered). `weekday` reduces the year modulo
400 first, so it never overflows. `from_iso` is a hand-written scanner
over the view's bytes (no allocation); `to_iso` uses `String::appendf`.
`unix_ms` is a new `clock_read(2)` in `src/fileio.rs`.

- **Oracle sweep** (`stress/datetime/sweep.cb` + `sweep.py`, cobc/gcc):
  every day from year -10000 to 10000 plus 10⁶ random seconds in
  ±2⁴³ — 8,304,850 values — `to_iso`, weekday against Python's
  `datetime` (shifted by 400-year cycles outside its years 1–9999): 0
  mismatches; each read back by `from_iso` and `to_unix`: 0 mismatches.
- **Cases:** `21-standard-library-semantics/datetime_*.cb`; rows
  `conf.datetime-*`, `conf.unix-ms`.

### Paths and file metadata (D-0141, 2026-10-04)

`CHG-0169`: `path_join`, `path_parent`, `path_file_name`, `path_stem`,
`path_extension`, `path_is_absolute`, `path_normalize`, `path_canonical`,
`FileInfo`/`file_info`, `copy_file`, `make_dir_all`, `remove_dir_all`,
`set_current_dir`, `temp_dir`, `home_dir` in `std/sys.cb`, over a new
std-private primitive `path_op`, realized in `src/fileio.rs` with Rust's
`std::path`/`std::fs` (the path grammar is the platform's).
`path_normalize` is lexical over `Path::components()`.

**One primitive shape for the new native facilities:** `path_op`,
`proc_op` and `net_op` all take `(op, h, a, an, b, bn, out, cap) : i64` —
two byte strings in, a number back, up to `cap` bytes of an answer out
(`std`'s `byte_op`/`byte_answer` helpers, `fs_answer`'s ask-again
protocol). `coby` routes the three through one dispatch in
`call_extern` that runs the Rust call inside `Interp::unblocked`, which
releases the GIL while other threads are live and restores the thread's
position after (as `block_until` does); `cbrt` has `cb_path_op`,
`cb_proc_op`, `cb_net_op` over one `byte_call`; `cobc` lowers the four
new primitives (with `os_random`) to those names from a table in
`lower_call`. `cbrt/build.rs` stamps `procio.rs`, `netio.rs`,
`osrand.rs`.

- **The `platform:` case header** (`// platform: unix`), honoured by
  `cb_conformance_suite`, `spec_rows`, `compiled_suite`,
  `compiled_spec_rows` (`tests/common::platform_skipped`).
- **Cases:** `path_parts_ok.cb` (Unix), `file_info_dirs_ok.cb`.

### Operating-system randomness (D-0142, 2026-10-04)

`CHG-0170`: `os_random_bytes`, `os_random_u64`, `Rng::from_os` in
`std/random.cb` over `os_random` (`src/osrand.rs`): Linux `getrandom(2)`
by syscall number (x86-64 and aarch64) with a `/dev/urandom` fallback,
other Unix `/dev/urandom`, Windows `BCryptGenRandom` (`bcrypt` added to
`cbrt::LINK_LIBS`). `diag.entropy-unavailable` (registered) when none.
Case `os_random_ok.cb`.

### `std::process`: `Command`, `Child`, interrupts (D-0143, 2026-10-04)

`CHG-0171`: a new submodule, `std/process.cb`, over `proc_op`
(`src/procio.rs`, Rust's `std::process`). A `Command` crosses as a
length-prefixed byte encoding. `output` runs to the end (input written
from its own thread while both pipes drain) and parks the record in a
table until `std` copies it out (`TAKE`, so the ask-again protocol never
reruns a program); children live in a handle table whose entries are
locked individually. Statuses are the exit code or minus the signal.
`watch_interrupts` installs `signal` + `siginterrupt(…, 1)` handlers for
SIGINT/SIGTERM (`SetConsoleCtrlHandler` on Windows) that set an atomic
flag; coby had no SIGINT handling of its own to conflict with.

- **Stress** (`stress/process/`): `pr1.cb` (every operation), `intr.cb`
  (a child sends `kill -INT $PPID`; the flag is seen, the program ends
  normally) — identical under coby and both cobc builds.
- **Cases:** `process_output_ok.cb`, `process_child_ok.cb` (Unix),
  `interrupt_flag_ok.cb`.

### `std::net`: TCP, UDP, resolution (D-0144, 2026-10-04)

`CHG-0172`: a new submodule, `std/net.cb`, over `net_op`
(`src/netio.rs`, Rust's `std::net`). One handle table for listeners,
streams and UDP sockets, each entry locked on its own (a thread blocked
in `accept` holds nothing else); streams keep a read-ahead buffer for
`read_line`. Addresses cross as 19-byte records; `IpAddr::parse`/`text`
use Rust's grammar, `SocketAddr::parse` splits the port in CobaltC.
Accept timeouts poll a non-blocking listener (1 ms, then 5 ms pauses);
while interrupts are watched an untimed `accept` polls too and gives
`TimedOut` once the flag is set; an interrupted `read` gives `TimedOut`.
`TcpStream::printf` is rewritten by `modres` to
`TcpStream::write_formatted`, as `File::printf` is.

- **Stress** (`stress/net/`): `n1.cb` (every operation, one program);
  `echo50.cb` (a thread per connection, 50 client threads × 20 lines:
  1000/1000 echoes under coby and both cobc builds; coby 23 s wall);
  `bulk.cb` (N MiB with writes and reads of random sizes, checked by a
  CRC-32 computed in CobaltC: 64 MiB arrive intact, same CRC under gcc
  and clang); `shutdown.cb` (a server loop stopped by SIGINT through
  the flag).
- **Throughput (cobc, gcc -O2, loopback, one connection):** 490 MiB/s
  with reads of up to 100,000 bytes and writes of up to 64 KiB
  (`bulk.cb 256 nocheck`); about 20 MiB/s when every byte also goes
  through a CobaltC CRC loop.
- **Cases:** `net_tcp_ok.cb`, `net_addr_ok.cb`, `net_udp_resolve_ok.cb`.

### `read_all`, `read_all_bytes` (D-0145, 2026-10-04)

`CHG-0173`: in `std/io.cb`, over `stdin_read` straight into a `Vec`'s
spare room, 64 KiB at a time. Cases `read_all_ok.cb`,
`read_all_bytes_ok.cb`, `read_all_not_utf8_ok.cb`.

### The battery after D-0140..D-0145 (2026-10-04)

The full battery (the one owed since Update 55): `cargo test
--workspace --no-fail-fast` in debug and in release, `gen.py --check`
(204 examples) and `showcase/run_all.sh`. Everything passed but one
showcase, `tier9/safe_save.cb`, under all three tools: its
`return Err(NotFound);` had become `[Resolve-Ambiguous]`, because the new
`ProcessError` and `NetError` repeated `FileError`'s variant names
(`NotFound`, `Denied`, `Io`, `Utf8`) and a variant written as a value
has no expected type to choose between imported enums. Fixed by
D-0134's own convention: running programs fails with `FileError`
(`ProcessError`, which had exactly its variants, is gone), and
`NetError`'s four are `UnknownHost`, `NotPermitted`, `Failed`,
`NotUtf8`. Case `std_variant_names_unique_ok.cb` checks that no two `std`
enums share a variant name. After the fix: release workspace tests,
`gen.py --check`, every showcase (Tier 10 included), the stress programs
and a sanitizer run of the net and process programs (results below).

### `unsafe extern fn` (D-0146, 2026-10-04)

`CHG-0174`, Rust 2024's rule: a foreign function is declared `unsafe
extern fn` (`export unsafe extern fn` with a visibility). The shared
parser (`parse_item`) accepts `unsafe` before `extern fn`, refuses a
plain `extern fn` with the repair in the message, and refuses `unsafe`
before `extern "…";`. Every declaration in `std`, the cases, the inline
tests, the guide and Tier 4 was rewritten; meaning is unchanged. Cases
`20-trust-boundaries/extern_without_unsafe_rejected.cb`,
`unsafe_extern_code_rejected.cb`.

### Building with Rust 1.77.2 (Windows 7), 2026-10-04

Rust 1.77.2 is the last release that runs on Windows 7, where CobaltC is
also built. `netio.rs` had used `ErrorKind::HostUnreachable`,
`NetworkUnreachable` and `NetworkDown` (stable only since 1.83: E0658);
`code` now recognises those failures by the system's error number
(`WSAENETDOWN`/`WSAENETUNREACH`/`WSAEHOSTUNREACH` on Windows,
`ENETDOWN`/`ENETUNREACH`/`EHOSTUNREACH` on Linux and on macOS and the
BSDs), the same mapping under every compiler. `Cargo.lock` is kept at
lock-file version 3, which Cargo 1.77 reads. Checked: `cargo +1.77.2
check --workspace --tests` for Linux and for `x86_64-pc-windows-msvc`,
and a 1.77.2 build running `stress/net/n1.cb` under coby and cobc
(gcc, clang).

### Review of D-0140..D-0145, two fixes (2026-10-04)

A review of the std-deployable work, with probe programs under
`stress/net`, `stress/process` and `stress/datetime`, found two defects,
both now fixed:

- **`UdpSocket::recv_from(u, max)` of a datagram longer than `max`**
  gave more bytes than asked for, some of them not the datagram's: the
  receive buffer in `netio.rs` was `cap` bytes (`max` and the 19-byte
  sender record), so the answer outgrew `cap` and `std` set the vector's
  length from the whole answer. The buffer is now `cap` less the record.
  Case `net_udp_truncate_ok.cb` (`conf.net-udp-truncate`).
- **SIGPIPE ended compiled programs.** `Command::output_with_input` to a
  program that exits without reading, and `Child::write_input` after the
  child's end, killed a `cobc` program with signal 13 where `coby` (a
  Rust program, whose start-up ignores SIGPIPE) gave `Err(Io)`. `cbrt`
  now ignores SIGPIPE in `cb_set_args`, so the tools agree; a compiled
  program writing to a closed standard output (`prog | head`) now fails
  its writes rather than dying, as `coby` did already. `spec/21` 4.11.1
  (`[Child-Streams]`), D-0143 amended. Case `process_closed_pipe_ok.cb`
  (`conf.process-closed-pipe`).

Also checked by the review and found right: the spec listings equal the
`std` sources, `cobfmt` leaves every new file unchanged, the Windows and
Rust 1.77.2 checks, dates at the `i64` limits, lines longer than the
8 KiB read-ahead chunk on child pipes and sockets, `SocketAddr::parse`'s
rejections. The full battery is still owed.

### Local time: `local_offset_seconds`, `DateTime::to_local` (D-0147, 2026-10-04)

`CHG-0175`: the machine's own zone, nothing more. `local_offset_seconds(i64
unix) : i64` is the platform's offset from UTC at that moment, daylight
saving included, over a new std-private primitive `tz_offset` realized
in `src/fileio.rs` for both tools: Unix `localtime_r` (only the nine
standard `struct tm` fields are read; the offset is computed from them
with Hinnant's `days_from_civil`, so `tm_gmtoff` is not relied on),
Windows `GetTimeZoneInformationForYear` + `SystemTimeToTzSpecificLocalTime`
(Vista and later, so Windows 7 and Rust 1.77.2 are fine). For a moment the
platform has no rule for, the offset now (the owner's choice over an
`Option`), clamped to ±18 hours. `DateTime::to_local` is `from_unix` of
the checked sum, `diag.invalid-datetime` on overflow, in CobaltC. The
result is zone-less: the guide says store UTC, convert last, print by
fields, never `to_iso` a local value.

- **Cases:** `datetime_local_ok.cb` (`conf.local-offset-range`,
  `conf.to-local-consistent`); the offset is never printed. Rust tests
  `fileio::tz_tests`.

### `min`, `max`, `abs`, `pow` move to `std::math` (D-0148, 2026-10-04)

`CHG-0176`: the four generic numeric helpers of D-0091 leave
`std/core.cb` for `std/math.cb`, unchanged; `std::math` is "numbers",
not "the natively realized float functions". `import std;` and every
unqualified or `std::`-qualified use is unaffected; no checked-in
program wrote `std::core::min`. `spec/21` 4.13.0, the guide's module
and intro tables.

### `[Str-Literal-String]`: a literal where a `String` is declared (D-0149, 2026-10-04)

`CHG-0177`: `String s = "text";`, a struct literal's `String` field, an
argument whose parameter is `String`, and what a function declared
`: String` returns (`return L;`, the body's tail through `if` and
`match`) are `String::from_str(L)`, by the same typing rule as D-0134's
view literals and in the same shared pass, `src/views.rs`, now carrying
two kinds. Nothing changed in the checker, the interpreter or the
compiler. `auto` keeps a literal a `str`; a `str` binding is never
converted (`string_of_str_binding_rejected.cb`). D-0113's earlier "no"
is annotated. Cases `str_literal_string_ok.cb`,
`string_of_str_binding_rejected.cb`.

### `exit(status)` (D-0150, 2026-10-04)

`CHG-0178`: `exit(u8) : never` in `std::process`, realized natively in
both tools (`modres` enters it as a native item, as `printf` and
`assert`; the checker types it `never`). `coby` raises its fault flow
with an internal `$exit:N` marker, so the calling thread unwinds through
the existing `[Fault-Unwind]` path (destructors run) and the top turns the
marker into `ok(N)`; a spawned thread's `exit` reaches `main` through the
shared program-outcome field a thread's fault uses. `cobc` emits
`cb_exit(s)`: `cbrt`'s fault unwind without a report, ending with the
status; a destructor that faults during that unwind is rendered and the
status is 1. `rule.fn.program` `[Terminate-Exit]` (`spec/15` 1.12.0),
`spec/18` 1.5.2, `spec/21` 4.15.0 `[Exit]`. Cases `process_exit_ok.cb`
(three calls deep, in a `match` arm; both destructors, inner first),
`process_exit_thread_ok.cb` (from a worker; `main`'s frames not unwound).

### `std::crypto`: SHA-256, HMAC-SHA-256, `digest_eq`; hex and base64 (D-0151, 2026-10-04)

`CHG-0179`: a new submodule `std/crypto.cb`, written in CobaltC (FIPS
180-4 and RFC 2104 as listed in `spec/21` §2m): `Sha256` as a plain
struct (state, pending block, length) with `new`/`update`/`finish` and
the one-shot `sha256`; `HmacSha256` over it; `digest_eq` comparing every
byte. `to_hex`, `from_hex`, `base64_encode`, `base64_decode` in
`std/text.cb` (`[Hex]`, `[Base64]`). Both tools compute the same
listing; nothing native. **A new `std` file must also be entered in
`src/prelude.rs`'s `STD_FILES`** (the tools embed `std`; forgetting it
makes every `import std;` a syntax error at `export module crypto`).

- **Vectors:** FIPS 180-4 (empty, `abc`, 56 and 112 bytes), the padding
  boundaries 55/56/63/64/65 bytes and a thousand `a` (Python's
  `hashlib`), RFC 4231 cases 1–4, 6, 7, RFC 4648's base64 vectors; cases
  `crypto_sha256_ok.cb`, `crypto_hmac_ok.cb`, `text_hex_base64_ok.cb`.
- **Sweep** (`stress/crypto/sweep.py` driving `sweep.cb`): 435 random
  messages (0–10,000 bytes) and keys (0–200 bytes) against Python's
  `hashlib`, `hmac` and `base64` under coby, cobc/gcc and cobc/clang;
  the streaming form against the one-shot form; base64 round trips.
- **Toward the owner's DNS-over-TLS client:** next HKDF, then
  ChaCha20-Poly1305, X25519, X.509 and the TLS 1.3 handshake, each its
  own decision.

### HKDF-SHA-256 (D-0152, 2026-10-04)

`CHG-0180`: `hkdf_extract`, `hkdf_expand`, `hkdf_sha256` in
`std/crypto.cb`, RFC 5869 over `HmacSha256`, in CobaltC; `len` above
8160 is the registered fault `diag.hkdf-length`. The first step of the
agreed crypto roadmap toward the DNS-over-TLS client (TLS 1.3's key
schedule is HKDF). Vectors: RFC 5869 A.1–A.3, the 8160 bound and the
fault above it (cases `crypto_hkdf_ok.cb`, `crypto_hkdf_too_long_faults.cb`);
the stress sweep (`stress/crypto/sweep.py`) gained an HKDF column
against a Python HKDF over `hmac`.

### ChaCha20-Poly1305 and X25519 (D-0153, D-0154, 2026-10-04)

`CHG-0181`, `CHG-0182`: `chacha20`, `poly1305`, `chacha20_poly1305_seal`,
`chacha20_poly1305_open` (RFC 8439) and `x25519`, `x25519_public_key`,
`x25519_private_key` (RFC 7748) in `std/crypto.cb`, in CobaltC.
Poly1305 is poly1305-donna's five 26-bit limbs in `u64`; Curve25519's
field is five 51-bit limbs multiplied through `u128`, the ladder swapping
by mask. `[Constant-Time]` in `spec/21` §2m states that no branch or index
depends on secret bytes. Wrong key/nonce lengths fault
`diag.crypto-length`. `open` and `x25519` return `Option` (one way to
fail each). Both drafts passed every RFC vector on their first run.

- **Cases:** `crypto_chacha20_poly1305_ok.cb` (RFC 8439 §2.4.2, §2.5.2,
  §2.8.2; tampering refused), `crypto_x25519_ok.cb` (RFC 7748 §5.2, §6.1,
  small order refused), `crypto_length_faults.cb`.
- **Sweeps:** `stress/crypto/aead_sweep.py` (random keys, nonces, aad,
  messages against Python's `cryptography`: seal, open, tamper, raw
  ChaCha20 at counter 7, Poly1305) and `x25519_sweep.py` (random and
  small-order keys; RFC 7748's 1000-round iterated vector under cobc).

### Literals in arms and branches (D-0155, 2026-10-04)

`CHG-0183`: `src/views.rs`'s `wrap` follows block, `if` and `match` tails,
so `String s = match (o) { Some(x) : …, None : "none", };` is accepted, as
the literal alone was (D-0149). Found writing the X25519 sweep. Case
`str_literal_in_arms_ok.cb`.

### The crypto roadmap to TLS 1.3 (D-0156..D-0161, 2026-10-04)

Done at the owner's "proceed with the entire roadmap using your
discretion"; all of it in CobaltC, in `std`:

- **D-0156 SHA-512/384** (`CHG-0184`): `Sha512`, `Sha384`, `sha512`,
  `sha384`; constants computed from their definitions.
- **D-0157 `BigUint`** (`CHG-0185`, `std/math.cb`): any-size unsigned
  integers, Knuth division, `mod_pow`, decimal text; not constant-time
  (public values only). A resource: `overwrite` to reassign.
- **D-0158 RSA verify** (`CHG-0186`): `HashKind`, `hash`,
  `RsaPublicKey`, PKCS #1 v1.5 (block rebuilt and compared whole) and PSS.
- **D-0159 ECDSA verify** (`CHG-0187`): P-256 and P-384 over fixed-limb
  Montgomery arithmetic (`u128` products, precomputed constants),
  Jacobian points, Shamir's trick. A first version over `BigUint` took
  242 s per P-384 verification under coby; this one takes 25 s under
  coby and under a millisecond compiled.
- **D-0160 `std::x509`** (`CHG-0188`, new `std/x509.cb`): strict DER,
  certificate parsing, PEM, `TrustStore` (Unix bundle files),
  `verify_chain` with RFC 6125 name matching. The system's 121 roots
  parse; 115 self-signatures verify (the others are SHA-1 or P-521).
- **D-0161 `std::tls`** (`CHG-0189`, new `std/tls.cb`): a TLS 1.3 client,
  ChaCha20-Poly1305 + X25519 only, chain, CertificateVerify and Finished
  checked, KeyUpdate followed, close_notify; `hkdf_expand_label` in
  `std::crypto`. The first handshake against OpenSSL succeeded unchanged.

**Tests.** Cases `crypto_sha512_ok`, `bigint_ok`, `bigint_sub_faults`,
`crypto_rsa_ok`, `crypto_ecdsa_p256_ok`, `crypto_ecdsa_p384_ok`,
`crypto_hkdf_expand_label_ok` (RFC 8448), `x509_chain_ok` (an openssl
PKI embedded as PEM), `tls_refusals_ok` (hermetic). Outside the suite:
`stress/crypto/pk_vectors.txt` (72 RSA/ECDSA vectors from Python's
`cryptography`), `bigint_sweep.py`, `sweep.py` (now with SHA-512/384),
`roots_selfsig.cb`, `tls_server.py` + `tls_client.cb` (a local OpenSSL
TLS 1.3 server: EC and RSA leaves, DNS/wildcard/IP names, wrong name
refused, 100 KB through `read_exact`, close_notify; coby 76 s, cobc
instant), and `stress/dot/smoke.cb`, a DNS-over-TLS query to
Cloudflare, Google, Quad9 and AdGuard (handshakes 46–225 ms compiled).
The DNS message format is left to the owner's flagship program; `std`
gives it everything beneath.

**Tooling found on the way.** A static error inside `std`'s own source
now names its file and line (`at <std>/tls.cb:596`) instead of
"unknown location" (`prelude::std_location`, used by
`resolve_user_location`). A new `std` file must be added to
`src/prelude.rs`'s `STD_FILES`.

**Friction found:** `Result::unwrap_or(arg(0), "default")` needed
`String::from_str` (a type parameter solved as `String` was not a
literal position); met in three test programs, fixed by D-0162 below.

### Literals through a type parameter (D-0162, 2026-10-04)

`CHG-0190`: `Result::unwrap_or(arg(0), "default")`, `Vec::push(&mut v,
"x")` for a `Vec<String>`, `HashMap::insert(&mut m, "k", 1)` and a
program's own generic function now take a text literal where the type
parameter is fixed as `String`. The checker defers a text literal
argument as D-0079 defers a numeric one; when its parameter comes out
`String` the literal's address is recorded and accepted; `lib.rs` then
rewrites those nodes (`views::rewrite_literals_as_string`, with the
items dropped so each `Arc` is unique and addresses hold), rebuilds and
checks again, so both tools see `String::from_str(L)`. A literal a
generic function's instantiations type differently stays a type error.
Case `str_literal_generic_string_ok.cb`.

### Showcase Tier 11 — secured (2026-10-04)

Five programs over the crypto roadmap, deterministic (fixed keys, fixed
dates): `hashsum` (a stdin checksum tool, `hash(kind, …)`, hex/base64,
`exit(2)` on a bad option), `webhook` (HMAC signatures checked with
`digest_eq`), `sealed_box` (X25519 + HKDF + ChaCha20-Poly1305; the first
sealed message checked against Python's `cryptography`), `bignum`
(`BigUint`: 52!, F(300), Fermat tests to 2^521 - 1, textbook RSA; all
checked against Python), `certcheck` (the test PKI embedded as PEM).
Identical under coby, cobc/gcc and cobc/clang. Frictions noted in the
showcase README: no public way to take a `StringView`'s bytes as a slice;
no `String == str`.

### `StringView::as_bytes` and `String == str` (D-0163, D-0164, 2026-10-04)

`CHG-0191`: `StringView::as_bytes(v) : slice<u8, shared>`, the view's
bytes without a copy, for everything that takes a byte slice (case
`stringview_as_bytes_ok.cb`, checked against Python's `hmac`).
`CHG-0192`: `==`/`!=` between a `String` and a `str` or a `StringView`,
either side, as `String::eq_str`/`String::eq_view` (new). The checker
records each such comparison in a rewrite table (D-0162's mechanism,
generalized: `typecheck::Rewrite`); `views::rewrite_literals_as_string`
turns it into the call, and the program is checked again. Orderings stay
within one text type, with a message naming D-0164. Cases
`text_mixed_eq_ok.cb`, `text_mixed_order_rejected.cb`. Both found by
showcase Tier 11.

### cobfmt breaks lines at 120 by default (2026-10-04)

At the owner's request the default width is 120 (it was "never"), with
`--no-wrap` for those who want lines left alone and `--width N` for
another width. Turning it on exposed two faults in the line breaker,
both fixed: it split a parameter list at the comma inside a type's
arguments (`slice<u8,` / `shared> salt`), and a broken line could leave
text the style formats differently, so a second run changed the file
again. Commas inside an `ident<…>` are no longer item separators, and
breaking and formatting now repeat to a fixed point. New tests:
`the_default_width_is_120_and_no_wrap_breaks_nothing`,
`breaking_keeps_type_arguments_together`. `std`, the conformance cases,
the showcases and the guide's examples were reformatted (the wrong
splits from the first run rejoined by `stress/width120/unbreak.py`
first); the spec's `std` listings follow `std`.

### Pushed as Update 61 without the final battery (2026-10-04)

The owner asked for the push without the battery. The full battery ran
once after the cobfmt change: release workspace tests, the guide, all 76
showcases, the stress and crypto programs, the TLS tests and the
sanitizer runs passed; the debug workspace run failed only because three
heavy crypto cases ran past the 600 s CPU limit under the debug
interpreter. Those cases were split (each now 282–400 s under debug
coby); the rerun that would confirm the split in both profiles was
stopped at the owner's request. A battery is owed.

### Round 7: real-world programs over the new std (2026-10-05)

`stress/round7/` (NOTES.md): JWT, a password vault, TOTP, a BigUint
calculator, a certificate dump of the system roots, a DNS-over-TLS `dig`
and an HTTPS GET, each checked against Python or the RFCs under coby and
both cobc builds. No fault in `std` or either tool was found. The HTTPS
tests showed the TLS client's one gap in practice: 4 of 40 popular sites
refuse TLS_CHACHA20_POLY1305_SHA256 with X25519 (eBay offers only AES-GCM).
`TlsError::text` now names the common alerts (alert 40 says which suite
and group are missing; `spec/21` 4.29.1).

### D-0165: AES-GCM, P-256 key agreement, the TLS client's mandatory set (2026-10-05)

`std::crypto` gains `aes_encrypt_block`, `aes_gcm_seal` and `aes_gcm_open`
(16-, 24- and 32-byte keys), and `p256_private_key`, `p256_public_key`
and `p256_ecdh`. The AES S-box is computed (x^254 by masked
multiplications, eight bytes at a time) and GHASH multiplies by masks.
The P-256 scalar is made 257 bits long and multiplied by double-and-add-
always with a masked choice. The P-256/P-384 field arithmetic, shared
with `ecdsa_verify`, now reduces by masks.

`std::tls` offers TLS_CHACHA20_POLY1305_SHA256 and TLS_AES_128_GCM_SHA256,
X25519 and P-256, and answers a HelloRetryRequest for P-256 (the
transcript restarting from message_hash). Results:
- **eBay** (AES-GCM only), **Facebook**, and **office.com** (P-256 by
  HelloRetryRequest, AES-128-GCM) now connect.
- **A local openssl `s_server`** completed every suite and group pair;
  AES-256-GCM-only and P-384-only servers are refused with the named
  alert.

Checks:
- FIPS 197 C.1–C.3, the GCM spec's cases 1/2/4/16, 24 AES-GCM vectors
  from Python and 12 P-256 exchanges from Python matched under cobc
  (gcc, clang) and coby.
- RFC 5903 §8.1 matched.
- 72 RSA/ECDSA vectors and the ECDSA, RSA, x509 and TLS cases were
  unchanged.

Rapid push (owner): quick tests only; a battery is owed.

### std::crypto completed (2026-10-05, D-0166–D-0172, `CHG-0194`–`CHG-0200`)

`std::crypto` gains HMAC and HKDF over any `HashKind`, Ed25519, curve-generic
EC keys with P-384 key agreement and RFC 6979 ECDSA signing, RSA private
keys with constant-time Montgomery signing (PKCS #1 v1.5 and PSS) and key
generation, BLAKE2b, PBKDF2, Argon2id and PHC-string password hashing;
`std::math` gains `BigUint::clone` and `mod_inverse`; `std::x509` reads and
writes keys as SubjectPublicKeyInfo and PKCS #8 (DER and PEM, PKCS #1 and
SEC 1 read too), signs with `sign`, and accepts Ed25519 certificates;
`std::tls` offers TLS_AES_256_GCM_SHA384, P-384 and Ed25519.

Checks: RFC 4231, 5869, 6979, 7693, 7914, 8032 and 5903 vectors; Python's
`cryptography`, `hashlib` and `argon2-cffi` and openssl as oracles for random
sweeps, key formats and signatures, under coby and cobc (gcc, clang); eight
local `openssl s_server` configurations (every suite, P-384 by
HelloRetryRequest, an all-Ed25519 chain, an RSA leaf).

**DNS over TLS under the interpreter.** The smoke test (`stress/dot`) got
`closed` from public resolvers under coby with this and the previous
release alike; compiled, every resolver answers in 50–200 ms. Measured:
Google's and Quad9's resolvers close a handshake that stalls for about 10 s,
Cloudflare's an idle connection within 15 s, and coby needed about 75 s of
handshake work, almost all of it two P-384 ECDSA verifications of the
chain at ~30 s each. Two changes, neither normative:

- **ECDSA verification** (public values, so variable time is allowed):
  small multiples by doubling instead of repeated addition, the
  double-scalar multiplication over 4-bit windows, and the final
  comparison made as X = r·Z² instead of inverting Z. P-384 verification
  under coby went from 30 s to 17 s, and Cloudflare's resolver now answers
  under coby (49 s handshake).
- **The interpreter's name tables** (variables, items, types) use a fixed
  multiplicative hash (src/fxhash.rs) instead of SipHash, which was a fifth
  of coby's time on arithmetic loops: about 20 % faster. Iteration order was
  already unspecified (SipHash is randomly keyed).

Google's and Quad9's 10 s limit is beyond the interpreter for any real
certificate chain; the guide's TLS section says to run TLS clients
compiled.

### `std::http` (2026-10-05, D-0173, D-0174, `CHG-0201`, `CHG-0202`)

An HTTP/1.1 client (`Url`, `Header`, `Request`, `Response`, `HttpClient`,
`http_get`, `http_post`: redirects, chunked and unlengthed bodies, interim
responses, limits, timeouts, https through `std::tls` with the system
roots loaded at the first https request) and a server (`HttpServer`,
`HttpConnection::request`/`respond`: keep-alive as negotiated, `Expect:
100-continue`, HEAD, 413 and 400 for what it refuses), written in CobaltC
over `std::net` and `std::tls` (impl/std/http.cb, spec/21 §2p).

Checks (stress/d0173): a loopback client and server in one program under
both tools; the client against Python's `http.server` (chunked with
extensions and trailers, a 1 MB body, a 1.0 body until close, 301/302/
303/307/308 with relative and absolute Locations, a redirect loop, both
lengths at once, HEAD, a 1500 ms timeout to a black-hole address); the
server against curl (keep-alive with two requests on one connection, HEAD,
a 3000-byte POST, a chunked upload, 204, 404, a 6000-byte body over the
limit answered 413) and a raw socket (400 for a line that is not HTTP);
compiled, five public sites (example.com over http and https, google.com's
84 KB chunked page, cloudflare.com HEAD, a 600 KB Wikipedia page) and DNS
over HTTPS by POST to 1.1.1.1 and by GET to dns.google. Found on the way:
`HttpClient::new` first loaded the system roots eagerly, a minute under
the interpreter per client, so `http_get` in a loop looked like a hang;
the roots now load at the first https request.

### The object cache, and a fault that waited (2026-10-05)

**A fault with a blocked thread hung both tools.** A run-time fault in the
main thread while another thread sat in `accept` (or `sleep`) never
reported: the fault's unwind destroyed the thread's `handle`, and
`[Handle-Destructor]` waited for a thread that `[Fault-Unwind]` says takes
no further steps. spec/18 1.5.3 and spec/19 1.8.1 clarify that a handle
destroyed by the unwind does not wait; coby (a `faulting` flag set as the
fault propagates) and cbrt (`unwinding`) skip the wait;
`conf.fault-with-live-thread` covers it. Found by a Tier 12 showcase whose
client faulted while its server thread waited.

**`cobc` object cache** (`COBC_CACHE`, cobc/src/cache.rs). A crypto- or
HTTP-heavy program took about 19 s to compile: gcc on 58 000 lines of
generated C, 94 % of it `std` code identical from program to program. The
lowering now makes that identity exact -- temporaries numbered per
function, string literals named by content, type ids a hash of the type's
mangled name (the runtime looks descriptors up by id, `cb_init` taking
the ids beside the table), and functions with external linkage -- and
emits the C in units, one per function, each marked pure `std` when it is
a `std` function instantiated at `std` types alone. Pure units are
compiled in translation units of their own (the runtime header, the
declarations the unit refers to closed over struct fields, the unit) and
their objects kept under a key that hashes the unit, those declarations,
the C compiler's identity, the options, the runtime stamp and the cobc
executable; the rest of the program is one unit; everything is linked
through a response file. Measured on token_gate.cb: 19 s without the
cache, 29 s cold with it (493 units, four at a time), about 3.5 s warm;
`hello.cb` 0.7 s either way. The warm cost is the lowering (0.6 s),
hashing and the program unit (about 2 s) and the link (0.6 s). A rebuilt
`cobc` or an edited `std` function invalidates exactly the units whose
text or declarations changed. `COBC_CACHE=off` is the old single-file
path, kept as the fallback and for `--keep-c`.

### `while (true)` without a `break` is `never` (2026-10-05, D-0175, `CHG-0203`)

The shared static pass gives a `while (true)` loop (the literal) that no
`break` leaves the type `never` and marks what follows unreachable;
`cobc`'s lowering tracks breaks per loop and lowers such a loop as never
completing (no value return after it). Fourteen dead values after such
loops were removed from `std` (crypto, http, io, sync, sys, std.cb) and
from spec/21's listings. Cases `while_true_never_ok`,
`while_true_break_needs_value_rejected`,
`while_condition_true_variable_rejected` (spec/12) pass under coby and
cobc with gcc and clang; the Tier 10–12 showcases that use the edited
`std` functions are unchanged.

### `std::fs` and `std::env` (2026-10-05, D-0176, `CHG-0204`)

`std::sys` is gone: files and paths (with `File`, `read_file`,
`write_file`, `read_bytes`, `write_bytes` and `FileError`, from `std::io`)
are in `impl/std/fs.cb`, the arguments and environment in
`impl/std/env.cb`, the three clocks in `std::time`; `std::io` keeps the
standard streams. The private helpers both new modules use
(`fs_answer`, `path_text`, `path_result`) moved to `std.cb`'s root. No item
changed; the tools name `std` items by their `std::` paths, so neither
needed a change beyond `prelude.rs`'s file list. Master Instructions §9
now states the placement rule (no catch-all submodule).

### `coby --check` and `cobc --check` (2026-10-05)

The static pass alone (loader, parser, resolution, every static check)
on each file named and each `.cb` file in each directory named; nothing
runs, no C is written. `src/check.rs` is shared: `cobc` adds the check
that each `extern "./…";` file exists (`link::resolve`), and `cobfmt`
now uses its `collect`. In a directory, a file another checked program
loads as a module (`Analysis::loaded`, from the loader) is checked as
part of that program, not alone. Exit 0 / 1 (rejected) / 2 (missing or
unreadable path, missing `extern` file). The first run over the
repository found `showcase/tier11/certcheck.cb` rejected (its `match`es
lacked the `Ed25519Key` and `Ed25519` arms D-0171/D-0172 added) and
`crypto_aes_gcm_ok.cb` not formatted; both fixed. Tests in
`tests/cli.rs` and `cobc/tests/driver.rs`.

### `read_le`/`read_be` and `crc32` placed (2026-10-05, D-0177, `CHG-0205`)

`read_le` and `read_be` moved from `std::text` to `std::collections`,
beside `Vec::push_le`/`push_be`; `crc32` to `std::crypto`, marked as not
cryptographic. No body changed. Cases `std_bytes_subjects_ok`,
`std_text_has_no_bytes_rejected`.

### Standard output is buffered (2026-10-06, D-0178, `CHG-0206`)

`src/outbuf.rs`, shared by `coby` and `cbrt`, holds every byte written to
standard output (`printf`, `print`, `stdout_write`) in an 8 KiB buffer,
written out when it fills, at each newline when standard output is a
terminal, and before any write to standard error, any read of standard
input, `Command::status` (whose child shares standard output), a fault's
report and the program's end (`cb_terminate_ok`, `cb_exit`, `fault`,
`coby`'s `run_items_raw`). An atomic flag makes the flush before each
read free when nothing is buffered. `flush_stdout()` in `std::io` is
written over the private extern `stdout_flush` (`cb_flush_out`). 2,000,000
`printf` lines to a file: 7.55 s before, 0.77 s after. Cases
`stdout_buffer_order_ok` (both streams to one file, checked by
`tests/print_output.rs` and `cobc/tests/output_order.rs`, which also
checks a fault's report), `flush_stdout_ok`, `stdout_buffer_boundary_ok`,
`stdout_buffer_child_ok`; row `conf.stdout-flush-private`.

### Compiler performance round (2026-10-06)

The gaps the measurement round of 2026-10-05 found
(`stress/perfguide/FINDINGS.md`), closed in `cobc` without a language
change; `coby` is unchanged but for `read_line`'s primitive.

- **Element field writes are confined.** `v[i].f… op= e` (the scan's
  `Assign` of a field chain on `x[i]`), `foreach (p in &mut v) { p.f = e; }`
  (`only_reads_w` accepts `x.f… = e`), and a named
  `ref<T, m> p = &m v[i];` whose block never names `v` again
  (`elem_ref_let`, `Fx::bind_elem_ref`: the element's address as a direct
  reference). A `foreach` holder is confined before the loop's condition
  (`confine_holder`), and over a field (`&h.v`) as over a local. The scan's
  `pushes_onto` now also counts `pop` and `reserve`: an address taken
  before the value is computed does not survive either.
- **More vectors reach the confined variants.** A shared borrow of any
  `Vec` place formed at the call goes to a shared confining parameter
  (`fresh_shared_vec_arg`); a confined local may be moved into a call as
  its last use (`overwrite(&mut h.v, v);`) and into the function's
  result through calls and struct literals (`tail_moves`,
  `Ok(String { .bytes = b })`); a `Vec` parameter taken by value is
  confined as a local is (`bind_param` → `bind_confined`).
- **Locals passed to single-name callees are not pinned**
  (`pure_direct_ref_arg` with the function's bound names), so a plain
  struct passed as `f(&big)` keeps its unchecked accesses.
- **Pure views.** A `StringView` parameter the body reads only through
  `v.bytes[i]`/`slice_len(v.bytes)` or hands to another pure view
  parameter, in a function whose result is plain (`Gen::pure_view`), is
  neither sent nor received; its byte reads are bounds-checked only. A
  temporary view passed there ends right after the call (`cb_absorb`), as
  the callee would have ended it: the first version left it alive to the
  end of the statement, and an `if` condition's view clashed with the
  body's write (the `std::http` redirect cases). `&s[0..$]` and
  `String::as_view` are made in place (one range borrow, no call).
  `used_as_value` matches `Type::f` by both names, so a local called `len`
  no longer marks `StringView::len` as a `fn` value.
- **Native `Vec::remove`** (plain elements, one `memmove`) and
  **`Vec::index_of`** (integer and `bool` elements; `contains` calls it).
- **`read_line`** reads through a private extern `stdin_read_line`
  (`fileio::read_line_chunk`, both tools): a line at a time from the
  buffered `stdin` instead of a byte at a time.

Measured with `cobc -O2` (see `stress/perfguide/results.txt`): element
field updates 5.4–6.9 s → 0.02 s; a struct's `Vec` passed to a helper or
moved out and back ~5 s → 0.75 s; `ref<Big, shared>` 2.5 s → 0.34 s
(faster than by value); passing a view 1.8 s → ~0, making one per call
6.2 s → 1.0 s; `read_all` + `split` 18.2 s → 2.5 s, `read_line` 1.5 s →
0.94 s; `Vec::remove(&mut v, 0)` over 10,000 elements 50 s → 0.02 s (still
quadratic: the data moved is the same, by `memmove` instead of checked
swaps); `Vec::contains` 8.2 s → 0.08 s. Indexing a `Vec` through a struct
field inside a loop (`h.v[i]`) is still tracked. Differential programs in
`stress/perfround/diff/` agree under `coby`, GCC and Clang.

### Two divergences closed: argument reads and a buffer moved under a write (2026-10-06)

- **coby read arguments late.** `f(x, { x = 5; 2 })` passed 5: a place
  argument stayed a place until its parameter was bound, after the later
  arguments ran, against `[Call]`'s left-to-right order and
  `[LValue-To-RValue]` (cobc passed 1). The same for closure calls and
  struct literal fields. `Interp::arg_now` now copies every non-resource
  place argument (but a bare reference, slice or `fn` value, which is
  handed on as it is) into a typed temporary of the statement as it is
  evaluated; `store` adopts it as it adopts a call's result. Case
  `eval_order_reads_ok`, row `conf.eval-order-reads`.
- **cobc kept an element's address across a call that moved the
  buffer.** `v[i] = e`, `v[i].f = e` and `v[i].f op= e` on a confined `v`
  take the element's address before `e` runs; the scan's `pushes_onto`
  counted only a direct `push`/`pop`/`reserve` in `e`, so
  `v[0].x = grow(&mut v)` wrote into the freed buffer (coby:
  `diag.stale-binding`). Now any argument handing `v` on, other than `&v`
  or to `Vec::len`/`Vec::index_shared`, counts, and `v` is checked. Case
  `elem_write_rhs_grows_vec_rejected`, row `conf.elem-write-rhs-grows`.
- The differential generator (`cobc/tests/differential.rs`) now writes a
  `Vec` of structs field by field (indexed, compound, `foreach &mut`, a
  named element reference, a growing right-hand side), arguments a later
  argument writes, `StringView` helpers, `Vec::remove`/`contains` and an
  owned `Vec` parameter; 800 programs agree, and 14 of 40 sampled differed
  on the tools before these fixes.

### Three more divergences closed, and `cobc --report-tracking` (2026-10-06)

- **coby let a destructor's fault replace the fault being unwound.**
  `[Fault-Unwind]` reports the fault that started the unwind; coby
  reported a destructor's own fault raised during it and went on
  destroying. `destroy_at_scope_end` now keeps the first fault
  (`fault_pending`) and destroys nothing more (`terminated`), as cbrt
  does; during `exit`'s unwind a destructor's fault is the outcome and
  destruction stops there too. Case
  `fault_unwind_destructor_fault_first_wins`, row
  `conf.fault-unwind-first-fault`.
- **coby checked `e[k]`'s borrow at the field reached, not the element.**
  `[Index-Vec]` checks it as an access to element k; with `&mut p[1].y`
  live, coby let `p[1].x` be read (cobc faulted). The element object is
  now checked whole. Found by the 5,000-program differential campaign
  (3 programs, all this; seeds 7301447, 7302608, 7304321). Case
  `index_element_borrow_same_element_rejected`, row
  `conf.index-borrow-whole-element`.
- **D-0179 (`CHG-0207`, `spec/15` 1.12.1):** "pending" (D-0107) covers a
  borrow an argument forms, not a path held by a value it computes;
  `f(String::as_view(&s), &mut s)` faults when `&mut s` is formed, as both
  implementations already did. Case `view_argument_and_mut_borrow_rejected`,
  row `conf.view-arg-not-pending`.
- **`cobc --report-tracking FILE.cb`** (`cobc/src/report.rs`): compiles
  nothing, reads the generated C back, and lists the program's lines whose
  C still calls the runtime's tracking (`cb_borrow*`, `cb_read`,
  `cb_write`, `cb_elem_*`, object and transfer calls), loops first, with
  the kinds of call. A tool option; no language change. Test in
  `cobc/tests/driver.rs`; the guide's chapter P points to it.

### A last-use argument unconfined the vector beside it (2026-10-07)

`bench/sieve_fn.cb` ran 5–7 times slower than `sieve.cb`: `mark` ran as
`f_mark`, not `f_mark_c1`, and `main`'s vector was tracked throughout
(`private/FINDINGS-2026-10-07.md`, §1). The cause was the rule of
2026-10-06 that lets a local be moved into a call as its last use in the
function body (`Scan::block`): when any argument was such a bare name
(`limit` in `mark(&mut is_prime, limit);`), the rule scanned the call's
other arguments itself, as plain places, and so skipped the call rules:
a confined vector passed to a confining parameter, or pushed onto
(`Vec::push(&mut v, x);` with `x`'s last use), was ruled out. The call
rules are now one function (`Scan::call`) that both paths use, the
moved arguments skipped. Output was correct throughout; only checks
were added. Test `last_use_argument_beside_a_confined_vector` in
`cobc/tests/tracking.rs`; case
`21-standard-library-semantics/vec_confined_variable_argument_ok.cb`;
the differential tester's `fill_twice` and `sum_from` are now also
called with a plain local declared just before the call.

### Inlining restored under the object cache (2026-10-07)

The same benchmark run found `cobc`'s output executing more instructions
than on 2026-10-03 (`private/FINDINGS-2026-10-07.md`, §2: `records`
+30%/+86%, `sieve` +37%/+21%, `bench` +34%/+13%, `tree` +14%/+11%, under
gcc/clang), and ascribed it to the newer C compilers. With the compilers
of 2026-10-03 (gcc 11.4, clang 14) the increases reproduce, and the
Update 63 `cobc` (before the object cache) matches the 2026-10-03 counts
exactly: the cause was the object cache (cobc/src/cache.rs), in three
ways.

- **Every function had external linkage**, the program's own included,
  even with `COBC_CACHE=off`, so a function called once or small (`step`
  in `records.cb`) was no longer inlined. `Gen::localize` makes a
  function `static` again when it is in the program's own translation
  unit and named by no cacheable unit (not `main`, as before).
- **Small `std` helpers were in objects of their own**, so `Vec::push`,
  `Vec::len`, the element accessors, `Rng::next_u64` and
  `HashMap::entry`'s accessor were calls in the program's loops. The
  program's translation unit now also gets an inline-only copy of each
  cached function of at most 50 lines (`INLINE_LINES`) that its code
  calls, and of those such a copy calls (`inline_copies`): the
  definition made `extern inline __attribute__((gnu_inline))`, which the
  C compiler may inline but never emits; a call it does not inline goes
  to the cached object. Not in the single-file C (`COBC_CACHE=off`,
  `--keep-c`), which has the definitions themselves.
- **Type ids became hashes**, so the runtime's descriptor lookups became
  probes of `TypeMap` (`tree` +12% after the two fixes above, almost all
  in `drop_in_place_tail`). The map's slots now hold the id and position
  together, read unchecked; `drop_in_place_tail` looks its type up once
  (`Rt::typ_pos`) rather than once more after the destructor and once per
  owning field.

User-space instructions, today's `cobc` with gcc 11.4 / clang 14 against
2026-10-03's: `bench`, `sieve`, `sieve_fn`, `collatz`,
`collatz_wrapping`, `records` within 0.0%; `strings` +0.0%/+0.5%;
`hashcount` −0.3%/+1.9%; `tree` +2.7%/+3.3% (the hashed lookups left on
the drop path). The cost is in compile time: `token_gate.cb` with a warm
cache took 3.15 s, now 4.65 s (`static` alone: 3.26 s; a 25-line limit:
3.85 s, but `tree` and `hashcount` then stay 6–8% up); a cold build is
unchanged (about 18 s). Checked with the strict compiler wrappers
(`stress/strictcc`). Test `program_functions_are_static_and_cached_helpers_link`
in `cobc/tests/driver.rs`.

### The `std` completeness round: D-0181 to D-0186 (2026-10-07)

The assessment of 2026-10-07 (`private/std-completeness-2026-10-07.md`)
read every export of `std` against what a deployed program meets; the
owner had all of it implemented at once, as six decisions
(`CHG-0209`..`CHG-0214`, `spec/21` 4.44.0, `spec/conformance.md`
3.174.0).

- **D-0181, the small holes**, one or two functions in thirteen
  modules: `strip_prefix`/`strip_suffix`, `lines`, `split_whitespace`,
  `code_point`/`push_code_point`, `parse<bool>`; `Vec::retain`,
  `dedup`, floats in `Vec::sort` (a NaN last); `clamp`, `is_nan`,
  `is_finite`, `gcd`, `asin`/`acos`/`atan`; `Rng::shuffle`, `uuid_v4`;
  `File::create_new`/`flush`/`position`; `current_exe`, `hostname`,
  `process_id`, `Child::terminate`, `read_error_line`;
  `iso_from_unix_ms`/`unix_ms_from_iso`, HTTP dates; keepalive,
  connected UDP; `Sha1` and `HashKind::Sha1`; `TlsStream::read_line`,
  `peer_certificate`; `Param`/`form_decode`/`form_encode`;
  `PgRow::get_i64` and friends. Native: `src/fileio.rs` (`create_new`,
  `flush`, `position`, `current_exe`, `hostname` by `gethostname` /
  `GetComputerNameExW`), `src/procio.rs` (`process_id`, `terminate` by
  SIGTERM, a standard-error read-ahead), `src/netio.rs` (`SO_KEEPALIVE`
  by the system's `setsockopt`, UDP connect/send/recv), `src/numtext.rs`
  (`parse_bool`, with `cb_parse_bool` in `cbrt`), the float functions in
  `typecheck`/`interp`/`lower`, and `key_less` on floats in both tools
  (`is_sort_type`).
- **D-0182 `std::json`** (`std/json.cb`, 630 lines): a `Json` tree,
  `parse`/`text`/`pretty`, `get`/`at`/`as_*`, `set`/`push`.
- **D-0183 `std::compress`** (`std/compress.cb`, 1 000 lines): DEFLATE
  both ways (LZ77 with hash chains, stored/fixed/dynamic blocks chosen
  by size), zlib, gzip, Adler-32; the HTTP client offers and decodes
  `gzip`/`deflate`. Interoperable with CPython's `zlib`/`gzip` both ways
  (`stress/std2/comp1.cb`).
- **D-0184 `Args`** in `std::env`: declare-while-reading options with
  generated help.
- **D-0185**: `Channel::try_recv`, `recv_timeout_ms` (a poll, no new
  primitive), `cpu_count` (`available_parallelism` through `proc_op`),
  `TcpStream::try_clone` (`dup`).
- **D-0186 TLS server**: `TlsStream::server(tcp, key, chain)` mirrors
  the client's handshake (the client's first suite, group and the key's
  scheme; HelloRetryRequest when it sent no usable share), and
  `HttpServer::bind_tls`; `HttpServer::accept` now gives `HttpError`.
  Checked against `openssl s_client` (every certificate kind under every
  suite, P-256/P-384 shares, an HRR via `-groups X448:X25519`) with
  `cobc`, and hermetically with our own client in the suite.

Seventeen new file cases (49 rows). Under `coby` the two TLS loopback
cases take about a minute each: a handshake is some fifty seconds of
interpreted public-key arithmetic for the two sides together (one
`ed25519_sign` or `verify` is 3 s, `verify_chain` 3.3 s, AES-GCM of a
KiB 5.6 s), well inside the harness's 600 s. `cargo check` for
`x86_64-pc-windows-msvc` passes; the full battery is owed.

**A divergence found on the way, and fixed (`stress/std2/div1.cb`):**
`match (r0) { Ok(r) : printf("%v\n", Option::is_some(&String::strip_prefix(&r, "x"))), … }`
was `diag.destroy-while-aliased` under `coby` and printed `true` under
`cobc`. The spec's `[Match]` runs an arm body as
`block'(f_arm, stmt-scoped(e_i, keep))`: the body is a statement of its
own, so a temporary made in it (here an `Option<StringView>` borrowing
`r`) ends at the arm expression's exit, before the arm's frame ends and
destroys `r`. `cobc` was right. The interpreter gave a block's trailing
expression that scope (`exec_block_body_expected`) but not an expression
arm's body, whose temporaries fell into the enclosing statement's scope
and outlived the binding; `eval_stmt_scoped` now serves both, at the
three arm-evaluation sites. A block arm (`Ok(r) : { … }`), a temporary
holding no reference (`Option<usize>`), `if (Some(r) = o)` and `foreach`
were never affected. Case
`conf.match-arm-temporary-borrowing-binder`
(`16-aggregates/match_arm_temporary_borrowing_binder_ok.cb`).

### The static pass checks `std` as a program reaches it (2026-10-08)

Every program start parsed, resolved and type-checked the whole prelude
(`src/prelude.rs` embeds all of `std`), 0.85 s of CPU for an empty
`fn main() {}` after the 2026-10-07 additions, and growing with `std`.
Parsing is 0.14 s of that; checking the bodies, 0.7 s. The owner chose to
check only what a program reaches ("std will only get larger over
time"), over caching the checked prelude per process (which helps only
in-process test runners) or precompiling it at build time.

- **`typecheck::check_program`** seeds its worklist with the program's
  own items (every non-generic function as itself, every generic body
  once with opaque type parameters, as before) and no longer with the
  prelude's. A `std` item joins the worklist when a checked body calls
  it or names it as a value (`[T-Item]` now records the function it
  types). A reached `std` generic's opaque body is checked before its
  first instantiation, as the seeding order used to guarantee.
  `COBALTC_CHECK_ALL=1` restores checking every item; `tests/cli.rs`
  `std_is_well_typed_as_a_whole` runs an empty program that way, so
  `std` itself is still verified whole by every suite run.
- **`Items::checked_fns`** (by key and type arguments) and
  **`Items::checked_bodies`** (by the declaration's address) record what
  the pass checked; **`Items::fn_keys`** maps a declaration back to its
  key. The evaluators call some `std` functions by name without a call
  in the program's text -- destructors, `printf`'s helpers, `Vec`'s
  growth, about ninety names in `cobc`'s lowering alone -- so each tool
  asks **`typecheck::check_more`** for any body not yet checked before
  using it: `coby` at its four function entries
  (`Interp::ensure_checked`, one set lookup for a checked body), `cobc`
  in `lower_fn`. What such a check records joins the item table; its
  D-0162 literal records are dropped (the text is final) and its
  `static_assert`s are not evaluated (the whole-`std` run evaluates them).
- An earlier shape, reaching every type a body met and pushing its
  `drop` and `clone`, was wrong: `Vec<Tracked>::clone` fails its bound
  legitimately when `Tracked` is not `clone`, and the program never
  calls it. On-demand checking needs no such guess.

Measured: an empty program 0.85 s → 0.20 s under `coby`; `cobc --run`
of hello-with-`std` 1.26 s → 0.59 s (half of it the C compiler). The
corpus runners' per-case time roughly halved. No language or `std`
surface changed, so there is no D or CHG record; `rule.fn.program`'s
"every item is well-typed" holds of `std` through the whole-`std` test
and of the program through the seeding.

### Nested `std` submodules and `std::extensions`: D-0187 (2026-10-08)

The owner's request of 2026-10-08, decided on the proposal
`private/std-submodules-2-proposal.md` ("proceed with your
recommendations, except make `std::ext` `std::extensions`"); `CHG-0215`,
`spec/21` 4.45.0, `spec/00` 1.2.0, `spec/17` §3, `spec/conformance.md`
3.177.0.

- **`std::crypto`** is four submodules, each re-exported by it:
  `digest` (the hashes, `digest_eq`, `crc32`), `kdf` (HMAC, HKDF, PBKDF2,
  Argon2id, password hashing), `aead` (ChaCha20-Poly1305, AES-GCM), `pk`
  (X25519, P-256/P-384, ECDSA, Ed25519, RSA). `crypto.cb` keeps what
  several of them use privately (`hash_len`, `block_len`, BLAKE2b's `G`
  table); the children reach it through the enclosing module, as `std`'s
  submodules reach the root's primitives. Not `hash`: a submodule's name
  cannot be an item's (`hash` is a function), and the re-export would
  have been `[Item-Duplicate]`.
- **`std::http`** is `client` and `server` over what `http.cb` itself
  declares (errors, URLs, forms, headers, messages, the buffered reader).
- **`std::database`** is `postgres` (every `Pg*` item) over the SCRAM
  client `database.cb` keeps for every database client to share.
- **`std::extensions`** is declared in `std.cb` and not re-exported: the
  implementation-defined area, empty here.
- **Keys** (`modres::qualify`, D-0136): an item anywhere under `std` keeps
  its short key (`std::sha256`), so no message, no case and nothing the
  tools name by string changed; an item under `std::extensions` keeps
  its full path, so an extension's item can never share a key with
  `std`'s. Modules themselves are keyed by their full path
  (`modres::module_key`): a two-deep module under the short rule would
  have been keyed `std::digest`, which is how the first attempt panicked
  in the resolver.
- **The prelude assembler** (`src/prelude.rs`) splices nested
  `export module m "path";` declarations recursively; a path is relative
  to `impl/std/`, which is what the language's rule (relative to the
  declaring file) gives for these files.
- Cases `conf.std-nested-submodule-path`, `conf.std-extensions-empty`.
  Nothing a program could observe changed, so the whole-`std` check, the
  CLI tests and shards of the four runners were the verification; the
  full battery is deferred at the owner's request (further `std` changes
  are coming).

### Dual bodies: reference arguments checked once at entry: D-0188 (2026-10-08)

The owner's decision of 2026-10-08 on `private/direct-params-proposal.md`;
`CHG-0216`, `spec/08` 1.2.0 §4, `spec/19` 1.9.0, `spec/conformance.md`
3.178.0. Ported by hand from the overnight prototype
(`stress/perf_oct7/work`), whose tree predates Update 83: copying it would
have reverted `lower_fn`'s `check_more`.

- **`cobc`** (`lower.rs`): `dual_eligible` / `dual_direct_params` pick a
  function whose reference parameters are two or more, one at least
  exclusive, all a `ref` or slice to plain data (not a `Vec`), each used
  directly and handed to nothing, its other parameters plain values.
  `lower_fn_dual` emits `NAME__fast` (all of them direct), `NAME__slow`
  (as any body) and `NAME`, the dispatcher. `emit_fn_body` is
  `lower_fn`'s old tail with an optional direct-set override.
- **The cap**: a fast body longer than `DUAL_LINES` (600 lines of C) is
  discarded and the function emitted once, checked.
- **`cbrt`**: `cb_paths_disjoint(n, toks, excl)` (valid, over live,
  initialized, not reclaimed objects, not lock-derived, pairwise without
  overlap where one is exclusive, by `check_access`'s projection/range
  overlap); `cb_peek_datum_tok(k, n)` reads a slice argument's token in
  flight and panics unless exactly the `n` slice data are in flight, each
  of one reference.
- **Effect**: a state-plus-input loop over 10 M bytes, 4685 ms → 27 ms.
  Hand-restructured `std` code has few candidates left (`fe_cswap`).
- **Tests**: the four new cases under coby and cobc (gcc, clang), the
  prototype's aliasing and staleness programs (`stress/dual_d0188`), the
  `tracking` test `state_and_input_gets_a_dual_body_within_the_cap`. The
  full battery is deferred at the owner's request.

### AES and GHASH, constant time and faster (2026-10-08)

At the owner's request (2026-10-08, "proceed with G1 and A1"). `std::crypto::aead`
only; no item, rule or result changed (`spec/21` 4.45.1, non-normative).

- **AES, bitsliced**: four blocks (64 bytes) as eight `u64`s, bit j of
  word b being bit b of byte j (`aes_pack`/`aes_unpack`, an 8×8 bit
  transpose per eight bytes). SubBytes is Boyar and Peralta's 113-gate
  circuit (`aes_sbox_ct`), ShiftRows masks and shifts within each block's
  16 bits, MixColumns rotations within each column's four bits and a
  doubling across the words. Round keys are kept bitsliced in `AesKey`;
  the key expansion's SubWord goes through the same circuit
  (`aes_sbox_bytes`). `gcm_ctr` encrypts four counter blocks at a time and
  exclusive-ors into a copy of the input. The x^254 code is gone.
- **GHASH**: `ghash_mul` by BearSSL's `ctmul64` method: 64-bit products of
  bits four apart (`ghash_bmul64`), the high halves from bit-reversed
  operands, Karatsuba, the reduction by shifts.
- **Speed** (1 MiB seal, this VM): AES-128-GCM 449 → 48 ms (gcc), 34.6 ms
  (clang); ChaCha20-Poly1305 is 51 ms.
- **Checked**: the S-box on all 256 bytes and 2,000 random words, GHASH on
  2,000 random pairs, and AES-128/192/256 on 300 random keys against the
  old code; FIPS 197 C.1; the 120 Python `cryptography` cases
  (`stress/aes_ct/gcm.cb`) under coby, gcc and clang;
  `crypto_aes_gcm_ok`, `crypto_aes_key_length_faults`,
  `tls_server_loopback_ok`, `https_loopback_ok`, `tls_refusals_ok` under
  all three. The full battery is deferred at the owner's request.

### Native `Vec::filled`, and the guide on reference parameters (2026-10-08)

At the owner's request (2026-10-08). No rule changed.

- **`cobc`**: `Vec::filled(n, x)` for a plain element type is a native
  body (`native_vec_filled`, beside `native_vec_from_slice`): one
  allocation of exactly `n` elements, filled by `memset` (one-byte types)
  or a plain loop, `out` bound, moved out and sent as the body's is. The
  body read `x` checked and pushed once per element. A size that
  overflows faults `diag.alloc-failure`, the fault the body's growth would
  reach. 8,388,608 bytes 246 → 6 ms; 8,388,608 `u64` 346 → 62 ms (mostly
  the first touch of 64 MB); 1,000,000 structs 56 → 18 ms. Case
  `conf.vec-filled-element-kinds` (`spec/conformance.md` 3.179.0).
- **Guide**, chapter P: "Functions that take references" (`perf-refs`):
  when a function's reference parameters run unchecked (only reference;
  all shared; or exclusive and shared to plain data, D-0188, checked once
  at entry), what "used only directly" means, measurements (10,000,000
  bytes into a struct of two counters: exclusive reference 1, by value
  0.6, through a helper 70, with a `Vec` in the struct 80), and an example
  that passes a struct's plain part alone; a rule of thumb. Programs in
  `stress/guide_refs`. The guide's `--check` (all examples) is left for
  the battery.

### `Vec` group 1: native bodies, cheaper growth and release (2026-10-08)

At the owner's request (2026-10-08), from the `Vec` survey
(`stress/vec_survey/FINDINGS.md`). No rule changed.

- **New native bodies for plain element types** (`lower.rs`): `swap`,
  `reverse`, `insert` (one `memmove`), `truncate` (one release), `clone`
  (one copy), `append` (`b` received and bound, its elements copied, its
  cells released, its destructor left an empty vector), `binary_search`
  (integer and `bool` keys). Each makes the std body's checks on the
  vector (`v.len` read, its bounds faults, the exclusive borrow check its
  element borrows are formed from), in the body's order.
- **Elements with objects.** A held reference to one element is a path on
  that element's own object, which a check on the vector does not see:
  the std bodies meet it when they borrow or read the element. Each native
  that reads or moves existing elements asks `cb_live_in` (new) whether
  any of them has a live object and, if so, calls the prelude's body,
  lowered beside it as `NAME__std` (`std_fallback`, `native_ctx`). The
  first versions of `swap`, `reverse` and `clone` skipped such clashes;
  the fault cases caught it. The same fallback was added to the older
  natives `index_of` (which missed the clash since it was written),
  `remove`, `extend_from` and `from_slice`.
- **Runtime** (`cbrt`): `cb_release_plain` (a release that skips the
  slot scan for cells that never hold a reference: `pop`, `clear`,
  `remove`, `truncate`), `cb_reallocate_plain` (`grow` and `reserve` of
  plain elements by `realloc`), `cb_live_in`, `cb_live_in2`.
- **D-0188 dispatcher**: also requires no live object over a slice
  argument's cells (`spec/08` §4 and D-0188 amended).
- **Speed** (ns per element, `stress/vec_survey`, before → after): swap
  575 → 135 (the rest is the call site's borrows of an unconfined vector),
  reverse 450 → 1.1, insert at the front about 2,000,000 → 1,100 per
  insert (5,000 elements), append 760 → 15, clone 65 → 2.8, truncate 110
  → 0, extend_from 21 → 8, push 16.5 → 9.2, pop 17 → 7.7, binary search
  9,800 → 650 per lookup (the rest is the call site's borrows).
- **Cases** (`spec/conformance.md` 3.180.0): `conf.vec-element-moves`,
  `conf.vec-swap-held-element`, `conf.vec-swap-beside-held-element`,
  `conf.vec-insert-held-element`, `conf.vec-index-of-held-element`,
  `conf.vec-clone-held-element`, `conf.vec-reverse-held-element`; fault
  comparisons in `stress/vec_g1/faults` (`cmp.sh`); 60 showcases spot-run.
- **Found, not fixed (owner decision)**: a read through a slice of a `Vec`
  whose element has a live object held exclusively faults in `coby`
  (`[Read-Conflict]`) and not in `cobc`, even through `cbrt`'s checked
  slice read (the Oct 7 build alike): `check_access` for a slice does not
  consult the element objects in its range. Programs in
  `stress/vec_g1/slice_gap`.

### Slice reads meet element objects; `Vec` group 2: D-0189 (2026-10-08)

Decided by the assistant, the owner having delegated the choices ("I
leave all these matters for you to decide"); `CHG-0217`,
`spec/conformance.md` 3.181.0–3.182.0. No rule changed.

- **The slice-read gap (correctness, fixed first).** An access through a
  slice of a `Vec` whose element had a live object held by another path
  faulted in `coby` and not in `cobc`, through every slice read (the Oct 7
  build alike). Every slice element access now calls `cb_elem_access` (an
  inline test of `cb_reclaimed_entries`, then the element object's check).
  A direct slice parameter computes `cb_live_in` once at entry and checks
  through `cb_elem_access_if` only when it found an object; a D-0188 fast
  half, whose dispatcher has ruled that out, uses a constant 0. The scope
  and location passes treat `cb_elem_access_if` and `cb_live_in` as
  recording nothing, so loops keep no per-iteration bookkeeping. Cases
  `conf.slice-read-beside-held-element`,
  `conf.slice-write-over-held-shared-element`,
  `conf.vec-from-slice-held-element`, `conf.vec-extend-from-held-element`.
- **G2a, `CONFINING_CALLS`**: `swap`, `reverse`, `insert`, `remove`,
  `truncate`, `clear`, `contains`, `index_of`, `binary_search`, `sort` keep a
  local or confining parameter confined (`scan_vec_uses`), called in their
  unchecked forms (`nc_variant`, `lower_confining_call`); native `contains`.
- **G2b, field dual bodies** (`field_dual_eligible`, `field_confined_uses`,
  `lower_fn_field_dual`): a looping function whose one reference is to a
  struct, using its `Vec` fields only as confined vectors are used, runs
  unchecked after `cb_paths_disjoint` and `cb_live_in` at entry. `cb_write`
  now returns for no path (0), as `cb_read` did.
- **G2d (partial)**: `foreach (x in v)` over an exclusive `Vec` reference
  lowered as `$each_at_mut` (521 → 29 ns).
- **G2e**: temporary keys of `contains`, `index_of`, `binary_search`
  (integer, `bool`) passed by address, lowered at the element type (a first
  version lowered `&2` as `i32` for an `i64` vector: gcc and clang disagreed;
  found by the fault comparisons, now `conf.vec-search-keys`).
- **Group 3 where a native form suffices**: native `dedup`; `retain` and
  `position` with a capture-free closure literal (`lower_pred_literal`); the
  `sort_by` literal through a parameter.
- **Results** (ns per element, `stress/vec_survey/results_g2.txt`): field
  write 546 → 1.8, field read 31 → 1.1, field `pop` 280 → 8.3, field `push`
  41 → 9, `swap` everywhere ~135 → 1.8, `binary_search` 650 → 142 a lookup,
  `retain` 1,066 → 3.5, `dedup` 400 → 2.2, `sort_by` through a parameter
  22,100 → 239.
- **Checked**: 42 fault programs against `coby` (gcc, clang,
  `stress/vec_g1/cmp.sh`), every case of the day under all three tools,
  the 70 survey programs and today's tests under ASan+UBSan (clean), cobc
  test targets, 60 showcases, the AES-GCM oracle, TLS/HTTPS cases, Rust
  1.77.2 and Windows builds. Battery not run (owner).
- **Left**: capturing closures and closure values in `retain`, `position`,
  `binary_search_by` (group 3 proper); `foreach` over a parameter (29 ns);
  structs inside a `Vec` take the checked body (`cb_paths_disjoint` refuses
  a reclaimed element path).

### The `Vec` items D-0189 left open: D-0190 (2026-10-08)

At the owner's request ("address the still open issues"); `CHG-0218`,
`spec/conformance.md` 3.183.0. No rule changed.

- **Structs inside a `Vec`**: `cb_paths_disjoint` admits a single path over
  a reclaimed object; the field dual body's struct path is direct in the
  fast half when nothing under it is borrowed and its other fields are plain
  (`field_confined_uses` returns that). A field loop updating a plain field
  over `bags[k]`: 615 → 2.6 ns an element.
- **`foreach` over a `Vec` parameter**: the holder is confined before the
  loop's condition (`foreach_alias` called ahead of it, its output
  discarded): 29 → 1.8 ns.
- **Capturing predicate literals** in `retain`/`position`: the closure value
  built at the argument's place, one exclusive borrow of it for the loop, its
  body compiled with `self` and the parameter direct
  (`pending_pred_closure`): 1,100 → 36 ns.
- Checked: 46 fault programs against `coby` (gcc, clang), every case of the
  day, ASan+UBSan on the new programs (clean), cobc test targets.
- Left: closure *values* in `retain`/`position`/`binary_search_by`, and
  `binary_search_by` itself (minted element borrows; a `cbrt` design).

### Predicates that are functions and `fn` values: D-0191 (2026-10-08)

At the owner's request ("address the still open issue"); `CHG-0219`,
`spec/conformance.md` 3.184.0. No rule changed.

- `lower_pred_literal` takes its predicate from `pred_source`: a closure
  literal (capture-free or capturing, D-0189/D-0190), a named function whose
  reference parameters are pure direct, or a `fn` value in a local, whose
  handle is tested at the call (`cb_fn_pure`, set by `cb_fn_item_pure` when
  a pure function becomes a value): pure handles run the plain loop through
  their thunk, others the prelude's function with the vector borrowed as the
  call borrows it.
- `binary_search_by` specialised the same way; keys for literals and named
  functions passed by address (no path minted).
- `retain` with a named function 1,123 → 3.4 ns, with a `fn` value 1,129 →
  8.5; `binary_search_by` 9,164 → 388 ns a lookup.
- Checked: 50 fault programs against `coby` (gcc, clang), every case of the
  day, ASan+UBSan on the new programs (clean), cobc test targets, 60
  showcases, Rust 1.77.2 and Windows builds.
- Left: a capturing closure stored in a variable (~1.1 µs an element).

### Performance documents re-measured (2026-10-08)

The guide's chapter P re-measured after D-0188 to D-0191
(`stress/perfguide/FINDINGS.md`, "Re-measured 2026-10-08"): the confined,
text, output, collection, arithmetic and reference-parameter figures
updated; advice changed where the measurements did (fill a sized `Vec` with
`Vec::filled`, `reserve` no longer measurable; a sorted `Vec` with
`binary_search` beats `HashSet` for membership; a struct passed by value
costs the same as by reference). §crypto: X25519 about half P-256's time,
the two AEADs equally fast. `bench`: `sieve` and `bench` build their vector
with `Vec::filled` (Rust `vec![true; n]`), `sieve_fn` keeps its pushing
`fill`; `bench/README.md` says why; `bench/RESULTS.md` regenerated by
`run.sh` on this VM.

### Performance close-out: D-0192 (2026-10-09)

At the owner's request ("close of all open items that are performance
related"); `CHG-0220`, `spec/conformance.md` 3.185.0. No rule changed.

- Movable elements (`String`, resources of plain fields): native `swap`,
  `reverse`, `insert`, `remove`, `pop`, `truncate`, `clear` (destructors in
  the prelude's order), `String` searches and `clone`; `_once` forms for
  non-confined local vectors (one borrow check). Search keys pure direct.
- Capturing closure values that only read their arguments marked pure when
  boxed (`cb_fn_mark_pure`): `retain` through one 1,237 → 149 ns an element.
- `Vec::from_fn` with a literal inline: 35 → 3.8 ns an element.
- `cbrt` lock: off once only the main thread runs; reader-writer, with the
  read-only checks shared and `fault_lock` before a report; `cb_allocate`
  unlocked. Two runtime-heavy threads 450 → 200 ms (sequential 188).
- D-0188 bare form for calls with distinct owned locals: 1,544 → 391 ns.
- Fixed: `used_as_value` counted bound locals as function names (no pure
  slice parameter for any function sharing a name with a local): 1,242 →
  145 ns a call. Fixed before commit: the `_once` path lowered an objectless
  key through the generic borrow (an internal error, now with its line).
- Checked: 61 concurrency cases ×3, spawn/join cycling 30/30 per compiler,
  ASan+UBSan, TLS/HTTPS loopback, 307 alias and Vec cases, 295 search cases,
  vec_g1 50/50; then the full battery.
- 12 conformance cases (D-0189 to D-0192) reformatted with `cobfmt`.
- Fixed (found by the battery, from D-0191): a clash in a native predicate
  loop (`retain`, `position`, `binary_search_by`) was reported at the last
  line the predicate's body recorded; `cb_elem_access_slow` now records the
  caller's location first, and the loops pass the call's.
- Left: faster-than-sequential threads (per-thread runtime state), the
  `Option<String>` return protocol (~900 ns a `pop`).

### Round-8 real-world programs: D-0193 (2026-10-09)

Under the owner's standing order to keep writing real-world programs and resolve what they find; `CHG-0221`,
`spec/21` 4.46.0, `spec/conformance.md` 3.186.0. Programs, oracles and results in `stress/round8/NOTES.md`.

- Fixed: `cobc` closure C names collided across functions (now hashed per C function); `coby` lost a captured
  closure's capture types and mis-moved (or leaked) resource-owning closures captured by `move`; `cbrt` shared
  nested closure boxes between copies. Destructors now run once, at the last holder's end, in both tools.
- Natives: `split_whitespace`, `trim*`, `to_ascii_lower`/`upper`, `Drain::take`, `PriorityQueue::push`/`pop`;
  the runtime lock spins before sleeping.
- std: `Vec::swap_remove`, `Response::text`.
- Checked: each program against its oracle under coby, gcc, clang; 99 text/foreach cases, 73 closure cases,
  61 concurrency cases (x2), cycling 30/30 per compiler, ASan+UBSan; coby's suite against headers.

### Owner's items 1-3 after round 8: D-0194, D-0195, D-0196 (2026-10-09)

- D-0194 (`CHG-0222`): quiet types; `=` replaces a live value whose end only frees memory (`[Write-Quiet-Replace]`);
  `coby` now destroys the pending value of a faulting overwrite. Four overwrite cases moved to a `resource` type.
- D-0195 (`CHG-0223`): `ChildInput`/`ChildOutput` (`Child::take_input`, `take_output`); `HttpClient::keep_alive`
  (opt-in; one kept connection, a stale one replaced once).
- D-0196 (`CHG-0224`): objectless `String`s by binding, builders, move-out and `__raw` returns; native `find_str`;
  lazy statement scopes. Single operations 3-6x faster; the round-8 programs unchanged (their cost is values
  inside containers). Found and fixed on the way: `truncate` on an objectless `String` (token 0 into
  `cb_borrow_range`).
- Checked: full parity sweeps (764 case files, coby/gcc/clang) after each cobc change, the guide check, ASan on the
  new paths, Rust 1.77.2 and Windows; then the full battery.

### `std::encoding`: D-0197 (2026-10-09)

- `CHG-0225`, `spec/21` 4.48.0, `spec/conformance.md` 3.190.0: JSON moved from `std::json` to
  `std::encoding::json` (`std/encoding.cb`, `std/encoding/json.cb`), the parent for future data formats. Names, keys
  and `import std;` programs unchanged; `std::json` still resolves as a re-export alias (D-0187 §6).
- Case: `conf.std-encoding-json-paths`.

### Hexadecimal and base64 to `std::encoding`: D-0198 (2026-10-09)

- `CHG-0226`, `spec/21` 4.49.0, `spec/conformance.md` 3.191.0: `to_hex`, `from_hex` moved from `std::text` to
  `std::encoding::hex` (`std/encoding/hex.cb`), `base64_encode`, `base64_decode` to `std::encoding::base64`
  (`std/encoding/base64.cb`). Names, keys and `import std;` programs unchanged; `import std::text;` alone no longer
  names them.
- Cases: `conf.std-encoding-hex-base64-paths`, `conf.std-text-has-no-hex`.

### String performance: D-0199 (2026-10-09)

- `CHG-0227`, `spec/conformance.md` 3.192.0. No rule changed.
- `cobc` natives:
  - `find`/`contains`/`starts_with`/`ends_with` of `String` and `StringView`, and `String::view`;
  - `Option`/`Result` `unwrap` and `expect` for plain or reference values;
  - literal views for pure parameters;
  - fused `from_view(view(…))` and `append(d, view(…))`;
  - fewer objects for values holding references.
- `cbrt`: range borrows, moves, copies and the datum flight allocate nothing; element access checks are per page
  (`cb_reclaimed_pages`).
- csvstat 3.50 → 2.61 s, extsort 11.42 → 5.79 s, wordpar 5.10 → 4.38 s (Python about 0.2 s); `String::find`
  3.1 µs → 0.28 µs.
- Checked: parity coby/gcc/clang on the 68 core cases and the 116 cases using the changed functions, plus 5 new
  cases. The full suites and the battery have not been run. Notes are in `stress/perf_oct9/NOTES.md`.

### Compile-time discharge: D-0200 (2026-10-10)

- `CHG-0228`, `spec/conformance.md` 3.193.0. No rule changed.
- `cobc` proves far more checks at compile time:
  - objectless locals of any container type, and objectless match binders;
  - writer parameters;
  - element references held in locals with no path;
  - confined shared `Vec` readers; quiet container locals;
  - pure read-only closures;
  - `__raw` forms for `std` and native functions, raw sinks and raw copies;
  - checks through token 0 folded before scopes are resolved.
- New natives:
  - `Channel::send`/`recv`;
  - `File::write`/`write_text`, `read_line`, `File::read_line`;
  - `Result::unwrap_or`, `Vec::sorted_order`, `*unwrap(HashMap::get(…))`.
- `cbrt`: `cb_lock_bare`, `cb_unlock_bare`, `cb_unlock_signal`, `cb_drop_plain_buf`.
- Benchmarks (`stress/perf_oct9`):

  | program | before | after | Python |
  |---|---|---|---|
  | csvstat | 3.50 s | 0.25 s | 0.23 s |
  | extsort | 11.42 s | 1.00 s | 0.17 s |
  | wordpar | 5.10 s | 0.54 s | 0.17 s |

- Left for the owner: unbuffered `File` writes (one system call per `write_text`), and `cbrt`'s single lock under
  threads.
- Checked: parity coby/gcc/clang over the whole conformance suite after each step, the guide's examples, a re-run
  of the guide's performance measurements and `bench/run.sh` (no regression), and the full battery
  (`SCOPE=full`, `slow:` cases), green on 2026-10-10. The battery found three compiler bugs, fixed before it went
  green (cases `conf.spawn-in-raw-form`, `conf.binder-moved-into-literal`):
  - a stack overflow on self-containing types;
  - duplicate thread trampolines;
  - probe passes leaving name marks behind.

### `File` writes buffered: D-0201 (2026-10-09)

- `CHG-0229`, `spec/21` 4.50.0, `spec/conformance.md` 3.194.0.
- `[File-Buffer]`: written bytes reach the file by fixed points: the file's next operation, any other file-system
  operation, a child's start, the program's end.
- Implemented once, in `impl/src/fileio.rs`, for both tools. extsort 1.01 s → 0.61 s.
- Checked: the 68 file-related conformance cases under coby/gcc/clang, plus the new
  `conf.file-write-buffer-visible`.

### Per-thread heaps in `cbrt`: D-0202 (2026-10-10)

- `CHG-0230`, `spec/conformance.md` 3.195.0. No rule changed; `coby` is unchanged.
- Each running thread keeps its objects', paths', slots' and reclaimed cells' records in a heap of its own, with
  its own lock. Most runtime calls take only the heaps they work in. Calls that may run a destructor or touch
  reference slots stay exclusive.
- A spawn's argument and a join's result move into the receiving thread's heap. Threads, mutex owners and channel
  counts sit under a small lock of their own.
- Threads now scale. Four threads doing four times one thread's work: 0.84 s → 0.30 s (one thread 0.16 s; four
  separate processes 0.24 s). One-thread programs are unchanged (csvstat 241 ms, extsort 644 ms).
- Checked:
  - with a runtime built with debug assertions (every heap access must be held): the concurrency cases and 58
    threaded programs (showcase, stress rounds 5 to 8, loopback cases) under coby/gcc/clang;
  - the whole conformance suite on the release runtime;
  - the new `conf.heaps-meet`, with 260 threads alive at once.

### Threads, phase 4: D-0202 (2026-10-10)

- `CHG-0231`, `spec/conformance.md` 3.196.0. No rule changed.
- The runtime's remaining stop-every-thread steps are local: frame and scope ends, reference slots, `lock`.
  An entry plans the heaps its work will touch before changing anything. Only destructors of the program, boxes,
  handles and `fn` values still pause other threads.
- Waits spin briefly before sleeping. OS threads are reused by later `spawn`s (at most 64 wait).
- `cobc` lowers a lock block that only reads and writes through its guard without a guard object (`cb_lock_bare`).
- Four threads on one mutex, 200,000 locks each: Update 90 10.3 s → 0.6 s (one thread 0.41 → 0.09 s). Four threads
  storing references in structs: 9.2 → 3.0 s (one thread 0.85 → 1.7 s: planning costs there).
- Checked: the threaded programs and concurrency cases on a runtime with debug assertions; the conformance suite
  on release.

### The concurrency guarantee: D-0203 (2026-10-10)

- `CHG-0232`, `spec/19` 1.10.0 (§4 `rule.conc.guarantee`), `spec/03` 1.4.0, `spec/conformance.md` 3.197.0.
- States what threads can rely on: interleaving of whole steps, no data races (the later access of a conflicting
  pair is a diagnostic, whatever the timing), nothing undefined. Also what an implementation may leave out.
- Writing it found a `coby` bug: a spawn's reference arguments were held only once the new thread first ran.
  They are now held from the spawn, as `[Spawn]` says. `conf.conc-race-detected`, `conf.conc-disjoint-elements`.

### Supervised tasks: D-0204 (2026-10-10)

- `CHG-0233`; `spec/18` 1.6.0, `spec/19` 1.11.0, `spec/21` 4.51.0, `spec/03` 1.5.0, diagnostics 1.50.0,
  `spec/conformance.md` 3.198.0.
- A spawned thread's fault is contained to it (unless it was lent an exclusive reference). Its mutexes are
  poisoned, its threads cancelled, and `join` or its handle's end raise the failure again. `try_join` and
  `is_finished` (std::sync) let a supervisor take failures as values. `cancel(&h)` ends the thread's current or
  next wait (join, lock, channel, sleep, socket). `diag.deadlock` for wait cycles and for every thread waiting.
- `coby --explore N` / `--schedule-seed S`: seeded deterministic schedules, outcomes grouped, a seed per outcome.
- `slice_parts` (std::collections) for data parallelism over disjoint exclusive slices.
- Found by the flagship load test (10,000 connections, 0 failures, 248 MB): more than 127 live threads gave a
  false `diag.aliasing-conflict` (heap ids reached the held-path bit; fixed, `conf.many-threads-owning-frames`);
  listen backlog 128 → 4096; per-connection work moved off the exclusive lock.
- Showcase tier 13: `supervised_dns`, `first_answer`, `deadlock_found`, `parallel_chunks`.

### Supervised tasks completed: CHG-0234 (2026-10-10)

- `cobc --explore N` / `--schedule-seed S` / `COBALTC_SCHEDULE_SEED`: a seeded one-thread-at-a-time scheduler in
  `cbrt`; swept over 138 thread programs (one outcome each but one the spec leaves to timing).
- A thread locking a mutex whose interior may hold an exclusive reference (a channel of them) is lent
  (`conf.thread-channel-exclusive`); `[Deadlock]` (b) without a cycle faults the ends of the wait chains only.
- Server cost per connection flat to 10,000 (0.16 ms a round trip): `str` views pathless, id 0 not heap 0, 256
  heaps, `cb_rebind` local, `ppoll` + `SIGURG` socket waits on Linux. DNS-over-TLS load test at 3,000 exact.
- Found under the scheduler: a socket call made by a destructor (runtime held) gave up its turn and deadlocked.

### Round 9: real-world programs after Update 94 (2026-10-10)

- `stress/round9`: a chat hub (supervised readers, per-client writers, a broadcast thread), a job scheduler
  (priority queue, worker pool, per-job threads with deadlines and `cancel`), parallel word counts (owned
  chunks, merged `HashMap`s, `slice_parts` sort), a key-value service over HTTP (keep-alive clients, a snapshot
  thread cancelled at the end, a failing handler), a parallel box blur (shared source, exclusive bands). Same
  output from `coby`, gcc and clang.
- `CHG-0235`: a kept HTTP connection closed unanswered no longer sends a POST again
  (`conf.http-keep-alive-post-not-resent`); `==` on a resource and reading a resource whole explain themselves.
- `CHG-0236`: a lookup of id 0 no longer reads heap 0's table unlocked (a race since CHG-0234, caught by the
  debug build); `cb_frame_hold`/`cb_frame_unhold`/`cb_recv_datum_tok` local (threads on their own data: 1.5x ->
  2.4x on four cores). Found: four threads reading one structure through a shared reference are slower than one
  (every derived path lives in the owner's heap) -- proposal in `private/proposal-shared-reads-scale.md`.

### D-0205: shared reads that scale, round-9 std additions (2026-10-10)

- `CHG-0237` (`cobc`): functions with frozen shared reference parameters get two bodies, chosen at entry by
  `cb_frozen_ok`; in the frozen one, reads through those parameters and hand-offs to readers need no runtime entry.
  csvquery.cb (20 000 rows): one thread 128 -> 48 ms, four threads now faster than one (30 ms). A first version
  without the entry test was unsound and caught by the compiled suite; the same flaw in D-0200's `_c` variants for
  `&v` passed fresh (an element held by reference read unchecked) is fixed with a run-time test at the call
  (`conf.frozen-param-held-element`).
- `CHG-0238`: `Result::ok`, `slice_eq`, `StringView::from_utf8`, `UdpSocket::try_clone`, `Child::kill_tree`
  (`platform: linux` header value for its case). round9 programs use them (dnscache: a sender thread on a clone).
- Benchmarks neutral (bench/*.cb, gcc, best of three).

