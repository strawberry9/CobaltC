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

`coby <file>` parses, loads the prelude (`src/prelude.rs`'s embedded
CobaltC source, `spec/21` transcribed verbatim), runs the static pass
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
  source** — transcribed verbatim into `src/prelude.rs` and run through
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

**`cobc` does not compile** (exit 3, `unsupported: …`): a thread whose result is a reference; a
generic function used as a value with nothing to fix its type
arguments (`auto f = id;` — `coby` accepts it, and whether it should be
`[Generic-Call-Uninferable]` is a specification question).

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

Friction met, for the owner (not changed):

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
