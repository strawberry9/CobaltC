// A transcription of a substantial subset of spec/conformance.md's cases
// into automated tests, per impl/STATUS.md's honesty ground rules.
//
// As of this pass, `src/typecheck.rs` adds a real (though deliberately
// narrowed and conservative -- see that module's own doc comment) static
// pass ahead of execution: whole-program type-checking per spec/12,
// spec/06 `rule.arith.literal`'s literal range/defaulting, literal-
// operand constant-fold overflow refutation (spec/14 §6), closure
// capture-list-mismatch (spec/15 §6), match exhaustiveness (`rule.agg.
// match`), and `rule.control.flow-analysis` (spec/14 §6) in full -- so
// the flow-analysis-decided facts spec/conformance.md documents as
// `(static)` are rejected by the static pass here too. It is
// conservative elsewhere (falls back to "unknown, don't check" rather
// than guess) — `expect_ok`'s own static-pass assertion below (run
// against literally every case in this file) is the regression guard
// that keeps this honest: it fails loudly the moment the static pass
// would reject a program this interpreter otherwise accepts.
//
// Cases are transcribed where this interpreter can actually be expected
// to reproduce the documented outcome:
//
// - Every `→ v` (value) case: a direct test of evaluator correctness.
// - Every `ok` case.
// - Every diagnostic case, whether caught statically (an explicit
//   `check_source_static` assertion) or only dynamically (`expect_diag`
//   and friends, via the evaluator).
//
// A small number of pure static well-formedness facts this pass's
// narrowed checker does not cover (e.g. `diag.unbounded-type-parameter`,
// `diag.cannot-infer-type-parameter`) remain omitted rather than faked;
// see impl/STATUS.md.

// CHG-0033: each program here stands for a whole program file, which
// reaches the standard library through `import std;` at its root; the
// suite supplies that first line, as `tests/common/mod.rs` does for the
// specification's table fragments.
fn with_std(src: &str) -> String {
    format!("import std;\n{}", src)
}

fn run_source(src: &str) -> Result<(), String> {
    coby::run_source(&with_std(src))
}

fn check_source_static(src: &str) -> Result<(), String> {
    coby::check_source_static(&with_std(src))
}

fn expect_ok(src: &str) {
    match run_source(src) {
        Ok(()) => {}
        Err(d) => panic!("expected ok, got diag `{}` for:\n{}", d, src),
    }
    // Regression guard for the static pass (src/typecheck.rs): run
    // against every `expect_ok`-asserted fragment in this suite. Any
    // currently-`ok` program the static pass would reject is a false
    // positive bug in that pass, not a real rejection -- surfaced loudly
    // here rather than silently wired into the default execution path
    // where it would just look like a mysterious new failure.
    if let Err(d) = check_source_static(src) {
        panic!("STATIC PASS FALSE POSITIVE: rejected `{}` for known-ok program:\n{}", d, src);
    }
}

// Asserts `src` is rejected by the static pass alone, before any
// execution -- for cases spec/conformance.md documents as `(static)`
// that `src/typecheck.rs` actually implements (see its own doc comment
// for which subset that is).
fn expect_diag_static(src: &str, diag: &str) {
    match check_source_static(src) {
        Err(d) => assert_eq!(d, diag, "for:\n{}", src),
        Ok(()) => panic!("expected a static rejection (`{}`) but the static pass accepted:\n{}", diag, src),
    }
}

fn expect_ok_body(body: &str) {
    expect_ok(&format!("fn main()\n{{\n{}\n}}\n", body));
}

// Uses `1/0` (div-by-zero) as the "assertion failed" sentinel for a
// boolean condition — safe because no test below both relies on this
// helper AND expects div-by-zero as its *real* outcome.
fn expect_true(expr: &str) {
    let src = format!("fn main()\n{{\n    if (!({}))\n    {{\n        1/(1-1);\n    }}\n}}\n", expr);
    match run_source(&src) {
        Ok(()) => {}
        Err(d) => panic!("assertion failed (`{}`): terminated with `{}`", expr, d),
    }
    if let Err(d) = check_source_static(&src) {
        panic!("STATIC PASS FALSE POSITIVE: rejected `{}` for known-ok program:\n{}", d, src);
    }
}

fn expect_diag_body(body: &str, diag: &str) {
    let src = format!("fn main()\n{{\n{}\n}}\n", body);
    expect_diag(&src, diag);
}

// Struct/enum declarations are top-level items in this grammar, never
// statements — a fragment that shows one inline is spec shorthand for
// "declared alongside `fn main`", not "nested inside it".
fn expect_true_with(items: &str, expr: &str) {
    let src = format!(
        "{}\nfn main()\n{{\n    if (!({}))\n    {{\n        1/(1-1);\n    }}\n}}\n",
        items, expr
    );
    match run_source(&src) {
        Ok(()) => {}
        Err(d) => panic!("assertion failed (`{}`): terminated with `{}`", expr, d),
    }
    if let Err(d) = check_source_static(&src) {
        panic!("STATIC PASS FALSE POSITIVE: rejected `{}` for known-ok program:\n{}", d, src);
    }
}
fn expect_ok_with(items: &str, body: &str) {
    expect_ok(&format!("{}\nfn main()\n{{\n{}\n}}\n", items, body));
}
fn expect_diag_with(items: &str, body: &str, diag: &str) {
    expect_diag(&format!("{}\nfn main()\n{{\n{}\n}}\n", items, body), diag);
}

fn expect_diag(src: &str, diag: &str) {
    match run_source(src) {
        Ok(()) => panic!("expected diag `{}` but program ran to completion:\n{}", diag, src),
        Err(d) => assert_eq!(d, diag, "for:\n{}", src),
    }
}

// ---------- 1. Arithmetic (spec/06) ----------

#[test]
fn conf_i32_add_in_range() {
    expect_true("2000000000: i32 + 100000000: i32 == 2100000000");
}

#[test]
fn conf_i32_add_overflow_static() {
    // covers: conf.i32-add-overflow-static
    // Both operands literal -> statically refutable (spec/14 §6),
    // rejected before `main` ever runs, not merely a runtime fault --
    // now actually asserted, not just diagnostic-name-checked.
    expect_diag_static("fn main()\n{\n2147483647: i32 + 1: i32;\n}\n", "diag.arith-overflow");
}

#[test]
fn conf_i32_add_overflow_dynamic() {
    expect_diag(
        "fn add(i32 a, i32 b) : i32 { a + b }\nfn main() { add(2147483647, 1); }",
        "diag.arith-overflow",
    );
}

#[test]
fn conf_i32_div_zero_dynamic() {
    expect_diag(
        "fn d(i32 a, i32 b) : i32 { a / b }\nfn main() { d(1, 0); }",
        "diag.div-by-zero",
    );
}

#[test]
fn conf_i32_div_min_neg_one() {
    expect_diag(
        "fn d(i32 a, i32 b) : i32 { a / b }\nfn main() { d(-2147483647 - 1, -1); }",
        "diag.div-overflow",
    );
}

#[test]
fn conf_u8_wrapping_add() {
    expect_true("wrapping_add(255: u8, 1: u8) == 0");
}

#[test]
fn conf_checked_add_none() {
    expect_ok_body(
        "match (checked_add(255: u8, 1: u8)) { Some(_) : { 1/(1-1); }, None : {}, }",
    );
}

#[test]
fn conf_checked_add_some() {
    expect_ok_body(
        "match (checked_add(1: u8, 1: u8)) { Some(v) : { if (v != 2) { 1/(1-1); } }, None : { 1/(1-1); }, }",
    );
}

#[test]
fn conf_shift_in_range() {
    expect_true("1: u32 << 31: u32 == 2147483648: u32");
}

#[test]
fn conf_shift_out_of_range() {
    expect_diag_body("1: u32 << 32: u32;", "diag.shift-amount-out-of-range");
}

#[test]
fn conf_shr_arithmetic() {
    expect_ok("fn s(i32 x) : i32 { x >> 1 }\nfn main() { if (s(-8) != -4) { 1/(1-1); } }");
}

#[test]
fn conf_literal_out_of_range() {
    expect_diag_body("256: u8;", "diag.literal-out-of-range");
}

#[test]
fn conf_narrow_overflow() {
    expect_diag(
        "fn n(i32 x) : u8 { narrow<u8>(x) }\nfn main() { n(300); }",
        "diag.narrowing-overflow",
    );
}

#[test]
fn conf_neg_in_range() {
    expect_true("-(5: i32) == -5");
}

#[test]
fn conf_neg_min_overflow() {
    expect_diag(
        "fn ng(i32 m) : i32 { -m }\nfn main() { ng(-2147483647 - 1); }",
        "diag.arith-overflow",
    );
}

#[test]
fn conf_cmp_le_ge_ne() {
    expect_true("1: i32 <= 1: i32");
    expect_true("2: i32 >= 1: i32");
    expect_true("1: i32 != 2: i32");
}

#[test]
fn conf_float_nan_ge_false() {
    expect_ok("fn z() : f64 { 0.0 }\nfn main() { if ((z() / z()) >= 1.0) { 1/(1-1); } }");
}

#[test]
fn conf_bitwise() {
    expect_true("(6: u8 & 3: u8) | (8: u8 ^ 8: u8) == 2");
}

// ---------- 2. Values, local declarations, bindings ----------

#[test]
fn conf_let_copy() {
    expect_true("{ i32 x = 1; auto y = x; y + x == 2 }");
}

#[test]
fn conf_let_uninit_then_assign_ok() {
    expect_ok_body("i32 x; x = 5; if (x != 5) { 1/(1-1); }");
}

#[test]
fn conf_definite_assignment_both_branches() {
    expect_ok(
        "fn f(bool c) : i32 { i32 x; if (c) { x = 1; } else { x = 2; } x }\nfn main() { f(true); f(false); }",
    );
}

#[test]
fn conf_shadowing() {
    expect_true("{ i32 x = 1; i32 x = 2; x == 2 }");
}

#[test]
fn conf_unbound_name() {
    expect_diag("fn f() : i32 { y }\nfn main() { f(); }", "diag.unbound-name");
}

// ---------- Module visibility (spec/17) ----------
//
// No existing spec/conformance.md row exercises `rule.module.visibility`
// or `rule.module.use` beyond `conf.unbound-name` above (checked: no
// `name-not-visible`/`ambiguous-name`/`import` row exists to transcribe),
// so these are newly written, not transcribed. `rule.module.visibility`
// is "purely lexical; always disposition: rejected" (no dynamic fallback
// exists in the spec for it), so every rejection case here uses
// `expect_diag_static` rather than the dynamic `expect_diag`.

#[test]
fn conf_module_private_item_not_visible() {
    expect_diag_static(
        r#"
        module m
        {
            fn secret() : i32
            {
                1
            }
        }
        fn main()
        {
            m::secret();
        }
        "#,
        "diag.name-not-visible",
    );
}

#[test]
fn conf_module_export_item_visible() {
    expect_ok(
        r#"
        module m
        {
            export fn greet() : i32
            {
                1
            }
        }
        fn main()
        {
            m::greet();
        }
        "#,
    );
}

// "A private item is usable from its own module and its sub-modules"
// (`rule.module.visibility`): `secret` (private, in `m`) is called
// unqualified from `m::inner` (a sub-module of `m`), resolved via
// `[Resolve-Unqualified]` clause (3) -- which carries no visibility
// premise at all, so `secret`'s own privacy never blocks it here.
#[test]
fn conf_module_private_item_visible_from_submodule() {
    expect_ok(
        r#"
        module m
        {
            fn secret() : i32
            {
                1
            }
            export module inner
            {
                export fn call_secret() : i32
                {
                    secret()
                }
            }
        }
        fn main()
        {
            m::inner::call_secret();
        }
        "#,
    );
}

// An `export` item behind a *private* intermediate module is still
// unreachable from outside that module -- every hop of a qualified path
// is visibility-checked here, not only the final one (`rule.module.
// visibility`'s prose: "An export item inside a private module is
// reachable only where that module is").
#[test]
fn conf_module_export_inside_private_module_not_visible() {
    expect_diag_static(
        r#"
        module m
        {
            module inner
            {
                export fn call_secret() : i32
                {
                    1
                }
            }
        }
        fn main()
        {
            m::inner::call_secret();
        }
        "#,
        "diag.name-not-visible",
    );
}

// `import p;` makes `p`'s last segment resolve, unqualified, within the
// importing module (`rule.module.use`) -- `helper` is not declared at
// the root at all, so without the `import` this would be
// `diag.unbound-name`.
#[test]
fn conf_module_import_enables_unqualified() {
    expect_ok(
        r#"
        module m
        {
            export fn helper() : i32
            {
                42
            }
        }
        import m::helper;
        fn main()
        {
            helper();
        }
        "#,
    );
}

// Importing a non-existent/non-visible path is itself ill-formed at the
// `import` declaration (`rule.module.use` depends on `rule.module.
// resolve`): `secret` is private, so `import m::secret;` cannot resolve
// its own target.
#[test]
fn conf_module_import_of_invisible_item_rejected() {
    expect_diag_static(
        r#"
        module m
        {
            fn secret() : i32
            {
                1
            }
        }
        import m::secret;
        fn main()
        {
        }
        "#,
        "diag.name-not-visible",
    );
}

// `[Resolve-Ambiguous]`: two `import`s whose last segment is the same
// name yield more than one candidate.
#[test]
fn conf_module_ambiguous_import() {
    expect_diag_static(
        r#"
        module a
        {
            export fn helper() : i32
            {
                1
            }
        }
        module b
        {
            export fn helper() : i32
            {
                2
            }
        }
        import a::helper;
        import b::helper;
        fn main()
        {
            helper();
        }
        "#,
        "diag.ambiguous-name",
    );
}

// ---------- 3. Resource authority ----------

#[test]
fn conf_transfer_invalidates_source() {
    expect_diag_body(
        "Vec<i32> v = Vec::new(); auto w = v; Vec::push(&mut v, 1);",
        "diag.stale-binding",
    );
}

#[test]
fn conf_double_destroy_rejected() {
    expect_diag_body("Vec<i32> v = Vec::new(); drop(v); drop(v);", "diag.stale-binding");
}

#[test]
fn conf_leak_free_by_construction() {
    expect_ok_body("{ Vec<i32> v = Vec::new(); }");
}

#[test]
fn conf_unstored_temporary_destroyed_at_stmt_end() {
    expect_ok("fn make() : Vec<i32> { Vec::new() }\nfn main() { make(); }");
}

#[test]
fn conf_returned_resource_destroyed_at_caller_exit() {
    expect_ok(
        "fn make_vec() : Vec<i32> { Vec<i32> v = Vec::new(); Vec::push(&mut v, 1); v }\nfn main() { { auto r = make_vec(); } }",
    );
}

#[test]
fn conf_early_return_destroys_locals() {
    expect_ok(
        "fn f() : i32 { Vec<i32> v = Vec::new(); if (true) { return 1; } 2 }\nfn main() { if (f() != 1) { 1/(1-1); } }",
    );
}

#[test]
fn conf_destroy_while_borrowed_rejected() {
    expect_diag_body(
        "Vec<i32> v = Vec::new(); auto r = &v; drop(v);",
        "diag.destroy-while-aliased",
    );
}

#[test]
fn conf_destroy_after_borrow_scope_ends_ok() {
    expect_ok_body("{ Vec<i32> v = Vec::new(); auto r = &v; }");
}

#[test]
fn conf_move_while_borrowed_rejected() {
    expect_diag_body(
        "Vec<i32> v = Vec::new(); auto r = &v; auto w = v;",
        "diag.move-while-aliased",
    );
}

#[test]
fn conf_overwrite_live_resource_rejected() {
    expect_diag_body(
        "Vec<i32> v = Vec::new(); v = Vec::new();",
        "diag.overwrite-of-live-resource",
    );
}

// D-0033 (1): `[Assign-Reestablish]`.
#[test]
fn conf_reassign_after_drop_ok() {
    expect_true("{ Vec<i32> v = Vec::new(); drop(v); v = Vec::new(); Vec::push(&mut v, 1); Vec::len(&v) == 1 }");
}

#[test]
fn conf_reassign_after_move_ok() {
    expect_true("{ Vec<i32> v = Vec::new(); auto w = v; v = Vec::new(); Vec::push(&mut v, 1); Vec::len(&v) + Vec::len(&w) == 1 }");
}

#[test]
fn conf_reassign_from_own_move_ok() {
    expect_true_with(
        "fn grow(Vec<i32> v) : Vec<i32> { Vec<i32> w = v; Vec::push(&mut w, 7); w }",
        "{ Vec<i32> v = Vec::new(); v = grow(v); v = grow(v); Vec::len(&v) == 2 }",
    );
}

#[test]
fn conf_reassign_in_inner_block_ok() {
    expect_true("{ Vec<i32> v = Vec::new(); { auto w = v; v = Vec::new(); } Vec::push(&mut v, 3); Vec::len(&v) == 1 }");
}

#[test]
fn conf_reassign_maybe_live_dynamic() {
    expect_diag_dynamic_only(
        "fn main() { bool c = true; Vec<i32> v = Vec::new(); if (!c) { drop(v); } v = Vec::new(); }",
        "diag.overwrite-of-live-resource",
    );
}

// D-0033 (2), (4): literals.
#[test]
fn conf_float_exponent_literal() {
    expect_true("{ f64 x = 1.5e-3; x == 0.0015 }");
}

#[test]
fn conf_float_literal_out_of_range() {
    expect_diag_static("fn main() { f64 x = 1e400; }", "diag.literal-out-of-range");
}

#[test]
fn conf_float_literal_f32_out_of_range() {
    expect_diag_static("fn main() { f32 y = 1e39; }", "diag.literal-out-of-range");
}

#[test]
fn conf_float_literal_f32_rounded() {
    expect_true("{ f32 y = 16777217.0; y == 16777216.0: f32 }");
}

#[test]
fn conf_byte_char_literal() {
    expect_true("{ u8 x = b','; x == 44 }");
}

#[test]
fn conf_byte_char_escape() {
    expect_true("b'\\n' + b'\\x01' == 11");
}

#[test]
fn conf_byte_char_is_u8() {
    expect_diag_static("fn main() { i32 x = b'a'; }", "diag.type-mismatch");
}

// D-0033 (6): `unwrap_or`.
#[test]
fn conf_option_unwrap_or() {
    expect_true("Option::unwrap_or(None, 7) + Option::unwrap_or(Some(5), 0) == 12");
}

#[test]
fn conf_result_unwrap_or_resource() {
    expect_true("{ Result<Vec<i32>, i32> r = Err(3); Vec<i32> v = Result::unwrap_or(r, Vec::new()); Vec::len(&v) == 0 }");
}

#[test]
fn conf_drop_then_shadow_ok() {
    expect_true("{ Vec<i32> v = Vec::new(); drop(v); Vec<i32> v = Vec::new(); Vec::len(&v) == 0 }");
}

#[test]
fn conf_move_out_of_field_rejected() {
    expect_diag_with(
        "struct P { Vec<i32> a; }",
        "auto p = P { .a = Vec::new() }; auto q = p.a;",
        "diag.move-out-of-field",
    );
}

#[test]
fn conf_while_body_frame_per_iteration() {
    expect_ok_body("i32 i = 0; while (i < 3) { Vec<i32> v = Vec::new(); i = i + 1; }");
}

#[test]
fn conf_break_destroys_body_locals() {
    expect_ok_body(
        "i32 i = 0; while (true) { Vec<i32> v = Vec::new(); break; } if (i != 0) { 1/(1-1); }",
    );
}

// ---------- 4. Aliasing ----------

#[test]
fn conf_two_shared_borrows_ok() {
    expect_true("{ i32 x = 1; auto r1 = &x; auto r2 = &x; *r1 + *r2 == 2 }");
}

#[test]
fn conf_owner_read_while_shared_ok() {
    expect_true("{ i32 x = 1; auto r = &x; x + *r == 2 }");
}

#[test]
fn conf_owner_write_while_shared_rejected() {
    expect_diag_body("i32 x = 1; auto r = &x; x = 2;", "diag.aliasing-conflict");
}

#[test]
fn conf_owner_read_while_exclusive_rejected() {
    expect_diag_body("i32 x = 1; auto r = &mut x; x", "diag.aliasing-conflict");
}

#[test]
fn conf_shared_then_exclusive_rejected() {
    expect_diag_body("i32 x = 1; auto r1 = &x; auto r2 = &mut x;", "diag.aliasing-conflict");
}

#[test]
fn conf_sequential_borrows_ok() {
    expect_true("{ i32 x = 1; { auto r1 = &mut x; *r1 = 2; } auto r2 = &x; *r2 == 2 }");
}

#[test]
fn conf_reborrow_ok() {
    expect_true("{ i32 x = 1; auto r1 = &mut x; { auto r2 = &*r1; *r2; } *r1 = 3; *r1 == 3 }");
}

// ---------- Mode-monotonicity (spec/08 [Borrow-Exceeds-Source]/
// [Write-Not-Exclusive]) -- a real soundness hole the file-based
// conformance suite (impl/conformance/) found: neither diagnostic was
// ever constructed anywhere, so a function receiving only a `ref<T,
// shared>` parameter could freely mutate the caller's data through it.
// Three distinct ways to trigger it, all fixed together since they
// share one root cause (see impl/STATUS.md).

#[test]
fn conf_borrow_exceeds_source_via_reborrow_rejected() {
    // `&mut *r` where `r` is itself `ref<i32, shared>` -- reborrowing
    // exclusive from a shared source.
    expect_diag(
        "fn tamper(ref<i32, shared> r) : i32 { auto rm = &mut *r; *rm = 99; *rm }\nfn main() { i32 x = 5; tamper(&x); if (x != 99) { 1/(1-1); } }",
        "diag.borrow-exceeds-source",
    );
}

#[test]
fn conf_write_through_shared_direct_deref_rejected() {
    // `*r = v` directly, no intermediate reborrow.
    expect_diag(
        "fn tamper(ref<i32, shared> r) : i32 { *r = 99; *r }\nfn main() { i32 x = 5; tamper(&x); if (x != 99) { 1/(1-1); } }",
        "diag.write-through-shared",
    );
}

#[test]
fn conf_write_through_shared_auto_deref_field_rejected() {
    // `r.field = v` -- auto-deref field write, no explicit `*` at all.
    expect_diag_with(
        "struct Pt { i32 x; }\nfn tamper(ref<Pt, shared> r) : i32 { r.x = 99; r.x }",
        "Pt p = Pt { .x = 5 }; tamper(&p); if (p.x != 99) { 1/(1-1); }",
        "diag.write-through-shared",
    );
}

#[test]
fn conf_mode_monotonicity_does_not_falsely_reject_legitimate_reborrow_and_realloc_patterns() {
    // Regression guard for the fix's own first (buggy) attempt: a
    // persistent table naively keyed by (obj, path) wrongly cached
    // "shared" from an *earlier*, already-ended access and leaked it
    // into a *later*, unrelated exclusive one to the same storage --
    // exactly the shape `Vec::index_shared` (shared) followed by
    // `Vec::pop`/`Vec::push` (exclusive) on the same Vec produces
    // internally. Also covers alternating shared/exclusive access to
    // the same place across separate calls, and a reborrow chain
    // (borrow exclusive, reborrow shared, read, then keep writing
    // through the original exclusive reference).
    expect_ok(
        r#"
        struct Pt { i32 x; i32 y; }
        fn read_x(ref<Pt, shared> p) : i32 { p.x }
        fn write_x(ref<Pt, exclusive> p) : i32 { p.x = p.x + 1; p.x }
        fn main()
        {
            Pt pt = Pt { .x = 1, .y = 2 };
            if (read_x(&pt) != 1) { 1/(1-1); }
            if (write_x(&mut pt) != 2) { 1/(1-1); }
            if (read_x(&pt) != 2) { 1/(1-1); }
            if (write_x(&mut pt) != 3) { 1/(1-1); }
        }
        "#,
    );
}

// ---------- D-0006 ("no implicit conversion between distinct nominal
// types") not enforced for call arguments -- another real gap the
// file-based conformance suite found: an i32 binding passed where i64
// was declared silently "worked," the value round-tripping as if
// implicitly widened.

// Static-only, matching `diag.type-mismatch`'s own registered Phase
// (`rule.type.typing` is a static typing rule; the dynamic evaluator
// does not separately re-check a bound argument's value against its
// parameter's declared type) — `expect_diag_static`, not `expect_diag`
// (which drives `run_source`, the dynamic-only path this fix does not
// touch).

#[test]
fn conf_call_argument_width_mismatch_rejected() {
    expect_diag_static(
        "fn takes_i64(i64 x) : i64 { x }\nfn main() { i32 y = 5; takes_i64(y); }",
        "diag.type-mismatch",
    );
}

#[test]
fn conf_call_argument_exact_width_match_ok() {
    expect_true_with(
        "fn takes_i64(i64 x) : i64 { x }",
        "{ i64 y = 5; takes_i64(y) == 5 }",
    );
}

#[test]
fn conf_call_argument_bare_literal_still_widens_to_param_type_ok() {
    // A bare literal is not "already typed" the way a binding is --
    // rule.arith.literal lets it take on whatever concrete type
    // context requires; this must keep working, including through a
    // *generic* function's own type-parameter inference (the literal's
    // pre-inference default type must not be compared against the
    // post-inference concrete type as a false mismatch -- found via
    // Vec::push(&mut vec_of_u8, 226) during this same fix).
    expect_true_with("fn takes_i64(i64 x) : i64 { x }", "takes_i64(5) == 5");
    expect_ok(
        "fn main() { Vec<u8> v = Vec::new(); Vec::push(&mut v, 226); if (Vec::len(&v) != 1) { 1/(1-1); } }",
    );
}

#[test]
fn conf_call_argument_concrete_param_mismatch_rejected_even_in_generic_call() {
    // A concrete (non-generic-parameter) parameter of an otherwise-
    // generic function must still be exactly checked, even while the
    // function's own type parameter remains unresolved elsewhere in
    // the same call.
    expect_diag_static(
        "fn takes<T>(T item, i64 tag) : T { item }\nfn main() { i32 bad_tag = 5; takes(1, bad_tag); }",
        "diag.type-mismatch",
    );
}

#[test]
fn conf_disjoint_field_borrows_ok() {
    expect_true_with(
        "struct P { i32 a; i32 b; }",
        "{ auto p = P { .a = 1, .b = 2 }; auto r1 = &mut p.a; auto r2 = &mut p.b; *r1 + *r2 == 3 }",
    );
}

#[test]
fn conf_sequential_exclusive_borrows_ok() {
    expect_true(
        "{ Vec<i32> nums = Vec::new(); Vec::push(&mut nums, 10); Vec::push(&mut nums, 20); Vec::len(&nums) == 2 }",
    );
}

#[test]
fn conf_reference_in_field_survives_statement() {
    expect_true_with(
        "struct H { ref<i32, shared> r; }",
        "{ i32 x = 1; auto h = H { .r = &x }; *h.r == 1 }",
    );
}

// ---------- 6. Aggregates ----------

#[test]
fn conf_index_in_bounds() {
    expect_true("{ array<i32, 3> a = [10, 20, 30]; a[2] == 30 }");
}

#[test]
fn conf_index_out_of_bounds_dynamic() {
    expect_diag(
        "fn at(usize i) : i32 { array<i32, 3> a = [10, 20, 30]; a[i] }\nfn main() { at(3); }",
        "diag.index-out-of-bounds",
    );
}

#[test]
fn conf_struct_copy() {
    expect_true_with(
        "struct P { i32 a; i32 b; }",
        "{ auto p = P { .a = 1, .b = 2 }; auto q = p; q.a + p.b == 3 }",
    );
}

#[test]
fn conf_struct_field_resource_transfer() {
    expect_diag_with(
        "struct P { Vec<i32> a; }",
        "Vec<i32> v = Vec::new(); auto p = P { .a = v }; Vec::len(&v);",
        "diag.stale-binding",
    );
}

#[test]
fn conf_composite_destroy_recurses() {
    expect_ok_with("struct P { Vec<i32> a; }", "{ auto p = P { .a = Vec::new() }; }");
}

#[test]
fn conf_match_exhaustive_ok() {
    expect_ok(
        "enum Sign { Pos, Neg, Zero, }\nfn d(Sign s) : i32 { match (s) { Pos : 1, Neg : -1, Zero : 0, } }\nfn main() { if (d(Neg) != -1) { 1/(1-1); } }",
    );
}

#[test]
fn conf_match_wildcard_ok() {
    // Arms rewritten as `void`-typed blocks (not bare `1`/`0`): with the
    // original `i32`-valued arms, this `match` -- as `main`'s tail
    // expression, with no way to write a trailing `;` after a block-like
    // statement in this grammar -- would be a genuine `diag.type-
    // mismatch` against `main`'s declared `void` return (spec/12: "body
    // type equal to the declared return type"), a real fixture bug this
    // pass's new static checker surfaced (the dynamic-only evaluator
    // never checked a function body's type against its declared return
    // type at all). The `1/(1-1)` sentinel still confirms the wildcard
    // arm, not `Pos`'s, is the one that actually runs for `Neg`.
    expect_ok(
        "enum Sign { Pos, Neg, Zero, }\nfn main() { auto s = Neg; match (s) { Pos : { 1/(1-1); }, _ : {}, } }",
    );
}

#[test]
fn conf_match_resource_payload_transfers() {
    expect_diag_with(
        "enum E { Has(Vec<i32>), Empty, }",
        "auto e = E::Has(Vec::new()); match (e) { Has(x) : Vec::len(&x), Empty : 0, } drop(e);",
        "diag.stale-binding",
    );
}

#[test]
fn conf_let_destructure() {
    expect_true_with(
        "struct W { Vec<i32> inner; }",
        "{ auto w = W { .inner = Vec::new() }; W { inner } = w; Vec::len(&inner) == 0 }",
    );
}

// ---------- 7. Trust boundaries ----------

#[test]
fn conf_rawptr_deref_outside_unsafe_rejected() {
    // `[Unsafe-Rejected]` is `disposition: rejected` -- a whole-program
    // static fact (the offending deref need not even run), not merely a
    // dynamic fault.
    expect_diag_static(
        "fn f(rawptr<i32> p) : i32 { *p }\nfn main() { i32 x = 7; f(rawptr_of(&x)); }",
        "diag.trusted-outside-unsafe",
    );
}

#[test]
fn conf_rawptr_deref_inside_unsafe_ok() {
    expect_ok(
        "fn f(rawptr<i32> p) : i32 { unsafe { *p } }\nfn main() { i32 x = 7; if (f(rawptr_of(&x)) != 7) { 1/(1-1); } }",
    );
}

#[test]
fn conf_extern_write_observed() {
    expect_ok(
        "fn send(rawptr<u8> p, usize n) : isize { unsafe { write(p, n) } }\nfn main() { auto message = [104:u8, 101:u8, 108:u8, 108:u8, 111:u8, 10:u8]; auto p = reinterpret_ptr<u8>(rawptr_of(&message)); send(p, 6); }",
    );
}

// `expect_ok` above only asserts `write` doesn't fault, not that it
// actually wrote the right bytes -- exactly the gap that let a real bug
// (encode/decode's Type::Array arm was silently missing, so any array
// value read through a rawptr/extern boundary was all zero bytes) ship
// undetected through four prior implementation passes and 118 "passing"
// tests. This spawns the real `coby` binary (CARGO_BIN_EXE_coby,
// the actual write(2)-backed path, not the library API) and asserts the
// real captured stdout bytes, closing that coverage gap permanently.
// Spawns the real built binary (CARGO_BIN_EXE_coby) and returns
// (exit_success, captured_stdout) -- used to assert *actual* byte
// content crossing the write()/rawptr boundary, not just "didn't
// fault", which is the coverage gap that let the missing Type::Array
// (and, in a later fix, Type::Named-enum) encode/decode arms ship
// undetected through five prior passes.
fn run_binary_stdout(src: &str) -> (bool, Vec<u8>) {
    use std::io::Write;
    let mut path = std::env::temp_dir();
    path.push(format!("coby_test_{}_{}.cb", std::process::id(), rand_suffix()));
    std::fs::File::create(&path).unwrap().write_all(with_std(src).as_bytes()).unwrap();
    let out = std::process::Command::new(env!("CARGO_BIN_EXE_coby"))
        .arg(&path)
        .output()
        .expect("failed to run coby binary");
    std::fs::remove_file(&path).ok();
    (out.status.success(), out.stdout)
}

// Tests run in parallel threads of one process, so the file name must be
// unique per call, not per process or per clock tick (Windows' clock is
// coarse enough that two concurrent calls saw the same nanosecond value
// and ran each other's program).
fn rand_suffix() -> u64 {
    use std::sync::atomic::{AtomicU64, Ordering};
    static NEXT: AtomicU64 = AtomicU64::new(0);
    NEXT.fetch_add(1, Ordering::Relaxed)
}

#[test]
fn e2e_extern_write_produces_real_bytes_on_stdout() {
    let src = "fn main() { auto message = [104:u8, 101:u8, 108:u8, 108:u8, 111:u8, 10:u8]; rawptr<u8> p = reinterpret_ptr<u8>(rawptr_of(&message)); usize len = 6; isize n = unsafe { write(p, len) }; }";
    let (ok, out) = run_binary_stdout(src);
    assert!(ok, "coby exited non-zero");
    assert_eq!(out, b"hello\n", "write() did not produce the expected real bytes");
}

// Regression test for the Type::Named-enum arm missing from encode/
// decode (found immediately after fixing the analogous Type::Array
// gap above). Per [Layout-Enum]/[Repr-Enum] (spec/06 §7, spec/16 §1)
// with this implementation's documented DW=4: Some(65:i32) on
// Option<i32> is discriminant 0 (variants numbered from 0 in
// declaration order: Some, None) at offset 0..4, payload 65 at
// offset 4..8 (payload-off = round_up(4, alignof(i32)=4) = 4) --
// 8 bytes total, little-endian.
#[test]
fn e2e_enum_through_rawptr_has_correct_discriminant_and_payload_bytes() {
    let src = "fn main() { Option<i32> o = Some(65); rawptr<u8> p = reinterpret_ptr<u8>(rawptr_of(&o)); usize len = sizeof<Option<i32>>(); isize n = unsafe { write(p, len) }; }";
    let (ok, out) = run_binary_stdout(src);
    assert!(ok, "coby exited non-zero");
    assert_eq!(out, vec![0, 0, 0, 0, 65, 0, 0, 0], "Some(65): i32 payload encoded wrong");
}

#[test]
fn e2e_enum_none_variant_has_correct_discriminant() {
    // None is variant index 1 (declared second); a payload-less variant
    // has no bytes of its own to check, only that the discriminant is
    // right and the call doesn't corrupt/panic.
    let src = "fn main() { Option<i32> o = None; rawptr<u8> p = reinterpret_ptr<u8>(rawptr_of(&o)); usize len = sizeof<Option<i32>>(); isize n = unsafe { write(p, len) }; }";
    let (ok, out) = run_binary_stdout(src);
    assert!(ok, "coby exited non-zero");
    assert_eq!(&out[0..4], &[1, 0, 0, 0], "None's discriminant encoded wrong");
}

#[test]
fn e2e_struct_field_of_enum_type_layout_is_correct() {
    // A struct field whose type is an enum -- exercises struct_layout's
    // recursive sizeof_ty/alignof_ty calls against enum_layout, not
    // just a bare enum value. Foo { Option<i32> a; i32 b; }: a at
    // offset 0 (size 8), b at offset 8 (size 4) -- 12 bytes total.
    let src = "struct Foo { Option<i32> a; i32 b; }\nfn main() { Foo f = Foo { .a = Some(7), .b = 99 }; rawptr<u8> p = reinterpret_ptr<u8>(rawptr_of(&f)); usize len = sizeof<Foo>(); isize n = unsafe { write(p, len) }; }";
    let (ok, out) = run_binary_stdout(src);
    assert!(ok, "coby exited non-zero");
    assert_eq!(
        out,
        vec![0, 0, 0, 0, 7, 0, 0, 0, 99, 0, 0, 0],
        "struct field of enum type laid out/encoded wrong"
    );
}

// ---------- 8. Failure semantics ----------

#[test]
fn conf_checked_fault_unwinds_and_terminates() {
    expect_diag(
        "fn d(i32 a, i32 b) : i32 { a / b }\nfn main() { { Vec<i32> v = Vec::new(); d(1, 0); } }",
        "diag.div-by-zero",
    );
}

#[test]
fn conf_propagate_ok() {
    expect_ok(
        "fn f() : Result<i32, i32> { Result<i32, i32> r = Ok(1); auto v = r?; Ok(v + 1) }\nfn main() { match (f()) { Ok(v) : { if (v != 2) { 1/(1-1); } }, Err(_) : { 1/(1-1); }, } }",
    );
}

#[test]
fn conf_propagate_err() {
    expect_ok(
        "fn f() : Result<i32, i32> { Result<i32, i32> r = Err(5); auto v = r?; Ok(v) }\nfn main() { match (f()) { Ok(_) : { 1/(1-1); }, Err(e) : { if (e != 5) { 1/(1-1); } }, } }",
    );
}

// ---------- 9. Generics and inference ----------

#[test]
fn conf_generic_fn_explicit() {
    expect_true_with("fn id<T>(T x) : T { x }", "id<i32>(5) == 5");
}

#[test]
fn conf_generic_call_inferred() {
    expect_ok("fn id<T>(T x) : T { x }\nfn main() { if (id(5) != 5) { 1/(1-1); } }");
}

#[test]
fn conf_generic_call_expected_type_inferred() {
    expect_ok_body("Vec<i32> nums = Vec::new(); if (Vec::len(&nums) != 0) { 1/(1-1); }");
}

#[test]
fn conf_generic_struct_monomorphize_resource() {
    expect_ok_with(
        "struct Box<T> { T value; }",
        "Box<Vec<i32>> c = Box { .value = Vec::new() };",
    );
}

#[test]
fn conf_literal_default_i32() {
    expect_ok_body("auto x = 5; i32 y = x;");
}

#[test]
fn conf_literal_context_u8() {
    expect_ok_body("u8 x = 200;");
}

// ---------- 10. Closures ----------

#[test]
fn conf_closure_borrow_capture() {
    expect_true("{ i32 x = 10; auto f = [x](i32 y) { x + y }; f(5) == 15 }");
}

#[test]
fn conf_closure_move_capture_invalidates_source() {
    expect_diag_body(
        "Vec<i32> v = Vec::new(); auto f = move [v]() { Vec::push(&mut v, 1); }; f(); Vec::push(&mut v, 2);",
        "diag.stale-binding",
    );
}

#[test]
fn conf_closure_owns_moved_resource() {
    expect_ok_body("{ Vec<i32> v = Vec::new(); auto f = move [v]() { Vec::len(&v); }; }");
}

// ---------- 11. Library ----------

#[test]
fn conf_vec_push_len() {
    expect_true(
        "{ Vec<i32> v = Vec::new(); Vec::push(&mut v, 1); Vec::push(&mut v, 2); Vec::len(&v) == 2 }",
    );
}

#[test]
fn conf_vec_index_shared_ok() {
    expect_true(
        "{ Vec<i32> v = Vec::new(); Vec::push(&mut v, 10); *Vec::index_shared(&v, 0) == 10 }",
    );
}

#[test]
fn conf_vec_index_out_of_bounds() {
    expect_diag_body(
        "Vec<i32> v = Vec::new(); Vec::push(&mut v, 10); *Vec::index_shared(&v, 1);",
        "diag.index-out-of-bounds",
    );
}

#[test]
fn conf_vec_pop_resource() {
    expect_true(
        "{ Vec<Vec<i32>> v = Vec::new(); Vec::push(&mut v, Vec::new()); match (Vec::pop(&mut v)) { Some(inner) : Vec::len(&inner) == 0, None : false, } }",
    );
}

#[test]
fn conf_vec_drop_destroys_elements() {
    expect_ok_body("{ Vec<Vec<i32>> v = Vec::new(); Vec::push(&mut v, Vec::new()); }");
}

// `conf.vec-ref-then-push-rejected`: this pass adds a narrow, targeted
// static check (`src/typecheck.rs`'s `deriv`/elision alias tracking)
// specifically for this shape -- `Vec::index_shared` has exactly one
// reference parameter and returns a reference, so a call to it derives
// the elision fact the spec's own `rule.temporal.elision` records
// (`deriv(r, v.ε, shared)`); the second `Vec::push(&mut v, 2)` then
// conflicts with that live fact, exactly as `rule.temporal.ref-escape`
// requires -- rejected *before* the program ever runs, matching the
// spec's own documented reasoning: this is precisely the one shape
// with no dynamic backstop at all (`Vec::index_shared`'s returned
// reference targets the reclaimed *element* object, never `v` itself,
// so the dynamic `clash` scan structurally cannot see the conflict).
#[test]
fn conf_vec_ref_then_push_rejected() {
    let src = "fn main()\n{\n    Vec<i32> v = Vec::new(); Vec::push(&mut v, 1); auto r = Vec::index_shared(&v, 0); Vec::push(&mut v, 2); *r;\n}\n";
    match check_source_static(src) {
        Err(d) => assert_eq!(d, "diag.aliasing-conflict"),
        Ok(()) => panic!("expected a static rejection for:\n{}", src),
    }
}

#[test]
fn conf_string_from_utf8_ok() {
    expect_ok_body(
        "Vec<u8> b = Vec::new(); Vec::push(&mut b, 104); Vec::push(&mut b, 105); match (String::from_utf8(b)) { Ok(s) : { if (String::len(&s) != 2) { 1/(1-1); } }, Err(_) : { 1/(1-1); }, }",
    );
}

#[test]
fn conf_string_from_utf8_err() {
    expect_ok_body(
        "Vec<u8> b = Vec::new(); Vec::push(&mut b, 255); match (String::from_utf8(b)) { Ok(_) : { 1/(1-1); }, Err(e) : { if (e.offset != 0) { 1/(1-1); } }, }",
    );
}

#[test]
fn conf_string_into_bytes_roundtrip() {
    expect_ok_body(
        "Vec<u8> b = Vec::new(); Vec::push(&mut b, 104); Vec::push(&mut b, 105); auto s = match (String::from_utf8(b)) { Ok(v) : v, Err(_) : String::from_str(\"\"), }; usize n1 = String::len(&s); auto bytes2 = String::into_bytes(s); usize n2 = Vec::len(&bytes2); if (n1 != 2) { 1/(1-1); } if (n2 != 2) { 1/(1-1); }",
    );
}

#[test]
fn conf_rc_clone_shares_allocation() {
    expect_ok_body("auto a = Rc::new(1); auto b = Rc::clone(&a);");
}

#[test]
fn conf_map_err_transforms() {
    expect_ok(
        "fn scale(i32 e) : i32 { e * 10 }\nfn main() { Result<i32, i32> r = Err(4); match (map_err(r, scale)) { Ok(v) : { 1/(1-1); }, Err(e) : { if (e != 40) { 1/(1-1); } }, } }",
    );
}

#[test]
fn conf_rc_last_drop_frees() {
    expect_ok_body("auto a = Rc::new(1); auto b = Rc::clone(&a); drop(a); drop(b);");
}

// ---------- 12. Concurrency ----------
// `spawn`/`join`/`lock` now run on real `std::thread`s sharing one
// `Interp` behind a real lock (impl/STATUS.md) -- genuine OS scheduling,
// not a simulation.

#[test]
fn conf_spawn_join_value() {
    expect_ok(
        "fn work(i32 n) : i32 { n * 2 }\nfn main() { auto h = spawn(work, 21); if (join(h) != 42) { 1/(1-1); } }",
    );
}

// `conf.cross-thread-write-conflict`: `outcome: unspecified { ok,
// diag.aliasing-conflict }` (spec/conformance.md §12) -- the spec itself
// says either is a legitimate outcome, decided only by which of the two
// threads' racing accesses happens to complete first. Genuinely racy now
// that spawn is real: this asserts "one of the two documented legitimate
// outcomes happened," not a single hardcoded expectation, and runs it
// several times to actually exercise real OS scheduling variance rather
// than asserting on a single, possibly-lucky run.
#[test]
fn conf_cross_thread_write_conflict() {
    let src = "fn w(ref<i32, shared> r) : i32 { *r }\nfn main() { i32 x = 1; auto h = spawn(w, &x); x = 2; join(h); }";
    for _ in 0..20 {
        match run_source(src) {
            Ok(()) => {}
            Err(d) if d == "diag.aliasing-conflict" => {}
            Err(d) => panic!("expected ok or diag.aliasing-conflict (spec: outcome: unspecified), got `{}`", d),
        }
    }
}

#[test]
fn conf_unjoined_handle_waits() {
    expect_ok("fn work(i32 n) : i32 { n * 2 }\nfn main() { { auto h = spawn(work, 1); } }");
}

#[test]
fn conf_mutex_lock_unlock() {
    expect_ok_body(
        "auto m = Mutex::new(0); { auto g = lock(&m); *g = *g + 1; } if (*lock(&m) != 1) { 1/(1-1); }",
    );
}

#[test]
fn conf_mutex_reentrant_rejected() {
    expect_diag_body(
        "auto m = Mutex::new(0); auto g = lock(&m); auto g2 = lock(&m);",
        "diag.mutex-reentrant-lock",
    );
}

// ---------- 13. End-to-end programs (spec/examples.md ex.e2e-*) ----------
// The first three below predate this pass and were reconstructed from
// memory rather than re-read verbatim (left as-is; still real, passing
// tests of the same shapes). Everything from `ex_e2e_threads_mutex`
// onward is a verbatim transcription of `spec/examples.md`'s own source
// for the previously-missing `ex.e2e-*` cases (conformance.md §13),
// closing the gap impl/STATUS.md documented ("not attempted at all, for
// lack of remaining time").

#[test]
fn ex_e2e_vec_nested_realloc_shape() {
    // covers: conf.e2e-vec-nested-realloc
    expect_ok_body(
        r#"
        Vec<Vec<i32>> outer = Vec::new();
        i32 k = 0;
        while (k < 5)
        {
            Vec<i32> inner = Vec::new();
            Vec::push(&mut inner, k);
            Vec::push(&mut outer, inner);
            k = k + 1;
        }
        {
            auto first = Vec::index_shared(&outer, 0);
            usize n = Vec::len(first);
            if (n != 1) { 1/(1-1); }
        }
        match (Vec::pop(&mut outer))
        {
            Some(v) : { Vec::push(&mut outer, v); },
            None : {},
        }
        usize total = Vec::len(&outer);
        if (total != 5) { 1/(1-1); }
        "#,
    );
}

#[test]
fn ex_e2e_rc_resource_payload_shape() {
    // covers: conf.e2e-rc-resource-payload
    expect_ok_body(
        r#"
        Vec<i32> payload = Vec::new();
        Vec::push(&mut payload, 42);
        auto a = Rc::new(payload);
        auto b = Rc::clone(&a);
        usize n = Vec::len(Rc::get(&b));
        if (n != 1) { 1/(1-1); }
        drop(a);
        usize m = Vec::len(Rc::get(&b));
        if (m != 1) { 1/(1-1); }
        drop(b);
        "#,
    );
}

#[test]
fn ex_e2e_propagate_chain_shape() {
    // covers: conf.e2e-propagate-chain
    expect_ok(
        r#"
        fn parse(u8 b) : Result<i32, i32>
        {
            if (b > 9)
            {
                return Err(1);
            }
            Ok(widen<i32>(b))
        }
        fn sum(ref<Vec<u8>, shared> bytes) : Result<i32, i32>
        {
            i32 total = 0;
            usize i = 0;
            usize n = Vec::len(bytes);
            while (i < n)
            {
                u8 b = *Vec::index_shared(bytes, i);
                i32 v = parse(b)?;
                total = total + v;
                i = i + 1;
            }
            Ok(total)
        }
        fn main()
        {
            Vec<u8> digits = Vec::new();
            Vec::push(&mut digits, 1);
            Vec::push(&mut digits, 2);
            Vec::push(&mut digits, 3);
            match (sum(&digits))
            {
                Ok(v) : { if (v != 6) { 1/(1-1); } },
                Err(_) : { 1/(1-1); },
            }
            Vec::push(&mut digits, 99);
            match (sum(&digits))
            {
                Ok(_) : { 1/(1-1); },
                Err(e) : { if (e != 1) { 1/(1-1); } },
            }
        }
        "#,
    );
}

#[test]
fn ex_e2e_threads_mutex() {
    // → `conf.e2e-threads-mutex-total`. Concurrency is simulated
    // synchronously (impl/STATUS.md), so this exercises the functional
    // result (`a + b == 7`), not the real cross-thread interleaving the
    // spec's own text discusses.
    expect_ok(
        r#"
        fn worker(ref<mutex<i32>, shared> m, i32 n) : i32
        {
            i32 i = 0;
            while (i < n)
            {
                auto g = lock(m);
                *g = *g + 1;
                i = i + 1;
            }
            n
        }
        fn main()
        {
            auto m = Mutex::new(0);
            auto h1 = spawn(worker, &m, 3);
            auto h2 = spawn(worker, &m, 4);
            i32 a = join(h1);
            i32 b = join(h2);
            i32 total = *lock(&m);
            if (total != 7) { 1/(1-1); }
        }
        "#,
    );
}

#[test]
    // Previously failed for the reason recorded in impl/STATUS.md's prior
    // "known bugs" section: the dynamic `clash` scan's "ancestor"
    // exemption (spec/08 `ancestors`) was tracked only transiently, for
    // the single deref/field-access operation that formed a new
    // reference, not persistently for the rest of that reference's
    // lifetime -- so `&*v`'s sub-borrow forgot it descended from `v`'s
    // own still-live borrow. Fixed by `token_ancestors` (interp.rs): each
    // newly-minted token's ancestor set is recorded once, at formation,
    // and consulted by every later `clash`/`solitary` check made through
    // it, however many frames removed.
fn ex_e2e_vec_realloc_stale_ref() {
    // → `conf.e2e-vec-realloc-stale-ref`. Unlike `conf.vec-ref-then-
    // push-rejected` (same-function, caught statically by this pass's
    // `deriv` check), the element reference here is held across a
    // reallocating `push` reached through a reference *parameter* --
    // invisible to `rule.control.flow-analysis`/`rule.temporal.ref-
    // escape` (their facts are defined only over a plain binding's own
    // projections), so this is a genuinely dynamic-only rejection; no
    // static assertion is expected or attempted here.
    expect_diag(
        r#"
        fn hold_across_grow(ref<Vec<i32>, exclusive> v) : i32
        {
            auto r = Vec::index_shared(&*v, 0);
            Vec::push(v, 20);
            Vec::push(v, 30);
            Vec::push(v, 40);
            Vec::push(v, 50);
            *r
        }
        fn main()
        {
            Vec<i32> v = Vec::new();
            Vec::push(&mut v, 10);
            hold_across_grow(&mut v);
        }
        "#,
        "diag.stale-binding",
    );
}

#[test]
    // Previously failed for the same reason as `ex_e2e_vec_realloc_stale_ref`
    // above (the `clash` scan's ancestor exemption was transient, not
    // persistent) -- fixed by `token_ancestors` (interp.rs).
fn ex_e2e_mutex_vec_resource_interior() {
    // → `conf.e2e-mutex-vec-resource-interior`.
    expect_ok_body(
        r#"
        Vec<i32> inner = Vec::new();
        auto m = Mutex::new(inner);
        {
            auto g = lock(&m);
            Vec::push(&mut *g, 1);
            Vec::push(&mut *g, 2);
            usize n = Vec::len(&*g);
            if (n != 2) { 1/(1-1); }
        }
        "#,
    );
}

#[test]
fn ex_e2e_closure_move_rc() {
    // → `conf.e2e-closure-move-rc`.
    expect_ok_body(
        r#"
        Vec<i32> payload = Vec::new();
        Vec::push(&mut payload, 7);
        auto a = Rc::new(payload);
        auto f = move [a]() { Vec::len(Rc::get(&a)) };
        usize x = f();
        usize y = f();
        if (x != 1) { 1/(1-1); }
        if (y != 1) { 1/(1-1); }
        drop(f);
        "#,
    );
}

#[test]
fn ex_e2e_thread_fault_abandons_guard() {
    // → `conf.e2e-thread-fault-abandons-guard`. `worker` always faults;
    // `main`'s guard `g` and everything after `spawn` is abandoned
    // mid-flight, exactly as spec/18 §1 states.
    expect_diag(
        r#"
        fn worker(i32 n) : i32
        {
            n / 0
        }
        fn main()
        {
            auto m = Mutex::new(0);
            auto g = lock(&m);
            auto h = spawn(worker, 5);
            *g = *g + 1;
            join(h);
        }
        "#,
        "diag.div-by-zero",
    );
}

#[test]
fn ex_e2e_spawn_join_resource_result() {
    // → `conf.spawn-join-resource-result` (`CHG-0015`'s authority
    // re-keying: a resource-typed thread result's destroy authority
    // must transfer from the worker thread to the joiner, or to
    // whatever sweeps an unjoined handle's discarded result).
    expect_ok(
        r#"
        fn make() : Vec<i32>
        {
            Vec<i32> v = Vec::new();
            Vec::push(&mut v, 5);
            v
        }
        fn main()
        {
            auto h1 = spawn(make);
            auto v1 = join(h1);
            usize n = Vec::len(&v1);
            if (n != 1) { 1/(1-1); }
            drop(v1);
            { auto h2 = spawn(make); }
        }
        "#,
    );
}

#[test]
    // Previously failed for the same reason as `ex_e2e_vec_realloc_stale_ref`
    // above (the `clash` scan's ancestor exemption was transient, not
    // persistent) -- fixed by `token_ancestors` (interp.rs).
fn ex_e2e_vec_pop_push_reuse_stale_ref() {
    // → `conf.e2e-vec-pop-push-reuse-stale-ref` (closes finding F-05,
    // `CHG-0019`: `Vec::pop` releases its slot for every element type,
    // so the reused slot's hazard is caught dynamically, never silent).
    expect_diag(
        r#"
        fn hazard(ref<Vec<i32>, exclusive> v) : i32
        {
            auto r = Vec::index_shared(&*v, 2);
            Vec::pop(v);
            Vec::push(v, 99);
            *r
        }
        fn main()
        {
            Vec<i32> v = Vec::new();
            Vec::push(&mut v, 10);
            Vec::push(&mut v, 20);
            Vec::push(&mut v, 30);
            hazard(&mut v);
        }
        "#,
        "diag.stale-binding",
    );
}

#[test]
fn ex_e2e_hello_print() {
    // → `conf.e2e-hello-print` (`CHG-0025`, D-0020): text in, text out,
    // no `unsafe` in the program; `who : str` is used three times and is
    // never moved. Byte-exact stdout is asserted by the `.cb` suite
    // (`21-standard-library-semantics/print_writes_bytes_ok.cb`); this
    // checks termination and the static pass.
    expect_ok(
        r#"
        fn main()
        {
            str who = "CobaltC";
            String owned = String::from_str(who);
            if (String::len(&owned) != str_len(who))
            {
                1 / (1 - 1);
            }
            printf("Hello, ");
            printf("%v", who);
            printf("!\n");
        }
        "#,
    );
}

// ---------- Sanity-pass regressions on encode/decode's remaining
// Type/Value combinations, found by systematically auditing every
// `Type` variant against encode/decode after the Array and Enum
// fixes above (impl/STATUS.md's "sanity pass" entry has the full
// audit). Each of these was empirically confirmed broken before the
// fix it now guards.

#[test]
fn e2e_sizeof_array_of_struct_with_enum_field_is_correct() {
    // Root-cause bug: sizeof_ty/alignof_ty's Type::Array arm delegated
    // to the free `sizeof()`/`alignof()` functions (value.rs), which
    // cannot see user-declared struct/enum layouts -- so
    // sizeof<array<Foo,2>>() silently returned 0 even though encode/
    // decode's own array arm (which calls self.sizeof_ty correctly)
    // was fine all along. Foo { Option<i32> a; i32 b; } is 12 bytes;
    // array<Foo,2> must be 24.
    let src = "struct Foo { Option<i32> a; i32 b; }\nfn main() { usize sz = sizeof<array<Foo,2>>(); if (sz != 24) { 1/(1-1); } }";
    expect_ok(src);
}

#[test]
fn e2e_array_of_struct_with_enum_field_encodes_correctly() {
    let src = "struct Foo { Option<i32> a; i32 b; }\nfn main() { Foo f1 = Foo { .a = Some(1), .b = 10 }; Foo f2 = Foo { .a = None, .b = 20 }; auto arr = [f1, f2]; usize sz = sizeof<array<Foo,2>>(); rawptr<u8> p = reinterpret_ptr<u8>(rawptr_of(&arr)); isize n = unsafe { write(p, sz) }; }";
    let (ok, out) = run_binary_stdout(src);
    assert!(ok, "coby exited non-zero");
    assert_eq!(
        out,
        vec![0, 0, 0, 0, 1, 0, 0, 0, 10, 0, 0, 0, 1, 0, 0, 0, 0, 0, 0, 0, 20, 0, 0, 0],
        "array<Foo,2> content wrong"
    );
}

#[test]
fn e2e_mutex_inner_value_encodes_at_offset_zero() {
    // Type::Mutex had no encode/decode arm at all -- [Repr-Mutex]
    // (spec/06 sec 4) requires the inner value at offset 0; the
    // interpreter represents mutex<T> as Value::Struct([inner]),
    // which needed its own arm distinct from the Type::Named struct
    // one (Type::Mutex is a separate Type variant).
    let src = "fn main() { mutex<i32> m = Mutex::new(42); rawptr<u8> p = reinterpret_ptr<u8>(rawptr_of(&m)); usize len = sizeof<mutex<i32>>(); isize n = unsafe { write(p, len) }; }";
    let (ok, out) = run_binary_stdout(src);
    assert!(ok, "coby exited non-zero");
    assert_eq!(&out[0..4], &[42, 0, 0, 0], "mutex<i32>'s inner value not at offset 0");
    // [Sizeof-Mutex]: `struct { i32 inner; usize state; }` per
    // [Layout-Struct] -- the state at offset 8, 16 bytes in all.
    assert_eq!(out.len(), 16, "sizeof(mutex<i32>) should be that of struct {{ i32 inner; usize state; }} = 16");
}

#[test]
fn e2e_fn_value_encodes_as_nonzero_deterministic_bytes() {
    // Type::Fn had no encode arm; a struct field holding a named `fn`
    // value silently encoded as all zeros, which would make every
    // distinct function compare equal if ever compared through a raw
    // byte view -- [Repr-Fn] (spec/06 sec 4) requires an injective
    // image. Not fully round-trippable (decode does not reconstruct a
    // callable FnVal from it -- see STATUS.md); this only checks the
    // encode side is real and deterministic, run twice to confirm it
    // isn't randomized per-process.
    let src = "fn double(i32 x) : i32 { x * 2 }\nstruct Holder { fn(i32):i32 cb; i32 tag; }\nfn main() { Holder h = Holder { .cb = double, .tag = 7 }; rawptr<u8> p = reinterpret_ptr<u8>(rawptr_of(&h)); usize len = sizeof<Holder>(); isize n = unsafe { write(p, len) }; }";
    let (ok1, out1) = run_binary_stdout(src);
    let (ok2, out2) = run_binary_stdout(src);
    assert!(ok1 && ok2, "coby exited non-zero");
    assert_ne!(&out1[0..8], &[0u8; 8], "fn value encoded as all zeros");
    assert_eq!(out1, out2, "fn value encoding is not deterministic across runs");
    assert_eq!(&out1[8..12], &[7, 0, 0, 0], "tag field after the fn field is wrong");
}

// ---------- Full silent-bug audit (2026-09-21), area 1 continued +
// arithmetic: i128/u128 host-representation bugs, and a broad
// literal-expected-type propagation gap in the dynamic evaluator.
// See impl/STATUS.md's own dated section for the full account.

#[test]
fn e2e_i128_arithmetic_does_not_panic_the_host_interpreter() {
    // int_min_max's i128 branch computed `(1i128 << 127) - 1` /
    // `-(1i128 << 127)` -- host i128 overflow (bit-pattern 1<<127 IS
    // i128::MIN) that happened to wrap to the mathematically-correct
    // answer in --release (overflow-checks off) but was a confirmed
    // real panic ("attempt to subtract with overflow") in a debug
    // build, meaning i128 was completely unusable outside --release.
    // This can only be verified against the debug binary, which isn't
    // built by `cargo test`, so this test instead exercises the exact
    // arithmetic ops that flowed through the buggy computation and
    // checks they're *correct*, not just non-panicking in release; the
    // debug-panic itself was confirmed by hand (`cargo build` +
    // `target/debug/coby`) before this fix, not re-derived here.
    expect_true_with(
        "fn add(i128 a, i128 b) : i128 { a + b }",
        "add(100, 1) == 101",
    );
    expect_true("170141183460469231731687303715884105727: i128 == 170141183460469231731687303715884105727");
}

#[test]
fn e2e_u128_upper_half_representable() {
    // u128's true range is [0, 2^128-1]. Value::Int's payload is still
    // i128, but for u128 it now holds the value's *bit pattern* and
    // every operation on it is done in u128 (value.rs `is_u128`), so
    // the upper half is representable; this test used to pin the old
    // limitation (a legal literal rejected as out of range).
    expect_true("{ u128 x = 170141183460469231731687303715884105728; x > 1 }");
    expect_true("{ u128 big = 340282366920938463463374607431768211455; u128 half = 170141183460469231731687303715884105728; big > half && half + (half - 1) == big && big / half == 1 && wrapping_add(big, 1: u128) == 0 && saturating_add(big, 5: u128) == big && narrow<u64>(half >> 64) == 9223372036854775808 }");
    expect_diag_body("u128 big = 340282366920938463463374607431768211455; u128 more = big + 1;", "diag.arith-overflow");
    expect_diag_body("i64 neg = -5; u128 u = narrow<u128>(neg);", "diag.narrowing-overflow");
}

#[test]
fn e2e_unsuffixed_wide_literal_in_let_gets_declared_type() {
    // `eval_inner`'s IntLit case unconditionally defaulted every
    // un-suffixed literal to i32, ignoring any expected type from
    // context -- `i64 y = 5000000000;` (a completely ordinary
    // declaration; 5000000000 does not fit in i32) was rejected
    // diag.literal-out-of-range even though it fits i64 fine.
    expect_true("{ i64 y = 5000000000; y == 5000000000 }");
}

#[test]
fn e2e_unsuffixed_wide_literal_in_assignment_gets_lhs_type() {
    expect_true("{ i64 y = 0; y = 5000000000; y == 5000000000 }");
}

#[test]
fn e2e_unsuffixed_wide_literal_as_call_argument_gets_param_type() {
    expect_true_with(
        "fn f(i64 x) : i64 { x }",
        "f(5000000000) == 5000000000",
    );
}

#[test]
fn e2e_unsuffixed_wide_literal_in_explicit_return_gets_fn_return_type() {
    // Two independent bugs compounded here, both fixed:
    // (1) the dynamic evaluator's `return` had no access to the
    //     enclosing function's declared return type at all;
    // (2) separately, the *static* checker's `ty_compat` was a bare
    //     `a == b` with no [T-Never] case, so even after (1) the
    //     static pass rejected this function's body (Never, from the
    //     bare `return` statement with no trailing expression) against
    //     its own declared `i64` return type as diag.type-mismatch.
    expect_true_with(
        "fn g() : i64 { return 5000000000; }",
        "g() == 5000000000",
    );
}

#[test]
fn e2e_never_typed_branch_compatible_with_any_expected_type_generally() {
    // The ty_compat fix is symmetric and used by several call sites
    // beyond the one that surfaced it (function-body-vs-return-type) --
    // this exercises the same [T-Never] rule via an if/else where one
    // arm is a bare `return` (Never) and the other is a real value, the
    // shape typecheck.rs's own pre-existing comment already claimed to
    // support ("an if/else if chain's final else arm").
    expect_true_with(
        "fn h(bool c) : i32 { if (c) { return 7; } else { } 3 }",
        "h(true) == 7 && h(false) == 3",
    );
}

#[test]
fn e2e_unsuffixed_wide_literal_as_binary_op_right_operand_gets_left_type() {
    // eval_binary evaluated both operands with no expected-type
    // awareness at all -- `i64 y = 5000000000; y == 5000000000`
    // (an entirely ordinary comparison) defaulted the literal to i32
    // and faulted, even though the left operand's real type (i64) was
    // already known by the time the right operand was evaluated
    // (left-to-right, D-0007). This is what actually caused the
    // "duplicate evaluation" appearance the two nested-block tests
    // below hit first -- traced with a temporary debug print before
    // being correctly diagnosed as two literals in one source line,
    // not one literal evaluated twice.
    expect_true("{ i64 y = 5000000000; y == 5000000000 }");
}

#[test]
fn e2e_wide_literal_as_binary_op_left_operand_takes_the_right_operands_type() {
    // `rule.type.expected`: "the other operand's determined type" --
    // in both directions. A bare literal on the *left* has no effects,
    // so the right operand is evaluated first and the literal then
    // takes its type (D-0007 is not observable here). Previously the
    // left literal defaulted to i32 and this faulted.
    expect_true("{ i64 y = 5000000000; 5000000000 == y }");
    expect_true("{ u8 d = 7; 48 + d == 55 }");
}

#[test]
fn e2e_nested_block_expression_literal_defaulting_also_fixed() {
    expect_ok("fn main() { bool r = { i64 y = 5000000000; y == 5000000000 }; if (!r) { 1/(1-1); } }");
}

#[test]
fn e2e_most_negative_literal_i32_min() {
    // `i32 x = -2147483648;` -- the positive magnitude 2147483648
    // (2^31) does not fit i32's positive range, but its negation is
    // exactly i32::MIN, a perfectly legal value. Was rejected
    // diag.literal-out-of-range (static) by both check_unary (no
    // expected-type threading, and no negated-value range check) and
    // the dynamic evaluator's eval_unary (evaluated the positive
    // magnitude with no expected type before negating).
    expect_true("{ i32 x = -2147483648; x == -2147483648 }");
}

#[test]
fn e2e_most_negative_literal_i64_min_and_i128_min() {
    expect_true("{ i64 y = -9223372036854775808; y == -9223372036854775808 }");
    expect_true(
        "{ i128 z = -170141183460469231731687303715884105728; z == -170141183460469231731687303715884105728 }",
    );
}

#[test]
fn e2e_most_negative_literal_as_binary_op_right_operand() {
    expect_true_with(
        "fn ng(i64 v) : i64 { v }",
        "ng(-9223372036854775808) == -9223372036854775808",
    );
}

#[test]
fn e2e_runtime_negation_of_actual_min_value_still_overflows() {
    // The most-negative-*literal* fix must not weaken the *runtime*
    // overflow check: negating a variable that already holds min(ty)
    // (as opposed to a literal token that directly denotes min(ty))
    // is still a real overflow -- checked_neg is untouched by this fix.
    expect_diag(
        "fn ng(i32 m) : i32 { -m }\nfn main() { ng(-2147483647 - 1); }",
        "diag.arith-overflow",
    );
}

#[test]
fn conf_negated_literal_min() {
    expect_true("-2147483648 < -2147483647");
}

#[test]
fn conf_negated_literal_below_min() {
    expect_diag_static("fn main() { -2147483649; }", "diag.literal-out-of-range");
}

#[test]
fn conf_negated_literal_parenthesized() {
    expect_diag_static("fn main() { -(2147483648); }", "diag.literal-out-of-range");
}

#[test]
fn conf_negated_literal_unsigned_rejected() {
    expect_diag_static("fn main() { -0: u8; }", "diag.type-mismatch");
}

#[test]
fn conf_neg_unsigned_rejected() {
    expect_diag_static("fn ng(u8 m) : u8 { -m }\nfn main() { ng(0); }", "diag.type-mismatch");
}

#[test]
fn conf_div_min_neg_one_static() {
    expect_diag_static("fn main() { -2147483648 / -1; }", "diag.div-overflow");
    expect_diag_static("fn main() { -2147483648 % -1; }", "diag.div-overflow");
}

#[test]
fn e2e_negated_literal_i128_below_magnitude_of_min() {
    // `[Literal-Negated]` for i128 once accepted only the minimum
    // itself: every other negated i128 literal, `-1` included, was
    // diag.literal-out-of-range.
    expect_true("{ i128 x = -1; x + 1 == 0 }");
    expect_true("{ i128 y = -170141183460469231731687303715884105727; y - 1 < y }");
}

#[test]
fn conf_limits_max_i32() {
    expect_true("max_value<i32>() == 2147483647");
}

#[test]
fn conf_limits_generic_ok() {
    expect_true_with(
        "fn or_max<T>(Option<T> r) : T { match (r) { Some(x) : x, None : max_value<T>(), } }",
        "or_max(checked_add(max_value<u8>(), 1: u8)) == 255",
    );
}

#[test]
fn conf_limits_not_integer() {
    expect_diag_static("fn main() { max_value<bool>(); }", "diag.type-mismatch");
    expect_diag_static("fn main() { min_value<bool>(); }", "diag.type-mismatch");
}

#[test]
fn conf_limits_float() {
    expect_true("max_value<f32>() == 3.4028235e38: f32 && min_value<f64>() == -1.7976931348623157e308");
}

#[test]
fn conf_widen_f32_to_f64() {
    expect_true("widen<f64>(0.5: f32) == 0.5 && to_float<f64>(0.1: f32) == widen<f64>(0.1: f32)");
}

#[test]
fn conf_to_float_f64_to_f32_rounds() {
    expect_true("to_float<f32>(1.0 / 3.0) == 0.33333334: f32 && to_float<f32>(1.0e300) == to_float<f32>(2.0e300)");
}

#[test]
fn conf_widen_f64_to_f32_rejected() {
    expect_diag_static("fn main() { f32 x = widen<f32>(1.5); }", "diag.type-mismatch");
}

#[test]
fn conf_reinterpret_float_bits() {
    expect_true("reinterpret<u32>(1.0: f32) == 0x3f80_0000: u32 && reinterpret<i64>(-0.0) == min_value<i64>() && reinterpret<f64>(reinterpret<u64>(2.5)) == 2.5");
}

#[test]
fn conf_reinterpret_float_width_rejected() {
    expect_diag_static("fn main() { u64 b = reinterpret<u64>(1.0: f32); }", "diag.type-mismatch");
}

#[test]
fn conf_limits_generic_not_integer() {
    expect_diag_static("fn lo<T>() : T { min_value<T>() }\nfn main() { lo<bool>(); }", "diag.type-mismatch");
}

#[test]
fn conf_limits_uninferable() {
    expect_diag_static("fn main() { i32 x = max_value(); }", "diag.cannot-infer-type-parameter");
}

#[test]
fn conf_limits_overflow_dynamic() {
    // Not a literal, so not refuted statically (D-0026 (c)): the static
    // pass accepts it and the run faults.
    let src = "fn main() { max_value<i32>() + 1; }";
    assert_eq!(check_source_static(src), Ok(()));
    expect_diag(src, "diag.arith-overflow");
}

#[test]
fn conf_alt_mixed_types_rejected() {
    expect_diag_static("fn main() { wrapping_add(1: i32, 2: i64); }", "diag.type-mismatch");
}

#[test]
fn conf_alt_non_integer_rejected() {
    expect_diag_static("fn main() { checked_add(1.0, 2.0); }", "diag.type-mismatch");
    expect_diag_static("fn main() { saturating_mul(true, false); }", "diag.type-mismatch");
}

#[test]
fn conf_alt_generic_non_integer_rejected() {
    expect_diag_static("fn f<T>(T a, T b) : T { wrapping_add(a, b) }\nfn main() { f(true, false); }", "diag.type-mismatch");
}

#[test]
fn conf_convert_wrong_kind_rejected() {
    expect_diag_static("fn main() { to_int<i32>(1); }", "diag.type-mismatch");
    expect_diag_static("fn main() { widen<i64>(1.5); }", "diag.type-mismatch");
    expect_diag_static("fn main() { to_float<i32>(1); }", "diag.type-mismatch");
}

#[test]
fn e2e_alt_literal_operand_takes_other_operand_type() {
    // `[T-Alt]`'s one-type check must not trip on an unsuffixed literal,
    // which takes its type from the other operand.
    expect_true("{ u8 x = 3; wrapping_add(x, 1) == 4 }");
    expect_true("wrapping_add(1, -1) == 0");
}

#[test]
fn conf_widen_not_contained_rejected() {
    expect_diag_static("fn w(i32 x) : u64 { widen<u64>(x) }\nfn main() { w(5); }", "diag.type-mismatch");
    expect_diag_static("fn w(usize n) : i64 { widen<i64>(n) }\nfn main() { w(5); }", "diag.type-mismatch");
}

#[test]
fn conf_narrow_contained_ok() {
    expect_true("narrow<i64>(5: i32) == 5");
    expect_true("{ usize n = 7; narrow<u64>(n) == 7 }");
    expect_true("narrow_wrapping<u64>(5: u8) == 5");
}

#[test]
fn conf_reinterpret_same_sign_rejected() {
    expect_diag_static("fn main() { reinterpret<i64>(5: i32); }", "diag.type-mismatch");
    expect_diag_static("fn main() { reinterpret<u64>(5: u32); }", "diag.type-mismatch");
}

#[test]
fn conf_call_too_many_args() {
    expect_diag_static("fn f(i32 a) : i32 { a }\nfn main() { f(1, 2); }", "diag.type-mismatch");
}

#[test]
fn conf_call_too_few_args() {
    expect_diag_static("fn g(i32 a, i32 b) : i32 { a }\nfn main() { g(1); }", "diag.type-mismatch");
}

#[test]
fn conf_closure_call_arity_rejected() {
    expect_diag_static("fn main() { auto c = [](i32 a) { a }; c(1, 2); }", "diag.type-mismatch");
    expect_diag_static("fn main() { auto c = [](i32 a) { a }; auto d = c; d(1, 2); }", "diag.type-mismatch");
    expect_diag_static("fn f(i32 a) : i32 { a }\nfn main() { fn(i32) : i32 g = f; g(); }", "diag.type-mismatch");
}

#[test]
fn conf_call_non_callable_rejected() {
    expect_diag_static("fn main() { i32 x = 3; x(1); }", "diag.type-mismatch");
}

#[test]
fn conf_item_shadows_intrinsic() {
    expect_true_with("fn widen(i32 a) : i32 { a + 1 }", "widen(1) == 2");
}

#[test]
fn conf_print_int() {
    expect_ok("import std;\nfn main() { printf(\"%v\", -42); }");
}

#[test]
fn conf_print_float() {
    expect_ok("import std;\nfn main() { printf(\"%v\", 0.1); }");
}

#[test]
fn conf_print_string_ref() {
    expect_ok("import std;\nfn main() { String s = String::from_str(\"x\"); printf(\"%v\", &s); }");
}

#[test]
fn conf_print_string_by_value_rejected() {
    expect_diag_static("import std;\nfn main() { String s = String::from_str(\"x\"); printf(\"%v\", s); }", "diag.type-mismatch");
}

#[test]
fn conf_print_not_printable() {
    expect_diag_static("import std;\nfn main() { printf(\"%v\", Some(1)); }", "diag.type-mismatch");
    expect_diag_static("import std;\nfn main() { printf(\"%v\", 1, 2); }", "diag.type-mismatch");
}

#[test]
fn conf_print_generic_ok() {
    expect_ok("import std;\nfn show<T>(T x) { printf(\"%v\", x); }\nfn main() { show(2.5); show(true); show(\"s\"); }");
}

// CHG-0038: `read_line`'s type; the function is never called, so no
// input is read (the cases that read are file cases with `stdin-hex:`).
#[test]
fn conf_read_line_typed() {
    expect_ok("import std;\nfn next() : Result<Option<String>, ReadError> { read_line() }\nfn main() { }");
}

#[test]
fn conf_read_outside_unsafe_rejected() {
    expect_diag_static("import std;\nfn f(rawptr<u8> p) : isize { read(p, 1) }\nfn main() { }", "diag.trusted-outside-unsafe");
}

// CHG-0040: program arguments. These run with no arguments; the cases
// that pass some are file cases with an `args:` header.
#[test]
fn conf_arg_typed() {
    expect_ok("fn first() : Result<String, Utf8Error> { arg(0) }\nfn main() { }");
}

#[test]
fn conf_arg_bytes_outside_unsafe_rejected() {
    expect_diag_static("fn f(rawptr<u8> p) : isize { arg_bytes(0, p, 1) }\nfn main() { }", "diag.trusted-outside-unsafe");
}

#[test]
fn conf_arg_count_none() {
    expect_true("arg_count() == 0");
}

#[test]
fn conf_arg_out_of_range() {
    expect_diag_dynamic_only("fn main() { auto a = arg(0); }", "diag.index-out-of-bounds");
}

// CHG-0040: `fn main() : u8` is the exit status (`[Terminate-Ok]`).
#[test]
fn conf_main_exit_status() {
    assert_eq!(coby::run_source_phased(&with_std("fn main() : u8 { 7 }")), coby::Outcome::Ok(7));
    assert_eq!(coby::run_source_phased(&with_std("fn main() { }")), coby::Outcome::Ok(0));
}

// The destructor runs before the status is reported: `dropped` is
// printed and the process exits 3.
#[test]
fn conf_main_exit_status_after_destructors() {
    use std::io::Write;
    let src = "resource struct S { i32 x; }\nfn S::drop(ref<S, exclusive> self) { printf(\"dropped\\n\"); }\nfn main() : u8 { S s = S { .x = 1 }; return 3; }";
    let mut path = std::env::temp_dir();
    path.push(format!("coby_test_{}_{}.cb", std::process::id(), rand_suffix()));
    std::fs::File::create(&path).unwrap().write_all(with_std(src).as_bytes()).unwrap();
    let out = std::process::Command::new(env!("CARGO_BIN_EXE_coby")).arg(&path).output().expect("run coby");
    std::fs::remove_file(&path).ok();
    assert_eq!(out.status.code(), Some(3), "stderr: {}", String::from_utf8_lossy(&out.stderr));
    assert_eq!(out.stdout, b"dropped\n");
    assert!(out.stderr.is_empty());
}

#[test]
fn conf_main_with_parameters_rejected() {
    expect_diag_static("fn main(i32 x) { }", "diag.no-main");
}

#[test]
fn conf_main_returns_i32_rejected() {
    expect_diag_static("fn main() : i32 { 0 }", "diag.no-main");
}

// CHG-0041: text conversions (`rule.stdlib.text`).
#[test]
fn conf_string_append_text() {
    expect_true("{ String s = String::new(); String::append(&mut s, 42); String::append(&mut s, \" \"); String::append(&mut s, 2.5); String::len(&s) == 6 }");
}

#[test]
fn conf_string_append_not_printable() {
    expect_diag_static("fn main() { String s = String::new(); String::append(&mut s, Some(1)); }", "diag.type-mismatch");
}

#[test]
fn conf_string_append_self_conflict() {
    expect_diag_dynamic_only("fn main() { String s = String::from_str(\"a\"); String::append(&mut s, &s); }", "diag.aliasing-conflict");
}

#[test]
fn conf_string_as_bytes() {
    expect_true("{ String s = String::from_str(\"hi\"); *Vec::index_shared(String::as_bytes(&s), 1) == 105 }");
}

#[test]
fn conf_parse_int() {
    expect_true("{ String s = String::from_str(\"-42\"); i32 v = match (parse<i32>(&s)) { Ok(v) : v, Err(_) : 0 }; v == -42 }");
}

#[test]
fn conf_parse_invalid_offset() {
    expect_true("{ String s = String::from_str(\"12x4\"); usize k = match (parse<i32>(&s)) { Ok(_) : 9, Err(e) : match (e) { Invalid(k) : k, _ : 8 } }; k == 2 }");
}

#[test]
fn conf_parse_out_of_range() {
    expect_true("{ String s = String::from_str(\"128\"); bool r = match (parse<i8>(&s)) { Ok(_) : false, Err(e) : match (e) { OutOfRange : true, _ : false } }; r }");
}

#[test]
fn conf_parse_not_numeric() {
    expect_diag_static("fn main() { String s = String::new(); auto r = parse<bool>(&s); }", "diag.type-mismatch");
}

// CHG-0044 (D-0035): radix literals, compound assignment, `for`.
#[test]
fn conf_radix_literals() {
    expect_true("{ u8 a = 0xFF; u8 b = 0b1010; u8 c = 0o17; widen<u32>(a) + widen<u32>(b) + widen<u32>(c) == 280 }");
}

#[test]
fn conf_compound_assign_ops() {
    expect_true("{ i32 x = 7; x -= 2; x *= 3; x /= 2; x %= 5; x <<= 4; x >>= 1; x |= 1; x &= 0xff; x ^= 0b1010; x == 27 }");
}

#[test]
fn conf_compound_assign_overflow() {
    expect_diag_dynamic_only("fn bump(i32 x) : i32 { i32 y = x; y += 1; y } fn main() { bump(2147483647); }", "diag.arith-overflow");
}

#[test]
fn conf_compound_assign_call_target() {
    expect_true("{ Vec<i32> v = Vec::new(); Vec::push(&mut v, 10); Vec::push(&mut v, 20); *Vec::index_exclusive(&mut v, 1) += 5; *Vec::index_shared(&v, 1) == 25 }");
}

#[test]
fn conf_for_sum_continue() {
    expect_true("{ u64 s = 0; for (u64 i = 0; i < 10; i += 1) { if (i % 3 == 0) { continue; } s += i; } s == 27 }");
}

#[test]
fn conf_for_empty_parts() {
    expect_true("{ i32 n = 0; for (; n < 5; ) { n += 2; } for (;;) { break; } n == 6 }");
}

#[test]
fn conf_for_variable_scoped() {
    expect_diag_static("fn main() { for (i32 i = 0; i < 1; i += 1) { } auto j = i; }", "diag.unbound-name");
}

// CHG-0046 (D-0038): `printf`, `String::appendf`; the output is a file case.
#[test]
fn conf_printf_arg_mismatch() {
    expect_diag_static("fn main() { printf(\"%s\", 42); }", "diag.type-mismatch");
}

#[test]
fn conf_printf_count_mismatch() {
    expect_diag_static("fn main() { printf(\"%d %d\", 1); }", "diag.type-mismatch");
}

#[test]
fn conf_printf_format_invalid() {
    expect_diag_static("fn main() { printf(\"%q\", 1); }", "diag.format-invalid");
}

#[test]
fn conf_printf_format_not_literal() {
    expect_diag_static("fn main() { str f = \"%d\"; printf(f, 1); }", "diag.format-invalid");
}

#[test]
fn conf_printf_generic_instantiation() {
    expect_diag_static("fn show<T>(T x) { printf(\"%d\", x); } fn main() { show(2.5); }", "diag.type-mismatch");
}

#[test]
fn conf_appendf_builds_string() {
    expect_true("{ String s = String::new(); String::appendf(&mut s, \"%05.1f|%x\", -2.5, 255); String::len(&s) == 8 }");
}

#[test]
fn conf_appendf_self() {
    expect_true("{ String s = String::from_str(\"ab\"); String::appendf(&mut s, \"%s-\", &s); String::len(&s) == 5 }");
}

// CHG-0047 (D-0039): printf is the one way to write output.
#[test]
fn conf_printf_value() {
    expect_true("{ String s = sprintf(\"%v|%-3v|%3v|%v\", 0.1, 7, \"ab\", false); String::len(&s) == 17 }");
}

#[test]
fn conf_printf_value_precision_rejected() {
    expect_diag_static("fn main() { printf(\"%.2v\", 1.5); }", "diag.format-invalid");
}

#[test]
fn conf_sprintf_returns_string() {
    expect_true("{ String s = sprintf(\"%d-%s\", 12, \"ab\"); String::len(&s) == 5 }");
}

#[test]
fn conf_print_unbound() {
    expect_diag_static("fn main() { print(1); }", "diag.unbound-name");
}

#[test]
fn conf_std_print_private() {
    expect_diag_static("fn main() { std::print(1); }", "diag.name-not-visible");
}

#[test]
fn conf_print_own_declaration_ok() {
    expect_ok_with("fn print(i32 x) { printf(\"%d\", x); }", "print(1);");
}

// CHG-0048 (D-0040): eprintf writes to standard error.
#[test]
fn conf_eprintf_format_checked() {
    expect_diag_static("fn main() { eprintf(\"%d\\n\", \"x\"); }", "diag.type-mismatch");
}

#[test]
fn conf_write_err_private() {
    expect_diag_static("fn main() { unsafe { std::write_err(str_ptr(\"x\"), 1); } }", "diag.name-not-visible");
}

// CHG-0049 (D-0041): hash tables.
#[test]
fn conf_hashmap_basic() {
    expect_true("{ HashMap<i32, bool> m = HashMap::new(); HashMap::insert(&mut m, 5, true); i32 k = 5; i32 j = 6; HashMap::len(&m) == 1 && HashMap::contains(&m, &k) && !HashMap::contains(&m, &j) }");
}

#[test]
fn conf_hashmap_insert_replaces() {
    expect_true("{ HashMap<str, i32> m = HashMap::new(); HashMap::insert(&mut m, \"a\", 1); i32 old = Option::unwrap_or(HashMap::insert(&mut m, \"a\", 2), 0); str k = \"a\"; i32 now = match (HashMap::get(&m, &k)) { Some(v) : *v, None : 0 }; old == 1 && now == 2 && HashMap::len(&m) == 1 }");
}

#[test]
fn conf_hashmap_entry_counts() {
    expect_true("{ HashMap<u8, u32> m = HashMap::new(); for (u8 i = 0; i < 100; i += 1) { *HashMap::entry(&mut m, i % 7, 0) += 1; } u8 k = 0; HashMap::len(&m) == 7 && *HashMap::value_at(&m, 0) == 15 && *HashMap::key_at(&m, 6) == 6 }");
}

#[test]
fn conf_hashmap_remove_swaps() {
    expect_true("{ HashMap<i32, i32> m = HashMap::new(); HashMap::insert(&mut m, 1, 10); HashMap::insert(&mut m, 2, 20); HashMap::insert(&mut m, 3, 30); i32 k = 1; i32 v = Option::unwrap_or(HashMap::remove(&mut m, &k), 0); v == 10 && *HashMap::key_at(&m, 0) == 3 && *HashMap::key_at(&m, 1) == 2 }");
}

#[test]
fn conf_hashmap_remove_ordered() {
    expect_true("{ HashMap<i32, i32> m = HashMap::new(); HashMap::insert(&mut m, 1, 10); HashMap::insert(&mut m, 2, 20); HashMap::insert(&mut m, 3, 30); i32 k = 1; i32 v = Option::unwrap_or(HashMap::remove_ordered(&mut m, &k), 0); v == 10 && *HashMap::key_at(&m, 0) == 2 && *HashMap::key_at(&m, 1) == 3 }");
}

#[test]
fn conf_hashset_basic() {
    expect_true("{ HashSet<String> s = HashSet::new(); bool a = HashSet::insert(&mut s, String::from_str(\"x\")); bool b = HashSet::insert(&mut s, String::from_str(\"x\")); String k = String::from_str(\"x\"); a && !b && HashSet::len(&s) == 1 && HashSet::remove(&mut s, &k) && HashSet::len(&s) == 0 }");
}

#[test]
fn conf_hashmap_float_key_rejected() {
    expect_diag_static("fn main() { HashMap<f64, i32> m = HashMap::new(); }", "diag.type-mismatch");
}

#[test]
fn conf_hashmap_struct_key_rejected() {
    expect_diag_static("struct P { i32 x; } fn main() { HashSet<P> s = HashSet::new(); }", "diag.type-mismatch");
}

#[test]
fn conf_hashmap_generic_key_checked() {
    expect_diag_static("fn keep<K>(K k) { HashSet<K> s = HashSet::new(); HashSet::insert(&mut s, k); } fn main() { keep(1.5); }", "diag.type-mismatch");
}

#[test]
fn conf_hashmap_fields_private() {
    expect_diag_static("fn main() { HashMap<i32, i32> m = HashMap::new(); usize n = Vec::len(&m.keys); }", "diag.name-not-visible");
}

#[test]
fn conf_key_hash_std_only() {
    expect_diag_static("fn main() { i32 k = 1; u64 h = key_hash(&k); }", "diag.unbound-name");
}

// CHG-0050 (D-0042): foreach.
#[test]
fn conf_foreach_borrowed() {
    expect_true("{ Vec<i32> v = Vec::new(); Vec::push(&mut v, 3); Vec::push(&mut v, 4); i32 s = 0; foreach (x in &v) { s += *x; } s == 7 && Vec::len(&v) == 2 }");
}

#[test]
fn conf_foreach_mut() {
    expect_true("{ Vec<i32> v = Vec::new(); Vec::push(&mut v, 3); Vec::push(&mut v, 4); foreach (x in &mut v) { *x *= 10; } *Vec::index_shared(&v, 1) == 40 }");
}

#[test]
fn conf_foreach_consumed() {
    expect_true("{ Vec<String> v = Vec::new(); Vec::push(&mut v, String::from_str(\"ab\")); Vec::push(&mut v, String::from_str(\"cde\")); usize n = 0; foreach (s in v) { n += String::len(&s); } n == 5 }");
}

#[test]
fn conf_foreach_index() {
    expect_true("{ Vec<i32> v = Vec::new(); Vec::push(&mut v, 3); Vec::push(&mut v, 4); usize last = 0; foreach (i, x in &v) { last = i; } last == 1 }");
}

#[test]
fn conf_foreach_array() {
    expect_true("{ array<i32, 3> a = [1, 2, 3]; foreach (x in &mut a) { *x += 1; } i32 s = 0; foreach (x in a) { s += x; } s == 9 }");
}

#[test]
fn conf_foreach_hashmap() {
    expect_true("{ HashMap<str, i32> m = HashMap::new(); HashMap::insert(&mut m, \"a\", 1); HashMap::insert(&mut m, \"b\", 2); foreach (k, v in &mut m) { *v *= 3; } i32 s = 0; foreach (k, v in m) { s += v; } s == 9 }");
}

#[test]
fn conf_foreach_hashset() {
    expect_true("{ HashSet<u8> h = HashSet::new(); HashSet::insert(&mut h, 4); HashSet::insert(&mut h, 5); u8 s = 0; foreach (x in &h) { s += *x; } s == 9 }");
}

#[test]
fn conf_foreach_ref_binding() {
    expect_true("{ Vec<i32> v = Vec::new(); Vec::push(&mut v, 2); ref<Vec<i32>, shared> r = &v; i32 s = 0; foreach (x in r) { s += *x; } s == 2 }");
}

#[test]
fn conf_foreach_in_is_a_name() {
    expect_true("{ i32 in = 4; in == 4 }");
}

#[test]
fn conf_foreach_change_collection_rejected() {
    expect_diag_static("fn main() { Vec<i32> v = Vec::new(); foreach (x in &v) { Vec::push(&mut v, 1); } }", "diag.aliasing-conflict");
}

#[test]
fn conf_foreach_consumed_then_used_rejected() {
    expect_diag_static("fn main() { Vec<i32> v = Vec::new(); foreach (x in v) { } usize n = Vec::len(&v); }", "diag.stale-binding");
}

#[test]
fn conf_foreach_not_iterable() {
    expect_diag_static("fn main() { foreach (x in 5) { } }", "diag.type-mismatch");
}

#[test]
fn conf_foreach_hashmap_one_name_rejected() {
    expect_diag_static("fn main() { HashMap<i32, i32> m = HashMap::new(); foreach (x in &m) { } }", "diag.type-mismatch");
}

#[test]
fn conf_foreach_hashset_mut_rejected() {
    expect_diag_static("fn main() { HashSet<i32> h = HashSet::new(); foreach (x in &mut h) { } }", "diag.type-mismatch");
}

#[test]
fn conf_printf_ref_value() {
    expect_true("{ i32 x = 5; ref<i32, shared> r = &x; String s = sprintf(\"%d|%v|%x\", r, r, r); String::len(&s) == 5 }");
}

// CHG-0051 (D-0043): digit separators.
#[test]
fn conf_digit_separators() {
    expect_true("{ u64 a = 1_000_000; u32 b = 0xFFFF_0000; u8 c = 0b1010_0101; f64 d = 1_234.567_8e1_0; a == 1000000 && b == 4294901760 && c == 165 && d == 12345678000000.0 }");
}

#[test]
fn conf_parse_rejects_digit_separator() {
    expect_true("{ String s = String::from_str(\"1_000\"); Result::unwrap_or(parse<u32>(&s), 7) == 7 }");
}

// Tier 6 findings (conformance 3.38.0): coby evaluated `&mut *f(k)`'s call
// twice, and left a statement's temporaries alive after a `return`.
#[test]
fn conf_reborrow_call_evaluated_once() {
    expect_true("{ HashMap<String, u32> m = HashMap::new(); String k = String::from_str(\"a\"); *HashMap::entry(&mut m, k, 0) += 1; HashMap::len(&m) == 1 }");
}

#[test]
fn conf_return_ends_statement_temporaries() {
    expect_true_with("fn look(ref<Vec<String>, shared> v) : i32 { match (Some(Vec::index_shared(v, 0))) { Some(r) : { return 1; }, None : {}, } 0 }", "{ Vec<String> v = Vec::new(); Vec::push(&mut v, String::from_str(\"x\")); look(&v) == 1 }");
}

// CHG-0052 (D-0044): the gaps Tier 6 met.
#[test]
fn conf_string_clone() {
    expect_true("{ String a = String::from_str(\"ab\"); String b = String::clone(&a); String::append(&mut b, \"c\"); String::len(&a) == 2 && String::len(&b) == 3 }");
}

#[test]
fn conf_string_push_ascii() {
    expect_true("{ String s = String::new(); String::push_ascii(&mut s, b'o'); String::push_ascii(&mut s, b'k'); String::len(&s) == 2 }");
}

#[test]
fn conf_string_push_ascii_rejects_non_ascii() {
    expect_diag("fn main() { String s = String::new(); String::push_ascii(&mut s, 200); }", "diag.not-ascii");
}

#[test]
fn conf_vec_clear() {
    expect_true("{ Vec<String> v = Vec::new(); Vec::push(&mut v, String::from_str(\"a\")); Vec::clear(&mut v); Vec::push(&mut v, String::from_str(\"b\")); Vec::len(&v) == 1 }");
}

#[test]
fn conf_destructure_all_fields() {
    expect_true_with("struct P { String a; i32 b; }", "{ P { a, b } = P { .a = String::from_str(\"x\"), .b = 2 }; String::len(&a) == 1 && b == 2 }");
}

#[test]
fn conf_destructure_missing_field_rejected() {
    expect_diag_static("struct P { i32 a; i32 b; } fn main() { P p = P { .a = 1, .b = 2 }; P { a } = p; }", "diag.type-mismatch");
}

#[test]
fn conf_destructure_destructor_rejected() {
    expect_diag_static("resource struct T { i32 a; } fn T::drop(ref<T, exclusive> self) { } fn main() { T t = T { .a = 1 }; T { a } = t; }", "diag.move-out-of-field");
}

#[test]
fn conf_foreach_map_position() {
    expect_true("{ HashMap<str, i32> m = HashMap::new(); HashMap::insert(&mut m, \"a\", 1); HashMap::insert(&mut m, \"b\", 2); usize last = 9; foreach (i, k, v in &m) { last = i; } last == 1 }");
}

#[test]
fn conf_foreach_three_names_rejected() {
    expect_diag_static("fn main() { Vec<i32> v = Vec::new(); foreach (i, j, x in &v) { } }", "diag.type-mismatch");
}

// CHG-0053 (D-0045): recursive types and Box.
#[test]
fn conf_recursive_type_rejected() {
    expect_diag_static("struct N { i32 v; Option<N> next; } fn main() { N n = N { .v = 1, .next = None }; }", "diag.recursive-type");
}

#[test]
fn conf_recursive_type_array_rejected() {
    expect_diag_static("struct A { array<A, 2> kids; } fn main() { i32 x = 1; }", "diag.recursive-type");
}

#[test]
fn conf_box_recursive_type_ok() {
    expect_true_with("struct Node { i32 v; Option<Box<Node>> next; }", "{ Option<Box<Node>> l = None; l = Some(Box::new(Node { .v = 1, .next = l })); l = Some(Box::new(Node { .v = 2, .next = l })); match (l) { Some(b) : Box::get(&b).v == 2, None : false } }");
}

#[test]
fn conf_box_get_mut() {
    expect_true("{ Box<i32> b = Box::new(41); *Box::get_mut(&mut b) += 1; *Box::get(&b) == 42 }");
}

#[test]
fn conf_box_into_inner() {
    expect_true("{ Box<String> b = Box::new(String::from_str(\"abc\")); String s = Box::into_inner(b); String::len(&s) == 3 }");
}

// CHG-0054 (D-0046): match through a reference.
#[test]
fn conf_match_by_ref_shared() {
    expect_true("{ Option<i32> o = Some(4); i32 x = match (&o) { Some(n) : *n, None : 0 }; x == 4 && Option::unwrap_or(o, 0) == 4 }");
}

#[test]
fn conf_match_by_ref_exclusive() {
    expect_true("{ Option<i32> o = Some(4); match (&mut o) { Some(n) : *n += 1, None : {}, } Option::unwrap_or(o, 0) == 5 }");
}

#[test]
fn conf_match_by_ref_resource_payload() {
    expect_true("{ Option<String> o = Some(String::from_str(\"abc\")); usize n = match (&o) { Some(s) : String::len(s), None : 0 }; String t = Option::unwrap_or(o, String::new()); n == 3 && String::len(&t) == 3 }");
}

#[test]
fn conf_match_by_ref_box_tree() {
    expect_true_with("enum T { Leaf(i32), Node(Box<T>) } fn depth(ref<T, shared> t) : i32 { match (t) { Leaf(v) : *v, Node(b) : 1 + depth(Box::get(b)), } }", "{ T t = Node(Box::new(Node(Box::new(Leaf(40))))); depth(&t) == 42 }");
}

#[test]
fn conf_match_by_ref_write_through_shared_rejected() {
    expect_diag_static("fn main() { Option<i32> o = Some(1); match (&o) { Some(n) : *n = 3, None : {}, } }", "diag.write-through-shared");
}

#[test]
fn conf_match_by_ref_replace_while_bound_rejected() {
    expect_diag("fn main() { Option<i32> o = Some(1); match (&o) { Some(n) : { o = None; i32 k = *n; }, None : {}, } }", "diag.aliasing-conflict");
}

// CHG-0055 (D-0047): slices, `$`, indexing a Vec.
#[test]
fn conf_slice_array() {
    expect_true("{ auto a = [10, 20, 30, 40, 50]; auto s = &a[1..4]; slice_len(s) == 3 && s[0] == 20 && s[2] == 40 }");
}

#[test]
fn conf_slice_vec() {
    expect_true("{ Vec<i32> v = Vec::new(); Vec::push(&mut v, 1); Vec::push(&mut v, 2); Vec::push(&mut v, 3); auto s = &v[1..$]; slice_len(s) == 2 && s[1] == 3 }");
}

#[test]
fn conf_slice_of_slice() {
    expect_true("{ auto a = [1, 2, 3, 4, 5]; auto s = &a[1..5]; auto t = &s[1..3]; slice_len(t) == 2 && t[0] == 3 }");
}

#[test]
fn conf_slice_dollar() {
    expect_true("{ auto a = [1, 2, 3, 4, 5]; auto s = &a[$ - 3 .. $]; s[0] == 3 && a[$ - 1] == 5 }");
}

#[test]
fn conf_slice_param_any_source() {
    expect_true_with("fn sum(slice<i32, shared> s) : i32 { i32 t = 0; foreach (x in s) { t += *x; } t }", "{ auto a = [1, 2, 3]; Vec<i32> v = Vec::new(); Vec::push(&mut v, 10); sum(&a[0..$]) + sum(&v[0..$]) == 16 }");
}

#[test]
fn conf_slice_exclusive_write() {
    expect_true("{ auto a = [1, 2, 3]; { auto s = &mut a[1..3]; s[0] = 20; s[1] += 1; } a[1] == 20 && a[2] == 4 }");
}

#[test]
fn conf_slice_foreach() {
    expect_true("{ Vec<i32> v = Vec::new(); Vec::push(&mut v, 1); Vec::push(&mut v, 2); foreach (x in &mut v[0..$]) { *x *= 10; } v[0] + v[1] == 30 }");
}

#[test]
fn conf_vec_index() {
    expect_true("{ Vec<i32> v = Vec::new(); Vec::push(&mut v, 7); Vec::push(&mut v, 8); v[0] + v[$ - 1] == 15 }");
}

#[test]
fn conf_vec_index_write() {
    expect_true("{ Vec<i32> v = Vec::new(); Vec::push(&mut v, 7); v[0] = 9; v[0] += 1; v[0] == 10 }");
}

#[test]
fn conf_slice_bounds_rejected() {
    expect_diag_static("fn main() { auto a = [1, 2, 3]; auto s = &a[2..4]; }", "diag.index-out-of-bounds");
}

#[test]
fn conf_slice_bounds_dynamic() {
    expect_diag("fn main() { Vec<i32> v = Vec::new(); auto s = &v[0..1]; }", "diag.index-out-of-bounds");
}

#[test]
fn conf_slice_push_while_borrowed_rejected() {
    expect_diag_static("fn main() { Vec<i32> v = Vec::new(); Vec::push(&mut v, 1); auto s = &v[0..1]; Vec::push(&mut v, 2); }", "diag.aliasing-conflict");
}

#[test]
fn conf_slice_write_through_shared_rejected() {
    expect_diag_static("fn main() { auto a = [1, 2]; auto s = &a[0..2]; s[0] = 5; }", "diag.write-through-shared");
}

#[test]
fn conf_slice_escape_rejected() {
    expect_diag_static("fn f() : slice<i32, shared> { auto a = [1, 2]; &a[0..2] } fn main() { auto s = f(); }", "diag.reference-escapes-scope");
}

#[test]
fn conf_vec_index_move_out_rejected() {
    expect_diag_static("fn main() { Vec<String> v = Vec::new(); Vec::push(&mut v, String::from_str(\"a\")); String t = v[0]; }", "diag.move-out-of-field");
}

#[test]
fn conf_dollar_empty_overflow() {
    expect_diag("fn main() { Vec<i32> v = Vec::new(); i32 x = v[$ - 1]; }", "diag.arith-overflow");
}

#[test]
fn conf_slice_in_vec() {
    expect_true("{ auto a = [1, 2]; Vec<slice<i32, shared>> vs = Vec::new(); Vec::push(&mut vs, &a[0..1]); Vec::push(&mut vs, &a[1..2]); vs[1][0] == 2 }");
}

#[test]
fn conf_slice_in_struct() {
    expect_true_with("struct View { slice<i32, shared> s; }", "{ auto a = [1, 2, 3]; View w = View { .s = &a[1..$] }; w.s[0] == 2 && slice_len(w.s) == 2 }");
}

#[test]
fn conf_slice_compare_rejected() {
    expect_diag_static("fn main() { auto a = [1, 2]; auto s = &a[0..$]; bool e = s == &a[0..$]; }", "diag.type-mismatch");
}

// CHG-0056 (D-0048): swap and replace.
#[test]
fn conf_if_literal_branch_takes_sibling_type() {
    expect_true("{ i64 x = 5000000000; auto a = if (x > 1) { x } else { 0 }; auto b = if (x < 1) { 0 } else { x }; a + b == 10000000000 }");
}

#[test]
fn conf_match_literal_arm_takes_later_type() {
    expect_true("{ Option<i64> o = None; auto a = match (o) { None : 1 << 40, Some(v) : v }; a == 1099511627776 }");
}

#[test]
fn conf_exclusive_ref_arg_for_shared_param() {
    expect_true_with("fn n(ref<Vec<i32>, exclusive> v) : usize { Vec::push(v, 1); Vec::len(v) }", "{ Vec<i32> v = Vec::new(); n(&mut v) == 1 }");
}

#[test]
fn conf_exclusive_slice_arg_for_shared_param() {
    expect_true_with(
        "fn total(slice<i64, shared> s) : i64 { i64 t = 0; foreach (x in s) { t += *x; } t } fn bump(slice<i64, exclusive> s) { s[0] = total(s); }",
        "{ array<i64, 3> a = [1, 2, 3]; bump(&mut a[0..$]); a[0] == 6 }",
    );
}

#[test]
fn conf_overwrite_none_of_resource_option() {
    expect_true_with(
        "fn fill(ref<Option<String>, exclusive> o) { *o = Some(String::from_str(\"ab\")); }",
        "{ Option<String> o = None; fill(&mut o); Option<String> p = None; p = Some(String::from_str(\"c\")); match (&o) { Some(s) : String::len(s) == 2, None : false } }",
    );
}

#[test]
fn conf_overwrite_some_of_resource_option_dynamic() {
    expect_diag_dynamic_only("fn main() { Option<String> o = Some(String::from_str(\"a\")); o = Some(String::from_str(\"b\")); }", "diag.overwrite-of-live-resource");
}

#[test]
fn conf_overwrite_owner_type_still_static() {
    expect_diag_static(
        "struct W { Option<String> s; } fn W::drop(ref<W, exclusive> self) { } fn main() { W w = W { .s = None }; w = W { .s = None }; }",
        "diag.overwrite-of-live-resource",
    );
}

#[test]
fn conf_match_wildcard_keeps_scrutinee() {
    expect_true("{ Option<String> o = Some(String::from_str(\"abc\")); match (o) { None : {}, _ : {}, } match (&o) { Some(s) : String::len(s) == 3, None : false } }");
}

#[test]
fn conf_match_move_arm_consumes() {
    expect_diag_dynamic_only("fn main() { Option<String> o = Some(String::from_str(\"a\")); match (o) { Some(s) : {}, None : {}, } match (&o) { Some(s) : {}, None : {}, } }", "diag.stale-binding");
}

#[test]
fn conf_static_assert_holds() {
    expect_ok("import std;\nstruct H { u32 a; u16 b; u16 c; u64 d; } const usize BUF = 4096;\nfn main() { static_assert(sizeof<H>() == 16); static_assert(BUF & (BUF - 1) == 0, \"power of two\"); }");
}

#[test]
fn conf_static_assert_fails() {
    expect_diag_static("fn main() { static_assert(sizeof<u64>() == 4, \"u64 is 8 bytes\"); }", "diag.static-assert-failed");
}

#[test]
fn conf_static_assert_uncalled() {
    expect_diag_static("fn never() { static_assert(1 > 2); } fn main() { }", "diag.static-assert-failed");
}

#[test]
fn conf_static_assert_generic() {
    expect_diag_static("fn small<T>(T x) : T { static_assert(sizeof<T>() <= 8); x } fn main() { small(5: i64); small(1: i128); }", "diag.static-assert-failed");
    expect_ok("fn small<T>(T x) : T { static_assert(sizeof<T>() <= 8); x } fn main() { small(5: i64); small(2.5); }");
}

#[test]
fn conf_static_assert_not_constant() {
    expect_diag_static("fn main() { i32 k = 3; static_assert(k > 1); }", "diag.const-not-constant");
}

#[test]
fn conf_static_assert_overflow() {
    expect_diag_static("fn main() { static_assert(max_value<i32>() + 1 > 0); }", "diag.arith-overflow");
}

#[test]
fn conf_static_assert_not_bool() {
    expect_diag_static("fn main() { static_assert(5); }", "diag.type-mismatch");
}

#[test]
fn conf_view_basic() {
    expect_true("{ String line = String::from_str(\"  hello, world  \"); StringView w = &line[2..7]; StringView r = &w[1..$]; StringView::len(w) == 5 && StringView::len(r) == 4 && StringView::len(&line[0..$]) == 16 }");
}

#[test]
fn conf_view_eq() {
    expect_true("{ String s = String::from_str(\"abcabc\"); StringView a = &s[0..3]; StringView b = &s[3..6]; a == b && a == \"abc\" && \"abc\" == b && a != \"abd\" }");
}

#[test]
fn conf_view_split_trim() {
    expect_true("{ String s = String::from_str(\" a, bb ,c \"); Vec<StringView> p = StringView::split(StringView::trim(&s[0..$]), \",\"); Vec::len(&p) == 3 && StringView::trim(p[1]) == \"bb\" && p[2] == \"c\" }");
}

#[test]
fn conf_view_parse() {
    expect_true("{ String s = String::from_str(\"x=42\"); match (StringView::parse<i64>(&s[2..$])) { Ok(v) : v == 42, Err(_) : false } }");
}

#[test]
fn conf_view_push_while_held_rejected() {
    expect_diag_static("fn main() { String s = String::from_str(\"ab\"); StringView v = &s[0..1]; String::push_ascii(&mut s, 33); StringView::len(v); }", "diag.aliasing-conflict");
}

#[test]
fn conf_view_escape_rejected() {
    expect_diag_static("fn bad() : StringView { String s = String::from_str(\"t\"); &s[0..1] } fn main() { bad(); }", "diag.reference-escapes-scope");
}

#[test]
fn conf_view_char_boundary() {
    expect_diag_dynamic_only("fn main() { String s = String::from_str(\"é!\"); StringView h = &s[0..1]; }", "diag.not-char-boundary");
}

#[test]
fn conf_view_exclusive_rejected() {
    expect_diag_static("fn main() { String s = String::from_str(\"ab\"); auto m = &mut s[0..1]; }", "diag.type-mismatch");
}

#[test]
fn conf_swap_locals() {
    expect_true("{ i32 a = 1; i32 b = 2; swap(&mut a, &mut b); a == 2 && b == 1 }");
}

#[test]
fn conf_swap_fields_through_ref() {
    expect_true_with("struct P { String l; String r; } fn flip(ref<P, exclusive> p) { swap(&mut p.l, &mut p.r); }", "{ P p = P { .l = String::from_str(\"a\"), .r = String::from_str(\"bc\") }; flip(&mut p); String::len(&p.l) == 2 }");
}

#[test]
fn conf_replace_returns_old() {
    expect_true("{ String s = String::from_str(\"old\"); String o = replace(&mut s, String::from_str(\"newer\")); String::len(&o) == 3 && String::len(&s) == 5 }");
}

#[test]
fn conf_vec_swap() {
    expect_true("{ Vec<String> v = Vec::new(); Vec::push(&mut v, String::from_str(\"a\")); Vec::push(&mut v, String::from_str(\"bb\")); Vec::swap(&mut v, 0, 1); Vec::swap(&mut v, 1, 1); String::len(&v[0]) == 2 }");
}

#[test]
fn conf_slice_swap_sort_strings() {
    expect_true_with("fn sort(slice<String, exclusive> s) { for (usize i = 1; i < slice_len(s); i += 1) { usize j = i; while (j > 0 && String::len(&s[j - 1]) > String::len(&s[j])) { slice_swap(s, j - 1, j); j -= 1; } } }", "{ Vec<String> v = Vec::new(); Vec::push(&mut v, String::from_str(\"ccc\")); Vec::push(&mut v, String::from_str(\"a\")); Vec::push(&mut v, String::from_str(\"bb\")); sort(&mut v[0..$]); String::len(&v[0]) == 1 && String::len(&v[2]) == 3 }");
}

#[test]
fn conf_swap_disjoint_elements() {
    expect_true("{ Vec<i32> v = Vec::new(); Vec::push(&mut v, 1); Vec::push(&mut v, 2); swap(&mut v[0], &mut v[1]); v[0] == 2 }");
}

#[test]
fn conf_swap_places_std_only() {
    expect_diag_static("fn main() { i32 a = 1; i32 b = 2; swap_places(&mut a, &mut b); }", "diag.unbound-name");
}

// CHG-0045 (D-0037): literal expressions take an expected number type.
#[test]
fn conf_literal_expression_typed() {
    expect_true("{ u64 x = 65536 * 65536; x == 4294967296 }");
}

#[test]
fn conf_literal_expression_default() {
    expect_diag_static("fn main() { auto d = 65536 * 65536; }", "diag.arith-overflow");
}

#[test]
fn conf_literal_expression_negated() {
    expect_true("{ i64 n = -(1 << 40) + 3; n == -1099511627773 }");
}

#[test]
fn conf_const_literal_expression() {
    expect_true_with("const u64 MIB = 1024 * 1024;", "MIB == 1048576");
}

// CHG-0044 (D-0036): `const`.
#[test]
fn conf_const_folded() {
    expect_true_with("const u64 KB = 1024; const u64 MB = KB * KB;", "MB == 1048576");
}

#[test]
fn conf_const_struct() {
    expect_true_with("struct P { i32 x; } const P ORIGIN = P { .x = 3 };", "ORIGIN.x == 3");
}

#[test]
fn conf_const_exported() {
    expect_true_with("module m { export const i32 K = 7; }", "m::K == 7");
}

#[test]
fn conf_const_shadowed_by_local() {
    expect_true_with("const i32 K = 7; fn f() : i32 { i32 K = 1; K }", "f() == 1");
}

#[test]
fn conf_const_cycle_rejected() {
    expect_diag_static("const i32 A = B + 1; const i32 B = A; fn main() { }", "diag.const-not-constant");
}

#[test]
fn conf_const_not_constant_rejected() {
    expect_diag_static("fn f() : i32 { 3 } const i32 A = f(); fn main() { }", "diag.const-not-constant");
}

#[test]
fn conf_const_resource_rejected() {
    expect_diag_static("const Vec<i32> V = Vec::new(); fn main() { }", "diag.const-not-constant");
}

#[test]
fn conf_const_overflow_static() {
    expect_diag_static("const i32 M = 2147483647; const i32 N = M + 1; fn main() { }", "diag.arith-overflow");
}

#[test]
fn conf_const_assign_rejected() {
    expect_diag_static("const i32 A = 5; fn main() { A = 6; }", "diag.type-mismatch");
}

// CHG-0043: files (`rule.stdlib.file`); the round trip is a file case.
#[test]
fn conf_read_file_not_found() {
    expect_true("{ String p = String::from_str(\"/nonexistent-cobaltc-dir/x.txt\"); bool nf = match (read_file(&p)) { Ok(_) : false, Err(e) : match (e) { NotFound : true, _ : false } }; nf }");
}

#[test]
fn conf_write_file_missing_dir() {
    expect_true("{ String p = String::from_str(\"/nonexistent-cobaltc-dir/x.txt\"); String t = String::from_str(\"x\"); bool nf = match (write_file(&p, &t)) { Ok(_) : false, Err(e) : match (e) { NotFound : true, _ : false } }; nf }");
}

#[test]
fn conf_generic_void_argument() {
    expect_ok("fn id<T>(T x) : T { x } fn main() { id(()); }");
}

// CHG-0041: a program's own variants before imported ones (`spec/17`
// clauses (4) and (4b)).
#[test]
fn conf_own_variant_wins() {
    expect_true_with("enum Shape { Dot(i32), Empty }", "{ Shape s = Empty; i32 v = match (s) { Dot(x) : x, Empty : 5 }; v == 5 }");
}

#[test]
fn conf_qualified_variant_shared_name() {
    expect_true_with("enum Shape { Dot(i32), Empty }", "{ ParseError p = ParseError::Empty; i32 v = match (p) { Empty : 1, _ : 2 }; v == 1 }");
}

#[test]
fn conf_imported_variants_ambiguous() {
    expect_diag_static("module a { export enum X { V } } module b { export enum Y { V } } import a; import b; fn main() { auto x = V; }", "diag.ambiguous-name");
}

#[test]
fn conf_generic_assoc_fn_inferred() {
    expect_ok("struct W { i32 a; } fn W::put<T>(ref<W, exclusive> w, T x) { } fn main() { W w = W { .a = 1 }; W::put(&mut w, 42); }");
}

#[test]
fn conf_print_generic_not_printable() {
    expect_diag_static("import std;\nfn show<T>(T x) { printf(\"%v\", x); }\nfn main() { show(Some(1)); }", "diag.type-mismatch");
}

#[test]
fn conf_local_shadows_intrinsic() {
    expect_true("{ auto max_value = [](i32 a) { a * 2 }; max_value(4) == 8 }");
}

// ---------- Area 3: resource destruction ordering (spec/07,
// [Destroy-Composite]) ----------

#[test]
fn e2e_struct_field_destruction_is_reverse_declaration_order() {
    // [Destroy-Composite] (spec/07): same-length paths (sibling
    // fields) tie-break by *reverse* declaration/index order -- was
    // forward, confirmed by a real program whose fields' destructors
    // each write a distinguishing byte and observing the actual order
    // emitted (was 1,2,3; must be 3,2,1).
    let src = "resource struct Tagged { u8 tag; }\nfn Tagged::drop(ref<Tagged, exclusive> self) { auto buf = [self.tag]; rawptr<u8> p = reinterpret_ptr<u8>(rawptr_of(&buf)); unsafe { write(p, 1); } }\nstruct Holder { Tagged a; Tagged b; Tagged c; }\nfn main() { Holder h = Holder { .a = Tagged { .tag = 1 }, .b = Tagged { .tag = 2 }, .c = Tagged { .tag = 3 } }; }";
    let (ok, out) = run_binary_stdout(src);
    assert!(ok, "coby exited non-zero");
    assert_eq!(out, vec![3, 2, 1], "struct field destruction order wrong");
}

#[test]
fn e2e_array_element_destruction_is_reverse_index_order() {
    let src = "resource struct Tagged { u8 tag; }\nfn Tagged::drop(ref<Tagged, exclusive> self) { auto buf = [self.tag]; rawptr<u8> p = reinterpret_ptr<u8>(rawptr_of(&buf)); unsafe { write(p, 1); } }\nfn main() { array<Tagged,3> arr = [Tagged { .tag = 1 }, Tagged { .tag = 2 }, Tagged { .tag = 3 }]; }";
    let (ok, out) = run_binary_stdout(src);
    assert!(ok, "coby exited non-zero");
    assert_eq!(out, vec![3, 2, 1], "array element destruction order wrong");
}

#[test]
fn e2e_block_scope_local_destruction_already_correct_reverse_order() {
    // Confirms the pre-existing, already-correct D-0008 block-scope
    // path (a *different* code path from the struct/array fix above)
    // was not itself broken -- a negative control for the two fixes.
    let src = "resource struct Tagged { u8 tag; }\nfn Tagged::drop(ref<Tagged, exclusive> self) { auto buf = [self.tag]; rawptr<u8> p = reinterpret_ptr<u8>(rawptr_of(&buf)); unsafe { write(p, 1); } }\nfn main() { Tagged a = Tagged { .tag = 1 }; Tagged b = Tagged { .tag = 2 }; Tagged c = Tagged { .tag = 3 }; }";
    let (ok, out) = run_binary_stdout(src);
    assert!(ok, "coby exited non-zero");
    assert_eq!(out, vec![3, 2, 1], "block-scope destruction order regressed");
}

// ---------- Deep generic nesting audit (2026-09-21) -- the human owner
// asked for this checked more rigorously than the prior pass's spot
// checks. Method: same as every other audit in this file -- build a
// real program, run the real binary, check real values/bytes. One
// initially-suspected bug (Rc::get's reference still lexically live
// when the Rc it derives from is later explicitly drop()ped) turned
// out, on reduction, to be this interpreter *correctly* rejecting a
// genuinely unsound program (dropping something while a live reference
// into it is still a reachable binding) -- confirmed by scoping the
// reference out first, which fixed it. Not a bug; recorded here as a
// permanent regression test for the correct-rejection behavior, not
// just the nesting cases that should succeed.

#[test]
fn e2e_deep_generic_nesting_vec_rc_vec_option_composes_correctly() {
    let src = r#"
        fn main()
        {
            Vec<Rc<Vec<Option<i32>>>> outer = Vec::new();
            Vec<Option<i32>> inner = Vec::new();
            Vec::push(&mut inner, Some(42));
            Vec::push(&mut inner, None);
            auto rc = Rc::new(inner);
            Vec::push(&mut outer, rc);
            auto rc2 = Rc::clone(Vec::index_shared(&outer, 0));
            {
                auto v = Rc::get(&rc2);
                usize n = Vec::len(v);
                if (n != 2) { 1/(1-1); }
                match (*Vec::index_shared(v, 0))
                {
                    Some(x) : { if (x != 42) { 1/(1-1); } },
                    None : { 1/(1-1); },
                }
                match (*Vec::index_shared(v, 1))
                {
                    Some(_) : { 1/(1-1); },
                    None : {},
                }
            }
            drop(rc2);
        }
    "#;
    expect_ok(src);
}

#[test]
fn e2e_dropping_rc_while_a_get_reference_is_still_in_scope_is_correctly_rejected() {
    // The reduction of the case above: this is NOT a bug -- v derived
    // from rc2 via Rc::get is still a live, reachable binding when
    // drop(rc2) runs, which would deallocate memory v still names.
    // Confirms the aliasing check catches this even though v is never
    // actually used again after the match (no last-use/NLL-style
    // liveness narrowing -- a binding is live for its whole lexical
    // scope, a conservative and sound choice, not an imprecision bug).
    // The diagnostic is `[Destroy-Not-Solitary]`'s own
    // (`diag.destroy-while-aliased`, as `conf.destroy-while-borrowed-
    // rejected` records), not the generic `diag.aliasing-conflict`.
    expect_diag(
        r#"
        fn main()
        {
            Vec<Option<i32>> inner = Vec::new();
            Vec::push(&mut inner, Some(42));
            auto rc = Rc::new(inner);
            auto v = Rc::get(&rc);
            match (*Vec::index_shared(v, 0))
            {
                Some(x) : { if (x != 42) { 1/(1-1); } },
                None : { 1/(1-1); },
            }
            drop(rc);
        }
        "#,
        "diag.destroy-while-aliased",
    );
}

#[test]
fn e2e_four_level_nested_vec_retrieves_correct_value_and_drops_cleanly() {
    expect_ok(
        r#"
        fn main()
        {
            Vec<Vec<Vec<Vec<i32>>>> v0 = Vec::new();
            Vec<Vec<Vec<i32>>> v1 = Vec::new();
            Vec<Vec<i32>> v2 = Vec::new();
            Vec<i32> v3 = Vec::new();
            Vec::push(&mut v3, 99);
            Vec::push(&mut v2, v3);
            Vec::push(&mut v1, v2);
            Vec::push(&mut v0, v1);
            {
                auto r1 = Vec::index_shared(&v0, 0);
                auto r2 = Vec::index_shared(r1, 0);
                auto r3 = Vec::index_shared(r2, 0);
                auto r4 = Vec::index_shared(r3, 0);
                if (*r4 != 99) { 1/(1-1); }
            }
        }
        "#,
    );
}

#[test]
fn e2e_generic_function_calling_generic_function_with_resource_type_arg() {
    expect_ok(
        r#"
        fn wrap<T>(T x) : Vec<T>
        {
            Vec<T> v = Vec::new();
            Vec::push(&mut v, x);
            v
        }
        fn double_wrap<T>(T x) : Vec<Vec<T>>
        {
            Vec<Vec<T>> outer = Vec::new();
            Vec::push(&mut outer, wrap(x));
            outer
        }
        fn main()
        {
            Vec<i32> inner = Vec::new();
            Vec::push(&mut inner, 5);
            auto result = double_wrap(inner);
            auto r1 = Vec::index_shared(&result, 0);
            auto r2 = Vec::index_shared(r1, 0);
            auto r3 = Vec::index_shared(r2, 0);
            if (*r3 != 5) { 1/(1-1); }
        }
        "#,
    );
}

#[test]
fn e2e_multi_param_generic_struct_nested_layout_and_bytes_correct() {
    let src = "struct Pair<A, B> { A a; B b; }\nfn main() { Pair<i32, Pair<i32, i32>> p = Pair { .a = 1, .b = Pair { .a = 2, .b = 3 } }; usize sz = sizeof<Pair<i32, Pair<i32, i32>>>(); rawptr<u8> rp = reinterpret_ptr<u8>(rawptr_of(&p)); isize n = unsafe { write(rp, sz) }; }";
    let (ok, out) = run_binary_stdout(src);
    assert!(ok, "coby exited non-zero");
    assert_eq!(out, vec![1, 0, 0, 0, 2, 0, 0, 0, 3, 0, 0, 0], "nested generic struct layout/bytes wrong");
}

#[test]
fn e2e_resource_type_param_in_generic_struct_array_field_destroys_in_reverse_order() {
    // Re-verifies (independently) a prior pass's claim that a resource
    // type parameter nested inside an array field of a generic struct
    // works -- extended here to also check destruction *order* through
    // generic instantiation, not just construction/access.
    let src = r#"
        resource struct Tagged { u8 tag; }
        fn Tagged::drop(ref<Tagged, exclusive> self)
        {
            auto b = [self.tag];
            rawptr<u8> p = reinterpret_ptr<u8>(rawptr_of(&b));
            unsafe { write(p, 1); }
        }
        resource struct Box3<T> { array<T,3> items; }
        fn main()
        {
            Box3<Tagged> b = Box3 { .items = [Tagged { .tag = 88 }, Tagged { .tag = 89 }, Tagged { .tag = 90 }] };
        }
    "#;
    let (ok, out) = run_binary_stdout(src);
    assert!(ok, "coby exited non-zero");
    assert_eq!(out, vec![90, 89, 88], "generic array-field resource destruction order regressed");
}

// ---------- diag.use-of-uninitialized (spec/11 rule.init.definite-
// assignment) -- another real gap the file-based conformance suite
// found: [Let-Uninit] (`τ x;`, no initializer) was entirely
// unimplemented; reading such a binding hit a type-mismatch by
// accident (the placeholder Value::Unit failing arithmetic), not the
// registered diagnostic.

#[test]
fn conf_use_of_uninitialized_binding_rejected() {
    expect_diag(
        "fn main() { i32 x; i32 y = x + 1; }",
        "diag.use-of-uninitialized",
    );
}

#[test]
fn conf_uninitialized_binding_then_write_then_read_ok() {
    expect_true("{ i32 x; x = 5; x == 5 }");
}

// ---------- diag.break-outside-loop (spec/14 rule.control.while) --
// another real gap the file-based conformance suite found: `break`
// outside any `while` fell through to a generic internal
// "unexpected control flow at top level" message, not the registered
// diagnostic. (`diag.return-outside-fn`'s sibling case is confirmed
// *not* a bug: `return` outside a function body is a parse error in
// this grammar -- "expected item, found Return" -- never reaching
// semantic analysis at all, so there is nothing for a separate check
// to catch; see impl/STATUS.md.)

#[test]
fn conf_break_outside_loop_rejected() {
    expect_diag_static("fn main()\n{\nbreak;\n}\n", "diag.break-outside-loop");
}

#[test]
fn conf_break_inside_loop_ok() {
    expect_true(
        "{ i32 i = 0; while (i < 5) { if (i == 3) { break; } i = i + 1; } i == 3 }",
    );
}

// ---------- diag.propagate-outside-fallible-context (spec/18
// [Propagate-Err-Mismatch]) -- another real gap the file-based
// conformance suite found: `?` in a function whose return type isn't
// `Result<_, E>` with a matching `E` was silently accepted.

#[test]
fn conf_propagate_wrong_return_type_rejected() {
    expect_diag_static(
        "fn f() : i32 { Result<i32, i32> r = Ok(1); r? }\nfn main() { f(); }",
        "diag.propagate-outside-fallible-context",
    );
}

#[test]
fn conf_propagate_mismatched_error_type_rejected() {
    expect_diag_static(
        "fn f() : Result<i32, bool> { Result<i32, i32> r = Ok(1); auto v = r?; Ok(v) }\nfn main() { f(); }",
        "diag.propagate-outside-fallible-context",
    );
}

#[test]
fn conf_propagate_matching_result_return_type_ok() {
    expect_ok(
        "fn f() : Result<i32, i32> { Result<i32, i32> r = Ok(1); auto v = r?; Ok(v + 1) }\nfn main() { match (f()) { Ok(v) : { if (v != 2) { 1/(1-1); } }, Err(_) : { 1/(1-1); }, } }",
    );
}

// ---------- diag.spawn-borrow-closure (spec/19 rule.conc.spawn) --
// another real gap the file-based conformance suite found: `spawn`
// accepted a borrowing (non-`move`) closure without rejection, which
// could let a spawned thread outlive data it only holds a reference
// to. Found and fixed alongside a genuine host-crash bug: calling
// `spawn` with an argument count mismatched to the closure's own
// parameter count was a Rust-level panic (index out of bounds), not a
// CobaltC-level diagnostic.

#[test]
fn conf_spawn_borrow_closure_rejected() {
    expect_diag(
        "fn main() { i32 x = 5; auto h = spawn([x](i32 n) { n + x }, 1); join(h); }",
        "diag.spawn-borrow-closure",
    );
}

#[test]
fn conf_spawn_move_closure_ok() {
    // A top-level `if (join(h) != ..)` statement, matching every other
    // spawn/join test's own shape (`conf_spawn_join_value` etc.) --
    // NOT `expect_true`'s block-as-if-condition wrapping, which nests
    // `join` deep inside an expression tree and hits the already-
    // documented resume-mechanism limitation (`impl/STATUS.md`'s
    // "How blocking works, and its one real limitation") as a genuine
    // hang, found the hard way while adding this very test.
    expect_ok(
        "fn main() { i32 x = 5; auto h = spawn(move [x](i32 n) { n + x }, 1); if (join(h) != 6) { 1/(1-1); } }",
    );
}

#[test]
fn conf_spawn_closure_arg_count_mismatch_does_not_panic_host() {
    // Regression guard for the panic found alongside this fix -- must
    // be a real diagnostic, not a Rust-level crash (exit code != 101).
    expect_diag(
        "fn main() { i32 x = 5; auto h = spawn(move [x](i32 n) { n + x }); join(h); }",
        "diag.type-mismatch",
    );
}

// ---------- diag.extern-non-ffi-type (spec/20 rule.trust.extern-call
// [Extern-Non-Ffi-Type]) -- another real gap the file-based
// conformance suite found: an `extern fn` declared with a
// resource-typed (non-FfiType) parameter was silently accepted.
// Checked unconditionally for every declared extern, not just ones
// actually called -- [Extern-Non-Ffi-Type] is disposition: rejected
// on the declaration itself.

#[test]
fn conf_extern_non_ffi_type_param_rejected() {
    // covers: conf.extern-non-ffi-type-rejected
    expect_diag_static("extern fn g(Vec<i32> v);\nfn main() {}", "diag.extern-non-ffi-type");
}

#[test]
fn conf_extern_ffi_type_params_ok() {
    expect_ok("extern fn h(i32 a, rawptr<u8> b) : i32;\nfn main() {}");
}

// ---------- Per-access ancestor chains (spec/08 `ancestors`) -- a real
// soundness bug in the dynamic `clash` backstop. Dereferencing a
// reference used to record its token in a persistent `place_excludes`
// table keyed by the (obj, path) it reached, and every later access to
// that place by *anyone* then treated the token as an ancestor and
// skipped it. So `auto r = &x; x = 2;` was rejected, but `auto r = &x;
// i32 a = *r; x = 2;` was accepted -- and `r` observed the mutation.
// The chain now travels with the individual `EvalResult::Place` and is
// never stored against the place. `[Store-Binding-Place-Copy]` also now
// copies through `[Read]` (clash-checked) instead of a raw peek.

#[test]
fn conf_shared_ref_used_then_owner_write_rejected() {
    expect_diag(
        "fn main() { i32 x = 1; auto r = &x; i32 before = *r; x = 2; i32 after = *r; }",
        "diag.aliasing-conflict",
    );
}

#[test]
fn conf_shared_ref_used_twice_then_owner_write_rejected() {
    // Two shared borrows, both used: neither use may exempt the other.
    expect_diag(
        "fn main() { i32 x = 1; auto r1 = &x; auto r2 = &x; i32 a = *r1; i32 b = *r2; x = 2; }",
        "diag.aliasing-conflict",
    );
}

#[test]
fn conf_exclusive_field_borrow_used_then_owner_write_rejected() {
    expect_diag_with(
        "struct Pair { i32 a; i32 b; }",
        "Pair p = Pair { .a = 1, .b = 2 }; auto ra = &mut p.a; auto rb = &mut p.b; *ra = 10; *rb = 20; p.b = 30;",
        "diag.aliasing-conflict",
    );
}

#[test]
fn conf_shared_ref_used_then_destroy_rejected() {
    // `solitary` must not be fooled either: the destroy itself is
    // refused, not merely the later use of the dangling reference.
    expect_diag(
        "fn main() { Vec<i32> v = Vec::new(); Vec::push(&mut v, 1); auto r = &v; usize n = Vec::len(r); drop(v); usize m = Vec::len(r); }",
        "diag.destroy-while-aliased",
    );
}

#[test]
fn conf_shared_ref_used_then_move_rejected() {
    expect_diag_with(
        "resource struct R { i32 v; } fn takes(R x) {}",
        "R r = R { .v = 1 }; auto rr = &r; i32 seen = rr.v; takes(r);",
        "diag.move-while-aliased",
    );
}

#[test]
fn conf_owner_read_while_exclusive_child_used_rejected() {
    // spec/conformance.md, conf.reborrow-ok's derivation: "reading `x`
    // itself here would be [Read-Conflict], since a_r1' is a live
    // exclusive descendant". Both the declaration-initialiser read
    // ([Store-Binding-Place-Copy]) and an operand read.
    expect_diag("fn main() { i32 x = 1; auto r = &mut x; *r = 5; i32 y = x; }", "diag.aliasing-conflict");
    expect_diag("fn main() { i32 x = 1; auto r = &mut x; *r = 5; i32 y = x + 1; }", "diag.aliasing-conflict");
}

#[test]
fn conf_owner_read_while_shared_child_used_ok() {
    // conf.owner-read-while-shared-ok, with the reference used first.
    expect_true("{ i32 x = 1; auto r = &x; i32 a = *r; x + *r + a == 3 }");
}

#[test]
fn conf_ref_used_then_scope_end_frees_owner_ok() {
    expect_true("{ i32 x = 1; { auto r = &x; i32 a = *r; i32 b = *r; } x = 2; { auto w = &mut x; *w = *w + 1; *w = *w + 1; } x == 4 }");
}

#[test]
fn conf_reborrow_through_used_parameter_still_ok() {
    // The ancestor exemption that `token_ancestors` provides is
    // unaffected: a sub-borrow formed by dereferencing an exclusive
    // reference parameter is still its descendant.
    expect_true_with(
        "fn read_len(ref<Vec<i32>, exclusive> v) : usize { usize k = Vec::len(&*v); auto shared_view = &*v; Vec::len(shared_view) + k }",
        "{ Vec<i32> v = Vec::new(); Vec::push(&mut v, 1); read_len(&mut v) == 2 }",
    );
}

// ---------- Borrow-capture closures: captured names resolve *through*
// the capture reference (spec/15 [Closure-Call]: `x_i` means
// `*self.f_i`). `bind_closure_captures` used to alias the captured name
// to the referent directly, so the body's own write to an exclusively
// captured variable found the closure's capture reference as a live
// non-ancestor exclusive path and was rejected. Now the name is bound
// to a per-call cell holding the reference, and `eval_path` carries the
// capture token as the access's ancestor.

#[test]
fn conf_closure_exclusive_capture_write_ok() {
    expect_true("{ i32 total = 0; { auto add = [total](i32 x) { total = total + x; }; add(5); add(7); } total == 12 }");
    expect_true("{ Vec<i32> v = Vec::new(); { auto p = [v](i32 x) { Vec::push(&mut v, x); }; p(1); p(2); } Vec::len(&v) == 2 }");
}

#[test]
fn conf_closure_exclusive_capture_blocks_owner_read_rejected() {
    expect_diag_body("i32 total = 0; auto add = [total](i32 x) { total = total + x; }; add(5); i32 seen = total;", "diag.aliasing-conflict");
}

#[test]
fn conf_closure_shared_capture_owner_read_ok_write_rejected() {
    expect_true("{ i32 x = 1; auto f = [x]() { x }; x + f() == 2 }");
    expect_diag_body("i32 x = 1; auto f = [x]() { x }; i32 y = x + f(); x = 2;", "diag.aliasing-conflict");
}

#[test]
fn conf_closure_exclusive_capture_blocks_second_capture_rejected() {
    expect_diag_body(
        "Vec<i32> v = Vec::new(); auto p = [v](i32 x) { Vec::push(&mut v, x); }; p(1); auto r = [v]() { Vec::len(&v) }; r();",
        "diag.aliasing-conflict",
    );
}

#[test]
fn conf_closure_nested_capture_reborrows_ok() {
    expect_true("{ i32 t = 0; { auto outer = [t]() { auto inner = [t]() { t = t + 1; }; inner(); inner(); }; outer(); outer(); } t == 4 }");
}

#[test]
fn conf_closure_nested_exclusive_from_shared_capture_rejected() {
    // Capture mode is derived from the whole body text, nested literals
    // included, so an inner write makes the outer capture exclusive too
    // and a nested write to a plain local is fine. What stays rejected
    // is writing through a captured *shared reference*: the write
    // crosses `ref<i32, shared>`, capture cells or not.
    expect_diag_with(
        "fn f(ref<i32, shared> r) { auto g = [r]() { auto h = [r]() { *r = 5; }; h(); }; g(); }",
        "i32 x = 1; f(&x);",
        "diag.write-through-shared",
    );
}

#[test]
fn conf_closure_move_captured_reference_ok() {
    expect_true("{ i32 x = 1; ref<i32, shared> r = &x; auto f = move [r]() { *r }; f() == 1 }");
}

#[test]
fn conf_closure_capture_released_after_call_ok() {
    // The per-call capture cell must not outlive the call: the copy of
    // the capture reference it holds would otherwise keep conflicting
    // with the owner after the closure itself is gone.
    expect_true("{ i32 x = 1; { auto f = [x]() { x + 1 }; f(); f(); } x = 2; x == 2 }");
}

// ---------- rule.control.flow-analysis (spec/14 §6) as specified: a
// forward dataflow over the syntactic CFG with `valid`/`init`/`deriv`
// in {T, F, ?}, joins at merges, a fixed point around `while`, block-
// exit transfers, and the discharge table at every reliance point. The
// tests below pin the *phase*: a refuted instance is rejected by the
// static pass alone; an `unknown` one is accepted statically and left
// to the dynamic check, which then faults.

fn expect_diag_dynamic_only(src: &str, diag: &str) {
    if let Err(d) = check_source_static(src) {
        panic!("expected the static pass to leave `{}` to the dynamic check, but it rejected `{}` for:\n{}", diag, d, src);
    }
    expect_diag(src, diag);
}

#[test]
fn flow_stale_binding_after_visible_move_is_static() {
    expect_diag_static("fn main() { Vec<i32> v = Vec::new(); auto w = v; Vec::push(&mut v, 1); }", "diag.stale-binding");
    expect_diag_static("fn main() { Vec<i32> v = Vec::new(); drop(v); drop(v); }", "diag.stale-binding");
    expect_diag_static(
        "struct P { Vec<i32> a; } fn main() { Vec<i32> v = Vec::new(); auto p = P { .a = v }; usize n = Vec::len(&v); }",
        "diag.stale-binding",
    );
    expect_diag_static(
        "fn main() { Vec<i32> v = Vec::new(); auto f = move [v]() { Vec::push(&mut v, 1); }; f(); Vec::push(&mut v, 2); }",
        "diag.stale-binding",
    );
}

#[test]
fn flow_definite_assignment_is_static_and_unknown_rejects() {
    // covers: conf.partial-init-rejected
    expect_diag_static("fn main() { bool c = true; i32 x; if (c) { x = 1; } i32 y = x; }", "diag.use-of-uninitialized");
    expect_diag_static("struct P { i32 a; i32 b; } fn main() { P p; p.a = 1; }", "diag.use-of-uninitialized");
    expect_diag_static("fn main() { i32 x; i32 i = 0; while (i < 3) { x = i; i = i + 1; } i32 y = x; }", "diag.use-of-uninitialized");
    expect_true("{ bool c = true; i32 x; if (c) { x = 1; } else { x = 2; } x == 1 }");
}

#[test]
fn flow_alias_conflicts_against_visible_borrows_are_static() {
    expect_diag_static("fn main() { i32 x = 1; auto r = &x; x = 2; }", "diag.aliasing-conflict");
    expect_diag_static("fn main() { i32 x = 1; auto r = &mut x; i32 y = x; }", "diag.aliasing-conflict");
    expect_diag_static("fn main() { i32 x = 1; auto r1 = &x; auto r2 = &mut x; }", "diag.aliasing-conflict");
    expect_diag_static(
        "struct P { i32 a; i32 b; } fn main() { auto p = P { .a = 1, .b = 2 }; auto r = &mut p.a; p.a = 5; }",
        "diag.aliasing-conflict",
    );
    expect_diag_static(
        "fn main() { Vec<i32> v = Vec::new(); Vec::push(&mut v, 1); auto r = Vec::index_shared(&v, 0); Vec::push(&mut v, 2); i32 y = *r; }",
        "diag.aliasing-conflict",
    );
    expect_diag_static("fn main() { Vec<i32> v = Vec::new(); auto r = &v; drop(v); }", "diag.destroy-while-aliased");
    expect_diag_static("fn main() { Vec<i32> v = Vec::new(); auto r = &v; auto w = v; }", "diag.move-while-aliased");
    expect_diag_static("fn main() { Vec<i32> v = Vec::new(); v = Vec::new(); }", "diag.overwrite-of-live-resource");
}

#[test]
fn flow_read_of_resource_and_move_out_of_field_are_static() {
    // covers: conf.read-of-resource-rejected, conf.move-out-of-field-rejected
    expect_diag_static("fn main() { Vec<i32> v = Vec::new(); Vec<i32> w = Vec::new(); auto s = v == w; }", "diag.read-of-resource");
    expect_diag_static("struct P { Vec<i32> a; } fn main() { auto p = P { .a = Vec::new() }; auto q = p.a; }", "diag.move-out-of-field");
}

#[test]
fn flow_literal_index_out_of_bounds_is_static() {
    expect_diag_static("fn main() { array<i32, 3> a = [1, 2, 3]; i32 y = a[3]; }", "diag.index-out-of-bounds");
    expect_true("{ array<i32, 3> a = [1, 2, 3]; a[2] == 3 }");
}

#[test]
fn flow_merge_and_loop_unknowns_are_left_to_the_dynamic_check() {
    // valid(v) joins T and F to ? at the merge / at the loop head.
    expect_diag_dynamic_only("fn main() { bool c = true; Vec<i32> v = Vec::new(); if (c) { auto w = v; } Vec::push(&mut v, 1); }", "diag.stale-binding");
    expect_diag_dynamic_only(
        "fn takes(Vec<i32> v) {} fn main() { Vec<i32> v = Vec::new(); i32 i = 0; while (i < 3) { takes(v); i = i + 1; } }",
        "diag.stale-binding",
    );
    // An access rooted at a dereference is never T and never F.
    expect_diag_dynamic_only("fn f(ref<i32, exclusive> r) { auto a = &*r; auto b = &mut *r; *b = 1; i32 y = *a; } fn main() { i32 x = 1; f(&mut x); }", "diag.aliasing-conflict");
}

#[test]
fn flow_block_exit_and_shadowing_ok() {
    expect_true("{ i32 x = 1; { auto r = &x; i32 a = *r; } x = 2; { auto r1 = &mut x; { auto r2 = &*r1; i32 b = *r2; } *r1 = 3; } x == 3 }");
    expect_true_with(
        "fn takes(Vec<i32> v) {}",
        "{ Vec<i32> v = Vec::new(); { Vec<i32> v = Vec::new(); takes(v); } Vec::push(&mut v, 1); Vec::len(&v) == 1 }",
    );
    expect_true_with(
        "fn takes(Vec<i32> v) {}",
        "{ i32 i = 0; while (i < 3) { Vec<i32> v = Vec::new(); takes(v); i = i + 1; } i == 3 }",
    );
    expect_true("{ i32 x = 1; auto r = &x; x + *r == 2 }");
    expect_true_with(
        "struct P { i32 a; i32 b; }",
        "{ P p = P { .a = 1, .b = 2 }; auto ra = &mut p.a; auto rb = &mut p.b; *ra = 10; *rb = 20; *ra + *rb == 30 }",
    );
}

// ---------- rule.temporal.ref-escape [Ref-Escape-Rejected] and
// rule.temporal.elision [Call-Multi-Ref-Return-Rejected] (spec/10):
// the two static rules that had no implementation at all.

#[test]
fn escape_return_of_local_borrow_rejected() {
    // conf.reference-escape-rejected: the zero-parameter shape reports
    // the body's own escape, not the declaration-level elision rule.
    expect_diag_static("fn f() : ref<i32, shared> { i32 x = 1; &x } fn main() { f(); }", "diag.reference-escapes-scope");
    expect_diag_static("fn f(i32 x) : ref<i32, shared> { &x } fn main() { i32 a = 1; f(a); }", "diag.reference-escapes-scope");
}

#[test]
fn escape_multi_and_zero_ref_param_returning_ref_rejected() {
    // conf.elision-multi-param-rejected
    expect_diag_static(
        "fn pick(ref<i32, shared> p1, ref<i32, shared> p2) : ref<i32, shared> { p1 } fn main() {}",
        "diag.lifetime-elision-ambiguous",
    );
    // Declaration-level: rejected even though never called.
    expect_diag_static("fn f() : ref<i32, shared> { i32 x = 1; auto r = &x; r } fn main() {}", "diag.lifetime-elision-ambiguous");
}

#[test]
fn escape_into_enclosing_block_rejected() {
    expect_diag_static(
        "fn main() { i32 outer = 0; ref<i32, shared> r = &outer; { i32 local = 5; r = &local; } i32 y = *r; }",
        "diag.reference-escapes-scope",
    );
    expect_diag_static("fn main() { auto r = { i32 y = 2; &y }; }", "diag.reference-escapes-scope");
    expect_diag_static(
        "struct H { ref<i32, shared> r; } fn main() { i32 x0 = 0; H h = H { .r = &x0 }; { i32 x = 1; h = H { .r = &x }; } }",
        "diag.reference-escapes-scope",
    );
}

#[test]
fn escape_single_ref_param_and_same_block_stores_ok() {
    // conf.elision-single-param-ok and friends: the exempted shapes.
    expect_true_with(
        "struct Pair { i32 a; i32 b; } fn first(ref<Pair, shared> p) : ref<i32, shared> { &p.a }",
        "{ Pair pr = Pair { .a = 1, .b = 2 }; auto y = first(&pr); *y == 1 }",
    );
    expect_true_with(
        "struct H { ref<i32, shared> r; }",
        "{ i32 x = 1; auto h = H { .r = &x }; *h.r == 1 }",
    );
    expect_true_with(
        "fn id(ref<i32, shared> p) : ref<i32, shared> { p }",
        "{ i32 x = 1; auto y = id(&x); { i32 z = 2; auto y2 = &*y; if (*y2 + z != 3) { 1 / (1 - 1); } } *y == 1 }",
    );
}

// ---------- The remaining static well-formedness rules, now decided
// by the static pass: [Generic-Call-Uninferable], rule.type.kind's
// bare-T restriction (checked once per generic body with T opaque),
// [Ref-Form-Not-Place]/[Ref-Form-Temporary], [Borrow-Exceeds-Source]/
// [Write-Not-Exclusive], destructor well-formedness, spawn's callee
// rule; and fn items as first-class values ([T-Item]).

#[test]
fn static_generic_call_uninferable_rejected() {
    expect_diag_static("fn main() { auto bad = Vec::new(); }", "diag.cannot-infer-type-parameter");
    expect_diag_static("fn make<T>() : i32 { 0 } fn main() { i32 x = make(); }", "diag.cannot-infer-type-parameter");
    // `Mutex::new<T>` is a generic call too: with `auto`, nothing fixes
    // its argument's own T.
    expect_diag_static("fn main() { auto m = Mutex::new(Vec::new()); }", "diag.cannot-infer-type-parameter");
    // Every inference route still works, including a nested generic
    // call whose T is only fixed by the enclosing call's other shapes.
    expect_ok_with(
        "struct Box<T> { T value; } fn boxed<T>(T x) : Box<T> { Box { .value = x } } fn id<T>(T x) : T { x }",
        "Vec<i32> a = Vec::new(); i64 w = id<i64>(3); auto s = id(5); Box<Vec<u8>> c = boxed(Vec::new()); Vec<Vec<i32>> vv = Vec::new(); Vec::push(&mut vv, Vec::new()); mutex<Vec<u8>> m = Mutex::new(Vec::new()); mutex<i64> big = Mutex::new(5000000000); Box<i32> b = Box { .value = id(7) };",
    );
}

#[test]
fn static_unbounded_type_parameter_rejected() {
    // conf.unbounded-type-parameter-rejected; rejected at the
    // declaration, so also for an uncalled generic.
    expect_diag_static("fn add<T>(T a, T b) : T { a + b } fn main() { i32 x = add(1, 2); }", "diag.unbounded-type-parameter");
    expect_diag_static("fn f<T>(T a) : bool { a == a } fn main() {}", "diag.unbounded-type-parameter");
    expect_diag_static("fn g<T>(T a) : i32 { a.x } fn main() {}", "diag.unbounded-type-parameter");
    // What a bare T may do: be stored, passed, referenced, moved.
    expect_ok_with(
        "struct W<T> { T v; } fn wrap<T>(T x) : W<T> { W { .v = x } } fn pass<T>(T x) : T { auto r = &x; wrap(x).v }",
        "i32 y = pass(4); if (y != 4) { 1/(1-1); }",
    );
}

#[test]
fn static_borrow_of_non_place_and_temporary_rejected() {
    expect_diag_static("fn make() : i32 { 5 } fn main() { auto r = &make(); }", "diag.borrow-of-non-place");
    expect_diag_static("fn make() : Vec<i32> { Vec::new() } fn main() { auto r = &make(); }", "diag.borrow-of-non-place");
    expect_diag_static("fn main() { auto r = &5; }", "diag.borrow-of-non-place");
    // conf.borrow-of-temporary-rejected
    expect_diag_static("struct P { i32 a; } fn main() { auto r = &P { .a = 1 }.a; }", "diag.borrow-of-temporary");
    expect_diag_static("struct P { i32 a; } fn make() : P { P { .a = 1 } } fn main() { auto r = &make().a; }", "diag.borrow-of-temporary");
}

#[test]
fn static_mode_monotonicity_rejected() {
    // conf.exclusive-from-shared-rejected, conf.write-through-shared-rejected
    expect_diag_static("struct P { i32 a; } fn g(ref<P, shared> p) { auto w = &mut p.a; } fn main() { P p = P { .a = 1 }; g(&p); }", "diag.borrow-exceeds-source");
    expect_diag_static("fn g(ref<i32, shared> p) { *p = 1; } fn main() { i32 x = 1; g(&x); }", "diag.write-through-shared");
    expect_diag_static("struct P { i32 a; } fn g(ref<P, shared> p) { p.a = 1; } fn main() { P p = P { .a = 1 }; g(&p); }", "diag.write-through-shared");
    expect_diag_static("fn tamper(ref<i32, shared> r) : i32 { auto rm = &mut *r; *rm = 99; *rm } fn main() { i32 x = 5; tamper(&x); }", "diag.borrow-exceeds-source");
}

#[test]
fn static_destructor_rules_rejected() {
    expect_diag_static("resource struct R { i32 v; } fn R::drop(R self) {} fn main() {}", "diag.bad-destructor-signature");
    expect_diag_static("resource struct R { i32 v; } fn R::drop(ref<R, exclusive> self) : i32 { 0 } fn main() {}", "diag.bad-destructor-signature");
    expect_diag_static(
        "resource struct R { i32 v; } fn R::drop(ref<R, exclusive> self) {} fn main() { R r = R { .v = 1 }; R::drop(&mut r); }",
        "diag.direct-destructor-call",
    );
}

#[test]
fn static_spawn_borrow_closure_rejected() {
    // conf.spawn-borrow-closure-rejected, as a literal and through a binding.
    expect_diag_static("fn main() { i32 x = 1; auto h = spawn([x]() { x + 1 }); join(h); }", "diag.spawn-borrow-closure");
    expect_diag_static("fn main() { i32 x = 1; auto f = [x]() { x + 1 }; auto h = spawn(f); join(h); }", "diag.spawn-borrow-closure");
}

#[test]
fn fn_item_as_first_class_value_ok() {
    expect_true_with(
        "fn apply(fn(i32) : i32 f, i32 x) : i32 { f(x) } fn double(i32 x) : i32 { x * 2 }",
        "{ fn(i32) : i32 g = double; apply(double, 4) + g(5) == 18 }",
    );
}

// ---------- Grammar: spec/22 §2 disambiguation (1) decided by a
// speculative type-list parse; qualified type names in type position
// and as struct literals for module-declared types; `Vec<i32>::new()`.

#[test]
fn grammar_comparison_versus_type_args() {
    expect_true("{ u8 b = 50; i32 a = 1; i32 c = 2; !(b < 48 || b > 57) && (a < c && c > a) && ((a < c) || (c > a)) }");
    expect_true("{ Vec<Vec<i32>> vv = Vec::new(); auto v = Vec<i32>::new(); Vec::push(&mut v, 1); Vec::push(&mut vv, v); Vec::len(&vv) == 1 }");
}

#[test]
fn grammar_module_declared_types() {
    expect_true_with(
        "module geom { export struct Point { export i32 x; export i32 y; } export enum Shape { Dot, Circle(i32) } export fn make(i32 x) : Point { Point { .x = x, .y = 0 } } export fn radius(Shape s) : i32 { match (s) { Dot : 0, Circle(r) : r, } } }",
        "{ geom::Point p = geom::Point { .x = 3, .y = 4 }; auto q = geom::make(1); geom::Shape s = geom::Shape::Circle(2); p.x + p.y + q.x + geom::radius(s) == 10 }",
    );
    expect_true_with(
        "module geom { export struct Point { export i32 x; export i32 y; } } import geom::Point; fn dist(ref<Point, shared> p) : i32 { p.x + p.y }",
        "{ Point p = Point { .x = 3, .y = 4 }; dist(&p) == 7 }",
    );
}

// ---------- Dynamic fidelity: a local's own cells behind rawptr_of,
// drop through a reference, tail-position expected types, and
// per-thread destroy authority.

#[test]
fn raw_write_through_rawptr_of_local_is_visible() {
    expect_true("{ i32 x = 7; auto p = rawptr_of(&mut x); unsafe { *p = 9; } bool a = x == 9; x = 11; bool b = unsafe { *p == 11 }; a && b }");
    expect_true_with("struct P { i32 a; i32 b; }", "{ P s = P { .a = 1, .b = 2 }; auto q = rawptr_of(&mut s.b); unsafe { *q = 20; } s.b == 20 && s.a == 1 }");
}

#[test]
fn drop_through_reference_rejected() {
    expect_diag_body("Vec<i32> v = Vec::new(); auto r = &mut v; drop(*r);", "diag.destroy-while-aliased");
}

#[test]
fn tail_literal_takes_expected_type() {
    // Previously pinned as a known limitation for the implicit tail.
    expect_true_with("fn g() : i64 { 5000000000 } fn h(bool c) : i64 { if (c) { 6000000000 } else { 7000000000 } }", "g() == 5000000000 && h(true) == 6000000000 && h(false) == 7000000000");
    expect_true_with("fn k(Option<i32> o) : i64 { match (o) { Some(_) : 8000000000, None : 9000000000, } }", "{ i64 b = { 10000000000 }; k(None) == 9000000000 && b == 10000000000 }");
}

#[test]
fn thread_resource_authority_rekeyed() {
    // Top-level statements only (a `join` nested inside an expression
    // tree cannot be resumed by the current blocking mechanism).
    expect_ok_with(
        "fn make() : Vec<i32> { Vec<i32> v = Vec::new(); Vec::push(&mut v, 5); v } fn takes(Vec<i32> v) : usize { Vec::len(&v) }",
        "auto h1 = spawn(make); auto v1 = join(h1); usize n = Vec::len(&v1); drop(v1); Vec<i32> w = Vec::new(); Vec::push(&mut w, 3); auto h2 = spawn(takes, w); usize m = join(h2); if (n + m != 2) { 1/(1-1); }",
    );
}

// ---------- Blocking at any depth (the GIL): join/lock/handle sweeps
// wait in place, so a wait nested inside a helper call or inside an
// expression works, an unjoined handle's sweep really waits, and
// threads interleave at statement granularity.

#[test]
fn blocking_join_nested_in_call_and_expression() {
    expect_true_with(
        "fn work(i32 n) : i32 { n * 2 } fn helper() : i32 { i32 before = 1; auto h = spawn(work, 21); i32 r = join(h); before + r }",
        "{ auto h = spawn(work, 5); helper() + helper() + join(h) == 96 }",
    );
}

#[test]
fn blocking_handle_sweep_waits_for_running_thread() {
    expect_true_with(
        "fn slow(ref<mutex<i32>, shared> m) : i32 { i32 i = 0; while (i < 500) { auto g = lock(m); *g = *g + 1; i = i + 1; } 0 }",
        "{ auto m = Mutex::new(0); { auto h = spawn(slow, &m); } *lock(&m) == 500 }",
    );
}

#[test]
fn blocking_threads_interleave_under_mutex() {
    expect_true_with(
        "fn count(ref<mutex<i32>, shared> m) : i32 { i32 i = 0; while (i < 50) { { auto g = lock(m); *g = *g + 1; } i = i + 1; } 0 }",
        "{ auto m = Mutex::new(0); auto h = spawn(count, &m); i32 i = 0; while (i < 50) { { auto g = lock(&m); *g = *g + 1; } i = i + 1; } join(h); *lock(&m) == 100 }",
    );
}

// ---------- Remaining spec/conformance.md cases, transcribed so that
// `tests/conf_coverage.rs` finds every `conf.*` id referenced somewhere
// in the suites.

#[test]
fn conf_closure_capture_list_mismatch_rejected() {
    expect_diag_static("fn main() { i32 x = 10; auto f = [](i32 y) { x + y }; f(1); }", "diag.capture-list-mismatch");
}

#[test]
fn conf_match_non_exhaustive_rejected() {
    expect_diag_static(
        "enum Sign { Pos, Neg, Zero } fn describe(Sign s) : i32 { match (s) { Pos : 1, Neg : -1, } } fn main() { describe(Zero); }",
        "diag.non-exhaustive-match",
    );
}

#[test]
fn conf_propagate_mismatch_rejected() {
    expect_diag_static("fn f() : i32 { Result<i32, i32> r = Ok(1); r? } fn main() { f(); }", "diag.propagate-outside-fallible-context");
}

#[test]
fn conf_cross_type_cmp_rejected() {
    expect_diag_static("fn main() { bool b = 1: i32 < 1: i64; }", "diag.type-mismatch");
}

#[test]
fn conf_let_synthesis() {
    // `auto` takes the initializer's synthesized type.
    expect_true("{ auto x = 3: i64 + 4: i64; x == 7: i64 }");
}

#[test]
fn conf_match_through_ref_rejected() {
    // [Match-Move-Through-Ref]: a resource payload cannot be bound
    // through a reference or a projection.
    expect_diag_static(
        "enum E { Has(Vec<i32>), Empty } fn f(ref<E, shared> e) : usize { match (*e) { Has(x) : Vec::len(&x), Empty : 0, } } fn main() { auto e = E::Has(Vec::new()); f(&e); }",
        "diag.move-out-of-field",
    );
    // Payload-less and `_` arms may inspect a resource enum through a reference.
    expect_true_with(
        "enum E { Has(Vec<i32>), Empty } fn f(ref<E, shared> e) : usize { match (*e) { Has(_) : 1, Empty : 0, } }",
        "{ auto e = E::Has(Vec::new()); f(&e) == 1 }",
    );
}

#[test]
fn conf_closure_drop_through_self_rejected() {
    // [Destroy-Projection] on `self.f_i`; moving the captured resource
    // out is the same rule.
    expect_diag_static("fn main() { Vec<i32> v = Vec::new(); auto f = move [v]() { drop(v); }; f(); }", "diag.move-out-of-field");
    expect_diag_static("fn takes(Vec<i32> v) {} fn main() { Vec<i32> v = Vec::new(); auto f = move [v]() { takes(v); }; f(); }", "diag.move-out-of-field");
    // Calling an item by its bare name inside a closure is not a capture
    // (previously a false diag.capture-list-mismatch).
    expect_true_with("fn helper(i32 x) : i32 { x + 1 }", "{ i32 k = 2; auto f = [k]() { helper(k) + narrow<i32>(sizeof<u8>()) }; auto g = [k]() { Option<i32> o = Some(k); match (o) { Some(v) : v, None : 0, } }; f() + g() == 6 }");
}

#[test]
fn conf_mutex_shared_across_threads() {
    expect_true_with(
        "fn inc(ref<mutex<i32>, shared> m) { auto g = lock(m); *g = *g + 1; }",
        "{ auto m = Mutex::new(0); auto h = spawn(inc, &m); { auto g = lock(&m); *g = *g + 1; } join(h); *lock(&m) == 2 }",
    );
}

#[test]
fn conf_parent_use_while_child_live_rejected() {
    // The write's root is `*r1`, so the analysis leaves it to the
    // dynamic check.
    expect_diag_dynamic_only("fn main() { i32 x = 1; auto r1 = &mut x; auto r2 = &*r1; *r1 = 2; }", "diag.aliasing-conflict");
}

#[test]
fn conf_reclaim_two_paths_clash() {
    expect_diag_body(
        "Vec<i32> v = Vec::new(); Vec::push(&mut v, 7); auto p = rawptr_of(Vec::index_shared(&v, 0)); unsafe { auto r1 = &mut reclaim<i32>(p); auto r2 = &reclaim<i32>(p); }",
        "diag.aliasing-conflict",
    );
}

#[test]
fn conf_reference_escape_dynamic() {
    // The store goes through `*h`, invisible to the static rule; the
    // dead reference faults at use.
    expect_diag_dynamic_only(
        "struct H { ref<i32, shared> r; } fn f(ref<H, exclusive> h) { i32 x = 1; h.r = &x; } fn main() { i32 y = 0; auto hh = H { .r = &y }; f(&mut hh); i32 z = *hh.r; }",
        "diag.stale-binding",
    );
}

#[test]
fn conf_elision_conflict_rejected() {
    expect_diag_static(
        "struct Pair { i32 a; i32 b; } fn first(ref<Pair, shared> p) : ref<i32, shared> { &p.a } fn main() { auto pr = Pair { .a = 1, .b = 2 }; auto y = first(&pr); pr.a = 3; i32 v = *y; }",
        "diag.aliasing-conflict",
    );
}

#[test]
fn conf_destroy_composite_multi_field_order() {
    // The order itself is asserted byte-for-byte by
    // 16-aggregates/struct_field_destruction_reverse_order.cb.
    expect_ok_with(
        "struct Pair { Vec<i32> a; Vec<i32> b; }",
        "Vec<i32> va = Vec::new(); Vec::push(&mut va, 1); Vec<i32> vb = Vec::new(); Vec::push(&mut vb, 2); Pair p = Pair { .a = va, .b = vb };",
    );
}

// ---------- Performance regression: aliasing checks must not scale with the object count ----------

// Every Vec element ever indexed becomes an object (`reclaim` mints one
// per address), and every read/borrow runs a `clash` check. That check
// used to scan *every* live object for reference occurrences, so
// indexing a 10,000-element Vec cost a thousand times more per access
// than indexing a 10-element one (a 200,000-access loop went from ~5s
// to >300s). `scan_refs` now walks only the objects that have ever
// held a `Ref`/`Guard`. This test would take minutes if that regressed;
// the bound is generous so it never flakes on a slow machine.
#[test]
fn perf_vec_index_cost_does_not_grow_with_vec_length() {
    let src = "fn main() { Vec<u32> v = Vec::new(); usize k = 0; while (k < 10000) { Vec::push(&mut v, 0); k = k + 1; } u64 i = 0; u64 acc = 0; while (i < 20000) { acc = acc + widen<u64>(*Vec::index_shared(&v, narrow<usize>(i % 10000))); i = i + 1; } }";
    let start = std::time::Instant::now();
    expect_ok(src);
    let took = start.elapsed();
    assert!(took.as_secs() < 20, "20,000 indexes into a 10,000-element Vec took {:?}; aliasing checks are scaling with the object count again", took);
}
