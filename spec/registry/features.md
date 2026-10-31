# CobaltC Feature Registry

Status: normative artifact
Version: 1.5.0
Conforms to: `spec/02-schema.md` (Kind: Feature, `feat.<name>`)
Governed by: `CobaltC_Master_Instructions.md` §18

## Purpose

Substantial feature proposals and their status. Only `ACCEPTED`
entries are part of the language; the full §18 field list is
recorded in the decision each entry cites, not repeated here
(Master Instructions §14).

## `feat.static-lifetime-tracking`
**Status:** ACCEPTED (scope: lexical escape, D-0005; single-parameter elision, D-0011; container-bound dynamic lifetime, D-0018)
**Depends on:** inv.temporal-validity, D-0005, D-0011, D-0018

## `feat.explicit-lifetime-parameters`
**Status:** REJECTED (`CHG-0022`: option (c) retained permanently —
`[Call-Multi-Ref-Return-Rejected]` stays the boundary; (a)/(b) rejected
— owner-delegated decision, Master Instructions §9/§7)
**Motivating capability:** a function with zero or several
reference-typed parameters returning a reference, statically checked
at every call site — e.g. `fn longer(ref<String,shared> a, ref<String,
shared> b) : ref<String,shared>` (pick whichever input to alias) or
`fn first_byte(ref<Vec<u8>,shared> v) : ref<u8,shared>` re-derived
without relying on `index_shared`'s one-argument shape (zero reference
parameters: the returned reference's origin is a *field or constant*,
not any parameter). **Semantic problem:** `rule.temporal.elision`'s
`[Call-Elided-Lifetime]` only knows which caller object a returned
reference depends on when there is *exactly* one reference parameter;
with zero or several, `rule.temporal.ref-escape`'s static rule has no
argument to name, and `[Call-Multi-Ref-Return-Rejected]` refuses the
declaration outright rather than fall back to `unknown`/dynamic (the
one static rule in the corpus that rejects the *declaration*, not a
call site, closing off the dynamic baseline too). **Examples
demonstrating need:** none yet in `spec/21` — every one of `Vec`,
`String`, `Rc`'s reference-returning functions (`index_shared`,
`index_exclusive`, `Rc::get`) has exactly one reference parameter, by
construction; the two illustrations above are hypothetical, not drawn
from an existing conformance case. **Affected invariants:**
`inv.temporal-validity` (which caller object a multi-parameter or
zero-parameter return depends on). **Required semantic state and
transitions:** a way to name, per reference parameter, which are
"the same lifetime" as the return — i.e., a partition of the reference
parameters (and the return) into equivalence classes, checked at the
call site against which argument(s) the escape analysis must track.
**Existing mechanisms considered:** D-0011 single-parameter elision
(by design insufficient — the problem this feature would solve).
**Alternative mechanisms:** (a) named lifetime parameters, syntax cost
`<'a>`-shaped annotations on the function and each relevant parameter/
return type (the Rust-shaped answer, explicit but well-understood
prior art); (b) reject only the case where the returned reference's
origin is *genuinely ambiguous among the parameters* and accept the
zero-parameter case unconditionally (a reference into `'static`/a
field never needs a caller-side check at all) — smaller, but requires
distinguishing "returns a reference independent of every parameter"
from "returns one of several, ambiguously," which is itself dataflow
inside the function body, not just its signature; (c) leave
`[Call-Multi-Ref-Return-Rejected]` as the accepted permanent boundary
(the `ABSENT`-by-default status quo) and require restructuring call
sites to single-reference-parameter helpers, composed by the caller.
**Library/inference/generation/tooling alternatives:** none identified
that avoid new syntax for genuinely ambiguous cases; option (b) above
needs no new syntax for the *unambiguous* zero-parameter case only.
**Smallest viable mechanism:** not derived — depends on which of (a)/
(b)/(c) the owner selects; (b) is smaller than (a) but does not cover
the two-or-more-ambiguous-parameter shape at all, which would remain
`ABSENT`. **Syntax/conceptual/interaction/specification cost:**
`(a)` is the highest on every axis (new binder, new type-parameter-like
position, interacts with generics and `rule.type.kind`); `(b)` is
low-cost but partial. **Future implementation cost:** `(a)` requires
solving a lifetime-parameter unification/substitution problem the
current design has never needed. **Diagnostic cost:** a new
diagnostic family either way (today's single
`diag.lifetime-elision-ambiguous` would need to become several,
naming which parameters were considered). **Runtime cost:** none (this
is a purely static refinement; D-0018's dynamic baseline already
covers every case safely, just without the static proof). **Learning
cost:** `(a)` reintroduces exactly the lifetime-annotation burden
Master Instructions §7 says not to assume a solution needs; `(b)`/`(c)`
add none. **Compatibility burden:** none — nothing currently accepted
would change meaning. **Safety implications:** none either way (the
dynamic baseline is already sound for every rejected case). **Prior-
art status:** `(a)` is directly Rust-shaped; `(b)`/`(c)` are not
modeled on prior art. **Decision rationale:** no construct in
`spec/21` currently forces this — the two illustrations above are
constructed, not encountered — so `UNDER_INVESTIGATION` remains
appropriate pending either a real need or an owner decision to close
the gap pre-emptively.
**Depends on:** inv.temporal-validity, D-0011, rule.temporal.ref-escape, CHG-0022

## `feat.user-defined-generics`
**Status:** ACCEPTED (D-0010; inference D-0012/D-0013/D-0014)
**Depends on:** rule.type.kind, D-0010

## `feat.closures`
**Status:** ACCEPTED (`rule.fn.closure`; lifetime of captures per D-0018)
**Depends on:** rule.fn.closure, D-0018

## `feat.disjoint-field-borrows`
**Status:** ACCEPTED (D-0018: `clash` is by target overlap; two exclusive borrows of disjoint fields coexist)
**Depends on:** inv.alias-validity, D-0018

## `feat.nested-patterns`
**Status:** ABSENT — recorded so the absence is deliberate (`spec/22` §5 item 2).

## `feat.trait-bounds`
**Status:** ABSENT — D-0010's named limitation; would be a separately derived feature.

## `feat.atomics`
**Status:** ABSENT — `spec/19` covers shared mutation with `mutex` only.

## `feat.minimal-io-extern-surface`
**Status:** ACCEPTED (`CHG-0020`: option (a) — one minimal `extern fn`
declared in the prelude; owner decision, not derived from the
invariants)
**Motivating capability:** `rule.fn.program`'s "observable behavior"
is defined as the termination outcome plus the sequence of `extern`
calls a program performs (`spec/15` §7) — but `spec/21` declares no
`extern` function at all, so today no CobaltC program can actually
produce an observable `extern` call; every conformance case's
"outcome" is a value or diagnostic an implementer must trust the
formalism for, never something a real compiled/run program could print
or a test harness could capture from outside the process.
**Semantic problem:** none — this is a completeness-of-surface
question, not a soundness one; every existing rule is unaffected
either way. **Examples demonstrating need:** none in the corpus;
motivated by `Master Instructions` §26's success criterion
("independent implementer... without unstated human intent") being
harder to verify end-to-end when no conformance case can be *run* and
observed, only derived by hand. **Affected invariants:**
`inv.trust-transition` (an `extern` call's result is exactly the kind
of unchecked claim that invariant already governs — `write` would add
no new invariant, only a new trust-boundary *instance*).
**Required semantic state and transitions:** none beyond what
`rule.trust.extern-call`'s `[Extern-Call]` already provides — a
minimal surface (e.g. `extern fn write(rawptr<u8> buf, usize len) :
isize;`, or similar, matching `FfiType`) would be declared in
`spec/21` §0 exactly like `allocate`/`deallocate` are, with no new
rule. **Existing mechanisms considered:** `rule.trust.extern-call`
already defines everything needed for *any* `extern` signature within
`FfiType`; the only missing piece is a *specific, named* one in the
prelude that every conformance case could route an observable value
through. **Alternative mechanisms:** (a) declare one minimal `extern
fn` in `spec/21` §0 (as above); (b) leave `extern` entirely
user-declared per program (already possible — any program can already
write its own `extern fn` and call it, this feature only asks whether
the *prelude* should provide one so the *corpus itself* can use it
uniformly); (c) stay out of scope permanently, accepting that
conformance is verified by hand-derivation only, never by execution —
matches `Master Instructions` §1's "design AI is not authorized to...
build an executable toolchain," which a *prelude* `extern` declaration
does not itself violate (it is still just a signature; nothing in this
specification would execute it) but which is the likely reason no one
has proposed this before. **Library/inference/generation/tooling
alternatives:** none — this is inherently an interface addition, not
something inference or tooling substitutes for. **Smallest viable
mechanism:** (a), one `extern fn` declaration, zero new rules.
**Syntax/conceptual/interaction/specification cost:** minimal — the
grammar and rules already fully support it; only `spec/21` §0's table
gains one row. **Future implementation cost:** an implementation must
provide *some* real OS binding for whichever signature is chosen,
which is exactly the kind of "calling convention... outside this
specification" `rule.trust.extern-call` already disclaims responsibility
for. **Diagnostic cost:** none new. **Runtime cost:** none beyond the
call itself. **Learning cost:** negligible (one more prelude entry).
**Compatibility burden:** none (purely additive). **Safety
implications:** none (the trust boundary is already fully specified;
this only gives it a named instance). **Composability implications:**
none. **Prior-art status:** every systems language provides *some*
minimal write-to-descriptor primitive; CobaltC currently provides the
mechanism (`extern`) but no instance. **Decision rationale:** genuinely
a scope question, not a design-completeness one — Master Instructions
§1 scopes this specification to language design, and whether the
*design* should include a standard minimal I/O surface (as opposed to
leaving every program to declare its own) is a project-value choice
for the owner, not something derivable from the invariants alone.
**Depends on:** inv.trust-transition, rule.trust.extern-call,
rule.stdlib.prelude, term.conformance, CHG-0020

## `feat.str-literal`
**Status:** ACCEPTED (`CHG-0025`, D-0020: a non-resource text value
type `str` with a `"…"` literal, a `b"…"` byte-array literal, and a
safe prelude `print`; human-directed)
**Motivating capability:** writing text in a program. Before this
feature the only spelling of `"hello\n"` was `[104:u8, 101:u8, 108:u8,
108:u8, 111:u8, 10:u8]` and the only way to emit it was a raw pointer
into that array passed to `write` inside `unsafe` (`ex.extern-write`,
`impl/cobaltc_examples/01_hello.cb`) — correct, but unreadable to a
human, and it forced every program that prints anything to write
`unsafe`. **Semantic problem:** what a string literal *denotes*. The
obvious answers each collide with an accepted decision: a
`ref<…,shared>` into static data is exactly the zero-reference-parameter
return shape `[Call-Multi-Ref-Return-Rejected]` forbids and `CHG-0022`
kept forbidden; an owned `String` literal would make every constant a
resource — allocated, moved, destroyed, tracked by `rule.resauth.*`
and `rule.control.flow-analysis` — for something that is semantically
a constant. **Examples demonstrating need:** `ex.extern-write`; both
byte-array-spelled greetings in `impl/cobaltc_examples`.
**Affected invariants:** `inv.str.utf8-validity` (new, established
statically by the literal grammar); `inv.string.utf8-validity` gains
a second establishment path (`String::from_str`) that inherits the
first. **Required semantic state and transitions:** none in `Σ` — a
`str` is a value (`term.value`), not an object; `[Str-Ptr]` widens
`state.storage` with the value's bytes outside every object's extent,
the same shape `[Rawptr-Of]` already has. **Existing mechanisms
considered:** `array<u8,N>` literals plus `write` (the status quo);
`rule.type.expected`'s literal typing (an untyped literal taking
`String`/`Vec<u8>`/`array<u8,N>` from context — the most
CobaltC-idiomatic sugar, but still bytes at the type level and still
a resource wherever it becomes a `String`). **Alternative
mechanisms:** (a) character literals only; (b) `"…"` as sugar for a
`u8` array; (c) `"…"` as `Vec<u8>`; (d) `"…"` as an owned `String`
validated lexically; (e) an untyped literal typed by expected type;
(f) a copyable non-resource text value type — **selected**; (g) no
literal, tooling renders byte arrays as text. D-0020 records each
verdict. **Library/inference/generation/tooling alternatives:** (g)
only, rejected: it concedes the language is unreadable and fixes the
editor instead. **Smallest viable mechanism:** (f) with three
intrinsics (`str_len`, `str_byte`, `str_ptr`), one library
constructor (`String::from_str`), one ordinary function (`print`), and
(b) spelled `b"…"` as pure `[Array-Construct]` sugar for the byte
path. **Syntax/conceptual/interaction/specification cost:** two
lexical productions with escape sets, one reserved type-name (`str`;
no program in the corpus used it as an identifier), one `Value`
alternative, one `[Sizeof-*]`/`[Repr-*]` pair, one `[Eq-*]`, one
typing rule, one invariant, one `spec/21` section. **Future
implementation cost:** a compiler places literal bytes in read-only
program data and represents a `str` as an address/length pair; an
interpreter interns them. **Diagnostic cost:** none new —
`str_byte` reuses `diag.index-out-of-bounds`, `extern fn f(str)`
reuses `diag.extern-non-ffi-type`, the orderings reuse
`diag.type-mismatch`; an ill-formed literal is a parse failure.
**Runtime cost:** `String::from_str` allocates and copies once,
explicitly; a `str` itself never allocates. **Learning cost:** one
new primitive type; the "it is a value like an integer, not a
reference and not a resource" rule is the whole story. **Compatibility
burden:** additive; `str` becomes reserved. **Safety implications:**
none new — `str_ptr` is safe for the same reason `rawptr_of` is (an
address is not a dereference); `print` contains the only `unsafe`.
**Composability implications:** `Vec<str>`, `Option<str>`, struct
fields of type `str`, and `str` parameters/returns all fall out of
`is-resource(str) = false` with no special case; `spawn` can pass a
`str` (a plain value) to a thread. **Prior-art status:** Rust's
`&'static str` is the same value (address, length, `Copy`) expressed
through its lifetime system, which CobaltC deliberately lacks
(`CHG-0022`); C's string literal is an array with static storage
duration. **Decision rationale:** D-0020 — a literal must be a value,
not a resource and not a reference; every other candidate is either
still bytes or still a resource. Chosen by the human owner from the
seven candidates.
**Depends on:** type.str, inv.str.utf8-validity, rule.stdlib.str,
rule.agg.array-construct, rule.type.is-resource, D-0020, CHG-0025

## `feat.consolidate-join-typing`
**Status:** REJECTED (`CHG-0023`: `[T-Join]` is not redundant — settled,
not merely assessed; see the record for the structural argument)
**Motivating capability:** none — a *simplification*, not a new
capability: `CHG-0008` left open whether `[T-Join]`
(`Γ ⊢ e : handle<τ> ⇒ Γ ⊢ join(e) : τ`, `spec/12`) is fully redundant
with ordinary `[T-Call]`/`[Generic-Call-Inferred]` typing applied to
`join`'s own documented prelude signature, `join<T>(handle<T> h) : T`
(`spec/21` §0). **Semantic problem:** if `join` were resolved and typed
exactly like any other generic function — `T` inferred from the
argument's `handle<T>` shape (`[Generic-Call-Inferred]`, D-0012 §3c),
then ordinary `[T-Call]` applied to the substituted signature — the
result is `join(e) : τ` for `e : handle<τ>`, which is syntactically
identical to `[T-Join]`'s own conclusion. Tracing it through by hand
finds no case where the two diverge. **What blocks confirming this
outright:** whether a prelude intrinsic like `join` is actually
registered in `state.items` (`spec/04` §1: `QualifiedName ⇀ Item`)
with a real `type.fn` the ordinary `callable`/`[Callable-Fn]` machinery
can see, or whether intrinsics are resolved through some other,
unspecified path that bypasses `Σ.items` entirely (unlike `spawn`,
whose variable arity means it provably *cannot* be typed by ordinary
generic-call inference, which is why `[T-Spawn]` is not in question
here). `rule.module.resolve` (`spec/17`) and `spec/21` §0's own framing
("in scope unqualified") do not settle this either way. **Existing
mechanisms considered:** `[T-Call]` + `[Generic-Call-Inferred]` (the
candidate replacement). **Alternative mechanisms:** keep `[T-Join]` as
a dedicated rule for symmetry with `[T-Spawn]`, even though only one of
the two is load-bearing. **Smallest viable mechanism:** removing
`[T-Join]` and relying on ordinary generic-call typing, *if* intrinsics
are confirmed to be ordinary `Σ.items` entries — this would be a net
simplification (Master Instructions §9) with no loss of precision.
**Specification cost:** negative (removes one rule) if confirmed
redundant; zero otherwise (nothing changes). **Compatibility burden:**
none either way — no observable behavior depends on which rule is
cited. **Decision rationale:** whether intrinsics are `Σ.items`
entries is a modeling choice this pass found no existing rule
answering either way, and settling it might affect more than just
`join` (e.g. every other prelude intrinsic's own typing derivation);
that is broader than a simple redundancy check, so it is prepared here
rather than decided.
**Depends on:** rule.type.typing, rule.fn.generic-call, rule.conc.join, D-0010, D-0012, CHG-0023

## `feat.module-file`
**Status:** ACCEPTED (`CHG-0026`, D-0021: a `module` declaration
whose body is a file, `module m "./m.cb";`; human-directed)
**Motivating capability:** a program spanning more than one source
file. Before this feature the only spelling of a module was inline,
so every item of a program lived in one text and the reference
interpreter took one file; reusing code across programs meant copying
it. **Semantic problem:** which construct spells the assembly, and
what it may add. `term.program` is already "a finite set of items in
one root module" with no word about files, so the question is purely
one of syntax — unless the mechanism is attached to `import`, which
today "introduces no binding and no `Σ` structure"
(`rule.module.use`) and would stop being true. **Examples
demonstrating need:** D-0021's Usability section (a two-file
`geometry.cb`/`main.cb` program); `impl/cobaltc_examples/` has no
multi-file example because none was expressible. **Affected
invariants:** none — modules introduce no invariant (D-0017), and
this is an equivalence over syntax. **Required semantic state and
transitions:** none in `Σ`; `Σ_0.items` is still "the program's
declarations." **Existing mechanisms considered:** inline
`module m { … }` (the status quo; hand concatenation of files is this
minus the name); `import` (rejected as the carrier: it would gain
`Σ`-changing semantics and the importer's chosen name would be
renaming, a `spec/22` §5 absence). **Alternative mechanisms:** (a)
`import … from file("…")`; (b) a conventional mapping `module m;` →
`m.cb`; (c) a nameless textual include; (d) separate compilation and
linking; (e) a bodiless `module m "p";` equivalent to the inline form
with the file's items as body — **selected**. D-0021 records each
verdict. **Library/inference/generation/tooling alternatives:** a
build tool that concatenates files before `coby` sees them — rejected:
it fixes the toolchain instead of the language, and loses the module
name and visibility boundary. **Smallest viable mechanism:** (e), one
grammar alternative, one rule stated as an equivalence, three static
rejections; no keyword (after `module identifier` only `{` or a
string literal can follow). **Syntax/conceptual/interaction/
specification cost:** one production alternative, one rule, three
diagnostics, one `spec/17` section; conceptually, "the declaring site
names the module and the file's top level is anonymous" — the fact
the inline form already has. **Future implementation cost:** a
pre-parse splice of the file's text in place of the declaration,
which `impl/src/lib.rs` already does for the prelude, plus a
line-range-to-file table for diagnostic locations; parser, resolver,
type checker, and evaluator untouched. **Diagnostic cost:** three new
static diagnostics with concrete provenance; every failure inside the
loaded module keeps its existing name. **Runtime cost:** none; the
mechanism is entirely static. **Learning cost:** one line, and the
same lookup rules as before. **Compatibility burden:** additive; the
form did not parse under `spec/22` 2.9.0. **Safety implications:**
none — no `Σ` change, no invariant, no `unsafe` interaction
(`spec/17` §4 is unchanged: no module-level gating). **Composability
implications:** a file-backed module nests inside inline modules and
vice versa; a file can itself declare file-backed modules, with paths
resolved against its own directory, so a library moves as a unit;
`export`, qualified paths, and `import` work across the boundary
unchanged. **Prior-art status:** Rust's bodiless `mod foo;` /
`#[path]` (the selected shape); C's `#include` (alternative (c));
C++20's build-system-mapped named modules (alternative (b)).
**Decision rationale:** D-0021 — a file-backed declaration is
spelling, not meaning: defined by equivalence to a form that already
has a meaning, so nothing semantic is touched; the one construct that
could have acquired new semantics (`import`) is exactly the one the
constraints closed. Chosen by the human owner, who also directed the
bare-literal spelling over `from` and `file(…)`.
**Depends on:** rule.module.file, rule.module.resolve, rule.module.visibility, term.module, term.program, D-0021, CHG-0026

## Change Log

- 1.5.0 — `CHG-0026` (D-0021): `feat.module-file` added, `ACCEPTED`
  — a `module` declaration whose body is a file; directed by the
  human owner.
- 1.4.0 — `CHG-0025` (D-0020): `feat.str-literal` added, `ACCEPTED`
  — the `str` text value type, `"…"`/`b"…"` literals, and `print`;
  decided by the human owner from the candidates D-0020 records.
- 1.3.0 — The two remaining `UNDER_INVESTIGATION` entries decided by
  the human owner (delegated to the design agent): `feat.explicit-
  lifetime-parameters` → `REJECTED` (`CHG-0022`, option (c) retained
  permanently); `feat.consolidate-join-typing` → `REJECTED`
  (`CHG-0023`, `[T-Join]` proven not redundant, not merely left
  unresolved). No `feat.*` entry remains `UNDER_INVESTIGATION`.
- 1.2.0 — `CHG-0020`: `feat.minimal-io-extern-surface` decided by the
  human owner — `ACCEPTED`, option (a) (one minimal `extern fn`
  declared in `spec/21` §0's prelude).
- 1.1.0 — `CHG-0017`: `feat.explicit-lifetime-parameters` expanded to
  the full §18 field set; `feat.minimal-io-extern-surface` and
  `feat.consolidate-join-typing` added — three open
  design items prepared for the human owner, none decided
  (`Master Instructions` §18/§22).
- 1.0.0 — Statuses reconciled with D-0018; `feat.disjoint-field-borrows` recorded as resolved; three deliberate absences registered as `ABSENT` (`spec/AUDIT-2.md` B-19).
- 0.2.0 and earlier — superseded.
