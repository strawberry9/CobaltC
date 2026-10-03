# D-0134 — Aligning `std` with one design

Status: ACCEPTED (2026-10-03, the owner: "proceed with your recommendations" on `private/std-proposal.md` §9, every default taken)
Kind: Decision (`spec/02-schema.md`)
Governed by: `CobaltC_Master_Instructions.md` §8 (item 3, semantic consistency), §9 (library helpers), §21, §22
Depends on: D-0020 (`str`), D-0024 (`std`), D-0041 (hash tables), D-0053 (`StringView`), D-0061, D-0079 (literal arguments typed by the call), D-0113 (`String::` views, `_str` lookups), D-0117, D-0120
Affects: `spec/21`, `spec/15` §5, `spec/20`, `spec/22` §5, `spec/conformance.md`, `spec/examples.md`, the guide, `impl/src/prelude.rs`, both implementations, every program that names a renamed item

## Problem

`std` grew one decision at a time (D-0024 to D-0129), and each
addition was right on its own. Read as a whole (`private/std-proposal.md`
§2–§8), it follows a small set of conventions — namespace by the first
parameter's type, `new` constructs, `from_`/`as_`/`into_`/`to_`, `_mut`,
`_by`, `_at`, `is_` — and breaks them in a few places that every program
meets:

1. **Text parameters had three conventions.** Paths and names (14
   functions: `read_file`, `File::open`, `make_dir`, `env_var`, …) took
   `ref<String, shared>`, so a literal path was a type error and a
   program wrote `read_file(&String::from_str("x.txt"))`, allocating to
   read a constant. Needles (`find`, `starts_with`, `ends_with`) took only
   `str`, so a `String` could not be searched for. Equality and map
   lookups had `_str` twins. `File::write_str` took a `String`, not a
   `str`, despite its name. The root cause: no borrowed text type both a
   literal and a `String` can become without allocating. `StringView`
   is that type for a `String`; a view of a `str` had been deferred
   three times (D-0020, D-0053, D-0113) with no objection recorded.
2. **`remove` meant two things.** `Vec::remove` keeps the order of the
   rest; `HashMap::remove` and `HashSet::remove` moved the last entry
   into the hole, changing iteration order silently; the order-keeping
   one was `remove_ordered`.
3. **Two I/O errors.** `ReadError { Io, Utf8 }` for standard input and
   `FileError { NotFound, Denied, Io, Utf8 }` for files: reading a line
   gave a different error type for stdin and for a `File`.
4. **Names that broke the conventions.** `Rng::seed` (constructors are
   `new`); free `map_err` beside `Result::unwrap_or`, `Result::expect`;
   free `parse` beside `StringView::parse`; `HashSet::at` beside
   `HashMap::key_at`; the raw externs `write` and `read`, the most
   generic verbs in the namespace, beside the safe `File::write` and
   `File::read`.
5. **A missing sibling.** `HashMap::clear`, but no `HashSet::clear`.

## Constraints

- No new tokens, no new syntax; no implicit conversions (D-0006); no
  method-call syntax (`CHG-0132`); the std helper admission test
  (Master Instructions §9).
- `rule.temporal.elision`: a function returning a borrow (a
  `StringView` counts) takes exactly one borrowed parameter.

## Candidate designs

For 1: (A.1) every read-only text parameter is a `StringView`, with
`StringView::of(str)` and a literal typed by its context; (A.2) the same
with `StringView::of("…")` written at every literal; (A.3) needles only;
(A.0) leave it. For 2: (1) `remove` keeps order and `swap_remove` is the
constant-time reordering one; (2) a tombstoned table whose `remove` is
both order-keeping and amortized constant time; (3) leave it,
documented. For 3: delete `ReadError`; or also rename `FileError` to
`IoError`. For 4 and 5: the renames and the one addition, or not.

## Selected design

1. **`StringView` is the type of text a function only reads.**
   - `StringView::of(str s) : StringView`, an exported function of
     `std` over the std-only intrinsic `str_slice`: a view of a `str`'s
     bytes. A `str` is a value that never ends, so the view is valid for
     the rest of the program; `StringView::of` is the one function
     returning a `StringView` with no borrowed parameter
     (`rule.temporal.elision` names it).
   - **`[Str-Literal-View]`:** a `str` literal (or the use of a `str`
     constant, which is that literal) where a `StringView` is expected
     is `StringView::of(L)`. "Expected" is a type written in a
     declaration: the parameter of the function a call names, the
     declared type of a local, the field of a struct literal. A literal
     has no type until its context gives it one, as an unsuffixed `5` has
     none (D-0079); a `str` *binding* has type `str` and is not affected
     (`StringView::of(t)`). This is not a conversion between two values:
     nothing exists to convert until the literal is typed.
   - The 14 path and name parameters, `write_file`'s text, and the
     needles of `find`/`starts_with`/`ends_with` (on `StringView` and on
     `String`) are `StringView`. A `String` reaches one as `&s[0..$]`.
   - `File::write_str(ref<String, shared>)` is now
     `File::write_text(StringView)`; `File::printf` writes through the
     private `File::write_formatted`.
   - `split`'s separator stays `str`: `split` returns views of its
     first argument, so by `rule.temporal.elision` it may take only one
     borrowed parameter.
   - The `_str` twins (`String::eq_str`, `StringView::eq_str`,
     `HashMap::get_str` and kin) stay: `_str` now means one thing
     everywhere, "the twin that takes a `str`".
2. **`remove` keeps order everywhere.** `HashMap::remove` and
   `HashSet::remove` keep the order of the other entries (the former
   `remove_ordered`, which is gone); `HashMap::swap_remove` and
   `HashSet::swap_remove` are the constant-time removals that move the
   last entry into the hole (the former `remove`). `remove_str` keeps
   order, as `remove` does.
3. **One I/O error.** `ReadError` is gone; `read_line` returns
   `Result<Option<String>, FileError>` (`NotFound` and `Denied` do not
   occur for standard input, as `Utf8` does not for `read_bytes`).
   `FileError` keeps its name.
4. **Renames:** `Rng::seed` → `Rng::new`; `map_err` → `Result::map_err`;
   `parse` → `String::parse`; `HashSet::at` → `HashSet::key_at`;
   the externs `write` → `stdout_write`, `read` → `stdin_read`, and
   `std`'s private `write_err` → `stderr_write`.
5. **`HashSet::clear`**, as `HashMap::clear`.
6. **Presentation:** `spec/21` §0 gains a conventions paragraph and its
   table is grouped by type; `std`'s source keeps each helper beside
   its type.

Kept, with the reason: `HashMap::entry` (its 14 special cases in
`cobc`; the name is tolerable); the time units in the names
(`monotonic_ns`, `unix_seconds`, `sleep_ms`: the unit is the name's
job, a `Duration` type would be a new type for three functions);
`String::view`/`StringView::sub` and `Vec::index_shared`/`index_exclusive`
(the definitional targets of `[View-Form]` and `[Index]`, not meant to
be called); `push_le` under `Vec::` beside free `read_le` (the namespace
follows the first parameter: a `Vec` and a slice).

## Rejected alternatives

- **A.2** (explicit `StringView::of` at every literal): three tokens
  at the commonest call, for no safety gain — a literal's view is
  always valid.
- **A.3** (needles only): leaves a literal path allocating, and two
  conventions.
- **An implicit `&String` → `StringView` coercion:** an implicit
  conversion between two values (D-0006), and an invisible borrow.
- **A tombstoned hash table:** the best end state, but it changes
  `key_at`'s positions, `HashDrain`, and `cobc`'s map layout; the
  revisit path if an order-keeping `remove` shows in a profile.
- **`IoError`:** cosmetic once `ReadError` is gone, and 100+ sites.
- **Aliases for the old names:** an alias doubles the surface this
  decision shrinks, and the specification has no deprecation mechanism.

## Semantic rationale

No rule's meaning changes except where a name or a parameter type does.
`StringView::of` adds no state: its view borrows an object over the
`str`'s bytes that never ends, so `[View-Form]`'s borrow discipline
holds for it trivially. `[Str-Literal-View]` is typing, not evaluation:
the literal denotes the same bytes it always did.

## Human-usability impact

`read_file("config.txt")`, `File::create("out.txt")`,
`String::starts_with(&line, "#")` and `StringView::find(v, &needle[0..$])`
read as they should. A `String` path costs `[0..$]` (six characters)
where it cost nothing; a literal path costs nothing where it cost a
`String::from_str`. `HashMap::remove` no longer reorders silently.

## Compatibility impact

**Breaking, source only.** Programs naming a renamed item, passing a
`ref<String, shared>` where a `StringView` is now taken, a `str` binding
as a needle, or matching `read_line`'s error exhaustively with `Io` and
`Utf8` alone are rejected statically, each with a message naming the
repair (`&s[0..$]`, `StringView::of(t)`). One change is silent: a
program calling `HashMap::remove`/`HashSet::remove`/`remove_str` now
keeps the others' order, so its later iteration order can differ (it
can no longer be the surprising one). The repository's own programs were
migrated by `stress/std-align/migrate.py` (kept), which turns an old
`remove` into `swap_remove`, preserving behaviour exactly.

## Implementation-feasibility impact

`coby`: `str_slice` makes, once per text, a never-ending `Vec<u8>`
object over the interned bytes. `cobc`: `str_slice` borrows the object
`[Reclaim]` establishes over the literal's static bytes (one per
address). The literal-typing pass runs after `consts` (`impl/src/views.rs`).
Found on the way: `coby`'s `spawn` ran a copy of the function body,
losing the checker's per-expression records, so a view formed in a
thread was `diag.type-mismatch`; it now runs the body itself.

## Prior-art status

Rust: `&str` as the parameter type of read-only text, with `String`
reaching it by borrowing; `swap_remove` for the order-breaking removal;
`io::Error` as the one I/O error. Go: one `error`. The literal typed by
its context is CobaltC's own (D-0079's shape).

## Invariant traceability

`inv.str.utf8-validity` (a view of a `str` is of valid UTF-8 bytes);
`inv.temporal-validity` (the object a `str` view borrows never ends);
`inv.alias` (shared views only).

## Revisit conditions

- An order-keeping `remove` that is linear shows up in a real profile:
  the tombstoned table.
- A demonstrated need for `HashMap::reserve`, `is_empty`, a `Duration`.
- `std` doubling in size: nested modules (`std::io`, `std::fs`), which
  `import std;` would then have to bring in (`spec/17`).
