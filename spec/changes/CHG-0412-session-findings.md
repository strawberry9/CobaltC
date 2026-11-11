# CHG-0412 — Repairs and decisions from ten sessions of ordinary programs

Status: implemented (2026-11-06), D-0311
Affects: `spec/14` 1.43.2 (§6: a `foreach`'s hidden holder is live after its statement); `spec/17` 2.19.0
(`[Resolve-Unqualified]`: a module is passed over in value position); `spec/19` 1.21.1 (`[Spawn]`'s arguments typed
as a call's); `spec/21` 4.79.0 (`StringView::rfind`, `StringView::parse_radix`, `Vec::eq`); `spec/22` 2.59.0
(disambiguation (4): `Name<…> x`); `spec/conformance.md` 3.296.0; `IMPLEMENTATION-NOTES.md` (`cb_lock_pathless`);
`coby` and `cobc` (the static pass's kept expressions, its inference, its messages; the resolver; the parser; the
test runner; `coby --explore`); `cobc` (the lowering's kept nodes and exact facts, `==` against a variant, borrows
of proven pathless places, `Vec::len` of a temporary, free scope effects); `cbrt` (`cb_lock_pathless`); `cbc` (the
same, in `impl/selfhost`); the book (ch05, ch09, ch12) and the guide

## What changes

1. **`spec/14` §6**: a `foreach`'s hidden holder is live after the statement that makes it, so a loop over views of
   a header's temporary is refuted, `diag.destroy-while-aliased` (static), as D-0135 refutes a view binding used
   after its statement. Before, a decided body read the temporary's freed bytes.
2. **The static pass and the lowering depend on the program alone.** Facts keyed by an expression's address are
   never left behind by a freed expression: the `foreach` expansions the pass checks are kept (and lowered, so
   `cobc` reads the facts of the code it lowers), and so is every node either makes. No rule changes; a verdict
   that varied between runs of one file, and two internal errors that came and went, no longer do.
3. **`[Generic-Call-Uninferable]`** is applied to an inner generic call under an expected type not yet fixed for
   every type parameter its result does not mention (no expected type can fix one), as the rule always said.
4. **`[Resolve-Unqualified]` (`spec/17`)**: in value position a module that clauses (1)–(3) yield is passed over,
   as a type is (CHG-0334), and the clauses go on.
5. **Disambiguation (4) (`spec/22` §2)**: `identifier '<' type, … '>' identifier` then `=` or `;` is a local
   declaration.
6. **`[Spawn]` (`spec/19`)**: each argument is typed as the argument of a call of the function, so a text literal
   where a `String` is declared is that `String` (D-0149, made by D-0162's rewrite).
7. **`spec/21`**: `StringView::rfind`, `StringView::parse_radix<T: integer>`, `Vec::eq<T: eq>`. Additive.
8. **`cbrt`**: under a schedule, `cb_lock_pathless` is a point where the turn may pass, and must when a release made
   a switch pending: `cobc --explore` reaches a decided program's lock interleavings.
9. **`cobc`/`cbc`, no rule changed**: `f(0) == Err(Bad)`, `o == None` and `None == o` take the variant's type from
   the other operand; a shared borrow of a place reached with no path, every access proven (a predicate's
   parameter, a `foreach` read alias's element), is its address; `Vec::len(&f(x))` evaluates `f(x)` once; an atomic
   operation's flag read is free of scopes (195 ns → 10 ns a store and a load).
10. **Tools**: `coby --explore` runs schedules in parallel and feeds the input as it arrives; the unit-test runner
    counts `unsupported` tests apart and shows each label while its test runs; messages name the fix for `!` on an
    integer, `auto` as a parameter's type, `as<T>`, `: T` after a range of variables, a `str` for a `String`
    field, a sliced `str`, and a place lent to a thread.

## Conformance

`conf.foreach-view-loops-checked-alike`, `conf.foreach-views-of-temporary`,
`conf.generic-call-uninferable-in-generic-argument`, `conf.eq-against-untyped-variant`,
`conf.borrowed-call-result-evaluated-once`, `conf.index-through-proven-reference`,
`conf.import-module-function-named-like-module`, `conf.spawn-text-literal-string-argument`. Tests in `cobc`:
`deterministic_c` (the same C on every build), `driver`'s `explore_reaches_the_books_lost_update` and
`a_module_file_read_alone_declares_a_sibling_generic_type`.

## Compatibility

A program that looped over views of a header's temporary is rejected; it read freed memory compiled and faulted
under `coby`. A program whose `foreach` was rejected by a stale fact is accepted. An unqualified call of a module's
own name now calls the function the module exports. Everything else is additive.
