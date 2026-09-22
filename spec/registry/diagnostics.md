# CobaltC Diagnostic Registry

Status: normative artifact
Version: 1.26.0
Conforms to: `spec/02-schema.md` (Kind: Diagnostic, `diag.<name>`)
Governed by: `CobaltC_Master_Instructions.md` §19

## Purpose

Every `diag.*` id any rule produces, with the fields §19 requires.
Every entry has **Status:** ACCEPTED. Conventions: **Phase** is
*static* (a `disposition: rejected` rule, or a `checked` rule whose
guard `rule.control.flow-analysis` refuted), *dynamic* (a `checked`
rule's runtime guard), or *both*. **Location** is, unless stated, the
expression the triggering rule's premises were evaluated against.
**Related** cites `ex.*` (`spec/examples.md`).

## Names and scoping

- `diag.unbound-name` — Phase: static. Rule: `rule.value-object.binding-lookup` `[Binding-Lookup-Unbound]`, `rule.module.resolve` `[Resolve-Unbound]`. Invariant: none. Required: the name resolves to a binding or item. Observed: it does not. Provenance: use site vs. every declaration in scope. Repair: declare it, fix the spelling, or add an `import` (`import std;` for the standard library, `CHG-0033`). Related: `ex.unbound-name`.
- `diag.ambiguous-name` — Phase: static. Rule: `rule.module.resolve` `[Resolve-Ambiguous]`. Required: exactly one candidate. Observed: several (two `import`s, two imported modules exporting the name, two enums of the same module with the variant, or, where no module of the program's own has it, two imported enums with it). Provenance: the candidate declarations. Repair: qualify the name.
- `diag.name-not-visible` — Phase: static. Rule: `rule.module.resolve` `[Resolve-Not-Visible]`; `rule.module.visibility` `[Field-Not-Visible]` (a field named by `e.f`, a struct literal, or a destructuring). Required: `visible(M', name, M)`. Observed: private. Provenance: the declaration's `export` marker vs. the use site's module. Repair: mark `export` or move the reference.
- `diag.no-main` — Phase: static. Rule: `rule.fn.program` `[Program-No-Main]`. Invariant: none. Required: exactly one non-generic, non-`extern` `main` at the root module, with no parameters, returning `void` or `u8`. Observed: none there (a `main` inside a module is the ordinary item `m::main`), or one with parameters, another return type, type parameters, or `extern`. Provenance: the root module's items. Repair: declare `fn main()` or `fn main() : u8` at the root (arguments come from `arg`, `spec/21` §2c), or run the file that has one — a file-backed module's body is not a program (`spec/17` §5). Location: the root `main`, if there is one; otherwise the program as a whole.
- `diag.const-not-constant` — Phase: static. Rule: `rule.module.const` `[Const-Not-Constant]`, `[Const-Cycle]` (`spec/17` §1a). Invariant: none. Required: a `const`'s type is not a resource, its initializer is a constant expression (literals, operators, other constants, struct, array and variant literals of constant expressions, a function's name, the pure intrinsics listed there), and no constant depends on itself. Observed: one of these fails. Provenance: the `const` declaration. Repair: compute the value in a function instead, or break the cycle. Location: the `const`.
- `diag.format-invalid` — Phase: static. Rule: `rule.stdlib.format` `[Format-Invalid]` (`spec/21` §2f). Invariant: none. Required: the format of `printf`, `sprintf`, `eprintf` or `String::appendf` is a string literal whose every `%` begins a specifier `%[flags][width][.precision]conversion` with a conversion of `d i u x X o b f F e E g G s v` (or is `%%`), and flags the conversion takes. Observed: a format that is not a literal, a `%` at the end, an unknown conversion, a flag the conversion does not take (`%#d`, `%0s`, `%+x`, `%05v`), a precision on `%v`, or a width or precision above 4096. Provenance: the call. Repair: fix the specifier; write `%%` for a `%`. Location: the call.
- `diag.module-file-not-found` — Phase: static. Rule: `rule.module.file` `[Module-File-Missing]`. Invariant: none. Required: the declaration's path, resolved against the declaring file's directory, names a readable file. Observed: it names none. Provenance: the declaration's path and the directory it was resolved against. Repair: fix the path or create the file. Location: the `module` declaration.
- `diag.module-cycle` — Phase: static. Rule: `rule.module.file` `[Module-File-Cycle]`. Invariant: none. Required: no file's body contains, directly or through further file-backed declarations, itself. Observed: a cycle `F → … → F`. Provenance: each declaration on the cycle. Repair: break the cycle — move the shared items into a third file both name. Location: the declaration that closes the cycle.
- `diag.module-file-duplicate` — Phase: static. Rule: `rule.module.file` `[Module-File-Duplicate]`. Invariant: none. Required: each file is named by at most one declaration in the program. Observed: two. Provenance: both declarations. Repair: declare the module once and reach it by qualified path or `import`. Location: the second declaration.
- `diag.duplicate-item` — Phase: static. Rule: `rule.module.resolve` `[Item-Duplicate]`. Invariant: none. Required: each qualified name — `M::name` for a `fn`, `extern fn`, `struct`, `enum` or `module`, `M::T::name` for an associated function, `M::E::V` for an enum variant — is added by at most one declaration. Observed: two. Provenance: both declarations. Repair: rename one, or move it into a module. Location: the second declaration.
- `diag.foreign-associated-fn` — Phase: static. Rule: `rule.module.resolve` `[Assoc-Fn-Foreign-Type]`. Invariant: none. Required: `fn T::name` is declared in the module that declares the struct or enum `T`. Observed: `T` is declared elsewhere (for example `std`'s `Vec`) or not at all. Provenance: the declaration and `T`'s module. Repair: declare an ordinary function, or move the declaration into `T`'s module. Location: the declaration.
- `diag.type-mismatch` — Phase: static. Rule: `rule.type.typing` (any `[T-*]` premise failing), `[T-Convert]`, `[T-Alt]`, `[Limits-Not-Number]`, `[Print-Not-Printable]`, `[Append-Not-Printable]`, `[Parse-Not-Numeric]`, `[Format-Arg-Mismatch]`, `[Key-Not-Hashable]`, `[Foreach-Not-Iterable]`, `[Let-Destructure-Fields]`. Required: the operand types the rule names. Observed: a different type. Provenance: each operand's declaration or synthesis site. Repair: convert explicitly (`rule.arith.convert`) or fix the types.
- `diag.unbounded-type-parameter` — Phase: static. Rule: `rule.type.kind`. Required: only the permitted uses of a bare `T`. Observed: arithmetic, comparison, field access, or call on `T`. Provenance: the generic declaration. Repair: pass concrete types or restructure; there is no bound system.
- `diag.cannot-infer-type-parameter` — Phase: static. Rule: `rule.fn.generic-call` `[Generic-Call-Uninferable]`, `rule.arith.limits` `[Limits-Uninferable]`. Required: every type parameter determined by an argument or the expected type. Observed: one is not. Provenance: the argument list and enclosing annotation, both lacking it. Repair: `name<τ>(…)` or annotate the declaration. Related: `ex.generic-box`.
- `diag.break-outside-loop`, `diag.return-outside-fn` — Phase: static. Rule: `rule.control.while` / `rule.fn.return`. Required: lexically inside a `while` / function body. Repair: move the statement.
- `diag.capture-list-mismatch` — Phase: static. Rule: `rule.fn.closure` `[Closure-Capture-List-Rejected]`. Invariant: none (a well-formedness check on the closure literal, not a safety invariant). Required: the written capture list, as a set, equals the closure body's free variables. Observed: a name missing from the list, an extra name not used in the body, or both. Provenance: the closure literal's `[...]` vs. a syntactic scan of its body. Repair: add the missing name(s) or remove the unused one(s); the list documents an already-derived fact, it cannot select a different capture set. Related: `ex.closure-capture`.

## Validity of access paths

- `diag.stale-binding` — Phase: both. Rule: `[Binding-Lookup-Stale]`, `[Read-Stale]`, `[Write-Stale]`, `[Borrow-Stale]`, `[Field-Access-Stale]`, `[Destroy-Stale]`. Invariant: `inv.temporal-validity`. Required: `temporally-valid(a)`. Observed: false. Provenance: the invalidating event (object end at frame exit, last holder ended, statement end, transfer, relocation, buffer release) vs. this use. Repair: use the current holder after a move, or give the binding a new value first (`x = e`, `spec/11` `[Assign-Reestablish]`); do not keep a reference past its referent's or its holder's scope; re-index after a `Vec` reallocation. Related: `ex.use-after-move`, `ex.double-destroy`.
- `diag.use-of-uninitialized` — Phase: static (`rule.init.definite-assignment`, rejects on unknown); dynamic guard `[Read-Uninitialized]`, `[Write-Partial-Init]`. Invariant: `inv.initialization-validity`. Required: `init = valid` (a whole-object write on every path; no field write into an uninitialized aggregate). Observed: a path without a write, or a projection write before the whole. Provenance: the CFG path from block entry. Repair: initialize on every path; initialize aggregates whole. Related: `ex.definite-assignment`.
- `diag.reference-escapes-scope` — Phase: static. Rule: `rule.temporal.ref-escape`, `rule.temporal.elision`. Invariant: `inv.temporal-validity`. Required: no use of the reference outside its referent's block. Observed: stored into an enclosing block's binding or returned. Provenance: `referent-block` vs. the escaping store. Repair: return an owned value, or restructure. Related: `ex.reference-escape`.
- `diag.lifetime-elision-ambiguous` — Phase: static. Rule: `rule.temporal.elision` `[Call-Multi-Ref-Return-Rejected]`. Required: exactly one reference parameter when returning a reference. Observed: zero or several. Provenance: the signature. Repair: one reference parameter, or return owned; `feat.explicit-lifetime-parameters` is the open general case. Related: `ex.lifetime-elision`.

## Aliasing

- `diag.aliasing-conflict` — Phase: both. Rule: `[Borrow-Denied]`, `[Read-Conflict]`, `[Write-Conflict]`, `[Match-Conflict]`. Invariant: `inv.alias-validity`, `inv.concurrency-validity`. Required: `¬clash(a, m)`. Observed: `witness(a, m, Σ)` — a live non-ancestor path with overlapping target and incompatible mode. Provenance: the witness's formation event (which borrow, in which thread) vs. this access. Repair: end the conflicting reference (let its holder go out of scope) before this access, or use `shared` on both sides. Related: `ex.borrow-conflict`, `ex.projection-conflict`.
- `diag.borrow-exceeds-source` — Phase: static. Rule: `[Borrow-Exceeds-Source]`. Invariant: `inv.alias-validity`. Required: an exclusive borrow's source is exclusive. Observed: source mode `shared`. Provenance: the source reference's declared mode. Repair: take `ref<τ, exclusive>` where mutation is needed. Related: `ex.shared-is-read-only`.
- `diag.write-through-shared` — Phase: static. Rule: `[Write-Not-Exclusive]`. Invariant: `inv.alias-validity`. Required: `mode(a) = exclusive`. Observed: `shared`. Provenance: the path's mode at formation. Repair: as above. Related: `ex.shared-is-read-only`.
- `diag.destroy-while-aliased` — Phase: both. Rule: `[Destroy-Not-Solitary]`. Invariant: `inv.alias-validity`, `inv.temporal-validity`. Required: `solitary(a)`. Observed: another live path (a reference, a guard). Provenance: that path's formation. Repair: let the reference's holder end first; release the guard. Related: `ex.destroy-while-borrowed`.
- `diag.move-while-aliased` — Phase: both. Rule: `[Authority-Transfer-Aliased]`, `[Relocate-In-Aliased]`, `[Rawptr-Move-In]`/`-Out]` premises. Invariant: `inv.alias-validity`. Required: no other live path on the moved object. Observed: one exists. Provenance: its formation. Repair: end the reference before moving. Related: `ex.move-while-borrowed`.
- `diag.move-out-of-field` — Phase: static. Rule: `[Store-Binding-Place-Transfer-Sub]`, `[Destroy-Projection]`, `[Match-Move-Through-Ref]`, `[Let-Destructure-Destructor]`. Invariant: `inv.resource-authority`. Required: a moved resource is a whole object. Observed: a field/element/payload of a live object, or a payload reached through a reference. Provenance: the projection. Repair: move the whole container, or take a reference to the field.
- `diag.borrow-of-non-place`, `diag.borrow-of-temporary` — Phase: static. Rule: `[Ref-Form-Not-Place]`, `[Ref-Form-Temporary]`. Required: the operand of `&` is a place rooted in a binding, reference, or reclaim. Observed: a value or a temporary. Repair: bind it first.

## Resource authority

- `diag.no-destroy-authority` — Phase: both. Rule: `[Destroy-No-Authority]`. Invariant: `inv.resource-authority`. Required: unconsumed authority in this thread. Observed: absent (moved to another thread) or consumed. Provenance: the consuming/transferring event. Repair: destroy in the owning thread, once. (A second `drop(x)` through the same binding reports `diag.stale-binding` first.) Related: `ex.double-destroy`.
- `diag.transfer-without-authority` — Phase: both. Rule: `[Authority-Transfer-Unauthorized]`. Invariant: `inv.resource-authority`, `inv.temporal-validity`. Required: a valid source holding authority. Observed: stale or unauthorized. Provenance: the earlier move/destroy. Repair: move from the current holder. Related: `ex.use-after-move`.
- `diag.overwrite-of-live-resource` — Phase: both. Rule: `[Write-Resource-Overwrite-Rejected]`. Invariant: `inv.resource-authority`. Required: the target holds no live resource. Observed: an outstanding obligation at the target. Provenance: the establishing/moving event that put it there. Repair: `drop` first. Related: `ex.no-silent-resource-overwrite`.
- `diag.read-of-resource` — Phase: static. Rule: `[Read-Resource-Rejected]`. Invariant: `inv.resource-authority`. Required: `¬is-resource(type(a))` in value position. Observed: resource. Provenance: the type declaration. Repair: move it, or take a reference. Related: `ex.no-copy-of-resource`.
- `diag.bad-destructor-signature`, `diag.direct-destructor-call` — Phase: static. Rule: `spec/07` §1. Required: `T::drop(ref<T, exclusive> self)`, invoked only by `[Destroy]`. Repair: fix the signature; use `drop(x)`.

## Arithmetic

- `diag.arith-overflow` — Phase: both. Rule: `[Arith-Checked-Overflow]`, `[Neg-Overflow]`. Invariant: `inv.arith.range-validity`. Required: `r ∈ represented-domain(τ)`. Observed: outside. Provenance: the operand values at this expression. Repair: widen, or use `wrapping_`/`saturating_`/`checked_`. Related: `ex.checked-arithmetic`, `ex.negation`.
- `diag.div-by-zero` — Phase: both. Rule: `[Div-By-Zero]`. Required: `v2 ≠ 0`. Repair: test first, or `checked_div`. Related: `ex.checked-arithmetic`.
- `diag.div-overflow` — Phase: both. Rule: `[Div-Overflow]`. Required: `¬(v1 = min(τ) ∧ v2 = -1)`. Repair: `checked_div` or widen. Related: `ex.checked-arithmetic`.
- `diag.shift-amount-out-of-range` — Phase: both. Rule: `[Shift-Amount-Invalid]`. Required: `0 ≤ n < bitwidth(τ)`. Repair: mask the amount.
- `diag.narrowing-overflow` — Phase: both. Rule: `[Narrow-Overflow]`, `[Float-To-Int-Invalid]`. Required: `v ∈ represented-domain(τ')`. Repair: `narrow_wrapping`, or widen the target.
- `diag.literal-out-of-range` — Phase: static. Rule: `[Literal-Out-Of-Range]`. Required: `val(L) ∈ represented-domain(τ)`; for a float literal, that it does not round beyond `τ`'s largest finite magnitude. Provenance: the literal vs. its determined type. Repair: fix the literal or annotation.

## Aggregates

- `diag.recursive-type` — Phase: static. Rule: `rule.agg.layout` `[Type-Recursive]` (`spec/16` §1). Invariant: none. Required: no struct or enum contains itself by value. Observed: a chain of fields, payloads, array elements or `mutex` values leading back to the type, with no pointer on the way. Provenance: the type's declaration. Repair: put the recursive part behind a `Box<T>` (one owner), an `Rc<T>` (shared) or a `Vec<T>` (many). Location: the declaration.
- `diag.static-assert-failed` — Phase: static. Rule: `rule.module.static-assert` `[Static-Assert-Failed]` (`spec/17` §1). Invariant: none. Required: the condition of `static_assert` is `true`. Observed: `false`, for the program as written or for one instantiation of a generic function (the message given, if any, is shown). Provenance: the assertion and the constants and types it names. Repair: fix what the assertion guards, or the assertion. Location: the assertion.
- `diag.not-char-boundary` — Phase: dynamic. Rule: `rule.stdlib.stringview` `[View-Boundary]` (`spec/21` §2h). Invariant: `inv.string.utf8-validity`. Required: both bounds of a `StringView` fall at the start of a character or at the end of the text. Observed: a bound inside a character's UTF-8 encoding. Provenance: the view's bounds. Repair: move the bound to a character's start (a byte that is not `0b10xxxxxx`), for example by searching with `StringView::find`. Location: the view.
- `diag.not-ascii` — Phase: dynamic. Rule: `rule.stdlib.text` `[Push-Ascii-Not-Ascii]` (`spec/21` §2d). Invariant: `inv.string.utf8-validity`. Required: the byte given to `String::push_ascii` is below 128. Observed: a byte of 128 or more, which is not UTF-8 on its own. Provenance: the call. Repair: build bytes in a `Vec<u8>` and check them with `String::from_utf8`. Location: the call.
- `diag.index-out-of-bounds` — Phase: both. Rule: `[Index-Out-Of-Bounds]`, `[Slice-Out-Of-Bounds]` (`spec/16` §3a), `spec/21` §1 `fault`, `[Str-Byte-Out-Of-Bounds]` (`spec/21` §2a), `arg`'s `fault` (`spec/21` §2c). Invariant: `inv.spatial-validity`. Required: `0 ≤ n < N` / `i < len` / `i < str_len(s)` / `i < arg_count()`. Observed: violated. Provenance: the index value vs. the fixed `N`, current `len`, the `str`'s byte count, or the number of arguments. Repair: check first, or use `pop`/`checked` accessors. Related: `ex.bounds-check`, `ex.str-literal`.
- `diag.non-exhaustive-match` — Phase: static. Rule: `[Match-Non-Exhaustive]`. Required: the arms together match every value (every variant, at every level a pattern descends to, or `_`). Observed: a pattern no arm matches, given as the message. Provenance: the enum declarations vs. the arms. Repair: add an arm for the pattern named, or a `_` arm. Related: `ex.exhaustive-match`.
- `diag.unreachable-arm` — Phase: static. Rule: `[Match-Unreachable]`. Required: each arm matches some value no earlier arm matches (arms are tried in order). Observed: an arm every value of which an earlier arm takes. Provenance: the arm vs. the earlier arms. Repair: remove the arm, or move it before the arm that takes its values.

## Failure, concurrency, trust

- `diag.propagate-outside-fallible-context` — Phase: static. Rule: `[Propagate-Err-Mismatch]`. Required: the function returns `Result<_, E>` with matching `E`. Provenance: the declared return type vs. the `?` site. Repair: change the return type, `map_err`, or `match`.
- `diag.spawn-borrow-closure` — Phase: static. Rule: `rule.conc.spawn`. Required: a `fn` value or `move` closure. Repair: add `move`.
- `diag.mutex-reentrant-lock` — Phase: dynamic. Rule: `[Lock-Reentrant]`. Invariant: `inv.concurrency-validity`. Required: the thread does not already hold the lock. Provenance: the live guard. Repair: release the guard first.
- `diag.trusted-outside-unsafe` — Phase: static. Rule: `[Unsafe-Rejected]`. Invariant: `inv.trust-transition`. Required: every trusted construct inside `unsafe { }`. Provenance: the construct vs. the nearest enclosing `unsafe`. Repair: wrap in `unsafe` after verifying the asserted property. Related: `ex.unsafe-required`.
- `diag.extern-non-ffi-type` — Phase: static. Rule: `[Extern-Non-Ffi-Type]`. Required: `FfiType` parameters and result. Repair: pass `rawptr`/scalars.
- `diag.alloc-failure` — Phase: dynamic. Rule: `spec/21` §1/§3 `fault(alloc_failure)` after `allocate` returns `Err`. Required: allocation succeeded. Provenance: the `allocate` call. Repair: none at the language level; callers needing recovery call `allocate` themselves.

## Change Log

- 1.26.0 — `CHG-0064` (D-0056): `diag.unreachable-arm`; `diag.non-exhaustive-match`
  names a pattern not covered.
- 1.25.0 — `CHG-0061` (D-0053): `diag.not-char-boundary`.

- 1.24.0 — `CHG-0060` (D-0052): `diag.static-assert-failed`.

- 1.23.0 — `CHG-0059` (D-0051): `diag.type-mismatch` names
  `[Limits-Not-Number]` (was `[Limits-Not-Integer]`: a float type now
  has limits).

- 1.22.0 — `CHG-0055` (D-0047): `diag.index-out-of-bounds` names
  `[Slice-Out-Of-Bounds]`.

- 1.21.0 — `CHG-0053` (D-0045): new entry `diag.recursive-type`.

- 1.20.0 — `CHG-0052` (D-0044): new entry `diag.not-ascii`;
  `diag.type-mismatch` names `[Let-Destructure-Fields]`; `diag.move-out-of-field`
  names `[Let-Destructure-Destructor]`.

- 1.19.0 — `CHG-0050` (D-0042): `diag.type-mismatch` names
  `[Foreach-Not-Iterable]`.

- 1.18.0 — `CHG-0049` (D-0041): `diag.type-mismatch` names
  `[Key-Not-Hashable]`.

- 1.17.0 — `CHG-0048` (D-0040): `diag.format-invalid` covers `eprintf`.

- 1.16.0 — `CHG-0047` (D-0039): `diag.format-invalid` covers `sprintf`
  and `%v`.

- 1.15.0 — `CHG-0046` (D-0038): new entry `diag.format-invalid`;
  `diag.type-mismatch` names `[Format-Arg-Mismatch]`.

- 1.14.0 — `CHG-0044` (D-0036): new entry `diag.const-not-constant`.

- 1.13.0 — `CHG-0042` (D-0033): `diag.literal-out-of-range` covers a
  float literal that rounds beyond its type; `diag.stale-binding`'s
  repair mentions giving a moved-from binding a new value.

- 1.12.0 — `CHG-0041` (D-0032): `diag.type-mismatch` names
  `[Append-Not-Printable]` and `[Parse-Not-Numeric]`;
  `diag.ambiguous-name`'s observed cases follow `spec/17`'s clauses (4)
  and (4b).

- 1.11.0 — `CHG-0040` (D-0031): `diag.no-main` also covers a root
  `main` with parameters or a return type other than `void`/`u8`, and
  is located at that `main`;
  `diag.index-out-of-bounds` names `arg`'s fault.

- 1.10.0 — `CHG-0037`: `diag.type-mismatch` names `[Print-Not-Printable]`.

- 1.9.0 — `CHG-0035`: `diag.type-mismatch` and
  `diag.cannot-infer-type-parameter` name the `spec/06` rules that now
  report them.

- 1.8.0 — `CHG-0033`: new entry `diag.foreign-associated-fn`;
  `diag.duplicate-item` no longer mentions the prelude (the standard
  library is the module `std`); `diag.ambiguous-name` and
  `diag.unbound-name` mention module imports and `import std;`.
- 1.7.0 — `CHG-0032`: new entry `diag.duplicate-item` for
  `rule.module.resolve` `[Item-Duplicate]`. No existing entry changed.
- 1.6.0 — `CHG-0027`, `CHG-0028`: `diag.name-not-visible` also cites
  `rule.module.visibility` `[Field-Not-Visible]`; new entry
  `diag.no-main` for `rule.fn.program` `[Program-No-Main]`.
- 1.5.0 — `CHG-0026` (D-0021): new entries
  `diag.module-file-not-found`, `diag.module-cycle`, and
  `diag.module-file-duplicate`, all static, emitted by
  `rule.module.file` (`spec/17` §5). No existing entry changed.
- 1.4.1 — Non-normative (`CHG-0025`): `diag.index-out-of-bounds`'s
  entry names `[Str-Byte-Out-Of-Bounds]` as a further emitting rule.
  No new diagnostic; no entry's meaning changed.
- 1.4.0 — `CHG-0006`: `diag.unbound-name`, `diag.ambiguous-name`, and
  `diag.name-not-visible`'s repair/observed text re-spelled `import`/
  `export` for the renamed keywords (`spec/22` 2.5.0). No diagnostic's
  phase, rule, invariant, or actual repair changed.
- 1.3.0 — `CHG-0004`: `diag.cannot-infer-type-parameter`'s repair text
  re-spelled `name<τ>(…)` (turbofish removed, `spec/22` 2.3.0) and its
  stray reference to the retired `let` keyword fixed to "the
  declaration" (missed by `CHG-0001`'s sweep). No diagnostic's phase,
  rule, invariant, or actual repair changed.
- 1.2.0 — `CHG-0002`: new entry `diag.capture-list-mismatch` for the
  C++11-style closure capture list (`spec/22` 2.1.0, `spec/15` §6).
- 1.1.0 — Non-normative fix (`CHG-0001`): the illustrative destructor
  signature in `diag.bad-destructor-signature`/`diag.direct-destructor-call`
  re-spelled to `spec/22` 2.0.0's type-first parameter order
  (`T::drop(ref<T, exclusive> self)`). No diagnostic's phase, rule,
  invariant, or repair changed.
- 1.0.0 — Rewritten (`spec/AUDIT-2.md` B-14, B-19): every diagnostic
  produced by the 1.0.0 rules listed (41), with the rules that
  produce each and the `stale-binding`-before-`no-destroy-authority`
  ordering stated.
- 0.4.0 and earlier — superseded.
