# CobaltC Diagnostic Registry

Status: normative artifact
Version: 1.61.0
Conforms to: `spec/02-schema.md` (Kind: Diagnostic, `diag.<name>`)
Governed by: `CobaltC_Master_Instructions.md` §19

## Purpose

Every `diag.*` id any rule produces, with the fields §19 requires.
Every entry has **Status:** ACCEPTED. Conventions: **Phase** is
*static* (a `disposition: rejected` rule, or a `checked` rule whose
guard `rule.control.flow-analysis` refuted), *dynamic* (a `checked`
rule's runtime guard), or *both*. **Summary** is the diagnostic's
message when its occurrence gives none of its own. **Location** is,
unless stated, the expression the triggering rule's premises were
evaluated against; it is a file, a line and a column, the column being
that of the location's first token, counted in characters from 1 (a
location that is a declaration or a statement is its first token).
**Related** cites `ex.*` (`spec/examples.md`). When a program has
several static errors, the one reported is the first in the source (the
lowest line; among one line's errors, any), whatever order a checker
visits the program in (`CHG-0130`).

**How a diagnostic is shown** (D-0228). Both implementations print every
diagnostic in the same form, byte for byte. By default it is short: a first
line `error[diag.x]: message` for a static diagnostic or `fault[diag.x]:
message` for a dynamic one, where the message is the occurrence's own or
the entry's **Summary**; then `--> file:line:column`; then the source line,
read from the file when the diagnostic is shown, with carets under the
location's token; then `= repair:` and the entry's **Repair**. A file
that cannot be read is shown as its location alone. With `--explain`
(both tools), or with `COBALTC_EXPLAIN` set to anything but empty or `0`
(this is how a compiled program is asked), the entry's Rule, Invariant,
Required, Observed and Provenance follow, then its Related examples and
this registry. A conformance case requires the id and the phase
(`spec/conformance.md`), which a runner reads from the first line's
opening `error[diag.ID]` or `fault[diag.ID]` (`impl/conformance/run.py`);
the location is as this registry gives it; the rest of the form's
wording is not normative.

## Syntax

- `diag.syntax-error` — Summary: the text is not a CobaltC program here. Phase: static. Rule: the grammar, `spec/22` (lexical §1, syntactic §2–§3, and each rule that makes a form a syntax error, such as `[Slice-Not-Borrowed]`). Invariant: none. Required: the text is a CobaltC program by the grammar. Observed: at the location, the first token (or character) where the grammar allows none of what the message says was expected; nothing of the program runs. Provenance: the source text. Repair: correct the text there; the message names what was expected and what was found. Location: that token.

## Names and scoping

- `diag.unbound-name` — Summary: this name is not declared. Phase: static. Rule: `rule.value-object.binding-lookup` `[Binding-Lookup-Unbound]`, `rule.module.resolve` `[Resolve-Unbound]`, `spec/21` §0 `fault`'s name (D-0083). Invariant: none. Required: the name resolves to a binding or item. Observed: it does not. Provenance: use site vs. every declaration in scope. Repair: declare it, fix the spelling, or add an `import` (`import std;` for the standard library, `CHG-0033`). Related: `ex.unbound-name`.
- `diag.ambiguous-name` — Summary: this name could mean more than one item. Phase: static. Rule: `rule.module.resolve` `[Resolve-Ambiguous]`. Required: exactly one candidate. Observed: several (two `import`s, two imported modules exporting the name, two enums of the same module with the variant, or, where no module of the program's own has it, two imported enums with it). Provenance: the candidate declarations. Repair: qualify the name.
- `diag.name-not-visible` — Summary: this item is private to its module. Phase: static. Rule: `rule.module.resolve` `[Resolve-Not-Visible]`; `rule.module.visibility` `[Field-Not-Visible]` (a field named by `e.f`, a struct literal, or a destructuring). Required: `visible(M', name, M)`. Observed: private. Provenance: the declaration's `export` marker vs. the use site's module. Repair: mark `export` or move the reference.
- `diag.no-main` — Summary: the program has no `main` it can start from. Phase: static. Rule: `rule.fn.program` `[Program-No-Main]`. Invariant: none. Required: exactly one non-generic, non-`extern` `main` at the root module, with no parameters, returning `void` or `u8`. Observed: none there (a `main` inside a module is the ordinary item `m::main`), or one with parameters, another return type, type parameters, or `extern`. Provenance: the root module's items. Repair: declare `fn main()` or `fn main() : u8` at the root (arguments come from `arg`, `spec/21` §2c), or run the file that has one — a file-backed module's body is not a program (`spec/17` §5). Location: the root `main`, if there is one; otherwise the program as a whole.
- `diag.const-not-constant` — Summary: a constant's value must be known before the program runs. Phase: static. Rule: `rule.module.const` `[Const-Not-Constant]`, `[Const-Cycle]`, `[Const-Local-Not-Constant]` (`spec/17` §1a). Invariant: none. Required: a `const`'s type is not a resource, its initializer is a constant expression (literals, operators, other constants, struct, array and variant literals of constant expressions and their fields and elements, a function's name, the pure intrinsics listed there), and no constant depends on itself; a local constant's initializer names no local variable and no type parameter of its function. Observed: one of these fails. Provenance: the `const` declaration. Repair: use only constants in the initializer (a local variable is not one, even when set from a literal: declare it `const` too), compute the value in a function instead, or break the cycle. Location: the `const`.
- `diag.format-invalid` — Summary: this format string is not valid. Phase: static. Rule: `rule.stdlib.format` `[Format-Invalid]` (`spec/21` §2f). Invariant: none. Required: the format of `printf`, `sprintf`, `eprintf` or `String::appendf` is a string literal whose every `%` begins a specifier `%[flags][width][.precision]conversion` with a conversion of `d i u x X o b f F e E g G s v` (or is `%%`), and flags the conversion takes. Observed: a format that is not a literal, a `%` at the end, an unknown conversion, a flag the conversion does not take (`%#d`, `%0s`, `%+x`, `%05v`), a precision on `%v`, or a width or precision above 4096. Provenance: the call. Repair: fix the specifier; write `%%` for a `%`. Location: the call.
- `diag.module-file-not-found` — Summary: the module's file cannot be read. Phase: static. Rule: `rule.module.file` `[Module-File-Missing]`. Invariant: none. Required: the declaration's path, resolved against the declaring file's directory, names a readable file. Observed: it names none. Provenance: the declaration's path and the directory it was resolved against. Repair: fix the path or create the file. Location: the `module` declaration.
- `diag.module-cycle` — Summary: these module files contain each other. Phase: static. Rule: `rule.module.file` `[Module-File-Cycle]`. Invariant: none. Required: no file's body contains, directly or through further file-backed declarations, itself. Observed: a cycle `F → … → F`. Provenance: each declaration on the cycle. Repair: break the cycle — move the shared items into a third file both name. Location: the declaration that closes the cycle.
- `diag.module-file-duplicate` — Summary: this file is already declared as a module. Phase: static. Rule: `rule.module.file` `[Module-File-Duplicate]`. Invariant: none. Required: each file is named by at most one declaration in the program. Observed: two. Provenance: both declarations. Repair: declare the module once and reach it by qualified path or `import`. Location: the second declaration.
- `diag.duplicate-local` — Summary: this name is already declared in this block. Phase: static. Rule: `rule.value-object.binding-form` `[Binding-Form-Redeclared]` (D-0131). Invariant: none. Required: each name is declared at most once in a declaration region — a block's statements with the parameters, loop names or pattern binder governing it. Observed: a second declaration (a local variable, a destructured binder, a local constant, a parameter, or a loop's or pattern's name). Provenance: both declarations (the message says how far above the first one is). Repair: assign to the existing variable (`x = …`, after a `drop` for a live resource), or choose another name, as when the type changes; a nested block may shadow. Location: the second declaration.
- `diag.duplicate-item` — Summary: this name is already declared in this module. Phase: static. Rule: `rule.module.resolve` `[Item-Duplicate]`. Invariant: none. Required: each qualified name — `M::name` for a `fn`, `extern fn`, `struct`, `enum` or `module`, `M::T::name` for an associated function, `M::E::V` for an enum variant — is added by at most one declaration. Observed: two. Provenance: both declarations. Repair: rename one, or move it into a module. Location: the second declaration.
- `diag.foreign-associated-fn` — Summary: an associated function must be declared beside its type. Phase: static. Rule: `rule.module.resolve` `[Assoc-Fn-Foreign-Type]`. Invariant: none. Required: `fn T::name` is declared in the module that declares the struct or enum `T`. Observed: `T` is declared elsewhere (for example `std`'s `Vec`) or not at all. Provenance: the declaration and `T`'s module. Repair: declare an ordinary function, or move the declaration into `T`'s module. Location: the declaration.
- `diag.unittest-name` — Summary: this test's name cannot be used. Phase: static. Rule: `rule.module.unittest` `[Unittest-Name]` (`spec/17` §6, D-0230). Invariant: none. Required: a `unittest`'s name, when it has one, is not empty, holds no control character, and is not the name of another test of the same module. Observed: an empty name, a control character, or a second test of the module with that name. Provenance: the test and, for a repeated name, the first test with it. Repair: name each test of a module differently, say what it checks, or leave the name out. Location: the name.
- `diag.type-mismatch` — Summary: the types do not match. Phase: static. Rule: `rule.type.typing` (any `[T-*]` premise failing), `[T-Convert]`, `[T-Alt]`, `[Limits-Not-Number]`, `[Print-Not-Printable]`, `[Append-Not-Printable]`, `[Parse-Not-Numeric]`, `[Format-Arg-Mismatch]`, `[Key-Not-Hashable]`, `[Foreach-Not-Iterable]`, `[Foreach-Range-Ill-Typed]`, `[Let-Destructure-Fields]`, `[Array-Repeat-Not-Plain]`. Required: the operand types the rule names. Observed: a different type. Provenance: each operand's declaration or synthesis site. Repair: convert explicitly (`rule.arith.convert`) or fix the types.
- `diag.unbounded-type-parameter` — Summary: the type parameter's bound does not allow this. Phase: static. Rule: `rule.type.kind`, `rule.type.bound` `[Bound-Missing]`. Required: only the permitted uses of `T`, and the operators its bound provides. Observed: an operator its bound (if any) does not provide, field access, or call on `T`. Provenance: the generic declaration. Repair: give `T` the bound the operator needs (`<T: ordered>`, D-0090), or pass the operation in as a `fn` parameter.
- `diag.cannot-infer-type-parameter` — Summary: a type argument cannot be inferred here. Phase: static. Rule: `rule.fn.generic-call` `[Generic-Call-Uninferable]`, `rule.arith.limits` `[Limits-Uninferable]`. Required: every type parameter determined by an argument or the expected type. Observed: one is not. Provenance: the argument list and enclosing annotation, both lacking it. Repair: `name<τ>(…)` or annotate the declaration. Related: `ex.generic-box`.
- `diag.instantiation-unbounded` — Summary: a generic function's instantiations do not end. Phase: static. Rule: `rule.fn.generic-call` `[Generic-Instantiation-Unbounded]`. Required: a finite set of instantiations, each type argument nested at most 64 deep, at most 100,000 in all. Observed: an instantiation at a type argument nested deeper (a function calling itself at a larger type: `f<T>` calling `f<Box<T>>`), or more instantiations. Provenance: the generic function's declaration. Repair: pass the larger value as a type the function already has (a `Vec`, a `Box` of one fixed type), or split the function.
- `diag.break-outside-loop`, `diag.return-outside-fn` — Summary: `break`, `continue` or `return` has nothing to leave here. Phase: static. Rule: `rule.control.while` / `rule.fn.return`. Required: lexically inside a `while` / function body. Repair: move the statement.
- `diag.capture-list-mismatch` — Summary: the capture list does not match the names the closure uses. Phase: static. Rule: `rule.fn.closure` `[Closure-Capture-List-Rejected]`. Invariant: none (a well-formedness check on the closure literal, not a safety invariant). Required: the written capture list, as a set, equals the closure body's free variables. Observed: a name missing from the list, an extra name not used in the body, or both. Provenance: the closure literal's `[...]` vs. a syntactic scan of its body. Repair: add the missing name(s) or remove the unused one(s); the list documents an already-derived fact, it cannot select a different capture set. Related: `ex.closure-capture`.

## Validity of access paths

- `diag.stale-binding` — Summary: this value is no longer valid. Phase: both. Rule: `[Binding-Lookup-Stale]`, `[Read-Stale]`, `[Write-Stale]`, `[Borrow-Stale]`, `[Field-Access-Stale]`, `[Destroy-Stale]`, `[Fn-Value-Empty-Call]`. Invariant: `inv.temporal-validity`. Required: `temporally-valid(a)`. Observed: false. Provenance: the invalidating event (object end at frame exit, last holder ended, statement end, transfer, relocation, buffer release, a `fn` value's closure moved out) vs. this use. Repair: use the current holder after a move, or give the binding a new value first (`x = e`, `spec/11` `[Assign-Reestablish]`); do not keep a reference past its referent's or its holder's scope; re-index after a `Vec` reallocation. Related: `ex.use-after-move`, `ex.double-destroy`.
- `diag.use-of-uninitialized` — Summary: this is used before it has a value. Phase: static (`rule.init.definite-assignment`, rejects on unknown); dynamic guard `[Read-Uninitialized]`, `[Write-Partial-Init]`. Invariant: `inv.initialization-validity`. Required: `init = valid` (a whole-object write on every path; no field write into an uninitialized aggregate). Observed: a path without a write, or a projection write before the whole. Provenance: the CFG path from block entry. Repair: initialize on every path; initialize aggregates whole. Related: `ex.definite-assignment`.
- `diag.reference-escapes-scope` — Summary: this reference would outlive what it refers to. Phase: static. Rule: `rule.temporal.ref-escape`, `rule.temporal.elision`. Invariant: `inv.temporal-validity`. Required: no use of the reference outside its referent's block. Observed: stored into an enclosing block's binding or returned. Provenance: `referent-block` vs. the escaping store. Repair: return an owned value, or restructure. Related: `ex.reference-escape`.
- `diag.lifetime-elision-ambiguous` — Summary: it is not clear which parameter the returned reference comes from. Phase: static. Rule: `rule.temporal.elision` `[Call-Multi-Ref-Return-Rejected]`. Required: exactly one reference parameter when returning a reference. Observed: zero or several. Provenance: the signature. Repair: one reference parameter, or return owned; `feat.explicit-lifetime-parameters` is the open general case. Related: `ex.lifetime-elision`.

## Aliasing

- `diag.aliasing-conflict` — Summary: this access conflicts with a live reference. Phase: both. Rule: `[Borrow-Denied]`, `[Read-Conflict]`, `[Write-Conflict]`, `[Match-Conflict]`, `[Thread-Affine]` (D-0213: an exclusive path of another running thread), `[Thread-Lend]` (a second lend of a live lent path). Invariant: `inv.alias-validity`, `inv.concurrency-validity`. Required: `¬clash(a, m)`. Observed: `witness(a, m, Σ)` — a live non-ancestor path with overlapping target and incompatible mode. Provenance: the witness's formation event (which borrow, in which thread) vs. this access. Repair: end the conflicting reference (let its holder go out of scope) before this access, or use `shared` on both sides. Related: `ex.borrow-conflict`, `ex.projection-conflict`.
- `diag.borrow-exceeds-source` — Summary: this reference would allow more than its source does. Phase: static; dynamic for `[Read-Fn-Through-Shared]`. Rule: `[Borrow-Exceeds-Source]`, `[T-Read-Through-Shared]` (an exclusive reference read through a shared one, where an exclusive one is expected), `[Read-Exclusive-Through-Shared]`, `[Match-Exclusive-Through-Shared]`, `[Read-Fn-Through-Shared]` (D-0218). Invariant: `inv.alias-validity`. Required: an exclusive borrow's source is exclusive; nothing reached through a shared reference is used as a writer. Observed: source mode `shared`; a copy holding an exclusive reference; a closure with an exclusive capture. Provenance: the source reference's declared mode. Repair: take `ref<τ, exclusive>` where mutation is needed. Related: `ex.shared-is-read-only`.
- `diag.write-through-shared` — Summary: this writes through a shared reference. Phase: static. Rule: `[Write-Not-Exclusive]` (a place reached through a shared path, including through an exclusive reference it holds: `[Read-Ref-Through-Shared]`, D-0218). Invariant: `inv.alias-validity`. Required: `mode(a) = exclusive`. Observed: `shared`. Provenance: the path's mode at formation. Repair: as above. Related: `ex.shared-is-read-only`.
- `diag.destroy-while-aliased` — Summary: this is destroyed while a reference to it is live. Phase: both. Rule: `[Destroy-Not-Solitary]`. Invariant: `inv.alias-validity`, `inv.temporal-validity`. Required: `solitary(a)`. Observed: another live path (a reference, a guard). Provenance: that path's formation. Repair: let the reference's holder end first; release the guard. Related: `ex.destroy-while-borrowed`.
- `diag.move-while-aliased` — Summary: this is moved while a reference to it is live. Phase: both. Rule: `[Authority-Transfer-Aliased]`, `[Relocate-In-Aliased]`, `[Rawptr-Move-In]`/`-Out]` premises. Invariant: `inv.alias-validity`. Required: no other live path on the moved object. Observed: one exists. Provenance: its formation. Repair: end the reference before moving. Related: `ex.move-while-borrowed`.
- `diag.move-out-of-field` — Summary: a part of a live value cannot be moved out on its own. Phase: static. Rule: `[Store-Binding-Place-Transfer-Sub]`, `[Destroy-Projection]`, `[Match-Move-Through-Ref]`, `[Let-Destructure-Destructor]`. Invariant: `inv.resource-authority`. Required: a moved resource is a whole object. Observed: a field/element/payload of a live object, or a payload reached through a reference. Provenance: the projection. Repair: move the whole container, or take a reference to the field.
- `diag.borrow-of-non-place`, `diag.borrow-of-temporary` — Summary: only a place can be borrowed. Phase: static. Rule: `[Ref-Form-Not-Place]`, `[Ref-Form-Temporary]`; `[Simd-Lane]` (`spec/16` §3b, D-0254). Required: the operand of `&` is a place rooted in a binding, reference, or reclaim. Observed: a value or a temporary; a bit field; a lane of a `simd`, or a range of them. Repair: bind it first; read or assign the lane, or borrow the whole vector.

## Resource authority

- `diag.no-destroy-authority` — Summary: this code no longer owns the value it destroys. Phase: both. Rule: `[Destroy-No-Authority]`. Invariant: `inv.resource-authority`. Required: unconsumed authority in this thread. Observed: absent (moved to another thread) or consumed. Provenance: the consuming/transferring event. Repair: destroy in the owning thread, once. (A second `drop(x)` through the same binding reports `diag.stale-binding` first.) Related: `ex.double-destroy`.
- `diag.transfer-without-authority` — Summary: this code no longer owns the value it hands on. Phase: both. Rule: `[Authority-Transfer-Unauthorized]`. Invariant: `inv.resource-authority`, `inv.temporal-validity`. Required: a valid source holding authority. Observed: stale or unauthorized. Provenance: the earlier move/destroy. Repair: move from the current holder. Related: `ex.use-after-move`.
- `diag.overwrite-of-live-resource` — Summary: this overwrites a resource that has not been ended. Phase: both. Rule: `[Write-Resource-Overwrite-Rejected]`. Invariant: `inv.resource-authority`. Required: the target holds no live resource, or its type is quiet (D-0194: then the write destroys it). Observed: an outstanding obligation at the target. Provenance: the establishing/moving event that put it there. Repair: `overwrite(&mut place, v)` to put `v` there and destroy the old value (D-0080), `replace(&mut place, v)` to keep the old value, or `drop` the old value first. Related: `ex.no-silent-resource-overwrite`.
- `diag.read-of-resource` — Summary: a resource cannot be copied by reading it. Phase: static. Rule: `[Read-Resource-Rejected]`. Invariant: `inv.resource-authority`. Required: `¬is-resource(type(a))` in value position. Observed: resource. Provenance: the type declaration. Repair: move it, or take a reference. Related: `ex.no-copy-of-resource`.
- `diag.destructor-on-plain-type` — Summary: only a resource can have a destructor. Phase: static. Rule: `spec/07` §1 (D-0075). Invariant: `inv.resource-authority`. Required: a type with a destructor is a resource in every instantiation. Observed: `T::drop` declared for a type that is not one. Provenance: the destructor's declaration. Repair: declare the type `resource struct T` (or `resource enum T`).
- `diag.bad-destructor-signature`, `diag.direct-destructor-call` — Summary: a destructor must be `T::drop(ref<T, exclusive> self)`, called only by `drop`. Phase: static. Rule: `spec/07` §1. Required: `T::drop(ref<T, exclusive> self)`, invoked only by `[Destroy]`. Repair: fix the signature; use `drop(x)`.

## Arithmetic

- `diag.arith-overflow` — Summary: arithmetic overflow. Phase: both. Rule: `[Arith-Checked-Overflow]`, `[Neg-Overflow]`. Invariant: `inv.arith.range-validity`. Required: `r ∈ represented-domain(τ)`. Observed: outside. Provenance: the operand values at this expression. Repair: widen, or use `wrapping_`/`saturating_`/`checked_`. Related: `ex.checked-arithmetic`, `ex.negation`.
- `diag.div-by-zero` — Summary: division by zero. Phase: both. Rule: `[Div-By-Zero]`. Required: `v2 ≠ 0`. Repair: test first, or `checked_div`. Related: `ex.checked-arithmetic`.
- `diag.div-overflow` — Summary: division overflow: the minimum divided by -1. Phase: both. Rule: `[Div-Overflow]`. Required: `¬(v1 = min(τ) ∧ v2 = -1)`. Repair: `checked_div` or widen. Related: `ex.checked-arithmetic`.
- `diag.shift-amount-out-of-range` — Summary: shift amount out of range for the type. Phase: both. Rule: `[Shift-Amount-Invalid]`. Required: `0 ≤ n < bitwidth(τ)`. Repair: mask the amount.
- `diag.narrowing-overflow` — Summary: the value does not fit the narrower type. Phase: both. Rule: `[Narrow-Overflow]`, `[Float-To-Int-Invalid]`, `[Bitfield-Overflow]` (`spec/16` §2a, D-0118). Required: `v ∈ represented-domain(τ')`. Repair: `narrow_wrapping`, or widen the target.
- `diag.literal-out-of-range` — Summary: this literal does not fit its type. Phase: static. Rule: `[Literal-Out-Of-Range]`. Required: `val(L) ∈ represented-domain(τ)`; for a float literal, that it does not round beyond `τ`'s largest finite magnitude. Provenance: the literal vs. its determined type. Repair: fix the literal or annotation.

## Aggregates

- `diag.recursive-type` — Summary: this type contains itself. Phase: static. Rule: `rule.agg.layout` `[Type-Recursive]` (`spec/16` §1). Invariant: none. Required: no struct or enum contains itself by value. Observed: a chain of fields, payloads, array elements or `mutex` values leading back to the type, with no pointer on the way. Provenance: the type's declaration. Repair: put the recursive part behind a `Box<T>` (one owner), an `Rc<T>` (shared) or a `Vec<T>` (many). Location: the declaration.
- `diag.static-assert-failed` — Summary: static assertion failed. Phase: static. Rule: `rule.module.static-assert` `[Static-Assert-Failed]` (`spec/17` §1). Invariant: none. Required: the condition of `static_assert` is `true`. Observed: `false`, for the program as written or for one instantiation of a generic function (the message given, if any, is shown, formatted with its arguments' values). Provenance: the assertion and the constants and types it names. Repair: fix what the assertion guards, or the assertion. Location: the assertion.
- `diag.static-assert-not-constant` — Summary: a static assertion's condition must be known before the program runs. Phase: static. Rule: `rule.module.static-assert` `[Static-Assert-Not-Constant]` (`spec/17` §1b). Invariant: none. Required: the condition of `static_assert` is a `bool` constant expression — literals, constants (module or local), operators, `sizeof`, `alignof`, the limits and the conversions — so that it can be computed before the program runs. Observed: the condition (or an argument of its message) names a local variable or parameter, calls a function or reads a container, or the condition is not a `bool`. Provenance: the condition. Repair: for a condition that is only known when the program runs, use `assert` (`spec/18` §1a); to check a value before the program runs, name it with `const` (a local `const` works: `const u8 D = B + 1;`). Location: the assertion.
- `diag.not-char-boundary` — Summary: this bound falls inside a character. Phase: dynamic. Rule: `rule.stdlib.stringview` `[View-Boundary]` (`spec/21` §2h). Invariant: `inv.string.utf8-validity`. Required: both bounds of a `StringView` fall at the start of a character or at the end of the text. Observed: a bound inside a character's UTF-8 encoding. Provenance: the view's bounds. Repair: move the bound to a character's start (a byte that is not `0b10xxxxxx`), for example by searching with `StringView::find`. Location: the view.
- `diag.invalid-datetime` — Summary: this date and time do not exist. Phase: dynamic. Rule: `rule.stdlib.time` `[Invalid-DateTime]` (`spec/21` §2j, D-0140). Invariant: none. Required: the `DateTime` names a moment of the proleptic Gregorian calendar (a month 1 to 12, a day within its month, an hour below 24, a minute and a second below 60) at most `i64` seconds from 1970. Observed: a field out of range, or a year too far from 1970. Provenance: the `DateTime`, written as a struct literal or with its fields changed. Repair: make it with `DateTime::new`, which returns `None` for such fields, or with `DateTime::from_unix`/`from_iso`. Location: the call of `DateTime::to_unix` (or `weekday`).
- `diag.entropy-unavailable` — Summary: the operating system gave no secure random bytes. Phase: dynamic. Rule: `spec/21` §0 `[Os-Random]` (D-0142). Invariant: none. Required: the operating system gives bytes from its cryptographically secure source. Observed: it has none, or refused (on Linux, no `getrandom` and no readable `/dev/urandom`, as in a bare container). Provenance: the call. Repair: run where the system provides secure randomness; never substitute a clock or a fixed seed for keys. Location: the call of `os_random_bytes`, `os_random_u64` or `Rng::from_os`.
- `diag.hkdf-length` — Summary: HKDF cannot produce more than 8160 bytes. Phase: dynamic. Rule: `spec/21` §2m `[Hkdf]` (D-0152). Invariant: none. Required: `hkdf_expand`'s `len` is at most 8160, 255 blocks of 32 bytes (RFC 5869's bound). Observed: a larger `len`. Provenance: the call. Repair: derive at most 8160 bytes from one pseudorandom key; derive more keys with different `info`. Location: the call of `hkdf_expand` or `hkdf_sha256`.
- `diag.crypto-length` — Summary: a key, nonce or digest of the wrong length. Phase: dynamic. Rule: `spec/21` §2m `[Crypto-Length]` (D-0153, D-0154, D-0165, D-0167, D-0168, D-0169, D-0170). Invariant: none. Required: a ChaCha20 or AEAD key of 32 bytes and a nonce of 12, an AES key of 16, 24 or 32 bytes and an AES-GCM nonce of 12 (D-0165), an X25519, P-256 or Ed25519 private key of 32 bytes and a P-384 private key of 48, a Poly1305 key of 32 bytes, a ChaCha20 message that fits in the 2^32 blocks after its starting counter, a BLAKE2b digest of 1 to 64 bytes under a key of at most 64, an RSA digest of its hash's length and a modulus long enough for the signature encoding, and an RSA key size of 1024 to 8192 bits in a multiple of 64. Observed: another length. Provenance: the call. Repair: pass keys and nonces of the algorithm's sizes (a key from `hkdf_expand` with `len` 32, a nonce of 12 bytes, a digest from `hash` of the `HashKind` named). Location: the call into `std::crypto`.
- `diag.kdf-parameters` — Summary: the key-derivation parameters are out of range. Phase: dynamic. Rule: `spec/21` §2m `[Kdf-Parameters]` (D-0170). Invariant: none. Required: `pbkdf2` runs at least one iteration; `argon2id` (and a PHC string `verify_password` reads, which is false instead) has at least one pass, 1 to 255 lanes, at least 8 KiB of memory per lane, a salt of at least 8 bytes and a result of at least 4 bytes (RFC 9106 §3.1). Observed: a parameter outside those bounds. Provenance: the call. Repair: pass the parameters the standard allows; `hash_password` chooses them itself. Location: the call of `pbkdf2` or `argon2id`.
- `diag.not-ascii` — Summary: this byte is not ASCII. Phase: dynamic. Rule: `rule.stdlib.text` `[Push-Ascii-Not-Ascii]` (`spec/21` §2d). Invariant: `inv.string.utf8-validity`. Required: the byte given to `String::push_ascii` is below 128. Observed: a byte of 128 or more, which is not UTF-8 on its own. Provenance: the call. Repair: build bytes in a `Vec<u8>` and check them with `String::from_utf8`. Location: the call.
- `diag.index-out-of-bounds` — Summary: index out of bounds. Phase: both. Rule: `[Index-Out-Of-Bounds]`, `[Slice-Out-Of-Bounds]` (`spec/16` §3a), `spec/21` §1 `fault`, `[Str-Byte-Out-Of-Bounds]` (`spec/21` §2a), `arg`'s `fault` (`spec/21` §2c). Invariant: `inv.spatial-validity`. Required: `0 ≤ n < N` / `i < len` / `i < str_len(s)` / `i < arg_count()`. Observed: violated. Provenance: the index value vs. the fixed `N`, current `len`, the `str`'s byte count, or the number of arguments. Repair: check first, or use `pop`/`checked` accessors. Related: `ex.bounds-check`, `ex.str-literal`.
- `diag.non-exhaustive-match` — Summary: this `match` does not cover every value. Phase: static. Rule: `[Match-Non-Exhaustive]`. Required: the arms together match every value (every variant, at every level a pattern descends to, or `_`). Observed: a pattern no arm matches, given as the message. Provenance: the enum declarations vs. the arms. Repair: add an arm for the pattern named, or a `_` arm. Related: `ex.exhaustive-match`.
- `diag.unreachable-arm` — Summary: this arm can never be reached. Phase: static. Rule: `[Match-Unreachable]`. Required: each arm matches some value no earlier arm matches (arms are tried in order). Observed: an arm every value of which an earlier arm takes. Provenance: the arm vs. the earlier arms. Repair: remove the arm, or move it before the arm that takes its values.

## Failure, concurrency, trust

- `diag.propagate-outside-fallible-context` — Summary: `?` cannot be used in this function. Phase: static. Rule: `[Propagate-Err-Mismatch]`, `[Propagate-Option-Mismatch]` (D-0130). Required: the function returns `Result<_, E>` with matching `E` (for a `Result` operand), or an `Option` (for an `Option` operand). Provenance: the declared return type vs. the `?` site. Repair: change the return type, `Result::map_err`, `Option::ok_or(o, e)?` (an `Option` in a `Result`-returning function), or `match`.
- `diag.spawn-borrow-closure` — Summary: a spawned thread needs a `move` closure. Phase: static. Rule: `rule.conc.spawn`. Required: a `fn` value or `move` closure. Repair: add `move`.
- `diag.mutex-reentrant-lock` — Summary: this thread already holds the lock. Phase: dynamic. Rule: `[Lock-Reentrant]`. Invariant: `inv.concurrency-validity`. Required: the thread does not already hold the lock. Provenance: the live guard. Repair: release the guard first.
- `diag.trusted-outside-unsafe` — Summary: this needs an `unsafe` block. Phase: static. Rule: `[Unsafe-Rejected]`. Invariant: `inv.trust-transition`. Required: every trusted construct inside `unsafe { }`. Provenance: the construct vs. the nearest enclosing `unsafe`. Repair: wrap in `unsafe` after verifying the asserted property. Related: `ex.unsafe-required`.
- `diag.extern-non-ffi-type` — Summary: this type cannot cross into C. Phase: static. Rule: `[Extern-Non-Ffi-Type]`. Required: `FfiType` parameters and result: integers, `f32`, `f64`, `bool`, `void` (a result), `rawptr<τ>`, and plain structs of those (`ffi-struct`, D-0252). Observed: the first type that is none of these, named through the field path that reaches it. Repair: pass `rawptr`/scalars, or a struct without resources, enums or bitstructs.
- `diag.null-pointer` — Summary: C's `NULL` where text was expected. Phase: dynamic. Rule: `rule.stdlib.ffi` `[From-C-Null]` (`spec/21` §3f, D-0252). Invariant: none. Required: the `rawptr<u8>` given to `String::from_c` or `Vec::from_c_bytes` is not `null_rawptr<u8>()`. Observed: address 0. Provenance: the C function that returned it. Repair: test with `is_null` first; C functions return `NULL` for "none". Location: the call.
- `diag.assert-failed` — Summary: assertion failed. Phase: dynamic. Rule: `rule.fail.assert` `[Assert-Fail]` (`spec/18` §1a). Invariant: none. Required: the condition of `assert` is `true`. Observed: `false` (the message the program gave, if any, formatted, is shown after the location). Provenance: the call. Repair: the program's own condition names what was assumed; find why it did not hold. Location: the call.
- `diag.channel-zero-capacity` — Summary: a channel needs a capacity of at least 1. Phase: dynamic. Rule: `rule.conc.channel` `[Channel-Zero-Capacity]` (`spec/19` §3). Invariant: none. Required: the capacity given to `Channel::new` is at least 1. Observed: 0. Provenance: the call. Repair: give the number of values the channel may hold; 1 for a hand-over. Location: the call.
- `diag.barrier-zero-parties` — Summary: a barrier needs at least one party. Phase: dynamic. Rule: `rule.stdlib.waiting` `[Barrier-Zero-Parties]` (`spec/21` §3c‴). Invariant: none. Required: the number of parties given to `Barrier::new` is at least 1. Observed: 0. Provenance: the call. Repair: give the number of threads that meet at the barrier. Location: the call.
- `diag.channel-deadlock` — Summary: the only running thread waits on a channel nothing else can change. Phase: dynamic. Rule: `rule.conc.channel` `[Channel-Deadlock]` (`spec/19` §3). Invariant: none. Required: while `Channel::send` or `Channel::recv` waits, another thread exists that could change the channel. Observed: the program's only running thread waits on a full or empty channel that is not closed. Provenance: the call. Repair: send or receive from another thread, or `close` the channel before receiving the last value. Location: the call.
- `diag.thread-cancelled` — Summary: this thread was cancelled. Phase: dynamic. Rule: `rule.conc.cancel` `[Cancel]` (`spec/19` §1, D-0204). Invariant: none. Required: the thread was not asked to stop while it waits. Observed: `cancel` was called on its handle; it was waiting (a join, a lock, a channel, sleeping, a socket) or began to. Provenance: the `cancel` call. Repair: none is needed when the stop was meant: the thread's failure is contained, and its joiner or handle sees it; to let a thread finish, do not cancel it. Location: the wait.
- `diag.mutex-poisoned` — Summary: a thread failed while holding this lock. Phase: dynamic. Rule: `rule.conc.lock` `[Lock-Poisoned]` (`spec/19` §2, D-0204). Invariant: `inv.concurrency-validity`. Required: the mutex was not held by a thread whose fault was contained. Observed: such a thread held it when it failed, so its interior may be half-changed. Provenance: the failed thread's fault. Repair: find that fault (`try_join` gives it); keep a lock's block short, so that little can fail inside it. Location: the `lock`.
- `diag.deadlock` — Summary: these threads would wait for each other forever. Phase: dynamic. Rule: `rule.conc.guarantee` `[Deadlock]` (`spec/19` §4, D-0204). Invariant: none. Required: the wait can end. Observed: waiting would close a cycle of threads waiting to join or lock each other's, or every thread waits on another, a mutex, a channel or an atomic (`Atomic::wait`, D-0253), and none on the clock or outside the program. Provenance: the waits. Repair: take locks in one order; send before waiting to receive; store and notify before waiting on an atomic another thread waits to see change; join a thread only after giving it what it waits for. Location: the wait.
- `diag.result-discarded` — Summary: a `Result` is discarded. Phase: static. Rule: `spec/13` §3 `[Stmt-Result-Discarded]` (D-0099). Invariant: none. Required: a statement's value that is a `Result` is used — propagated with `?`, matched, bound, passed or returned. Observed: an expression statement (or a block-like statement) whose value is a `Result`, discarded. Provenance: the statement. Repair: handle the error (`?`, `match`), or discard the value on purpose with `_ = e;`.
- `diag.stack-exhausted` — Summary: calls nested too deeply for the stack. Phase: dynamic. Rule: `spec/15` §3 `[Call-Stack-Exhausted]` (D-0077). Invariant: none (an implementation limit). Required: the implementation can give the call a frame. Observed: calls nested deeper than the stack the implementation provides. Provenance: the call. Repair: recurse less deeply (a loop, an explicit stack in a `Vec`).
- `diag.unwrap-failed` — Summary: nothing to unwrap: `None` or `Err`. Phase: dynamic. Rule: `spec/21` §1 `Option::unwrap`, `Option::expect`, `Result::unwrap`, `Result::expect` (D-0082). Invariant: none. Required: the `Option` is `Some`, the `Result` is `Ok`. Observed: `None` or `Err` (`expect`'s message, or which call it was, is shown after the location). Provenance: the call. Repair: match on the value, or give a default with `unwrap_or`, where its absence is possible. Location: the call.
- `diag.alloc-failure` — Summary: memory could not be allocated. Phase: dynamic. Rule: `spec/21` §1/§3 `fault(alloc_failure)` after `allocate` returns `Err`. Required: allocation succeeded. Provenance: the `allocate` call. Repair: none at the language level; callers needing recovery call `allocate` themselves.

## Change Log

- 1.61.0 — `CHG-0357`: `diag.instantiation-unbounded`.
- 1.60.0 — `CHG-0323` (D-0260): `diag.barrier-zero-parties`.
- 1.59.0 — `CHG-0320` (D-0258): `diag.write-through-shared` also for `&mut` of a static; `diag.read-of-resource` for a static read or moved whole; `diag.type-mismatch` and `diag.literal-out-of-range` for an ill-formed static; `diag.borrow-of-non-place` for `&mut` of a constant.
- 1.58.0 — `CHG-0319` (D-0257): `diag.extern-non-ffi-type` also from `[Extern-Entry]` (an `extern fn` with a body); `diag.duplicate-item` also for a C name two entry points share or the runtime reserves; `diag.trusted-outside-unsafe` also from `Vec::from_raw` and `copy_to_raw`.
- 1.57.0 — `CHG-0312` (D-0254): `diag.borrow-of-non-place` names a `simd`'s lane.
- 1.56.0 — `CHG-0311` (D-0253): `diag.deadlock` covers a wait on an atomic.
- 1.55.0 — `CHG-0309`, `CHG-0310` (D-0252): `diag.extern-non-ffi-type` admits plain structs; `diag.null-pointer`.
- 1.54.0 — `CHG-0274` (D-0230): `diag.unittest-name`.
- 1.53.0 — `CHG-0271` (D-0228): every entry has a **Summary**; a location has a column; "How a diagnostic
  is shown" gives the short form and `--explain`.
- 1.52.0 — `CHG-0257` (D-0218): `diag.borrow-exceeds-source` and
  `diag.write-through-shared` name the read-through-shared rules.
- 1.51.0 — `CHG-0251` (D-0213): `diag.aliasing-conflict` names `[Thread-Affine]` and `[Thread-Lend]`.
- 1.50.0 — `CHG-0233` (D-0204): `diag.thread-cancelled`, `diag.mutex-poisoned`, `diag.deadlock`.

- 1.49.0 — `CHG-0222` (D-0194): `diag.overwrite-of-live-resource` does not apply
  to a quiet type, whose old value the write destroys.
- 1.48.0 — `CHG-0195`–`CHG-0198` (D-0167–D-0170): `diag.kdf-parameters`; `diag.crypto-length` covers Ed25519 and P-384 private keys, BLAKE2b's lengths and RSA's.
- 1.47.0 — `CHG-0193` (D-0165): `diag.crypto-length` covers AES keys and nonces and P-256 private keys.
- 1.46.0 — `CHG-0181` (D-0153): `diag.crypto-length`.
- 1.45.0 — `CHG-0180` (D-0152): `diag.hkdf-length`.
- 1.44.0 — `CHG-0168` (D-0140): `diag.invalid-datetime`. `CHG-0170` (D-0142): `diag.entropy-unavailable`.
- 1.43.1 — `CHG-0158` (D-0134): `Result::map_err` in a repair.
- 1.43.0 — `CHG-0154` (D-0131): new entry `diag.duplicate-local`.
- 1.42.0 — `CHG-0144` (D-0121): `diag.trusted-outside-unsafe` also from `read_volatile`/`write_volatile`;
  `CHG-0149` (D-0126): `diag.syntax-error` for a length that is not a constant expression.
- 1.41.0 — `CHG-0141` (D-0118): `diag.narrowing-overflow` also from `[Bitfield-Overflow]`;
  `diag.borrow-of-non-place` also from `[Bitfield-No-Place]`; `diag.syntax-error` for a
  `bitstruct` whose fields do not fill it.
- 1.40.0 — `CHG-0130`: of several static errors, the first in the
  source is reported.
- 1.39.0 — `CHG-0118` (D-0099): `diag.result-discarded`.
- 1.38.0 — `CHG-0112` (D-0097): `diag.type-mismatch` names
  `[Foreach-Range-Ill-Typed]`.
- 1.37.0 — `CHG-0105` (D-0090): `diag.unbounded-type-parameter` names
  bounds as the repair.
- 1.36.0 — `CHG-0104` (D-0089): `diag.stale-binding` also for a call of a
  `fn` value whose closure moved out (`[Fn-Value-Empty-Call]`).
- 1.35.0 — `CHG-0096` (D-0080): `diag.overwrite-of-live-resource`'s repair
  names `overwrite`. `CHG-0098` (D-0082): `diag.unwrap-failed`.
  `CHG-0099` (D-0083): `diag.unbound-name` covers a `fault` name that is
  not a run-time fault.
- 1.34.0 — `CHG-0091` (D-0077): `diag.stack-exhausted`.
- 1.33.0 — `CHG-0088` (D-0075): `diag.destructor-on-plain-type`.
- 1.32.0 — `CHG-0085` (D-0072): `diag.overwrite-of-live-resource`'s repair
  names `replace`; `diag.type-mismatch` covers `[Array-Repeat-Not-Plain]`.
- 1.31.0 — `CHG-0082`: `diag.syntax-error`.
- 1.30.0 — `CHG-0078` (D-0068): the two `static_assert` entries cover a
  message with arguments.
- 1.29.0 — `CHG-0077` (D-0067): `diag.static-assert-not-constant`;
  `diag.const-not-constant` covers local constants (D-0066).
- 1.28.0 — `CHG-0075` (D-0065): `diag.assert-failed`.
- 1.27.0 — `CHG-0073` (D-0063): `diag.channel-zero-capacity`,
  `diag.channel-deadlock`.
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
