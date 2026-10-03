# `cobc` — Stage 1 plan: a native compiler for x86_64 Linux

Status: plan (2026-09-22); **Stage 1 complete the same day (M1–M5)** —
see `STATUS.md` "`cobc`: the native compiler, Stage 1 / M1" and its
M2, M4, M3 and M5 subsections. Every non-concurrency conformance row,
file case, and example compiles to `coby`'s outcome, phase, and
output. **Stage 2 (concurrency) done 2026-09-23 (M6–M8) — §9, and `STATUS.md` "Stage 2".** **Stage 3 (speed) done 2026-09-23 — §10.** Decisions here are made;
open questions are listed as such at the end. This document is the
reference for the Stage 1 work and is updated as it lands; `STATUS.md`
records what is actually working.

## 0. What a CobaltC compiler is

The specification discharges many rules dynamically — aliasing
(`¬clash`, `solitary`), temporal validity, initialization, resource
authority — and `spec/conformance.md` pins the phase of every case: a
`✗ diag.x (dynamic)` row must be *accepted* statically and *fault at
run time*. `rule.control.flow-analysis` (`spec/14`) fixes exactly what
the static pass may prove. A conforming compiler is therefore, by
construction:

    native code for the program  +  a runtime holding Σ's dynamic components

and nothing else is on the table for Stage 1. Speed beyond
sanitizer-build territory comes later, and only by widening the static
discharge table (a `CHG` each time) — Stage 3.

Stage 1's finish line: **every non-concurrency case in the file suite,
every non-concurrency row of `spec/conformance.md`, and every example
produces the same outcome, phase, diagnostic, and output under `cobc`
as under `coby`.** `coby` is the oracle throughout.

## 1. Decisions

| # | Decision | Why |
|---|---|---|
| D1 | **Target: x86_64 Linux, SysV ABI, ELF, only.** | Already the committed platform; `AddrWidth = 64`, little-endian, `DW = 4` are `coby`'s choices and stay. |
| D2 | **Backend emits C (gnu11), compiled by the system `cc`.** | No LLVM dependency; `cc -O2` does register allocation; `#line` gives gdb; `[Layout-Struct]`/`[Layout-Enum]` coincide with C's struct/union layout for the same scalars; `extern fn` is a C prototype; `__builtin_*_overflow` and `__int128` cover checked arithmetic and `i128`/`u128`. |
| D3 | **Runtime `cbrt` is a fresh Rust `staticlib` implementing `spec/04`'s dynamic Σ components over real addresses.** Not a lift of `interp.rs`. | `interp.rs` is a big-value model (`objects: HashMap<u64, Object>` holding structured `Value`s; references found by scanning values). A compiled program's memory is bytes; occurrences of references must be tracked explicitly. The *check logic* (`clash`, `solitary`, `excluded`, `ancestors`, `sync-exempt`, destruction order) is ported from `interp.rs` with its spec citations; the *data model* follows `spec/04` literally. |
| D4 | **The front end is `coby`'s, unchanged:** loader, parser, `modres`, `typecheck` (`lib::analyze`). `cobc` adds a complete typing of every expression, done *by the lowering walk itself* as it emits (revised at M1: no separate typer pass or type map). | `typecheck.rs` is deliberately conservative (18 `Ok(None)` sites) because the evaluator is the authority; a compiler needs one type per expression. Hardening `typecheck` would risk the interpreter; computing types in the lowering (run only on programs `typecheck` accepted) cannot change acceptance, and one walk with expected-type propagation turned out simpler than two. |
| D5 | **References are fat in registers, thin in memory.** `struct cb_ref { void *p; uint64_t tok; }` for locals, parameters, results; a stored reference is the 8-byte pointer (`[Repr-Ref]`, `sizeof = AddrWidth/8`) plus a runtime *ref-datum* entry keyed by the slot's address. | Keeps `sizeof`/`rawptr` reinterpretation honest while giving every access its access-path token. |
| D6 | **The runtime owns frames, temporaries, destruction, and unwinding.** Generated C never `longjmp`s; `cb_fault` prints the diagnostic, runs `[Fault-Unwind]` inside the runtime (destructor callbacks into generated code via type descriptors), and `exit(1)`s. | `[Fault-Unwind]` is "run destructors down the stack, then terminate" — no catching (D-0009) — so a runtime-maintained frame stack is the whole mechanism. |
| D7 | **Expressions are lowered to statements** (three-address form): every temporary is a named C local registered with the runtime, evaluated in `spec/13`'s order. | `rule.control.stmt`'s temp scopes (D-0019) and `[Stmt-Exit]` need every temporary to be an object the runtime knows. |
| D8 | **Monomorphization**, driven by the typer's worklist, exactly as `typecheck::Ck` walks instantiations; only functions reachable from `main` are emitted. | Generics are templates (`spec/12`); tree-shaking also keeps unsupported Stage 2 prelude paths out of Stage 1 builds. |
| D9 | **`fn` value image = the function's address** (impl-defined, injective); `str` literals live in `.rodata` (`str_ptr` returns that address); `dangling<T>()` = 1; `mutex` extra cells stay 8 bytes. | Documented in `STATUS.md`'s impl-defined table for `cobc` when it lands; the reasons are the spec's own (`[Repr-Fn]`, `[Str-Ptr]`). |
| D10 | **Diagnostics render identically to `coby`** (`error: diag.x (dynamic)\n  at file:line …`), from the same registry, so every existing runner works unchanged on a `cobc --run` binary. | The suites drive the binary; identical output means zero new test-harness logic beyond "which binary". |
| D11 | **Threads, mutexes, handles, `spawn`/`join`/`lock` are Stage 2.** `cobc` rejects a program that reaches them with `unsupported: concurrency (Stage 2)` at compile time. | They need real threads and a lock discipline over the shadow state; nothing in Stage 1 depends on them. |
| D12 | **Layout:** `impl/` becomes a Cargo workspace: `coby` (root, unchanged), `cobc` (compiler, depends on the `coby` lib), `cbrt` (runtime, no dependencies; shares `diagnostics.rs` by `#[path]` include). | The interpreter crate is not touched; the runtime cannot accidentally depend on the AST. |

## 2. The runtime ABI (`cbrt`)

Everything generated code says to the runtime. Types: `obj` and `tok`
are `uint64_t` identities; `ty` is an index into the program's type
table; `loc` is a `(file_id, line)` pair from the loader's source map,
passed as constants.

    -- program
    void     cb_init(const cb_type *types, size_t n, const char *const *files);
    void     cb_terminate_ok(void);                     // [Terminate-Ok]; exit 0
    noreturn cb_fault(const char *diag, cb_loc at);     // [Fault-Unwind] then exit 1
    -- frames and statement scopes (spec/14 §1, §2)
    void     cb_frame_push(void);   void cb_frame_pop(void);   // [Block-Enter]/[Block-Exit]
    void     cb_stmt_push(void);    void cb_stmt_pop(void);    // [Stmt-Enter]/[Stmt-Exit]
    -- objects (state.objects, state.holder, state.init, obligations, authority)
    obj      cb_establish(void *addr, ty t, cb_loc at);       // fresh, temporary (D-0019)
    obj      cb_establish_uninit(void *addr, ty t, cb_loc at);// [Let-Uninit]
    tok      cb_adopt(obj o);                                  // holder := this frame's root path
    void     cb_result(obj o);                                 // re-stamp as the statement's result
    -- access paths (state.access-paths; spec/08, spec/10, spec/16)
    tok      cb_borrow(tok base, const cb_proj *p, size_t n, cb_mode m, cb_loc at);  // [Borrow]: ¬clash
    tok      cb_project(tok base, const cb_proj *p, size_t n);                       // field/index narrowing
    void     cb_read(tok t, cb_loc at);      // temporally-valid, init-state, ¬clash(shared)
    void     cb_write(tok t, cb_loc at);     // temporally-valid, ¬clash(exclusive), ¬live-resource-at
    void     cb_index_check(tok t, uint64_t i, uint64_t n, cb_loc at);
    -- moves and destruction (spec/07)
    void     cb_transfer(tok src, obj dst, cb_loc at);         // [Transfer]/[Relocate-In]
    void     cb_move_out(tok src, cb_loc at);                  // [Relocate-Out] (payloads, match)
    void     cb_destroy(tok t, cb_loc at);                     // drop(x): [Destroy]
    -- reference data (D5)
    void     cb_store_ref(void *slot, tok t);  tok cb_load_ref(const void *slot);
    void     cb_copy_datum(void *dst, const void *src, ty t); // struct copies carrying refs
    -- raw storage (spec/20, spec/21 §0)
    void    *cb_allocate(uint64_t size, uint64_t align, cb_loc at);
    void     cb_deallocate(void *p, cb_loc at);                // [Release] on reclaimed objects
    obj      cb_reclaim(void *p, ty t, cb_loc at);             // [Reclaim]
    void    *cb_rawptr_of(tok t);
    -- strings and output
    const uint8_t *cb_str_ptr(const uint8_t *data, uint64_t len);
    int64_t  cb_write(const void *p, uint64_t n);              // the `write` extern

Type descriptors (`cb_type`) carry: name, size, align, kind, `is_resource`,
fields `{offset, ty}` (for `destroy-composite` and datum copying),
enum variants, and a destructor pointer `void (*drop)(cb_ref self)`
generated for every type with a user `fn T::drop`. Projections
(`cb_proj`) are `{FIELD i | INDEX n | PAYLOAD}` arrays emitted as
static constants per access site; a dynamic index is passed in.

Checked arithmetic is *not* a runtime call: `CB_ADD_I32(a, b, loc)`
macros use `__builtin_add_overflow` and call `cb_fault` on overflow;
division and remainder special-case zero and `min / -1`
(`[Div-By-Zero]`, `[Div-Overflow]`); shifts check the count.

The runtime's `clash`/`solitary` are the `interp.rs` functions with
`scan_refs` replaced by the ref-datum table and the per-object
`held-by` sets `spec/04` specifies; `ancestors` is recorded at borrow
formation exactly as `record_token_ancestors` does today.

## 3. Lowering (`cobc`)

1. **Typer** (`cobc/src/typer.rs`): a complete type assignment for
   every expression of every reachable instantiation, keyed by
   expression address, plus the closure capture-struct types it
   synthesizes. Reuses `typecheck`'s substitution and unification
   helpers. A `None` here is a `cobc` bug and aborts compilation with
   an internal error naming the site — never a silent guess.
2. **Types to C**: scalars as `int8_t … __int128`, `float`/`double`,
   `bool` → `uint8_t`, `void` → no storage; `str` → `{ptr, len}`;
   `ref`/`rawptr` → `void *` in memory; `array<T,N>` → `struct { T a[N]; }`;
   structs in declaration order (= `[Layout-Struct]`); enums as
   `struct { uint32_t tag; union { … } u; }` (= `[Layout-Enum]`);
   generic instantiations mangled (`Vec__i32`). A `static_assert` on
   `sizeof`/`_Alignof` against the spec's `[Sizeof-*]` per emitted type.
3. **Functions**: one C function per instantiation, `cb_ref` for
   reference parameters and results, hidden capture parameter for
   closures. Bodies in three-address form (D7) with `cb_stmt_push/pop`
   around each statement and `cb_frame_push/pop` around each block;
   every local and temporary is a stack object registered with
   `cb_establish`/`cb_adopt`.
4. **Accesses**: each place expression becomes a token (`cb_project`
   from a binding's root token or a dereferenced `cb_ref`) followed by
   `cb_read`/`cb_write` at the use, then the plain C load/store.
5. **Intrinsics** (`typecheck::INTRINSIC_NAMES`): arithmetic families
   and conversions inline; `sizeof`/`alignof` constants; `drop`,
   `reclaim`, `release`, `rawptr_of`, `reinterpret_ptr`, `allocate`,
   `deallocate`, `copy_raw`, `dangling`, `str_*`, `write`, `fault` →
   runtime calls; `spawn`/`join`/`lock`/`Mutex::new`/`map_err`'s
   thread-related uses → Stage 2 rejection.
6. **Diagnostics**: static ones come from the `coby` front end at
   compile time with `file:line` as today; dynamic ones from the
   runtime with the `(file_id, line)` the generated code passes.
7. **Driver**: `cobc main.cb [-o prog]` writes the C beside the output
   with its extension replaced (`prog.c`; `a.out` → `a.c`), invokes
   `cc -std=gnu11 -O2 -ffp-contract=off -w -o prog prog.c -L<cbrt> -lcbrt -lpthread -ldl -lm`
   (`-O LEVEL` / `-O1` etc. replaces the default `-O2`; `-march=CPU` is
   passed through, none by default; `-ffp-contract=off` is always given,
   so no `-march` can fuse `a*b + c` into one rounding against spec/06;
   `--cc COMPILER` runs another C compiler in place of `cc` with the same
   options, which GCC and Clang both accept; `--help` lists every flag),
   and deletes the C afterwards unless
   `--keep-c` is given (it is always kept when `cc` fails);
   `cobc --run main.cb` compiles to a temporary directory and runs the
   result, forwarding exit status and streams — a drop-in for `coby`.

## 4. Testing

- `cobc --run` under the existing runners: `cb_conformance_suite`,
  `examples_run`, and a compiled variant of `spec_rows` (assembled
  programs written to a temp file, `cobc --run` on it). Selection by
  `COBALTC_BIN=cobc` so nothing is duplicated.
- **Differential**: every case's outcome, phase, diagnostic id,
  location, and stdout must equal `coby`'s. Concurrency cases (spec
  §12 rows, `19-concurrency/`) are skipped by an explicit list that the
  test prints, so the gap is visible, not hidden.
- Layout assertions at C compile time (§3.2) catch any drift between
  the emitted structs and `[Sizeof-*]`/`[Layout-*]`.
- *(2026-09-24)* Location was listed above but not compared until now:
  the compiled suites compared only the diagnostic id and phase, and
  stdout. They now compare all of stderr as well. The exception is the
  racy `conf.cross-thread-write-conflict` row. Concurrency cases are no
  longer skipped (Stage 2).
- *(2026-09-24)* **Random programs** (`cobc/tests/differential.rs`):
  generated, well-typed programs aimed at the checks Stage 3 removes
  must agree with `coby` in exit status, stdout and stderr (see
  `STATUS.md`).

## 5. Milestones

Each gate is a directory of the file suite (plus the matching
`spec/conformance.md` sections) passing under `cobc`.

| M | Scope | Gate |
|---|---|---|
| M1 **(done 2026-09-22)** | workspace, driver, runtime skeleton (frames, fault, diagnostics), scalars, control flow, functions and generics, structs/enums/`match`, arrays, `str`/`print` | every case of `06`, `12`, `13`, `14` that uses no reference or resource (27); in all, 99 file cases pass and 51 are skipped as later milestones — the directory-level gate was too coarse, since most cases borrow a `Vec` to have something to check |
| M2 **(done 2026-09-22)** | borrows and access paths, moves, destructors, block/statement exits, temporaries, definite assignment, `[Fault-Unwind]` | `07`, `08`, `09`, `10`, `11`: 30 of 35 pass; the 5 skipped use `Vec` (M4). 115 file cases pass overall, stdout identical to `coby`'s |
| M3 **(done 2026-09-22)** | generics (monomorphization; M1), closures (capture structs, capture modes, `self` borrow), fn values as callable boxes with per-signature thunks, `?`/`Result`, `map_err` | `15`, `16`, `18`, rest of `12`: every non-concurrency case passes; 141 file cases overall, 12 of 13 examples native |
| M4 **(done 2026-09-22)** | raw storage: `allocate`/`reclaim`/`release`, `rawptr`, `str`, the prelude's `Vec`/`String`/`Rc` compiled as user code; `[Stmt-Exit]` token retirement | `20`, `21`: every non-concurrency case passes; 132 file cases overall, 10 of 13 examples native |
| M5 **(done 2026-09-22)** | modules and file-backed modules (loader shared), surface-syntax cases, all examples, all non-concurrency spec rows via `cobc/tests/compiled_spec_rows.rs` (row assembly shared in `tests/common/mod.rs`); `STATUS.md` compiler section and `cobc`'s impl-defined table | `17`, `22`, `cobaltc_examples`, `spec_rows` minus §12: 143 rows pass, 10 skipped (concurrency); 141 file cases; 12 of 13 examples |

M1 is one session of work; M2–M3 are where the semantics live and
where differential mismatches will cluster; M4 is mostly the prelude
finding runtime gaps; M5 is closure.

## 6. Risks, in the order they will bite

1. **Typer completeness** — closures whose capture types depend on
   inference, `match` arms typed by their first arm, generic calls
   inferred from expected type. Mitigation: the typer runs on every
   accepted case in the suites *before* any C is emitted, as its own
   test.
2. **Reference occurrences inside aggregates** — a struct with a `ref`
   field copied, stored into a `Vec` element, moved through an enum
   payload: every path must keep the datum table right. Mitigation:
   `cb_copy_datum` driven by type descriptors, never by hand-written
   per-site code; §08/§10 cases are the gate.
3. **Temporaries and result re-stamping** (D-0019) — which objects
   `[Stmt-Exit]` ends. Mitigation: D7's three-address lowering makes
   every temporary explicit; `cb_result` marks the one that flows out.
4. **Destruction order and unwinding** through nested blocks, loops,
   `return` from inside blocks, `?`. Mitigation: the runtime, not
   generated code, decides what ends — generated code only reports
   frame and statement boundaries.
5. **Real addresses vs `coby`'s arena** — `rawptr` arithmetic and
   `reclaim` at addresses inside stack objects. Mitigation: the object
   table is keyed by real address ranges; `reclaim` finds the
   enclosing or previously reclaimed object exactly as `[Reclaim]`
   specifies.

## 7. Out of scope for Stage 1

Concurrency (Stage 2); any check elision (Stage 3); optimization of
the runtime's tables; Windows/macOS; separate compilation, `.o`
files, or linking two CobaltC programs (a `spec/22` §5 absence).

## 8. Open questions (to settle when reached, not now)

- Whether `cbrt` keeps `held-by` as per-path sets (spec-literal) or
  per-object occurrence lists (what `scan_refs` effectively computes);
  choose by profiling M2.
- Whether closure capture objects are stack (as `coby` models them)
  or heap when a `move` closure is returned — `spec/15` §6 decides;
  read it at M3.
- The exact skip list for concurrency rows in `spec_rows`.

## 9. Stage 2: concurrency

Begun 2026-09-23. Scope: `spawn`, `join`, `handle<τ>` and its
destructor, `Mutex::new`, `lock`, `guard<τ>`, `sync-exempt`, and a fault
raised in any thread — every case `cobc` now answers `unsupported:
concurrency (Stage 2)`. The finish line is Stage 1's, without the
exception: every case, row and example identical to `coby`.

### Decisions

| # | Decision | Why |
|---|---|---|
| S1 | **One global lock (GIL) in `cbrt`, as in `coby`.** Every CobaltC thread is an OS thread; it holds the GIL whenever it runs generated code or the runtime, gives it up only while blocked (`join`, `lock`, a handle's destructor) and, while more than one thread is live, at each statement boundary (`cb_stmt_push`). A yield hands the GIL to a waiting thread before re-acquiring it, so a busy thread cannot starve the others. | `spec/19` `[Thread-Step]`: each step is atomic and the order sequentially consistent. With the GIL, the runtime's tables and the program's own memory are touched by one thread at a time, so no C data race exists to reason about. Parallel speed-up is not a Stage 2 goal; finer locking belongs with Stage 3. |
| S2 | **Per-thread runtime state**: the frame/scope stack, the in-flight queue and the current location are swapped in and out with the GIL. Objects, paths and slots stay global (`spec/04`: one `Σ`). | Frames are per thread (`frame-stack(ℓ)`); everything the checks read is shared. |
| S3 | **`spawn` stores its arguments in a heap block, and the new thread makes an ordinary call from it**, through the callee as a `fn` value (a named function's interned box, or a boxed `move` closure) and its per-signature thunk. A reference argument is stored in the block as a held slot; a resource moves into the block; reference data is copied with its slots. | An ordinary call hands references over before the caller's statement ends; a thread may start later, when `[Stmt-Exit]` would already have retired an unheld path. Holding the arguments in the block is `[Spawn]`'s `store(binding(p_i), r_i)` done early. Calling through the thunk reuses every calling convention Stage 1 built. |
| S4 | **`handle<τ>` is the thread id (`uint64_t`)**; its descriptor kind `CB_K_HANDLE` makes its destructor `[Handle-Destructor]`: wait, then discard an unclaimed result. `join` waits, copies the result out of the block (the object identity moves with it), marks it taken, and consumes the handle. | `[Repr-Handle]` allows any injective encoding; a thread's result lives in its block until someone claims or discards it. |
| S5 | **`mutex<τ>` is `struct { τ inner; uint64_t state; }`** (`[Sizeof-Mutex]`, a `CB_K_STRUCT` descriptor, so destroying it destroys `inner`). Lock state is kept by the runtime, keyed by the mutex's address. **`guard<τ>` is a pointer to `inner` whose token is kept in the slot table** — a stored exclusive reference (`[Repr-Guard]`), so `*g` lowers like `*r` and the lock path takes part in `clash` while the guard holds it. `CB_K_GUARD`'s destructor is `[Guard-Drop]`. | The representations are the spec's own; treating a guard as a stored reference reuses the whole aliasing machinery. |
| S6 | **`sync-exempt`**: a lock path records the projection of the mutex it was formed on (`lock_of`), and paths borrowed through it inherit it. `clash` skips a shared path onto that mutex when the access is lock-derived, and skips a lock-derived path when the access is a shared one onto the mutex. | `spec/19` §2, the one extension to `clash`. |
| S7 | **A fault in any thread** reports, unwinds that thread's stack only, and exits 1 while still holding the GIL, so no other thread takes another step. | `spec/18` `[Fault-Unwind]`: "other threads take no further steps; their frames are not unwound." |

### Milestones

| M | Scope | Gate |
|---|---|---|
| M6 **(done 2026-09-23)** | GIL and per-thread state in `cbrt`; `spawn`, `join`, `handle<τ>`, `[Handle-Destructor]`; faults in threads | `spawn_join_value_ok`, `join_nested_in_call_ok`, `thread_resource_authority_rekeyed_ok`, `spawn_borrow_closure_rejected` |
| M7 **(done 2026-09-23)** | `mutex<τ>`, `Mutex::new`, `lock`, `guard<τ>`, `[Guard-Drop]`, `[Lock-Reentrant]`, `sync-exempt` | the rest of `19-concurrency/`, every concurrency spec row, `09_threads.cb` |
| M8 **(done 2026-09-23)** | Closure: `STATUS.md`, the impl-defined table, README's status line, and the guide's §19 examples under `cobc` | all suites: 0 skipped |

## 10. Stage 3: speed

Begun 2026-09-23. A compiled program performs every check the
specification discharges dynamically; Stage 3 makes that cheaper
without changing any program's outcome. Three tracks, in the order
they cost the specification nothing:

| Track | What | Spec change |
|---|---|---|
| T0 | The runtime's own cost: its tables, allocation per check | none — invisible to programs |
| T1 | Checks `rule.control.flow-analysis` *already* proves: `spec/14` §6 says a proven condition is discharged statically, "no runtime check", so performing it was conforming but unnecessary | none |
| T2 | Proving more: each widening of the analysis's discharge table is a `CHG` to `rule.control.flow-analysis`, decided by the owner | one `CHG` each |

Parallel execution of threads (replacing Stage 2's GIL) is a fourth,
independent track, **T3 — done 2026-09-23** (`STATUS.md` "T3").

### Measurements (`12_collatz.cb`, release build)

| Step | Instructions (callgrind) | Time |
|---|---|---|
| Stage 2 end | 2.97 G | 0.58 s |
| T0: id hasher for the runtime's maps (SipHash was 54% of all instructions) | 1.34 G | 0.31 s |
| T0: `cb_read`/`cb_write` check in place, no allocation | — | 0.28 s |
| T1: unchecked plain locals | 0.61 G | 0.155 s |
| T1: scope-free statement scopes and block frames dropped | 0.59 G | 0.155 s |
| T0 (option C): id lists reused from a pool; `cb_borrow` checks in place | 0.52 G | 0.14 s |
| T3: the lock-mode check on every runtime call (parallel threads) | 0.60 G | 0.15 s |
| T1: unborrowed reference-holding locals unchecked | 0.59 G | 0.15 s |

`coby` takes 2.3 s. What is left is structural: 82% of the run is
`chain_length`'s accesses to `memo` through a reference — `*r` accesses
are "never proven" by the current table — and `Vec::index_shared`
alone is 35%, the prelude's own reclaim-borrow-return per element.
Further large gains are T2.

### T1 as built

- **Unchecked locals.** A local of a plain type (not a resource, no
  reference inside, not callable) that the function never borrows,
  captures or drops has every check proven: `valid` holds in scope (a
  plain value is never consumed), `init` is definite assignment's
  static guarantee (`spec/11` §3: the dynamic check "is unreachable in
  a well-formed program"), and with `escaped` and every `deriv` false,
  `¬clash` holds. It gets no runtime object and no checks.
  `pinned_names` collects the names a body borrows, captures or drops,
  by name across scopes, which only errs toward checking; a missed one
  would be a loud internal error, not a skipped check.
- **Scope-free regions.** A statement scope or block frame whose
  lowered code calls the runtime only through calls that form no
  temporary or path (`cb_fault`, `cb_index_check`, `cb_str_eq`,
  `cb_write_out`, `cb_at`) and contains no jump (a jump would pop the
  scope itself) is dropped: `[Stmt-Exit]`/`[Block-Exit]` would have
  nothing to end. Threads still yield at every statement that keeps
  its scope; a run of scope-free statements is one step of
  `[Thread-Step]`'s interleaving, which the spec allows.

### Stage 3 status (2026-09-23)

Everything that needs no specification change is done: T0, T1 (plain
locals; reference-holding locals, whose object stays because its slots
make their references held; scope-free statements and blocks) and T3.
`12_collatz.cb` runs in 0.15 s against the interpreter's 2.3 s; four
threads of plain computation run 3.2× faster than under the GIL. What
remains is T2, and every T2 item is a specification decision:

- candidates 1–2 are unsound under D-0018 as it stands (below);
- candidate 3 needs a `CHG` and a real `deriv`/`escaped` dataflow in
  `cobc`, and helps only call sites of library functions, not loops
  through references, which is where the remaining time goes;
- candidate 4 is how `spec/20`/`spec/21` form an element reference.

**Resolved (2026-09-23, decisions delegated by the owner): Stage 3 is
complete.**

- **Candidates 1–2 / D-0018:** D-0018 stays; `spec/decisions/D-0022`
  records why (validity through a reference parameter stays dynamic
  under any aliasing rule, and hold-time checking would move faults in
  existing programs for a small measured gain).
- **Candidate 3:** declined. Measured on `12_collatz.cb`, the borrow
  checks it would remove (at call sites of non-retaining functions in
  `main`) are a negligible share; nearly all are in `chain_length` and
  `Vec::index_shared`, through references.
- **Candidate 4:** declined at the time — the reclaimed element objects
  are what make an element reference stale after a reallocating `push`
  (`conf.e2e-vec-realloc-stale-ref`). **Revisited by
  `spec/decisions/D-0023`** (owner, 2026-09-23): the model stays, but
  `spec/21` §0 already lets the library be realized natively, and a
  plain element object with no path but its root is unobservable. See
  "Vec element access realized natively" below.

**Correction to this section's framing.** T2 was described as needing a
`CHG` per widening. That holds for making static *rejections* stronger;
merely skipping a run-time check that provably cannot fail changes no
outcome and needs no rule change (`rule.control.flow-analysis` fixes
which programs are rejected, not which passing checks run) — T1 is
exactly that. Future elisions need a soundness argument, not a `CHG`.

### T2 candidates 1–2: examined and set aside (2026-09-23)

Proving accesses through reference parameters is unsound under the
current specification, for two independent reasons found by
counterexample (both run to the diagnostic shown in `coby` and `cobc`):

- **Validity.** A thread's referent can end during the thread without
  any scope ending: `spawn(reader, Vec::index_shared(&v, 0))` followed
  by reallocating pushes is `diag.stale-binding` inside `reader`. No
  rule about handles can see storage release.
- **Clash.** A conflicting reference can become *held* with no check:
  `f(&mut x, &x)`, `g(&x, spawn(writer, &mut x))` — each borrow forms
  while the other is still unheld, and the first access through the
  parameter is where D-0018's use-time check catches the conflict.

Making them provable would need clash checked when a reference becomes
held (stored in a binding, parameter or aggregate) — a revision of
D-0018 to be weighed on its language merits, not a `CHG` for speed.

### T2 candidate 4 as far as the runtime goes (2026-09-23)

`Vec::index_shared` fell from 205 M to 176 M instructions but is still
a third of `12_collatz.cb`: its remaining work is what the prelude's
definition requires per call (a frame, the parameter binding holding
the reference, two checked reads through `*v`, a reclaimed element
object and a fresh borrow of it, the scopes that retire that borrow).
Cheaper element access beyond this is a `spec/20`/`spec/21` question
about how an element reference is formed. *(Superseded: see the next
section.)*

### Vec element access realized natively (D-0023, 2026-09-23)

The last paragraph was wrong on one point. `spec/21` §0 says the
library bodies are normative for their *observable behavior* only. So
`cobc` compiles the prelude's own `Vec::index_shared`/`index_exclusive`
(every `T`), and `Vec::push`/`Vec::drop` (plain, non-zero-sized `T`),
to native bodies that perform only the checks that can fail
(`native_vec_index`/`native_vec_push`/`native_vec_drop` in `lower.rs`;
`cb_elem_borrow`/`cb_vec_drop_plain` in `cbrt`). An element object
`cb_elem_borrow` establishes for a plain type is *ephemeral*: it ends
when its last derived path ends, unless a `[Reclaim]` written in the
program has re-attached it. That removes the per-element memory that
used to grow until `[Release]`.

| Program | Before | After |
|---|---|---|
| `12_collatz.cb` | 0.15 s | 0.07 s |
| Sieve of 1,000,000 `bool` | 16.2 s, 581 MB | 3.1 s, 3 MB |
| Sieve of 10,000,000 + Collatz below 1,000,000 | 210 s, 4.6 GB | 33 s, 18 MB (Rust: 0.34 s) |

What remains per element access is general, not `Vec`'s: the caller's
borrow of `&v`, statement scopes, and each path record.

### Cheaper borrows and scopes in the runtime (2026-09-23)

The runtime's own cost, with no change to what any program observes:

| Step | Instructions (sieve of 20,000) | Sieve of 1,000,000 |
|---|---|---|
| After D-0023 | 280 M | 3.12 s |
| Paths and objects in generation-checked arenas, not hash maps | 221 M | 2.17 s |
| An object's path list kept in the object (`Obj::toks`), not a side map | 195 M | 1.77 s |
| Ephemeral element objects indexed by a hash map; the `BTreeMap` keeps the persistent ones for `[Release]`'s range queries | 185 M | 1.68 s |
| Ephemeral objects get no root path; the element borrow is checked against the object directly and sent without a scope round trip | 156 M | 1.50 s |
| `&v` written in a `Vec::index_*` call: `cb_borrow_check` plus a `…_pre` body, no token minted | 120 M | 1.04 s |
| The `…_pre` borrow recorded in the caller's scope directly; one-lookup removal | 110 M | 0.95 s |

The fast call rests on D-0018: a borrow formed as a call argument and
never stored is unheld, no check counts an unheld path, and it ends
with its statement. So its only effect is its own check at formation,
which `cb_borrow_check` makes, together with the initialization check of
the body's first read, which can fail for nothing else. It applies only
when the index expression is stateless (locals, literals, field reads,
arithmetic), so that no call can run between the call site's checks and
the point where the body would have made them.

`12_collatz.cb`: 0.04 s (`coby`: 2.4 s). The full sieve-and-Collatz
benchmark: 11.3 s (was 33 s; Rust 0.35 s, 0.66 s with overflow checks).

### `cc` optimization levels (2026-09-24)

The driver's default became `-O2` (was `-O1`), with `-O LEVEL` and
`-march=CPU` to override it and `-ffp-contract=off` always given (§3.7).
Release build, median of three runs, Core i7 920 (no AVX); every build
prints the same result as the Rust program.

| `cc` flags | Sieve of 10,000,000 + Collatz below 1,000,000 | Collatz below 1,000,000 | Sieve of 1,000,000 | Compile |
|---|---|---|---|---|
| `-O0` | 16.6 s | 5.22 s | 1.13 s | 0.45 s |
| `-O1` | 10.8 s | 0.57 s | 0.94 s | 0.49 s |
| `-O2` (default) | 10.6 s | 0.50 s | 0.95 s | 0.58 s |
| `-O3` | 10.6 s | 0.50 s | 0.94 s | 0.78 s |
| `-Os` | 11.0 s | — | — | 0.57 s |
| `-O2 -march=native` | 10.6 s | 0.50 s | 0.94 s | 0.59 s |
| Rust | 0.35 s (0.67 s with overflow checks) | 0.25 s | 0.00 s | |

The level matters only where the checks are gone. The Collatz loop
works on plain locals whose checks T1 elides, so it is ordinary C:
`-O2` is 12% faster than `-O1` and within 2× of Rust. The sieve, about
95% of the full benchmark, spends 96.5% of its instructions (callgrind,
sieve of 20,000: 110 M) in `libcbrt` — `check_access`, `mint_in`,
`retire_token`, `cb_stmt_push`/`pop`, `cb_frame_push`/`pop`, element
borrows — which Cargo compiles and no `cc` flag reaches. `-O3` and
`-march=native` add nothing over `-O2` here; this CPU gives `native`
little to use. Faster sieve code means cheaper runtime checks, not
different C flags.

### Checks proven by `cc`, emptier scopes (2026-09-24)

Three more T1 items, none of which changes any outcome:

- **Bounds checks `cc` can see.** `cb_index_check` was an opaque call
  into the runtime (and took the runtime lock once threads existed),
  so a loop guard `i < 3` could not discharge `a[i]`. It is now a
  `static inline` comparison in `cbrt.h` that calls `cb_fault`. In
  `matrix_multiplication.cb`'s `mul3`, `-O2` removes all six. Overflow
  checks on induction variables, division by a constant and `narrow`
  between equal widths were already folded by `cc`, so `cobc` does no
  range analysis of its own.
- **Scopes settled per function (`resolve_scopes`).** `elide_scope`
  dropped a scope only if nothing inside it called the runtime at all,
  and never looked at branch tails or a function's own frame. Scopes
  are now emitted as markers, and once the body is lowered, each is
  kept only if some runtime call made while it is open could record
  into it: into the innermost statement scope (`top_scope`:
  temporaries, derived paths, `…_pre`) or the innermost frame
  (`top_frame`: `cb_bind`). A call not classified keeps every open
  scope. A jump's pops of a dropped scope go with it.
- **Conversions in a `Vec::index_*` fast-path index.** A conversion
  that cannot fault is as stateless as its operand, so
  `Vec::index_shared(&*memo, narrow<usize>(n))` takes the `…_pre`
  path.

| Program (callgrind) | Before | After |
|---|---|---|
| `12_collatz.cb` | 246 M (0.042 s) | 134 M (0.024 s) |
| `prime_sieve.cb` | 459 M (0.076 s) | 408 M (0.068 s) |
| `mini_interpreter.cb` | 4.5 M | 3.9 M |
| `matrix_multiplication.cb` | 0.57 M | 0.45 M |

### Element accesses used at once; `Vec::len` realized natively (2026-09-24)

Two more items resting on D-0023 and D-0018, again with no outcome
changed:

- **`*Vec::index_*(…)` read, or assigned a stateless value, at once**
  (`elem_access_place`, `native_elem_access`, `cb_elem_access`), for a
  plain element type. The element borrow, the read or write check
  through it, and the statement scope that would end it are replaced
  by one call. Where no live object lies over the element's cells
  (the usual case), the borrow would establish an ephemeral object
  whose only path is this one, so neither check can fail and the call
  returns at once. Otherwise it makes the same checks and ends the
  unheld path immediately: no check counts an unheld path, and the
  ephemeral object's earlier end is unobservable. The `&v` form keeps
  the `…_pre` order. Any other first argument, such as a reference
  parameter, is evaluated as for the call and then runs the body's
  checked read of `len`, so the index may be any expression.
- **`Vec::len` realized natively.** Its body is one checked read of
  `v.len`. The frame and the parameter binding that holds the
  reference are unobservable, since nothing else runs while it is
  held. `Vec::len(&v)` takes `cb_borrow_check` and reads in place.

| Program (callgrind) | Before | After |
|---|---|---|
| `12_collatz.cb` | 134 M (0.025 s) | 54 M (0.011 s) |
| `prime_sieve.cb` | 408 M (0.069 s) | 118 M (0.021 s) |
| `mini_interpreter.cb` | 3.9 M | 3.2 M |
| `sort_vec.cb` | 0.88 M | 0.59 M |

What is left in both is mostly `cb_borrow_check` of `&v` at every
access (its `check_access`), and `Vec::push`'s call path.

### Confined local `Vec`s (2026-09-24)

The benchmark in `bench/` (`bench.cb`: sieve of 10,000,000 + Collatz below
1,000,000) took 3.65 s against Rust's 0.35 s (0.66 s with overflow
checks). The Collatz half was already faster than checked Rust. The
sieve was 24× slower than checked Rust: about 400 instructions per
element access, nearly all in the runtime (`&is_prime`'s borrow check,
the path made and ended around each `push`, the checks inside `push`).
Removing by hand the checks that cannot fail in that function brought
it to 0.15 s. That is what was then built, and it is T2 candidate 3
widened. Owner's decision, 2026-09-24: proceed.

- **The analysis (`Gen::confined_vecs`, `Fx::bind_confined`).** A local
  `Vec<T>` (plain, non-zero-sized `T`), initialized where it is
  declared, whose every use is `&x`/`&mut x` passed straight to the
  prelude's `Vec::push` or `Vec::len`, or to `Vec::index_*` whose result
  is dereferenced at once, is *confined*. "At once" means a read in a
  value position, or the target of an assignment whose value pushes
  nothing onto `x`. The local may also be moved out as the function's
  own result. It is matched by name across scopes (like `pinned_names`),
  so any other use of the name disqualifies it, as does any mention in
  a closure.
- **Why nothing can fail on it.** Every borrow of `x` is unheld and ends
  with its call: `push` keeps only a plain `T`, `len` returns a number,
  and an element reference is used at once. So no path but `x`'s root
  ever exists. Its borrow checks can meet no conflict, `x` is never
  stale (never moved but last), and it is never uninitialized. The
  bodies' checked reads and writes of `len` fail for the same reasons.
  No element of it ever has an object: elements are reached only
  through these calls (`cb_elem_access`'s first case), and `grow` only
  copies bytes and releases the old buffer. An assignment whose value
  might push (and so reallocate) is left to the ordinary checks, which
  fault on the stale element.
- **What is emitted.** `Vec::len(&x)` becomes `x.len`. `Vec::push(&mut
  x, e)` becomes a `static inline` `…_nc` body with no checks, whose
  `grow` gets `x`'s root path. `*Vec::index_*(&x, i)` becomes the
  bounds check and the element's address. The prelude bodies' `cb_at`
  and faults are kept, so locations are unchanged.
- **Dead locations (`drop_dead_locations`).** A `cb_at` overwritten in
  straight-line code before anything can read it (a call, or a fault
  without its own location) is dropped. Any brace or jump keeps it.
  Checked by comparing the full diagnostic header (location included)
  of every file case under both tools.

Measured when this landed (2026-09-24; `cc -O2`, best of 7; current
figures: `bench/RESULTS.md`):

| | cobc | Rust | Rust, overflow checks |
|---|---|---|---|
| Sieve of 10,000,000 | 3.09 s → 0.151 s | 0.099 s | 0.103 s |
| Collatz below 1,000,000 | 0.46 s | 0.25 s | 0.57 s |
| Both (`bench.cb`) | 3.65 s → 0.626 s | 0.345 s | 0.662 s |

What remains in the sieve is plain C: bounds and overflow checks, and
reloads of `len`/`ptr` after each `bool` store, because a `uint8_t`
store may alias anything in C. Collatz's gap to unchecked Rust was put
down to the overflow checks; hardware counters later showed it is
mostly a mispredicted branch that GCC keeps even without checks
(2026-09-25, `bench/README.md`).

### Confined vectors passed to functions (2026-09-24)

Item 2 of the owner's review, option C (owner's decision, 2026-09-24;
D-0018 and D-0022 unchanged). With the sieve's work moved into functions
taking the vector by reference, the program took 3.87 s against 0.16 s
for the local version. Passing `&mut is_prime` to a user function had
also unconfined it in `main`, so its pushes were checked too.

- **Confining parameters (`Gen::confining_table`).** A `ref<Vec<_>, _>`
  parameter of a program function is *confining* when the body never
  re-binds its name and uses it only as a confined local may be used.
  That means as the vector argument of `Vec::push`, `Vec::len` or a
  `Vec::index_*` dereferenced at once (written `p`, `&*p` or `&mut *p`),
  or passed on to another confining parameter. Never returned (unlike a
  local, the body's result may not be the name), stored, captured or
  copied. The table is the greatest fixed point, so parameters that only
  pass the vector to one another, recursion included, are confining.
- **Callers.** A confined local, or a confining parameter, may be passed
  to a confining parameter, provided no other argument of the same call
  passes it as well (the two parameters would alias; `f(&mut v, &v)`
  keeps the ordinary checks and faults as D-0022 describes). Other uses
  in other arguments (reads, `len`, even pushes, which move the buffer
  but not the `Vec`) finish before the call begins and leave no held
  path.
- **Why nothing through the parameter can fail.** While the call runs,
  its caller is suspended. The parameter is the only path to the vector:
  no other argument passed it, nothing stored it, no thread was given
  it, and the callee only passes it on the same terms. So its borrows
  meet no conflict. The vector is not stale (the caller neither moved
  nor ended it) and was initialized. No element has an object. This is
  narrower than D-0022's case, which concerns arbitrary reference
  parameters: there another path, or a thread, may reach the referent.
- **Variants (`Gen::request_variant`).** A call with confined arguments
  goes to a copy of the function compiled for them, named by the mask
  of those parameters (`f_mark_c1`). The copy binds no runtime object
  for them and reaches the vector through `p.p`; it passes each on to
  the matching copy of the next function. Each argument is passed as
  its address and its root path, with no borrow formed. Other calls use
  the ordinary function, and only the copies a program calls are
  compiled.

Measured when this landed (2026-09-24, sieve of 10,000,000): the
sieve in functions went from 3.87 s to 0.16 s, the same as the local
version. Current figures: `bench/RESULTS.md`.

### T2 candidates (for the owner)

1. **Exclusive reference parameters as analysis roots.** Treat `*r`
   for a `ref<τ, exclusive>` parameter like a local root (`escaped`,
   `deriv` facts): the caller's borrow proved no conflicting path
   exists, and only paths formed through `r` can appear during the
   call. Would prove most accesses in functions like `chain_length`.
2. **Shared reference parameters, reads.** A read through a
   `ref<τ, shared>` parameter cannot clash (an exclusive path would
   have clashed with the caller's borrow) and its referent outlives the
   call.
3. **`escaped` after a call that cannot retain the reference** — the
   "confirmed trade" `spec/14` §6 names: a borrow passed to a call whose
   signature gives it nowhere to keep it (`Vec::push(&mut v, …)`) need
   not make `v` escaped. Needs an argument from `rule.temporal.ref-escape`.
4. **The prelude's element access.** `Vec::index_*` establishes and
   borrows a reclaimed object per call; a cheaper model of element
   identity is a `spec/21`/`spec/20` question, not only an analysis one.
