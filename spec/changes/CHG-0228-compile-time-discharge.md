# CHG-0228 — Compile-time discharge (D-0200): objectless locals and lent parameters, then proofs across calls

Kind: Change record (`spec/02-schema.md` §2)
Status: ACCEPTED (2026-10-10; D-0200)
Governed by: `CobaltC_Master_Instructions.md` §13
Depends on: D-0200
Affects: `spec/conformance.md` 3.193.0; `impl/cobc/src/lower.rs`, `impl/cbrt/src/lib.rs`, `impl/cbrt/include/cbrt.h`

## What changed

- **`cobc`, phase 1:**
  - Writer parameters (`Gen::writer_param`).
  - Objectless locals of any container type (`Gen::objectless_type`), with moved flags.
  - Native `String::new`/`Vec::new`, `HashSet<String>::insert` and `PriorityQueue` over objectless elements.
  - Fused `foreach` over `chars`/`split_whitespace`.
- **`cobc`, phase 2** (D-0200 lists each proof):
  - Element references held in locals with no path (`elem_ref_lets`).
  - Confined shared `Vec` readers (`held_vec_reader`), and `foreach` element aliases over exclusive holders.
  - Quiet container locals (`quiet_locals`, `drop_quiet_checks`).
  - Readers and writers extended: derived aliases, `as_view`, fused loops in pure views, compound assignments,
    `match (&r.f)`.
  - Read-only closures marked pure.
  - `__raw` forms for `std` and native functions, raw sinks for a `__raw` form's last `match`, and raw copies
    (`lower_raw_string`) into fields, pushes, set insertions and channel sends.
  - Objectless match binders; `replace(&mut x, …::new())` pushed whole.
  - `fold_zero_tokens` before `resolve_scopes`.
  - Natives:
    - `Channel::send`/`recv` (and a `_raw` send);
    - `File::write`, `File::write_text`, `read_line`, `File::read_line`;
    - `Result::unwrap_or`, `Vec::sorted_order`;
    - `HashMap::get`'s `_acc` read under `Option::unwrap`;
    - `HashSet::insert`'s `_raw` form.
- **`cbrt`:**
  - `cb_lock_bare`, `cb_unlock_bare` and `cb_unlock_signal`: a channel's lock with no guard.
  - `cb_drop_plain_buf` (`cbrt.h`, inline): an objectless buffer freed where no object can lie over it.
- **Conformance:**
  - `conf.objectless-lend-and-move`, `conf.objectless-moved-then-used`, `conf.raw-form-move-to-call`;
  - `conf.fused-text-loops`;
  - `conf.elem-ref-local`, `conf.elem-ref-local-conflict`;
  - `conf.map-get-unwrap-read`, `conf.map-get-unwrap-none`;
  - `conf.file-read-line-utf8-error`, `conf.spawn-in-raw-form`, `conf.binder-moved-into-literal`.

## What did not change

No rule, diagnostic or observable behavior; `coby` is unchanged. Two proofs were found unsound under D-0107 and are
not made (D-0200, *Withdrawn*).
