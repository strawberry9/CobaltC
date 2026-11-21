# CHG-0422 — `cbc`'s lowering lends a function's declaration instead of copying it per call, and a discarded result with a destructor of its own is a noted temporary

Status: implemented (2026-11-10), D-0315, D-0296; step 2 of the allocation plan (the agent's proposal of 2026-11-10 on the owner's speed test; the owner: "implement the proposal in full")
Affects: `impl/selfhost/back/state.cb` (`Gen.fn_copies`, `get_fn` and `give_fn`, moved from `gen.cb`;
`NativeCtx` without its copy), `gen.cb` (`lower_call`, `lower_fn`), `direct.cb` (`fn_decl` lends), `stage1a.cb`
(`pure_direct_in`), `strings4.cb` (`pure_view_in`), `natives.cb` (`raw_ret_ok`), `native_vec.cb` (`std_fallback`);
`impl/cobc/src/lower/stmts.rs` and `impl/selfhost/back/gen.cb` (`lower_discarded`, `lower_expr_stmt`);
`spec/conformance.md` 3.303.0 (one row). No rule changes. No program's outcome changes.

## What changes

1. **A function's declaration is lent, not copied per call.** `cobc` holds `FnDecl` behind `Arc` in `Items` and its
   lookups copy a pointer. The port read it as a value: `get_fn` copied a whole function, body included, for every
   call `lower_call` lowered (10% of `cbc`'s cycles on the Lisp interpreter). Now `Gen` keeps one copy per function,
   made on its first use after its body is loaded; `get_fn` (and `direct::fn_decl`) moves it out to its caller,
   and `lower_call`, `lower_fn`, `pure_direct_in`, `pure_view_in`, `raw_ret_ok` and `std_fallback` give it back
   (`give_fn`) when they are done; the native-body context no longer holds a copy. A use of the same function while its copy is lent (a call in its own
   argument) makes a copy, as before, and a caller that does not give its copy back costs the next one a copy. During
   the lowering only `load_body` writes the function table, and only for a pending body, so a kept copy is the
   declaration.

   **Not done: handles (`Rc`).** The plan proposed `Rc<FnDecl>`, `Rc<StructDecl>` and `Rc<EnumDecl>`. Built and
   measured, it removed the copies (the copy's share fell from 10.0% to 0.6%) but made `cbc` slower: 6.9 s against
   5.8 s on `large_200.cb`. An `Rc`'s value lies in raw cells, and while any object lies over raw cells the
   runtime's count of them (`cb_reclaimed_entries`) is not zero. Then every drop of a decided program takes the
   generic path (`cb_drop_local`) in place of its drop glue, and element accesses take their slow check: 21% of
   `cbc`'s cycles. A decided program that holds an `Rc` pays this today; a follow-up should let a value in raw
   cells that no view or slice reaches leave the count alone. The structs' and enums' copies (`layout::struct_decl`,
   `enum_decl`, 4.5% of the final source's cycles on the Lisp program) stay: their readers take `Gen` shared, and lending needs it exclusive.
2. **A discarded call result whose type has a destructor of its own is a noted statement temporary** (D-0315 step
   4's rule, which covered an argument's temporary but not a result nothing takes). In a decided program,
   `HashMap::insert(&mut m, k, Rc::new(d));`, `put(&mut slot, v);` and `_ = make();` no longer give the result a
   runtime object and a statement scope: it is registered in the function's table and ends where its statement
   ends, as `[Stmt-Exit]` says. Both toolchains. `conf.discarded-noted-results-end-with-statement` holds the order:
   the discarded value ends before the next statement, a discarded `Rc` another handle shares ends nothing, and a
   fault later in the body ends the locals and not the discarded values again.

## Measured

On the final source of this Update (all four records), against Update 175 on the same machine, median of five
runs of `--emit-c`:

| program | `cbc` before | after | `cbc` / `cobc` before → after |
|---|---|---|---|
| `complex_lisp.cb` | 3,956 ms | 3,103 ms (−21.6%) | 1.30 → 1.07 |
| `complex_typeinf.cb` | 2,398 ms | 2,098 ms (−12.5%) | 1.42 → 1.25 |
| `large_200.cb` | 5,498 ms | 5,111 ms (−7.0%) | 1.44 → 1.36 |
| all ten programs | 16.33 s | 14.43 s (−11.6%) | 1.22 → 1.11 |

The copies of `FnDecl` fell from 11.4% of `cbc`'s cycles on the Lisp program to 0.9%, and from 6.4% to 1.1% on
`large_200.cb`. `cbc`'s C is `cobc`'s on the showcases and the speed test (101 files), and the new row holds under
`coby`, `cobc` and `cbc` with gcc and clang.
