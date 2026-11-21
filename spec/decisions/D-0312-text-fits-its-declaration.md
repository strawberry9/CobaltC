# D-0312 — Text fits its declaration: one coercion, probes for lookups, one comparison

Status: ACCEPTED (2026-11-06, the owner: "a new proposal I want you to implement in full, that is a significant
change", of the text proposal v2, with the work it paused left to this record's judgement: "I leave it to you to
solve all the issues autonomously"; and on its §3.5, first deferred, 2026-11-07: "yes, implement 3.5 in this
Update"); implemented by `CHG-0413`
Kind: Decision (`spec/02-schema.md`); typing rules and a type-level function (`spec/12`), comparison rules
(`spec/06`), a function's result source (`spec/10`, `spec/22`), and a `std` sweep (`spec/21`, `spec/STABILITY.md`
§3a)
Governed by: `CobaltC_Master_Instructions.md` §8, §9 ("one way of writing one thing")
Depends on: D-0006 (no implicit conversion), D-0011 and D-0047 (`rule.temporal.elision`), D-0053, D-0108 and
D-0164 (text comparison), D-0110 (`[Key-Bytes]`), D-0134, D-0149, D-0155, D-0162, D-0225 and D-0304 (a literal
typed by its position), D-0218 (`[Read-Exclusive-Through-Shared]`), D-0302 (`&s` and a `str` as views), D-0305
(`String::from_view`), D-0308 (the static `String` reference, deleted here)
Supersedes the typing content of: D-0134, D-0149, D-0155, D-0162's list of positions, D-0225, D-0302, D-0304,
D-0308's `[Str-Literal-String-Ref]`, and D-0164's mixed equality and its refusal to order mixed text; those
records stay as history

## 1. The problem

Text had three types and many rules turning one into another, each with its own list of positions: a literal
was a view in some, a `String` in others, a `String` reference in a third set; `&s` and a `str` were views in a
fourth. Equality crossed the types and ordering did not. A `String`-keyed map was probed through a static
`String` made for each literal, which never ended. Nearly every finding of the two text audits was a position
some list had missed, or one more rule a reader had to learn.

## 2. Decisions

1. **`[Coerce-Text]`** (`spec/12` `rule.type.coerce-text`). Wherever `rule.type.expected` knows a position's
   type:
   - A literal or a `str` constant is a `String` where `String` is declared (the one allocation), and a view
     where `StringView` is declared.
   - A `str` value is a view.
   - `&s` of a `String` place is `&s[0..$]`.
   - A `ref<String, m>` is `&(*e)[0..$]`, whatever gives it: a binding, a field or a call's result. An exclusive
     one counts too, since a view only reads.

   Nothing else: no `String` by value becomes a view, and nothing becomes a `str` or a `ref<String, _>`. Each
   conversion is the expression a writer could have spelled. The front ends write it out and check the program
   again, so the back ends see nothing new. The five rules and their lists of positions become clauses of this
   one, and the old syntactic pre-pass is gone.
2. **`[Coerce-Slice]`** (`spec/12` `rule.type.coerce-slice`), the same rule for every sequence. `&v` (`&mut v`)
   of a `Vec<T>` or an `array<T, N>` where a `slice<T, m>` is declared is `&v[0..$]` (`&mut v[0..$]`). A
   reference to one is `&(*e)[0..$]` where the mode allows it. Such an argument fixes a slice parameter's
   element type. `[lo..hi]` is now written only where a part is meant.
3. **D-0006 has exactly two coercions**, `[Coerce-Text]` and `[Coerce-Slice]`, and `spec/12`'s header and
   `spec/22` §5 say so. The record D-0006 itself is history and is not edited.
4. **In a generic body**, a node the rule converts in one instantiation and leaves in another is a type error
   at the node (`[Coerce-Text-Generic-Disagree]`, generalising D-0162's guard). One body cannot be both.
5. **`[Cmp-Text]`** (`spec/06`). `==`, `!=`, `<`, `<=`, `>` and `>=` between any two of `str`, `String` and
   `StringView`, either side, compare the bytes. A `String` operand is borrowed, never moved.

   **`[Cmp-Through-Shared-Ref]`**: a `ref<τ, shared>` compared with a value of `τ` (or text with text) compares
   its referent. This is narrowed from the proposal: two references keep `[Eq-Ref]`, identity, because
   `rule.ref.identity-eq` is normative and has its own cases.
6. **`probe<T>`** (`spec/12` `rule.type.probe`) is the type that probes a `T`:
   - `probe<String>` is `StringView`;
   - a number, `bool` or `str` is its own probe;
   - any other type is probed by `ref<T, shared>`.

   The proposal's "plain" (`is-resource` false) is decided here from the type alone. A plain struct or enum is
   probed by reference, as the proposal's own example has it ("a struct-keyed map is probed with `&key` as
   today"), so every substitution can evaluate the probe without a table of items. Written with a type
   parameter, it stays `probe<K>` until `K` is fixed, and it fixes no type parameter itself.

   The `ref`-only fallback the proposal offered was tried and refused. `entry` takes the probe, so integer
   counting would have needed `&key`, and a computed key (`i % 7`) a binding.
7. **The lookups take probes**: `HashMap::get`, `get_mut`, `contains`, `remove`, `swap_remove` and `entry`;
   `HashSet::contains`, `remove` and `swap_remove`; `Vec::contains`, `index_of` and `binary_search`. `insert`
   keeps its key by value. `entry` copies the key only when it adds it: a view by `String::copy`, a value as it
   is, a referent by `clone`. Counting words therefore makes one `String` per distinct word. Each of these
   works:
   - `HashMap::get(&counts, 7)`
   - `HashMap::get(&m, "k")`
   - `HashMap::get(&m, word)`
   - `HashMap::get(&m, &s)`
   - `entry(&mut m, i % 7, 0)`

   Generic code looking up by a `ref<K>` across instantiations takes `probe<K>` instead.
8. **D-0308 is deleted**: `[Str-Literal-String-Ref]`, `String::static_ref`, the `str_string` intrinsic and the
   runtime's `cb_str_string` intern table. A literal is never a `ref<String, _>`.
9. **The `std` sweep**:
   - `String::copy(StringView) : String` replaces `String::from_view`, which is retired with its replacement
     named.
   - `StringView::to_ascii_lower` and `to_ascii_upper` take the view and replace the `String` ones.
   - `String::eq`, `String::less`, `StringView::eq`, `StringView::less` and `String::as_bytes` are no longer
     exported. Programs write the operators and `StringView::as_bytes(&s)`.
10. **The `from` clause** (proposal §3.5; `spec/10` `rule.temporal.elision`, `spec/22` `result-source`).
    `fn f(…) : τ from p` names the parameter the result's references come from. `[Call-Elided-Lifetime]` uses
    it where it uses "the one reference parameter", and `[Call-Multi-Ref-Return-Rejected]` does not apply. The
    body is checked so that the name is true: every other parameter that may hold a reference (a type
    parameter counts) is only *probed* (`[Result-Source-Probe-Only]`, `diag.result-source-kept`), read for a
    value holding no reference of it. An occurrence may pass through derivations (a field, an element, `*`, a
    borrow, a call whose result may hold a reference) and must end in a probe: an operator's operand, a read of
    a value holding no reference, or an argument of a named function whose result holds no reference (or whose
    own `from` clause names another parameter) and none of whose parameters could receive one. Anything else, a
    binding, a store, the result, a capture, a `match` over it, a call through a `fn` value, keeps it.
    - **Why this is sound where the deferral said it was not yet statable.** The deferral asked for a proof
      that every reference the result holds derives from `p`, through the elements of a `Vec<StringView>`.
      Probe-only asks the converse, which needs no flow: no reference of another parameter is ever held by
      anything but the expression reading it, so nothing the body builds can hold one. It rejects some sound
      bodies (a local copy of the separator, then probed) and is checked in each instantiation, where a type
      parameter may be a view.
    - `feat.explicit-lifetime-parameters` stays `REJECTED`: the clause relates one input to the result and
      names no lifetime (`feat.result-source-clause`). A result from either of two inputs, the guide's
      `longest(a, b)`, is still not expressible.
    - `[Result-Source-Form]` (`diag.result-source-invalid`): the clause names a parameter of reference type,
      and the result's type may hold a reference.
    - **In `std`**: `StringView::split(StringView v, StringView sep) : Vec<StringView> from v`, so the separator
      is any text, as every sibling's needle is. The lookups by name that returned a view for the same reason
      take a `StringView` name too: `http`'s `find_param`, `find_header`, `Request::header` and
      `Response::header`, and `PgConnection::parameter`. A literal or a `str` there is a view by `[Coerce-Text]`,
      so no call changes.
11. **Deferred:**
    - **A static slice from a `byte-literal`.** `b"…"` stays the `array<u8, N>` value `spec/22` defines;
      `&b"…"` is a slice as an argument, as any borrowed temporary is. A slice of static bytes elsewhere needs
      `[Ref-Form-Const]` for slices (`&K[lo..hi]` of a constant) in every tool, which is a design of its own.
12. **Lints of `--check`** (§4 of the proposal):
    - `diag.text-copy-literal`: `String::copy("…")` where a `String` is declared, since the literal already is
      that `String`.
    - `diag.text-copy-whole`: `String::copy(&s)` of a whole `String`; `String::clone(&s)` says what happens.

    These are warnings; the program is accepted.

## 3. Found while implementing

- **Field access through a reference to a reference** was left untyped by both checkers: a binder of
  `match (&o)` with `o` an `Option<ref<S>>`. A text field there was not viewed where a view is declared.
  `[Field-Access-Auto-Deref]` now applies at every level, as the tools already read it.
- **Probes passed on in the back ends.** A probe parameter forwarded to another lookup, a struct key by
  reference, was classified by the callee's declared type, `probe<T>`, rather than its instantiated one. The
  back ends now use the instantiated type: the probe's C type, its pure-view status and the search's direct
  key.
- **`entry` with a `String` key** lost `cobc`'s native form when its key became a probe, and counting words ran
  2.8 times the instructions. The native form now probes with the view and copies its bytes only for a new key,
  as std's body does, and counting is below its count before this record.
- **A native body taking a view must receive it.** `StringView::to_ascii_lower` kept its body for a `String`
  reference, which never received the view a tracked caller sends, so the borrow outlived the call. The body now
  receives it, as `trim`'s does.
- **The matrix** (`stress/text_v2/matrix.py`) checks every text form at every declared type and position: 504
  cells, each tool. Its accepted cells and two positions of its rejected ones are the conformance cases
  `conf.coerce-text-*` (`stress/text_v2/gen_conf.py`). The whole matrix is a sweep.
