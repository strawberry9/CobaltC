# D-0219 — An independent executable model of the alias and thread rules, and the text it found wanting

Status: ACCEPTED (2026-10-12; the owner handed over `private/AGENT-INSTRUCTIONS-2026-10-12.md` with its recommended
answers standing, Q1: "an independent executable model … compared with coby and cobc --explore"). Findings 7, 8,
9 and 10 below were decided by the owner on 2026-10-12 (D-0221, D-0222).
Kind: Decision (`spec/02-schema.md`)
Governed by: `CobaltC_Master_Instructions.md` §12 (safe-program guarantee), §20 (completeness and adversarial review)
Depends on: D-0018, D-0019, D-0107, D-0111, D-0133, D-0204, D-0206, D-0209, D-0212, D-0213, D-0214, D-0215, D-0218
Affects: `impl/model/` (new); `spec/05` 1.8.0, `spec/14` 1.25.0, `spec/16` 1.24.0, `spec/19` 1.15.0, `spec/20` 1.17.0,
`spec/conformance.md` 3.213.0, `spec/README.md` (the *modelled* evidence word); `impl/battery.sh` (stage 1),
`impl/tools/suites.sh`, `impl/tools/manifest.py`; six conformance cases (CHG-0258)

## Problem

Three independent reviews in a row (2026-10-06, -10, -11) found priority-1 holes in the safety argument, and the
work on the last one found another (m01, D-0218). Every one sat in one corner: a reference value carrying authority
across a boundary — into a thread, into storage, through a shared path. The tests had found none of them; reading
had. And the two implementations share their static pass (`coby`'s `typecheck.rs` is `cobc`'s front end), so the
accepted set had one implementation, not two. Nothing independent of the implementations checked what the rules
say against what the tools do.

## Options

A. **An executable model**: a separate program, written from the specification alone and sharing no code with the
tools, that runs a fragment of the language by the rules, explores every interleaving, and lists every outcome the
rules allow; generated programs are compared with the tools.
B. **A mechanised proof** of `[Standing-Local]` and `[Conc-No-Race]` for the fragment, in a proof assistant. Not
installed here, weeks of work, and a proof is about the rules, not about the tools.
C. **More review.** Each round finds holes, and nothing bounds what it does not find.

## Decision

A, in `impl/model` (`cobmodel`, Rust standard library only, no dependency on `coby`, `cobc` or `cbrt`). Its
fragment, rules and commands are in `impl/model/README.md`: the static half (typing with D-0218, the flow analysis
of `spec/14` §6 as tabulated, `rule.temporal.ref-escape`, `[Ref-Binding-Last-Use]`) and the dynamic half (`Σ` of
`spec/04` with `held-by` derived from storage; `[Read]`, `[Write]`, `[Borrow]`, `clash`, `sync-exempt`,
`[Thread-Affine]`, `[Thread-Lend]`, statement and block exits, calls, `Vec`'s bodies' effects including `[Release]`
and `foreign-held`, mutexes and poisoning, channels, `[Deadlock]`, `[Channel-Deadlock]`, contained faults), with
every interleaving explored. A program is generated, printed as CobaltC, read back, run by the model, and run by
`coby` and by `cobc --explore` under GCC and Clang; every outcome a tool shows must be in the model's set.

Each disagreement goes to one of three bins: (a) a tool bug — fixed, with a conformance case; (b) a model bug —
fixed in the model; (c) the text — an ambiguity or a gap. A (c) finding where both tools already do the evident
intent is repaired as a clarification (`spec/STABILITY.md` §2), with a case pinning it; one that would change what
is accepted goes to the owner.

## Findings (bin c)

1. **A projection path had no end.** `[Field-Access]` formed a path for `e.f` and never said when it ended; by the
   letter it lived to its statement's end, so in `p.a = p.a + 1` the target's exclusive projection clashed with the
   read's, and every such statement faulted. `spec/08` §4's lemma already assumed "it exists only for its access".
   Repaired: a projection exists for the one access made through it (`spec/16`; `conf.projection-one-access`).
2. **A returned reference parameter was invalidated.** `fn pass(ref<i32, exclusive> p) : ref<i32, exclusive> { p }`:
   the borrow passed in is held only by `p`, and `[Object-End]`'s `Dead` set invalidated it when the parameter frame
   ended, though it is the result. `unwind-to` and `[Stmt-Exit]` spare a result; `[Block-Exit]` did not. Repaired:
   `[Block-Exit]` spares its result's references from the ends it makes (`spec/05`, `spec/14`;
   `conf.returned-reference-parameter`). Sparing every path whose statement is still open — the first reading tried —
   is too wide: it would keep `read(&x)`'s argument borrow alive through `put(&mut x, read(&x) + 1)` (D-0107).
3. **The early end at a last use was missing from the flow table**, and `[Ref-Binding-Last-Use]` did not say that a
   reference lent to a thread is "borrowed through". Repaired: a row for a binding ended at its last use (as block
   exit, for that binding), and the lend named as a borrow through `x` (`spec/14`;
   `conf.ref-lent-to-thread-lives-to-join`).
4. **A received value was exposed to the sender's statement exit.** A lent path's statement scope is the sender's;
   between `recv` taking it from the channel and the receiver binding it, nothing held it, and the sender's next
   statement exit would invalidate it — a stale fault in the receiver, depending on timing. `[Join]` re-stamps a
   result into the joiner's scope; `[Recv]` did not. Repaired (`spec/19`; `conf.channel-lent-ref-received`).
5. **A vector of references lost them when it grew.** `[Copy-Raw]` copied cells but gave a copied reference no
   holder, so `Vec::grow`'s `[Deallocate]` of the old buffer ended the only objects holding them: by the letter, the
   fifth push into a `Vec<ref<…>>` invalidated every reference in it. Repaired: a copied reference is held by a
   reclaimed object over its new cells, as `[Rawptr-Write]` establishes one (`spec/20`; `conf.vec-of-refs-grown`).
6. **`*v[k] = e` for a vector of exclusive references.** `[Index-Vec]` reaches `v[k]` by `index_exclusive` "where
   the place is written"; in `*v[k] = e` the place written is `*v[k]`, and `v[k]` read by `index_shared` would, under
   D-0218, yield a shared reference and reject the write. Both tools reach it exclusively. Repaired: the written place
   covers the operand of `*` in such a position (`spec/16`; `conf.vec-of-exclusive-refs-write-element`).
7. **`[Closure-Call]` and calls through a shared reference — decided: option (i), D-0221.** `spec/15` §6 says a call borrows
   the closure exclusively, so "a closure cannot be called while another borrow of it is live". Both tools accept a
   call of a closure while a shared borrow of it is live — one that only reads its captures, and one that writes
   them — and both accept a call through `ref<fn(…), shared>` of a closure without an exclusive capture, which D-0218
   (`[Read-Fn-Through-Shared]`) and the case `impl/conformance/15-function-semantics/closure_binding_borrowed_as_fn_reference_ok.cb` assume. By the letter each such
   call is `[Borrow-Exceeds-Source]` or `[Borrow-Denied]`. The tools' behaviour looks sound — D-0218 already keeps a
   writing closure from being called through a shared reference — but it accepts programs the text rejects, so the
   repair is the owner's call. Options: (i) the text follows the tools: a call borrows the closure shared, and the
   exclusive capture paths carry the writes (recommended); (ii) the tools follow the text. Closures are not modelled
   until this is settled.

8. **`[Ref-Binding-Last-Use]` as the implementations apply it — decided: option (i), D-0221.** The rule ends a binding
   after its last mention, or, when a binding that may hold a reference shares a statement with it and is used
   later, at its block's exit. Both tools instead *defer* such an end: the binding ends after the first later
   statement once no such binding is mentioned any more, and whether another binding's borrow is rooted in it
   is judged by the flow facts at that point, not over the whole block. So
   `auto r1 = &mut y; Hold h1 = Hold { .r = r1 }; *r1 = *r1 + 1; printf("%v\n", y);` runs under both tools
   (`h1` ends after the third statement) and faults by the text (`h1` holds `r1`'s path to its block's end).
   The deferral looks sound — a binding never mentioned again is ended later than its last use, never
   earlier — but it accepts programs the text does not. Options: (i) the text follows the tools (recommended:
   it is what programs have been written against since D-0111); (ii) the tools follow the text. The model
   applies the tools' reading by default and the text's with `--literal-last-use`.
9. **A shared reference to a container does not freeze its elements — decided: option (i), D-0222.** With
   `auto r = &mut w; auto s = &*r;` live and `w : Vec<i32>`, `set(r, 0, 9)` (a function writing
   `*Vec::index_exclusive(v, i)` through its parameter) writes `w[0]` while `s` lives, and `s` then reads the new
   value. No rule is broken: passing `r` on forms no borrow, `index_exclusive` reads the vector through it (a
   shared access, permitted beside `s`), and the element is an object of its own, so no `clash` arises. Both tools
   and the model agree. But a function given `s` and `r` as two parameters sees what its shared parameter reaches
   change — what D-0218 and Master Instructions §17 say a shared reference rules out. Memory safety and
   `[Conc-No-Race]` are not affected: the element's own accesses are checked. Options: (i) `index_exclusive`
   (and the other element-writing bodies) take the container exclusively (`&mut *v`), so a live shared path to it
   clashes — closes it, costs a check per call where `cobc` cannot elide it; (ii) state the limit: a shared
   reference to a container freezes the container, not its elements against its exclusive parent — and say so in
   `spec/08` and the guide; (iii) a container rule in `clash`: a path to a reclaimed element counts as reaching
   its container. Recommended: (i), measured first.

10. **`Vec` element places in the flow analysis — decided: option (ii), D-0221.** The table's places include `[i]` for a
    literal index, and `[Index-Vec]` checks the borrow `v[k]` forms "as an access to e's element k", so
    `auto r1 = &mut v[0]; auto r2 = &mut v[0]; *r1 = 2;` and a read of `v[0]` while `r1` lives are refuted. Both
    tools record a borrow of `v[k]` as a borrow of all of `v` — so `Vec::push(&mut v, …)` or `&v` while it lives is
    refuted — and never compare one element access with another statically: those conflicts fault when they run.
    Options: (i) the tools follow the text — more programs rejected, including any with a non-literal index while an
    element borrow lives (a non-literal index overlaps every index); (ii) the text follows the tools: element-level
    conflicts are left to the run, and a borrow of an element counts against the container (recommended: it rejects
    nothing the tools accept, and the dynamic check is exact). The model follows the tools.

## Tool defects found (bin a)

- **`cobc`, priority 1 (soundness): an argument passed on whole was not checked where it arrived.** `cobc` runs a
  function whose reference parameters are only read and written (D-0188 dual bodies, D-0209 direct parameters)
  by an unchecked body when a test at entry passes. The direct dispatcher skipped its test whenever the thread
  was alone, and the dual-body test (`cb_paths_disjoint`) compared the arguments only with each other. Both
  assumed "the caller formed and checked the borrow" — false for an existing reference passed on whole
  (`bump(r)`), which forms no borrow and may have a live reborrow. A shared reference then saw its referent
  change while it lived: `auto r = &mut x; auto s = &*r; bump(r); *s` printed `2` under `cobc` where `coby` and
  `cobc --all-checks` fault. Fixed: both tests now require each argument to be clash-free as it arrives
  (`cb_direct_ok`, `cb_paths_disjoint`, `access_clashes` in `cbrt`), as `spec/08` §4's H3 says; the reclaimed-cells
  test still runs only while other threads do. Cases: `conf.reference-passed-on-while-reborrowed` (both routes).
- **Both tools (the shared static pass): a read through `*r` was never refuted statically.** The discharge
  table's row for a place rooted at `*q` applies to reads as to writes; `typecheck.rs` checked writes through
  `*r` and left reads to the run, where they faulted. Fixed: the read is a `[Read]` of `*r` for the analysis
  (`conf.read-through-reborrowed-reference`). A program whose conflicting read through `*r` is in a branch that
  never runs is now rejected, as the text always required.

- **Both tools: a deadlock was looked for only when a wait began.** `[Deadlock]` (b) is judged over the threads
  still running; when a thread ended after the last wait had begun, every remaining thread waited and the program
  hung (about one run in three of a program whose short thread finished late). Fixed: the check also runs when a
  thread ends or fails (`find_deadlock` in `coby`; `cb_thread_done`, `cb_thread_failed` in `cbrt`)
  (`conf.deadlock-after-last-thread-ends`, which hangs under the tools of Update 102).
- **Both tools: the main thread at the end of a chain of waits.** When `main` and a spawned thread were both ends
  of a chain (each waiting on a channel), the spawned thread's contained fault could leave `main` the only running
  thread before `main` woke, and `main` then reported `[Channel-Deadlock]` instead of its own `diag.deadlock`.
  Fixed: a thread `[Deadlock]` has marked faults with it first (`conf.deadlock-main-end-of-chain`).
- **Both tools, after the fix above: the main thread waiting alone on a channel** once its last other thread ended
  was marked by `[Deadlock]` (b) and reported `diag.deadlock`; `spec/19` §3 makes that `[Channel-Deadlock]`. Fixed:
  the deadlock search leaves a lone main thread's channel wait to its own rule.
- **`coby`: D-0049's reborrow was missing from ordinary calls.** An exclusive reference passed to a shared
  parameter was bound as it was; a conflict with a live exclusive reborrow surfaced at a later access inside the
  callee, at a line `cobc` (which reborrows at the call) did not report. Fixed: the binding reborrows shared, with
  `[Borrow]`'s check, at the call (`conf.shared-param-given-reborrowed-exclusive`).
- **`coby`: a fault raised by a block's exit had no location** when the block's last statement ended inside a
  call into `std` (a deadlock in a handle's destructor after a `printf`): it is now reported at the block's last
  line of the program's own code, as `cobc` reports it (`conf.deadlock-at-block-exit-located`).

## Results

- **The model**: `impl/model`, about 4,000 lines of Rust, standard library only; it builds on Windows and with
  Rust 1.77.2. The alphabet of `src/enumerate.rs` gives 23 one-statement programs, 589 of up to two statements,
  and 17,332 of up to three, all of which the model runs; generated programs mix references, structs holding
  them, vectors of integers and of exclusive references, mutexes, channels of exclusive references and threads.
- **The comparisons** (`coby`, and `cobc --explore 4` with GCC and Clang, against the model's set): rounds of 450,
  3,000, 589, 3,000 and 2,001 programs while the model and the tools were being corrected, then the final sets —
  every program of up to two statements, a selection of 3,000 of the three-statement ones (all with more than one
  outcome or a dynamic fault, then an even sample) and 2,001 random programs of six statements — after the last fixes, no program in any of them had a tool outcome outside the
  model's set (23, 589, 3,000 and 2,001 programs; 0, 0, 5 and 1 model outcomes no tool showed).
  The tools' outcomes are always within the model's set; a model outcome no tool showed (a handful, each a
  schedule the tools did not try) is advisory.
- **Findings in the text**: 1–6 repaired as clarifications (CHG-0258), each with a case; 7, 8 and 10 settled the
  implementations' way (D-0221), 9 by a soundness repair (D-0222).
- **Defects in the tools**: six, all fixed with cases (above). The soundness one (`cobc`'s arrival test) is the
  first defect the conformance suite and the differential campaigns had not found.
- **The model's own bugs** found by disagreement and fixed: last-use typing and scoping, the refers-to and
  lent-through conditions, sibling-block names, the result exemption's scope (finding 2's first reading was too
  wide), the deferred last-use order, containment of lent threads, destructors in a contained unwind, and the
  block-exit reporting line.
- **Cost of the `cobc` repair** (instructions against Update 102, `perf`): `bench` unchanged except `strings`
  +1.5%, `tree` +3.1% and `hashcount` +3.9%; `stress/perfguide` within 0.5% except `e06` c +3.4% and `e11` +2.5–2.8%.
  A first form that always tested arrival cost `tree` +65%; the `__arrive` twins confine the test to calls that
  pass an existing reference on.
- **In the battery**: stage 1 compares `impl/model/sample` (254 programs: the hand cases, every program that
  disagreed, the one-statement set and 180 of the three-statement selection) with both tools; the manifest says
  *modelled* when that comparison is clean.
