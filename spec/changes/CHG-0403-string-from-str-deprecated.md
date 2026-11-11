# CHG-0403 — `String::from_str` deprecated; every tracked tree written without it (D-0305)

Kind: Change record (`spec/02-schema.md` §2)
Status: ACCEPTED (2026-11-02)
Governed by: `CobaltC_Master_Instructions.md` §21
Depends on: D-0305, D-0303, D-0304
Affects: `impl/std/text.cb` (the attribute) and every `std` file that called it; `spec/21` (4.74.0: the listing, §2a,
§2f `[Sprintf]`, `type.str`), `spec/12` (1.27.0), `spec/03` (1.6.4), `spec/23` (1.0.2), `spec/examples.md`,
`spec/conformance.md`; the messages that named it (`impl/src/typecheck.rs`, `impl/src/lib.rs`, their self-hosted
twins); `impl/selfhost` (every source), the conformance cases, the CobaltC programs in the Rust tests
(`impl/tests/conformance.rs`, `impl/tests/common/mod.rs`, `impl/cobc/tests/{driver,tracking}.rs`), `impl/fuzz/fuzz.cb`,
the guide (`content_a.py`, `content_b.py`, `content_c.py`, `content_intro.py`, `content_cprog.py`), the book (ch06,
ch08, ch10, ch11, ch12, ch15 and their examples), the showcases; `stress/` (not tracked)

## What changed

1. `String::from_str` carries `[[deprecated("…")]]`; a call of it is `diag.deprecated-use` unless
   `--allow-deprecated`. Its own `unittest`s stay (they follow it).
2. Every call in the tracked trees was rewritten by `stress/retire/migrate.py` (`migrate2.py` for the trees after
   `impl/selfhost`): a literal or a constant's name where the program's checked verdict is unchanged; `String::from_view`
   otherwise; `String::from_view(&p)` of a whole `String` as `String::clone(&p)`. `stress/retire/unwrap_if.py` then
   wrote `String::from_view(if (c) { "a" } else { "b" })` as the `if` where the verdict is unchanged (D-0155).
3. Where a function of our own took `ref<String, shared>` only to read text and was called with
   `&String::from_view("…")`, the parameter is a `StringView` and the call passes the literal (proposal §6.3 step 4):
   eight functions of the self-hosted toolchain (`named_diag`, `refute`, `work`, `internal`, `path_is`, `eq_expr`,
   `halve_reduce`, `map_probe`), 32 calls. Not `pure_view` and `pure_direct`, whose key reaches `String`-keyed maps
   (a view there would be copied on each call), and not the `HashMap`/`HashSet` lookups, whose key type is generic.
4. The literal's `String` and `sprintf`'s result are still realized as the call `std::String::from_str` the front end
   writes after names are resolved (no use of a name is checked there), so the lowering, its natives and the C are
   unchanged until Update C renames that call. The proposal (§6.4) put the new target,
   `std::String::from_view(std::StringView::of(L))`, and `cobc`'s natives for it in this Update; they move to Update
   C, where the declaration becomes a tombstone and the target must change anyway, so the lowering and the static
   decisions change once, under one perf gate, instead of twice.
5. Conformance: `conf.std-from-str-deprecated` (✗ `diag.deprecated-use`), `conf.std-from-str-allowed` (ok under
   `--allow-deprecated`).

## Verification

- `COBALTC_CHECK_ALL=1 coby --check` of `cbc`, `cbparse` and the self-hosted `cobfmt` after the migration, all of
  `std` included; `impl/selfhost/build.sh` rebuilt them; every conformance case and `std`'s `unittest`s keep
  their outcomes (the `cargo test` run of `CHG-0401`).
- No call of `String::from_str` remains in a tracked program but the declaration, its `unittest`s and the two new
  cases; the name stays in the internal call the front ends write and the back ends know (item 3), in the records
  and in the `spec/21` listing; `stress/` keeps old copies of `std`'s declaration.
