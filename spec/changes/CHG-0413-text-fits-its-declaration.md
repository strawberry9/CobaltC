# CHG-0413 — Text fits its declaration: `[Coerce-Text]`, `[Coerce-Slice]`, `probe<T>`, `[Cmp-Text]` across types

Status: implemented (2026-11-06), D-0312
Affects:
- `spec/06` 1.22.0: `[Cmp-Text]` orders across text types; `[Cmp-Through-Shared-Ref]`.
- `spec/09` 1.8.1.
- `spec/10` 1.5.0: the `from` clause in `rule.temporal.elision`; `[Result-Source-Form]`,
  `[Result-Source-Probe-Only]`.
- `spec/12` 1.28.0: `rule.type.coerce-text`, `rule.type.coerce-slice`, `rule.type.probe`.
- `spec/16` 1.33.2: `[Field-Access-Auto-Deref]` at every level.
- `spec/21` 4.80.0: the "Text" preamble; the text rules as clauses; the lookups' probes; `String::copy`; the
  `to_ascii_*` of a view; five functions no longer exported.
- `spec/22` 2.60.0: `result-source`.
- `spec/registry/features.md` 1.7.0: `feat.result-source-clause`.
- `spec/registry/diagnostics.md` 1.63.0, `spec/conformance.md` and `spec/examples.md` 3.22.0.
- `IMPLEMENTATION-NOTES.md`: `cb_str_string` removed.
- `coby`, `cobc`, `cbrt` and `cbc`.
- The guide and the book.

## What changes

1. **`[Coerce-Text]`** converts text at every position whose expected type the checker knows. This replaces
   `[Str-Literal-View]`, `[Str-Literal-String]`, `[Ref-String-View]` and `[Str-Value-View]`, which become
   clauses of it, and the syntactic pre-pass that applied them.
   - It now also covers a result, an array element, a payload, a closure's result, and a generic parameter
     fixed by an argument or by the expected type.
   - A `ref<String, exclusive>`, and a reference a call returns, are viewed too.
   - A node converted in one instantiation of a generic body and not in another is `diag.type-mismatch` at
     the node.
2. **`[Coerce-Slice]`**: `&v` and `&mut v` of a `Vec` or an array where a slice is declared, and a reference
   to one; it fixes a slice parameter's element type. The corpus's whole-range slices were rewritten to `&v`
   by the checker's own report of each site.
3. **`[Cmp-Text]`** orders any two text types as it compares them, and **`[Cmp-Through-Shared-Ref]`** compares
   one shared reference with a value of its referent type.
4. **`probe<T>`**, and the lookups that take it. A lookup by `&k` of a number key is now `k`, and a `String`
   value key is `&s`. `entry` copies an absent key only.
5. **D-0308's static `String` is gone**: `[Str-Literal-String-Ref]`, `String::static_ref`, `str_string` and
   `cb_str_string`.
6. **`std`**:
   - `String::copy` replaces `String::from_view`, with a tombstone.
   - `StringView::to_ascii_lower` and `to_ascii_upper` replace the `String` ones, with tombstones.
   - `String::eq`, `String::less`, `StringView::eq`, `StringView::less` and `String::as_bytes` are not
     exported.
7. **`--check`'s text lints**: `diag.text-copy-literal` and `diag.text-copy-whole`, as warnings.
8. **`[Field-Access-Auto-Deref]`** applies at every level, so a field through a reference to a reference is
   typed.
9. **The `from` clause**: `fn f(…) : τ from p` names the parameter a result's references come from; the other
   reference parameters are only probed (`diag.result-source-kept`), and a clause naming no such parameter, or on
   a result holding no reference, is `diag.result-source-invalid`. `StringView::split`'s separator is a
   `StringView` (`from v`), and so is the name `http`'s `find_param`, `find_header`, `Request::header`,
   `Response::header` and `PgConnection::parameter` look up.
10. **Conformance**:
   - New: `conf.coerce-text-*` (generated from the matrix), `conf.coerce-slice`,
     `conf.coerce-slice-exclusive-from-shared-rejected`, `conf.field-through-reference-to-reference`,
     `conf.probe-lookups`, `conf.probe-int-key-by-reference-rejected`,
     `conf.probe-struct-key-by-value-rejected`, `conf.probe-string-key-by-value-rejected`,
     `conf.probe-generic-lookup-disagrees-rejected`, `conf.probe-generic-parameter`,
     `conf.string-from-view-removed`, `conf.string-eq-not-exported`, `conf.string-to-ascii-of-view` and
     `conf.literal-amp-rejected`.
   - Gone with D-0308: `conf.literal-string-ref-param`, `-returned`, `-no-amp`, `-exclusive-rejected` and
     `-unfixed-k-rejected`.
   - Flipped to positive: `conf.ref-string-view-type-parameter` and `conf.text-mixed-order`.
   - Rows whose programs used a changed spelling use the new one, their ids unchanged.

Breaking (source):
- a lookup by `&k` of a number key, or by a `String` value;
- a literal or a `str` where a `ref<String, shared>` is declared;
- `String::from_view` and `String::to_ascii_*`;
- a call of the five functions no longer exported;
- a function value of `StringView::split` or one of the five lookups by name, whose parameter type changes.
