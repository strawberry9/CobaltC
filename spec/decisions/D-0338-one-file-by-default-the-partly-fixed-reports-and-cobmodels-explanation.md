# D-0338 — One file by default, the partly fixed reports, and cobmodel's explanation

Status: §2.1 is the owner's request for Update 209 (one C file without link-time optimisation by default, `--lto`
for the former default, `--one-unit` removed); §2.2 was delegated by the owner ("I let you make the decision on
whether a new design is a better option for CobaltC") and decided by the implementer; the rest decided by the
implementer under the owner's instruction to address the partly fixed reports where it makes sense; for the owner's
review (2026-11-24). Update 208 (`90757a5`) left fourteen reports partly fixed: 311, 315 and 337 of its own round
(D-0337 §3) and 059, 065, 073, 108, 109, 114, 148, 183, 209, 228 and 229 of Update 207's (each report's "Status
after Update 207"). This record holds the choices that changed a default, a rule, the syntax or a documented
behaviour, chose a design, or left a report with a reason. Each was chosen as the smallest choice consistent with the
earlier decisions and with the owner's rule that code reads on its own: no implicit rule a reader cannot see on the
line. Implemented by `CHG-0481`
Kind: Decision (`spec/02-schema.md`)
Depends on: D-0111 (`[Ref-Binding-Last-Use]`), D-0135 (a view of a temporary argument), D-0146 (`unsafe extern`),
D-0180 (the PostgreSQL client), D-0204 (`[Lock-Poisoned]`), D-0215 (`[Guard-Drop]`), D-0218 (shared reaches
read-only), D-0219 (the executable model), D-0252 (`ffi-struct`), D-0277 (no object cache; the C in parts with
link-time optimisation), D-0310 (a mutex's state follows its value), D-0318 (several result sources), D-0326 (the
toolchain without Rust), D-0335 (the destruction of deep structures, tail position), D-0336 §2.7 (a reference ends
after an `if` condition), §2.19 (the executable cache), §2.20 (the variadic table), §2.25 (`saslprep`), D-0337 §2.9
(313), §2.16 (337), §3 (311, 315, 337 left)
Revisits: D-0277's default build form (the C in parts, linked with link-time optimisation); CHG-0470's `--one-unit`
Affects: `spec/09`, `spec/10`, `spec/14`, `spec/20`, `spec/21`, `spec/22`, `spec/IMPLEMENTATION-NOTES.md`,
`spec/OVERVIEW.md`, `spec/registry/diagnostics.md`, `spec/conformance.md`

## 1. Problem

Three requests. (1) Since D-0277 a large program's C was compiled in parts, at most eight, all at once, and linked
with link-time optimisation (GCC's `-flto`, Clang's ThinLTO); CHG-0470 added `--one-unit` for one file without it,
which `impl/build.sh` used for the compiler itself because the parts made it 6% slower and 9% larger. The owner asked
for one file to be the default. (2) Fourteen reports were partly fixed: what still failed ranged from a slow lock
(315), a stack-deep `Rc` destruction (337) and a tracked divide-and-conquer split (311), through programs refused or
faulting that the rules can decide (108, 109, 114, 183, 229), to wrong calls of variadic C functions (228), passwords
PostgreSQL prepares differently (209), slow builds of large text matches (073) and of large programs (059), fault
messages without values (065) and per-call checks of `fn` values (148). (3) The guide and the book named `cobmodel`
without saying why it exists, when to use it or how.

## 2. Decision

1. **One C file without link-time optimisation by default; `--lto` (owner's request).** `cobc` compiles the
   program's C, the `std` it reaches included, as one file with no link-time optimisation, whatever its size. New
   `--lto` gives D-0277's form: the C in parts (`part_count`, at most eight, one per megabyte), compiled at once and
   linked with GCC's `-flto` or Clang's ThinLTO where the C compiler has it; a program of under a megabyte of C stays
   one part. This revisits D-0277's default, kept whole under the option. Measured (one run each, four cores, other
   builds running):
   - Programs built run about as fast and are smaller. On the `bench/` workloads, one part in both forms, GCC's
     instructions one file / `--lto`: `hashcount` 579.7M / 601.5M, `hashmap` 1067.5M / 1091.9M, `strings` 498.8M /
     501.0M, `tree` 138.0M / 139.8M, the other four equal; Clang's identical in size and within 0.3%. Large programs
     are 4.5–5% larger with `--lto`. The compiler itself built with `--lto` is 9% larger (15.70 MB / 14.42 MB) and
     runs 4% more instructions (`--emit-c` of `token_gate`: 16.57G / 15.92G, 7.27 s / 6.69 s; `--check` of its own
     source 55.4 s / 51.3 s), which is why `impl/build.sh` built it as one file already.
   - Builds take longer on several cores: `dns_over_http` 48.1 s / 44.0 s (GCC), 53.6 s / 36.5 s (Clang);
     `token_gate` 49.4 / 47.9 s and 57.2 / 39.8 s; an `http_get` program 27.7 / 22.3 s at `-O 2`, 19.4 / 14.5 s at
     `-O 0`; the compiler 880 s / 488 s (the one-file build under twice the load). Small programs within 0.1–0.25 s.
   So the default gives the build one keeps the smaller, equally fast program, and `--lto` is the choice for the
   edit-and-build loop of a large program on a machine with cores to spare, most of all with Clang. The guide's
   "How long a build takes, and `--lto`" gives these numbers.
   - **`--one-unit` is removed, with a message.** It is refused as an unknown option (status 2) with "the program's C
     is compiled as one file by default; --lto builds it in parts with link-time optimisation". Keeping it as a no-op
     was not chosen: an option that does nothing says something false on the line that uses it, and no script, gate
     unit, bench, showcase or workflow of the repository passes it. `impl/build.sh` still gives `--one-unit` to a
     compiler whose `--help` names it (stage 1, or an installed compiler from before Update 209), and nothing to the
     new one.
   - **The executable cache's format is 2.** The key's head is `cobc build cache 2`, and a `--lto` build adds an `lto`
     line (the one-file build had added `one unit`), so neither form matches the other's entries nor an entry of
     Update 208's cache (D-0336 §2.19).
2. **The lock is in the mutex's own cells (315, delegated decision).** D-0337 §3 left an uncontended `lock` at 62–73
   ns against C's 11 ns: each lock and unlock took its stripe's mutex, a hash-map insert or remove of the holder
   record and a waiters lookup, and each guard's end entered the runtime under its global lock. A redesign was the
   remaining step, which the owner had reserved and has now delegated. It is chosen on its merits: fewer than half the
   instructions uncontended, four to six times faster four-threaded on the report's shape than Update 208 on the same
   machine (about thirty times the report's own figure), and every guarantee kept.
   - The design: a mutex's `[Sizeof-Mutex]` state cells (D-0310: the state goes with the mutex's place) hold the
     holder's thread id (low 40 bits, 0 when free), a waited-for bit and the poison bit. An uncontended lock is one
     compare-and-swap from 0 to the thread's id, an unlock one compare-and-swap clearing the holder and keeping the
     poison (`[Guard-Drop]`, D-0215). A thread that must wait sets the waited-for bit in the compare-and-swap that
     finds the mutex held, under the runtime's book, and waits as before; a release that sees the bit wakes through
     the book as before, so no wake-up is lost. Each thread keeps the list of mutexes it holds in its own state, with
     no lock, for `[Fault-Contain]`'s poisoning (D-0204); a guard ended on another thread clears the locker's entry
     under the book before the cells are let go, so no entry outlives its lock. The deadlock finder and
     `[Lock-Reentrant]` read the holder from the cells. A decided program's guard ends without entering the runtime.
   - Kept: the deadlock reports (lock cycles, lock–join cycles, a lone waiter), poisoning and its reason, the
     reentrancy fault, cancellation of a waiting thread, `--explore` (the reentrancy check before the claim, as
     before), the decided and `--all-checks` forms, and Windows (atomic builtins and the existing platform layer
     only). The stripe's lock records and their maps are removed.
   - Measured: the report's lock-only loop 1.006e9 → 0.428e9 instructions (GCC and Clang), 30 ns a lock and unlock
     (was 62–73; C's 11 ns), a guard given to a function 41 ns (was 87–95); the report's locked map, one thread 178 →
     62 ms, four threads 482–716 ms (Update 208) → 116–134 ms (Go: 90–110 ms). The cost moved to the rare case: a
     guard ended on another thread takes the book's lock.
   - What separates it from C's 11 ns is the two locked instructions' latency on this machine, the held-list note
     and, where the guard is an object, its notes. No language rule changes (`spec/19` describes no implementation).
3. **A variadic C function is declared with `...` (228).** `unsafe extern fn name(τ1 p1, …, τn pn, ...) : τr;`, with
   at least one fixed parameter and `...` last, as C and Rust write it, declares a C function with a variable
   argument list (`spec/20` `[Extern-Call-Variadic]`, `spec/22` §3). A call passes the fixed arguments, checked as
   before, then any number of others, each typed by its own text (no expected type: a literal is `i32` or `f64`, so
   `widen<i64>(n)` is written where a `long` is meant), each an `FfiType` other than `void`, plain structs by value
   included since C passes them; anything else is `diag.extern-non-ffi-type` (`[Extern-Call-Variadic-Arg]`). C's
   default argument promotions happen in C, which `cobc`'s generated code calls (`f32` to `double`, `bool` and
   narrow integers to `int`); Rust's refusal of unpromoted types was not chosen, since nothing is lost when the
   caller is C. The declaration says the function is variadic, so no hidden table decides how a call is made, and one
   declaration serves calls with different lists, as `printf` needs; a marker before typed variable parameters was
   not chosen for that reason. D-0336 §2.20's table form is kept for a declaration without `...` of a listed C
   library function (the `printf` and `scanf` families, `open`, `fcntl`, `ioctl`, …), extended with `strfmon`,
   `strfmon_l`, `error`, `error_at_line`, `semctl`, `mremap` and `ptrace`; whether to deprecate it in favour of
   `...` is the owner's.
4. **Two ranges meeting at a stable local are disjoint (311).** `&mut s[0..mid]` beside `&mut s[mid..n]` never
   overlap when `mid` is one literal or one *stable* local (`spec/14` §6): declared once in the function, as a
   parameter or by a `let` outside every loop and closure body, with no other binding of its name anywhere in the
   function, and never assigned, borrowed or captured without `move`, so it has one value wherever the frame names
   it. The rule is syntactic and visible in the text; it covers adjacency only (`[a..m]` and `[m..b]`), not
   arithmetic on bounds. The report's divide-and-conquer split is now untracked (`--report-tracking`: "no run-time
   tracking"): 32.0e9 → 0.43e9 instructions at depth 2.
5. **An integer or `bool` `match` scrutinee ends a reference (114).** D-0336 §2.7's end after an `if` condition
   extends to `match (e)` with `e` of an integer type or `bool`: a binding last mentioned in `e` and in no arm ends
   once `e`'s value is read, before any arm is tried, in the checker and the run alike (`spec/14`
   `[Ref-Binding-Last-Use]`). Only those types, because their value is copied before any arm; a binder of an enum or
   text scrutinee may refer into the scrutinee's place, so those keep the old rule.
6. **A `match`'s binders decide what its scrutinee keeps (183).** In D-0318's probe-only classification a `match`
   (and `if (P = e)`) whose scrutinee is made from a parameter `q` keeps `q` only where its arms' binders are kept:
   each binder is followed through its arm as a binding declared from `q` is, every part read through it counting as
   kept, since the syntactic pass does not know the binder's type (`spec/10` `[Result-Source-Probe-Only]`). A
   scrutinee with no binder, or a binder named `q`, keeps `q` as before.
7. **A block's value may be borrowed as an argument (229).** `&{ … }` and `&unsafe { … }` of a block that declares no
   binding take the tail's shape, so a `String` one is viewed where a `StringView` is expected, as a call's result is
   (`spec/09` `[Ref-Form-Temporary-Argument]`, `spec/21` `[View-Form-Temporary-Argument]`, D-0135). Elsewhere the
   message says such a borrow is allowed only as a call's argument, and to bind the value first.
8. **A large text `match` is a sorted table (073).** A `match` on text with more than 64 arms, every arm before the
   last a distinct text literal, is lowered as two function-local constant tables (the literals in byte order, and
   each one's arm), a binary search, then the arms as before; otherwise the lowering is unchanged. The C compiler's
   cost was the long chain of comparisons (GCC: 16,000 arms 136 s in one function); now 16,000 arms build in 8.4 s
   (Update 207: 179 s) and a lookup is a search, not a scan (4,000 arms, 10^6 lookups: 33.6 s → 0.12 s). No C
   `switch`, since an arm's `break` is C's `break`. An implementation choice; no rule changes.
9. **`saslprep` follows PostgreSQL's reading (209).** Where PostgreSQL's own SASLprep (`src/common/saslprep.c`,
   run by the server on the password it stores and by libpq on the one it sends) and the letter of RFC 4013 differ,
   `std` follows PostgreSQL, since a password prepared differently cannot log in: C.1.2 mapped before B.1 (so U+200B
   becomes a space), the prohibition, unassigned and bidirectional checks on the mapped text before NFKC, an empty
   result refused, ASCII text returned as given (controls included; it was `None`, which made no login difference),
   and NFKC with Corrigendum #4's five ideograph decompositions, as every Unicode version after 3.2 (`spec/21`
   `[Pg-Connect]`, revisiting D-0336 §2.25's literal reading). Over 18 test passwords and a wrong one against the
   local PostgreSQL 14, 8 differed before and none after.
10. **`Rc` chains are destroyed iteratively in the decided form (337).** D-0337 §2.16 made a stack exhausted in a
    destructor one report; the chain itself now ends in constant stack in a decided program. While `Rc`'s
    destructor runs, an `Rc` it reaches in tail position (D-0335: what is destroyed after it only frees memory)
    does not run nested: it waits on a per-thread list, and the enclosing destructor runs the waiting ones in a loop
    when it returns. The order of observable effects is the nested order, since `Rc`'s destructor does nothing
    observable after ending its value. This is a mark on `std`'s `Rc` descriptor, not a language rule; `Arc` (which
    may end on any thread) and user resource types (whose `drop` may do anything after) keep nesting. Not during
    the program's unwind; a contained fault clears the list.
11. **Fault messages and per-call checks (065, 148).** The fault of `-a`, `a / b`, `a % b` and a shift names its
    values and type, as other arithmetic faults did since Update 207: "-(-2147483648) is outside `i32` (…)", "7 /
    0: the divisor is zero", "1 << 40: the shift amount 40 is not below 32, the width of `i32`" (new runtime export
    `cb_fault_op`, `spec/IMPLEMENTATION-NOTES.md`); the hot path is unchanged. A call of a `fn` value through a
    shared reference no longer checks a `fn` item or a closure that captures nothing (neither holds an exclusive
    capture, D-0218), and `--report-tracking` lists the checks that remain, in decided programs too.
12. **The checker refuses what it can (108, 109).** A move out of a field reached through a slice (`ps[0].name`
    through `slice<Person, shared>`) is refused as one through a `ref` is; a closure *binding* made without `move`
    and stored in a container or assigned to its element (`Vec::push(&mut fs, c)`, `fs[0] = [i]() …`) is refused as
    the literal form was (D-0336 §2.9). Both faulted at the run on every such program.
13. **cobmodel is explained (owner's request).** The guide (§19 "Every schedule, from the rules: `cobmodel`", and the
    introduction's tools) and the book (ch13 "Every schedule at once: `cobmodel`", with a tested example; ch16;
    appendix A) say why it exists (an executable model written from the specification alone, a second
    implementation of the rules, D-0219), when to use it (a surprising result, the compiler against the rules,
    learning; not for a program outside its fragment, nor as a proof) and how (`check`, `--explore`, `--witness`,
    `compare` over a directory, `statics`, `certify`, `// MODEL:` headers, the gate's unit). `spec/OVERVIEW.md` §11
    is brought up to the model as it is (its fragment, D-0326, the gate's unit, eight programs under
    `--pre-d0236`). With it, `cobmodel compare` reads `cobc --explore`'s outcome form of report 274 and the model's
    statics take report 313's row (D-0337 §2.9); without both, the gate's `model` unit would have failed.

## 3. Left, with the reason

- **059.** A large program's first build after an edit compiles all of its C again; with one file the default it is
  slower than Update 207's default on several cores (27.7 s against 22.3 s with `--lto`). What would close it is an
  object cache of `std`'s own C, compiled as a part of its own and linked without cross-part inlining: the cache
  D-0277 removed at the owner's word ("this cache is always causing issues"), after which D-0336 §2.19 chose the
  executable cache. The `std` part's text also holds every declaration of the program, so many edits would miss it,
  and without LTO the program would lose `std`'s inlining. Left for the owner; the guide says what a build costs,
  what the cache saves, and that `--lto` and `-O 0` shorten the loop.
- **073.** Clang's own passes over the arms' run stay superlinear (8,000 arms 37.5 s → 10.4 s; 16,000 35 s).
- **114.** Scrutinees of other types keep the old rule (§2.5).
- **148.** A closure that captures something, called through a shared reference, is still checked at each call:
  its captures are not in its `fn` type (D-0218), and hoisting the check needs the `fn` value known unchanged across
  iterations, which a shared reference to a `Vec` element does not make known. `--report-tracking` names each line.
- **228.** A declaration without `...` of a variadic function not in the table is a fixed prototype: `cobc` reads
  no C headers, so it cannot know; the documentation says to write `...`.
- **229.** A block that declares bindings before a `String` tail, where a view is expected, keeps the old message
  (its tail may name those bindings).
- **311.** Other computed bounds (`[0..mid]` beside `[mid + 1..n]`, a bound the function writes, `$`) may overlap.
- **337.** Under `--all-checks` the chain still nests and ends in one `diag.stack-exhausted`; a chain linked through
  `Arc` or a user resource type still nests.
- **065.** SIMD lanes and an atomic `fetch_add` overflow still name no values.

## 4. Consequences

- Behaviour that changes for building: one C file without link-time optimisation by default (§2.1); `--one-unit`
  refused with a message; the executable cache's entries of Update 208 are not reused.
- Programs newly accepted: ranges meeting at a stable local lent at once (no longer tracked, §2.4); a reference
  last read by an integer or `bool` scrutinee (§2.5); a `match` on a value made from a parameter whose binders are
  not kept (§2.6); `&{ … }` of a `String` where a view is expected (§2.7); calls of variadic C functions with `...`
  and any list of `FfiType` arguments (§2.3).
- Programs newly refused, each of which faulted at the run: a move out through a slice, a closure binding stored in
  a container (§2.12). A variadic argument that is not an `FfiType` is refused (a new form, §2.3).
- Behaviour that changes for running programs: `saslprep` follows PostgreSQL (§2.9; ASCII text with controls now
  comes back as given); a long `Rc` chain no longer exhausts the stack in a decided program (§2.10); the fault of
  `-`, `/`, `%` and shifts names its values (§2.11); a lock is faster (§2.2).
- No diagnostic id is new; `diag.extern-non-ffi-type` also comes from `[Extern-Call-Variadic-Arg]`.

## 5. Verification

See `CHG-0481`.
