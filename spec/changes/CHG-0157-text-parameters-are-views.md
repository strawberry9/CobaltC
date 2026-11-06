# CHG-0157 — Text a function only reads is a `StringView`

Kind: Change record (`spec/02-schema.md` §2)
Status: ACCEPTED (2026-10-03; D-0134)
Governed by: `CobaltC_Master_Instructions.md` §13
Depends on: D-0134
Affects: `spec/21` (4.0.0) §0, §2a, §2e, §2e′, §2h; `spec/15` (`rule.type.expected`); `spec/22` §5; `spec/conformance.md`; the guide; `impl/src/prelude.rs`, `impl/src/views.rs`, `impl/src/typecheck.rs`, `impl/src/interp.rs`, `impl/cobc/src/lower.rs`

## What changed

- **`StringView::of(str s) : StringView`** (`spec/21` §2h), over the std-only intrinsic `str_slice`: a view of a `str`'s bytes, valid for the rest of the program. It is the one function returning a `StringView` with no borrowed parameter (`rule.temporal.elision`).
- **`[Str-Literal-View]`** (`spec/21` §2a, `spec/15` `rule.type.expected`): a `str` literal, or a `str` constant's use, where a declared `StringView` is expected (a parameter, a local's declared type, a struct literal's field) is `StringView::of(L)`.
- **Parameters now `StringView`:** `read_file`, `write_file` (both), `read_bytes`, `write_bytes` (the path), `File::open`, `create`, `append`, `open_rw`, `make_dir`, `remove_file`, `remove_dir`, `rename` (both), `list_dir`, `path_kind`, `env_var`; the needle of `StringView::find`/`starts_with`/`ends_with` and of `String::find`/`starts_with`/`ends_with`.
- **`File::write_str(ref<String, shared>)` → `File::write_text(StringView)`**; `File::printf` is `File::write_formatted(f, $fmt(…))` (private).
- `split`'s separator stays `str` (`rule.temporal.elision`); the `_str` twins stay.
- **Previous semantics:** those parameters were `ref<String, shared>` or `str`; a literal path was `diag.type-mismatch`.
- **Rows:** `conf.str-literal-as-view`, `conf.view-of-str`, `conf.view-of-str-binding-rejected`, `conf.string-needle`, `conf.read-file-literal-path`; every row naming a changed function rewritten (`&p` → `&p[0..$]`).

## Compatibility classification

Breaking (source): a `ref<String, shared>` argument is now `diag.type-mismatch` with the repair `&s[0..$]`; a `str` binding as a needle needs `StringView::of`. A literal argument that was rejected is accepted.
