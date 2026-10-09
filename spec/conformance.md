# CobaltC Conformance Cases

Status: normative artifact
Version: 3.201.0
Conforms to: `spec/02-schema.md` (Kind: Conformance case, `conf.<name>`)
Governed by: `CobaltC_Master_Instructions.md` §1, §19, §20

## Purpose

Scenarios a conforming implementation must reproduce (`term.
conformance`). Each row gives the fragment, the required outcome, and
the **derivation**: the rule steps that produce that outcome under
the 1.0.0 rules, so the row is checkable against the rules and not
merely asserted. Every case has **Status:** ACCEPTED.

**Conventions.** Fragments are statements inside `fn main()` unless a
function is shown, in a program whose root begins with `import std;`
(`CHG-0033`; a fragment's `Vec`, `print`, `Some` … are `std`'s); unsuffixed literals default per
`rule.arith.literal`; `c` denotes a `bool` binding of an enclosing block (so a fragment may
declare its own `c`, D-0131)
whose value `rule.control.flow-analysis` does not see (it reasons about
literals only), so both branches of `if (c)` are live. Outcomes: `→ v` (the last expression's value),
`ok` (well-formed, terminates `ok(0)`), `ok, exit status n`
(terminates `ok(n)`, `rule.fn.program`), `✗ diag.x (static)` (rejected
before execution: a `disposition: rejected` rule, or a `checked` rule
whose guard `rule.control.flow-analysis` refuted), `✗ diag.x
(dynamic)` (terminates `diag.x` at run time). Abbreviations in
derivations: `a_x` the root path of binding `x`; `a_r'` the borrow
path a reference-typed binding `r` holds; `o_x` `x`'s object; `FA` =
`rule.control.flow-analysis`; `SE`/`BE` = `[Stmt-Exit]`/`[Block-Exit]`.
A case that needs standard input (`rule.stdlib.read`) or links the
program's own foreign code (`rule.trust.extern-code`) is a file with a
`cobc-only:` header, and a `stdin-hex:` header gives its input; an
implementation that does not provide what it needs must refuse it
instead (`CHG-0038`, `CHG-0039`). A case that reads program
arguments (`rule.stdlib.args`) is a file whose `args:` header gives
them; a case with no such header runs with none (`CHG-0040`). A file
case runs in its own directory, so a relative path in its `args:`
names a file beside it, and `$TMP` there is a new, empty directory for
each run, for a case that writes files (`CHG-0043`).
Fragments stay single-line, including brace-delimited ones — a
structural exception to this specification's Allman brace convention
(`spec/22` §1), not a deviation from it: a table cell cannot hold real
line breaks without breaking the table.

## 1. Arithmetic (`spec/06`)

| id | Fragment | Outcome | Derivation |
|---|---|---|---|
| `conf.float-exponent-literal` | `f64 x = 1.5e-3; x == 0.0015` | `→ true` | `spec/22` §1 `exponent`: `1.5e-3` denotes 0.0015, rounded to `f64` like `0.0015` (`rule.arith.literal`) |
| `conf.float-literal-out-of-range` | `f64 x = 1e400;` | ✗ `diag.literal-out-of-range` (static) | `1e400` rounds beyond `f64`'s largest finite magnitude: `[Literal-Out-Of-Range]` |
| `conf.float-literal-f32-out-of-range` | `f32 y = 1e39;` | ✗ `diag.literal-out-of-range` (static) | typed `f32` by the declaration; 10^39 rounds beyond `f32`'s range |
| `conf.float-literal-f32-rounded` | `f32 y = 16777217.0; y == 16777216.0: f32` | `→ true` | `[Literal-Type-From-Context]`: the literal is an `f32`, so its value is 16777217 rounded to `f32` (ties to even: 16777216) |
| `conf.byte-char-literal` | `u8 x = b','; x` | `→ 44` | `byte-char-literal` (`spec/22` §1, D-0033): the byte of `,`, of type `u8` |
| `conf.byte-char-escape` | `b'\n' + b'\x01'` | `→ 11` | escapes as in a `byte-literal`: 10 + 1 |
| `conf.byte-char-is-u8` | `i32 x = b'a';` | ✗ `diag.type-mismatch` (static) | a `byte-char-literal` is `u8` in every context, like a suffixed literal |
| `conf.radix-literals` | `u8 a = 0xFF; u8 b = 0b1010; u8 c = 0o17; widen<u32>(a) + widen<u32>(b) + widen<u32>(c)` | `→ 280` | `spec/22` §1 `int-literal` (D-0035): base 16, 2 and 8; 255 + 10 + 15 |
| `conf.digit-separators` | `u64 a = 1_000_000; u32 b = 0xFFFF_0000; u8 c = 0b1010_0101; f64 d = 1_234.567_8e1_0; a == 1000000 && b == 4294901760 && c == 165 && d == 12345678000000.0` | `→ true` | `spec/22` §1 `digits` (D-0043): each `_` is between two digits and does not change the value |
| `conf.parse-rejects-digit-separator` | `String s = String::from_str("1_000"); Result::unwrap_or(String::parse<u32>(&s), 7) == 7` | `→ true` | `rule.stdlib.text` `[Parse]`: `_` is not part of `number(T)`; a separator is source syntax, not text a program reads |
| `conf.literal-expression-typed` | `u64 x = 65536 * 65536; x` | `→ 4294967296` | `rule.type.expected` (D-0037): the operands are all literals, so the declaration's `u64` is each one's expected type; `[Arith-Checked]` in `u64` |
| `conf.literal-expression-default` | `auto d = 65536 * 65536;` | ✗ `diag.arith-overflow` (static) | no expected type: the literals default to `i32`, and FA refutes the product's range |
| `conf.literal-expression-negated` | `i64 n = -(1 << 40) + 3; n` | `→ -1099511627773` | `-`, parentheses and `<<` of literals are a literal expression: all `i64` |
| `conf.const-literal-expression` | `const u64 MIB = 1024 * 1024;` `MIB` | `→ 1048576` | a constant's initializer is typed as a declaration's (`rule.module.const`): `u64` |
| `conf.compound-assign-ops` | `i32 x = 7; x -= 2; x *= 3; x /= 2; x %= 5; x <<= 4; x >>= 1; x \|= 1; x &= 0xff; x ^= 0b1010; x` | `→ 27` | `[Compound-Assign]` (`spec/13`, D-0035): each `x op= e` is `x = x op e`, `x` evaluated once; `<<=`, `>>=` take a `u32` amount |
| `conf.compound-assign-overflow` | `fn bump(i32 x) : i32 { i32 y = x; y += 1; y }` `bump(2147483647)` | ✗ `diag.arith-overflow` (dynamic) | `[Compound-Assign]` is `y = y + 1`: `[Arith-Checked]` as for `+` |
| `conf.compound-assign-call-target` | `Vec<i32> v = Vec::new(); Vec::push(&mut v, 10); Vec::push(&mut v, 20); *Vec::index_exclusive(&mut v, 1) += 5; *Vec::index_shared(&v, 1)` | `→ 25` | the target contains a call, so it is evaluated once, through an exclusive reference: `{ auto t = &mut *Vec::index_exclusive(&mut v, 1); *t = *t + 5; }` |
| `conf.reborrow-call-evaluated-once` | `HashMap<String, u32> m = HashMap::new(); String k = String::from_str("a"); *HashMap::entry(&mut m, k, 0) += 1; HashMap::len(&m) == 1` | `→ true` | the call in `&mut *f(…)` (here the hidden reference of `[Compound-Assign]`) is evaluated once: `k` moves into it once |
| `conf.i32-add-in-range` | `2000000000: i32 + 100000000: i32` | `→ 2100000000` | `[Arith-Checked]`: sum in range |
| `conf.i32-add-overflow-static` | `2147483647: i32 + 1: i32` | ✗ `diag.arith-overflow` (static) | both operands literal → FA value-range refuted → `[Arith-Checked-Overflow]` instance rejected |
| `conf.literal-bound-local-overflow-static` | `u8 x = 200; u8 y = x + 100;` | ✗ `diag.arith-overflow` (static) | `x` is literal-bound (`spec/14` §FA, CHG-0092) → value-range refuted |
| `conf.literal-bound-local-written` | `u8 x = 200; x = 20; x + 100` | `→ 120` | the write ends the literal binding; the sum is decided at run time and fits |
| `conf.i32-add-overflow-dynamic` | `fn add(i32 a, i32 b) : i32 { a + b }` `add(2147483647, 1)` | ✗ `diag.arith-overflow` (dynamic) | operands are parameters → FA unknown → runtime guard fails |
| `conf.i32-div-zero-dynamic` | `fn d(i32 a, i32 b) : i32 { a / b }` `d(10, 0)` | ✗ `diag.div-by-zero` (dynamic) | `[Div-By-Zero]` |
| `conf.i32-div-min-neg-one` | `d(-2147483647 - 1, -1)` | ✗ `diag.div-overflow` (dynamic) | `[Div-Overflow]` (`min(i32)` is computed, not a literal, so FA does not refute the guard; the literal form is `conf.div-min-neg-one-static`) |
| `conf.u8-wrapping-add` | `wrapping_add(255: u8, 1: u8)` | `→ 0` | `[Wrapping]` |
| `conf.checked-add-none` | `checked_add(255: u8, 1: u8)` | `→ None` | `[Checked-None]`; type `Option<u8>` |
| `conf.checked-neg` | `checked_neg(-2147483647 - 1)` | `→ None` | `checked_neg(a)` is `checked_sub(0, a)` (D-0072); `0 - min(i32)` is out of range |
| `conf.checked-neg-unsigned` | `Option::unwrap_or(checked_neg(0: u8), 9: u8) + Option::unwrap_or(checked_neg(3: u8), 9: u8)` | `→ 9` | `checked_sub(0, 0) = Some(0)`; `checked_sub(0, 3)` on `u8` is `None` |
| `conf.shift-in-range` | `1: u32 << 31: u32` | `→ 2147483648` | `[Shl-Checked]` |
| `conf.shift-out-of-range` | `1: u32 << 32: u32` | ✗ `diag.shift-amount-out-of-range` (static) | literals → FA refuted |
| `conf.shr-arithmetic` | `fn s(i32 x) : i32 { x >> 1 }` `s(-8)` | `→ -4` | `1 : u32` from the shift-amount position (`rule.type.expected`); `[Shr-Checked]`: floor(-8/2) |
| `conf.shift-binds-tighter-than-bitor` | `u32 hi = 1; u32 lo = 2; hi << 8 \| lo` | `→ 258` | D-0086: `(hi << 8) \| lo`, as in C and Rust |
| `conf.shift-binds-tighter-than-bitand` | `u32 x = 0xAB; x >> 4 & 0xF` | `→ 10` | D-0086: `(x >> 4) & 0xF` |
| `conf.shift-looser-than-additive` | `u32 n = 2; 1: u32 << n + 1` | `→ 8` | D-0086: `1 << (n + 1)` |
| `conf.literal-out-of-range` | `256: u8` | ✗ `diag.literal-out-of-range` (static) | `[Literal-Out-Of-Range]` |
| `conf.negated-literal-min` | `-2147483648 < -2147483647` | `→ true` | `[Literal-Default]`: i32; `[Literal-Negated]`: −2³¹ ∈ represented-domain(i32) |
| `conf.negated-literal-signed-mins` | `impl/conformance/06-arithmetic/negated_literal_min_ok.cb` | ok | `[Literal-Negated]` for the minimum of `i8`, `i16`, `i32`, `i64` and `i128`, from the declared type and from a suffix |
| `conf.negated-literal-below-min` | `-2147483649` | ✗ `diag.literal-out-of-range` (static) | `[Literal-Negated]`: −2³¹ − 1 ∉ represented-domain(i32) |
| `conf.negated-literal-parenthesized` | `-(2147483648)` | ✗ `diag.literal-out-of-range` (static) | the minus is not applied directly, so `[Literal-Negated]` is inapplicable; `2147483648 : i32` is `[Literal-Out-Of-Range]` |
| `conf.negated-literal-unsigned-rejected` | `-0: u8` | ✗ `diag.type-mismatch` (static) | `[Literal-Negated]` requires signed(τ); `[T-Neg]` |
| `conf.neg-unsigned-rejected` | `fn ng(u8 m) : u8 { -m }` `ng(0)` | ✗ `diag.type-mismatch` (static) | `[T-Neg]`: `u8` is neither signed nor float |
| `conf.div-min-neg-one-static` | `-2147483648 / -1` | ✗ `diag.div-overflow` (static) | `[Literal-Negated]`: `min(i32)`; both operands literal → FA refuted → `[Div-Overflow]` instance rejected |
| `conf.limits-max-i32` | `max_value<i32>() == 2147483647` | `→ true` | `[Max-Value]`: max(i32) = 2³¹ − 1 |
| `conf.limits-values` | `impl/conformance/06-arithmetic/type_limits_ok.cb` | ok | `[Min-Value]`, `[Max-Value]` for all twelve integer types; `isize`/`usize` agree with their `AddrWidth` |
| `conf.limits-generic-ok` | `fn or_max<T>(Option<T> r) : T { match (r) { Some(x) : x, None : max_value<T>(), } }` `or_max(checked_add(max_value<u8>(), 1: u8))` | `→ 255` | instantiation `T = u8`: `[Max-Value]` well-typed; `[Checked-None]` → `None` arm |
| `conf.limits-not-integer` | `max_value<bool>()` | ✗ `diag.type-mismatch` (static) | `[Limits-Not-Number]` (a float type has limits since D-0051; `bool` has none) |
| `conf.limits-generic-not-integer` | `fn lo<T>() : T { min_value<T>() }` `lo<bool>()` | ✗ `diag.type-mismatch` (static) | `rule.type.kind`: typed at the instantiation `T = bool` → `[Limits-Not-Number]` |
| `conf.limits-float` | `max_value<f32>() == 3.4028235e38: f32 && min_value<f64>() == -1.7976931348623157e308` | `→ true` | D-0051: a float's limits are its largest finite value and its negation |
| `conf.widen-f32-to-f64` | `widen<f64>(0.5: f32) == 0.5 && to_float<f64>(0.1: f32) == widen<f64>(0.1: f32)` | `→ true` | `[Widen-Float]`, `[Float-To-Float]`: every f32 is an f64, exactly |
| `conf.to-float-f64-to-f32-rounds` | `to_float<f32>(1.0 / 3.0) == 0.33333334: f32 && to_float<f32>(1.0e300) == to_float<f32>(2.0e300)` | `→ true` | `[Float-To-Float]`: nearest f32 (ties to even); beyond f32's finite range, an infinity |
| `conf.widen-f64-to-f32-rejected` | `f32 x = widen<f32>(1.5);` | ✗ `diag.type-mismatch` (static) | `[T-Convert]`: not every f64 is an f32; `to_float<f32>` rounds |
| `conf.reinterpret-float-bits` | `reinterpret<u32>(1.0: f32) == 0x3f80_0000: u32 && reinterpret<i64>(-0.0) == min_value<i64>() && reinterpret<f64>(reinterpret<u64>(2.5)) == 2.5` | `→ true` | `[Reinterpret-Float]`: the IEEE-754 bits, and back |
| `conf.reinterpret-float-width-rejected` | `u64 b = reinterpret<u64>(1.0: f32);` | ✗ `diag.type-mismatch` (static) | `[T-Convert]`: `[Reinterpret-Float]` needs one width (32 bits for f32) |
| `conf.limits-uninferable` | `i32 x = max_value();` | ✗ `diag.cannot-infer-type-parameter` (static) | `[Limits-Uninferable]`: the type argument is written explicitly |
| `conf.limits-overflow-dynamic` | `max_value<i32>() + 1` | ✗ `diag.arith-overflow` (dynamic) | the left operand is not a literal → FA unknown → runtime guard fails (D-0026 (c)) |
| `conf.alt-mixed-types-rejected` | `wrapping_add(1: i32, 2: i64)` | ✗ `diag.type-mismatch` (static) | `[T-Alt]`: two integer types |
| `conf.alt-non-integer-rejected` | `checked_add(1.0, 2.0)` | ✗ `diag.type-mismatch` (static) | `[T-Alt]`: f64 is not an integer type |
| `conf.alt-generic-non-integer-rejected` | `fn f<T>(T a, T b) : T { wrapping_add(a, b) }` `f(true, false)` | ✗ `diag.type-mismatch` (static) | `rule.type.kind`: typed at the instantiation `T = bool` → `[T-Alt]` |
| `conf.convert-wrong-kind-rejected` | `to_int<i32>(1)` | ✗ `diag.type-mismatch` (static) | `[T-Convert]`: `to_int`'s operand must be f32 or f64 |
| `conf.widen-not-contained-rejected` | `fn w(i32 x) : u64 { widen<u64>(x) }` `w(5)` | ✗ `diag.type-mismatch` (static) | `[T-Convert]`: represented-domain(i32) ⊄ represented-domain(u64), so `[Widen]` does not apply |
| `conf.narrow-contained-ok` | `narrow<i64>(5: i32) == 5` | `→ true` | `[Narrow-Checked]` takes any two integer types (D-0027); 5 ∈ represented-domain(i64) |
| `conf.reinterpret-same-sign-rejected` | `reinterpret<i64>(5: i32)` | ✗ `diag.type-mismatch` (static) | `[T-Convert]`: `[Reinterpret-Sign]` needs one bitwidth and differing signedness |
| `conf.call-too-many-args` | `fn f(i32 a) : i32 { a }` `f(1, 2)` | ✗ `diag.type-mismatch` (static) | `[T-Call]`: two arguments for one parameter |
| `conf.call-too-few-args` | `fn g(i32 a, i32 b) : i32 { a }` `g(1)` | ✗ `diag.type-mismatch` (static) | `[T-Call]`: one argument for two parameters |
| `conf.closure-call-arity-rejected` | `auto c = [](i32 a) { a }; c(1, 2)` | ✗ `diag.type-mismatch` (static) | `[T-Call]` through the closure's `callable` type (`rule.fn.closure`) |
| `conf.call-non-callable-rejected` | `i32 x = 3; x(1)` | ✗ `diag.type-mismatch` (static) | `[T-Call]`: `i32` is not callable |
| `conf.item-shadows-intrinsic` | `fn widen(i32 a) : i32 { a + 1 }` `widen(1) == 2` | `→ true` | `[Resolve-Unqualified]` (1) finds the item; the intrinsic is not reached (`spec/21` §0) |
| `conf.local-shadows-intrinsic` | `auto max_value = [](i32 a) { a * 2 }; max_value(4) == 8` | `→ true` | the local binding comes first (`rule.value-object.binding-lookup`); `spec/21` §0 |
| `conf.narrow-overflow` | `fn n(i32 x) : u8 { narrow<u8>(x) }` `n(300)` | ✗ `diag.narrowing-overflow` (dynamic) | `[Narrow-Overflow]` (i32 → u8: differing signedness, `[Widen]` inapplicable, so the checked narrowing applies) |
| `conf.cross-type-cmp-rejected` | `1: i32 < 1: i64` | ✗ `diag.type-mismatch` (static) | `[T-Cmp]` requires one τ |
| `conf.neg-in-range` | `-(5: i32)` | `→ -5` | `[Neg]` |
| `conf.neg-min-overflow` | `fn ng(i32 m) : i32 { -m }` `ng(-2147483647 - 1)` | ✗ `diag.arith-overflow` (dynamic) | `[Neg-Overflow]` |
| `conf.cmp-le-ge-ne` | `1: i32 <= 1: i32`, `2: i32 >= 1: i32`, `1: i32 != 2: i32` | all `→ true` | `[Cmp-Le]`, `[Cmp-Ge]`, `[Cmp-Ne]` |
| `conf.float-nan-ge-false` | `fn z() : f64 { 0.0 }` `(z() / z()) >= 1.0` | `→ false` | `[Arith-Float]` NaN; `[Cmp-Ge]`: neither disjunct |
| `conf.bitwise` | `(6: u8 & 3: u8) \| (8: u8 ^ 8: u8)` | `→ 2` | `[Bit-And]`, `[Bit-Xor]`, `[Bit-Or]` |
| `conf.impl-defined-width-observed` | `impl/conformance/06-arithmetic/impl_defined_width_observed_ok.cb` | ok; prints `sizeof<usize>()` — `8` where `AddrWidth = 64`, `4` where it is 32 — then `true` | D-0206 (`term.implementation-dependent-program`): `[Sizeof-Addr]` is `outcome: impl-defined { 2, 4, 8, 16 }`, so the first value is the implementation's documented choice and legitimately differs between conforming implementations; the program is implementation-independent (it names nothing of `std::extensions`) but not a portable program (`term.portable-program`). The second value is a relation every choice satisfies (`[Sizeof-Addr]` gives `usize`, `rawptr<τ>` and `ref<τ,m>` one size, at least 2 bytes), so it is `true` everywhere. The case's own header expects the 64-bit output; an implementation at another width documents the first value it prints instead |
| `conf.ref-byte-image-width` | `impl/conformance/06-arithmetic/ref_byte_image_width_ok.cb` | ok; prints `8 8 7` where `AddrWidth = 64` (`AddrWidth/8`, the same count, `7`) | D-0206 (`spec/06` §7 byte view): `r = &x` is a `ref<i32, shared>` value `a` held in `r`'s cells, `represent(ref<i32,shared>, a) = [ref(a)] ++ [cont]^(AddrWidth/8 − 1)`; `rawptr_of(r)` → `[Rawptr-Of]` gives `min(target(a))`, `x`'s address, held in `p`'s cells as `[Repr-Rawptr]`'s `AddrWidth/8` address bytes in order `BO`. `rawptr_of(&r)` addresses `r`'s own cells; `copy_raw(dst, src, n)` → `[Copy-Raw]` copies the `n = sizeof(ref<i32, shared>)` cells into `buf` (trusted: both ranges are objects' extents, `buf` is a live plain array whose cells the write replaces as `[Rawptr-Write]` would). The byte view of the copied group — a `ref(a)` cell followed by its `cont` cells, all inside the range — is one `AddrWidth/8`-byte image of `min(target(a))` in order `BO`, byte `j` at position `j`: exactly what `[Repr-Rawptr]` stored for `p`. Each `buf[i]` (`[Index-Checked]`, then `value-at(u8)`) equals `*(q + i)` (`[Rawptr-Offset]`, `[Rawptr-Read]` of `p`'s byte `i`), so `same = n`; `*r` still reads `7` through the untouched reference. Found two `coby` divergences on its first run (CHG-0243): the image was all zeros, and `rawptr_of` of a reference binding answered the binding's cells |


## 2. Values, local declarations, and bindings (`spec/05`, `spec/11`)

| id | Fragment | Outcome | Derivation |
|---|---|---|---|
| `conf.let-copy` | `i32 x = 1; auto y = x; y + x` | `→ 2` | `[Let]`→`[Store-Binding-Value]` for x; y's initializer is value position (non-resource) → `[LValue-To-RValue]` reads `a_x` → `[Store-Binding-Value]` into fresh `o_y`; both readable |
| `conf.let-uninit-then-assign-ok` | `i32 x; x = 5; x` | `→ 5` | `[Let-Uninit]`; whole-object `[Write]` (`init-ok`: uninitialized, whole); FA `init(x) = T` at the read |
| `conf.definite-assignment-both-branches` | `i32 x; if (c) { x = 1; } else { x = 2; } x` | `ok` | FA: `init(x) = T` on both branches → join T |
| `conf.definite-assignment-missing-branch` | `i32 x; if (c) { x = 1; } x` | ✗ `diag.use-of-uninitialized` (static) | join of T and F = ? → `rule.init.definite-assignment` rejects on unknown |
| `conf.partial-init-rejected` | `struct P { i32 a; i32 b; } P p; p.a = 1;` | ✗ `diag.use-of-uninitialized` (static) | projection write with `init = uninitialized` → `[Write-Partial-Init]`; FA `init(p) = F` refuted |
| `conf.shadowing` | `i32 x = 1; i32 y = { i32 x = 2; x }; x + y` | `→ 3` | a nested block's `[Binding-Form]` binds a new `x` that shadows the outer one to the block's end; the outer `x` is untouched (D-0131) |
| `conf.redeclared-local-in-block-rejected` | `i32 x = 1; i32 x = 2;` | ✗ `diag.duplicate-local` (static) | `[Binding-Form-Redeclared]`: `x` is already declared in the block (D-0131) |
| `conf.redeclared-param-rejected` | `fn f(i32 n) : i32 { i32 n = n * 2; n }` | ✗ `diag.duplicate-local` (static) | a function's parameters govern its body (D-0131) |
| `conf.duplicate-param-rejected` | `fn f(i32 a, i32 a) : i32 { a }` | ✗ `diag.duplicate-local` (static) | two parameters of one function (D-0131) |
| `conf.redeclared-foreach-name-rejected` | `array<i32, 2> a = [1, 2]; foreach (x in &a) { i32 x = 0; }` | ✗ `diag.duplicate-local` (static) | a `foreach`'s names govern its body (D-0131) |
| `conf.duplicate-destructure-binder-rejected` | `struct S { i32 a; i32 b; } S { a, b: a } = S { .a = 1, .b = 2 };` | ✗ `diag.duplicate-local` (static) | two binders of one destructuring (D-0131) |
| `conf.unbound-name` | `fn f() : i32 { y }` | ✗ `diag.unbound-name` (static) | `[Binding-Lookup-Unbound]`, `[Resolve-Unbound]` |
| `conf.deref-non-reference` | `impl/conformance/13-expression-semantics/deref_non_reference_rejected.cb` | ✗ `diag.type-mismatch` (static) | `* 3`: `[T-Deref]` of an integer |

## 3. Resource authority (`spec/07`, `spec/14`)

| id | Fragment | Outcome | Derivation |
|---|---|---|---|
| `conf.transfer-invalidates-source` | `Vec<i32> v = Vec::new(); auto w = v; Vec::push(&mut v, 1);` | ✗ `diag.stale-binding` (static) | `auto w = v`: `[Store-Binding-Place-Transfer]` → `[Authority-Transfer]` invalidates `a_v`; FA `valid(v) = F` → `[Binding-Lookup-Stale]` refuted |
| `conf.double-destroy-rejected` | `Vec<i32> v = Vec::new(); drop(v); drop(v);` | ✗ `diag.stale-binding` (static) | first `drop`: `[Destroy]` → `[Object-End]` invalidates `a_v`; FA `valid(v) = F` |
| `conf.leak-free-by-construction` | `{ Vec<i32> v = Vec::new(); }` | `ok`; `Vec::drop` runs once | `Vec::new` → `[Struct-Construct]` temp with authority (resource marker); `[Store-Binding-Temp]` adopts; BE: `owned-by-frame(o_v)` → `[Destroy]` → `[Run-Destructor]` → `[Object-End]` |
| `conf.unstored-temporary-destroyed-at-stmt-end` | `fn make() : Vec<i32> { Vec::new() }` `make();` | `ok`; `Vec::drop` runs at the `;` | the call result is `temp o` with `temp-scope` re-stamped to the caller's statement; SE (`discard`): `Temps ∋ o` → `[Destroy]` |
| `conf.returned-resource-destroyed-at-caller-exit` | `fn make_vec() : Vec<i32> { Vec<i32> v = Vec::new(); Vec::push(&mut v, 1); v }` `{ auto r = make_vec(); }` | `ok`; destroyed at the caller's BE | trailing `v` (keep): `[Store-Result-Place-Release]` clears `holder(o_v)`, invalidates `a_v`; BE of `make_vec`'s body and `[Call]`'s BE exempt `objs-in(temp o_v)`; `[Let]` adopts; caller BE destroys |
| `conf.early-return-destroys-locals` | `fn f() : i32 { Vec<i32> v = Vec::new(); if (true) { return 1; } 2 }` `f()` | `→ 1`; `Vec::drop` runs | `[Return]`: `unwind-to(f_b, 1)` runs SE/BE of the `if` branch and the body frame → `o_v` destroyed |
| `conf.destroy-while-borrowed-rejected` | `Vec<i32> v = Vec::new(); auto r = &v; drop(v); usize n = Vec::len(r);` | ✗ `diag.destroy-while-aliased` (static) | `r` is used after (D-0111); `a_r'` valid on `o_v` → `[Destroy-Not-Solitary]`; FA `deriv(r, v.ε, shared) = T` → `solitary` refuted |
| `conf.destroy-after-borrow-scope-ends-ok` | `{ Vec<i32> v = Vec::new(); auto r = &v; }` | `ok` | BE: `Owned = {o_v, o_r}` descending origin → `o_r` ends first (`[Object-End]`: `a_r'` `held-by = {o_r}` → Dead → invalid) → `[Destroy]` on `a_v`: solitary ✓ |
| `conf.move-while-borrowed-rejected` | `Vec<i32> v = Vec::new(); auto r = &v; auto w = v; usize n = Vec::len(r);` | ✗ `diag.move-while-aliased` (static) | `r` is used after (D-0111); `[Authority-Transfer-Aliased]`: `a_r'` ∉ {a_v, a_w}; FA `deriv` T → refuted |
| `conf.overwrite-live-resource-rejected` | `resource struct R { i32 v; } R r = R { .v = 1 }; r = R { .v = 2 };` | ✗ `diag.overwrite-of-live-resource` (static) | `[Write-Resource-Overwrite-Rejected]`: `o_v ∈ obligations`; FA `¬live-resource-at` row: `valid(v) = T ∧ init(v) = T` → refuted |
| `conf.reassign-after-drop-ok` | `Vec<i32> v = Vec::new(); drop(v); v = Vec::new(); Vec::push(&mut v, 1); Vec::len(&v)` | `→ 1` | `drop` ends `o_v`; `v = Vec::new()` is `[Assign-Reestablish]` (`spec/11` §2, D-0033): a new object bound to `v` in `main`'s frame; FA `valid(v) := T` again |
| `conf.reassign-after-move-ok` | `Vec<i32> v = Vec::new(); auto w = v; v = Vec::new(); Vec::push(&mut v, 1); Vec::len(&v) + Vec::len(&w)` | `→ 1` | `auto w = v` transfers `o_v` to `w`; `[Assign-Reestablish]` gives `v` a new object; `w` keeps the old one |
| `conf.reassign-from-own-move-ok` | `fn grow(Vec<i32> v) : Vec<i32> { Vec<i32> w = v; Vec::push(&mut w, 7); w }` `Vec<i32> v = Vec::new(); v = grow(v); v = grow(v); Vec::len(&v)` | `→ 2` | `grow(v)` moves `v`'s value away while the right side is evaluated; the assignment then re-establishes `v` with the result |
| `conf.reassign-in-inner-block-ok` | `Vec<i32> v = Vec::new(); { auto w = v; v = Vec::new(); } Vec::push(&mut v, 3); Vec::len(&v)` | `→ 1` | the new object is bound in the frame that declared `v`, not the inner block's, so it outlives the block; `w`'s object ends at the block's BE |
| `conf.reassign-maybe-live-dynamic` | `Channel<u8> v = Channel::new(1); if (!c) { drop(v); } v = Channel::new(1);` | ✗ `diag.overwrite-of-live-resource` (dynamic) | FA `valid(v) = unknown` after the `if`, so `¬live-resource-at` is checked at run time; `c` is true, `v` was not dropped, and it still holds a live `Channel` (not quiet, D-0194) |
| `conf.drop-then-redeclare-rejected` | `Vec<i32> v = Vec::new(); drop(v); Vec<i32> v = Vec::new();` | ✗ `diag.duplicate-local` (static) | `[Binding-Form-Redeclared]`: once `v`'s value has ended, `v = Vec::new();` gives it a new one (`conf.reassign-after-drop-ok`); declaring it again is rejected (D-0131) |
| `conf.read-of-resource-rejected` | `Vec<i32> v = Vec::new(); Vec<i32> w = Vec::new(); v == w` | ✗ `diag.read-of-resource` (static) | comparison operands are value position → `[Read-Resource-Rejected]` (before `[T-Cmp]`'s own rejection) |
| `conf.move-out-of-field-rejected` | `struct P { Vec<i32> a; } auto p = P { .a = Vec::new() }; auto q = p.a;` | ✗ `diag.move-out-of-field` (static) | `[Store-Binding-Place-Transfer-Sub]` |
| `conf.destructor-on-plain-type-rejected` | `struct Noisy { u32 id; }` `fn Noisy::drop(ref<Noisy, exclusive> self) { }` `Noisy n = Noisy { .id = 1 };` | ✗ `diag.destructor-on-plain-type` (static) | a plain value is copied and never destroyed; the type must be `resource` (D-0075) |
| `conf.move-through-reference-rejected` | `String x = String::from_str("a"); String y = *(&x);` | ✗ `diag.move-while-aliased` (dynamic) | `[Authority-Transfer-Aliased]`: the place is reached through `&x`, and `x`'s own path still reaches the object; FA: the root is a temporary → unknown → dynamic |
| `conf.move-through-param-rejected` | `fn take(ref<String, exclusive> r) : String { *r }` `String x = String::from_str("a"); String y = take(&mut x);` | ✗ `diag.move-while-aliased` (dynamic) | the result `*r` would move the caller's `String` out from under `x`; `[Authority-Transfer-Aliased]` at the move, in the callee |
| `conf.while-body-frame-per-iteration` | `i32 i = 0; while (i < 3) { Vec<i32> v = Vec::new(); i = i + 1; }` | `ok`; `Vec::drop` runs 3 times, each before the next test | `[While-True]`: `loop-body(b) ; while (…) …` — the body block's BE runs before `[Seq-Value]` reaches the re-test |
| `conf.for-sum-continue` | `u64 s = 0; for (u64 i = 0; i < 10; i += 1) { if (i % 3 == 0) { continue; } s += i; } s` | `→ 27` | `[For]` (`spec/14`, D-0035): `continue` runs the step before the next test; 1 + 2 + 4 + 5 + 7 + 8 |
| `conf.for-empty-parts` | `i32 n = 0; for (; n < 5; ) { n += 2; } for (;;) { break; } n` | `→ 6` | each part may be left out; a missing condition is `true` |
| `conf.return-ends-statement-temporaries` | `fn look(ref<Vec<String>, shared> v) : i32 { match (Some(Vec::index_shared(v, 0))) { Some(r) : { return 1; }, None : {}, } 0 }` `Vec<String> v = Vec::new(); Vec::push(&mut v, String::from_str("x")); look(&v) == 1` | `→ true` | `rule.control.stmt`: the `return` leaves the `match` statement, whose temporary scrutinee (holding a reference into `v`) ends then; `v` is later destroyed with no reference left |
| `conf.for-variable-scoped` | `for (i32 i = 0; i < 1; i += 1) { } i` | ✗ `diag.unbound-name` (static) | the initializer's binding belongs to the `for`: `{ i32 i = 0; while … }` |
| `conf.break-destroys-body-locals` | `i32 i = 0; while (true) { Vec<i32> v = Vec::new(); break; } i` | `→ 0`; `Vec::drop` once | `[Break]`: `unwind-to(f_w, ())` runs the body frame's BE |
| `conf.resource-in-none-binding-destroyed` | `impl/conformance/07-resource-authority/resource_stored_in_none_binding_destroyed_ok.cb` | ok | a binding written `Option<Noisy>` made from `None` is a resource; a `Noisy` stored in it by assignment, `swap` or `replace` is destroyed by `drop` or at block end (`[Destroy-Composite]`) |

## 4. Aliasing (`spec/08`, `spec/16`)

| id | Fragment | Outcome | Derivation |
|---|---|---|---|
| `conf.two-shared-borrows-ok` | `i32 x = 1; auto r1 = &x; auto r2 = &x; *r1 + *r2` | `→ 2` | `[Borrow]` twice: `clash(a_x, shared)`: `a_r1'` shared → permitted; reads: `a_x` is an ancestor of each, siblings shared |
| `conf.owner-read-while-shared-ok` | `i32 x = 1; auto r = &x; x + *r` | `→ 2` | `[Read]` through `a_x`: `clash(a_x, shared)` — `a_r'` is a descendant, shared → permitted |
| `conf.owner-write-while-shared-rejected` | `i32 x = 1; auto r = &x; x = 2; i32 y = *r;` | ✗ `diag.aliasing-conflict` (static) | `r` is used after (D-0111); `[Write-Conflict]`: `a_r'` shared vs exclusive; FA `deriv(r, x.ε, shared) = T` refuted |
| `conf.owner-read-while-exclusive-rejected` | `i32 x = 1; auto r = &mut x; i32 y = x; *r = 2;` | ✗ `diag.aliasing-conflict` (static) | `[Read-Conflict]`: `a_r'` exclusive, and `r` is used after (D-0111) |
| `conf.shared-then-exclusive-rejected` | `i32 x = 1; auto r1 = &x; auto r2 = &mut x; i32 y = *r1;` | ✗ `diag.aliasing-conflict` (static) | `r1` is used after (D-0111); `[Borrow-Denied]`: `clash(a_x, exclusive)` with `a_r1'` |
| `conf.ref-binding-ends-at-last-use` | `i32 x = 1; auto r = &x; i32 y = *r; x = 2; x + y` | `→ 3` | `[Ref-Binding-Last-Use]` (D-0111): `r` ends after its last use, so the write to `x` clashes with nothing |
| `conf.ref-holding-binding-ends-at-last-use` | `impl/conformance/14-control-flow/ref_holding_binding_last_use_ok.cb` | ok | `[Ref-Binding-Last-Use]` (D-0133): an `Option<ref<…>>` and a struct of references end after their last use |
| `conf.ref-holding-binding-used-later-rejected` | `impl/conformance/14-control-flow/ref_holding_binding_used_later_rejected.cb` | ✗ `diag.aliasing-conflict` (static) | used after the write: still borrowing |
| `conf.ref-binding-used-later-rejected` | `impl/conformance/08-alias-validity/ref_binding_used_later_rejected.cb` | ✗ `diag.aliasing-conflict` (static) | `b` is used after `String::clear(&mut cur)`, so it still holds `cur` there |
| `conf.ref-binding-used-in-loop-rejected` | `impl/conformance/08-alias-validity/ref_binding_used_in_loop_rejected.cb` | ✗ `diag.aliasing-conflict` (static) | a use in a loop is a use by the whole loop statement: `first` lives through it, and the push inside clashes |
| `conf.sequential-borrows-ok` | `i32 x = 1; { auto r1 = &mut x; *r1 = 2; } auto r2 = &x; *r2` | `→ 2` | inner BE ends `o_r1` → `a_r1'` Dead; second `[Borrow]` finds no live conflicting path |
| `conf.reborrow-ok` | `i32 x = 1; auto r1 = &mut x; { auto r2 = &*r1; *r2; } *r1 = 3; *r1` | `→ 3` | `&*r1`: `[Ref-Deref-Place]` → `[Borrow]` from `a_r1'` (shared from exclusive; `a_x` is an ancestor); after the block `a_r2'` is Dead; `*r1 = 3` and `*r1` clash with nothing (reading `x` itself here would be `[Read-Conflict]`, since `a_r1'` is a live exclusive descendant) |
| `conf.parent-use-while-child-live-rejected` | `i32 x = 1; auto r1 = &mut x; auto r2 = &*r1; *r1 = 2;` | ✗ `diag.aliasing-conflict` (static) | `[Write-Conflict]`: `a_r2'` is a shared descendant of `a_r1'`; FA (D-0071): `deriv(r2, *r1, shared) = T` overlaps the write at `*r1` → refuted |
| `conf.reborrow-field-through-param-rejected` | `struct P { i32 a; i32 b; } fn f(ref<P, exclusive> p) : i32 { auto a = &mut p.a; p.a = 3; *a }` `P q = P { .a = 1, .b = 2 }; f(&mut q)` | ✗ `diag.aliasing-conflict` (static) | `p.a` is `(*p).a` (`[Field-Access-Auto-Deref]`); FA (D-0071): `deriv(a, *p.a, exclusive) = T` overlaps the write → refuted |
| `conf.reborrow-disjoint-field-ok` | `struct P { i32 a; i32 b; } fn f(ref<P, exclusive> p) : i32 { { auto a = &mut p.a; p.b = 3; *a = 4; } p.a + p.b }` `P q = P { .a = 1, .b = 2 }; f(&mut q)` | `→ 7` | paths `*p.a` and `*p.b` do not overlap; the write through `p` is not refuted (and, the root being `*p`, not proven either: D-0022) |
| `conf.reborrow-reassigned-ok` | `i32 x = 1; i32 y = 2; auto r1 = &mut x; auto r2 = &*r1; r1 = &mut y; *r1 = 5; *r2 + *r1` | `→ 6` | assigning `r1` clears the facts rooted at `*r1` (D-0071): `r2` still borrows `x`, `*r1` is now `y` |
| `conf.projection-write-while-borrowed-rejected` | `struct P { i32 a; i32 b; } auto p = P { .a = 1, .b = 2 }; auto r = &mut p.a; p.a = 5; *r = 6;` | ✗ `diag.aliasing-conflict` (static) | `r` is used after (D-0111); fresh projection `a_pa2` (base `a_p`); `clash(a_pa2, exclusive)`: `a_r'` valid, not an ancestor, overlapping; FA `deriv(r, p.a, exclusive)` overlaps `p.a` → refuted |
| `conf.disjoint-field-borrows-ok` | same `P`; `auto r1 = &mut p.a; auto r2 = &mut p.b; *r1 + *r2` | `→ 3` | `clash(a_pb, exclusive)`: `a_r1'` targets `a`'s cells, no overlap; FA paths `a`, `b` do not overlap → proven |
| `conf.exclusive-from-shared-rejected` | `fn g(ref<P, shared> p) { auto w = &mut p.a; }` | ✗ `diag.borrow-exceeds-source` (static) | `[Field-Access-Auto-Deref]` gives a shared projection; `[Borrow-Exceeds-Source]` |
| `conf.write-through-shared-rejected` | `fn g(ref<i32, shared> p) { *p = 1; }` | ✗ `diag.write-through-shared` (static) | `[Write-Not-Exclusive]` |
| `conf.sequential-exclusive-borrows-ok` | `Vec<i32> nums = Vec::new(); Vec::push(&mut nums, 10); Vec::push(&mut nums, 20); Vec::len(&nums)` | `→ 2` | each `&mut nums` is held only by the callee's parameter object; `[Call]`'s BE ends it → the borrow is Dead before the next statement |
| `conf.reference-in-field-survives-statement` | `struct H { ref<i32, shared> r; } i32 x = 1; auto h = H { .r = &x }; *h.r` | `→ 1` | `[Store-Sub-Value]` adds `o_h` to the borrow's `held-by`; SE does not touch held paths; `h.r` read → value `a` → `[Ref-Deref-Place]` → read |
| `conf.borrow-of-temporary-rejected` | `struct P { i32 a; } auto r = &P { .a = 1 }.a;` | ✗ `diag.borrow-of-temporary` (static) | `[Temp-Root]` then `[Ref-Form-Temporary]` |
| `conf.temp-borrow-argument` | `fn total(ref<Vec<i32>, shared> v) : usize { Vec::len(v) }` `fn make() : Vec<i32> { Vec<i32> v = Vec::new(); Vec::push(&mut v, 1); v }` `total(&make())` | `→ 1` | `[Ref-Form-Temporary-Argument]` (D-0073): `make()`'s `Vec` is a temporary of the statement, borrowed for the call; it ends with the statement |
| `conf.temp-borrow-binding-rejected` | `fn make() : Vec<i32> { Vec::new() }` `auto r = &make();` | ✗ `diag.borrow-of-non-place` (static) | not an argument: `[Ref-Form-Not-Place]`, as before |
| `conf.temp-borrow-escape-stale` | `fn first(ref<Vec<i32>, shared> v) : Option<ref<i32, shared>> { if (Vec::len(v) == 0) { None } else { Some(Vec::index_shared(v, 0)) } }` `fn make() : Vec<i32> { Vec<i32> v = Vec::new(); Vec::push(&mut v, 42); v }` `Option<ref<i32, shared>> o = first(&make()); match (o) { Some(r) : { i32 k = *r; }, None : {}, }` | ✗ `diag.stale-binding` (dynamic) | the returned `Option` holds a reference into the temporary, which ended with its statement (D-0073) |
| `conf.temp-borrow-in-condition` | `fn find(ref<Vec<i32>, shared> v, ref<i32, shared> x) : Option<usize> { None } fn make_one() : i32 { 1 }` `Vec<i32> v = Vec::new(); bool c = true; c && Option::is_none(&find(&v, &make_one()))` | `→ true` | temporaries borrowed as arguments inside a short-circuit condition (CHG-0087) |
| `conf.temp-borrow-joined-result` | `fn mk(u32 n) : Vec<u32> { Vec<u32> v = Vec::new(); Vec::push(&mut v, n); v } fn total(ref<Vec<u32>, shared> v) : u32 { v[0] }` `auto h = spawn(mk, 7); total(&join(h))` | `→ 7` | the joined result, made in the other thread, is this statement's temporary and this thread's to destroy (`[Join]` re-keys it, as binding it would; CHG-0089) |
| `conf.temp-borrow-operator-argument` | `fn v(ref<u64, shared> r) : u64 { *r }` `u64 k = 3; v(&((k << 32) \| 7))` | `→ 12884901895` | `[Ref-Form-Temporary-Argument]` (D-0084): an operator's result, borrowed for the call, a temporary of the statement |
| `conf.temp-borrow-literal-typed-by-parameter` | `HashMap<u64, u8> m = HashMap::new(); HashMap::insert(&mut m, 40000000000, 9); *Option::unwrap(HashMap::get(&m, &40000000000))` | `→ 9` | `[Ref-Form-Temporary-Argument]` (D-0084): the literal is evaluated at the parameter's referent type `u64` (`rule.type.expected`) |
| `conf.temp-borrow-literal-binding-rejected` | `ref<i32, shared> r = &5;` | ✗ `diag.borrow-of-non-place` (static) | not an argument: `[Ref-Form-Not-Place]` (D-0084 admits a literal only as an argument) |
| `conf.rc-clone-while-value-borrowed` | `auto a = Rc::new(5); ref<i32, shared> r = Rc::get(&a); auto b = Rc::clone(&a); drop(b); *r` | `→ 5` | `Rc::clone` and `Rc::drop` borrow the box's count only, disjoint from the value `r` refers into (CHG-0100) |
| `conf.vec-from-slice` | `array<u8, 4> a = [1, 2, 3, 4]; Vec<u8> v = Vec::from_slice(&a[1..3]); Vec::len(&v) * 10 + widen<usize>(v[1])` | `→ 23` | `Vec::from_slice` (D-0085): the slice's elements copied in order |
| `conf.vec-from-slice-resource-rejected` | `impl/conformance/21-standard-library-semantics/vec_from_slice_of_resources_rejected.cb` | ✗ `diag.type-mismatch` (static) | an element type that is not `clone` (a `resource` type without a `clone`) cannot be copied (D-0085; `T: clone` since D-0116) |
| `conf.vec-filled` | `Vec<u8> v = Vec::filled(3, 7: u8); Vec::len(&v) * 10 + widen<usize>(v[2])` | `→ 37` | `Vec::filled` (D-0112): `n` copies of a plain `x`; also `impl/conformance/21-standard-library-semantics/vec_helpers_ok.cb` |
| `conf.vec-filled-resource-rejected` | `impl/conformance/21-standard-library-semantics/vec_filled_resource_rejected.cb` | ✗ `diag.type-mismatch` (static) | `x` is copied into every element with `clone`, so a `T` that is not `clone` is rejected at the call (D-0112, D-0116) |
| `conf.vec-from-fn` | `Vec<usize> v = Vec::from_fn(4, [](usize i) : usize { i * i }); v[3] + Vec::len(&v)` | `→ 13` | `Vec::from_fn` (D-0112): `f(0), …, f(n-1)` in order; resources and grids in `vec_helpers_ok.cb` |
| `conf.vec-append` | `impl/conformance/21-standard-library-semantics/vec_helpers_ok.cb` | ok | `Vec::append(&mut a, b)` moves `b`'s elements onto `a` in order and consumes `b` (D-0112) |
| `conf.vec-extend-from` | `Vec<u8> a = Vec::filled(1, 9: u8); array<u8, 3> b = [1, 2, 3]; Vec::extend_from(&mut a, &b[1..3]); Vec::len(&a) * 10 + widen<usize>(a[2])` | `→ 33` | `Vec::extend_from` (D-0112): the slice's elements copied onto the end, in order |
| `conf.view-sub-not-char-boundary` | `impl/conformance/21-standard-library-semantics/view_sub_not_char_boundary_faults.cb` | ✗ `diag.not-char-boundary` (dynamic) | `StringView::sub` ending inside a two-byte character, after a whole-character `sub` of the same view (D-0053) |
| `conf.vec-extend-from-resource-rejected` | `impl/conformance/21-standard-library-semantics/vec_extend_from_resource_rejected.cb` | ✗ `diag.type-mismatch` (static) | copying elements that are not `clone` from a slice is rejected at the call (D-0112, D-0116) |
| `conf.vec-position` | `Vec<i32> v = Vec::filled(3, 5); v[2] = 8; auto big = [](ref<i32, shared> x) : bool { *x > 6 }; match (Vec::position(&v, big)) { Some(i) : i, None : 99, }` | `→ 2` | `Vec::position` (D-0112): the least index whose element satisfies the predicate; `None` when none does (`vec_helpers_ok.cb`) |
| `conf.vec-index-of` | `Vec<String> v = Vec::from_fn(3, [](usize i) : String { sprintf("k%d", i) }); String k = String::from_str("k1"); match (Vec::index_of(&v, &k)) { Some(i) : i, None : 99, }` | `→ 1` | `Vec::index_of` (D-0112): the least index whose element `== *x`, a `String` compared in place (D-0108) |
| `conf.vec-contains` | `Vec<u8> v = Vec::filled(2, 4: u8); Vec::contains(&v, &(4: u8)) && !Vec::contains(&v, &(5: u8))` | `→ true` | `Vec::contains` (D-0112): whether `index_of` finds the value |
| `conf.vec-contains-not-eq-rejected` | `impl/conformance/21-standard-library-semantics/vec_contains_not_eq_rejected.cb` | ✗ `diag.type-mismatch` (static) | `contains`/`index_of` need `T: eq`, which no struct satisfies (D-0090); `[Bound-Unsatisfied]` at the call |
| `conf.vec-reverse` | `Vec<u8> v = Vec::from_fn(4, [](usize i) : u8 { narrow<u8>(i) }); Vec::reverse(&mut v); widen<usize>(v[0]) * 10 + widen<usize>(v[3])` | `→ 30` | `Vec::reverse` (D-0112): the order inverted in place; resources in `vec_helpers_ok.cb` (nothing destroyed) |
| `conf.closure-compares-captured-string` | `impl/conformance/15-function-semantics/closure_compares_captured_string_ok.cb` | ok | `*t == key` on a `String` captured by borrow, inside the closure: D-0108's in-place comparison applies in a closure body as outside one (CHG-0134) |
| `conf.slice-argument-held-across-push-rejected` | `impl/conformance/16-aggregates/slice_argument_held_across_push_rejected.cb` | ✗ `diag.aliasing-conflict` (dynamic) | `grow(&mut a, &a[0..2])`: both borrows pending until bound (D-0107); the first `Vec::push` through the exclusive one reaches the whole `Vec` and clashes with the live slice (`[Slice-Form]`, CHG-0134) |
| `conf.vec-extend-from-alias-rejected` | `impl/conformance/21-standard-library-semantics/vec_extend_from_alias_rejected.cb` | ✗ `diag.aliasing-conflict` (dynamic) | `std`'s own `Vec::extend_from(&mut a, &a[0..2])`: the slice's first element is read while the exclusive borrow is live (as `conf.slice-argument-held-across-push-rejected`); an empty slice reads nothing |
| `conf.vec-filled-element-kinds` | `impl/conformance/21-standard-library-semantics/vec_filled_element_kinds_ok.cb` | ok; prints the five lines its header gives | `Vec::filled(n, x)` is `n` clones of `x` for plain element types of every kind, for `n = 0`, and for a resource (`String`); the vector grows and is written afterwards as any other (a shape `cobc`'s native body must keep; no rule changed) |
| `conf.vec-element-moves` | `impl/conformance/21-standard-library-semantics/vec_element_moves_ok.cb` | ok; prints what its header gives | `swap`, `reverse`, `insert`, `truncate`, `clone`, `append`, `binary_search`, `extend_from` and `reserve` on plain elements at their edges, on elements borrowed before and through growth, and on a struct element type (shapes `cobc`'s native bodies must keep; no rule changed) |
| `conf.vec-swap-held-element` | `impl/conformance/21-standard-library-semantics/vec_swap_held_element_rejected.cb` | ✗ `diag.aliasing-conflict` (dynamic) | a shared reference to `v[0]`, live where the analysis cannot follow it; `Vec::swap(&mut v, 0, 1)`'s exclusive borrow of `v[0]` clashes (`[Borrow-Denied]`) |
| `conf.vec-swap-beside-held-element` | `impl/conformance/21-standard-library-semantics/vec_swap_beside_held_element_ok.cb` | ok | the same reference beside `Vec::swap(&mut v, 1, 2)`: neither borrowed element is `v[0]` |
| `conf.vec-insert-held-element` | `impl/conformance/21-standard-library-semantics/vec_insert_held_element_rejected.cb` | ✗ `diag.aliasing-conflict` (dynamic) | `Vec::insert(&mut v, 0, 5)` with spare capacity moves `v[0]` by a swap whose borrow clashes with the live reference; with growth the reference would be stale instead |
| `conf.vec-index-of-held-element` | `impl/conformance/21-standard-library-semantics/vec_index_of_held_element_rejected.cb` | ✗ `diag.aliasing-conflict` (dynamic) | `Vec::index_of` reads `v[0]` while an exclusive reference to it lives (`[Read-Conflict]`); `cobc`'s native `index_of` missed this before 2026-10-08 |
| `conf.vec-clone-held-element` | `impl/conformance/21-standard-library-semantics/vec_clone_held_element_rejected.cb` | ✗ `diag.aliasing-conflict` (dynamic) | `Vec::clone` reads `v[0]` while an exclusive reference to it lives |
| `conf.vec-reverse-held-element` | `impl/conformance/21-standard-library-semantics/vec_reverse_held_element_rejected.cb` | ✗ `diag.aliasing-conflict` (dynamic) | `Vec::reverse` swaps `v[0]` while an exclusive reference to it lives |
| `conf.slice-read-beside-held-element` | `impl/conformance/08-alias-validity/slice_read_beside_held_element_rejected.cb` | ✗ `diag.aliasing-conflict` (dynamic) | a slice over `v[0..4]` read while an exclusive reference to `v[1]` lives: the read of `s[1]` meets the element's own object and its path (`[Read-Conflict]`); the slice's check concerns the slice's object alone (`spec/08` §4) |
| `conf.slice-write-over-held-shared-element` | `impl/conformance/08-alias-validity/slice_write_over_held_shared_element_rejected.cb` | ✗ `diag.aliasing-conflict` (dynamic) | a shared reference to `v[1]`: reads through a slice and writes to `v[2..4]` pass; a write to `v[0..2]` reaches `v[1]` (`[Write-Conflict]`) |
| `conf.vec-from-slice-held-element` | `impl/conformance/21-standard-library-semantics/vec_from_slice_held_element_rejected.cb` | ✗ `diag.aliasing-conflict` (dynamic) | `Vec::from_slice(&v[0..2])` reads `v[0]` while an exclusive reference to it lives |
| `conf.vec-extend-from-held-element` | `impl/conformance/21-standard-library-semantics/vec_extend_from_held_element_rejected.cb` | ✗ `diag.aliasing-conflict` (dynamic) | `Vec::extend_from(&mut c, &v[0..2])` reads `v[1]` while an exclusive reference to it lives |
| `conf.vec-struct-field-loops` | `impl/conformance/21-standard-library-semantics/vec_struct_field_loops_ok.cb` | ok; prints what its header gives | functions looping over the `Vec` fields of a struct given by reference (indexing, `push`, `pop`, `insert`, `swap`, `reverse`, `truncate`, `len`) beside its other fields, for a local struct and one inside a `Vec` (`cobc`'s D-0189 dual bodies must leave what the checked ones leave) |
| `conf.vec-struct-field-held-element` | `impl/conformance/21-standard-library-semantics/vec_struct_field_held_element_rejected.cb` | ✗ `diag.aliasing-conflict` (dynamic) | an exclusive reference to `g.cells[2]` live while `sum(&mut g)` reads `g.cells` in a loop: the read of `g.cells[2]` meets it (`[Read-Conflict]`); D-0189's entry test must find the element's object |
| `conf.vec-local-calls` | `impl/conformance/21-standard-library-semantics/vec_local_calls_ok.cb` | ok; prints what its header gives | `swap`, `reverse`, `insert`, `remove`, `truncate`, `clear`, `contains`, `index_of`, `binary_search`, `sort` on local vectors and through a reference parameter, plain and struct elements (calls that keep a vector confined in `cobc`, D-0189) |
| `conf.vec-search-keys` | `impl/conformance/21-standard-library-semantics/vec_search_keys_ok.cb` | ok; prints what its header gives | literal and computed keys for `contains`, `index_of`, `binary_search` over `u8`, `i64`, `u16`, `i8` vectors: a literal key has the element type |
| `conf.vec-sort-by-through-parameter` | `impl/conformance/21-standard-library-semantics/vec_sort_by_through_parameter_ok.cb` | ok; prints what its header gives | `Vec::sort_by` with a printing closure literal on a vector reached through `ref<Vec<i64>, exclusive>`: the prelude's order of comparisons, a stable result |
| `conf.vec-predicate-literals` | `impl/conformance/21-standard-library-semantics/vec_predicates_ok_then_fault.cb` | ✗ `diag.div-by-zero` (dynamic); prints what its header gives first | `retain` and `position` with closure literals (one printing each element in order), on a local and through a parameter, `dedup` over runs and an empty vector, then a predicate dividing by zero on the first element: the fault inside `retain`, after all that came before |
| `conf.vec-retain-held-element` | `impl/conformance/21-standard-library-semantics/vec_retain_held_element_rejected.cb` | ✗ `diag.aliasing-conflict` (dynamic) | `Vec::retain` moves the kept `v[4]` by a swap whose exclusive borrow meets a live shared reference to it |
| `conf.vec-dedup-held-element` | `impl/conformance/21-standard-library-semantics/vec_dedup_held_element_rejected.cb` | ✗ `diag.aliasing-conflict` (dynamic) | `Vec::dedup` moves a kept element into `v[2]`'s place while a shared reference to `v[2]` lives |
| `conf.vec-position-held-element` | `impl/conformance/21-standard-library-semantics/vec_position_held_element_rejected.cb` | ✗ `diag.aliasing-conflict` (dynamic) | `Vec::position` borrows `v[3]` for its predicate while an exclusive reference to it lives |
| `conf.vec-struct-in-vec-loops` | `impl/conformance/21-standard-library-semantics/vec_struct_in_vec_loops_ok.cb` | ok; prints what its header gives | a looping function over a struct's `Vec` field, called on the elements of a `Vec` of such structs, its plain field updated in the loop (D-0190) |
| `conf.vec-struct-in-vec-slice-held` | `impl/conformance/21-standard-library-semantics/vec_struct_in_vec_slice_held_rejected.cb` | ✗ `diag.aliasing-conflict` (dynamic); prints `first 6` first | a shared slice over `bags[0..2]` lives when `&mut bags[1]` is formed for the call (`[Borrow-Denied]`) |
| `conf.vec-struct-in-vec-inner-held` | `impl/conformance/21-standard-library-semantics/vec_struct_in_vec_inner_held_rejected.cb` | ✗ `diag.aliasing-conflict` (dynamic) | an exclusive reference to `bags[0].items[1]` lives while `work(&mut bags[0])` reads the field (`[Read-Conflict]`) |
| `conf.vec-foreach-over-parameter` | `impl/conformance/21-standard-library-semantics/vec_foreach_over_parameter_ok.cb` | ok; prints what its header gives | `foreach (x in v)` over exclusive and shared `Vec` reference parameters: `continue`, `break`, empty, nested over the same vector |
| `conf.vec-capturing-predicates` | `impl/conformance/21-standard-library-semantics/vec_capturing_predicates_ok.cb` | ok; prints what its header gives | `retain` and `position` with capturing closure literals: a value and a `String` read, a counter written by exclusive capture, each element given in order |
| `conf.vec-capturing-predicate-held-element` | `impl/conformance/21-standard-library-semantics/vec_capturing_predicate_held_element_rejected.cb` | ✗ `diag.aliasing-conflict` (dynamic); prints `false` first | a capturing predicate writing through its capture, then `retain` over a vector whose `v[3]` is held exclusively: the predicate's borrow of `v[3]` clashes |
| `conf.vec-predicate-values` | `impl/conformance/21-standard-library-semantics/vec_predicate_values_ok.cb` | ok; prints what its header gives | named functions, `fn` values holding a named function or a closure (one reassigned between calls), and `binary_search_by` with a named function, a `fn` value and a literal, keys a place and a temporary: the prelude's order of calls (D-0191) |
| `conf.vec-predicate-value-keeps-element` | `impl/conformance/21-standard-library-semantics/vec_predicate_value_keeps_element_rejected.cb` | ✗ `diag.aliasing-conflict` (dynamic); prints `true` first | a `fn` value whose closure keeps the element references it is given: the kept reference reads its element; a push onto the vector while it lives clashes |
| `conf.vec-retain-fn-value-held-element` | `impl/conformance/21-standard-library-semantics/vec_retain_fn_value_held_element_rejected.cb` | ✗ `diag.aliasing-conflict` (dynamic) | `retain` with a `fn` value holding a named function: the borrow of `v[3]` for the predicate meets a live exclusive reference to it |
| `conf.vec-binary-search-by-held-element` | `impl/conformance/21-standard-library-semantics/vec_binary_search_by_held_element_rejected.cb` | ✗ `diag.aliasing-conflict` (dynamic); prints `true` first | `binary_search_by` with a named function: a search whose halving never borrows the held `v[3]` completes; one that does clashes |
| `conf.vec-resource-elements` | `impl/conformance/21-standard-library-semantics/vec_resource_elements_ok.cb` | ok; prints what its header gives | `Vec<String>` and a `Vec` of a resource with a destructor through `swap`, `reverse`, `insert` (front, middle, end), `remove`, `pop` (`Some` and `None`), `truncate` and `clear`: each destructor runs where the prelude's bodies run it, last element first for `truncate` and `clear` (D-0192: `cobc`'s native forms for movable elements) |
| `conf.vec-string-search` | `impl/conformance/21-standard-library-semantics/vec_string_search_ok.cb` | ok; prints what its header gives | `contains`, `index_of` and `binary_search` over `Vec<String>`, keys a place and temporaries, empty vectors and empty strings; `Vec::clone` of `String`s (D-0192) |
| `conf.vec-resource-predicates` | `impl/conformance/21-standard-library-semantics/vec_resource_predicates_ok.cb` | ok; prints what its header gives | `retain`, `position` and `binary_search_by` literals over `Vec<String>` and a resource `Vec`: the prelude's order of calls, a capture read, the dropped elements' destructors (D-0192) |
| `conf.vec-string-clone` | `impl/conformance/21-standard-library-semantics/vec_string_clone_ok.cb` | ok; prints `[alpha][][n42]`, `[alpha!][x][n42][extra]`, `3 4`, `0` | `Vec::clone` of `String`s is deep: writes through the copy leave the original unchanged (D-0192) |
| `conf.vec-from-fn-literal` | `impl/conformance/21-standard-library-semantics/vec_from_fn_literal_rejected.cb` | ✗ `diag.narrowing-overflow` (dynamic); prints `b0 b1 b2 b3 ` last | `Vec::from_fn` with closure literals (capturing, a capture written per call, struct elements, length 0, 100000 elements); the last closure's `narrow<u8>` faults in its fourth call, after that call printed `b3` (D-0192: `cobc`'s inline loop) |
| `conf.vec-capturing-closure-values` | `impl/conformance/21-standard-library-semantics/vec_capturing_closure_values_ok.cb` | ok; prints what its header gives | capturing closures held in `fn` locals as `retain`, `position` and `binary_search_by` predicates, one reassigned between calls; a closure capturing a struct that holds a `Vec` (D-0192) |
| `conf.vec-search-key-places` | `impl/conformance/21-standard-library-semantics/vec_search_key_places_ok.cb` | ok; prints what its header gives | search keys that are fields, elements of another `Vec`, an element of the searched `Vec`, locals in a loop, searched before and after the `Vec` is handed to `sort_by` (D-0192: keys passed by address) |
| `conf.text-split-trim-case` | `impl/conformance/21-standard-library-semantics/text_split_trim_case_ok.cb` | ok; prints what its header gives | `split_whitespace`, `trim`, `trim_start`, `trim_end`, `to_ascii_lower`/`upper` over every ASCII white-space byte, UTF-8, empty and blank text; `foreach` consuming a `Vec` of views (kept past it) and of numbers (left early, the rest dropped with it) (D-0193: `cobc`'s native forms and `Drain::take`) |
| `conf.text-chars-count` | `impl/conformance/21-standard-library-semantics/text_chars_count_rejected.cb` | ✗ `diag.not-char-boundary` (dynamic); prints what its header gives first | `StringView::chars` and `char_count` over empty, ASCII, 2-, 3- and 4-byte characters and control bytes; `String::char_count` and `chars` agree; `String::view` cut inside a character faults (D-0193) |
| `conf.text-parse-int-edges` | `impl/conformance/21-standard-library-semantics/text_parse_int_edges_ok.cb` | ok; prints what its header gives | `StringView::parse` and `String::parse` at `u8` … `u128`, `i8` … `i128`, `isize`: limits and one past them, `+` and `-`, empty text, white space, `_`, invalid bytes and their position (D-0193) |
| `conf.process-child-pipes` | `impl/conformance/21-standard-library-semantics/process_child_pipes_ok.cb` | ok (Unix); prints what its header gives | `[Child-Pipes]`: `seq`'s output pumped into `sort`'s input by a thread while this one reads `sort`; each end taken once (`None` after); an output end read after its `Child` was dropped (D-0195) |
| `conf.http-keep-alive` | `impl/conformance/21-standard-library-semantics/http_keep_alive_ok.cb` | ok; prints what its header gives | `[Http-Keep-Alive]`: ten requests take ten connections without `keep_alive`, four with it against a server that closes each after three, the closed kept connection replaced without the request failing (D-0195) |
| `conf.objectless-strings-by-binding` | `impl/conformance/21-standard-library-semantics/objectless_strings_by_binding_rejected.cb` | ✗ `diag.index-out-of-bounds` (dynamic); prints what its header gives first | `String` locals of the same name in several blocks, built with the `std` builders and returned from several places, a function whose result is not its own local, `get_str`, `contains_str`, `remove_str`; `truncate` past the end faults (D-0196) |
| `conf.string-builders-raw-return` | `impl/conformance/21-standard-library-semantics/string_builders_raw_return_rejected.cb` | ✗ `diag.not-char-boundary` (dynamic); prints what its header gives first | a `String` built by every `std` builder and returned to a caller that binds it; `truncate` inside a character faults (D-0196) |
| `conf.vec-swap-remove` | `impl/conformance/21-standard-library-semantics/vec_swap_remove_response_text_rejected.cb` | ✗ `diag.index-out-of-bounds` (dynamic); prints what its header gives first | `Vec::swap_remove` on numbers, resources and a one-element `Vec`; `Response::text` of UTF-8 and of bytes that are not; `swap_remove` past the end (D-0193) |
| `conf.priority-queue-order` | `impl/conformance/21-standard-library-semantics/priority_queue_order_ok.cb` | ok; prints what its header gives | a queue whose order prints each comparison, equal keys first in first out; `i8`, `f64` and 1000 `u64` keys (D-0193: `cobc`'s native `push`/`pop` make the same comparisons in the same order) |
| `conf.state-and-input-arguments-disjoint` | `impl/conformance/08-alias-validity/state_and_input_arguments_disjoint_ok.cb` | ok; prints `7 2`, `2 1`, `8`, `36`, `84`, `16` | one exclusive reference beside shared ones, all to plain data, in each disjoint shape (a `Vec` slice and a struct, two disjoint fields, a field and its sibling's slice, through a fn value, in a spawned thread, an array sub-range): no `clash` at any access (D-0188: `cobc`'s dual bodies run unchecked here) |
| `conf.state-and-own-field-input-rejected` | `impl/conformance/08-alias-validity/state_and_own_field_input_rejected.cb` | ✗ `diag.aliasing-conflict` (dynamic); prints `4 9` first | `copy_in(&mut x, &x.b[0..4])`: both pending until bound (D-0107); the first read of `s[i]` clashes with the live exclusive path to `x` (`[Read-Conflict]`); the dual body's entry test finds the overlap and the checked body faults (D-0188) |
| `conf.state-and-field-of-state-rejected` | `impl/conformance/08-alias-validity/state_and_field_of_state_rejected.cb` | ✗ `diag.aliasing-conflict` (dynamic) | `bump_all(&mut p, &p.n)`: a shared field reference beside the exclusive whole; the body's first access clashes (D-0188) |
| `conf.overlapping-slice-arguments-rejected` | `impl/conformance/08-alias-validity/overlapping_slice_arguments_rejected.cb` | ✗ `diag.aliasing-conflict` (dynamic); prints `0 10` first | `mix(&mut v[0..4], &v[4..8])` completes (disjoint ranges); `mix(&mut v[0..4], &v[2..6])` clashes at its first access (`[Slice-Form]` ranges overlap; D-0188) |
| `conf.dual-body-bare-call` | `impl/conformance/15-function-semantics/dual_body_bare_call_rejected.cb` | ✗ `diag.index-out-of-bounds` (dynamic); prints `7461 331 10` last | a dual-body function called with a struct local and slices of a distinct `Vec` and array (the call site proves them disjoint: D-0192's bare form), once with an element of the `Vec` held across the call; `&data[60..70]` of 64 elements faults at the call |
| `conf.closure-names-across-functions` | `impl/conformance/15-function-semantics/closure_names_across_functions_ok.cb` | ok; prints `7 8` | two functions whose first closures have different result types: each call runs its own closure (D-0193: `cobc` numbered closures per function, and the two shared a C name) |
| `conf.closure-captures-closure` | `impl/conformance/15-function-semantics/closure_captures_closure_ok.cb` | ok; prints `direct true`, `wrapped true`, `local true` | a closure from a function, and a local capturing closure, each captured by a `move` closure that calls it: the inner closure's captures keep their types (D-0193: `coby` lost them) |
| `conf.closure-captured-owner-destructors` | `impl/conformance/15-function-semantics/closure_captured_owner_destructors_ok.cb` | ok; prints what its header gives | closures capturing closures, some owning a resource: those owning nothing are copied (each copy its own closures), those owning one move (D-0089 through nesting), and each destructor runs once, when its last holder ends (D-0193) |
| `conf.closure-combinator-loop` | `impl/conformance/15-function-semantics/closure_combinator_loop_ok.cb` | ok; prints `step 0`, `step 1`, `true` | `keep = both(keep, c)` in a loop, `c` owning a `String`: the previous value moves into the new closure each time (D-0193) |
| `conf.string-append-own-view-rejected` | `impl/conformance/21-standard-library-semantics/string_append_own_view_rejected.cb` | ✗ `diag.aliasing-conflict` (dynamic) | `String::append(&mut s, String::as_view(&s))`: the view's path is live while the append writes `s`; through `String::clone` it appends |
| `conf.string-view-functions` | `impl/conformance/21-standard-library-semantics/string_view_functions_ok.cb` | ok | `String::find`, `starts_with`, `ends_with`, `trim`, `trim_start`, `trim_end`, `as_view`: each the `StringView::` function applied to `&s[0..$]` (D-0113) |
| `conf.string-split` | `String s = String::from_str("a,b,,c"); Vec::len(&String::split(&s, ","))` | `→ 4` | `String::split` (D-0113): the parts of the whole-string view, empty fields kept |
| `conf.map-lookup-by-str` | `impl/conformance/21-standard-library-semantics/map_lookup_by_str_ok.cb` | ok | `HashMap::get_str`, `get_mut_str`, `contains_str` on a `String`-keyed map find the entry a `String` of the same bytes would: `[Lookup-Str]` (D-0113) |
| `conf.map-remove-str` | `impl/conformance/21-standard-library-semantics/map_lookup_by_str_ok.cb` | ok | `HashMap::remove_str` removes that entry and moves the last entry into its place, as `remove` does (D-0113) |
| `conf.set-lookup-by-str` | `impl/conformance/21-standard-library-semantics/map_lookup_by_str_ok.cb` | ok | `HashSet::contains_str`, `remove_str` (D-0113) |
| `conf.string-chars` | `impl/conformance/21-standard-library-semantics/string_chars_ok.cb` | ok | `String::chars`: one view per character, in order, one to four bytes each, together the whole text; they compare, match and print as text (D-0128) |
| `conf.stringview-chars` | `impl/conformance/21-standard-library-semantics/string_chars_ok.cb` | ok | `StringView::chars` of a view (a split word) (D-0128) |
| `conf.string-chars-empty` | `impl/conformance/21-standard-library-semantics/string_chars_ok.cb` | ok | empty text has no characters: an empty `Vec` (D-0128) |
| `conf.string-chars-borrows` | `impl/conformance/21-standard-library-semantics/string_chars_borrow_rejected.cb` | ✗ `diag.aliasing-conflict` (dynamic) | the views borrow the `String`, as any view does: it cannot be changed while they are in use (D-0128) |
| `conf.vec-reserve` | `impl/conformance/21-standard-library-semantics/vec_reserve_ok.cb` | ok | `Vec::reserve(&mut v, n)` makes room for `n` more; the length and elements are unchanged, and pushes after it work as before (D-0129) |
| `conf.vec-reserve-keeps` | `impl/conformance/21-standard-library-semantics/vec_reserve_ok.cb` | ok | reserving room already there, or none, changes nothing (D-0129) |
| `conf.vec-reserve-overflow` | `impl/conformance/21-standard-library-semantics/vec_reserve_overflow_rejected.cb` | ✗ `diag.arith-overflow` (dynamic) | room for more than `usize` can count overflows `len + n`, before anything is allocated (D-0129) |
| `conf.vec-reserve-stale` | `impl/conformance/21-standard-library-semantics/vec_reserve_stale_rejected.cb` | ✗ `diag.stale-binding` (dynamic) | when `reserve` grows a `Vec`, its elements move: a reference into the old cells is stale afterwards, as after a growing `push` (D-0129) |
| `conf.string-reserve` | `impl/conformance/21-standard-library-semantics/vec_reserve_ok.cb` | ok | `String::reserve(&mut s, n)` makes room for `n` more bytes; the text is unchanged (D-0129) |
| `conf.map-get-str-non-string-key-rejected` | `impl/conformance/21-standard-library-semantics/map_get_str_non_string_key_rejected.cb` | ✗ `diag.type-mismatch` (static) | a `_str` lookup on a map not keyed by `String`: the parameter's `String` is checked as written (D-0113, CHG-0136) |
| `conf.generic-concrete-argument-checked` | `impl/conformance/15-function-semantics/generic_concrete_argument_checked_rejected.cb` | ✗ `diag.type-mismatch` (static) | `k<B>(P<String, B>)` with a `P<u64, i32>`: `B` is inferred, `String` against `u64` is a mismatch (`[T-Call]`, D-0006; CHG-0136) |
| `conf.generic-literal-argument-checked` | `impl/conformance/15-function-semantics/generic_literal_argument_checked_rejected.cb` | ✗ `diag.type-mismatch` (static) | `Option::unwrap(5: u64)` where a `u64` is expected: the literal is checked against `Option<u64>` (`[T-Call]`, D-0006; CHG-0233) |
| `conf.if-pattern-some` | `impl/conformance/14-control-flow/pattern_conditions_ok.cb` | ok | `if (Some(x) = o) b1 else b2` is `match (o) { Some(x) : b1, _ : b2 }`: `[If-Pattern]` (D-0115) |
| `conf.if-pattern-else-value` | `Option<i32> o = Some(4); if (Some(x) = o) { x * 2 } else { 0 }` | `→ 8` | the `if` is the `match`'s value; `x` is bound in the first block only (D-0115) |
| `conf.if-pattern-nested` | `impl/conformance/14-control-flow/pattern_conditions_ok.cb` | ok | `Ok(Some(n))`, `Ok(None)`, `Err(_)` as conditions: the full pattern language (D-0056, D-0109) |
| `conf.if-pattern-literal` | `u32 k = 3; if (3 = k) { k = 10; } k` | `→ 10` | a literal pattern as a condition (D-0057, D-0115) |
| `conf.if-pattern-text` | `str c = "quit"; if ("quit" = c) { 1 } else { 0 }` | `→ 1` | a text literal pattern as a condition (D-0127, D-0115) |
| `conf.if-pattern-qualified-variant` | `impl/conformance/14-control-flow/pattern_conditions_ok.cb` | ok | `shapes::Disc(rad)` and `Disc(rad)` as conditions |
| `conf.if-pattern-binder-scope-rejected` | `impl/conformance/14-control-flow/if_pattern_binder_scope_rejected.cb` | ✗ `diag.unbound-name` (static) | the binder does not outlive the block (D-0115) |
| `conf.if-assignment-still-rejected` | `impl/conformance/14-control-flow/if_assignment_still_rejected.cb` | ✗ `diag.type-mismatch` (static) | `if (x = y)`: a bare name is the assignment, whose value is `unit`, not a pattern (D-0115, `[T-If]`) |
| `conf.while-pattern-pop-moves` | `impl/conformance/14-control-flow/pattern_conditions_ok.cb` | ok | `while (Some(top) = Vec::pop(&mut v))`: each iteration moves the element out and destroys it at the body's end; the loop ends at `None` (`[While-Pattern]`) |
| `conf.while-pattern-break-continue` | `impl/conformance/14-control-flow/pattern_conditions_ok.cb` | ok | `break` and `continue` in the body belong to the pattern loop (D-0115) |
| `conf.clone-plain` | `impl/conformance/12-type-system/clone_bound_ok.cb` | ok | `clone(&p)` of a plain struct is a read; the copy is independent (`[T-Clone]`, D-0116) |
| `conf.clone-derived-struct` | `impl/conformance/12-type-system/clone_bound_ok.cb` | ok | a struct with a `String` field is copied field by field by its derived `clone` (`rule.agg.derived-clone`) |
| `conf.clone-derived-enum` | `impl/conformance/12-type-system/clone_bound_ok.cb` | ok | an enum's derived `clone`: the same variant with its payload copied; an `Rc` payload shares its cell |
| `conf.clone-generic-struct` | `impl/conformance/12-type-system/clone_bound_ok.cb` | ok | `Pair<String, i32>` and `Option<String>` are `clone` by derivation at those arguments |
| `conf.clone-std-containers` | `impl/conformance/12-type-system/clone_bound_ok.cb` | ok | `Vec<String>`, `Vec<Vec<u8>>` (`Vec::filled`), `Box<String>`, `HashMap<String, Vec<u8>>`, `HashSet<String>` copied by `clone` |
| `conf.clone-declared-called` | `impl/conformance/12-type-system/clone_bound_ok.cb` | ok | a `resource` type's own `Counter::clone` is what `clone` and `Vec::filled` call, observably; `Rc::clone` shares |
| `conf.clone-bound-generic-fn` | `impl/conformance/12-type-system/clone_bound_ok.cb` | ok | `fn dup<T: clone>(ref<T, shared> x) : T { clone(x) }` at a derived struct |
| `conf.clone-resource-without-clone-rejected` | `impl/conformance/12-type-system/clone_resource_without_clone_rejected.cb` | ✗ `diag.type-mismatch` (static) | a `resource` type with no `clone` is not `clone` (`[Clone-Unsatisfied]`) |
| `conf.clone-field-not-clone-rejected` | `impl/conformance/12-type-system/clone_field_not_clone_rejected.cb` | ✗ `diag.type-mismatch` (static) | a struct with a field that is not `clone` has no derived `clone`; the message names the field |
| `conf.clone-unbounded-parameter-rejected` | `impl/conformance/12-type-system/clone_unbounded_parameter_rejected.cb` | ✗ `diag.type-mismatch` (static) | `clone(x)` on a `T` not declared `clone` (D-0116) |
| `conf.clone-mutex-rejected` | `impl/conformance/12-type-system/clone_mutex_rejected.cb` | ✗ `diag.type-mismatch` (static) | a `mutex` has no clone |
| `conf.bitstruct-fields` | `impl/conformance/16-aggregates/bitstruct_ok.cb` | ok | fields low bit first, each the smallest unsigned type of its width; a write changes only its bits (`[Bitfield-Read]`, `[Bitfield-Write]`, D-0118) |
| `conf.bitstruct-bits-roundtrip` | `bitstruct B : u8 { lo : 4; hi : 4; } B b = B { .lo = 3, .hi = 2 }; widen<usize>(B::bits(b)) * 100 + widen<usize>(B::from_bits(0xA5).lo)` | `→ 3505` | `bits` packs low bit first (3 \| 2 << 4 = 35); `from_bits` is total and its inverse (D-0118) |
| `conf.bitstruct-raw-image` | `impl/conformance/16-aggregates/bitstruct_ok.cb` | ok | `[Repr-Bitstruct]`: through a `rawptr<u32>` the value is its backing integer, and a write to the integer is seen by the fields |
| `conf.bitstruct-through-reference` | `impl/conformance/16-aggregates/bitstruct_ok.cb` | ok | reads and writes through `ref<B, exclusive>`, a guard and a `Vec` element; `sizeof` is the backing type's |
| `conf.bitstruct-key` | `impl/conformance/16-aggregates/bitstruct_ok.cb` | ok | a bitstruct is a key type by its bits (`[Key-Bytes]`); `==` compares bits |
| `conf.bitstruct-literal-overflow-rejected` | `impl/conformance/16-aggregates/bitstruct_literal_overflow_rejected.cb` | ✗ `diag.narrowing-overflow` (static) | `.lo = 16` into 4 bits (`[Bitfield-Overflow]`, refuted) |
| `conf.bitstruct-write-overflow-faults` | `impl/conformance/16-aggregates/bitstruct_write_overflow_faults.cb` | ✗ `diag.narrowing-overflow` (dynamic) | `b.lo = value(16)`: 15 fits, 16 faults at the write |
| `conf.bitstruct-field-borrow-rejected` | `impl/conformance/16-aggregates/bitstruct_field_borrow_rejected.cb` | ✗ `diag.borrow-of-non-place` (static) | `&b.lo`: a run of bits has no address (`[Bitfield-No-Place]`) |
| `conf.bitstruct-fill-rejected` | `impl/conformance/16-aggregates/bitstruct_fill_rejected.cb` | ✗ `diag.syntax-error` (static) | `bitstruct B : u8 { lo : 4; hi : 3; }` fills 7 of 8 bits |
| `conf.bitstruct-destructure-rejected` | `impl/conformance/16-aggregates/bitstruct_destructure_rejected.cb` | ✗ `diag.type-mismatch` (static) | `B { lo, hi } = b` (`[Bitstruct-Whole]`) |
| `conf.vec-truncate` | `Vec<i32> v = Vec::new(); Vec::push(&mut v, 1); Vec::push(&mut v, 2); Vec::push(&mut v, 3); Vec::truncate(&mut v, 1); Vec::len(&v)` | `→ 1` | `Vec::truncate` (D-0085): the elements past `n` destroyed |
| `conf.vec-truncate-beyond-length` | `Vec<i32> v = Vec::new(); Vec::push(&mut v, 7); Vec::truncate(&mut v, 3); Vec::len(&v)` | `→ 1` | `n` beyond the length keeps every element (D-0085) |
| `conf.generic-call-extra-type-arguments-rejected` | `fn f(i32 x) : i32 { x }` `f<i64>(1)` | ✗ `diag.type-mismatch` (static) | `[Generic-Call-Arity]` (CHG-0100): type arguments a function does not take |
| `conf.field-of-enum-rejected` | `Option<i32> o = Some(1); o.unwrap()` | ✗ `diag.type-mismatch` (static) | `[T-Field]` (CHG-0100): an enum value has no fields; there is no method-call syntax |
| `conf.closure-capture-written-while-held-rejected` | `i32 count = 0; auto bump = [count]() : void { count += 1; }; count = 10; bump();` | ✗ `diag.aliasing-conflict` (static) | `bump` borrows `count` exclusively while it exists (spec/15 §6); spec/14's `deriv(bump, count, exclusive)` (CHG-0100) refutes the write |
| `conf.map-value-held-across-insert-rejected` | `HashMap<u32, String> m = HashMap::new(); HashMap::insert(&mut m, 1, String::from_str("one")); ref<String, shared> r = Option::unwrap(HashMap::get(&m, &1)); HashMap::insert(&mut m, 2, String::from_str("two")); String::len(r)` | ✗ `diag.aliasing-conflict` (static) | D-0088: `r` borrows `m`, the first reference argument of `HashMap::get` |
| `conf.map-key-written-while-value-held` | `impl/conformance/14-control-flow/map_key_written_while_value_held_ok.cb` | `ok` | D-0088: the result borrows the map, not the key |
| `conf.fn-value-copy-independent` | `impl/conformance/15-function-semantics/fn_value_copy_is_independent_ok.cb` | `ok` | `[Fn-Value-Read]` (D-0089): a copy of a `fn` value has its own closure |
| `conf.fn-value-owning-closure-moves` | `impl/conformance/15-function-semantics/fn_value_owning_closure_moves_ok.cb` | `ok` | `[Fn-Value-Read]`: a closure owning a `String` moves out of a field and into a `Vec<fn…>` |
| `conf.fn-value-emptied-call` | `impl/conformance/15-function-semantics/fn_value_emptied_call_faults.cb` | ✗ `diag.stale-binding` (dynamic) | `[Fn-Value-Empty-Call]` (D-0089): `g = f` moved `f`'s closure, which owns a `String` |
| `conf.bound-ordered` | `impl/conformance/12-type-system/bounded_type_parameters_ok.cb` | `ok` | `[T-Bound-Op]` (D-0090): comparisons on an `ordered` `T`, arithmetic on a `number` one, bitwise on an `integer` one, `==` on an `eq` one |
| `conf.bound-number-literal-float` | `fn z<T: number>() : T { T x = 0; x }` `z<f64>() == 0.0` | `→ true` | `[T-Bound-Literal]`: the literal at a floating-point `T` is that value as a float |
| `conf.bound-missing-rejected` | `fn add<T: ordered>(T a, T b) : T { a + b }` `add(1, 2)` | ✗ `diag.unbounded-type-parameter` (static) | `[Bound-Missing]`: `+` needs `number` |
| `conf.bound-unsatisfied-rejected` | `fn larger<T: ordered>(T a, T b) : T { if (a > b) { a } else { b } }` `larger(Some(1), Some(2))` | ✗ `diag.type-mismatch` (static) | `[Bound-Unsatisfied]`: `Option<i32>` is not `ordered` |
| `conf.bound-ordered-text` | `fn larger<T: ordered>(T a, T b) : T { if (a > b) { a } else { b } }` `String w = larger(String::from_str("pear"), String::from_str("apple")); String::eq_str(&w, "pear") && larger("a", "b") == "b" && larger(false, true)` | `→ true` | `rule.type.bound` (D-0108): `String`, `str` and `bool` are `ordered`; the instantiation at `String` compares in place and moves the larger out |
| `conf.first-error-in-source-order-rejected` | `impl/conformance/12-type-system/first_error_in_source_order_rejected.cb` | ✗ `diag.type-mismatch` (static) | registry reporting order (`CHG-0130`): `helper`'s mismatch (line 15) is reported, not `main`'s later use of a moved binding |
| `conf.variant-temp-borrow-argument` | `fn some(ref<Option<i32>, shared> o) : bool { Option::is_some(o) }` `!some(&None) && some(&Some(1))` | `→ true` | `[Ref-Form-Temporary-Argument]` (`CHG-0130`): a variant without a payload is a value, borrowed as the statement's temporary |
| `conf.variant-borrow-outside-argument-rejected` | `auto r = &None;` | ✗ `diag.borrow-of-non-place` (static) | `[Ref-Form-Not-Place]`: a variant is not a place; only an argument may borrow it |
| `conf.bound-weaker-caller-rejected` | `impl/conformance/12-type-system/bound_weaker_caller_rejected.cb` | ✗ `diag.type-mismatch` (static) | `[Bound-Unsatisfied]`: a caller's `eq` does not imply `ordered` |
| `conf.min-max` | `min(3, -4) + max(2, 9)` | `→ 5` | D-0091: `std`'s `min`/`max` over `ordered` |
| `conf.abs-min-overflows` | `abs(-2147483648)` | ✗ `diag.arith-overflow` (dynamic) | D-0091: `abs` is checked |
| `conf.pow-checked` | `pow(3: i64, 39) == 4052555153018976267 && pow(2, 0) == 1` | `→ true` | D-0091: `pow` by squaring |
| `conf.float-fns` | `sqrt(2.0) * sqrt(2.0) != 2.0 && floor(-2.5) == -3.0 && round(-2.5) == -3.0 && trunc(-2.7) == -2.0 && ceil(2.1) == 3.0` | `→ true` | `rule.arith.float-fns`: IEEE-754's results |
| `conf.float-fn-int-rejected` | `sqrt(4)` | ✗ `diag.type-mismatch` (static) | `[Float-Fn-Operand]` |
| `conf.float-fn-needs-import` | `impl/conformance/21-standard-library-semantics/float_fn_needs_import_rejected.cb` | ✗ `diag.unbound-name` (static) | `sqrt` without `import std;`: a function of `std::math`, not an intrinsic (D-0136) |
| `conf.float-fn-name-own-item` | `impl/conformance/21-standard-library-semantics/float_fn_name_own_item_ok.cb` | `ok`; prints `5 3.0` | a program's own `round` is an ordinary item; a module importing `std` calls `std::math`'s |
| `conf.hashmap-clear` | `impl/conformance/21-standard-library-semantics/numeric_functions_ok.cb` | `ok` | D-0091: `HashMap::clear` empties the map, which stays usable |
| `conf.field-through-unwrapped-reference-borrowed` | `struct P { i32 a; }` `HashMap<u32, P> m = HashMap::new(); HashMap::insert(&mut m, 1, P { .a = 5 }); { auto r = &mut Option::unwrap(HashMap::get_mut(&mut m, &1)).a; *r = 7; } Option::unwrap(HashMap::get(&m, &1)).a` | `→ 7` | `Option::unwrap`'s `T` is a reference here, so `f().a` is `(*f()).a`, a place through it (`[Ref-Form-Temporary]`'s reference case, CHG-0100) |
| `conf.move-out-of-temporary-field` | `struct W { Vec<i32> v; u32 n; }` `fn mk() : W { W { .v = Vec::new(), .n = 1 } }` `Vec<i32> x = mk().v; Vec::len(&x)` | `→ 0` | D-0103 `[Temp-Field-Move]`; was rejected before D-0103 |
| `conf.move-out-of-temporary-field-destructor-rejected` | `resource struct W { Vec<i32> v; }` `fn W::drop(ref<W, exclusive> self) { }` `fn mk() : W { W { .v = Vec::new() } }` `Vec<i32> x = mk().v;` | ✗ `diag.move-out-of-field` (static) | `[Temp-Field-Move]` does not take apart a struct with a destructor (D-0103, D-0044) |

## 5. Temporal validity (`spec/10`)

| id | Fragment | Outcome | Derivation |
|---|---|---|---|
| `conf.reference-escape-rejected` | `fn f() : ref<i32, shared> { i32 x = 1; &x }` | ✗ `diag.reference-escapes-scope` (static) | `referent-block = decl-block(x)`; escapes to `caller`; root binding not a reference parameter |
| `conf.reference-escape-dynamic` | `struct H { ref<i32, shared> r; } fn f(ref<H, exclusive> h) { i32 x = 1; h.r = &x; }` then `i32 y = 0; auto hh = H { .r = &y }; f(&mut hh); *hh.r` | ✗ `diag.stale-binding` (dynamic) | static rule: store through `*h` → `unknown`; at `f`'s BE `o_x` ends → `[Object-End]` invalidates the borrow (`of = o_x`); later read → `[Read-Stale]` |
| `conf.elision-single-param-ok` | `struct Pair { i32 a; i32 b; } fn first(ref<Pair, shared> p) : ref<i32, shared> { &p.a }` `auto pr = Pair { .a = 1, .b = 2 }; auto y = first(&pr); *y` | `→ 1` | inside `first`: projection through the parameter reference, not rejected (reference-typed parameter); call site: elision names `&pr`; `y` stored in the same block |
| `conf.elision-conflict-rejected` | as above then `pr.a = 3;` | ✗ `diag.aliasing-conflict` (static) | `a_y'` is a valid shared path into `o_pr` not derived from the write's projection; FA: elision records `deriv(y, pr.ε, shared)`, which overlaps `pr.a` → refuted |
| `conf.elision-multi-param-rejected` | `fn pick(ref<i32, shared> p1, ref<i32, shared> p2) : ref<i32, shared> { p1 }` | ✗ `diag.lifetime-elision-ambiguous` (static) | `[Call-Multi-Ref-Return-Rejected]` |

## 6. Aggregates (`spec/16`)

| id | Fragment | Outcome | Derivation |
|---|---|---|---|
| `conf.index-in-bounds` | `array<i32, 3> a = [10, 20, 30]; a[2]` | `→ 30` | `[Array-Construct]` temp adopted (element literals take `i32` from the annotation); `2 : usize` from the index position (`rule.type.expected`); `[Index-Checked]` |
| `conf.index-out-of-bounds-static` | `a[3]` | ✗ `diag.index-out-of-bounds` (static) | literal index → FA refuted |
| `conf.index-out-of-bounds-dynamic` | `fn at(usize i) : i32 { array<i32, 3> a = [10, 20, 30]; a[i] }` `at(3)` | ✗ `diag.index-out-of-bounds` (dynamic) | `[Index-Out-Of-Bounds]` |
| `conf.array-repeat` | `array<u64, 5> z = [7; 5]; z[4] + z[0]` | `→ 14` | `[Array-Repeat]` (D-0072): `7` read once, typed `u64` from the annotation, copied into all five elements |
| `conf.array-repeat-inferred` | `auto f = [false; 3]; f[2]` | `→ false` | `[Array-Repeat]`: `array<bool, 3>` from the value |
| `conf.array-repeat-reference-rejected` | `i32 x = 1; auto a = [&x; 3];` | ✗ `diag.type-mismatch` (static) | `[Array-Repeat-Not-Plain]`: copies of a reference |
| `conf.array-repeat-resource-rejected` | `String s = String::new(); auto a = [s; 2];` | ✗ `diag.read-of-resource` (static) | a resource in place position is not read (`[Read-Resource-Rejected]`); `[Array-Repeat-Not-Plain]` |
| `conf.array-trailing-comma` | `array<i32, 3> a = [1, 2, 3,]; a[2]` | `→ 3` | `spec/22`: a trailing comma in an array literal (D-0072) |
| `conf.array-length-constant` | `const usize W = 3;` `array<array<u8, W>, 2> g = [[1; W]; 2]; g[1][W - 1]` | `→ 1` | `length` names a constant whose initializer is an integer literal (D-0074) |
| `conf.slice-through-returned-ref` | `String s = String::from_str("abc"); slice_len(&String::as_bytes(&s)[0 .. 2])` | `→ 2` | `String::as_bytes` returns a reference; the slice is formed through it, as `&f()[i]` is (CHG-0086) |
| `conf.struct-copy` | `struct P { i32 a; i32 b; } auto p = P { .a = 1, .b = 2 }; auto q = p; q.a + p.b` | `→ 3` | `p` in value position → `[LValue-To-RValue]` reads the struct value (`[Repr-Struct]`) → `[Store-Binding-Value]` into `o_q` |
| `conf.struct-field-resource-transfer` | `struct P { Vec<i32> a; } Vec<i32> v = Vec::new(); auto p = P { .a = v }; Vec::len(&v)` | ✗ `diag.stale-binding` (static) | `[Store-Sub-Relocate]` → `[Relocate-In]` ends `o_v`; FA: `v` used as field initializer → `valid(v) = F` |
| `conf.composite-destroy-recurses` | `{ auto p = P { .a = Vec::new() }; }` | `ok`; `Vec::drop` runs for `(o_p, a)` before `o_p` ends | BE → `[Destroy]` → `[Destroy-Composite]` (path `a`) → `[Object-End]` |
| `conf.match-exhaustive-ok` | `enum Sign { Pos, Neg, Zero } fn d(Sign s) : i32 { match (s) { Pos : 1, Neg : -1, Zero : 0 } }` `d(Neg)` | `→ -1` | `[Enum-Construct-Unit]`; `[Match]` binder `_` (payload-less); arms cover all |
| `conf.match-non-exhaustive-rejected` | `match (s) { Pos : 1, Neg : -1 }` | ✗ `diag.non-exhaustive-match` (static) | `[Match-Non-Exhaustive]` |
| `conf.match-wildcard-ok` | `match (s) { Pos : 1, _ : 0 }` | `ok` | `_` arm satisfies exhaustiveness |
| `conf.match-nested` | `impl/conformance/16-aggregates/match_nested_patterns_ok.cb` | `ok` | D-0056: arms in order; `Some(Circle(r))`, `Some(Shape::Dot)`, three levels, `_` inside; a `String` moved out two levels down a binding; a match moving nothing leaves its scrutinee usable; `match (&mut r)` changes `Ok(Some(x))`'s payload; `File::read_line` in one match |
| `conf.match-nested-non-exhaustive` | `impl/conformance/16-aggregates/match_nested_non_exhaustive_rejected.cb` | ✗ `diag.non-exhaustive-match` (static) | `[Match-Non-Exhaustive]`: `Ok(None)` is not covered, and the message says so |
| `conf.match-duplicate-arm` | `impl/conformance/16-aggregates/match_duplicate_arm_rejected.cb` | ✗ `diag.unreachable-arm` (static) | `[Match-Unreachable]`: a second `Some(y)` arm |
| `conf.match-covered-by-nested` | `impl/conformance/16-aggregates/match_arm_covered_by_nested_arms_rejected.cb` | ✗ `diag.unreachable-arm` (static) | `[Match-Unreachable]`: `Some(Some(x))` and `Some(None)` leave nothing for `Some(_)` |
| `conf.match-nested-wrong-enum` | `impl/conformance/16-aggregates/match_nested_wrong_enum_rejected.cb` | ✗ `diag.type-mismatch` (static) | `[Pattern-Type]`: `Ok` inside `Some` of an `Option<Option<i32>>` |
| `conf.match-nested-generic-payload` | `impl/conformance/16-aggregates/match_nested_generic_payload_rejected.cb` | ✗ `diag.type-mismatch` (static) | `[Pattern-Type]`: a pattern into `T`, a type parameter |
| `conf.match-nested-move-through-ref` | `impl/conformance/16-aggregates/match_nested_move_through_ref_rejected.cb` | ✗ `diag.move-out-of-field` (static) | `[Match-Move-Through-Ref]` two levels down `*r` |
| `conf.match-nested-move-consumes` | `impl/conformance/16-aggregates/match_nested_move_consumes_rejected.cb` | ✗ `diag.stale-binding` (static) | the arm binding a nested `String` consumes the scrutinee; the `_` arm does not, so `o` may have been moved (D-0087) |
| `conf.match-literals` | `impl/conformance/16-aggregates/match_literal_patterns_ok.cb` | `ok` | D-0057: `match` on integers, bytes and `bool`s; `Ok(0)`, `Err(-5)`, `Some(Some(true))`; `u128`'s largest and `i128`'s smallest literals; a computed scrutinee; a literal through a reference; arms that `break`, `continue`, `return` |
| `conf.match-literal-non-exhaustive` | `impl/conformance/16-aggregates/match_literal_non_exhaustive_rejected.cb` | ✗ `diag.non-exhaustive-match` (static) | literals never cover an integer type; the message is `_` |
| `conf.match-bool-non-exhaustive` | `impl/conformance/16-aggregates/match_bool_non_exhaustive_rejected.cb` | ✗ `diag.non-exhaustive-match` (static) | `Some(false)` is not covered, and the message says so |
| `conf.match-text-str` | `str s = "put"; match (s) { "get" : 1, "put" : 2, _ : 0, }` | `→ 2` | D-0127: a `match` on `str`, text literals compared byte for byte |
| `conf.match-text-literals` | `impl/conformance/16-aggregates/match_text_patterns_ok.cb` | `ok` | D-0127: `match` on `str`, `String` (a binding, a field, a temporary, `*r`), `StringView`; `Some("x")`, `Ok("x")` by value and by reference, through a reference payload; `""`; a binder arm after literals takes the whole `String`, `_` leaves it in place; `if ("quit" = c)` and `while ("go" = next(&mut q))` |
| `conf.match-text-non-exhaustive-rejected` | `impl/conformance/16-aggregates/match_text_non_exhaustive_rejected.cb` | ✗ `diag.non-exhaustive-match` (static) | text literals never cover a text type; the message says `_` |
| `conf.match-text-literal-type-rejected` | `impl/conformance/16-aggregates/match_text_literal_type_rejected.cb` | ✗ `diag.type-mismatch` (static) | `[Pattern-Type]`: `1` against a `String` |
| `conf.match-text-on-integer-rejected` | `impl/conformance/16-aggregates/match_text_on_integer_rejected.cb` | ✗ `diag.type-mismatch` (static) | `[Pattern-Type]`: `"1"` against an `i32` |
| `conf.match-text-reference-rejected` | `impl/conformance/16-aggregates/match_text_reference_rejected.cb` | ✗ `diag.type-mismatch` (static) | `[Pattern-Type]`: `"a"` against a `ref<String, shared>` at the top level; `match (*r)` reads the text |
| `conf.match-text-duplicate-rejected` | `impl/conformance/16-aggregates/match_text_duplicate_rejected.cb` | ✗ `diag.unreachable-arm` (static) | `Some("a")` twice: `[Match-Unreachable]` |
| `conf.match-literal-duplicate` | `impl/conformance/16-aggregates/match_literal_duplicate_rejected.cb` | ✗ `diag.unreachable-arm` (static) | a second `3` arm |
| `conf.match-literal-wrong-type` | `impl/conformance/16-aggregates/match_literal_wrong_type_rejected.cb` | ✗ `diag.type-mismatch` (static) | `[Pattern-Type]`: `1` against a `bool` |
| `conf.match-literal-out-of-range` | `impl/conformance/16-aggregates/match_literal_out_of_range_rejected.cb` | ✗ `diag.literal-out-of-range` (static) | `300` against a `u8` |
| `conf.match-literal-on-enum` | `impl/conformance/16-aggregates/match_literal_on_enum_rejected.cb` | ✗ `diag.type-mismatch` (static) | `[Pattern-Type]`: a literal where a variant is |
| `conf.match-through-ref-payload` | `impl/conformance/16-aggregates/match_through_ref_payload_ok.cb` | `ok` | `[Match-Through-Ref]` (D-0109): `Some(Num(v))` on `HashMap::get`'s `Option<ref<Cell, shared>>`; `v : ref<i64, shared>`; through `get_mut` the arm writes `*v`; two references looked through as one |
| `conf.match-through-ref-literal` | `impl/conformance/16-aggregates/match_through_ref_payload_ok.cb` | `ok` | `[Pattern-Type]` (D-0109): `Some(5)` compares through `Option<ref<u32, shared>>` |
| `conf.match-through-ref-binder-borrows-rejected` | `impl/conformance/16-aggregates/match_through_ref_binder_borrows_rejected.cb` | ✗ `diag.aliasing-conflict` (static) | `[Match-Through-Ref]`: the binder derives from `get`'s reference, which borrows the map (D-0088); `HashMap::clear(&mut m)` while it lives clashes |
| `conf.match-through-ref-non-exhaustive-rejected` | `impl/conformance/16-aggregates/match_through_ref_non_exhaustive_rejected.cb` | ✗ `diag.non-exhaustive-match` (static) | coverage splits the referent's variants; `None` is not covered |
| `conf.match-literal-through-reference` | `impl/conformance/16-aggregates/match_literal_through_reference_rejected.cb` | ✗ `diag.type-mismatch` (static) | `[Pattern-Type]`: `match (&x)` on an `i32` |
| `conf.match-binder` | `impl/conformance/16-aggregates/match_binder_whole_value_ok.cb` | `ok` | D-0058: a binder alone binds the whole value: an integer's remaining values, a `String` moved, an enum after its variants, through a reference, a call's result |
| `conf.match-binder-moves` | `impl/conformance/16-aggregates/match_binder_moves_rejected.cb` | ✗ `diag.stale-binding` (static) | the binder moved the `String` |
| `conf.match-arm-after-binder` | `impl/conformance/16-aggregates/match_arm_after_binder_rejected.cb` | ✗ `diag.unreachable-arm` (static) | `[Match-Unreachable]`: after a binder |
| `conf.match-const` | `impl/conformance/16-aggregates/match_const_patterns_ok.cb` | `ok` | D-0058: constants as literals: plain, computed, negative, `bool`, a module's, nested |
| `conf.match-const-wrong-type` | `impl/conformance/16-aggregates/match_const_wrong_type_rejected.cb` | ✗ `diag.type-mismatch` (static) | `[Pattern-Type]`: a `u16` constant against a `u8` |
| `conf.match-const-not-binder` | `impl/conformance/16-aggregates/match_const_not_binder_rejected.cb` | ✗ `diag.non-exhaustive-match` (static) | a constant's name is not a binder; the missing `_` shows it |
| `conf.match-resource-payload-transfers` | `enum E { Has(Vec<i32>), Empty } auto e = E::Has(Vec::new()); match (e) { Has(x) : Vec::len(&x), Empty : 0 }` | `→ 0`; `e` stale afterwards | scrutinee `a_e` root; `[Relocate-Out]` → temp adopted by `x` in the arm frame; consume (the `Has` arm moved the payload out): `[Destroy]` on `a_e` (no obligations left); arm BE destroys `o_x`; FA: only the `Has` arm consumes (D-0049), so after the match `valid(e) = unknown` and a later `drop(e)` is `[Binding-Lookup-Stale]` at run time (`diag.stale-binding`, dynamic) |
| `conf.match-through-ref-rejected` | `fn f(ref<E, shared> e) : usize { match (*e) { Has(x) : Vec::len(&x), Empty : 0 } }` | ✗ `diag.move-out-of-field` (static) | `[Match-Move-Through-Ref]`: `base(a) ≠ None` |
| `conf.let-destructure` | `struct W { Vec<i32> inner; } auto w = W { .inner = Vec::new() }; W { inner } = w; Vec::len(&inner)` | `→ 0` | `[T-Let-Destructure]`; `[Let-Destructure]`: `[Relocate-Out]` then `[Destroy]` of `o_w`; FA: `w` consumed |
| `conf.slice-range-disjoint` | `impl/conformance/16-aggregates/slice_range_disjoint_ok.cb` | ok | `[Slice-Form]` (D-0070): exclusive slices of disjoint ranges of an array and a `Vec` together; a slice of a slice split; two empty slices; an element outside a live range |
| `conf.slice-range-threads` | `impl/conformance/16-aggregates/slice_range_threads_ok.cb` | ok | an in-place quicksort on the two sides of its pivot; two threads sorting the halves of one `Vec` |
| `conf.slice-range-overlap` | `impl/conformance/16-aggregates/slice_range_overlap_rejected.cb` | ✗ `diag.aliasing-conflict` (dynamic) | ranges `0 .. 4` and `3 .. $` meet |
| `conf.slice-range-literal-conflict` | `impl/conformance/16-aggregates/slice_range_literal_conflict_rejected.cb` | ✗ `diag.aliasing-conflict` (static) | a write inside a live slice's literal range (`spec/14` `deriv`); element 4, outside it, is not a conflict |
| `conf.slice-range-source-whole` | `impl/conformance/16-aggregates/slice_range_source_whole_rejected.cb` | ✗ `diag.aliasing-conflict` (static) | `Vec::push(&mut v, …)` while a slice of `v` is live |
| `conf.match-arm-temporary-borrowing-binder` | `impl/conformance/16-aggregates/match_arm_temporary_borrowing_binder_ok.cb` | ok | `[Match]`: the arm body is `stmt-scoped(e_i, keep)`, so a temporary made in an expression arm that borrows the arm's binding (`&String::strip_prefix(&r, "x")`, `&String::chars(&r)` as arguments, `[Ref-Form-Temporary-Argument]`) ends at the arm expression's `[Stmt-Exit]`, before `[Block-Exit]` ends the arm frame and destroys `r` solitary; only the arm's value, a copy here, outlives the arm |

## 7. Trust boundaries (`spec/20`)

| id | Fragment | Outcome | Derivation |
|---|---|---|---|
| `conf.rawptr-deref-outside-unsafe-rejected` | `fn f(rawptr<i32> p) : i32 { *p }` | ✗ `diag.trusted-outside-unsafe` (static) | `[Unsafe-Rejected]` |
| `conf.rawptr-deref-inside-unsafe-ok` | `fn f(rawptr<i32> p) : i32 { unsafe { *p } }` `i32 x = 7; f(rawptr_of(&x))` | `→ 7` | `[Rawptr-Of]` gives `min(target)`; `[T-Deref]` (rawptr case); `[Rawptr-Read]` reads the cells `[Repr-Int]` wrote — its trusted condition holds: `a_x` is a root, the argument borrow is shared |
| `conf.reclaim-two-paths-clash` | `Vec<i32> v = Vec::new(); Vec::push(&mut v, 7); auto p = rawptr_of(Vec::index_shared(&v, 0)); unsafe { auto r1 = &mut reclaim<i32>(p); auto r2 = &reclaim<i32>(p); }` | ✗ `diag.aliasing-conflict` (dynamic) | `index_shared` established element 0 as a reclaimed object (its borrow died at that statement's SE); each `reclaim` re-attaches it as a fresh root path (`[Reclaim]`'s trusted condition holds: the cells belong to `v`'s allocation, not to a fresh-storage object); the second `[Borrow]` (shared) sees `a_r1'` (exclusive, not an ancestor) → `[Borrow-Denied]`; FA: root is a reclaim → unknown → dynamic |
| `conf.extern-non-ffi-type-rejected` | `unsafe extern fn g(Vec<i32> v);` | ✗ `diag.extern-non-ffi-type` (static) | `[Extern-Non-Ffi-Type]` |
| `conf.extern-without-unsafe-rejected` | `impl/conformance/20-trust-boundaries/extern_without_unsafe_rejected.cb` | ✗ `diag.syntax-error` (static) | `spec/22` §3 `extern-decl` (D-0146): `extern fn abs(i32 x) : i32;` without `unsafe`; the declaration's signature is `[Extern-Decl]`'s unchecked claim |
| `conf.unsafe-extern-code-rejected` | `impl/conformance/20-trust-boundaries/unsafe_extern_code_rejected.cb` | ✗ `diag.syntax-error` (static) | `spec/22` §3 `extern-code` (D-0146): `unsafe extern "m";` — linking declares no signature, so it takes no `unsafe` |
| `conf.extern-write-observed` | `ex.extern-write` | `ok; → claim : isize` (`Σ.trust(claim) = unchecked-claim`; the concrete value is outside this specification's scope, `feat.minimal-io-extern-surface`) | `[Array-Construct]` builds `message : array<u8,6>`, a fresh plain (non-resource) object; `&message` is a shared borrow of its root path `a_message`; `[Rawptr-Of]` (safe) gives `p0 = min(target(a_message)) : rawptr<array<u8,6>>`; `reinterpret_ptr<u8>` (safe, identity on the address) gives `p : rawptr<u8>`; `send(p, 6)` evaluates `p, 6` left to right (`u8`/`usize`/`rawptr<u8>` args, none resources or places) and enters `send`'s body; `write(p, n)` occurs lexically inside `unsafe { }`, satisfying `[Unsafe-Rejected]`'s guard; `[Extern-Call]`'s side-conditions are the author's trusted assertion (`p` is reachable storage from a live object, `write` only reads those 6 cells) — `disposition: trusted-unchecked`, so no rule checks it; conclusion: `claim : isize` with `Σ.trust(claim) := unchecked-claim`, `storage` widened only outside any live object's cells; `send` returns `claim`; no diagnostic, terminates `ok` |
| `conf.extern-code-library-name` | `impl/conformance/20-trust-boundaries/extern_code_library_name_ok.cb` | ok | `extern "m";` (`rule.trust.extern-code`) declares no name; `sqrt` is an ordinary `[Extern-Call]` whether or not the implementation links foreign code |
| `conf.extern-code-c-file` | `impl/conformance/20-trust-boundaries/extern_code_c_file_ok.cb` | ok | a program's own C file, named relative to the declaring file (the case, and a module in `extern_code/`); `[Extern-Call]` into it, the count it returns checked before use. Only an implementation that links foreign code runs it (`cobc-only:`) |
| `conf.vec-of-refs-usable` | `i32 a = 10; i32 b = 20; Vec<ref<i32, shared>> v = Vec::new(); Vec::push(&mut v, &a); Vec::push(&mut v, &b); **Vec::index_shared(&v, 0) + **Vec::index_shared(&v, 1)` | `→ 30` | each `push`: `[Rawptr-Write]` of a reference-bearing `ref<i32,shared>` establishes the element's reclaimed object as holder (`CHG-0031`), so `&a`/`&b` are held, not `Unheld`, at the push statement's SE; `index_shared` re-attaches the element object and borrows it; `**` reads through the stored reference (valid) |
| `conf.vec-holds-exclusive-ref-conflict` | `i32 x = 1; Vec<ref<i32, exclusive>> v = Vec::new(); Vec::push(&mut v, &mut x); x = 5;` | ✗ `diag.aliasing-conflict` (dynamic) | `&mut x` is held by the element's reclaimed object (`CHG-0031`); `x = 5`: `[Write]`'s `¬clash(a_x, exclusive)` finds that held, non-ancestor exclusive path; FA: `&mut x` passed to a call → `escaped(x)` → dynamic |
| `conf.vec-pop-releases-ref` | `i32 x = 1; Vec<ref<i32, exclusive>> v = Vec::new(); Vec::push(&mut v, &mut x); Vec::pop(&mut v); x = 5; x` | `→ 5` | `pop` reads the element (`[Rawptr-Read]`: a copy, the discarded `Option` ends at SE) and `release`s the slot: `[Release]` → `[Object-End]` of the element object, so `&mut x` is held by nothing and invalid; `x = 5` meets no conflicting path |
| `conf.safe-client-vec-trace` | `Vec<i32> v = Vec::new(); Vec::push(&mut v, 1); Vec::push(&mut v, 2); Vec::push(&mut v, 3); Vec::push(&mut v, 4); Vec::push(&mut v, 5); i32 a = *Vec::index_shared(&v, 4); match (Vec::pop(&mut v)) { Some(x) : a + x, None : 0 }` | `→ 10` | D-0206 (`term.safe-program`): a safe client — the fragment holds no `unsafe` — whose execution reaches trusted side-conditions only in the trusted library base; each is listed in `spec/21` §0's table with the fact that discharges it. `Vec::new`: no trusted step (`dangling<T>()`, `cap = 0`). First `push`: `len = cap = 0` → `grow` → `[Allocate]` (fallible, not trusted) of 4 slots; `[Copy-Raw]` of `0` bytes into the fresh range and no `[Deallocate]` (`cap = 0`) — table row `Vec::grow`: the destination is what `allocate` just returned; then `[Rawptr-Write]` of slot 0 — row `Vec::push`: `len < cap` after `grow`, no element object there. Pushes 2–4: `[Rawptr-Write]` of slots 1–3 under the same fact. Fifth `push`: `len = 4 = cap` → `grow` to 8: `[Copy-Raw]` of 4 elements into the fresh range, `[Deallocate]` of the 4-slot buffer — row `Vec::grow`: `(ptr, cap·sizeof, align)` is exactly the previous allocation and nobody freed it; then `[Rawptr-Write]` of slot 4. `index_shared(&v, 4)`: `4 < len` checked by the body, then `[Reclaim]` of slot 4 — row `Vec::index_shared`: the cells lie inside the buffer and hold the element `push` wrote; the reclaimed element object is borrowed shared, `a` reads `5` through it, and the borrow dies with the statement. `pop`: `len := 4`, `[Rawptr-Read]`/`[Rawptr-Move-Out]` of slot 4 then `[Release]` — row `Vec::pop`: slot `len` is the element `push` wrote last, and the index borrow of it has ended (it would otherwise have clashed with `&mut v`); `Some(5)`, so `a + x = 10`. Block exit: `Vec::drop` → `[Reclaim]` of slots 0–3 to destroy plain `i32`s (row `Vec::drop`, as `index_shared`), `[Deallocate]` of the 8-slot buffer — the `Vec`'s one deallocation under `[Destroy]`'s single-use authority. Every trusted claim on the path is the implementation's (`term.conformance`); the program made none, so `spec/03` holds in every reachable `Σ` unconditionally |
| `conf.safe-client-string-trace` | `String s = String::from_str("ab"); String t = String::clone(&s); String::len(&s) + String::len(&t)` | `→ 4` | D-0206: as `conf.safe-client-vec-trace`, through `String`'s `Vec<u8>`. `from_str("ab")`: `[Str-Len]`, `[Str-Byte]` ×2 (checked, not trusted), two `Vec::push`es — `grow` 0→4 (`[Copy-Raw]` of 0 bytes, row `Vec::grow`), `[Rawptr-Write]` ×2 (row `Vec::push`); `String { .bytes = v }` relocates the vector, no trusted step. `clone(&s)`: `String::clone` is `String::new()` then `String::append_string`, which reads each of `s`'s bytes below `len` by `[Rawptr-Read]` — row `String::append_string`: live for the call through the shared reference — and `push`es it into the fresh vector (rows `Vec::grow`, `Vec::push`). `len` ×2: plain field reads, no trusted step: `2 + 2 = 4`. Block exit: two `Vec::drop`s — `[Reclaim]` of the `u8` slots and one `[Deallocate]` each (row `Vec::drop`) |
| `conf.safe-client-box-trace` | `Box<i32> b = Box::new(5); i32 x = *Box::get(&b); x + Box::into_inner(b)` | `→ 10` | D-0206: `Box::new(5)`: `[Allocate]` of `Box::cells<i32>()`, then `[Rawptr-Write]` of the value into it — row `Box::new`: fresh cells with no object over them; `full = true`. `get(&b)`: `[Reclaim]` of the value — row `Box::get`: `ptr` came from `Box::new` and `full` holds; `x = 5` read through the shared borrow. `into_inner(b)`: `b` consumed, `full := false`, `[Rawptr-Read]`/`[Rawptr-Move-Out]` of the value then `[Release]` — row `Box::into_inner`: `full` was true and is now false, so `drop` will not reclaim it again; `5`, so `x + 5 = 10`. The consumed `b`'s `Box::drop` runs at the end of `into_inner`: `full` is false, so no `[Reclaim]`, and `[Deallocate]` of the one allocation `Box::new` made (row `Box::drop`) |
| `conf.safe-client-rc-trace` | `Rc<i32> a = Rc::new(7); Rc<i32> b = Rc::clone(&a); *Rc::get(&a) + *Rc::get(&b)` | `→ 14` | D-0206: `Rc::new(7)`: `[Allocate]` of an `RcBox<i32>`, `[Rawptr-Write]`/`[Rawptr-Move-In]` of `RcBox { count = 1, weak = 0, value = Some(7) }` — row `Rc::new`: fresh cells, no object over them. `clone(&a)`: `[Reclaim]` of the box, `count := 2` through an exclusive borrow of that field alone — row `Rc::clone`: `ptr` came from `Rc::new` and the box lives while any `Rc` holds it; only `count` is borrowed, disjoint from any `Rc::get` reference into `value`. `get(&a)`, `get(&b)`: `[Reclaim]` of the box (row `Rc::get`: a live `Rc` keeps `value` `Some`), each a shared borrow into `value` that dies with its statement: `7 + 7 = 14`. Block exit, `b` then `a`: `Rc::drop` → `[Reclaim]`, `count := 1` (not last); then `count := 0`, last: `overwrite(…, None)` ends the value through the reclaimed object, `weak = 0` so `drop(reclaim<RcBox>)` and `[Deallocate]` — row `Rc::drop`: the single deallocation of what `Rc::new` allocated |
| `conf.safe-client-channel-trace` | `Channel<i32> ch = Channel::new(2); _ = Channel::send(&ch, 3); match (Channel::recv(&ch)) { Some(v) : v, None : 0 }` | `→ 3` | D-0206: `Channel::new(2)`: two slots pushed (rows `Vec::grow`, `Vec::push`), then two `[Extern-Call]`s `event_op(new)` — row `Channel::new`: they touch no program storage; the state goes into a `Mutex::new` (no trusted step). `send(&ch, 3)`: `[Lock]`, the slot written through the guard (checked), `event_op(signal)` — row `Channel::send`: an `[Extern-Call]` on the channel's own event ids, touching no program storage; `Ok(())`, discarded by `_ =`. `recv(&ch)`: `[Lock]`, `ChannelState::pop` reads the slot through the guard, `event_op(signal)` (row `Channel::recv`); `Some(3)`. Block exit: `Channel::drop` destroys the remaining (no) values, then `event_op(end)` ×2 (row `Channel::drop`), the mutex's and the slots' `Vec::drop` (row `Vec::drop`). Nothing crosses the boundary but the event ids and a result code: the trusted claims are the implementation's, as in every row above |


## 8. Failure semantics (`spec/18`)

| id | Fragment | Outcome | Derivation |
|---|---|---|---|
| `conf.checked-fault-unwinds-and-terminates` | `fn d(i32 a, i32 b) : i32 { a / b }` `{ Vec<i32> v = Vec::new(); d(1, 0); }` | terminates `diag.div-by-zero`; `Vec::drop` ran | `[Fault-Unwind]`: `unwind-to(f_0, ())` runs the block's BE |
| `conf.recursion-ten-thousand-deep` | `fn depth(u64 n) : u64 { if (n == 0) { 0 } else { 1 + depth(n - 1) } }` `depth(10000)` | `→ 10000` | within the guaranteed minimum depth (D-0077) |
| `conf.recursion-stack-exhausted` | `fn depth(u64 n) : u64 { if (n == 0) { 0 } else { 1 + depth(n - 1) } }` `depth(100000000)` | ✗ `diag.stack-exhausted` (dynamic) | `[Call-Stack-Exhausted]`: deeper than any implementation's stack, a fault at the call, never a host crash (D-0077) |
| `conf.propagate-ok` | `fn f() : Result<i32, i32> { Result<i32, i32> r = Ok(1); auto v = r?; Ok(v + 1) }` `f()` | `→ Ok(2)` | `?` → `[Propagate]` → `[Match]`, arm `Ok(v)` copies the payload (`[Store-Binding-Place-Copy]`) into the arm frame |
| `conf.propagate-err` | `fn f() : Result<i32, i32> { Result<i32, i32> r = Err(5); auto v = r?; Ok(v) }` `f()` | `→ Err(5)` | `?` → `[Propagate]` → `[Match]`, arm `Err(err) : return Err(err)` → `[Return]` (`never`-typed arm, `[T-Never]`) |
| `conf.propagate-mismatch-rejected` | `fn f() : i32 { Result<i32, i32> r = Ok(1); r? }` | ✗ `diag.propagate-outside-fallible-context` (static) | `[Propagate-Err-Mismatch]` |
| `conf.assert-holds` | `impl/conformance/18-error-failure-semantics/assert_holds_ok.cb` | ok | `[Assert]`: the condition evaluated once, the message's arguments not at all |
| `conf.assert-fails` | `impl/conformance/18-error-failure-semantics/assert_fails_rejected.cb` | ✗ `diag.assert-failed` (dynamic) | `[Assert-Fail]`: the formatted message reported with the call's location; `[Fault-Unwind]` destroys `k` |
| `conf.assert-without-message` | `impl/conformance/18-error-failure-semantics/assert_without_message_fails_rejected.cb` | ✗ `diag.assert-failed` (dynamic) | `assert(c)`: a true one passes, a false one fails with no message |
| `conf.assert-not-bool` | `impl/conformance/18-error-failure-semantics/assert_not_bool_rejected.cb` | ✗ `diag.type-mismatch` (static) | `[Assert-Ill-Formed]`: an `i32` condition |

## 9. Generics and inference (`spec/12`, `spec/15`, `spec/16`)

| id | Fragment | Outcome | Derivation |
|---|---|---|---|
| `conf.generic-fn-explicit` | `fn id<T>(T x) : T { x }` `id<i32>(5)` | `→ 5` | `[Generic-Call-Explicit]`; `x` non-resource: returned by copy |
| `conf.generic-call-inferred` | `id(5)` | `→ 5` | `[Generic-Call-Inferred]`: `T := i32` (literal default) |
| `conf.generic-assoc-fn-inferred` | `struct W { i32 a; } fn W::put<T>(ref<W, exclusive> w, T x) { } W w = W { .a = 1 }; W::put(&mut w, 42);` | ok | `[Generic-Call-Inferred]`: `T := i32` from the second argument; `W` in the first parameter's type is the struct, not a type parameter |
| `conf.generic-void-argument` | `fn id<T>(T x) : T { x }` `id(());` | ok | `[Generic-Call-Inferred]`: `T := void` from `()`; a parameter may have type `void` |
| `conf.generic-call-expected-type-inferred` | `Vec<i32> nums = Vec::new(); Vec::len(&nums)` | `→ 0` | `[Generic-Call-Expected-Type]` matches `Vec<T>` to the annotation |
| `conf.generic-call-uninferable-rejected` | `auto bad = Vec::new();` | ✗ `diag.cannot-infer-type-parameter` (static) | no argument, no expected type |
| `conf.own-generic-hiding-std-checked-at-own-types` | `impl/conformance/15-function-semantics/own_generic_hiding_std_checked_at_own_types_ok.cb` | ok | a program's own `swap<T>` hides `std::swap` (D-0024); each generic instantiation is of the function the call names (D-0010), so std's internal `swap<Vec<usize>>` is not checked against the program's body (CHG-0110) |
| `conf.generic-item-value-uninferable` | `impl/conformance/12-type-system/generic_item_value_uninferable_rejected.cb` | ✗ `diag.cannot-infer-type-parameter` (static) | `[T-Item-Value-Uninferable]` (D-0093): `auto f = id;` with nothing fixing `T`; explicit arguments (`id<i64>`) and the expected fn type stay accepted (`generic_fn_item_as_value_ok.cb`) |
| `conf.generic-item-value-wrong-expected` | `impl/conformance/12-type-system/generic_item_value_wrong_expected_rejected.cb` | ✗ `diag.type-mismatch` (static) | `[T-Item-Generic]` (D-0093): the instantiation must equal the expected fn type; with `T := i32`, `id` is `fn(i32) : i32`, never `fn(i32) : bool` |
| `conf.generic-struct-monomorphize-resource` | `struct Box<T> { T value; } Box<Vec<i32>> c = Box { .value = Vec::new() };` | `ok`; `is-resource(Box<Vec<i32>>) = true`; `Vec::drop` at BE | `rule.type.expected` (D-0014) fixes the field's expected type; `[Struct-Construct]` relocates the temp Vec in; BE → `[Destroy-Composite]` |
| `conf.unbounded-type-parameter-rejected` | `fn add<T>(T a, T b) : T { a + b }` | ✗ `diag.unbounded-type-parameter` (static) | `rule.type.kind` |
| `conf.literal-default-i32` | `auto x = 5;` | `x : i32` | `[Literal-Default]` |
| `conf.literal-context-u8` | `u8 x = 200;` | `x : u8` | `[Literal-Type-From-Context]` |
| `conf.let-synthesis` | `auto x = 3: i64 + 4: i64;` | `x : i64` | `[T-Let]` from `[T-Arith]` |
| `conf.enum-literal-expected-type` | `impl/conformance/12-type-system/enum_literal_takes_expected_type_ok.cb` | ok | `rule.type.expected`: `Some(5000000000)`'s `T` from the expected `Option<u64>` in a declaration, an assignment, a return, an argument (generic: `T` fixed by the arguments to the left), a field, a nested literal, and either operand of `==`/`!=` |
| `conf.while-true-never` | `impl/conformance/12-type-system/while_true_never_ok.cb` | ok | `[T-While-Forever]` (D-0175): functions ending in a `while (true)` that only returns from it, one whose inner loop has the `break`, and a loop with its own `break` followed by a value |
| `conf.while-true-break-needs-value` | `impl/conformance/12-type-system/while_true_break_needs_value_rejected.cb` | ✗ `diag.type-mismatch` (static) | `[T-While]`: a `break` that leaves the `while (true)` makes it `unit`, so the function's value is missing |
| `conf.while-true-literal-only` | `impl/conformance/12-type-system/while_condition_true_variable_rejected.cb` | ✗ `diag.type-mismatch` (static) | `[T-While-Forever]`: a condition that is always true at run time but not the literal `true` keeps `unit` |

## 10. Closures (`spec/15` §6)

| id | Fragment | Outcome | Derivation |
|---|---|---|---|
| `conf.closure-borrow-capture` | `i32 x = 10; auto f = [x](i32 y) { x + y }; f(5)` | `→ 15` | `[Closure-Form-Borrow]`: capture mode shared; struct temp adopted by `f`; `[Closure-Call]`: exclusive self-borrow of `o_f`, body `*self.f1 + y` |
| `conf.closure-capture-list-mismatch-rejected` | `i32 x = 10; auto f = [](i32 y) { x + y };` | ✗ `diag.capture-list-mismatch` (static) | `[Closure-Capture-List-Rejected]`: written list `∅` ≠ free variables `{x}` |
| `conf.closure-move-capture-invalidates-source` | `Vec<i32> v = Vec::new(); auto f = move [v]() { Vec::push(&mut v, 1); }; f(); Vec::push(&mut v, 2);` | ✗ `diag.stale-binding` (static) | `[Closure-Form-Move]` relocates `o_v` into the closure; FA: move capture → `valid(v) = F` |
| `conf.closure-result-checked` | `fn(u32) : u32 f = [](u32 x) { x > 1 };` | ✗ `diag.type-mismatch` (static) | `[T-Closure]` with an expected `fn` type: the body gives `bool`, `u32` is expected (D-0073) |
| `conf.closure-in-vec-callable` | `u32 k = 3; Vec<fn(u32) : u32> fs = Vec::new(); Vec::push(&mut fs, move [k](u32 x) { x * k }); auto g = fs[0]; g(5)` | `→ 15` | the closure is a value of `fn(u32) : u32`; stored in the `Vec` and read back, it is still callable (CHG-0086) |
| `conf.closure-generic-inferred` | `fn map_all<T>(ref<Vec<u32>, shared> xs, fn(u32) : T f) : Vec<T> { Vec<T> out = Vec::new(); foreach (x in xs) { Vec::push(&mut out, f(*x)); } out }` `Vec<u32> xs = Vec::new(); Vec::push(&mut xs, 4); Vec<bool> ys = map_all(&xs, [](u32 x) { x > 3 }); ys[0]` | `→ true` | `T` fixed by the closure's type `fn(u32) : bool` (`rule.fn.generic-call`) |
| `conf.closure-drop-through-self-rejected` | `Vec<i32> v = Vec::new(); auto f = move [v]() { drop(v); }; f();` | ✗ `diag.move-out-of-field` (static) | body's `drop(v)` is `drop(self.f1)` → `[Destroy-Projection]` |
| `conf.closure-owns-moved-resource` | `{ Vec<i32> v = Vec::new(); auto f = move [v]() { Vec::len(&v); }; }` | `ok`; `Vec::drop` at BE | closure object is a resource by derivation; BE → `[Destroy-Composite]` on its field |

## 11. Library (`spec/21`)

| id | Fragment | Outcome | Derivation |
|---|---|---|---|
| `conf.vec-push-len` | `Vec<i32> v = Vec::new(); Vec::push(&mut v, 1); Vec::push(&mut v, 2); Vec::len(&v)` | `→ 2` | `grow` on first push (`[Allocate]`; its `if` branches take `usize` from the declaration, `rule.type.expected`; `[T-Rawptr-Offset]`), `[Rawptr-Write]` twice; each `&mut v` argument is held only by the callee's parameter object and is Dead after the call |
| `conf.vec-insert-remove` | `Vec<i32> v = Vec::new(); Vec::push(&mut v, 1); Vec::push(&mut v, 3); Vec::insert(&mut v, 1, 2); Vec::insert(&mut v, 0, 0); i32 r = Vec::remove(&mut v, 3); r * 10 + v[2]` | `→ 32` | `Vec::insert`/`Vec::remove` (D-0073): `[0, 1, 2, 3]`, then `3` removed |
| `conf.option-result-predicates` | `Option<i32> o = Some(1); Result<i32, str> r = Err("e"); Option::is_some(&o) && !Option::is_none(&o) && Result::is_err(&r) && !Result::is_ok(&r)` | `→ true` | D-0073; nothing consumed |
| `conf.string-eq` | `String a = String::from_str("ab"); String b = String::from_str("ab"); String::eq(&a, &b) && String::eq_str(&a, "ab") && !String::eq_str(&b, "a")` | `→ true` | `[View-Eq]` on the whole texts (D-0073) |
| `conf.string-compare-operators` | `String a = String::from_str("app"); String b = String::from_str("apple"); (a != b) && (a < b) && (b >= a) && !(a == b) && (a == String::from_str("app"))` | `→ true` | `[Cmp-Text]` (D-0108): byte order, a prefix first; a temporary operand lives to the statement's end |
| `conf.string-compare-in-place` | `String a = String::from_str("x"); bool e = a == a; String b = a; e && String::eq_str(&b, "x")` | `→ true` | `[Cmp-Text]`: each `String` operand is borrowed shared for the comparison, not read whole, so `a` can be moved afterwards |
| `conf.string-compare-while-exclusive-rejected` | `String a = String::from_str("x"); String b = String::from_str("y"); ref<String, exclusive> m = &mut a; bool z = a < b; String::push_ascii(m, 65);` | ✗ `diag.aliasing-conflict` (static) | `[Cmp-Text]` borrows `a` shared while `m`, used later, holds it exclusively (`clash`) |
| `conf.view-ordering` | `String a = String::from_str("pear"); StringView v = &a[1..$]; StringView w = &a[0..2]; (v < w) && (w > v) && (v <= v) && !(v >= w)` | `→ true` | `[Cmp-Text]` on two views (D-0108): `"ear" < "pe"` by the first byte |
| `conf.string-less-sort` | `Vec<String> v = Vec::new(); Vec::push(&mut v, String::from_str("pear")); Vec::push(&mut v, String::from_str("app")); Vec::push(&mut v, String::from_str("apple")); Vec::sort_by(&mut v, String::less); String::eq_str(&v[0], "app") && String::eq_str(&v[2], "pear")` | `→ true` | `String::less` is byte order, a prefix first (D-0076) |
| `conf.string-truncate` | `String t = String::from_str("héllo"); String::truncate(&mut t, 3); String::eq_str(&t, "hé")` | `→ true` | `é` is two bytes; 3 is a character boundary (D-0076) |
| `conf.string-truncate-inside-character` | `String t = String::from_str("héllo"); String::truncate(&mut t, 2);` | ✗ `diag.not-char-boundary` (dynamic) | byte 2 is inside `é` (D-0076) |
| `conf.vec-index-shared-ok` | `… Vec::push(&mut v, 10); *Vec::index_shared(&v, 0)` | `→ 10` | `index_shared`: `[Reclaim]` establishes the element object; `[Borrow]` shared; caller derefs |
| `conf.vec-index-out-of-bounds` | `… *Vec::index_shared(&v, 1)` | ✗ `diag.index-out-of-bounds` (dynamic) | `fault(index_out_of_bounds)` |
| `conf.vec-pop-resource` | `Vec<Vec<i32>> v = Vec::new(); Vec::push(&mut v, Vec::new()); match (Vec::pop(&mut v)) { Some(inner) : Vec::len(&inner), None : 9 }` | `→ 0` | push: `[Rawptr-Move-In]`; pop: `[Rawptr-Move-Out]` → temp inside `Some`; `[Match]` on a temp scrutinee moves the payload to `inner` |
| `conf.vec-drop-destroys-elements` | `{ Vec<Vec<i32>> v = Vec::new(); Vec::push(&mut v, Vec::new()); }` | `ok`; the outer `Vec::drop` runs the inner `Vec::drop` (loop) and then `deallocate`; the outer object then ends | outer BE → `[Destroy]` → `[Run-Destructor]` `Vec::drop`: loop `drop(reclaim<T>(…))` → `[Reclaim]` re-attaches the element `[Rawptr-Move-In]` created → `[Destroy]` on it (authority was re-keyed to it); `[Deallocate]` releases; `[Object-End]` of the outer |
| `conf.vec-ref-then-push-rejected` | `Vec<i32> v = Vec::new(); Vec::push(&mut v, 1); auto r = Vec::index_shared(&v, 0); Vec::push(&mut v, 2); *r` | ✗ `diag.aliasing-conflict` (static) | `index_shared` has one reference parameter and returns a reference, so `rule.temporal.elision` and FA record `deriv(r, v.ε, shared)`; `&mut v` then has `¬clash` refuted → `[Borrow-Denied]` statically. Dynamically `a_r'` targets the reclaimed element object rather than `o_v`, so had the borrow been admitted the later reallocation would have ended that object and `*r` would be `[Read-Stale]`; the static rule is the conservative front line |
| `conf.string-from-utf8-ok` | `Vec<u8> b = Vec::new(); Vec::push(&mut b, 104); Vec::push(&mut b, 105); match (String::from_utf8(b)) { Ok(s) : String::len(&s), Err(_) : 99 }` | `→ 2` | `b` transferred into `bytes`; validator loop accepts two ASCII bytes (the `else { return Err(…); }` branch is `never`-typed, `[T-Block]`); `String { .bytes = bytes }` relocates (`[Relocate-In]`: every earlier `&bytes` borrow died with its callee's parameter object); `[Match]` on the temporary `Result` moves `s` out |
| `conf.string-from-utf8-err` | bytes `[255]` | `→ Err(Utf8Error { .offset = 0 })` | first branch chain rejects `255` |
| `conf.string-into-bytes-roundtrip` | `Vec<u8> b = Vec::new(); Vec::push(&mut b, 104); Vec::push(&mut b, 105); auto s = match (String::from_utf8(b)) { Ok(v) : v, Err(_) : String::from_str("") }; usize n1 = String::len(&s); auto bytes2 = String::into_bytes(s); usize n2 = Vec::len(&bytes2);` | `n1 = 2`, `n2 = 2` | `String::len(&s)`: `&s.bytes` — field projection through the reference, base narrowed to the `bytes` sub-range, mode shared; `Vec::len` reads `.len` through it → 2. `String::into_bytes(s)`: `s` by value, `String{bytes} = s;` → `[Let-Destructure]`: `e = s` in place position, `base(a_s,Σ) = None`, `type-of(o_s)` has exactly one field `bytes : Vec<u8>`, `init-state(o_s) = valid`, `solitary(a_s)` (no other path on `o_s`); `is-resource(Vec<u8>) = true` → `relocate-out(o_s, bytes)`: `(o_s,bytes) ∈ destruction-obligations` with authority (set when `from_utf8`'s `[Relocate-In]` first tracked it), so `[Relocate-Out]` promotes it to a fresh temp `o_f`, moving its obligation/authority/storage out and marking the sub-range `uninit`; `store(binding(bytes), temp o_f)` adopts it under the new local `bytes`; `destroy(root path of o_s)`: `is-resource(String) = true` (structural), `authority(ℓ,destroy,o_s)` still held, `solitary` (the `bytes` obligation already left), `destructor(String) = None` → `[Run-Destructor-None]`; `destroy-composite(o_s)` finds nothing left; `end-object(o_s)` releases `s`'s own (already-vacated) storage. The function body's result is `bytes` (now `o_f`); `Vec::len(&bytes2)` → 2 |
| `conf.rc-clone-shares-allocation` | `auto a = Rc::new(1); auto b = Rc::clone(&a);` | `count = 2`, `a.ptr == b.ptr` | `Rc::new(1)`: `T = i32` (plain) so `RcBox<i32>` is plain too; `*bp = RcBox{.count=1,.value=1}` → `[Rawptr-Write]` (¬is-resource; freshly allocated cells, nothing reclaimed there yet) — unlike `conf.e2e-rc-resource-payload`'s resource `T`, no object is established at construction time at all. `Rc::clone(&a)`: `reclaim<RcBox<i32>>(r.ptr)` — no reclaimed object exists yet, so `[Reclaim]`'s fresh-identity branch establishes `o'` (¬is-resource(`RcBox<i32>`): no authority/obligation granted, matching `[Object-Establish-Plain]`'s pattern); `&mut` from its root (exclusive, solitary); `b.count = b.count + 1` → `count = 2`; `Rc{.ptr = r.ptr}` copies the address. `a.ptr == b.ptr`: `[Ref-Identity-Eq]`-adjacent — both are the same `rawptr` value (copied, not compared as `ref`), equal by value |
| `conf.destroy-composite-multi-field-order` | `struct Pair { Vec<i32> a; Vec<i32> b; } fn main() { Vec<i32> va = Vec::new(); Vec::push(&mut va, 1); Vec<i32> vb = Vec::new(); Vec::push(&mut vb, 2); Pair p = Pair { .a = va, .b = vb }; }` | `ok`; `b` destroyed before `a` | `Pair` is a resource by derivation (two resource fields); construction `[Store-Sub-Relocate]` twice tracks obligations `{(o_p,a), (o_p,b)}` (`va`/`vb` each already grown `0→4`). Main BE → `[Destroy]` of `p` → `destroy-composite(o_p)`: `P = [p_i \| (o_p,p_i) ∈ obligations]`, "longest path first, ties by reverse declaration order" (`spec/07` §4) — both `a` and `b` have path length 1 (a tie), read as a lexicographic total order over declaration position at the first (here, only) diverging component: `b`, declared after `a`, sorts first under *reverse* order. `P = [b, a]`: `Vec::drop` of `b`'s Vec (deallocate) runs before `Vec::drop` of `a`'s Vec. Confirms `rule.resauth.destroy-composite`'s ordering clause is a well-defined total order (no two readings differ) for the one shape — sibling resource fields at the same struct depth — no existing derivation had exercised; the order is unobservable here (independent deallocations) but would matter for a struct whose fields have destructors with externally visible side effects, which none in this corpus does |
| `conf.option-unwrap-or` | `Option::unwrap_or(None, 7) + Option::unwrap_or(Some(5), 0)` | `→ 12` | `Option::unwrap_or` (D-0033): `T := i32` from `d`; `None` gives `d`, `Some(5)` gives 5 |
| `conf.option-ok-or` | `Option<i32> n = None; Result::unwrap_or(Option::ok_or(Some(5), "none"), 0) + Result::unwrap_or(Option::ok_or(n, "none"), 7)` | `→ 12` | `Option::ok_or` (D-0114): `Some(5)` gives `Ok(5)`, `None` gives `Err(e)` |
| `conf.option-ok-or-propagates` | `fn twice(Option<i32> o) : Result<i32, str> { i32 v = Option::ok_or(o, "none")?; Ok(v * 2) }` `Result::unwrap_or(twice(Some(4)), 0) + match (twice(None)) { Ok(_) : 0, Err(e) : narrow<i32>(str_len(e)), }` | `→ 12` | `Option::ok_or(o, e)?` returns `Err(e)` from the enclosing function on `None` (D-0114 with `[Propagate]`) |
| `conf.propagate-option` | `impl/conformance/18-error-failure-semantics/propagate_on_option_ok.cb` | ok | `?` on an `Option` in a function returning an `Option`: the payload, or `None` returned (D-0130 `[Propagate-Option]`) |
| `conf.propagate-option-in-result-rejected` | `impl/conformance/18-error-failure-semantics/propagate_option_in_result_rejected.cb` | ✗ `diag.propagate-outside-fallible-context` (static) | `?` on an `Option` in a function returning a `Result`: no conversion; `Option::ok_or(o, e)?` names the error (D-0130 `[Propagate-Option-Mismatch]`) |
| `conf.file-error-text` | `String p = String::from_str("no-such-dir/x.txt"); match (read_file(&p[0..$])) { Ok(_) : 0, Err(e) : str_len(FileError::text(&e)), }` | `→ 9` | `FileError::text`: `NotFound` is "not found" (D-0117) |
| `conf.parse-error-text` | `String s = String::from_str("12x"); String t = String::new(); str a = match (String::parse<i32>(&s)) { Ok(_) : "", Err(e) : ParseError::text(&e), }; str b = match (String::parse<i32>(&t)) { Ok(_) : "", Err(e) : ParseError::text(&e), }; str_len(a) * 10 + str_len(b)` | `→ 125` | `ParseError::text`: `Invalid(_)` is "not a number", `Empty` is "empty" (D-0117) |
| `conf.bytes-le-be` | `impl/conformance/21-standard-library-semantics/bytes_and_crc32_ok.cb` | ok | `Vec::push_le`/`push_be` write the image least or most significant byte first; `read_le`/`read_be` read it back (D-0119) |
| `conf.bytes-signed-roundtrip` | `impl/conformance/21-standard-library-semantics/bytes_and_crc32_ok.cb` | ok | signed values round-trip through their two's-complement image at every width, `i8` to `i128` |
| `conf.bytes-short-buffer-faults` | `impl/conformance/21-standard-library-semantics/bytes_short_buffer_faults.cb` | ✗ `diag.index-out-of-bounds` (dynamic) | `read_le<u16>` with one byte left from `at` |
| `conf.crc32-check` | `String s = String::from_str("123456789"); crc32(&String::as_bytes(&s)[0..$]) == 0xCBF43926` | `→ true` | `[CRC32]`'s check value (D-0119) |
| `conf.crc32-empty` | `Vec<u8> v = Vec::new(); crc32(&v[0..$])` | `→ 0` | `[CRC32]` of no bytes |
| `conf.rng-check-values` | `impl/conformance/21-standard-library-semantics/rng_ok.cb` | ok | `[Rng]`'s check values from seeds 42 and 0 (D-0120) |
| `conf.rng-below-range` | `impl/conformance/21-standard-library-semantics/rng_ok.cb` | ok | `below(r, n)` is in `0 .. n`, and each value about equally often over a run |
| `conf.rng-below-zero-faults` | `impl/conformance/21-standard-library-semantics/rng_below_zero_faults.cb` | ✗ `diag.div-by-zero` (dynamic) | `below(r, 0)` |
| `conf.rng-unit-range` | `impl/conformance/21-standard-library-semantics/rng_ok.cb` | ok | `unit_f64` is in `[0, 1)`; the first from seed 42 is the check value |
| `conf.volatile-read-write` | `impl/conformance/20-trust-boundaries/volatile_ok.cb` | ok | `write_volatile` then `read_volatile` through a `rawptr<u32>`, and against a plain `*p` (`[Rawptr-Read-Volatile]`, `[Rawptr-Write-Volatile]`, D-0121) |
| `conf.volatile-outside-unsafe-rejected` | `impl/conformance/20-trust-boundaries/volatile_outside_unsafe_rejected.cb` | ✗ `diag.trusted-outside-unsafe` (static) | `read_volatile` outside `unsafe` |
| `conf.volatile-resource-rejected` | `impl/conformance/20-trust-boundaries/volatile_resource_rejected.cb` | ✗ `diag.type-mismatch` (static) | `read_volatile` through a `rawptr<String>`: a resource crosses by `*p` |
| `conf.checked-narrow-some` | `u64 big = 70000; Option::unwrap_or(checked_narrow<u32>(big), 0) + widen<u32>(Option::unwrap_or(checked_narrow<u16>(big), 1))` | `→ 70001` | `[Checked-Narrow]` (D-0122): `Some(70000)` into `u32`, `None` into `u16` |
| `conf.checked-narrow-none` | `Option::is_none(&checked_narrow<i8>(200: u8)) && Option::unwrap_or(checked_narrow<i8>(-5: i64), 0) == -5` | `→ true` | a value that does not fit is `None`, one that does is `Some` at the target type (D-0122) |
| `conf.bit-counts` | `impl/conformance/06-arithmetic/bit_counts_ok.cb` | ok | `count_ones`, `leading_zeros`, `trailing_zeros` on the cell image at every width, 0 giving the width (`[Count-Ones]`, `[Leading-Zeros]`, `[Trailing-Zeros]`, D-0123) |
| `conf.bit-rotate` | `impl/conformance/06-arithmetic/bit_counts_ok.cb` | ok | `rotate_left`/`rotate_right` by `k mod width`, signed operands as their image (`[Rotate]`) |
| `conf.bit-count-non-integer-rejected` | `impl/conformance/06-arithmetic/bit_count_non_integer_rejected.cb` | ✗ `diag.type-mismatch` (static) | `count_ones(2.5)` |
| `conf.sleep-at-least` | `impl/conformance/21-standard-library-semantics/sleep_ok.cb` | ok | `sleep_ms(30)` takes at least 30 ms by `monotonic_ns`, while another thread runs (`[Sleep]`, D-0124) |
| `conf.weak-upgrade-while-alive` | `impl/conformance/21-standard-library-semantics/weak_ok.cb` | ok | `Weak::upgrade` gives a strong handle while one exists (D-0125) |
| `conf.weak-upgrade-after-end` | `impl/conformance/21-standard-library-semantics/weak_ok.cb` | ok | after the last strong handle the value ends (its destructor runs) and `upgrade` is `None` |
| `conf.weak-keeps-box` | `impl/conformance/21-standard-library-semantics/weak_ok.cb` | ok | weak handles outlive the value; the box goes with the last of them; a parent pointer through `Weak` |
| `conf.length-constant-expression` | `impl/conformance/16-aggregates/length_constant_expressions_ok.cb` | ok | `array<u8, N * 2 + 1>`, `(N << 1)`, `Vec<array<u8, N * 2>>`, the grammar's precedence (D-0126) |
| `conf.length-later-constant` | `impl/conformance/16-aggregates/length_constant_expressions_ok.cb` | ok | a constant naming one declared after it |
| `conf.length-module-expression` | `impl/conformance/16-aggregates/length_constant_expressions_ok.cb` | ok | a module's constant computed from another, used qualified and in an expression |
| `conf.length-not-constant-rejected` | `impl/conformance/16-aggregates/length_not_constant_rejected.cb` | ✗ `diag.syntax-error` (static) | `array<u8, f()>` |
| `conf.length-negative-rejected` | `impl/conformance/16-aggregates/length_negative_rejected.cb` | ✗ `diag.syntax-error` (static) | `array<u8, N - 3>` with `N = 2` |
| `conf.option-ok-or-error-unused-destroyed` | `impl/conformance/21-standard-library-semantics/option_ok_or_error_unused_destroyed_ok.cb` | ok | the error value is built before the call; on `Some` it is destroyed at the statement's end (D-0114) |
| `conf.result-unwrap-or-resource` | `Result<Vec<i32>, i32> r = Err(3); Vec<i32> v = Result::unwrap_or(r, Vec::new()); Vec::len(&v)` | `→ 0` | `Err(_)` arm: the payload ends with the match; `d` is moved out as the result |
| `conf.map-err-transforms` | `fn scale(i32 e) : i32 { e * 10 }` `Result<i32, i32> r = Err(4); match (Result::map_err(r, scale)) { Ok(v) : v, Err(e) : e }` | `→ 40` | `Result::map_err<i32,i32,i32>(r, scale)`: `T,E1,E2 := i32` inferred from `r`'s and `scale`'s types (`[Generic-Call-Inferred]`); body `match (r) { Ok(v) : Ok(v), Err(e) : Err(f(e)) }` types under `[T-Match]`: `r : Result<i32,i32>`; arm `Err(e)` binds `e : i32`; `f : fn(i32):i32` is `callable` (`[Callable-Fn]`), so `f(e)` types via `[T-Call]` as `i32`; `Err(f(e)) : Result<i32,i32>` matches the arm above's `Ok(v) : Result<i32,i32>` (`rule.type.expected` propagates the function's declared return type into both arms) — exhaustive, `[T-Match]` gives `Result<i32,i32>`, matching `map_err`'s declared return type. Evaluated: `r = Err(4)` selects the `Err` arm, `e := 4`; `scale(4) = 40` (`[Arith-Checked]`, no overflow); `Err(40)` returned; outer `match`'s `Err(e) : e` → `40` |
| `conf.str-literal-type` | `str s = "hi"; str_len(s)` | `→ 2` | `[Str-Literal]`: `"hi"` decodes to `⟪104, 105⟫`, type `str` (`[T-Lit-Str]`; the annotation is compared, not propagated); `[Let]` → `[Store-Binding-Value]` (`¬is-resource(str)`); `str_len(s)`: `s` in value position → `[Read]` copies; `[Str-Len]` → 2 |
| `conf.str-copy-no-move` | `str s = "hi"; str t = s; str_len(s) + str_len(t)` | `→ 4` | `str t = s`: `s` in place position, `¬is-resource(str)` → `[Store-Binding-Place-Copy]`, not `-Transfer`: no `[Authority-Transfer]`, `a_s` stays valid, FA `valid(s) = T`; both `str_len` calls read their argument; `[Arith-Checked]` 2 + 2 |
| `conf.str-eq` | `("ab" == "ab") && ("ab" != "ba")` | `→ true` | `[T-Cmp]`: τ = `str` has `=_str` and is not a resource; `[Eq-Str]` bytewise: equal lengths and bytes → `true`; `[Cmp-Ne]` on unequal bytes → `true`; `[T-Logic]` |
| `conf.str-ordering` | `("a" < "b") && ("ab" > "a") && !("b" <= "abc")` | `→ true` | `[Cmp-Text]` (D-0108): `str` in byte order, a prefix first |
| `conf.str-byte` | `str_byte("hi", 1)` | `→ 105` | `1 : usize` from the parameter type (`rule.type.expected`); `[Str-Byte]`: `k = 1 < n = 2` → `b_1 = 105 : u8` |
| `conf.print-int` | `printf("%v", -42)` | ok | `[Print]`, `[Print-Int]`: writes `-42` |
| `conf.print-float` | `printf("%v", 0.1)` | ok | `[Print-Float]`: the shortest digits reading back as the f64 nearest 0.1 are `1`, k = −1 → `0.1` |
| `conf.print-string-ref` | `String s = String::from_str("x"); printf("%v", &s)` | ok | `ref<String, shared>` is printable; its bytes are written |
| `conf.print-string-by-value-rejected` | `String s = String::from_str("x"); printf("%v", s)` | ✗ `diag.type-mismatch` (static) | `[Print-Not-Printable]`: a `String` value is not printable (it would be moved into `print`) |
| `conf.print-not-printable` | `struct P { i32 x; }` `printf("%v", P { .x = 1 });` | ✗ `diag.type-mismatch` (static) | `[Print-Not-Printable]`: a struct |
| `conf.print-generic-ok` | `fn show<T>(T x) { printf("%v", x); }` `show(2.5)` | ok | `rule.type.kind`: `%v` checked at the instantiation `T = f64`, printable |
| `conf.print-generic-not-printable` | `struct P { i32 x; }` `fn show<T>(T x) { printf("%v", x); }` `show(P { .x = 1 });` | ✗ `diag.type-mismatch` (static) | at the instantiation `T = P`: `[Print-Not-Printable]` |
| `conf.print-output` | `impl/conformance/21-standard-library-semantics/print_output_ok.cb` | ok | `[Print-Int]`, `[Print-Float]` and the other cases of `text`; its exact output is checked by `impl/tests/print_output.rs` |
| `conf.read-line-typed` | `fn next() : Result<Option<String>, FileError> { read_line() }` | ok | `read_line`'s type (`rule.stdlib.read`); never called, so no input is read |
| `conf.read-outside-unsafe-rejected` | `fn f(rawptr<u8> p) : isize { stdin_read(p, 1) }` | ✗ `diag.trusted-outside-unsafe` (static) | `stdin_read` is an `extern` (`[Read]`, an instance of `[Extern-Call]`): `[Unsafe-Rejected]` |
| `conf.read-line-lines` | `impl/conformance/21-standard-library-semantics/read_line_lines_ok.cb` | ok | input `ab\r\n\na\rb\r\nc\r`: lines `ab` (a `\r` before the `\n` is consumed with it, D-0092), `` (empty), `a\rb` (any other `\r` is kept), `c\r` (ended by the end of input, its `\r` kept), then `Ok(None)` |
| `conf.read-line-end-of-input` | `impl/conformance/21-standard-library-semantics/read_line_end_of_input_ok.cb` | ok | empty input: `Ok(None)`, and `Ok(None)` again |
| `conf.read-line-invalid-utf8` | `impl/conformance/21-standard-library-semantics/read_line_invalid_utf8_ok.cb` | ok | input `a\xff\nok\n`: `Err(Utf8(e))`, `e.offset = 1`; the line was consumed, so the next call is `Ok(Some(ok))` |
| `conf.file-read-line-crlf` | `impl/conformance/21-standard-library-semantics/file_read_line_crlf_ok.cb` | ok | D-0092 on a `File`: `ab\r\n\ncd\r` reads as `ab`, `` (empty), `cd\r`, then `Ok(None)` |
| `conf.arg-typed` | `fn first() : Result<String, Utf8Error> { arg(0) }` | ok | `arg`'s type (`rule.stdlib.args`); never called |
| `conf.arg-bytes-outside-unsafe-rejected` | `fn f(rawptr<u8> p) : isize { arg_bytes(0, p, 1) }` | ✗ `diag.trusted-outside-unsafe` (static) | `arg_bytes` is an `extern` (`[Arg-Bytes]`, an instance of `[Extern-Call]`): `[Unsafe-Rejected]` |
| `conf.arg-count-none` | `arg_count()` | `→ 0` | run with no arguments: `[Arg-Bytes]` gives −1 for `i = 0`, so the loop stops at once |
| `conf.arg-out-of-range` | `auto a = arg(0);` | ✗ `diag.index-out-of-bounds` (dynamic) | run with no arguments: `arg_bytes(0, …)` is −1, so `arg` calls `fault(index_out_of_bounds)` |
| `conf.args-read` | `impl/conformance/21-standard-library-semantics/args_read_ok.cb` | ok | arguments `one`, `` (empty) and `wörld`: `arg_count() = 3`; each `arg(i)` is `Ok`, with the argument's bytes (the program's name is not an argument) |
| `conf.arg-invalid-utf8` | `impl/conformance/21-standard-library-semantics/arg_invalid_utf8_ok.cb` | ok | arguments `a\xffb` and `ok`: `arg(0)` is `Err(e)`, `e.offset = 1`; `arg(1)` is still `Ok(ok)` |
| `conf.string-append-text` | `String s = String::new(); String::append(&mut s, 42); String::append(&mut s, " "); String::append(&mut s, 2.5); String::len(&s)` | `→ 6` | `[Append]` three times, `T` = `i32`, `str`, `f64`: `text` gives `42`, ` `, `2.5` (`[Print-Int]`, `[Print-Float]`), pushed onto `s.bytes` |
| `conf.string-append-not-printable` | `String s = String::new(); String::append(&mut s, Some(1));` | ✗ `diag.type-mismatch` (static) | `[Append-Not-Printable]`: `T = Option<i32>` |
| `conf.string-append-self-conflict` | `String s = String::from_str("a"); String::append(&mut s, &s);` | ✗ `diag.aliasing-conflict` (dynamic) | `&mut s` and `&s` are both arguments, so `s`'s bytes are read through a shared reference while an exclusive one to `s` is live (`rule.alias.borrow`) |
| `conf.string-as-bytes` | `String s = String::from_str("hi"); *Vec::index_shared(String::as_bytes(&s), 1)` | `→ 105` | `as_bytes` returns `&s.bytes` (`rule.temporal.elision`: derived from its one reference parameter); element 1 is `i`, 105 |
| `conf.parse-int` | `String s = String::from_str("-42"); match (String::parse<i32>(&s)) { Ok(v) : v, Err(_) : 0 }` | `→ -42` | `[Parse]`: `-42` is a sentence of `number(i32)`, and −42 is a value of `i32` |
| `conf.parse-invalid-offset` | `String s = String::from_str("12x4"); usize k = match (String::parse<i32>(&s)) { Ok(_) : 9, Err(e) : match (e) { Invalid(k) : k, _ : 8 } }; k` | `→ 2` | `[Parse]`: the longest prefix that begins a number is `12`, so `Invalid(2)` |
| `conf.parse-out-of-range` | `String s = String::from_str("128"); bool r = match (String::parse<i8>(&s)) { Ok(_) : false, Err(e) : match (e) { OutOfRange : true, _ : false } }; r` | `→ true` | `[Parse]`: `128` is a sentence of `number(i8)`, but 128 is not a value of `i8` |
| `conf.parse-not-numeric` | `String s = String::new(); auto r = String::parse<String>(&s);` | ✗ `diag.type-mismatch` (static) | `[Parse-Not-Numeric]`: `T = String` (a `bool` reads since D-0181, `[Parse-Bool]`) |
| `conf.text-round-trip` | `impl/conformance/21-standard-library-semantics/text_append_parse_ok.cb` | ok | every integer type's limits and floats in each of `%v`'s forms: `String::parse<T>` of `text(v)` is `v`; each `ParseError`, with `Invalid`'s offsets |
| `conf.read-file-not-found` | `String p = String::from_str("/nonexistent-cobaltc-dir/x.txt"); bool nf = match (read_file(&p[0..$])) { Ok(_) : false, Err(e) : match (e) { NotFound : true, _ : false } }; nf` | `→ true` | `[Read-File]`: no such file |
| `conf.write-file-missing-dir` | `String p = String::from_str("/nonexistent-cobaltc-dir/x.txt"); String t = String::from_str("x"); bool nf = match (write_file(&p[0..$], &t[0..$])) { Ok(_) : false, Err(e) : match (e) { NotFound : true, _ : false } }; nf` | `→ true` | `[Write-File]`: a directory on the path does not exist |
| `conf.file-round-trip` | `impl/conformance/21-standard-library-semantics/files_round_trip_ok.cb` | ok | `write_file` then `read_file` give back the text; a second write replaces it; `NotFound` for a missing file and a missing directory; a fixture whose third byte is 0xFF is `Utf8(e)`, `e.offset = 2` |
| `conf.file-handle-round-trip` | `impl/conformance/21-standard-library-semantics/file_handle_round_trip_ok.cb` | ok | `[File-Open]` in each mode, `[File-Write]` of text and of bytes that are not text, `[File-Read-Line]` (and `Utf8(e)`, `e.offset = 2`, on the fixture), `[File-Read]` after `[File-Seek]`, `[File-Close]`, an `open_rw` write where reading stopped, `read_bytes`/`write_bytes`, `NotFound` |
| `conf.file-destructor-closes` | `impl/conformance/21-standard-library-semantics/file_destructor_closes_ok.cb` | ok | `[File-Drop]`: 2000 files opened and never closed by the program are each closed by the destructor; two open files keep their own positions |
| `conf.file-moved-to-thread` | `impl/conformance/21-standard-library-semantics/file_moved_to_thread_ok.cb` | ok | a `File` moves into a spawned thread, which writes and closes it |
| `conf.file-use-after-close-rejected` | `impl/conformance/21-standard-library-semantics/file_use_after_close_rejected.cb` | ✗ `diag.stale-binding` (static) | `close` takes the `File`: writing to it afterwards names a moved binding |
| `conf.file-forged-rejected` | `File f = File { .id = 0, .open = true };` | ✗ `diag.name-not-visible` (static) | a `File`'s fields are private to `std` (`[Field-Not-Visible]`) |
| `conf.file-printf` | `impl/conformance/21-standard-library-semantics/file_printf_ok.cb` | ok | `[File-Printf]`: formatted text to a file; nothing for an empty format; `Err(Io)` on a read-only file |
| `conf.file-printf-format` | `impl/conformance/21-standard-library-semantics/file_printf_format_rejected.cb` | ✗ `diag.type-mismatch` (static) | the format is checked as `printf`'s is |
| `conf.propagate-temporary-consumed` | `impl/conformance/18-error-failure-semantics/propagate_temporary_ends_at_question_ok.cb` | ok | D-0060: `match (peek(s)?)` holds only `?`'s reference; a named `Result` stays usable after `r?` |
| `conf.fs-env-clocks` | `impl/conformance/21-standard-library-semantics/fs_env_clocks_ok.cb` | ok | `rule.stdlib.fs`: `make_dir` twice, `list_dir` in byte order with `path_kind`s, `rename`, a full directory refused, removals, `NotFound`s; `rule.stdlib.env`: `PATH` set, an unlikely name not, the working directory; the clocks |
| `conf.sort-search` | `impl/conformance/21-standard-library-semantics/sort_search_ok.cb` | ok | `[Sort]`, `[Sort-By]` (stable), `[Binary-Search]` over integers, `bool`s, `String`s, records and `Vec`s |
| `conf.sort-not-key-type` | `impl/conformance/21-standard-library-semantics/sort_not_key_type_rejected.cb` | ✗ `diag.type-mismatch` (static) | `[Sort-Not-Key]`: a struct with an `f64` field (a bare float orders since D-0181, `[Sort-Float]`) |
| `conf.sort-by-comparator-type` | `impl/conformance/21-standard-library-semantics/sort_by_comparator_type_rejected.cb` | ✗ `diag.type-mismatch` (static) | a comparison of `i64`s for a `Vec<i32>` |
| `conf.sort-element-borrowed` | `impl/conformance/21-standard-library-semantics/sort_while_element_borrowed_rejected.cb` | ✗ `diag.aliasing-conflict` (static) | sorting borrows the Vec exclusively |
| `conf.printf-output` | `impl/conformance/21-standard-library-semantics/printf_output_ok.cb` | ok | every conversion and flag, with C's text (`%#o` aside: `0o`); `String::appendf` into a `String` |
| `conf.printf-arg-mismatch` | `printf("%s", 42);` | ✗ `diag.type-mismatch` (static) | `[Format-Arg-Mismatch]`: `%s` takes text, not an `i32` |
| `conf.printf-count-mismatch` | `printf("%d %d", 1);` | ✗ `diag.type-mismatch` (static) | `[Format-Arg-Mismatch]`: two specifiers, one argument |
| `conf.printf-format-invalid` | `printf("%q", 1);` | ✗ `diag.format-invalid` (static) | `[Format-Invalid]`: `q` is not a conversion |
| `conf.printf-format-not-literal` | `str f = "%d"; printf(f, 1);` | ✗ `diag.format-invalid` (static) | `[Format-Invalid]`: the format is not a string literal |
| `conf.printf-generic-instantiation` | `fn show<T>(T x) { printf("%d", x); }` `show(2.5)` | ✗ `diag.type-mismatch` (static) | checked at the instantiation `T = f64`, as for `%v` |
| `conf.appendf-builds-string` | `String s = String::new(); String::appendf(&mut s, "%05.1f|%x", -2.5, 255); String::len(&s)` | `→ 8` | `[Appendf]`: `-02.5|ff` |
| `conf.appendf-self` | `String s = String::from_str("ab"); String::appendf(&mut s, "%s-", &s); String::len(&s)` | `→ 5` | the text is formatted, from a copy of `s`, before it is appended: `abab-` |
| `conf.printf-value` | `String s = sprintf("%v|%-3v|%3v|%v", 0.1, 7, "ab", false); String::len(&s)` | `→ 17` | `%v` is `text(v)` (`rule.stdlib.print`), padded: `0.1|7  | ab|false` |
| `conf.printf-value-precision-rejected` | `printf("%.2v", 1.5);` | ✗ `diag.format-invalid` (static) | `[Format-Invalid]`: `%v` takes no precision |
| `conf.sprintf-returns-string` | `String s = sprintf("%d-%s", 12, "ab"); String::len(&s)` | `→ 5` | `[Sprintf]`: a new `String` holding `12-ab` |
| `conf.print-unbound` | `print(1);` | ✗ `diag.unbound-name` (static) | `print` is not exported from `std` (D-0039), so `import std;` brings no `print` |
| `conf.std-print-private` | `std::print(1);` | ✗ `diag.name-not-visible` (static) | `[Resolve-Not-Visible]`: `std::print` is private to `std` |
| `conf.print-own-declaration-ok` | `fn print(i32 x) { printf("%d", x); }` `print(1);` | ok | a program's own `print` is no duplicate of `std`'s private one |
| `conf.eprintf-stderr` | `impl/conformance/21-standard-library-semantics/eprintf_stderr_ok.cb` | ok | `[Eprintf]`: standard output holds only `printf`'s text; `impl/tests/print_output.rs` checks standard error |
| `conf.eprintf-format-checked` | `eprintf("%d\n", "x");` | ✗ `diag.type-mismatch` (static) | `[Format-Arg-Mismatch]`, as for `printf` |
| `conf.stderr-write-private` | `unsafe { std::stderr_write(str_ptr("x"), 1); }` | ✗ `diag.name-not-visible` (static) | `[Resolve-Not-Visible]`: the extern `eprintf` writes through is private to `std` |
| `conf.stdout-buffer-order` | `impl/conformance/21-standard-library-semantics/stdout_buffer_order_ok.cb` | ok | `[Print-Buffer]` (D-0178): `eprintf` writes standard output's buffer out first; `impl/tests/print_output.rs` and `impl/cobc/tests/output_order.rs` send both streams to one file and find `a`, `b`, `c` in order |
| `conf.flush-stdout` | `impl/conformance/21-standard-library-semantics/flush_stdout_ok.cb` | ok | `[Print-Buffer]`: `flush_stdout` changes no byte of the output, and with nothing buffered does nothing |
| `conf.stdout-buffer-boundary` | `impl/conformance/21-standard-library-semantics/stdout_buffer_boundary_ok.cb` | ok | `[Print-Buffer]`: writes across the buffer's boundaries and one larger than it arrive whole and in order |
| `conf.stdout-buffer-child` | `impl/conformance/21-standard-library-semantics/stdout_buffer_child_ok.cb` | ok | `[Print-Buffer]`: a child started by `Command::status` writes after what the program wrote before it (Unix) |
| `conf.stdout-flush-private` | `unsafe { std::stdout_flush(); }` | ✗ `diag.name-not-visible` (static) | `[Resolve-Not-Visible]`: the primitive `flush_stdout` is written over is private to `std` |
| `conf.eval-order-reads` | `impl/conformance/13-expression-semantics/eval_order_reads_ok.cb` | ok | `[Call]`, `[LValue-To-RValue]` (D-0007): a place in value position is read when it is evaluated, so an argument (of a function or a closure, an element included) or a struct literal field is the value before a later argument or field writes it |
| `conf.elem-write-rhs-grows` | `impl/conformance/05-value-object-semantics/elem_write_rhs_grows_vec_rejected.cb` | ✗ `diag.stale-binding` (dynamic) | `[Write-Stale]`: `v[0].x = e` reaches the element before `e`; `e` grows `v` through a helper, so the element reached has moved and the write is stale |
| `conf.view-arg-not-pending` | `impl/conformance/15-function-semantics/view_argument_and_mut_borrow_rejected.cb` | ✗ `diag.aliasing-conflict` (dynamic) | `[Call]` (D-0107, D-0179): the view `String::as_view(&s)` returns is a value the first argument computed; its path is not pending, so forming `&mut s` for the second argument meets it |
| `conf.index-borrow-whole-element` | `impl/conformance/16-aggregates/index_element_borrow_same_element_rejected.cb` | ✗ `diag.aliasing-conflict` (dynamic) | `[Index-Vec]` (D-0070): with `&mut p[1].x` live, `p[0].y` is reached (another element); with `&mut p[1].y` live, `p[1].x` is not (the borrow is checked as an access to the whole element 1) |
| `conf.fault-unwind-first-fault` | `impl/conformance/18-error-failure-semantics/fault_unwind_destructor_fault_first_wins.cb` | ✗ `diag.index-out-of-bounds` (dynamic) | `[Fault-Unwind]`: the unwind runs `inner`'s destructors; one of them faults, and the program still reports the fault that started the unwind, destroying nothing after the second (`main`'s destructor prints nothing) |
| `conf.hashmap-basic` | `HashMap<i32, bool> m = HashMap::new(); HashMap::insert(&mut m, 5, true); i32 k = 5; i32 j = 6; HashMap::len(&m) == 1 && HashMap::contains(&m, &k) && !HashMap::contains(&m, &j)` | `→ true` | `rule.stdlib.hashmap`: one entry; 5 found, 6 not |
| `conf.hashmap-insert-replaces` | `HashMap<str, i32> m = HashMap::new(); HashMap::insert(&mut m, "a", 1); i32 old = Option::unwrap_or(HashMap::insert(&mut m, "a", 2), 0); str k = "a"; i32 now = match (HashMap::get(&m, &k)) { Some(v) : *v, None : 0 }; old == 1 && now == 2 && HashMap::len(&m) == 1` | `→ true` | the second `insert` returns `Some(1)`; `get` gives 2; `len` stays 1 |
| `conf.hashmap-entry-counts` | `HashMap<u8, u32> m = HashMap::new(); for (u8 i = 0; i < 100; i += 1) { *HashMap::entry(&mut m, i % 7, 0) += 1; } HashMap::len(&m) == 7 && *HashMap::value_at(&m, 0) == 15 && *HashMap::key_at(&m, 6) == 6` | `→ true` | seven keys in insertion order 0..6; key 0 counted 15 times |
| `conf.hashmap-swap-remove` | `HashMap<i32, i32> m = HashMap::new(); HashMap::insert(&mut m, 1, 10); HashMap::insert(&mut m, 2, 20); HashMap::insert(&mut m, 3, 30); i32 k = 1; i32 v = Option::unwrap_or(HashMap::swap_remove(&mut m, &k), 0); v == 10 && *HashMap::key_at(&m, 0) == 3 && *HashMap::key_at(&m, 1) == 2` | `→ true` | `swap_remove` returns `Some(10)`; the last entry (3) takes position 0: order 3, 2 (D-0134) |
| `conf.hashmap-remove-keeps-order` | `HashMap<i32, i32> m = HashMap::new(); HashMap::insert(&mut m, 1, 10); HashMap::insert(&mut m, 2, 20); HashMap::insert(&mut m, 3, 30); i32 k = 1; i32 v = Option::unwrap_or(HashMap::remove(&mut m, &k), 0); v == 10 && *HashMap::key_at(&m, 0) == 2 && *HashMap::key_at(&m, 1) == 3` | `→ true` | `remove` returns `Some(10)`; the others keep their order: 2, 3 (D-0134) |
| `conf.hashset-basic` | `HashSet<String> s = HashSet::new(); bool a = HashSet::insert(&mut s, String::from_str("x")); bool b = HashSet::insert(&mut s, String::from_str("x")); String k = String::from_str("x"); a && !b && HashSet::len(&s) == 1 && HashSet::remove(&mut s, &k) && HashSet::len(&s) == 0` | `→ true` | `insert` is true, then false; `remove` is true; `len` 1 then 0 |
| `conf.str-literal-as-view` | `impl/conformance/21-standard-library-semantics/str_literal_views_ok.cb` | ok | `[Str-Literal-View]`: a literal as an argument, a local's initializer, a struct literal's field, and a `str` constant's use is a view; `StringView::of` of a `str` binding; such views sliced, kept and compared (D-0134) |
| `conf.view-of-str` | `str t = "a,b"; Vec::len(&StringView::split(StringView::of(t), ",")) == 2` | `→ true` | `StringView::of`: a view of a `str`'s bytes (§2h, D-0134) |
| `conf.view-of-str-binding-rejected` | `str t = "abc"; StringView v = t;` | ✗ `diag.type-mismatch` (static) | `[Str-Literal-View]` types a literal only; `t` is a `str`, and no conversion is implicit (D-0006, D-0134) |
| `conf.str-literal-as-string` | `impl/conformance/21-standard-library-semantics/str_literal_string_ok.cb` | ok | `[Str-Literal-String]` (D-0149): a literal as a `String` local's initializer, a struct literal's `String` field, an argument whose parameter is `String`, a `str` constant's use, and what a function declared `: String` returns (`return`, the tail, an `if` branch, a `match` arm) is `String::from_str` of it; `auto` keeps a literal a `str` |
| `conf.string-of-str-binding-rejected` | `impl/conformance/21-standard-library-semantics/string_of_str_binding_rejected.cb` | ✗ `diag.type-mismatch` (static) | `[Str-Literal-String]` types a literal only; `str t = "abc"; String s = t;` is a conversion of a value, which no rule makes implicit (D-0006) |
| `conf.str-literal-in-arms` | `impl/conformance/21-standard-library-semantics/str_literal_in_arms_ok.cb` | ok | `[Str-Literal-String]`, `[Str-Literal-View]` (D-0155): literals given by `match` arms, `if` branches and a block, as a `String` local's initializer, a `String` field, a `String` argument and a `StringView` local |
| `conf.stringview-as-bytes` | `impl/conformance/21-standard-library-semantics/stringview_as_bytes_ok.cb` | ok | `[View-As-Bytes]` (D-0163): a literal's, a `String`'s and a part's bytes as a slice, passed to `sha256` and `hmac_sha256` |
| `conf.text-mixed-eq` | `impl/conformance/21-standard-library-semantics/text_mixed_eq_ok.cb` | ok | `[Text-Mixed-Eq]` (D-0164): `String` against a literal, a `str` binding and a view, either side, `==` and `!=`; a temporary `String`; the `String` is not moved |
| `conf.text-mixed-order-rejected` | `impl/conformance/21-standard-library-semantics/text_mixed_order_rejected.cb` | ✗ `diag.type-mismatch` (static) | `[Text-Mixed-Eq]`: `String < str` is not defined |
| `conf.str-literal-generic-string` | `impl/conformance/21-standard-library-semantics/str_literal_generic_string_ok.cb` | ok | `[Str-Literal-String]` (D-0162): literal defaults of `Result::unwrap_or` and `Option::unwrap_or`, elements of a `Vec<String>`, a key of a `HashMap<String, i32>`, and a program's own generic function's `String` argument; a literal nothing fixes stays `str` |
| `conf.string-needle` | `String s = String::from_str("abcabc"); String n = String::from_str("ca"); String::find(&s, &n[0..$]) == Some(2)` | `→ true` | the text `find` looks for is a `StringView`: a `String`'s `&n[0..$]` (D-0134) |
| `conf.string-contains` | `impl/conformance/21-standard-library-semantics/string_replace_join_contains_ok.cb` | ok | `[Contains]`: `String::contains(&s, t)` and `StringView::contains(v, t)` are whether `find` is `Some`; the empty text is in every text (D-0139) |
| `conf.string-replace` | `impl/conformance/21-standard-library-semantics/string_replace_join_contains_ok.cb` | ok | `[Replace]`: every occurrence, left to right, not overlapping (`"aaa"`, `"aa"` → `"b"` gives `"ba"`); an empty `from` gives a copy; multi-byte characters replaced whole; the arguments unchanged (D-0139) |
| `conf.string-join` | `impl/conformance/21-standard-library-semantics/string_replace_join_contains_ok.cb` | ok | `[Join]`: `StringView::join` of a slice of views and `String::join` of a slice of `String`s, with `sep` between each two; one part gives itself, no parts the empty `String`; `join` of `split` gives the text back (D-0139) |
| `conf.read-file-literal-path` | `bool nf = match (read_file("/nonexistent-cobaltc-dir/x.txt")) { Ok(_) : false, Err(e) : match (e) { NotFound : true, _ : false } }; nf` | `→ true` | a literal path is a view (`[Str-Literal-View]`); `[Read-File]`: no such file |
| `conf.hashset-clear` | `HashSet<i32> s = HashSet::new(); HashSet::insert(&mut s, 1); HashSet::insert(&mut s, 2); HashSet::clear(&mut s); HashSet::len(&s) == 0 && HashSet::insert(&mut s, 1)` | `→ true` | `HashSet::clear` empties the set, which stays usable (D-0134) |
| `conf.hashset-remove-keeps-order` | `impl/conformance/21-standard-library-semantics/hashset_clear_and_order_ok.cb` | ok | `HashSet::remove` keeps the others' order, `swap_remove` moves the last key into the hole, `clear` empties (D-0134) |
| `conf.hashset-union` | `impl/conformance/21-standard-library-semantics/hashset_set_operations_ok.cb` | ok | `[Set-Ops]`: the first set's keys in its order, then the second's new keys in theirs; a set with itself or with an empty set (D-0138) |
| `conf.hashset-intersection` | `impl/conformance/21-standard-library-semantics/hashset_set_operations_ok.cb` | ok | `[Set-Ops]`: the first set's keys that are in the second, in the first's order; with an empty set, empty (D-0138) |
| `conf.hashset-difference` | `impl/conformance/21-standard-library-semantics/hashset_set_operations_ok.cb` | ok | `[Set-Ops]`: the first set's keys not in the second, both ways round; a set minus itself is empty; the arguments unchanged (D-0138) |
| `conf.view-in-thread` | `impl/conformance/21-standard-library-semantics/view_formed_in_thread_ok.cb` | ok | `[View-Form]` inside a spawned thread: the thread runs the function's own body (D-0134) |
| `conf.hashmap-float-key-rejected` | `HashMap<f64, i32> m = HashMap::new();` | ✗ `diag.type-mismatch` (static) | `[Key-Not-Hashable]`: `f64` is not a key type |
| `conf.hashmap-struct-key-rejected` | `struct P { f32 x; }` `HashSet<P> s = HashSet::new();` | ✗ `diag.type-mismatch` (static) | `[Key-Not-Hashable]`: a struct with a field that is not a key type (`f32`) is not one (D-0110) |
| `conf.hashmap-generic-key-checked` | `fn keep<K>(K k) { HashSet<K> s = HashSet::new(); HashSet::insert(&mut s, k); }` `keep(1.5)` | ✗ `diag.type-mismatch` (static) | `[Key-Not-Hashable]` at the instantiation `K = f64` |
| `conf.queue-fifo` | `impl/conformance/21-standard-library-semantics/queue_fifo_ok.cb` | ok | `[Queue]`: `push_back` then `pop_front` is first in, first out, also with elements added while earlier ones wait (D-0137) |
| `conf.queue-empty` | `Queue<i32> q = Queue::new(); Queue::len(&q) == 0 && Option::is_none(&Queue::pop_front(&mut q)) && Option::is_none(&Queue::pop_back(&mut q)) && Option::is_none(&Queue::front(&q))` | `→ true` | `[Queue]`: an empty queue gives `None` from either end (D-0137) |
| `conf.queue-both-ends` | `impl/conformance/21-standard-library-semantics/queue_both_ends_ok.cb` | ok | `[Queue]`: `push_front` before the front, `pop_back` from the back; taking from the two ends in turn keeps both orders (D-0137) |
| `conf.queue-destroy` | `impl/conformance/21-standard-library-semantics/queue_destroys_front_to_back_ok.cb` | ok | `[Queue]`: `clear` and the queue's end destroy the elements front to back; a taken element is the taker's (D-0137) |
| `conf.queue-front-borrows` | `impl/conformance/21-standard-library-semantics/queue_front_borrow_rejected.cb` | ✗ `diag.aliasing-conflict` (static) | `front`'s reference borrows the queue (`rule.temporal.elision`), so a `push_back` while it is used is rejected (D-0137) |
| `conf.priority-queue-key-order` | `impl/conformance/21-standard-library-semantics/priority_queue_key_order_ok.cb` | ok | `[Priority-Queue]` with the key types' order: least first, a struct field by field, text by its bytes; `peek` takes nothing (D-0137) |
| `conf.priority-queue-by` | `impl/conformance/21-standard-library-semantics/priority_queue_by_ties_fifo_ok.cb` | ok | `[Priority-Queue]` with `new_by`'s `less` ("greater": largest first) (D-0137) |
| `conf.priority-queue-by-any-type` | `impl/conformance/21-standard-library-semantics/priority_queue_by_any_type_ok.cb` | ok | `new_by` orders any element type: references, and `Rc` handles the queue owns; elements moved within the heap keep what they hold (D-0137) |
| `conf.priority-queue-ties` | `impl/conformance/21-standard-library-semantics/priority_queue_by_ties_fifo_ok.cb` | ok | `[Priority-Queue]`: elements `less` calls equal come out in arrival order, also with others pushed and popped between them (D-0137) |
| `conf.priority-queue-destroy` | `impl/conformance/21-standard-library-semantics/priority_queue_clear_destroys_ok.cb` | ok | `clear` and the queue's end destroy the elements in the order the heap holds them (D-0137) |
| `conf.priority-queue-not-key` | `impl/conformance/21-standard-library-semantics/priority_queue_not_key_rejected.cb` | ✗ `diag.type-mismatch` (static) | `[Priority-Queue-Not-Key]`: `Vec<u8>` (D-0137; a float orders since D-0181) |
| `conf.priority-queue-generic-key-checked` | `fn keep<T>(T x) { PriorityQueue<T> q = PriorityQueue::new(); PriorityQueue::push(&mut q, x); }` `Vec<i32> v = Vec::new(); keep(v);` | ✗ `diag.type-mismatch` (static) | `[Priority-Queue-Not-Key]` at the instantiation `T = Vec<i32>` (D-0137; a float orders since D-0181, `[Sort-Float]`) |
| `conf.hashmap-fields-private` | `HashMap<i32, i32> m = HashMap::new(); usize n = Vec::len(&m.keys);` | ✗ `diag.name-not-visible` (static) | `HashMap`'s fields are private to `std` |
| `conf.key-hash-std-only` | `i32 k = 1; key_hash(&k)` | ✗ `diag.unbound-name` (static) | `key_hash` is a std-only intrinsic |
| `conf.hashmap-output` | `impl/conformance/21-standard-library-semantics/hashmap_ok.cb` | ok | insertion order, both removals, growth and resource destruction, output pinned |
| `conf.hashmap-struct-key` | `impl/conformance/21-standard-library-semantics/hashmap_composite_keys_ok.cb` | `ok` | D-0110: a struct of `i32`s, and one with a `String` field, are key types; an equal key is found whatever its storage |
| `conf.hashmap-enum-key` | `impl/conformance/21-standard-library-semantics/hashmap_composite_keys_ok.cb` | `ok` | D-0110: an enum whose payloads are key types is one (`[Key-Bytes]`: variant index, then payload) |
| `conf.sort-struct-key` | `impl/conformance/21-standard-library-semantics/hashmap_composite_keys_ok.cb` | `ok` | `[Sort]` (D-0110): field by field; an enum by variant order, then payload |
| `conf.hashmap-key-float-field-rejected` | `impl/conformance/21-standard-library-semantics/hashmap_key_float_field_rejected.cb` | ✗ `diag.type-mismatch` (static) | `[Key-Not-Hashable]`: a field `f64` is not a key type; the message names the field |
| `conf.foreach-borrowed` | `Vec<i32> v = Vec::new(); Vec::push(&mut v, 3); Vec::push(&mut v, 4); i32 s = 0; foreach (x in &v) { s += *x; } s == 7 && Vec::len(&v) == 2` | `→ true` | `[Foreach-Borrowed]`: `x : ref<i32, shared>`; `v` usable after |
| `conf.foreach-mut` | `Vec<i32> v = Vec::new(); Vec::push(&mut v, 3); Vec::push(&mut v, 4); foreach (x in &mut v) { *x *= 10; } *Vec::index_shared(&v, 1) == 40` | `→ true` | `x : ref<i32, exclusive>`: each element changed in place |
| `conf.foreach-consumed` | `Vec<String> v = Vec::new(); Vec::push(&mut v, String::from_str("ab")); Vec::push(&mut v, String::from_str("cde")); usize n = 0; foreach (s in v) { n += String::len(&s); } n == 5` | `→ true` | `[Foreach-Consumed]`: each `String` moved into `s` and destroyed with it |
| `conf.foreach-index` | `Vec<i32> v = Vec::new(); Vec::push(&mut v, 3); Vec::push(&mut v, 4); usize last = 0; foreach (i, x in &v) { last = i; } last == 1` | `→ true` | with two names the first is the position, `usize` |
| `conf.foreach-array` | `array<i32, 3> a = [1, 2, 3]; foreach (x in &mut a) { *x += 1; } i32 s = 0; foreach (x in a) { s += x; } s == 9` | `→ true` | an array by `&mut`, then by value: element `$i` read from it |
| `conf.foreach-hashmap` | `HashMap<str, i32> m = HashMap::new(); HashMap::insert(&mut m, "a", 1); HashMap::insert(&mut m, "b", 2); foreach (k, v in &mut m) { *v *= 3; } i32 s = 0; foreach (k, v in m) { s += v; } s == 9` | `→ true` | `k : ref<str, shared>`, `v : ref<i32, exclusive>`; then the map consumed, key and value moved out |
| `conf.foreach-hashset` | `HashSet<u8> h = HashSet::new(); HashSet::insert(&mut h, 4); HashSet::insert(&mut h, 5); u8 s = 0; foreach (x in &h) { s += *x; } s == 9` | `→ true` | a set's keys in insertion order |
| `conf.foreach-ref-binding` | `Vec<i32> v = Vec::new(); Vec::push(&mut v, 2); ref<Vec<i32>, shared> r = &v; i32 s = 0; foreach (x in r) { s += *x; } s == 2` | `→ true` | a reference written without `&` is looped over as the borrow it is |
| `conf.foreach-in-is-a-name` | `i32 in = 4; in == 4` | `→ true` | `in` is reserved only inside a `foreach` (spec/22) |
| `conf.foreach-change-collection-rejected` | `Vec<i32> v = Vec::new(); foreach (x in &v) { Vec::push(&mut v, 1); }` | ✗ `diag.aliasing-conflict` (static) | the loop's shared borrow of `v` lasts the loop |
| `conf.foreach-consumed-then-used-rejected` | `Vec<i32> v = Vec::new(); foreach (x in v) { } usize n = Vec::len(&v);` | ✗ `diag.stale-binding` (static) | the consuming loop moved `v` |
| `conf.foreach-not-iterable` | `foreach (x in 5) { }` | ✗ `diag.type-mismatch` (static) | `[Foreach-Not-Iterable]`: `i32` is not a collection |
| `conf.foreach-hashmap-one-name-rejected` | `HashMap<i32, i32> m = HashMap::new(); foreach (x in &m) { }` | ✗ `diag.type-mismatch` (static) | `[Foreach-Not-Iterable]`: a map is looped over with two names |
| `conf.foreach-hashset-mut-rejected` | `HashSet<i32> h = HashSet::new(); foreach (x in &mut h) { }` | ✗ `diag.type-mismatch` (static) | `[Foreach-Not-Iterable]`: a set's keys are not changed in place |
| `conf.foreach-output` | `impl/conformance/14-control-flow/foreach_ok.cb` | ok | every form over every collection, leaving early with resources, output pinned |
| `conf.printf-ref-value` | `i32 x = 5; ref<i32, shared> r = &x; String s = sprintf("%d|%v|%x", r, r, r); String::len(&s) == 5` | `→ true` | a reference to a number is formatted as the number (D-0042) |
| `conf.string-clone` | `String a = String::from_str("ab"); String b = String::clone(&a); String::append(&mut b, "c"); String::len(&a) == 2 && String::len(&b) == 3` | `→ true` | a new `String` with the same bytes; the original is untouched |
| `conf.string-push-ascii` | `String s = String::new(); String::push_ascii(&mut s, b'o'); String::push_ascii(&mut s, b'k'); String::len(&s) == 2` | `→ true` | two ASCII bytes added |
| `conf.string-push-ascii-rejects-non-ascii` | `String s = String::new(); String::push_ascii(&mut s, 200);` | ✗ `diag.not-ascii` (dynamic) | `[Push-Ascii-Not-Ascii]`: 200 is not UTF-8 on its own |
| `conf.vec-clear` | `Vec<String> v = Vec::new(); Vec::push(&mut v, String::from_str("a")); Vec::clear(&mut v); Vec::push(&mut v, String::from_str("b")); Vec::len(&v) == 1` | `→ true` | the `String` is destroyed, the buffer kept, and the Vec reused |
| `conf.destructure-all-fields` | `struct P { String a; i32 b; }` `P { a, b } = P { .a = String::from_str("x"), .b = 2 }; String::len(&a) == 1 && b == 2` | `→ true` | `[Let-Destructure]` over both fields: the `String` relocates out, the `i32` is copied |
| `conf.destructure-missing-field-rejected` | `struct P { i32 a; i32 b; }` `P p = P { .a = 1, .b = 2 }; P { a } = p;` | ✗ `diag.type-mismatch` (static) | `[Let-Destructure-Fields]`: `b` is not named |
| `conf.destructure-destructor-rejected` | `resource struct T { i32 a; } fn T::drop(ref<T, exclusive> self) { }` `T t = T { .a = 1 }; T { a } = t;` | ✗ `diag.move-out-of-field` (static) | `[Let-Destructure-Destructor]`: `T::drop` would run on a struct its field had left |
| `conf.foreach-map-position` | `HashMap<str, i32> m = HashMap::new(); HashMap::insert(&mut m, "a", 1); HashMap::insert(&mut m, "b", 2); usize last = 9; foreach (i, k, v in &m) { last = i; } last == 1` | `→ true` | three names over a map: position, key, value |
| `conf.foreach-three-names-rejected` | `Vec<i32> v = Vec::new(); foreach (i, j, x in &v) { }` | ✗ `diag.type-mismatch` (static) | `[Foreach-Not-Iterable]`: three names are for a map only |
| `conf.closure-borrow-escapes-to-caller` | `impl/conformance/10-temporal-validity/closure_borrow_escapes_to_caller_rejected.cb` | ✗ `diag.reference-escapes-scope` (static) | a closure without `move` returned from the function whose local it borrows (CHG-0113) |
| `conf.closure-borrow-escapes-block` | `impl/conformance/10-temporal-validity/closure_borrow_escapes_block_rejected.cb` | ✗ `diag.reference-escapes-scope` (static) | a borrowing closure stored into a binding of an enclosing block (CHG-0113) |
| `conf.closure-move-escape-ok` | `impl/conformance/10-temporal-validity/closure_move_escape_ok.cb` | ok | a `move` closure owns its captures and may leave their scope (CHG-0113) |
| `conf.destroy-temporary-while-viewed` | `impl/conformance/07-resource-authority/destroy_temporary_while_viewed.cb` | ✗ `diag.destroy-while-aliased` (dynamic) | `[Stmt-Exit]` destroys the temporary `String` while the views held by `parts` still reach it |
| `conf.destroy-local-while-viewed-outside` | `impl/conformance/07-resource-authority/destroy_local_while_viewed_outside.cb` | ✗ `diag.destroy-while-aliased` (dynamic) | `[Block-Exit]` destroys `s` while views of it are held outside its block |
| `conf.closure-calls-borrowed-capture` | `impl/conformance/15-function-semantics/closure_calls_borrowed_capture_ok.cb` | ok | calling a `fn` value and a closure captured by borrow (`*self.f_i`) |
| `conf.spawn-closure-capturing-reference` | `impl/conformance/19-concurrency/spawn_closure_capturing_reference_ok.cb` | ok | a spawned `move` closure's captured reference ends with its thread (CHG-0114) |
| `conf.closure-borrowed-capture-operations` | `impl/conformance/15-function-semantics/closure_borrowed_capture_operations_ok.cb` | ok | indexes, fields, lengths and a match on values a closure captured by borrow |
| `conf.qualified-variant-clash` | `impl/conformance/17-modules/qualified_variant_clash_ok.cb` | ok | `n::Circle(...)` is module `n`'s variant even beside a local enum's `Circle` |
| `conf.map-err-resource-error` | `impl/conformance/21-standard-library-semantics/map_err_resource_error_ok.cb` | ok | `Result::map_err` whose function returns a String, and a type with a destructor (run once) |
| `conf.return-bare-in-arm` | `fn f(Option<i32> o) { match (o) { Some(_) : {}, None : return, } }` `f(None); f(Some(1)); 7` | `→ 7` | `'return' expr?` as a match arm of a void function, before the arm's `,` |
| `conf.float-rem-rejected` | `fn f(f64 a, f64 b) : f64 { a % b }` | ✗ `diag.type-mismatch` (static) | `[T-Arith]`: a float has `+ - * /` only |
| `conf.float-bitand-rejected` | `fn f(f32 a, f32 b) : f32 { a & b }` | ✗ `diag.type-mismatch` (static) | `[T-Arith]`: `&` is for integers |
| `conf.bool-arith-rejected` | `fn f(bool a, bool b) : bool { a & b }` | ✗ `diag.type-mismatch` (static) | `[T-Arith]`: `bool` has `&&`, `\|\|`, `!=` |
| `conf.compare-resource-results-rejected` | `fn mk() : Vec<u32> { Vec::new() }` `mk() == mk()` | ✗ `diag.read-of-resource` (static) | `[T-Cmp]`: not a resource, whatever the operands' form |
| `conf.logic-int-rejected` | `i32 a = 1; bool x = a && a;` | ✗ `diag.type-mismatch` (static) | `[T-Logic]`: `bool` operands |
| `conf.neg-bool-rejected` | `bool a = true; auto x = -a;` | ✗ `diag.type-mismatch` (static) | `[T-Neg]`: signed or float |
| `conf.bitnot-bool-rejected` | `bool a = true; auto x = ~a;` | ✗ `diag.type-mismatch` (static) | `[T-Not]`: `~` on an integer |
| `conf.order-bool` | `bool a = true; bool f = false; (f < a) && !(a < a) && (a >= f) && (a <= a)` | `→ true` | `[Cmp-Bool]` (D-0108): `false < true` |
| `conf.index-string-rejected` | `String a = String::from_str("ab"); auto x = a[0];` | ✗ `diag.type-mismatch` (static) | `[T-Index]`: arrays, `Vec`s and slices |
| `conf.shift-float-rejected` | `f64 x = 1.0; u32 n = 1; auto y = x << n;` | ✗ `diag.type-mismatch` (static) | `[T-Shift]`: the shifted operand is an integer |
| `conf.shift-amount-not-u32-rejected` | `u64 k = 3; u64 v = 1; u64 w = v << k;` | ✗ `diag.type-mismatch` (static) | `[T-Shift]`: the amount is a `u32` |
| `conf.return-value-in-void-rejected` | `fn f() { return 5; }` `f();` | ✗ `diag.type-mismatch` (static) | `[T-Return]`: a void function's `return` takes no value |
| `conf.return-bare-in-value-fn-rejected` | `fn f() : i32 { return; }` `f();` | ✗ `diag.type-mismatch` (static) | `[T-Return]`: a bare `return` gives `()`, not `i32` |
| `conf.unit-equality` | `fn g() { }` `g() == g()` | `→ true` | `[Eq-Unit]`: `() =_unit ()` |
| `conf.struct-field-type-rejected` | `struct P { i32 x; i32 y; }` `P p = P { .x = true, .y = 2 };` | ✗ `diag.type-mismatch` (static) | `[T-Struct]`: a field's value has the field's type |
| `conf.struct-field-twice-rejected` | `struct P { i32 x; i32 y; }` `P p = P { .x = 1, .x = 2, .y = 3 };` | ✗ `diag.type-mismatch` (static) | a field is given once |
| `conf.variant-payload-type-rejected` | `enum E { A(i32), B }` `E e = A(true);` | ✗ `diag.type-mismatch` (static) | `[T-Enum]`: the payload has the variant's type |
| `conf.array-element-types-rejected` | `auto a = [1, true];` | ✗ `diag.type-mismatch` (static) | `[T-Array]`: one element type |
| `conf.auto-untyped-variant-rejected` | `auto o = None;` | ✗ `diag.cannot-infer-type-parameter` (static) | a binding is never polymorphic: nothing fixes `Option<T>`'s `T` |
| `conf.assign-fn-item-rejected` | `fn f() { }` `f = f;` | ✗ `diag.type-mismatch` (static) | a function is a value, not a place |
| `conf.if-without-else-value-rejected` | `bool c = true; auto x = if (c) { 1 };` | ✗ `diag.type-mismatch` (static) | `[T-If]`: no `else` makes the `if` and its branch `()` |
| `conf.private-enum-variant-qualified-rejected` | `module m { enum P { A, B } }` `auto p = m::A;` | ✗ `diag.name-not-visible` (static) | only an exported enum's variants are reachable from outside its module |
| `conf.foreach-over-map-err-propagate` | `fn mk() : Result<Vec<i32>, i32> { Vec<i32> v = Vec::new(); Vec::push(&mut v, 5); Ok(v) }` `fn f() : Result<i32, String> { i32 s = 0; foreach (x in Result::map_err(mk(), [](i32 e) : String { sprintf("%d", e) })?) { s += x; } Ok(s) }` `Result::unwrap(f())` | `→ 5` | `foreach` over `Result::map_err(…)?` iterates the `Ok` payload: the mapped `Result` keeps its type |
| `conf.generic-struct-lit-field-shape-rejected` | `struct W<T> { Vec<T> v; }` `auto w = W { .v = [1, 2] };` | ✗ `diag.type-mismatch` (static) | `[T-Struct]`: an array is no `Vec<T>`, whatever `T` is |
| `conf.tail-recursion-stack-exhausted` | `fn down(u64 n) : bool { if (n == 0) { true } else { down(n - 1) } }` `down(100000000)` | ✗ `diag.stack-exhausted` (dynamic) | `[Call-Stack-Exhausted]`: a tail call is a call; no implementation turns it into a jump |
| `conf.narrow-constant-static` | `u8 x = narrow<u8>(300);` | ✗ `diag.narrowing-overflow` (static) | `[Narrow-Overflow]`: static where constant |
| `conf.narrow-in-const-static` | `const u8 N = narrow<u8>(250 + 10);` (unused) | ✗ `diag.narrowing-overflow` (static) | `[Const-Checked-Failure]`: a constant that would fault is ill-formed, used or not |
| `conf.to-int-constant-static` | `i32 x = to_int<i32>(1e20);` | ✗ `diag.narrowing-overflow` (static) | `[Float-To-Int-Invalid]` of a literal |
| `conf.trailing-comma-call` | `fn add(i32 a, i32 b) : i32 { a + b }` `add(2, 3,)` | `→ 5` | D-0098: a trailing comma before `)` |
| `conf.trailing-comma-params` | `fn add(i32 a, i32 b,) : i32 { a + b }` `fn(i32, i32,) : i32 f = add; f(4, 5)` | `→ 9` | D-0098: in a parameter list and a `fn` type |
| `conf.trailing-comma-empty-call-rejected` | `impl/conformance/22-surface-syntax/trailing_comma_empty_call_rejected.cb` | ✗ `diag.syntax-error` (static) | a comma is never the whole list |
| `conf.result-discarded-rejected` | `fn f() : Result<i32, str> { Ok(1) }` `f();` | ✗ `diag.result-discarded` (static) | `[Stmt-Result-Discarded]` (D-0099) |
| `conf.result-discarded-void-rejected` | `fn f() : Result<void, str> { Err("no") }` `if (true) { f() } else { f() } printf("end");` | ✗ `diag.result-discarded` (static) | a block-like statement's `Result` value is discarded too |
| `conf.discard-underscore` | `fn f() : Result<i32, str> { Err("no") }` `_ = f(); 7` | `→ 7` | `[Discard]`: `_ = e;` discards on purpose |
| `conf.discard-underscore-destroys-now` | `impl/conformance/13-expression-semantics/discard_destroys_now.cb` | ok | the discarded value's destructor runs at the `_ = e;`, before the next statement |
| `conf.format-star-width` | `usize w = 5; String s = sprintf("[%-*d]", w, 42); String::eq_str(&s, "[42   ]")` | `→ true` | D-0100: `*` takes the width from a `usize` argument |
| `conf.format-star-precision` | `String s = sprintf("%.*s", 3: usize, "abcdef"); String::eq_str(&s, "abc")` | `→ true` | D-0100: `.*` takes the precision |
| `conf.format-star-not-usize-rejected` | `i32 w = 3; printf("%*d", w, 5);` | ✗ `diag.type-mismatch` (static) | a `*` takes a `usize` |
| `conf.format-enum-name` | `enum Color { Red, Green }` `Color c = Green; String s = sprintf("%v", c); String::eq_str(&s, "Green")` | `→ true` | D-0100: `%v` of an enum is its variant's name |
| `conf.float-transcendental` | `abs(ln(exp(2.0)) - 2.0) < 1e-12 && abs(sin(0.5) * sin(0.5) + cos(0.5) * cos(0.5) - 1.0) < 1e-12 && powf(2.0, 10.0) == 1024.0 && abs(atan2(1.0, 1.0) * 4.0 - 3.141592653589793) < 1e-12` | `→ true` | D-0101: the platform's, within 1 ulp |
| `conf.float-transcendental-mixed-rejected` | `f32 y = 1.0; auto z = powf(y, 2.0);` | ✗ `diag.type-mismatch` (static) | two operands of one type (the literal is `f64`) |
| `conf.ascii-helpers` | `ascii_is_digit(b'7') && !ascii_is_digit(b'x') && ascii_is_alpha(b'Q') && ascii_is_space(b'\t') && ascii_to_lower(b'A') == b'a' && ascii_to_upper(b'z') == b'Z' && ascii_to_lower(b'1') == b'1'` | `→ true` | D-0101 |
| `conf.string-to-ascii-case` | `String s = String::from_str("Ab-ç"); String l = String::to_ascii_lower(&s); String::eq_str(&l, "ab-ç")` | `→ true` | D-0101: only ASCII letters change |
| `conf.byte-literal-u32-context` | `u32 c = 124; u64 big = b'A' + 1000; c == b'|' && big == 1065` | `→ true` | D-0102: an unsigned type from context |
| `conf.byte-literal-default-u8` | `auto b = b'x'; u8 c = b; c == 120` | `→ true` | D-0102: `u8` when nothing else is expected |
| `conf.byte-literal-signed-context-rejected` | `i32 s = b'x';` | ✗ `diag.type-mismatch` (static) | D-0102: never a signed type |
| `conf.temp-field-borrow-argument` | `struct T { String name; }` `fn t() : T { T { .name = String::from_str("hello") } }` `String::len(&t().name)` | `→ 5` | D-0103: part of a temporary as an argument |
| `conf.temp-array-slice-argument` | `fn sum(slice<i32, shared> s) : i32 { s[0] + s[1] }` `sum(&[1, 2, 3, 4][1..3])` | `→ 5` | D-0103: a temporary sliced as an argument |
| `conf.temp-field-slice-argument` | `struct T { Vec<i32> v; }` `fn t() : T { Vec<i32> v = Vec::new(); Vec::push(&mut v, 7); Vec::push(&mut v, 8); T { .v = v } }` `fn sum(slice<i32, shared> s) : i32 { s[0] + s[1] }` `sum(&t().v[0..2])` | `→ 15` | D-0103 |
| `conf.temp-field-move-ends-rest` | `impl/conformance/16-aggregates/temp_field_move_ends_rest.cb` | ok | D-0103: the temporary's other fields end where the field is moved out |
| `conf.destructure-rest` | `struct S { i32 a; i32 b; i32 c; }` `S s = S { .a = 1, .b = 2, .c = 3 }; S { b, .. } = s; b` | `→ 2` | D-0104: `..` for the fields not named |
| `conf.destructure-rename` | `struct S { i32 a; i32 b; }` `S s = S { .a = 1, .b = 2 }; S { a: x, b: y } = s; x * 10 + y` | `→ 12` | D-0104: `field : name` |
| `conf.destructure-two-of-one-type` | `struct E { u64 w; u64 s; }` `E p = E { .w = 3, .s = 1 }; E q = E { .w = 4, .s = 2 }; E { w: wp, .. } = p; E { w: wq, .. } = q; wp + wq` | `→ 7` | D-0104: two values of one type in one scope |
| `conf.destructure-missing-without-rest-rejected` | `struct S { i32 a; i32 b; }` `S s = S { .a = 1, .b = 2 }; S { a } = s;` | ✗ `diag.type-mismatch` (static) | without `..`, every field is named (D-0044) |
| `conf.destructure-rest-destroys-at-block-end` | `impl/conformance/11-initialization/destructure_rest_destroys_at_block_end.cb` | ok | D-0104: the fields `..` covers end at the block's end |
| `conf.array-length-qualified-constant` | `module geom { export const usize DIM = 3; }` `array<i32, geom::DIM> a = [7; geom::DIM]; a[2] == 7` | `→ true` | D-0105 |
| `conf.array-length-qualified-private-rejected` | `impl/conformance/17-modules/array_length_qualified_private_rejected.cb` | ✗ `diag.syntax-error` (static) | D-0105: the constant must be visible |
| `conf.enum-codes` | `enum Op : u8 { Push = 1, Pop, Add = 10 }` `Op::code(Pop) == 2 && Op::code(Add) == 10` | `→ true` | D-0106: the written code, or one more than the previous |
| `conf.enum-from-code` | `enum Sign : i8 { Neg = -1, Zero, Pos }` `Option::is_none(&Sign::from_code(5)) && Sign::code(Option::unwrap(Sign::from_code(0))) == 0` | `→ true` | D-0106: `from_code` of an unused code is `None` |
| `conf.enum-code-duplicate-rejected` | `impl/conformance/16-aggregates/enum_code_duplicate_rejected.cb` | ✗ `diag.syntax-error` (static) | D-0106: distinct codes |
| `conf.enum-code-without-type-rejected` | `impl/conformance/16-aggregates/enum_code_without_type_rejected.cb` | ✗ `diag.syntax-error` (static) | D-0106: a code needs the enum's code type |
| `conf.argument-borrow-then-shared-argument` | `fn put(ref<i32, exclusive> p, i32 v) { *p = v; }` `fn read(ref<i32, shared> r) : i32 { *r }` `i32 x = 1; put(&mut x, read(&x) + 1); x` | `→ 2` | D-0107: the first argument's path is pending while the second is evaluated |
| `conf.two-exclusive-arguments-fault-at-use` | `fn two(ref<i32, exclusive> a, ref<i32, exclusive> b) { *a = 1; *b = 2; }` `i32 x = 0; two(&mut x, &mut x);` | ✗ `diag.aliasing-conflict` (dynamic) | D-0107/D-0022: bound, the two paths clash at the first write |
| `conf.foreach-range` | `usize s = 0; foreach (v in 1..5) { s += v; } s` | `→ 10` | `[Foreach-Range]`: 1 to 4; `hi` is not visited; two literal bounds are `usize` (D-0097) |
| `conf.foreach-range-literals-index` | `array<i32, 3> a = [4, 5, 6]; i32 s = 0; foreach (i in 0..3) { s += a[i]; } s` | `→ 15` | two literal bounds: `i : usize` indexes |
| `conf.foreach-range-negative-literal-typed` | `i32 s = 0; foreach (v in -3..0: i32) { s += v; } s` | `→ -6` | a typed bound fixes `T`; `-3` takes it |
| `conf.foreach-range-position` | `usize p = 9; usize w = 0; foreach (i, v in 5..8) { p = i; w = v; } p == 2 && w == 7` | `→ true` | two names: the position (`usize`, from 0) and the value |
| `conf.foreach-range-literal-takes-type` | `u8 top = 255; u32 k = 0; foreach (b in 0..top) { k += 1; } k` | `→ 255` | the literal `0` is a `u8`; the loop ends at the type's largest value without overflow |
| `conf.foreach-range-empty` | `i32 k = 0; foreach (v in 5..2) { k += 1; } k` | `→ 0` | `hi ≤ lo`: the body never runs |
| `conf.foreach-range-output` | `impl/conformance/14-control-flow/foreach_range_ok.cb` | ok | bounds once, `lo` first; `continue`, `break`; nested ranges; output pinned |
| `conf.foreach-range-negative-literals-rejected` | `foreach (v in -3..0) { }` | ✗ `diag.type-mismatch` (static) | `[Foreach-Range-Ill-Typed]`: two literal bounds are `usize`, and `-3` is not one |
| `conf.foreach-range-mixed-rejected` | `usize n = 3; i32 m = 0; foreach (v in m..n) { }` | ✗ `diag.type-mismatch` (static) | `[Foreach-Range-Ill-Typed]`: the bounds are of two types |
| `conf.foreach-range-float-rejected` | `foreach (v in 0.0..2.0) { }` | ✗ `diag.type-mismatch` (static) | `[Foreach-Range-Ill-Typed]`: the bounds are not integers |
| `conf.foreach-range-three-names-rejected` | `foreach (i, j, v in 0..3) { }` | ✗ `diag.type-mismatch` (static) | `[Foreach-Range-Ill-Typed]`: a range takes one name or two |
| `conf.foreach-range-dollar-rejected` | `impl/conformance/22-surface-syntax/foreach_range_dollar_rejected.cb` | ✗ `diag.syntax-error` (static) | `$` is written only inside `[…]`: a range has no length to name |
| `conf.range-not-a-value` | `impl/conformance/22-surface-syntax/range_not_a_value_rejected.cb` | ✗ `diag.syntax-error` (static) | a range is written only in a `foreach` |
| `conf.recursive-type-rejected` | `struct N { i32 v; Option<N> next; }` `N n = N { .v = 1, .next = None };` | ✗ `diag.recursive-type` (static) | `[Type-Recursive]`: `N` contains `Option<N>`, whose payload is `N`, by value |
| `conf.recursive-type-array-rejected` | `struct A { array<A, 2> kids; }` `i32 x = 1;` | ✗ `diag.recursive-type` (static) | `[Type-Recursive]` through an array element |
| `conf.box-recursive-type-ok` | `struct Node { i32 v; Option<Box<Node>> next; }` `Option<Box<Node>> l = None; l = Some(Box::new(Node { .v = 1, .next = l })); l = Some(Box::new(Node { .v = 2, .next = l })); match (l) { Some(b) : Box::get(&b).v == 2, None : false }` | `→ true` | `Box<Node>` holds a `rawptr<Node>`: the recursion goes through a pointer; each node owns the next |
| `conf.box-get-mut` | `Box<i32> b = Box::new(41); *Box::get_mut(&mut b) += 1; *Box::get(&b) == 42` | `→ true` | `rule.stdlib.box`: the value changed through an exclusive borrow of the `Box` |
| `conf.box-into-inner` | `Box<String> b = Box::new(String::from_str("abc")); String s = Box::into_inner(b); String::len(&s) == 3` | `→ true` | `into_inner` moves the `String` out; the `Box`'s destructor then frees only the memory |
| `conf.match-by-ref-shared` | `Option<i32> o = Some(4); i32 x = match (&o) { Some(n) : *n, None : 0 }; x == 4 && Option::unwrap_or(o, 0) == 4` | `→ true` | `[Match-By-Ref]`: `n : ref<i32, shared>`; `o` is not consumed |
| `conf.match-by-ref-exclusive` | `Option<i32> o = Some(4); match (&mut o) { Some(n) : *n += 1, None : {}, } Option::unwrap_or(o, 0) == 5` | `→ true` | `n : ref<i32, exclusive>`: the payload changed in place |
| `conf.match-by-ref-resource-payload` | `Option<String> o = Some(String::from_str("abc")); usize n = match (&o) { Some(s) : String::len(s), None : 0 }; String t = Option::unwrap_or(o, String::new()); n == 3 && String::len(&t) == 3` | `→ true` | a resource payload bound by reference, not moved: `o` still owns its `String` |
| `conf.match-by-ref-box-tree` | `enum T { Leaf(i32), Node(Box<T>) } fn depth(ref<T, shared> t) : i32 { match (t) { Leaf(v) : *v, Node(b) : 1 + depth(Box::get(b)), } }` `T t = Node(Box::new(Node(Box::new(Leaf(40))))); depth(&t) == 42` | `→ true` | a reference-typed scrutinee: each level's `Box` payload reached through the level above (D-0045, D-0046) |
| `conf.match-by-ref-write-through-shared-rejected` | `Option<i32> o = Some(1); match (&o) { Some(n) : *n = 3, None : {}, }` | ✗ `diag.write-through-shared` (static) | `n` is a shared reference |
| `conf.match-by-ref-replace-while-bound-rejected` | `Option<i32> o = Some(1); match (&o) { Some(n) : { o = None; i32 k = *n; }, None : {}, }` | ✗ `diag.aliasing-conflict` (static) | `o` is written while `n`, a reference into it, is live; FA (D-0071): the arm's binder holds `deriv(n, o, shared)` → refuted |
| `conf.match-by-ref-after-arm-ok` | `Option<i32> o = Some(1); i32 k = 0; match (&o) { Some(n) : { k = *n; }, None : {}, } o = None; k` | `→ 1` | the binder's fact ends with its arm |
| `conf.slice-array` | `auto a = [10, 20, 30, 40, 50]; auto s = &a[1..4]; slice_len(s) == 3 && s[0] == 20 && s[2] == 40` | `→ true` | `[Slice-Form]`: elements 1 up to 4 exclusive |
| `conf.slice-vec` | `Vec<i32> v = Vec::new(); Vec::push(&mut v, 1); Vec::push(&mut v, 2); Vec::push(&mut v, 3); auto s = &v[1..$]; slice_len(s) == 2 && s[1] == 3` | `→ true` | a `Vec` sliced; `$` is its length |
| `conf.slice-of-slice` | `auto a = [1, 2, 3, 4, 5]; auto s = &a[1..5]; auto t = &s[1..3]; slice_len(t) == 2 && t[0] == 3` | `→ true` | a slice of a slice views the same source, offset |
| `conf.slice-dollar` | `auto a = [1, 2, 3, 4, 5]; auto s = &a[$ - 3 .. $]; s[0] == 3 && a[$ - 1] == 5` | `→ true` | `rule.agg.dollar`: `$` in a slice and in an index |
| `conf.slice-param-any-source` | `fn sum(slice<i32, shared> s) : i32 { i32 t = 0; foreach (x in s) { t += *x; } t }` `auto a = [1, 2, 3]; Vec<i32> v = Vec::new(); Vec::push(&mut v, 10); sum(&a[0..$]) + sum(&v[0..$]) == 16` | `→ true` | one parameter type takes an array and a `Vec` |
| `conf.slice-exclusive-write` | `auto a = [1, 2, 3]; { auto s = &mut a[1..3]; s[0] = 20; s[1] += 1; } a[1] == 20 && a[2] == 4` | `→ true` | writes through an exclusive slice change the source |
| `conf.slice-foreach` | `Vec<i32> v = Vec::new(); Vec::push(&mut v, 1); Vec::push(&mut v, 2); foreach (x in &mut v[0..$]) { *x *= 10; } v[0] + v[1] == 30` | `→ true` | `rule.control.foreach` over an exclusive slice |
| `conf.vec-index` | `Vec<i32> v = Vec::new(); Vec::push(&mut v, 7); Vec::push(&mut v, 8); v[0] + v[$ - 1] == 15` | `→ true` | `[Index-Vec]`: read |
| `conf.vec-index-write` | `Vec<i32> v = Vec::new(); Vec::push(&mut v, 7); v[0] = 9; v[0] += 1; v[0] == 10` | `→ true` | `[Index-Vec]`: written, through `index_exclusive` |
| `conf.slice-bounds-rejected` | `auto a = [1, 2, 3]; auto s = &a[2..4];` | ✗ `diag.index-out-of-bounds` (static) | `[Slice-Out-Of-Bounds]`: 4 > N = 3, all literal |
| `conf.slice-bounds-dynamic` | `Vec<i32> v = Vec::new(); auto s = &v[0..1];` | ✗ `diag.index-out-of-bounds` (dynamic) | the `Vec` is empty: `hi` = 1 > 0 |
| `conf.slice-push-while-borrowed-rejected` | `Vec<i32> v = Vec::new(); Vec::push(&mut v, 1); auto s = &v[0..1]; Vec::push(&mut v, 2); i32 y = s[0];` | ✗ `diag.aliasing-conflict` (static) | `s` is used after (D-0111); the slice keeps `v` borrowed: no push while it lives; a slice binding is a borrow of its source to `rule.control.flow-analysis`, as `auto r = &v;` is |
| `conf.slice-write-through-shared-rejected` | `auto a = [1, 2]; auto s = &a[0..2]; s[0] = 5;` | ✗ `diag.write-through-shared` (static) | a shared slice is only read |
| `conf.slice-escape-rejected` | `fn f() : slice<i32, shared> { auto a = [1, 2]; &a[0..2] }` `auto s = f();` | ✗ `diag.reference-escapes-scope` (static) | a slice of a local does not outlive it (`rule.temporal.elision`) |
| `conf.vec-index-move-out-rejected` | `Vec<String> v = Vec::new(); Vec::push(&mut v, String::from_str("a")); String t = v[0];` | ✗ `diag.move-out-of-field` (static) | a resource element is borrowed in place, never moved out |
| `conf.dollar-empty-overflow` | `Vec<i32> v = Vec::new(); i32 x = v[$ - 1];` | ✗ `diag.arith-overflow` (dynamic) | `$` is a `usize`: `0 - 1` overflows before any index |
| `conf.slice-in-vec` | `auto a = [1, 2]; Vec<slice<i32, shared>> vs = Vec::new(); Vec::push(&mut vs, &a[0..1]); Vec::push(&mut vs, &a[1..2]); vs[1][0] == 2` | `→ true` | a slice is an ordinary value: stored in a `Vec` (in its buffer) and indexed through it |
| `conf.slice-in-struct` | `struct View { slice<i32, shared> s; }` `auto a = [1, 2, 3]; View w = View { .s = &a[1..$] }; w.s[0] == 2 && slice_len(w.s) == 2` | `→ true` | a slice held in a struct field |
| `conf.slice-compare-rejected` | `auto a = [1, 2]; auto s = &a[0..$]; bool e = s == &a[0..$];` | ✗ `diag.type-mismatch` (static) | no `==` on slices: equality is element by element, written by the program |
| `conf.swap-locals` | `i32 a = 1; i32 b = 2; swap(&mut a, &mut b); a == 2 && b == 1` | `→ true` | `[Swap-Places]` on two locals |
| `conf.swap-fields-through-ref` | `struct P { String l; String r; } fn flip(ref<P, exclusive> p) { swap(&mut p.l, &mut p.r); }` `P p = P { .l = String::from_str("a"), .r = String::from_str("bc") }; flip(&mut p); String::len(&p.l) == 2` | `→ true` | two resources exchanged behind a reference, where moving out is refused |
| `conf.replace-returns-old` | `String s = String::from_str("old"); String o = replace(&mut s, String::from_str("newer")); String::len(&o) == 3 && String::len(&s) == 5` | `→ true` | `replace`: the old value back, the new one in place |
| `conf.vec-swap` | `Vec<String> v = Vec::new(); Vec::push(&mut v, String::from_str("a")); Vec::push(&mut v, String::from_str("bb")); Vec::swap(&mut v, 0, 1); Vec::swap(&mut v, 1, 1); String::len(&v[0]) == 2` | `→ true` | `Vec::swap`, and nothing when `i == j` |
| `conf.slice-swap-sort-strings` | `fn sort(slice<String, exclusive> s) { for (usize i = 1; i < slice_len(s); i += 1) { usize j = i; while (j > 0 && String::len(&s[j - 1]) > String::len(&s[j])) { slice_swap(s, j - 1, j); j -= 1; } } }` `Vec<String> v = Vec::new(); Vec::push(&mut v, String::from_str("ccc")); Vec::push(&mut v, String::from_str("a")); Vec::push(&mut v, String::from_str("bb")); sort(&mut v[0..$]); String::len(&v[0]) == 1 && String::len(&v[2]) == 3` | `→ true` | a sort over a slice of resources |
| `conf.swap-disjoint-elements` | `Vec<i32> v = Vec::new(); Vec::push(&mut v, 1); Vec::push(&mut v, 2); swap(&mut v[0], &mut v[1]); v[0] == 2` | `→ true` | two different elements are disjoint places: both borrowed exclusively at once |
| `conf.swap-places-std-only` | `i32 a = 1; i32 b = 2; swap_places(&mut a, &mut b);` | ✗ `diag.unbound-name` (static) | `swap_places` is a std-only intrinsic |
| `conf.if-literal-branch-takes-sibling-type` | `i64 x = 5000000000; auto a = if (x > 1) { x } else { 0 }; auto b = if (x < 1) { 0 } else { x }; a + b == 10000000000` | `→ true` | D-0049: a literal branch takes the type of the other branch (`rule.type.expected`), before or after it |
| `conf.match-literal-arm-takes-later-type` | `Option<i64> o = None; auto a = match (o) { None : 1 << 40, Some(v) : v }; a == 1099511627776` | `→ true` | D-0049: a literal arm takes the type of the first arm that is not one, even a later one |
| `conf.exclusive-ref-arg-for-shared-param` | `fn n(ref<Vec<i32>, exclusive> v) : usize { Vec::push(v, 1); Vec::len(v) }` `Vec<i32> v = Vec::new(); n(&mut v) == 1` | `→ true` | D-0049: `ref<τ, exclusive>` passed for `ref<τ, shared>` is the shared reborrow `&*v` |
| `conf.exclusive-slice-arg-for-shared-param` | `fn total(slice<i64, shared> s) : i64 { i64 t = 0; foreach (x in s) { t += *x; } t } fn bump(slice<i64, exclusive> s) { s[0] = total(s); }` `array<i64, 3> a = [1, 2, 3]; bump(&mut a[0..$]); a[0] == 6` | `→ true` | D-0049: likewise for a slice; the reborrow ends with the call, so the write that follows is admitted |
| `conf.overwrite-none-of-resource-option` | `fn fill(ref<Option<String>, exclusive> o) { *o = Some(String::from_str("ab")); }` `Option<String> o = None; fill(&mut o); Option<String> p = None; p = Some(String::from_str("c")); match (&o) { Some(s) : String::len(s) == 2, None : false }` | `→ true` | D-0049: a `None` owns nothing, so writing over it is not `[Write-Resource-Overwrite-Rejected]` (`live-resource-at` requires `owns`) |
| `conf.overwrite-some-of-resource-option-dynamic` | `Option<Channel<u8>> o = Some(Channel::new(1)); o = Some(Channel::new(2));` | ✗ `diag.overwrite-of-live-resource` (dynamic) | a `Some(Channel)` owns its `Channel`, which is not quiet (D-0194); `Option<Channel<u8>>` has a variant owning nothing, so the static pass leaves it to the value |
| `conf.quiet-replace-ok` | `Vec<i32> v = Vec::new(); Vec::push(&mut v, 1); v = Vec::new(); String s = String::from_str("a"); s = String::from_str("bc"); Vec::len(&v) == 0 && String::len(&s) == 2` | `→ true` | `[Write-Quiet-Replace]` (D-0194): both types are quiet, so each write destroys the old value |
| `conf.overwrite-owner-type-still-static` | `struct W { Option<String> s; } fn W::drop(ref<W, exclusive> self) { }` `W w = W { .s = None }; w = W { .s = None };` | ✗ `diag.overwrite-of-live-resource` (static) | a type with a destructor always owns (its destructor is the obligation), whatever its fields hold |
| `conf.overwrite-field-of-live-object` | `impl/conformance/07-resource-authority/overwrite_field_of_live_object_rejected.cb` | ✗ `diag.overwrite-of-live-resource` (static) | `Tok`, declared `resource`, is not quiet (D-0194); `spec/14` §6 (D-0095): a field of a valid, initialized object always holds its value, and every `String` owns — refuted before the run |
| `conf.overwrite-through-exclusive-reference` | `impl/conformance/07-resource-authority/overwrite_through_exclusive_reference_rejected.cb` | ✗ `diag.overwrite-of-live-resource` (static) | `Vec<Tok>`, `Tok` declared `resource`, is not quiet (D-0194); D-0095: `t.rows = …` through a valid exclusive reference reaches a live object; refuted statically |
| `conf.overwrite-optional-field-dynamic` | `impl/conformance/07-resource-authority/overwrite_optional_field_dynamic.cb` | ✗ `diag.overwrite-of-live-resource` (dynamic) | `Option<Tok>`, `Tok` declared `resource`, is not quiet (D-0194); D-0095 refutes only where every value of the part's type owns (D-0049): an `Option<String>` field may be `None`, so the value decides — here a `Some`, at run time |
| `conf.quiet-replace` | `impl/conformance/05-value-object-semantics/quiet_replace_rejected.cb` | ✗ `diag.overwrite-of-live-resource` (dynamic); prints what its header gives first | `[Write-Quiet-Replace]`: `String` locals (also `s = sprintf("%s…", &s)`), `String`, `Vec` and `Option<String>` fields, `Vec<String>` elements, a struct element and its field, a moved-out binding reassigned, 300 replacements in a loop; then `Option<Noisy>` (a destructor: not quiet) written over its `Some` faults, the value being written destroyed first (D-0194) |
| `conf.overwrite-fault-destroys-pending` | `impl/conformance/05-value-object-semantics/overwrite_fault_destroys_pending_value_rejected.cb` | ✗ `diag.overwrite-of-live-resource` (dynamic); prints `set`, then `[drop 2][drop 1]` | `[Fault-Unwind]`: the value being written is a temporary of the faulting statement, destroyed before the binding's (D-0194) |
| `conf.quiet-not-channel` | `impl/conformance/05-value-object-semantics/quiet_not_channel_rejected.cb` | ✗ `diag.overwrite-of-live-resource` (static) | a `String` local is replaced; a `Channel` is a `std` resource that is not quiet, so `c = Channel::new(2);` over a live one is refuted (D-0194) |
| `conf.overwrite-computed-index-bounds-first` | `impl/conformance/07-resource-authority/overwrite_computed_index_bounds_first.cb` | ✗ `diag.index-out-of-bounds` (dynamic) | D-0095 refutes an element overwrite only at a literal index inside the array; at a computed index the element may not exist, and the bounds fault comes first |
| `conf.match-wildcard-keeps-scrutinee` | `Option<String> o = Some(String::from_str("abc")); match (o) { None : {}, _ : {}, } match (&o) { Some(s) : String::len(s) == 3, None : false }` | `→ true` | D-0049: an arm that moves nothing out leaves the scrutinee with its owner |
| `conf.match-move-arm-consumes` | `Option<String> o = Some(String::from_str("a")); match (o) { Some(s) : {}, None : {}, } match (&o) { Some(s) : {}, None : {}, }` | ✗ `diag.stale-binding` (static) | the `Some(s)` arm moved the payload out and consumed `o`; after the match `o` is valid on one path only, so `o` may have been moved and the use is refuted (D-0087) |
| `conf.str-byte-out-of-bounds` | `fn at(str s, usize i) : u8 { str_byte(s, i) }` `at("hi", 2)` | ✗ `diag.index-out-of-bounds` (dynamic) | `i` is a parameter → FA unknown → the `checked` guard runs; `k = 2 ≥ n = 2` → `[Str-Byte-Out-Of-Bounds]` |
| `conf.byte-literal-array` | `array<u8, 3> b = b"hi\n"; b[2]` | `→ 10` | `b"hi\n"` is `[104:u8, 105:u8, 10:u8]` (`spec/16` §3) → `[Array-Construct]`, type `array<u8,3>` matching the annotation; temp adopted; `2 : usize` from the index position; `[Index-Checked]` → 10 |
| `conf.string-from-str` | `String s = String::from_str("hi"); String::len(&s)` | `→ 2` | `[Str-Len]` → `n = 2`; loop: `str_byte(s, 0)`, `str_byte(s, 1)` (`[Str-Byte]`, in range) each pushed (`grow` 0→4 on the first: `[Allocate]`; `[Rawptr-Write]` twice); `String { .bytes = v }` → `[Store-Sub-Relocate]` (`v`'s obligation re-keyed); `inv.string.utf8-validity` holds by `inv.str.utf8-validity` — no validator ran; result temp adopted by `s`; `String::len(&s)` → `Vec::len` → 2 |
| `conf.str-in-vec` | `Vec<str> v = Vec::new(); Vec::push(&mut v, "a"); Vec::push(&mut v, "bc"); str_len(*Vec::index_shared(&v, 1))` | `→ 2` | `T := str` from the annotation (`[Generic-Call-Expected-Type]`); each `push`: `[Rawptr-Write]` of `represent(str, ·)` (`[Repr-Str]`, `[Sizeof-Str]`; `¬is-resource(str)`, so a write not a move-in); `index_shared(&v, 1)` → `[Reclaim]` establishes element 1 (plain: no obligation) → `[Borrow]` shared; `*` reads `value-at(str, …)` back → `⟪98, 99⟫`; `[Str-Len]` → 2 |
| `conf.str-not-ffi` | `unsafe extern fn put(str s);` | ✗ `diag.extern-non-ffi-type` (static) | `str ∉ FfiType` (`spec/20` §3) → `[Extern-Non-Ffi-Type]` on the declaration |
| `conf.print-observed` | `printf("hi\n")` | `ok`; the bytes `104 105 10` written | `[Printf]`: no specifiers, so `format` is the literal's bytes and the call is `print(s)` of them; `[Str-Literal]` → `⟪104, 105, 10⟫`; `[Call]` binds `s` by `[Store-Binding-Value]`; `str_ptr(s)` → `[Str-Ptr]`: `p : rawptr<u8>` with `storage(p..p+3)` holding the bytes outside every object's extent (safe: no path is affected); `str_len(s)` → 3; `write(p, 3)` occurs lexically inside `unsafe { }`, satisfying `[Unsafe-Rejected]`'s guard; `[Extern-Call]` is `disposition: trusted-unchecked` — the author's assertion that `write` reads exactly those 3 cells is discharged nowhere; `claim : isize`, `Σ.trust(claim) := unchecked-claim`, discarded at the statement's SE (plain value); `print`, and so `printf`, returns `()`; the caller's SE discards it; terminates `ok` |
| `conf.rc-last-drop-frees` | `… drop(a); drop(b);` | `count` reaches 0; `deallocate` called once | `drop(a)` → `Rc::drop`: `reclaim<RcBox<i32>>(self.ptr)` re-attaches `o'` (established by `clone`'s reclaim above); `count = 2 − 1 = 1`; `last = false`; `b`'s block ends. `drop(b)` → `Rc::drop`: reclaim re-attaches `o'` again; `count = 0`; `last = true`; the counting block's borrow ends; `drop(reclaim<RcBox<i32>>(self.ptr))`: reclaim re-attaches `o'` once more (still alive, solitary — nothing else reaches it); `¬is-resource(RcBox<i32>)` → `[Destroy-Plain]` (not `[Destroy]`: no authority to consume, no composite obligations to run — unlike `RcBox<Vec<i32>>`'s `[Destroy]` in `conf.e2e-rc-resource-payload`, which does both): `solitary(a')` holds, `end-object(o')` runs directly; `deallocate(self.ptr, sizeof<RcBox<i32>>(), alignof<RcBox<i32>>())` → `[Release]` finds no reclaimed object left (just ended) |
| `conf.datetime-from-unix` | `impl/conformance/21-standard-library-semantics/datetime_calendar_ok.cb` | ok | `[From-Unix]`, `[To-Iso]` (D-0140): the epoch and the second before it, 2000-02-29, 1900-02-28 followed by 1900-03-01, years 0 and -1, 9999-12-31T23:59:59Z |
| `conf.datetime-to-unix-limits` | `impl/conformance/21-standard-library-semantics/datetime_calendar_ok.cb` | ok | `[To-Unix]`: every second shown, `i64`'s least and greatest included, comes back from `to_unix` (D-0140) |
| `conf.datetime-weekday` | `impl/conformance/21-standard-library-semantics/datetime_calendar_ok.cb` | ok | `[Weekday]`: 1970-01-01 a Thursday, 2000-01-01 and 2026-10-03 Saturdays; the far ends of `i64` have weekdays too (D-0140) |
| `conf.datetime-iso` | `impl/conformance/21-standard-library-semantics/datetime_iso_ok.cb` | ok | `[From-Iso]`: `to_iso`'s form and a bare date read back; `Empty`; `OutOfRange` for 2023-02-29, 2024-02-30, a second 60, a month 13 or 0, an hour 24, a year beyond `i64`; a fraction of the seconds dropped (D-0181); `Invalid(i)` at a lower-case `z`, an offset, a short field, an unsigned five-digit year, a missing time, another separator (D-0140) |
| `conf.datetime-new` | `impl/conformance/21-standard-library-semantics/datetime_new_sort_ok.cb` | ok | `DateTime::new` gives `None` for a 29 February in a common year, a 31 April, an hour 24, a minute 60, a day 0 (D-0140) |
| `conf.datetime-sort` | `impl/conformance/21-standard-library-semantics/datetime_new_sort_ok.cb` | ok | `DateTime::less` sorts a `Vec<DateTime>` by `Vec::sort_by`, earliest first; `DateTime::eq` (D-0140) |
| `conf.unix-ms` | `impl/conformance/21-standard-library-semantics/datetime_new_sort_ok.cb` | ok | `[Unix-Ms]`: `unix_ms() / 1000` within a second of `unix_seconds()` (D-0140) |
| `conf.local-offset-range` | `impl/conformance/21-standard-library-semantics/datetime_local_ok.cb` | ok | `[Local-Offset]`: `local_offset_seconds(unix_seconds())` is within ±18 hours, whole minutes, and the same asked twice; the value itself is the machine's and not printed (D-0147) |
| `conf.to-local-consistent` | `impl/conformance/21-standard-library-semantics/datetime_local_ok.cb` | ok | `[To-Local]`: `DateTime::to_local(&t)` equals `from_unix(to_unix(&t) + local_offset_seconds(to_unix(&t)))`; a second later is a second later; moments of years -10000 and 9999 convert (D-0147) |
| `conf.datetime-invalid` | `impl/conformance/21-standard-library-semantics/datetime_invalid_faults.cb` | ✗ `diag.invalid-datetime` (dynamic) | `[Invalid-DateTime]`: `to_unix` of a struct literal with month 13 (D-0140) |
| `conf.path-join` | `impl/conformance/21-standard-library-semantics/path_parts_ok.cb` | ok | `[Path-Join]` (D-0141): `logs` + `app.log`; a base ending in a separator; an absolute tail replaces the base (Unix) |
| `conf.path-parts` | `impl/conformance/21-standard-library-semantics/path_parts_ok.cb` | ok | `[Path-Parts]`: parent, file name, stem, extension and absoluteness of `/a/b/c.txt`, `a/../b`, `./x`, `/`, `""`, `..`, `.bashrc` (no extension), `archive.tar.gz` (`gz`), `a//b/` (Unix) |
| `conf.path-normalize` | `impl/conformance/21-standard-library-semantics/path_parts_ok.cb` | ok | `[Path-Normalize]`: `.` removed, `x/..` resolved, a leading `..` kept, `/..` is `/`, nothing left is `.` — by the text alone (Unix) |
| `conf.file-info` | `impl/conformance/21-standard-library-semantics/file_info_dirs_ok.cb` | ok | `[File-Info]` (D-0141): a file's kind, length, modification time within a minute of now and read-only flag; a directory's kind; `NotFound` |
| `conf.copy-file` | `impl/conformance/21-standard-library-semantics/file_info_dirs_ok.cb` | ok | `[Copy-File]`: the count copied, and `read_file` of the copy gives the same text |
| `conf.dir-all` | `impl/conformance/21-standard-library-semantics/file_info_dirs_ok.cb` | ok | `[Dir-All]`: `make_dir_all` of three levels, `Ok` again when they exist; `remove_dir_all` of the tree, then `NotFound` |
| `conf.set-current-dir` | `impl/conformance/21-standard-library-semantics/file_info_dirs_ok.cb` | ok | `[Set-Current-Dir]`: `current_dir` gives the new directory, and a relative path resolves against it |
| `conf.path-canonical` | `impl/conformance/21-standard-library-semantics/file_info_dirs_ok.cb` | ok | `[Path-Canonical]`: absolute for a file that exists, `NotFound` for one that does not |
| `conf.temp-home-dir` | `impl/conformance/21-standard-library-semantics/file_info_dirs_ok.cb` | ok | `[Temp-Dir]`: the temporary directory's path is not empty |
| `conf.os-random-u64` | `impl/conformance/21-standard-library-semantics/os_random_ok.cb` | ok | `[Os-Random]` (D-0142): two secure `u64`s differ (a false failure has probability 2⁻⁶⁴) |
| `conf.os-random-bytes` | `impl/conformance/21-standard-library-semantics/os_random_ok.cb` | ok | `[Os-Random]`: 32 secure bytes are not all zero |
| `conf.os-random-empty` | `impl/conformance/21-standard-library-semantics/os_random_ok.cb` | ok | `[Os-Random]`: an empty slice is filled with nothing, without a fault |
| `conf.rng-from-os` | `impl/conformance/21-standard-library-semantics/os_random_ok.cb` | ok | `[Rng-From-Os]`: an `Rng` seeded from the system draws as any `Rng` does |
| `conf.process-output` | `impl/conformance/21-standard-library-semantics/process_output_ok.cb` | ok | `[Command]`, `[Output]` (D-0143): `/bin/echo` with an argument holding a space (no shell: one argument); `sh -c` writing to both streams and exiting 3 (Unix) |
| `conf.process-input` | `impl/conformance/21-standard-library-semantics/process_output_ok.cb` | ok | `[Output]`: `output_with_input` feeds `/bin/cat` (Unix) |
| `conf.process-status` | `impl/conformance/21-standard-library-semantics/process_output_ok.cb` | ok | `[Status]`: the exit status of a run with this program's streams (Unix) |
| `conf.process-signal-status` | `impl/conformance/21-standard-library-semantics/process_output_ok.cb` | ok | `[Status]`: a child ended by SIGKILL has status -9 (Unix) |
| `conf.process-not-found` | `impl/conformance/21-standard-library-semantics/process_output_ok.cb` | ok | `[Command]`: a program that is not there is `Err(NotFound)`, from `output` and from `status` (Unix) |
| `conf.process-dir-env` | `impl/conformance/21-standard-library-semantics/process_output_ok.cb` | ok | `[Command]`: `current_dir` (`pwd` in `/`); `clear_env` and `env` (only `X` is set) (Unix) |
| `conf.process-spawn-lines` | `impl/conformance/21-standard-library-semantics/process_child_ok.cb` | ok | `[Spawn-Child]`, `[Child-Streams]`: `write_input`, `close_input`, `read_output_line` (`\r\n` and a last line without an end), `read_output`, `read_error`, `wait` (Unix) |
| `conf.process-kill` | `impl/conformance/21-standard-library-semantics/process_child_ok.cb` | ok | `[Kill]`, `[Wait]`: `kill` of `sleep 100`, then `wait` gives -9, and again -9 (Unix) |
| `conf.process-try-wait` | `impl/conformance/21-standard-library-semantics/process_child_ok.cb` | ok | `[Wait]`: `try_wait` is `None` while the child runs and its status after (Unix) |
| `conf.process-closed-pipe` | `impl/conformance/21-standard-library-semantics/process_closed_pipe_ok.cb` | ok | `[Child-Streams]`, `[Output]`: `write_input` to a child that has ended is `Err(Io)`; `output_with_input` to a program that exits without reading gives its status; the program goes on (Unix) |
| `conf.exit-runs-destructors` | `impl/conformance/21-standard-library-semantics/process_exit_ok.cb` | `ok, exit status 3` | `[Exit]`, `[Terminate-Exit]` (D-0150): `exit(3)` three calls deep in a `match` arm unwinds the thread's frames innermost first (both `Lock` destructors print, inner before outer), nothing after it runs, status 3 |
| `conf.exit-from-thread` | `impl/conformance/21-standard-library-semantics/process_exit_thread_ok.cb` | `ok, exit status 4` | `[Exit]` (D-0150): `exit(4)` in a spawned thread ends the program with status 4; the worker's frames unwind, `main`'s (waiting in `join`) do not |
| `conf.sha256-vectors` | `impl/conformance/21-standard-library-semantics/crypto_sha256_ok.cb` | ok | `[Sha256]` (D-0151): FIPS 180-4's vectors (the empty message, `abc`, the 56- and 112-byte messages), the padding boundaries 55, 56, 63, 64 and 65 bytes, and a thousand `a`, each shown with `to_hex` |
| `conf.sha256-streaming` | `impl/conformance/21-standard-library-semantics/crypto_sha256_ok.cb` | ok | `[Sha256]`: `Sha256::update` in three pieces gives the one-shot digest; a copied `Sha256` continues independently |
| `conf.hmac-sha256-vectors` | `impl/conformance/21-standard-library-semantics/crypto_hmac_ok.cb` | ok | `[Hmac-Sha256]` (D-0151): RFC 4231's cases 1, 2, 3, 4, 6 and 7 (a 131-byte key is hashed first); the streaming form agrees with the one-shot form |
| `conf.digest-eq` | `impl/conformance/21-standard-library-semantics/crypto_hmac_ok.cb` | ok | `[Digest-Eq]` (D-0151): true for equal digests and for two empty slices, false for one bit's difference and for a prefix |
| `conf.hex` | `impl/conformance/21-standard-library-semantics/text_hex_base64_ok.cb` | ok | `[Hex]` (D-0151): `to_hex` of all 256 byte values round-trips through `from_hex`; either case is read; a non-digit is `Invalid(i)`, an odd count `Invalid(len)`, "" `Empty` |
| `conf.base64` | `impl/conformance/21-standard-library-semantics/text_hex_base64_ok.cb` | ok | `[Base64]` (D-0151): RFC 4648's vectors for "" to "foobar"; all 256 byte values round-trip; a bad character, a misplaced `=` and a length not a multiple of four are `Invalid` |
| `conf.hkdf-vectors` | `impl/conformance/21-standard-library-semantics/crypto_hkdf_ok.cb` | ok | `[Hkdf]` (D-0152): RFC 5869's A.1, A.2 (82 bytes, more than two blocks) and A.3 (empty salt and info): `hkdf_extract`'s key and `hkdf_expand`'s output; `hkdf_sha256` agrees |
| `conf.hkdf-bound` | `impl/conformance/21-standard-library-semantics/crypto_hkdf_ok.cb` | ok | `[Hkdf]`: 8160 bytes come out whole, 0 bytes come out empty, and the long output begins with the shorter one |
| `conf.hkdf-too-long` | `impl/conformance/21-standard-library-semantics/crypto_hkdf_too_long_faults.cb` | ✗ `diag.hkdf-length` (dynamic) | `[Hkdf]`: 8161 bytes asked of one pseudorandom key (D-0152) |
| `conf.chacha20-vector` | `impl/conformance/21-standard-library-semantics/crypto_chacha20_poly1305_ok.cb` | ok | `[ChaCha20]` (D-0153): RFC 8439 §2.4.2, the 114-byte message at counter 1 |
| `conf.poly1305-vector` | `impl/conformance/21-standard-library-semantics/crypto_chacha20_poly1305_ok.cb` | ok | `[Poly1305]` (D-0153): RFC 8439 §2.5.2 |
| `conf.aead-vector` | `impl/conformance/21-standard-library-semantics/crypto_chacha20_poly1305_ok.cb` | ok | `[Aead-Seal]`, `[Aead-Open]` (D-0153): RFC 8439 §2.8.2's ciphertext and tag; it opens to the plaintext |
| `conf.aead-tamper` | `impl/conformance/21-standard-library-semantics/crypto_chacha20_poly1305_ok.cb` | ok | `[Aead-Open]`: one bit changed in the ciphertext, the tag or the associated data, or a message shorter than a tag, gives `None`; restored, it opens |
| `conf.crypto-length` | `impl/conformance/21-standard-library-semantics/crypto_length_faults.cb` | ✗ `diag.crypto-length` (dynamic) | `[Crypto-Length]` (D-0153): a 31-byte ChaCha20-Poly1305 key |
| `conf.x25519-vectors` | `impl/conformance/21-standard-library-semantics/crypto_x25519_ok.cb` | ok | `[X25519]` (D-0154): RFC 7748 §5.2's two vectors and the iterated vector after one round |
| `conf.x25519-exchange` | `impl/conformance/21-standard-library-semantics/crypto_x25519_ok.cb` | ok | `[X25519]`: RFC 7748 §6.1's public keys and shared secret from both sides; two fresh `x25519_private_key`s agree on a secret |
| `conf.x25519-small-order` | `impl/conformance/21-standard-library-semantics/crypto_x25519_ok.cb` | ok | `[X25519]`: the all-zero public key gives `None` |
| `conf.aes-vectors` | `impl/conformance/21-standard-library-semantics/crypto_aes_gcm_ok.cb` | ok | `[Aes]` (D-0165): FIPS 197 Appendix C's block under 16-, 24- and 32-byte keys |
| `conf.aes-gcm-vectors` | `impl/conformance/21-standard-library-semantics/crypto_aes_gcm_ok.cb` | ok | `[Aes-Gcm-Seal]`, `[Aes-Gcm-Open]` (D-0165): the GCM specification's test cases 1, 2, 4 and 16, each opened again; a changed ciphertext, tag, aad or nonce and a 15-byte message are refused |
| `conf.p256-ecdh-vectors` | `impl/conformance/21-standard-library-semantics/crypto_p256_ecdh_ok.cb` | ok | `[P256-Ecdh]` (D-0165): RFC 5903 §8.1's public key g^i and shared x-coordinate of g^ir |
| `conf.p256-ecdh-refusals` | `impl/conformance/21-standard-library-semantics/crypto_p256_ecdh_ok.cb` | ok | `[P256-Ecdh]`: private keys 0 and n, and peer points off the curve, compressed or 64 bytes long, give `None` |
| `conf.aes-key-length` | `impl/conformance/21-standard-library-semantics/crypto_aes_key_length_faults.cb` | ✗ `diag.crypto-length` (dynamic) | `[Crypto-Length]` (D-0165): a 20-byte AES key |
| `conf.hmac-generic` | `impl/conformance/21-standard-library-semantics/crypto_hmac_generic_ok.cb` | ok | `[Hasher]`, `[Hmac]` (D-0166): RFC 4231's case 2 under SHA-384 and SHA-512; the SHA-256 form agrees with `hmac_sha256`; a streamed `Hmac` and a streamed `Hasher` agree with the one-shot forms |
| `conf.hkdf-generic` | `impl/conformance/21-standard-library-semantics/crypto_hmac_generic_ok.cb` | ok | `[Hkdf-With]` (D-0166): derivations over SHA-384 and SHA-512 (values confirmed by Python's hmac module), RFC 5869 A.1 over SHA-256 as `hkdf_sha256` gives it, extract then expand, and `hkdf_expand_label_with` as `hkdf_expand_label` |
| `conf.ed25519-vectors` | `impl/conformance/21-standard-library-semantics/crypto_ed25519_ok.cb` | ok | `[Ed25519-Sign]`, `[Ed25519-Verify]` (D-0167): RFC 8032 §7.1's tests 1 to 3, public keys and signatures, each verified |
| `conf.ed25519-refusals` | `impl/conformance/21-standard-library-semantics/crypto_ed25519_ok.cb` | ok | `[Ed25519-Verify]`: a changed R, a changed S, another message, another key, S = L and a 63-byte signature are false |
| `conf.ecdsa-sign-vectors` | `impl/conformance/21-standard-library-semantics/crypto_ecdsa_sign_ok.cb` | ok | `[Ec-Keys]`, `[Ecdsa-Sign]` (D-0168): RFC 6979 A.2.5's P-256 public key and signature of SHA-256("sample"), verified, the same twice; RFC 5903 §8.1's exchange through `ecdh` |
| `conf.ec-key-refusals` | `impl/conformance/21-standard-library-semantics/crypto_ecdsa_sign_ok.cb` | ok | `[Ec-Keys]`, `[Ecdh]`: private keys 0 and n and a peer point off the curve give `None`; a fresh key has the curve's lengths |
| `conf.ecdsa-sign-p384` | `impl/conformance/21-standard-library-semantics/crypto_ecdsa_sign_p384_ok.cb` | ok | `[Ecdsa-Sign]` (D-0168): RFC 6979 A.2.6's P-384 signature of SHA-384("sample"), verified |
| `conf.ecdh-p384` | `impl/conformance/21-standard-library-semantics/crypto_ecdsa_sign_p384_ok.cb` | ok | `[Ecdh]` (D-0168): RFC 5903 §8.2's shared x-coordinate on P-384 |
| `conf.ec-key-length` | `impl/conformance/21-standard-library-semantics/crypto_ec_key_length_faults.cb` | ✗ `diag.crypto-length` (dynamic) | `[Crypto-Length]` (D-0168): a 32-byte private key on P-384 |
| `conf.rsa-sign-vectors` | `impl/conformance/21-standard-library-semantics/crypto_rsa_sign_ok.cb` | ok | `[Rsa-Private-Key]`, `[Rsa-Private-Op]`, `[Rsa-Sign-Pkcs1v15]`, `[Rsa-Sign-Pss]` (D-0169): a 1024-bit openssl key's PKCS #1 v1.5 signature of SHA-256("hello") is the one the Python `cryptography` library computes and verifies; two PSS signatures differ and both verify; `public_key` gives n and e |
| `conf.rsa-private-key-refusals` | `impl/conformance/21-standard-library-semantics/crypto_rsa_sign_ok.cb` | ok | `[Rsa-Private-Key]`: swapped or wrong parts make no key |
| `conf.rsa-generate-size` | `impl/conformance/21-standard-library-semantics/crypto_rsa_generate_size_faults.cb` | ✗ `diag.crypto-length` (dynamic) | `[Rsa-Generate]` (D-0169): 1000 bits (generation itself is exercised by the stress harness: minutes under an interpreter) |
| `conf.bigint-mod-inverse` | `impl/conformance/21-standard-library-semantics/bigint_mod_inverse_ok.cb` | ok | `[Big-Mod-Inverse]` (D-0169): 3⁻¹ mod 11, no inverse of 6 mod 9 or mod 1, a 200-bit inverse multiplied back, `clone` |
| `conf.blake2b-vectors` | `impl/conformance/21-standard-library-semantics/crypto_kdf_ok.cb` | ok | `[Blake2b]` (D-0170): RFC 7693 Appendix A ("abc"), the empty message, a 20-byte digest, the official keyed test (key 00..3f), and a streamed digest |
| `conf.pbkdf2-vectors` | `impl/conformance/21-standard-library-semantics/crypto_kdf_ok.cb` | ok | `[Pbkdf2]` (D-0170): RFC 7914 §11's first PBKDF2-HMAC-SHA-256 vector |
| `conf.argon2id-vectors` | `impl/conformance/21-standard-library-semantics/crypto_kdf_ok.cb` | ok | `[Argon2id]` (D-0170): two small-parameter tags as the reference implementation (argon2-cffi) computes them, one with four lanes |
| `conf.password-hash` | `impl/conformance/21-standard-library-semantics/crypto_kdf_ok.cb` | ok | `[Password-Hash]` (D-0170): a PHC string from the reference implementation verifies with its password, not with another, and an Argon2i or malformed string is false |
| `conf.kdf-parameters` | `impl/conformance/21-standard-library-semantics/crypto_kdf_parameters_faults.cb` | ✗ `diag.kdf-parameters` (dynamic) | `[Kdf-Parameters]` (D-0170): a 7-byte salt |
| `conf.key-formats` | `impl/conformance/21-standard-library-semantics/x509_keys_ok.cb` | ok | `[Key-Der]`, `[Key-Pem]` (D-0171): an RSA, a SEC 1 P-256 and an Ed25519 key openssl wrote give the public keys openssl wrote, and survive DER and PEM round trips as PKCS #8; garbage is `BadEncoding` |
| `conf.sign-verify` | `impl/conformance/21-standard-library-semantics/x509_keys_ok.cb` | ok | `[Sign]` (D-0171): each key signs under its scheme and `verify_signature` accepts it, rejects another message, and `sign` refuses `UnsupportedSignature` |
| `conf.x509-ed25519` | `impl/conformance/21-standard-library-semantics/x509_ed25519_chain_ok.cb` | ok | `[Cert-Parse]`, `[Verify-Chain]` (D-0171): an Ed25519 leaf signed by an Ed25519 root parses with that key and scheme and verifies for its name, not another |
| `conf.url-parse` | `impl/conformance/21-standard-library-semantics/http_url_ok.cb` | ok | `[Url-Parse]` (D-0173): scheme and host folded, default and explicit ports, an empty path, a query, a dropped fragment, a bracketed IPv6 host; no scheme, no host, user information and a bad port refused |
| `conf.percent-encoding` | `impl/conformance/21-standard-library-semantics/http_url_ok.cb` | ok | `[Percent]` (D-0173): every byte class encoded and decoded; a `%` without two hex digits is None |
| `conf.http-headers` | `impl/conformance/21-standard-library-semantics/http_url_ok.cb` | ok | `[Header-Find]` (D-0173): a header found whatever its case, the first of two, none; `Request::new`'s fields; `reason_phrase` |
| `conf.http-get-post` | `impl/conformance/21-standard-library-semantics/http_loopback_ok.cb` | ok | `[Http-Send]`, `[Http-Body]`, `[Http-Request]`, `[Http-Respond]` (D-0173, D-0174): GET and POST to a loopback `HttpServer` with headers and bodies both ways, a 404 with its body, a 204 without one |
| `conf.http-redirects` | `impl/conformance/21-standard-library-semantics/http_loopback_ok.cb` | ok | `[Http-Redirect]` (D-0173): 302 and a relative 303 followed by GET without the body, 307 with the method and body |
| `conf.http-server-keep-alive` | `impl/conformance/21-standard-library-semantics/http_loopback_ok.cb` | ok | `[Http-Request]`, `[Http-Respond]` (D-0174): two requests on one connection answered in order, keep-alive then close as the client asked |
| `conf.http-server-no-body` | `impl/conformance/21-standard-library-semantics/http_loopback_ok.cb` | ok | `[Http-Respond]` (D-0174): a HEAD answered with the length and no body |
| `conf.http-server-refusals` | `impl/conformance/21-standard-library-semantics/http_loopback_ok.cb` | ok | `[Http-Request]` (D-0174): a body over the server's limit is TooLarge, answered 413; a request line that is not HTTP is BadMessage, answered 400 |
| `conf.http-chunked` | `impl/conformance/21-standard-library-semantics/http_client_bodies_ok.cb` | ok | `[Http-Body]` (D-0173): a hand-written chunked response with a chunk extension and a trailer decoded; an interim 100 Continue read past |
| `conf.http-until-close` | `impl/conformance/21-standard-library-semantics/http_client_bodies_ok.cb` | ok | `[Http-Body]` (D-0173): an HTTP/1.0 response without a length read to the close |
| `conf.http-client-refusals` | `impl/conformance/21-standard-library-semantics/http_client_bodies_ok.cb` | ok | `[Http-Body]`, `[Http-Redirect]`, `[Http-Error]` (D-0173): both lengths, a body over `max_body`, a redirect loop past `max_redirects`, an ftp URL and a bad URL each refused with their error |
| `conf.scram-sha256-vectors` | `impl/conformance/21-standard-library-semantics/postgres_scram_vectors_ok.cb` | ok | `[Pg-Auth]` (D-0180): RFC 7677 §3's SCRAM-SHA-256 exchange (its nonces and salt; 4 iterations, the interpreter's sake) through `Scram`: the client-first and client-final messages, the server's final message verified; a wrong signature, an `e=` answer and a nonce that does not continue the client's refused; the user escaped; `scram_salted_password` |
| `conf.postgres-url` | `impl/conformance/21-standard-library-semantics/postgres_url_ok.cb` | ok | `[Pg-Url]` (D-0180): a full URL, the defaults (5432, the user's database, verify-full), a bracketed IPv6 host, percent-encoded user, password and database, `postgresql://`, sslmode=disable; refused: another scheme, no user, no host, a bad port, `sslmode=require`, a misspelt key, an unknown key, a bad percent-encoding |
| `conf.postgres-values` | `impl/conformance/21-standard-library-semantics/postgres_url_ok.cb` | ok | `[Pg-Query]` (D-0180): the text of every `PgValue`: NULL, text, an integer, floats (a tenth, the infinities, NaN), the bools, bytea's hex form, empty bytes |
| `conf.postgres-loopback` | `impl/conformance/21-standard-library-semantics/postgres_loopback_ok.cb` | ok | `[Pg-Connect]`, `[Pg-Auth]`, `[Pg-Query]`, `[Pg-Result]`, `[Pg-Execute]`, `[Pg-Script]`, `[Pg-Notice]`, `[Pg-Error]` (D-0180): a mock backend on a loopback port in a thread: an SSLRequest answered N (TlsRefused), a wrong password (AuthFailed), a cleartext request (AuthRefused), SASL without SCRAM-SHA-256 and MD5 (UnsupportedAuth); a session: the startup parameters, parameters of every kind echoed with their columns, an INSERT's count and tag, NULL, non-ASCII and bytea cells, `PgResult::column` and `PgRow::get` past the end, a server error with its SQLSTATE, position and hint and the connection usable after it, ResultTooLarge likewise, a notice kept, a script and a failing one, Disconnected after `close`; a session the server breaks with a message the protocol does not allow (ProtocolViolation, then Disconnected) |
| `conf.text-strip-prefix-suffix` | `impl/conformance/21-standard-library-semantics/text_prefix_lines_words_ok.cb` | ok | `[Strip-Prefix]` (D-0181): `strip_prefix`/`strip_suffix` give the rest of the text or `None`, on a `StringView` and through the `String::` twins |
| `conf.text-lines-words` | `impl/conformance/21-standard-library-semantics/text_prefix_lines_words_ok.cb` | ok | `[Lines]`, `[Split-Whitespace]` (D-0181): a `\r` before each `\n` dropped, no empty line after a final `\n`, an empty text no lines; words between runs of ASCII whitespace, none empty |
| `conf.text-code-points` | `impl/conformance/21-standard-library-semantics/text_prefix_lines_words_ok.cb` | ok | `[Code-Point]` (D-0181): the code point of one-, two-, three- and four-byte characters; `None` for empty text; `push_code_point` of each width, and U+FFFD for a surrogate and for a value above U+10FFFF |
| `conf.parse-bool` | `impl/conformance/21-standard-library-semantics/text_prefix_lines_words_ok.cb` | ok | `[Parse-Bool]` (D-0181): `true` and `false` read, `tru`, `True` and `fals` refused with the longest prefix as the offset |
| `conf.vec-retain` | `impl/conformance/21-standard-library-semantics/vec_retain_dedup_sort_floats_ok.cb` | ok | `[Retain]` (D-0181): the elements kept in order, the rest destroyed after every `keep` call, last first; resources and `String`s |
| `conf.vec-dedup` | `impl/conformance/21-standard-library-semantics/vec_retain_dedup_sort_floats_ok.cb` | ok | `[Dedup]` (D-0181): adjacent duplicates removed, the first of each run kept; a one-element and an empty `Vec` unchanged |
| `conf.vec-sort-floats` | `impl/conformance/21-standard-library-semantics/vec_retain_dedup_sort_floats_ok.cb` | ok | `[Sort-Float]` (D-0181): `f64` sorted with `-inf` first, `-0.0` and `0.0` as equal in their order, every NaN last; `f32` sorted and searched; a `PriorityQueue<f64>` gives the least first and a NaN last |
| `conf.math-clamp` | `impl/conformance/21-standard-library-semantics/math_clamp_nan_gcd_trig_ok.cb` | ok | `[Clamp]` (D-0181): below, above and within the bounds, on integers, floats and `str`; equal bounds |
| `conf.math-nan-finite` | `impl/conformance/21-standard-library-semantics/math_clamp_nan_gcd_trig_ok.cb` | ok | `[Is-Nan]`, `[Is-Finite]` (D-0181): a NaN, the infinities, finite floats of both widths, and integers (never NaN, always finite) |
| `conf.math-gcd` | `impl/conformance/21-standard-library-semantics/math_clamp_nan_gcd_trig_ok.cb` | ok | `[Gcd]` (D-0181): signed and unsigned, a negative argument, zero, `gcd(0, 0)`, `u128` |
| `conf.float-inverse-trig` | `impl/conformance/21-standard-library-semantics/math_clamp_nan_gcd_trig_ok.cb` | ok | `[Float-Transcendental]` (D-0181): `asin`, `acos`, `atan` at 1, 0.5 and infinity in `f64` and `f32`; `asin(2.0)` a NaN |
| `conf.rng-shuffle` | `impl/conformance/21-standard-library-semantics/random_shuffle_uuid_ok.cb` | ok | `[Rng-Shuffle]` (D-0181): seed 42 over 0..10 gives one fixed order (every element once), `String`s shuffled without copying, a one-element and an empty slice |
| `conf.uuid-v4` | `impl/conformance/21-standard-library-semantics/random_shuffle_uuid_ok.cb` | ok | `[Uuid-V4]` (D-0181): 36 characters with hyphens at 8, 13, 18, 23, version `4`, variant `8`/`9`/`a`/`b`; two draws differ |
| `conf.time-iso-ms` | `impl/conformance/21-standard-library-semantics/time_ms_http_date_ok.cb` | ok | `[Iso-Ms]`, `[From-Iso]` (D-0181): `iso_from_unix_ms` of positive, negative and zero moments; `unix_ms_from_iso` of a one-digit and a four-digit fraction and of a date alone; `from_iso` drops a fraction, refuses an empty one and a missing `Z`; a moment beyond `i64` milliseconds is `OutOfRange` |
| `conf.time-http-date` | `impl/conformance/21-standard-library-semantics/time_ms_http_date_ok.cb` | ok | `[Http-Date]` (D-0181): `to_http_date` of two moments; `from_http_date` of the IMF-fixdate, RFC 850 (a two-digit year on each side of 70) and asctime forms, surrounding whitespace; empty, a zone other than GMT, a 31 February and a misspelt month refused |
| `conf.file-create-new` | `impl/conformance/21-standard-library-semantics/file_create_new_flush_position_ok.cb` | ok | `[Create-New]` (D-0181): a file made; `Ok(None)` for the same name again and for a directory; `Err` under a missing directory |
| `conf.file-flush-position` | `impl/conformance/21-standard-library-semantics/file_create_new_flush_position_ok.cb` | ok | `[Flush]`, `[Position]` (D-0181): the position after a write, a seek, a read line and a read; `flush` succeeds on a writable file and is `Err` on one open for reading; the bytes written are read back |
| `conf.env-current-exe` | `impl/conformance/21-standard-library-semantics/env_exe_hostname_process_id_ok.cb` | ok | `[Current-Exe]` (D-0181): an absolute path to a file |
| `conf.env-hostname` | `impl/conformance/21-standard-library-semantics/env_exe_hostname_process_id_ok.cb` | ok | `[Hostname]` (D-0181): non-empty text without a space |
| `conf.process-id` | `impl/conformance/21-standard-library-semantics/env_exe_hostname_process_id_ok.cb` | ok | `[Process-Id]` (D-0181): positive |
| `conf.child-terminate` | `impl/conformance/21-standard-library-semantics/child_terminate_error_line_ok.cb` | ok | `[Child-Terminate]` (D-0181, Unix): a sleeping child ends with status -15; `terminate` again is `Ok` |
| `conf.child-read-error-line` | `impl/conformance/21-standard-library-semantics/child_terminate_error_line_ok.cb` | ok | `[Child-Streams]` (D-0181): standard error read a line at a time, a CRLF line, `read_error` taking what was read ahead, standard output still its own, `None` at the end |
| `conf.channel-try-recv` | `impl/conformance/21-standard-library-semantics/channel_try_recv_timeout_cpu_ok.cb` | ok | `[Try-Recv]` (D-0185): `Ok(None)` on an empty channel, a value when one waits, `Err(())` once closed and drained |
| `conf.channel-recv-timeout` | `impl/conformance/21-standard-library-semantics/channel_try_recv_timeout_cpu_ok.cb` | ok | `[Recv-Timeout]` (D-0185): `Ok(None)` after the time on an empty channel, a value at once when one waits, every value from a producer thread and then `Err(())`; a zero wait |
| `conf.cpu-count` | `impl/conformance/21-standard-library-semantics/channel_try_recv_timeout_cpu_ok.cb` | ok | `[Cpu-Count]` (D-0185): at least 1 |
| `conf.tcp-try-clone` | `impl/conformance/21-standard-library-semantics/net_try_clone_keepalive_udp_connect_ok.cb` | ok | `[Try-Clone]` (D-0185): a clone with the same peer, read in a spawned thread while the original is written from the main one; a clone dropped leaves the connection open |
| `conf.tcp-keepalive` | `impl/conformance/21-standard-library-semantics/net_try_clone_keepalive_udp_connect_ok.cb` | ok | `[Keepalive]` (D-0181): set on and off on a connected stream |
| `conf.udp-connect` | `impl/conformance/21-standard-library-semantics/net_try_clone_keepalive_udp_connect_ok.cb` | ok | `[Udp-Connect]` (D-0181): two connected sockets `send`/`recv` each way, a datagram from a third socket dropped, `recv` with a small `max` truncating, `send` on an unconnected socket refused |
| `conf.sha1` | `impl/conformance/21-standard-library-semantics/sha1_vectors_ok.cb` | ok | `[Sha1]` (D-0181): FIPS 180-4's "abc", the empty message, the 56-byte message and a million `a`s in 1000-byte updates; `Hasher`/`hash` over `Sha1` |
| `conf.hmac-sha1` | `impl/conformance/21-standard-library-semantics/sha1_vectors_ok.cb` | ok | `[Hmac]`, `[Hash]` (D-0181): HMAC-SHA-1 of the fox, RFC 2202's cases 1 and 6 (a key longer than the block); PBKDF2-HMAC-SHA-1 of RFC 6070 (2 iterations); RFC 6238's SHA-1 TOTP at time step 1 (94287082) |
| `conf.json-parse` | `impl/conformance/21-standard-library-semantics/json_ok.cb` | ok | `[Json-Parse]` (D-0182): a document with every kind of value, every escape, a surrogate pair and a lone surrogate, whole and fractional numbers, `-0`, empty containers, surrounding whitespace |
| `conf.json-text` | `impl/conformance/21-standard-library-semantics/json_ok.cb` | ok | `[Json-Text]`, `[Json-Pretty]` (D-0182): the compact text with its escapes and whole numbers without a fraction, read back equal; the indented text; a NaN written `null`, 2^53 + 1 written as a float, an empty array `[]` |
| `conf.json-tree` | `impl/conformance/21-standard-library-semantics/json_ok.cb` | ok | `[Json-Get]`, `[Json-Set]` (D-0182): `get`, `at`, `is_null` and the `as_*` readers on each kind (`None` across kinds, `as_i64` of a fraction and of 10^21); `set` replacing and appending, `push` |
| `conf.json-errors` | `impl/conformance/21-standard-library-semantics/json_ok.cb` | ok | `[Json-Error]` (D-0182): empty and blank text; nineteen malformed documents, each `Invalid` at its first bad byte, `1e999` `OutOfRange`; 512 levels accepted and 513 `OutOfRange` |
| `conf.deflate-inflate` | `impl/conformance/21-standard-library-semantics/compress_ok.cb` | ok | `[Deflate]`, `[Inflate]` (D-0183): a raw stream CPython's zlib made, inflated; text, empty data (`0300`), three bytes, random bytes and long runs round-tripped, the compressed bytes identical in every implementation |
| `conf.zlib` | `impl/conformance/21-standard-library-semantics/compress_ok.cb` | ok | `[Zlib]` (D-0183): CPython's zlib stream inflated; a round trip with the `78 9c` header and the Adler-32 trailer |
| `conf.gzip` | `impl/conformance/21-standard-library-semantics/compress_ok.cb` | ok | `[Gzip]` (D-0183): CPython's gzip member with a name inflated; a round trip with the fixed header and the CRC-32 and length trailer; an empty member |
| `conf.adler32` | `impl/conformance/21-standard-library-semantics/compress_ok.cb` | ok | `[Adler32]` (D-0183): the empty slice is 1, `Wikipedia` is 300286872, 20 000 bytes reduced in blocks |
| `conf.inflate-errors` | `impl/conformance/21-standard-library-semantics/compress_ok.cb` | ok | `[Compress-Error]` (D-0183): `Oversize` at a small `max`, `Truncated` for a stream cut short and for three bytes to `zlib_inflate`, `Corrupt` at the trailer for a wrong Adler-32 and at byte 0 for a wrong gzip header; `CompressError::text` |
| `conf.args-flags-options` | `impl/conformance/21-standard-library-semantics/args_ok.cb` | ok | `[Args-Flag]`, `[Args-Option]` (D-0184): `-v`, `--output=out.txt`, `-I inc1` and `--include inc2` read, a flag given nowhere false, the arguments after `--` kept whole (`--help` among them) |
| `conf.args-finish` | `impl/conformance/21-standard-library-semantics/args_ok.cb` | ok | `[Args-Finish]` (D-0184): the positional arguments in order; an undeclared `--bogus` refused by name, `-n` without its value refused, a lone `-` and a second `--` kept |
| `conf.args-help` | `impl/conformance/21-standard-library-semantics/args_ok.cb` | ok | `[Args-Help]` (D-0184): the usage line and the aligned option lines, a short form absent, a value name shown; a program with no options |
| `conf.postgres-typed-cells` | `impl/conformance/21-standard-library-semantics/postgres_typed_cells_ok.cb` | ok | `[Pg-Cell]` (D-0181): `get_i64`, `get_f64` (`NaN`, `Infinity`, `-Infinity`), `get_bool`, `get_bytes` on the server's text forms; `None` for NULL, past the end, and text of another form |
| `conf.tls-server` | `impl/conformance/21-standard-library-semantics/tls_server_loopback_ok.cb` | ok | `[Tls-Server]` (D-0186): a loopback TLS 1.3 server with a self-signed Ed25519 certificate for `localhost`, the client trusting it: lines both ways; a client for another name refused (`HostMismatch`) and the server going on; a peer that is not TLS answered with an alert; the server counting its failed handshakes |
| `conf.tls-read-line` | `impl/conformance/21-standard-library-semantics/tls_server_loopback_ok.cb` | ok | `[Tls-Read-Line]` (D-0181): a CRLF and an LF line, `None` after close_notify |
| `conf.tls-peer-certificate` | `impl/conformance/21-standard-library-semantics/tls_server_loopback_ok.cb` | ok | `[Tls-Peer-Certificate]` (D-0181): the server's certificate seen by the client, `None` on the server |
| `conf.https-server` | `impl/conformance/21-standard-library-semantics/https_loopback_ok.cb` | ok | `[Https-Bind]`, `[Http-Accept]` (D-0186): a loopback https server with a self-signed P-256 certificate, fetched by `HttpClient` with that certificate as its only root, a POST answered with JSON over TLS |
| `conf.http-forms` | `impl/conformance/21-standard-library-semantics/https_loopback_ok.cb` | ok | `[Form]` (D-0181): `form_encode` of a name with spaces and `&` and a value with `%`; `query_pairs` of a URL with `%20`; `form_decode` of the body; `find_param` |
| `conf.http-content-encoding` | `impl/conformance/21-standard-library-semantics/https_loopback_ok.cb` | ok | `[Content-Encoding]` (D-0183): a server answering gzip and deflate bodies, each decoded with the encoding header dropped; an unknown encoding (`br`) left as it came with its header |
| `conf.sha512-vectors` | `impl/conformance/21-standard-library-semantics/crypto_sha512_ok.cb` | ok | `[Sha512]` (D-0156): SHA-512 of the empty message, `abc` and the 112-byte message (FIPS 180-4); the streaming form agrees with the one-shot form; `hash` gives 32, 48 and 64 bytes |
| `conf.sha384-vectors` | `impl/conformance/21-standard-library-semantics/crypto_sha512_ok.cb` | ok | `[Sha512]`: SHA-384 of the same three messages; streaming agrees |
| `conf.bigint-arithmetic` | `impl/conformance/21-standard-library-semantics/bigint_ok.cb` | ok | `[BigUint]` (D-0157): sum, difference, product, quotient and remainder, a modular power, shifts and comparisons of 200-bit and 90-bit numbers, as Python computes them |
| `conf.bigint-text` | `impl/conformance/21-standard-library-semantics/bigint_ok.cb` | ok | `[BigUint]`: decimal text both ways, big-endian bytes both ways and padded; `parse` refuses a non-digit with `Invalid` |
| `conf.bigint-sub-underflow` | `impl/conformance/21-standard-library-semantics/bigint_sub_faults.cb` | ✗ `diag.arith-overflow` (dynamic) | `[Big-Division]`: 3 − 4 as `BigUint` |
| `conf.rsa-pkcs1v15` | `impl/conformance/21-standard-library-semantics/crypto_rsa_ok.cb` | ok | `[Rsa-Pkcs1v15]` (D-0158): a 2048-bit key's SHA-256 signature verifies; with one byte changed it does not |
| `conf.rsa-pss` | `impl/conformance/21-standard-library-semantics/crypto_rsa_ok.cb` | ok | `[Rsa-Pss]` (D-0158): a 2048-bit SHA-256 signature verifies; under another digest it does not |
| `conf.rsa-pss-3072` | `impl/conformance/21-standard-library-semantics/crypto_rsa_3072_ok.cb` | ok | `[Rsa-Pss]`: a 3072-bit key's SHA-512 signature verifies |
| `conf.ecdsa-p256` | `impl/conformance/21-standard-library-semantics/crypto_ecdsa_p256_ok.cb` | ok | `[Ecdsa]` (D-0159): a P-256 signature verifies; with s changed, with a key off the curve, or truncated, it does not |
| `conf.ecdsa-p384` | `impl/conformance/21-standard-library-semantics/crypto_ecdsa_p384_ok.cb` | ok | `[Ecdsa]`: on P-384, a signature verifies; with a key off the curve, or truncated, it does not |
| `conf.ecdsa-p384-tamper` | `impl/conformance/21-standard-library-semantics/crypto_ecdsa_p384_tamper_ok.cb` | ok | `[Ecdsa]`: on P-384, the signature with s changed does not verify (a case of its own so each does one full verification) |
| `conf.hkdf-expand-label` | `impl/conformance/21-standard-library-semantics/crypto_hkdf_expand_label_ok.cb` | ok | `[Hkdf-Expand-Label]` (D-0161): RFC 8448's early and derived secrets; key, IV and key-update labels against an independent HKDF |
| `conf.x509-parse` | `impl/conformance/21-standard-library-semantics/x509_chain_ok.cb` | ok | `[Cert-Parse]` (D-0160): a test PKI's leaf (names, addresses), intermediate (CA, path length 0) and root parsed |
| `conf.x509-chain` | `impl/conformance/21-standard-library-semantics/x509_chain_ok.cb` | ok | `[Verify-Chain]`, `[Host-Match]`: the leaf chains through the RSA-PSS-signing intermediate to the P-384 root at 2030-06-01; names match by DNS name, case, final dot, one-label wildcard and IP address |
| `conf.x509-refusals` | `impl/conformance/21-standard-library-semantics/x509_refusals_ok.cb` | ok | `[Verify-Chain]`, `[Der-Read]`: another host, an expired leaf, a time before validity, no intermediate and no roots are each refused with their reason; a truncated certificate is `BadEncoding`; a non-minimal DER length is refused |
| `conf.tls-not-tls` | `impl/conformance/21-standard-library-semantics/tls_refusals_ok.cb` | ok | `[Tls-Error]` (D-0161): a loopback peer answering in plain text is `ProtocolError` |
| `conf.tls-closed` | `impl/conformance/21-standard-library-semantics/tls_refusals_ok.cb` | ok | `[Tls-Error]`: a peer that closes at once is `NetFailure(Closed)` |
| `conf.interrupt-flag` | `impl/conformance/21-standard-library-semantics/interrupt_flag_ok.cb` | ok | `[Interrupt-Flag]`: false before and after `watch_interrupts` while no interrupt has come |
| `conf.net-bind-port` | `impl/conformance/21-standard-library-semantics/net_tcp_ok.cb` | ok | `[Bind]` (D-0144): a loopback listener on port 0 is given a free port, which `local_addr` names |
| `conf.net-echo` | `impl/conformance/21-standard-library-semantics/net_tcp_ok.cb` | ok | `[Accept]`, `[Connect]`, `[Stream-Read]`, `[Stream-Write]`: a server thread reads a line and writes it back in capitals with `TcpStream::printf`; `read_line` drops the `\r\n`; each blocking call lets the other thread run |
| `conf.net-read-exact` | `impl/conformance/21-standard-library-semantics/net_tcp_ok.cb` | ok | `[Stream-Read]`: `read_exact` of the four bytes sent; past the end, `Err(Closed)` |
| `conf.net-shutdown-read-zero` | `impl/conformance/21-standard-library-semantics/net_tcp_ok.cb` | ok | `[Shutdown]`: after the peer's `shutdown_write`, `read` gives 0 |
| `conf.net-addr-in-use` | `impl/conformance/21-standard-library-semantics/net_tcp_ok.cb` | ok | `[Net-Error]`: a second `bind` of a bound address is `AddrInUse` |
| `conf.net-accept-timeout` | `impl/conformance/21-standard-library-semantics/net_tcp_ok.cb` | ok | `[Stream-Timeout]`: `accept` with a 100 ms timeout and no client is `Err(TimedOut)` |
| `conf.net-refused` | `impl/conformance/21-standard-library-semantics/net_tcp_ok.cb` | ok | `[Net-Error]`: `connect` to a port whose listener was dropped is `Err(Refused)` |
| `conf.net-addr-parse` | `impl/conformance/21-standard-library-semantics/net_addr_ok.cb` | ok | `[Addr]` (D-0144): `SocketAddr::parse` and `text` round-trip IPv4 and bracketed IPv6, IPv6 written shortest |
| `conf.net-addr-reject` | `impl/conformance/21-standard-library-semantics/net_addr_ok.cb` | ok | `[Addr]`: `1.2.3:80`, `x:80`, a missing port or colon, an IPv4 in brackets, an IPv6 without them, a non-digit in the port are `Invalid(i)`; port 70000 `OutOfRange`; `""` `Empty` |
| `conf.net-addr-constants` | `impl/conformance/21-standard-library-semantics/net_addr_ok.cb` | ok | `[Addr]`: `localhost_v4`, `unspecified_v4`, `localhost_v6`, `unspecified_v6` as text; `IpAddr::parse` refuses an octet 256 |
| `conf.net-udp` | `impl/conformance/21-standard-library-semantics/net_udp_resolve_ok.cb` | ok | `[Udp]` (D-0144): a datagram between two loopback sockets arrives whole, with the sender's port; a read timeout with nothing is `TimedOut` |
| `conf.net-udp-truncate` | `impl/conformance/21-standard-library-semantics/net_udp_truncate_ok.cb` | ok | `[Udp]` (D-0144): `recv_from(u, max)` of a datagram longer than `max` gives exactly its first `max` bytes and the sender; the rest is lost, and the next datagram follows whole |
| `conf.net-resolve-localhost` | `impl/conformance/21-standard-library-semantics/net_udp_resolve_ok.cb` | ok | `[Resolve]`: `localhost` resolves without a network, each address with the port asked for |
| `conf.read-all` | `impl/conformance/21-standard-library-semantics/read_all_ok.cb` | ok | `[Read-All]` (D-0145): after a `read_line`, `read_all` gives the 11 bytes left (a `\r\n` and a last line without an end, unchanged); again at the end, an empty `String` |
| `conf.read-all-bytes` | `impl/conformance/21-standard-library-semantics/read_all_bytes_ok.cb` | ok | `[Read-All]`: `read_all_bytes` gives every byte, `00 ff` included |
| `conf.read-all-not-utf8` | `impl/conformance/21-standard-library-semantics/read_all_not_utf8_ok.cb` | ok | `[Read-All]`: input that is not UTF-8 is `Err(Utf8(e))` from `read_all`, `e.offset` 1 |
| `conf.std-variant-names-unique` | `impl/conformance/21-standard-library-semantics/std_variant_names_unique_ok.cb` | ok | `[Resolve-Unqualified]`: no two of `std`'s enums share a variant name, so `FileError`'s `NotFound`, `Denied`, `Io` and `NetError`'s `UnknownHost`, `Refused`, `TimedOut`, `Failed` are unambiguous unqualified as values; running a missing program is `FileError::NotFound` (D-0143, D-0144) |

## 12. Concurrency (`spec/19`)

| id | Fragment | Outcome | Derivation |
|---|---|---|---|
| `conf.spawn-join-value` | `fn work(i32 n) : i32 { n * 2 }` `auto h = spawn(work, 21); join(h)` | `→ 42` | `[Spawn]` temp handle adopted (`[Repr-Handle]`); `join(h)`: `h` in place position; `[Join]` waits, marks the result `taken`, `[Destroy]` the handle (`[Handle-Destructor]` discards nothing), returns `r_b` |
| `conf.unjoined-handle-waits` | `{ auto h = spawn(work, 1); }` | `ok`; thread finished before BE completes | `[Handle-Destructor]` |
| `conf.spawn-borrow-closure-rejected` | `i32 x = 1; auto h = spawn([x]() { x + 1 });` | ✗ `diag.spawn-borrow-closure` (static) | `rule.conc.spawn` |
| `conf.spawn-slice-arguments` | `impl/conformance/19-concurrency/spawn_slice_arguments_ok.cb` | ok | four threads a round, each given a shared reference to one `Vec` and an exclusive slice of a disjoint range of another; the borrows travel to the thread and stay valid until its call returns, however soon the spawning statement ends |
| `conf.temporaries-across-thread-switches` | `impl/conformance/19-concurrency/temporaries_across_thread_switches_ok.cb` | ok | each thread's statement temporaries are its own: a thread switched out between the statements of a call made mid-statement ends only its own (CHG-0089) |
| `conf.cross-thread-write-conflict` | `fn w(ref<i32, shared> r) : i32 { *r }` `i32 x = 1; auto h = spawn(w, &x); x = 2; join(h)` | ✗ `diag.aliasing-conflict` (static) | `x = 2`: the spawned thread's parameter object holds the borrow (`a'` valid, shared, not an ancestor) until it finishes; dynamically the outcome would depend on whether the thread has finished; FA (D-0071): `h` holds `deriv(h, x, shared)` until `join(h)` → refuted |
| `conf.spawn-exclusive-read-rejected` | `fn w(ref<i32, exclusive> r) : i32 { *r = 5; 0 }` `i32 x = 1; auto h = spawn(w, &mut x); i32 y = x; join(h)` | ✗ `diag.aliasing-conflict` (static) | reading `x` while the thread holds it exclusively; FA (D-0071) `deriv(h, x, exclusive) = T` |
| `conf.spawn-borrow-after-join-ok` | `fn w(ref<i32, shared> r) : i32 { *r }` `i32 x = 1; auto h = spawn(w, &x); i32 y = x; i32 z = join(h); x = 2; x + y + z` | `→ 4` | reads while the thread holds a shared borrow are permitted; `join(h)` ends the handle's facts (D-0071), so the write after it is not refuted |
| `conf.spawn-join-in-branch-dynamic` | `fn w(ref<i32, shared> r) : i32 { *r }` `i32 x = 1; bool b = true; auto h = spawn(w, &x); if (b) { join(h); } x = 2; x` | `→ 2` | joined on one branch only: the handle's fact is `?` after the merge, so the write is left to the dynamic check, which finds the thread finished |
| `conf.spawn-ref-result` | `impl/conformance/19-concurrency/spawn_ref_result_ok.cb` | ok | `[T-Spawn]` with `τr` a reference (D-0094): a shared element reference read after the join, and an exclusive one — the function returns its own reference parameter — written through after it |
| `conf.spawn-ref-result-source-written-rejected` | `impl/conformance/19-concurrency/spawn_ref_result_source_written_rejected.cb` | ✗ `diag.aliasing-conflict` (static) | the handle of a reference-returning thread holds the argument's borrow (D-0071, D-0094): writing the source before the join is refuted |
| `conf.mutex-lock-unlock` | `auto m = Mutex::new(0); { auto g = lock(&m); *g = *g + 1; } *lock(&m)` | `→ 1` | `[Lock]` path `base := a_m'`; `*g`: `g` in place position → `[Guard-Deref]`; write: `a_m`, `a_m'` ancestors; BE → `[Destroy]` of the guard → `[Guard-Drop]` (built-in `destructor`, `spec/07` §1); second lock succeeds; `*lock(&m)` derefs the temporary guard, which ends at SE |
| `conf.mutex-reentrant-rejected` | `auto m = Mutex::new(0); auto g = lock(&m); auto g2 = lock(&m);` | ✗ `diag.mutex-reentrant-lock` (dynamic) | `[Lock-Reentrant]` |
| `conf.thread-vec-of-resources-dropped` | `fn consume(Vec<Vec<i32>> vv) : usize { Vec::len(&vv) }` `Vec<Vec<i32>> vv = Vec::new(); Vec<i32> inner = Vec::new(); Vec::push(&mut inner, 1); Vec::push(&mut vv, inner); auto h = spawn(consume, vv); join(h)` | `→ 1` | `Vec::push(&mut vv, inner)`: `[Rawptr-Move-In]` makes element 0 a top-level reclaimed object, its authority re-keyed from `inner` — main's. `spawn`: `[Authority-Transfer]` moves `vv`'s own authority to `ℓ'` (not the element's: nothing in `Σ` links them). In `ℓ'`, `vv`'s BE → `Vec::drop` → `drop(reclaim<Vec<i32>>(…))`: `[Reclaim]` re-attaches element 0 and takes its authority for `ℓ'` (`CHG-0030`), so `[Destroy]`'s `authority(ℓ', destroy, o)` holds; `inner`'s buffer deallocated, then `vv`'s |
| `conf.thread-vec-of-resources-returned` | `fn make() : Vec<Vec<i32>> { Vec<Vec<i32>> vv = Vec::new(); Vec::push(&mut vv, Vec::new()); vv }` `auto h = spawn(make); auto vv = join(h); Vec::len(&vv)` | `→ 1` | the element is established in `ℓ'`; `[Join]` re-keys `vv`'s top-level authority to main (`CHG-0015`), not the element's; main's BE → `Vec::drop` → `[Reclaim]` re-attaches the element and takes its authority (`CHG-0030`) → `[Destroy]` holds |
| `conf.mutex-shared-across-threads` | `fn inc(ref<mutex<i32>, shared> m) { auto g = lock(m); *g = *g + 1; }` `auto m = Mutex::new(0); auto h = spawn(inc, &m); { auto g = lock(&m); *g = *g + 1; } join(h); *lock(&m)` | `→ 2` | both threads hold shared references to `o_m`; each lock path's writes are `sync-exempt` from the other thread's shared reference; `[Lock]` serializes |
| `conf.mutex-in-shared-struct` | `impl/conformance/19-concurrency/mutex_in_shared_struct_ok.cb` | ok | a struct with two `mutex` fields shared by reference among three threads; each lock path's writes (a field, a `Vec` element, a `push`) are `sync-exempt` from the other threads' shared references to the struct (CHG-0072); the two mutexes lock independently |
| `conf.lock-through-exclusive-ref` | `impl/conformance/19-concurrency/lock_through_exclusive_ref_ok.cb` | ok | `[Lock]` through `&a.balance` formed from an exclusive reference (a parameter, a destructor's `self`): the lock path's `base` chain includes it, so it is an ancestor, not a competitor |
| `conf.guard-interior-borrow-conflict` | `impl/conformance/19-concurrency/guard_interior_borrow_conflict_rejected.cb` | ✗ `diag.aliasing-conflict` (static) | a shared borrow of `(*g).v` and a write through `g` are both lock-derived: not `sync-exempt`, so `clash` as any two paths; FA (D-0071): `deriv(r, *g.v, shared)` overlaps `&mut (*g).v` → refuted |
| `conf.guard-auto-deref` | `impl/conformance/19-concurrency/guard_auto_deref_ok.cb` | ok | `[Guard-Auto-Deref]`: `g.f`, `g.v[i]`, `g.a[i]`, `&mut g.v`, `&g.v` through a guarded struct; `l[i]`, `p[i]` through a guarded `Vec` and array; `g.head + 1` typed `usize` |
| `conf.guard-auto-deref-stale` | `impl/conformance/19-concurrency/guard_auto_deref_stale_rejected.cb` | ✗ `diag.stale-binding` (static) | `g.n` after `drop(g)`: the guard `[Guard-Auto-Deref]` reaches through is gone |
| `conf.channel-close` | `impl/conformance/19-concurrency/channel_send_recv_close_ok.cb` | ok | `[Send]`, `[Recv]` oldest first across the ring's wrap; `[Close]` twice; `send` after it is `Err(v)`; `recv` drains, then `None` |
| `conf.channel-destroy` | `impl/conformance/19-concurrency/channel_send_recv_close_ok.cb` | ok | `[Channel-Destroy]`: the values left are destroyed oldest first, at the block's end |
| `conf.channel-threads` | `impl/conformance/19-concurrency/channel_worker_pool_ok.cb` | ok | three workers `recv` jobs until `None` after `close`; two producers and a consumer through a channel of capacity 1; every value received once |
| `conf.channel-fifo-bounded` | `impl/conformance/19-concurrency/channel_bounded_fifo_ok.cb` | ok | a producer waits on a full channel of capacity 1 (`[Send]`'s blocking case) 200 times; the values arrive in the order sent |
| `conf.channel-zero-capacity` | `impl/conformance/19-concurrency/channel_zero_capacity_rejected.cb` | ✗ `diag.channel-zero-capacity` (dynamic) | `[Channel-Zero-Capacity]` |
| `conf.channel-deadlock` | `impl/conformance/19-concurrency/channel_recv_alone_deadlock_rejected.cb`, `impl/conformance/19-concurrency/channel_send_full_deadlock_rejected.cb` | ✗ `diag.channel-deadlock` (dynamic) | `[Channel-Deadlock]`: the main thread waits on an empty channel, and on a full one after its only other thread has ended |
| `conf.channel-deadlock-late` | `impl/conformance/19-concurrency/channel_deadlock_after_last_thread_ends_rejected.cb` | ✗ `diag.channel-deadlock` (dynamic) | `[Channel-Deadlock]` holds from the moment the last spawned thread ends while the main thread waits, not only when the wait begins |
| `conf.spawn-argument-parameter-type` | `impl/conformance/19-concurrency/spawn_argument_takes_parameter_type_ok.cb` | ok | `[Spawn]` binds arguments as a call does (`rule.fn.bind-param`): a literal argument takes the parameter's type, for a `fn` and for a `move` closure |
| `conf.temporary-guard-released` | `impl/conformance/19-concurrency/temporary_guard_released_at_statement_end_ok.cb` | ok | a temporary guard (`*lock(&m)`) is destroyed at its statement's end (`[Guard-Drop]`): the mutex can be locked again, and a `Vec` of mutexes read so is destroyed normally |
| `conf.join-result-matched` | `impl/conformance/19-concurrency/join_result_matched_ok.cb` | ok | `[Join]`'s result has the thread's result type: `match (join(h))` reads a `Result`'s payload, `join(p).b` a struct's field |
| `conf.fault-with-live-thread` | `impl/conformance/19-concurrency/fault_with_blocked_thread_faults.cb` | ✗ `diag.index-out-of-bounds` (dynamic) | `[Fault-Unwind]`, `[Handle-Destructor]`: a thread asleep for a minute when main faults; the program ends at once reporting the fault, the handle's destructor not waiting (spec/18 1.5.3, spec/19 1.8.1) |

## 13. End-to-end programs (`spec/examples.md` `ex.e2e-*`)

Whole programs derived from `[Program]` to `[Terminate-Ok]`. The
fragment column names the example holding the program.

| id | Fragment | Outcome | Derivation |
|---|---|---|---|
| `conf.e2e-threads-mutex-total` | `ex.e2e-threads-mutex` | `total = 7`, `a + b = 7`; `ok` | `Mutex::new(0)`: `[Mutex-New]` (`[Store-Sub-Value]` into `inner` per `[Sizeof-Mutex]`), temp adopted by `m`. Each `spawn`: `[T-Spawn]` via `callable`; `&m` → `[Borrow]` shared (`¬clash(a_m, shared)`: the other thread's borrow is shared); `[Spawn]` establishes the handle (`[Repr-Handle]`), binds `m`/`n` in `ℓ'` by `[Store-Binding-Value]` with `current-scope(ℓ') = ⊥` (`spec/04`), the borrow now held by the parameter object. In `worker`: `lock(m)` reads the parameter's token; `[Lock]` blocks while the other thread holds the lock, else forms `a_g` (base: the borrow) and a guard temp adopted by `g`; `*g = *g + 1`: `g` in place position → `[Guard-Deref]`; `[Read]`/`[Write]` through `a_g`: `a_m` and the borrow are ancestors, the other thread's shared borrow is `sync-exempt`; body BE → `[Destroy]` of the guard → built-in `[Guard-Drop]` (`spec/07` §1). Trailing `n` → `[Store-Result-Place-Copy]`; `[Thread-Body-Done]`. `join(h1)`: `h1` in place position; `[Join]` waits, `taken`, `[Destroy]` → `[Handle-Destructor]` discards nothing; value 3 stored. `*lock(&m)` → temp guard → `[Guard-Deref]` (temp case) → read 7; SE destroys the guard. Main BE: plain objects end; `[Destroy]` of `m` (solitary: every borrow died with its holder) → composite of `inner` (none). FA: every `*g` root is `*r` → dynamic. `[Terminate-Ok]` |
| `conf.e2e-vec-nested-realloc` | `ex.e2e-vec-nested-realloc` | `n = 1`, `total = 5`; `ok`; six `Vec::drop`s, three `deallocate`s | Loop body per iteration: `inner` adopted; `Vec::push(&mut inner, k)` (`grow` at 0→4: `[Allocate]`, `copy_raw` of 0 bytes; `[Rawptr-Write]`); `Vec::push(&mut outer, inner)`: `inner` in place position → `[Store-Binding-Place-Transfer]` into `x` (solitary modulo `a_x`: the earlier `&mut inner` died with its callee's parameter object; FA `escaped(inner)` → dynamic); in `push`, `*p = x` → `[Rawptr-Move-In]`: reclaimed `o'` with `inner`'s obligation and authority re-keyed, `o_inner` ended, so the body's BE has nothing to sweep (`holder` absent). Iteration 5 (`len = 4 = cap`): `grow` → `[Allocate]` 8 slots, `copy_raw` of 4 elements (bytes only: `[Repr-Struct]` of `Vec<i32>` holds no `ref` datum), `[Deallocate]` → `[Release]` ends the four reclaimed elements (trusted: their obligations are re-established below); then `[Rawptr-Move-In]` of the fifth. Inner block: `index_shared` → `[Reclaim]` on element 0: no reclaimed object exists after the release, so a fresh one with fresh authority/obligation (trusted: not tracked elsewhere) — `inv.resource-authority`'s reclaimed-owner clause; `[Borrow]` shared; `first` holds it; `Vec::len(first)` → 1; BE ends `o_first` → the borrow is Dead; FA `deriv(first, ·) := F`. `match (Vec::pop(&mut outer))`: `pop` → `[Rawptr-Move-Out]` of element 4 (reclaimed object from its `[Rawptr-Move-In]`) → temp inside `Some` (`[Store-Sub-Relocate]`); `[Match]` on the temp: `[Temp-Root]`, `[Relocate-Out]` into `v` in `f_arm`, consume `[Destroy]` of the `Option` temp; arm body `Vec::push(&mut outer, v)`: `¬clash(a_outer, exclusive)`: the `pop` borrow died with `pop`'s parameter object (its statement is still open, but `held-by` became `∅` at `[Object-End]`, so it is Dead); `v` transferred in; `[Block-Exit]` of `f_arm` sweeps nothing. `Vec::len(&outer)` → 5. Main BE: `[Destroy]` of `outer` → `Vec::drop`: `drop(reclaim<T>(…))` five times — elements 0–3 re-attached fresh (post-release), element 4 re-attached to its Move-In object — each `[Destroy]` → inner `Vec::drop` → `[Deallocate]` of the inner buffer; then `[Deallocate]` of `outer`'s buffer. `[Terminate-Ok]` |
| `conf.e2e-rc-resource-payload` | `ex.e2e-rc-resource-payload` | `n = 1`, `m = 1`; `ok`; inner `Vec::drop` once, box deallocated once | `Rc::new(payload)`: `T := Vec<i32>`; `payload` transferred into `v`; `RcBox { .count = 1, .value = v }` → `[Store-Sub-Relocate]` (obligation `(o_rb, value)`; `RcBox<Vec<i32>>` is a resource by derivation, so `[Object-Establish-Resource]` granted `o_rb` authority); `*bp = temp` → `[Rawptr-Move-In]`: reclaimed `o'` with obligations `{o', (o', value)}` and authorities re-keyed. `Rc::clone(&a)`: `[Reclaim]` re-attaches `o'`; `&mut` from its root; `count` write: `¬live-resource-at` (a `usize` sub-range); `o_b` ends with the `unsafe` block → its borrow Dead. `Rc::get(&b)`: `&reclaim<…>(r.ptr).value` → `[Reclaim]`, projection, shared `[Borrow]` (`¬clash`: no live path on `o'`); one reference parameter → elision; `Vec::len` reads through it → 1; the borrow dies with `len`'s parameter object. `drop(a)`: `[Destroy]` (solitary: clone's borrow of `a` died with its parameter object) → `Rc::drop` → `last = false`. Second `Rc::get` → 1. `drop(b)` → `Rc::drop`: count 0, `last = true`; first `unsafe` block ends (`b`'s borrow Dead); `drop(reclaim<RcBox<T>>(self.ptr))` → `[Destroy]` on `o'`: authority re-keyed at Move-In, solitary ✓, `destructor(RcBox<…>) = None`, `[Destroy-Composite]` on `(o', value)` → `[Run-Destructor]` `Vec::drop` through a temporary exclusive root into the sub-range → inner `[Deallocate]`; `[Object-End]` of `o'` (reclaimed: storage kept); `[Deallocate]` of the box → `[Release]` finds no reclaimed object. Main BE: `payload` was transferred (not owned); `a`, `b` already ended. `[Terminate-Ok]` |
| `conf.e2e-propagate-chain` | `ex.e2e-propagate-chain` | `total = 7`, `bad = -1`; `ok` | `parse`: `9 : u8` from `b`; `{ return Err(1); }` is `never` (`[T-Block]`), `[If-No-Else]` gives `unit`; `Err(1)`: `1 : i32` from the payload type; `widen<i32>(b)`: `[Widen]` (u8 ⊆ i32). `sum`: `Vec::len(bytes)` reads the parameter's token; `parse(…)?` → `[Propagate]` → `[Match]` on the temporary `Result` (`[Temp-Root]`, `f_arm` pushed, payload copied): arm `Ok(v) : v` → `block'(f_arm, …)` → value; arm `Err(err) : return Err(err)` → `[Return]`: `unwind-to(f_b)` folds SE of the `return`'s scope, BE of `f_arm`, SE of the `i32 d = …;` scope, BE of the loop-body frame, SE of the `while` statement's scope, BE of `f_b` — each pair matched by `scope-frame`; `[Call]` then exits the parameter frame. `acc = acc + d` etc. In `main`: `sum(&v)` → `escaped(v)` (not elided: returns a `Result`), so the later `&mut v` is checked dynamically and passes (the borrow died with `sum`'s parameter object); `match` on the temporary: `Ok(t) : t` copies; `-1 : i32` via the first arm's type propagated through unary `-`. First call: `parse(3)`, `parse(4)` → `Ok`, total 7. Third push; second call: `parse(12)` → `Err(1)` → `?` returns `Err(1)` from `sum` → arm `Err(_) : -1`. `[Terminate-Ok]` |
| `conf.e2e-vec-realloc-stale-ref` | `ex.e2e-vec-realloc-stale-ref` | ✗ `diag.stale-binding` (dynamic); no other output | `main`: `Vec::push(&mut v, 10)` grows `0→4` (`[Allocate]`, `[Rawptr-Write]`), `len = 1`; `hold_across_grow(&mut v)`: `[Borrow]` exclusive, no other path on `o_v` yet, admitted (static: proven). Inside: `&*v` — `[Ref-Deref-Place]` on `v`'s value gives place `a_v'`; `[Borrow]` shared from it (`a_v` is an ancestor, no clash); `rule.temporal.ref-escape`'s `referent-block` is `unknown` for a place rooted at `*`-of-a-parameter, and `rule.control.flow-analysis`'s `deriv`/`escaped` facts are defined only over a plain binding's `x.π` projections, so nothing here is one of its tracked shapes either — the check is `discharge: dynamic`, and dynamically no other path targets `o_v`, so it passes. `Vec::index_shared(&*v, 0)`: `[Reclaim]` establishes element 0 as `o_0` (`¬is-resource(i32)`: no authority/obligation granted); `[Borrow]` shared over it; the returned reference is stored in `r` (`[Store-Binding-Value]`). Each `Vec::push(v, …)`: `v` read (a `ref<Vec<i32>,exclusive>` value, `¬clash(a_vparam, shared)` trivially — `v`'s own slot has no other path) and passed on; `rule.fn.bind-param`'s `ref<τ,m>` row: "no new borrow" — no `[Borrow]`/clash check ever names `o_v` at these call sites. Inside `push`, `v.len`/`v.cap`/`*(v.ptr+…)` read/write through `*v` against `o_v`; `o_0`'s `of` differs from `o_v`, so `clash` (same-`of` only) could never witness `r` regardless of discharge. `push(v,20)`,`push(v,30)`,`push(v,40)`: `len` `1→2→3→4`, `cap` stays 4, each a plain `[Rawptr-Write]` of a fresh element. `push(v,50)`: `len = cap = 4` → `grow`: `[Allocate]` 8 slots; `copy_raw` of the 4 live elements; `cap>0` → `[Deallocate]` → `[Release]` on the old buffer — `extent(o_0,Σ) ⊆ [old_p, old_p+4·sizeof(i32))`, and since `¬is-resource(i32)` there is no outstanding obligation to hand over, so `[Release]`'s trusted condition holds vacuously; `end-object(o_0)` runs, setting `a_r'`'s (the borrow `r` holds) `.valid := false` (`a_r' ∈ A_{o_0}`) and dropping it from every `held-by`. `[Rawptr-Write]` of element 4 into the new buffer; `v.len := 5`. `*r`: `[Ref-Deref-Place]` gives place `a_r'`; `[Read]`'s guard `temporally-valid(a_r',Σ) ∧ alive(of(a_r',Σ),Σ)` is a shape `rule.control.flow-analysis`'s discharge table does not cover (its root is a dereferenced reference into reclaimed storage, not a tracked binding) → `discharge: dynamic`; `alive(o_0,Σ)` is now false → `[Read-Stale]` → `diag.stale-binding`. `[Fault-Unwind]` unwinds `hold_across_grow`'s body and parameter frames, then `main`'s body frame — `[Destroy]` of `v` (`solitary`: the exclusive parameter borrow ended with `hold_across_grow`'s parameter frame) → `Vec::drop` reclaims and reads its five plain `i32` elements (no obligations), then `[Deallocate]`s the buffer — down to `main`'s own parameter frame; `terminate(diag.stale-binding, Σ')`. Confirms the `CHG-0011` "Revisit conditions" first case with no rule change: the static front line (`conf.vec-ref-then-push-rejected`) only fires when the element reference is formed and used in the same function; reached through a reference parameter, `[Release]` → `[Object-End]` → `[Read-Stale]` is the only enforcement, exactly as `spec/21` §1's prose states |
| `conf.e2e-mutex-vec-resource-interior` | `ex.e2e-mutex-vec-resource-interior` | `n = 2`; `ok`; one inner `Vec::drop`, one `deallocate` (the inner buffer; the mutex's own storage is `fresh`, released by plain `end-object`, never `[Release]`) | `inner` starts and stays empty until moved (no push before the move). `Mutex::new(inner)`: `inner` (resource) in place position; `establish(mutex<Vec<i32>>)` grants `o_m` authority + obligation (`[Object-Establish-Resource]`; `[Sizeof-Mutex]`/`[Repr-Mutex]` fix its layout: one `inner` field plus `sizeof(usize)` impl-defined state cells); `store(sub(o_m,inner,Vec<i32>), place a_inner)` → `[Store-Sub-Relocate]` → `[Relocate-In]`: cells copied, `inner`'s obligation `{o_inner}` re-keyed to `{(o_m,inner)}`, authority re-keyed, `o_inner` ends; temp `o_m` adopted by `m` (`sync(o_m) := unlocked`). Block: `&m` → `[Borrow]` shared, `a_m'`; `lock(a_m')` → `[Lock]` (`sync-state(o_m)=unlocked`): forms `a_g` (exclusive, `of = o_m`, `target = sub-range(inner)`, `base = a_m'`) and a fresh `guard<Vec<i32>>` object `o_g` holding it (`a_g.held-by := {o_g}`); temp `o_g` adopted by `g`; `a_m'` is Unheld at this statement's SE (nothing holds it), invalidated — `a_g`'s own validity does not depend on its ancestor staying valid (`temporally-valid` checks `a_g`'s own flag and `alive(o_m,Σ)`, not the base chain). `*g`: `g` in place position (a guard is a resource) → `[Guard-Deref]` → place `a_g`. `&mut *g`: `[Borrow]` exclusive from `a_g` (already exclusive; `¬clash`: the only other paths on `o_m`, `a_m` and the now-dead `a_m'`, are ancestors or invalid) → passed to `Vec::push`. First `push`: `inner.len = inner.cap = 0` → `grow` (`0→4`: `[Allocate]`, `copy_raw` of 0 bytes, no `deallocate` since `cap=0`); `[Rawptr-Write]` of element 0; `len := 1`. Second `push`: `len(1) ≠ cap(4)`, no grow; `[Rawptr-Write]` of element 1; `len := 2`. `&*g`: `[Guard-Deref]` then `[Borrow]` shared (no clash) → `Vec::len` reads `2`. Block exit: `o_g` owned here, obligated → `[Destroy]` of `g`: `solitary` (only path on `o_g`); `destructor(guard<τ>) = [Guard-Drop]` (built-in, `spec/07` §1) → `sync(o_m) := unlocked`, `a_g.valid := false`; no sub-obligations; `end-object(o_g)`. Main BE: `o_m` owned, obligation `{(o_m,inner)}` outstanding → `[Destroy]` of `m`: `solitary(a_m)` (`a_m'` and `a_g` both already invalid); `run-destructor`: the mutex's built-in destructor *is* the `destroy-composite` step that follows (`spec/07` §1 — no separate call); `destroy-composite(o_m)`: one obligation `(o_m,inner)` → fresh exclusive root into it → `[Run-Destructor]`: `destructor(Vec<i32>) = Vec::drop` → reclaims and reads its two plain `i32` elements (no obligations), `[Deallocate]`s the 4-slot buffer; `end-object(o_m)` (`storage-kind = fresh`: its own two cells' storage is released directly, not through `[Release]`). `[Terminate-Ok]` |
| `conf.e2e-closure-move-rc` | `ex.e2e-closure-move-rc` | `x = 1`, `y = 1`; `ok`; one inner `Vec::drop`, `RcBox` deallocated once | `Vec::push(&mut payload, 7)`: grows `0→4`, `len = 1`. `Rc::new(payload)`: as `conf.e2e-rc-resource-payload` — `payload` transferred into `v`; `RcBox{.count=1,.value=v}` → `[Store-Sub-Relocate]` (obligation `(o_rb,value)`); `*bp = temp` → `[Rawptr-Move-In]`: reclaimed `o'` with obligations `{o', (o',value)}`, authority re-keyed; `a`'s object is `o'`. `move [a]() { Vec::len(Rc::get(&a)) }`: free variables `{a}`, matching the written list; `[Closure-Form-Move]`: `c = {a: Rc<Vec<i32>>}` (`is-resource(c)` by derivation, `rule.agg.*`); `c{.a = a}` → `[Store-Sub-Relocate]`: `o'` relocated into `(o_f,a)`, `a`'s obligation `{o'}` re-keyed to `{(o_f,a)}`, `a`'s own binding ends; temp `o_f` adopted by `f`. `f()` (both calls): `[Closure-Call]`: `borrow(a_f, exclusive)` → `a_self`; body' = `Vec::len(Rc::get(&self.a))`; `&self.a`: `[Borrow]` shared, projection into `(o_f,a)` (no clash: nothing else live on `o_f`); `Rc::get`: `reclaim<RcBox<Vec<i32>>>(r.ptr)` re-attaches `o'` (still alive, `storage-kind = reclaimed` — the same object both calls); `.value` projection, `[Borrow]` shared; `Vec::len` → `1`. Every borrow formed inside ends with its own call's frame/statement exits, so the second call finds `o_f` exactly as the first left it. `drop(f)`: `f` in place position; `[Destroy]`: `solitary(a_f)` (`a_self` and its descendants all ended with their calls); `destructor(c) = None` (anonymous closure type, no user `drop`) → `[Run-Destructor-None]`, no-op; `destroy-composite(o_f)`: one obligation `(o_f,a)` → fresh exclusive root → `[Run-Destructor]`: `destructor(Rc<Vec<i32>>) = Rc::drop`, exactly as `conf.e2e-rc-resource-payload` derives for a last drop — count `1→0`, `last`, the counting `unsafe` block's borrow `b` ends before the second, which `[Destroy]`s `o'` (solitary: no path survives `b`'s block) via `destructor(RcBox<Vec<i32>>) = None` → `destroy-composite(o')`'s one obligation `(o',value)` → `Vec::drop` through a temporary exclusive root: reclaims and destroys the one plain element, `[Deallocate]`s its 4-slot buffer; `end-object(o')` (reclaimed: storage kept); `[Deallocate]` of the `RcBox` allocation finds no reclaimed object to release; `end-object(o_f)`. `[Terminate-Ok]` |
| `conf.e2e-vec-pop-push-reuse-stale-ref` | `ex.e2e-vec-pop-push-reuse-stale-ref` | ✗ `diag.stale-binding` (dynamic); no other output | `main`: three pushes build `v = [10,20,30]`, `len = 3`, `cap = 4` (grown once `0→4`). `hazard(&mut v)`: `&*v` reborrows shared (as `conf.e2e-vec-realloc-stale-ref`, no static tracking through a reference parameter). `Vec::index_shared(&*v, 2)`: `[Reclaim]` establishes fresh `o_2` (`i=2<3`; `¬is-resource(i32)`: no obligation); `[Borrow]` shared → `a_r'`, stored in `r`. `Vec::pop(v)`: `v.len := 2`; `p = v.ptr + 2`; `auto x = *p;` → `[Rawptr-Read]` (`i32` plain): the tightened trusted condition needs no *exclusive* competitor — `a_r'` is shared, so it does not block this read regardless; reads `30`. `release(reinterpret_ptr<u8>(p), sizeof<i32>())` → `[Release]`: `R = {o_2}` (`extent(o_2) ⊆ [addr,addr+4)`); no obligation to discharge (plain) → trusted condition holds vacuously; `end-object(o_2)` — `a_r'` (`of = o_2`) is directly invalidated (`a_r' ∈ A_{o_2}`), not merely `alive(o_2)` becoming false. `Some(x)` discarded. `Vec::push(v, 99)`: `len(2) ≠ cap(4)`, no grow; `*(v.ptr+2) = 99` → `[Rawptr-Write]`: the tightened trusted condition ("no live derived path of *any* object reaches them") now holds because there is genuinely nothing left there — `o_2` is gone, `a_r'` invalid; under the pre-`CHG-0019` wording this write would have been "trusted" anyway (via the exemption it no longer needs), landing on cells `r` still (incorrectly) aliased. `v.len := 3`. `*r`: `[Ref-Deref-Place]` gives place `a_r'`; `[Read]`'s guard is a shape the discharge table does not cover (root is a dereferenced reference into reclaimed storage) → `discharge: dynamic`; `a_r'.valid` is already `false` (set directly by `[Object-End]` during `release`, not merely `alive(o_2,Σ)`) → `[Read-Stale]` → `diag.stale-binding`. `[Fault-Unwind]` unwinds `hazard`'s frames and `main`'s body frame (`[Destroy]` of `v` → `Vec::drop` over its three plain elements, `[Deallocate]`) down to `main`'s parameter frame; `terminate(diag.stale-binding, Σ')`. Closes **F-05**: before `CHG-0019`, `[Rawptr-Read]`'s benign trusted condition and `Vec::pop`'s missing `release` would have left `o_2` alive through the `push`, so `*(v.ptr+2) = 99` would have landed on cells `a_r'` still validly targeted, and `*r` would have read `99` with no diagnostic at all |
| `conf.spawn-join-resource-result` | `ex.e2e-spawn-join-resource-result` | `n = 1`; `ok`; two `Vec::drop`s, two `deallocate`s | `spawn(make)` (twice): `[Spawn]` establishes each handle in `ℓ_0` (its own `authority(ℓ_0,destroy,o_h)` is fine from the start — only the *result* crosses threads); inside each worker: `Vec::new()`+`push` grows `0→4`, `len=1`; trailing `v` → `[Store-Result-Place-Release]`: temp `o_v` (authority granted to the *worker* thread by `[Object-Establish-Resource]`, since that is the performer of `establish`), flows through both `[Block-Exit]`s untouched (excluded as the result); `[Thread-Body-Done]`: `threads(ℓ').value := temp o_v`. `join(h1)`: `[Join]` blocks until `ℓ1` finishes, then `threads(ℓ1).value := taken` and — the fix — `authority(ℓ_0,destroy,o_{v1}) := {consumed:false}`, `authority(ℓ1,destroy,o_{v1}).consumed := true` (`o_{v1} ∈ objs-in(r_b)`); `destroy(a_{h1})` → `[Handle-Destructor]` finds `taken`, discards nothing. `auto v1 = join(h1);` adopts the temp. `Vec::len(&v1)` → `1`. `drop(v1)`: **without `CHG-0015` this is `[Destroy-No-Authority]`** — `authority(ℓ_0,destroy,o_{v1})` would be undefined, since establishment granted it to `ℓ1`; with the fix, `[Destroy]` proceeds (`solitary` ✓) → `Vec::drop` → `[Deallocate]`. Inner block: `h2` never joined; at block exit `[Destroy]` of `h2` → `[Handle-Destructor]` blocks until `ℓ2` finishes, `threads(ℓ2).value = temp o_{v2} ≠ taken` → re-keys `authority(ℓ_0,destroy,o_{v2})` the same way, *then* discards: `o_{v2} ∈` `destruction-obligations` → `destroy`s it (not merely `end-object`s it) → `Vec::drop` → `[Deallocate]` — **without the fix this second `destroy` attempt is also `[Destroy-No-Authority]`**, so an unjoined thread returning any resource would fault the whole program on its own automatic cleanup. `[Terminate-Ok]` |
| `conf.e2e-hello-print` | `ex.e2e-hello-print` | `ok`; the bytes of `"Hello, CobaltC!\n"` written, in three `stdout_write` calls | `[Program]` → `main`. `str who = "CobaltC";`: `[Str-Literal]` → `⟪67, 111, 98, 97, 108, 116, 67⟫ : str`; `[Let]` → `[Store-Binding-Value]` into `o_who` (plain). `String::from_str(who)`: `who` in value position → `[Read]` copies (`¬is-resource(str)`; `a_who` untouched); as `conf.string-from-str` with `n = 7`: `grow` 0→4 at the first push, 4→8 at the fifth (`[Allocate]`, `copy_raw` of 4 × 1 byte, `[Deallocate]` → `[Release]` ends the four reclaimed element objects `push` never established — vacuous), seven `[Rawptr-Write]`s; `String { .bytes = v }` relocates; temp adopted by `owned` (`is-resource(String)` by derivation: authority and obligation `{o_owned, (o_owned, bytes)}`). `String::len(&owned)` → 7; `str_len(who)` → 7 (`[Read]` again, still no move); `[Cmp-Ne]` → `false` → `[If-No-Else]` skips the never-reached fault. Three `printf` calls, each as `conf.print-observed`: `[Str-Ptr]` widens `storage` with 7, 7, and 2 bytes respectively (the second call's `who` is `[Read]` a third time), `[Extern-Call]` inside `std`'s `unsafe` block, three `isize` claims discarded. Main BE: `Owned = {o_who, o_owned}` descending origin → `o_owned` first: `[Destroy]` (solitary: every `&owned` borrow died with `len`'s parameter object) → `destructor(String) = None` → `[Run-Destructor-None]` → `[Destroy-Composite]` on `(o_owned, bytes)` → `Vec::drop`: reclaims and reads seven plain `u8` elements (no obligations), `[Deallocate]` of the 8-slot buffer; `[Object-End]` of `o_owned`; then `o_who` ends (plain, `[Object-End]`). `[Terminate-Ok]`. Observable behavior (`rule.fn.program`): outcome `ok` and the `extern` sequence `write(·,7), write(·,7), write(·,2)` whose cells hold the sixteen bytes of `"Hello, CobaltC!\n"` in order |
| `conf.e2e-thread-fault-abandons-guard` | `ex.e2e-thread-fault-abandons-guard` | ✗ `diag.div-by-zero`; deterministic for every interleaving | `Mutex::new(0)`: `inner` plain (`i32`), `Obl = ∅` (`rule.resauth.relocate-in`'s plain-source case); temp `o_m` adopted by `m`. `lock(&m)` → `[Lock]`: forms `a_g` (exclusive lock path) and guard `o_g`, adopted by `g`. `spawn(worker, 5)`: `[Spawn]` establishes handle `o_h`, fresh thread `ℓ'`, `current-scope(ℓ') = ⊥` (`spec/04` 1.2.0, `CHG-0011` E-01); `n := 5` bound in `ℓ'`'s parameter frame `f'`. From here, `[Thread-Step]` interleaves `ℓ_0` (`main`) and `ℓ'` (`worker`) in any order (`outcome: unspecified { any interleaving }`), but the two candidate `main` steps left (`*g = *g + 1`, then blocking in `[Join]`) perform no `extern` call and their values are never read again, so no interleaving changes what is observable. In `ℓ'`: body block pushes `f'_body`; `n / 0` → `[Div-By-Zero]` → `⟨n/0,Σ⟩ ↛ diag.div-by-zero` in thread `ℓ'`. `[Fault-Unwind]` fires program-wide: `f_0 =` the bottom of `Σ.frame-stack(ℓ')` = `f'`; `unwind-to(f', (), Σ)` folds only `ℓ'`'s own stack (`stmt-scoped(n/0,keep)`'s SE, `[Block-Exit]` of `f'_body` — `n` is bound in `f'`, not swept here — then `[Block-Exit]` of `f' = f_target`, ending `n`'s plain object) — `ℓ_0` "takes no further steps" and "is not unwound" (`spec/18` §1): whatever `main` had or had not done to `g`/`m`/`h` by that point is simply abandoned, mid-statement or mid-block if that is where the interleaving left it — no destructor for `g` or `m` ever runs, and `h`'s handle is never joined or dropped. `⟨program, Σ⟩ ↛↛ terminate(diag.div-by-zero, Σ')`. No design gap: `rule.fn.program`'s observable behavior is only the termination outcome and the `extern`-call sequence, neither of which `main`'s abandoned steps ever touch, so the outcome is fully deterministic despite the interleaving and despite the never-run guard/mutex destructors — exactly the consequence `spec/18` §1's prose already states ("other threads take no further steps; their frames are not unwound... a second unwind could itself fault") |

## 14. Modules, fields, and program assembly (`spec/17`, `spec/15` §7)

A fragment in this section that is a path to a `.cb` file under
`impl/conformance/` is a **whole program** in the file-based suite,
run as `coby <file>` runs it, with its fixture files (first line
`// CONFORMANCE-FIXTURE`) beside it — the only way a case can span
files, since a table cell holds one fragment (`CHG-0029`). The row's
outcome is that program's.

| id | Fragment | Outcome | Derivation |
|---|---|---|---|
| `conf.module-file-ok` | `impl/conformance/17-modules/file_module_qualified_call_ok.cb` | `ok` | `[Module-File]`: `geo` is `module geo { I }` with `I` = `fixtures/geometry.cb`'s items; `geo::square` → `[Resolve-Qualified]`, `visible` (export) |
| `conf.module-file-import-ok` | `impl/conformance/17-modules/file_module_import_ok.cb` | `ok` | as above; `import geo::square;` → `[Resolve-Unqualified]` clause (2), `rule.module.use` untouched by the file form |
| `conf.item-intrinsic-not-inherited` | `impl/conformance/17-modules/item_named_intrinsic_stays_in_its_module_ok.cb` | `ok` | the root's own `reinterpret`, `sizeof`, `allocate`, `widen`, `drop`, `dangling` are its root code's; `std`'s `Vec`, `String`, `HashMap`, `Box`, `Rc`, `File` and a nested module still reach the intrinsics (`[Resolve-Unqualified]` (3), D-0055) |
| `conf.file-module-intrinsic-not-inherited` | `impl/conformance/17-modules/file_module_intrinsic_not_inherited_ok.cb` | `ok` | a library file calling `sizeof<u32>()` and `widen` gets the intrinsics although the loading program declares `sizeof` and `widen` at its root |
| `conf.item-intrinsic-nested-rejected` | `impl/conformance/17-modules/item_named_intrinsic_not_seen_from_nested_rejected.cb` | ✗ `diag.type-mismatch` (static) | in a nested module, `sizeof(1)` is the intrinsic, not the root's `fn sizeof` |
| `conf.module-file-type-ok` | `impl/conformance/17-modules/file_module_types_visible_to_prescan_ok.cb` | `ok` | `geo::Rect a = …` and, after `import geo::Rect;`, `Rect b = …` are declaration statements (`spec/22` §2 rule 4: the item set spans files); `[Struct-Construct]` with `export` fields (`[Field-Not-Visible]` not triggered) |
| `conf.module-file-private-rejected` | `impl/conformance/17-modules/file_module_private_item_rejected.cb` | ✗ `diag.name-not-visible` (static) | `geo::scale_factor` is not `export`; `[Resolve-Not-Visible]` across the file boundary exactly as inline |
| `conf.module-file-nested-ok` | `impl/conformance/17-modules/file_module_nested_relative_path_ok.cb` | `ok` | `fixtures/lib/a.cb` declares `export module b "./b.cb";` — `[Module-File]` resolves `./b.cb` against `lib/`, the declaring file's directory; `a::b::f` visible (both `export`) |
| `conf.module-file-missing-rejected` | `impl/conformance/17-modules/file_module_missing_file_rejected.cb` | ✗ `diag.module-file-not-found` (static) | `[Module-File-Missing]` |
| `conf.module-file-cycle-rejected` | `impl/conformance/17-modules/file_module_cycle_rejected.cb` | ✗ `diag.module-cycle` (static) | `cycle/a.cb` → `cycle/b.cb` → `cycle/a.cb` while `a.cb`'s body is being assembled: `[Module-File-Cycle]` |
| `conf.module-file-duplicate-rejected` | `impl/conformance/17-modules/file_module_duplicate_file_rejected.cb` | ✗ `diag.module-file-duplicate` (static) | two declarations name `fixtures/geometry.cb`: `[Module-File-Duplicate]` |
| `conf.module-file-main-is-ordinary-ok` | `impl/conformance/17-modules/file_module_main_in_file_is_ordinary_item_ok.cb` | `ok` | the loaded file's `main` is the item `lib::main` (`spec/17` §5); the root `main` is the entry (`[Program]`) |
| `conf.field-private-read-rejected` | `impl/conformance/17-modules/field_private_read_rejected.cb` | ✗ `diag.name-not-visible` (static) | `p.a` names private field `a` of `m::P` from the root: `[Field-Not-Visible]` |
| `conf.field-private-literal-rejected` | `impl/conformance/17-modules/field_private_struct_literal_rejected.cb` | ✗ `diag.name-not-visible` (static) | `m::P { .a = 1 }` names `a`: `[Field-Not-Visible]` |
| `conf.field-private-destructure-rejected` | `impl/conformance/17-modules/field_private_destructure_rejected.cb` | ✗ `diag.name-not-visible` (static) | `W { v } = …` names `v`: `[Field-Not-Visible]` |
| `conf.field-export-ok` | `impl/conformance/17-modules/field_export_ok.cb` | `ok` | `export i32 a` read, written and constructed from the root, and `Q`'s exported fields destructured there; the private `hidden` named only inside `m` (`visible`: `M` is `M'`) |
| `conf.unbound-qualified-name-static` | `impl/conformance/17-modules/unbound_qualified_name_rejected.cb` | ✗ `diag.unbound-name` (static) | `m::absent` is a qualified path, never a binding: `[Resolve-Unbound]`, a static fact |
| `conf.duplicate-fn-rejected` | `impl/conformance/17-modules/duplicate_fn_rejected.cb` | ✗ `diag.duplicate-item` (static) | `fn f` declared twice at the root: `[Item-Duplicate]`, at the second |
| `conf.duplicate-struct-rejected` | `impl/conformance/17-modules/duplicate_struct_rejected.cb` | ✗ `diag.duplicate-item` (static) | `struct P` twice, never used: `[Item-Duplicate]` |
| `conf.duplicate-variant-fn-rejected` | `impl/conformance/17-modules/duplicate_variant_fn_rejected.cb` | ✗ `diag.duplicate-item` (static) | variant `E::A` and `fn E::A` add the same name: `[Item-Duplicate]` |
| `conf.duplicate-module-rejected` | `impl/conformance/17-modules/duplicate_module_rejected.cb` | ✗ `diag.duplicate-item` (static) | two `module m` declarations: `[Item-Duplicate]` |
| `conf.same-name-other-module-ok` | `impl/conformance/17-modules/same_name_other_module_ok.cb` | `ok` | `f` and `m::f` are distinct qualified names |
| `conf.foreign-assoc-fn-rejected` | `impl/conformance/17-modules/foreign_assoc_fn_rejected.cb` | ✗ `diag.foreign-associated-fn` (static) | `fn Vec::first_or<T>` at the root; `Vec` is declared in `std`, not the root: `[Assoc-Fn-Foreign-Type]` |
| `conf.std-private-field-rejected` | `impl/conformance/17-modules/std_private_field_rejected.cb` | ✗ `diag.name-not-visible` (static) | `v.len = 100000` from the root names `std::Vec`'s private field `len`: `[Field-Not-Visible]` (the root is not `std` nor nested in it) |
| `conf.std-not-imported-unbound` | `impl/conformance/17-modules/std_not_imported_unbound.cb` | ✗ `diag.unbound-name` (static) | `printf` with no `import std;`: no clause of `[Resolve-Unqualified]` yields an item |
| `conf.std-qualified-ok` | `impl/conformance/17-modules/std_qualified_ok.cb` | `ok` | `std::Vec`, `std::Vec::new`, `std::printf` by `[Resolve-Qualified]`, every hop exported; no import needed |
| `conf.std-own-declaration-wins-ok` | `impl/conformance/17-modules/std_own_declaration_wins_ok.cb` | `ok`; prints `[own] hello` | the root's `printf` by clause (1) before `std::printf` by clause (2b); `printf` and `std::printf` are distinct names, so `[Item-Duplicate]` does not apply |
| `conf.std-import-per-module` | `impl/conformance/17-modules/std_import_per_module_rejected.cb` | ✗ `diag.unbound-name` (static) | `printf` inside `m`, whose own items and imports have no `printf`; the root's `import std;` acts only in the root (`rule.module.use`) |
| `conf.module-import-exports-ok` | `impl/conformance/17-modules/module_import_exports_ok.cb` | `ok` | `import geo;`: `area` by clause (2b), `Square`/`Dot` by clause (4) through the imported module's exported enum, `geo::area` as before |
| `conf.module-import-private-unbound` | `impl/conformance/17-modules/module_import_private_unbound.cb` | ✗ `diag.unbound-name` (static) | clause (2b) brings only exported items; `geo::hidden` is private |
| `conf.module-import-ambiguous-rejected` | `impl/conformance/17-modules/module_import_ambiguous_rejected.cb` | ✗ `diag.ambiguous-name` (static) | `f` used unqualified; clause (2b) yields `a::f` and `b::f`: `[Resolve-Ambiguous]` |
| `conf.module-import-unused-clash-ok` | `impl/conformance/17-modules/module_import_unused_clash_ok.cb` | `ok` | the same imports with `f` never used unqualified: ambiguity is a property of a use, not of the imports |
| `conf.module-import-no-submodule` | `impl/conformance/17-modules/module_import_brings_no_submodule_ok.cb` | `ok`; prints `42` | `import std;` in `cpu` does not bring in `std::memory`, so `memory::size` is the program's root `memory` (clause (2b), D-0136) |
| `conf.reexport-module` | `impl/conformance/17-modules/reexport_module_ok.cb` | `ok`; prints `3 7 4 5 3` | `export import lib::shapes;`: `lib::shapes`'s exported struct, its associated function, its enum and variants, and its function resolve through `import lib;`; `lib::Square` is `lib::shapes::Square` (`rule.module.use`, D-0136) |
| `conf.reexport-item` | `impl/conformance/17-modules/reexport_item_ok.cb` | `ok`; prints `7` | `export import a::seven;` makes `b::seven` an alias of `a::seven` |
| `conf.reexport-item-only` | `impl/conformance/17-modules/reexport_item_only_that_item_rejected.cb` | ✗ `diag.unbound-name` (static) | a re-export of one item passes on that item only: `b::eight` names nothing |
| `conf.reexport-chain` | `impl/conformance/17-modules/reexport_chain_ok.cb` | `ok`; prints `1` | `c` re-exports `b`, which re-exports `a`: `import c;` brings in `a::one` |
| `conf.reexport-same-item-twice` | `impl/conformance/17-modules/reexport_same_item_twice_ok.cb` | `ok`; prints `2` | one item brought in three ways is one name: not `[Item-Duplicate]` |
| `conf.reexport-conflict-rejected` | `impl/conformance/17-modules/reexport_conflict_rejected.cb` | ✗ `diag.duplicate-item` (static) | `m` declares `f` and re-exports `a`, whose `f` is a different item: `[Item-Duplicate]` at the `export import` |
| `conf.reexport-cycle` | `impl/conformance/17-modules/reexport_cycle_ok.cb` | `ok`; prints `3 4` | `a` and `b` re-export each other: each has both modules' items, once |
| `conf.reexport-private-not-passed` | `impl/conformance/17-modules/reexport_private_not_passed_rejected.cb` | ✗ `diag.unbound-name` (static) | a re-export passes on exported items only: `lib::helper` names nothing |
| `conf.reexport-unbound-rejected` | `impl/conformance/17-modules/reexport_unbound_rejected.cb` | ✗ `diag.unbound-name` (static) | `[Reexport-Unbound]` |
| `conf.std-submodule-path` | `impl/conformance/21-standard-library-semantics/std_submodule_paths_ok.cb` | `ok`; prints `2 3 ab` | `std::collections::Vec` is `std::Vec`, `std::text::String` is `String`, `std::math::sqrt` is `sqrt` (`spec/21` §0, D-0136) |
| `conf.std-nested-submodule-path` | `impl/conformance/21-standard-library-semantics/std_nested_submodule_paths_ok.cb` | `ok`; prints `186 173 true` | A submodule of `std` may have submodules, each re-exported up to the root (D-0187): `std::crypto::digest::sha256` is `std::crypto::sha256` and `sha256`, `std::http::client::HttpClient` is `HttpClient`, `std::database::postgres::PgSslMode` is `PgSslMode`; `import std::crypto::digest;` imports one alone. |
| `conf.std-extensions-empty` | `impl/conformance/21-standard-library-semantics/std_extensions_empty_ok.cb` | `ok`; prints `ok` | `std::extensions` exists and may be empty (D-0187): `import std::extensions;` is accepted and brings in nothing the specification defines. |
| `conf.std-encoding-json-paths` | `impl/conformance/21-standard-library-semantics/std_encoding_json_paths_ok.cb` | `ok`; prints `[1,true,{"k":null}]` | JSON is `std::encoding::json` (D-0197), re-exported by `std::encoding` and by `std`: `std::encoding::json::Json`, `std::encoding::Json` and `Json` are one type, and `import std::encoding::json;` imports the format alone. |
| `conf.std-encoding-hex-base64-paths` | `impl/conformance/21-standard-library-semantics/std_encoding_hex_base64_paths_ok.cb` | `ok`; prints `4d616e 4d616e 4d616e`, `TWFu TWFu`, `110 77` | Hexadecimal is `std::encoding::hex` and base64 `std::encoding::base64` (D-0198), each re-exported by `std::encoding` and by `std`: `std::encoding::hex::to_hex`, `std::encoding::to_hex` and `to_hex` are one function, and `import std::encoding::hex;` imports one encoding alone. |
| `conf.std-text-has-no-hex` | `impl/conformance/21-standard-library-semantics/std_text_has_no_hex_rejected.cb` | ✗ `diag.unbound-name` (static) | `rule.stdlib.prelude` (D-0198): `to_hex` is in `std::encoding::hex`, so `import std::text;` alone does not name it |
| `conf.text-search-edges` | `impl/conformance/21-standard-library-semantics/text_search_natives_ok.cb` | `ok`; prints its `expect-stdout-hex` | `find`, `contains`, `starts_with`, `ends_with` of a `String` and a `StringView` at their edges (an empty needle found at 0, a needle longer than the text, multi-byte text); `String::view`, `String::from_view` of a view, `append` of views of another `String`, empty ones included (D-0199: native in `cobc`, no rule changed). |
| `conf.text-append-view-of-self` | `impl/conformance/21-standard-library-semantics/text_view_append_self_rejected.cb` | ✗ `diag.aliasing-conflict` (dynamic) | `rule.stdlib.text` (D-0199): appending to a `String` a view of that same `String` clashes. |
| `conf.text-append-empty-view-of-self` | `impl/conformance/21-standard-library-semantics/text_view_append_self_empty_ok.cb` | `ok`; prints `hello` | `rule.stdlib.text` (D-0199): an empty view of a `String` appended to it appends nothing and borrows nothing, so it does not clash. |
| `conf.text-substring-not-char-boundary` | `impl/conformance/21-standard-library-semantics/text_view_boundary_rejected.cb` | ✗ `diag.not-char-boundary` (dynamic) | `rule.stdlib.text` (D-0199): `String::from_view(String::view(s, lo, hi))` with `hi` inside a character. |
| `conf.option-unwrap-ref-none` | `impl/conformance/21-standard-library-semantics/option_unwrap_ref_none_rejected.cb` | ✗ `diag.unwrap-failed` (dynamic) | D-0082, D-0199: `Option::unwrap` of a `None` that would have held a reference. |
| `conf.objectless-lend-and-move` | `impl/conformance/11-initialization/objectless_lend_and_move_ok.cb` | `ok`; prints `3 rows, 7 fields` and `0 1` | `rule.value-object.store` (D-0200): containers with no runtime object lent to functions that keep nothing, moved into a container, a variant and a result, and dropped on every path. |
| `conf.objectless-moved-then-used` | `impl/conformance/11-initialization/objectless_moved_then_used_rejected.cb` | ✗ `diag.stale-binding` (static) | `[Binding-Lookup-Stale]` (D-0200): a `Vec` with no runtime object moved on one path, then lent again. |
| `conf.raw-form-move-to-call` | `impl/conformance/11-initialization/raw_form_move_to_call_ok.cb` | `ok`; prints `104` | `rule.value-object.store` (D-0200): in a function with a `__raw` form, a local with no object moved into a call takes an object there. |
| `conf.fused-text-loops` | `impl/conformance/14-control-flow/fused_text_loops_ok.cb` | `ok`; prints `5 words 14 bytes`, `14 one-byte chars`, `2`, `0` | `rule.control.foreach` (D-0200): `foreach` over `chars` and `split_whitespace`, the pieces viewed, trimmed and copied. |
| `conf.elem-ref-local` | `impl/conformance/08-alias-validity/elem_ref_local_ok.cb` | `ok`; prints `a 10 8 10`, `b 64 8 64`, `99` | `rule.alias.borrow` (D-0200): references to elements of local vectors held in locals, read and written through and lent on. |
| `conf.elem-ref-local-conflict` | `impl/conformance/08-alias-validity/elem_ref_local_rejected.cb` | ✗ `diag.aliasing-conflict` (static) | `rule.alias.borrow` (D-0200): a vector lent exclusively while a reference to one of its elements lives. |
| `conf.map-get-unwrap-read` | `impl/conformance/21-standard-library-semantics/map_get_unwrap_ok.cb` | `ok`; prints `21 -39` and `k20 k11 k3 ` | `rule.stdlib.hashmap` (D-0200): `*Option::unwrap(HashMap::get(&m, k))` read at once, also inside a comparator that captures the map. |
| `conf.map-get-unwrap-none` | `impl/conformance/21-standard-library-semantics/map_get_unwrap_none_rejected.cb` | ✗ `diag.unwrap-failed` (dynamic), after printing `1` | `rule.stdlib.hashmap` (D-0200): the same for a key the map lacks. |
| `conf.file-read-line-utf8-error` | `impl/conformance/21-standard-library-semantics/file_read_line_utf8_error_ok.cb` | `ok`; prints `line [ok]`, `utf8 at 0`, `line [last]`, `end` | `rule.stdlib.file-handle` (D-0200): `File::read_line` over a line that is not UTF-8, which it consumes, and a last line with no `\n`. |
| `conf.file-write-buffer-visible` | `impl/conformance/21-standard-library-semantics/file_write_buffer_visible_ok.cb` | `ok`; prints `[abc⏎]`, `4`, `line [abc]`, `7 7`, `[abc⏎defghi]` | `[File-Buffer]` (D-0201): buffered writes are seen through `read_file`, `file_info`, a second handle, the writer's `position`/`len`, and after a handle is destroyed unclosed. |
| `conf.spawn-in-raw-form` | `impl/conformance/19-concurrency/spawn_in_raw_form_ok.cb` | `ok`; prints `9 4 17` | `rule.conc.spawn` (D-0200): two functions that spawn, one of them also lowered as its `__raw` form; each spawn keeps a thread body of its own. |
| `conf.binder-moved-into-literal` | `impl/conformance/11-initialization/binder_moved_into_literal_ok.cb` | `ok`; prints `data 1400000 200000` three times, then `missing` | `rule.value-object.store` (D-0200): a match binder with no runtime object moved into a struct literal beside a `String` made with no object, the function's result; the bytes stay the result's. |
| `conf.std-import-one-subject` | `impl/conformance/21-standard-library-semantics/std_import_one_subject_ok.cb` | `ok`; prints `hello, 5` | `import std::text; import std::io;` without `import std;` |
| `conf.std-import-one-subject-only` | `impl/conformance/21-standard-library-semantics/std_import_one_subject_rejected.cb` | ✗ `diag.unbound-name` (static) | `import std::text;` alone does not bring in `Vec` (`std::collections`) |
| `conf.std-subjects-fs-env` | `impl/conformance/21-standard-library-semantics/std_subjects_fs_env_ok.cb` | ok | `rule.stdlib.prelude` (D-0176): `std::io`, `std::fs`, `std::env` and `std::time` imported alone, one item of each used |
| `conf.std-bytes-subjects` | `impl/conformance/21-standard-library-semantics/std_bytes_subjects_ok.cb` | ok | `rule.stdlib.prelude` (D-0177): `read_le`/`read_be` named through `import std::collections;` alone and `crc32` through `import std::crypto;` alone |
| `conf.std-text-has-no-bytes` | `impl/conformance/21-standard-library-semantics/std_text_has_no_bytes_rejected.cb` | ✗ `diag.unbound-name` (static) | `rule.stdlib.prelude` (D-0177): `read_le` is in `std::collections`, so `import std::text;` alone does not name it |
| `conf.std-io-has-no-files` | `impl/conformance/21-standard-library-semantics/std_io_has_no_files_rejected.cb` | ✗ `diag.unbound-name` (static) | `rule.stdlib.prelude` (D-0176): `read_file` is in `std::fs`, so `import std::io;` alone does not name it |
| `conf.std-message-short-path` | `std::collections::Vec<i32> v = Vec::new(); i32 x = v;` | ✗ `diag.type-mismatch` (static), naming `Vec<i32>` | a message names a `std` item by its shortest path |
| `conf.module-struct-destructor-runs` | `impl/conformance/17-modules/module_struct_destructor_runs.cb` | `ok`; prints `dropped` | `fn S::drop` in `m`, which declares `S`, adds `m::S::drop`: `S`'s destructor (`spec/07` §1), run by `[Destroy]` at `main`'s block exit |
| `conf.no-main` | `impl/conformance/15-function-semantics/no_main_rejected.cb` | ✗ `diag.no-main` (static) | no `fn main()` at the root (`m::main` is an ordinary item): `[Program-No-Main]` |
| `conf.const-folded` | `const u64 KB = 1024; const u64 MB = KB * KB;` `MB` | `→ 1048576` | `rule.module.const` (`spec/17` §1a, D-0036): a use of `MB` is its value; `KB * KB` in `u64` |
| `conf.const-struct` | `struct P { i32 x; } const P ORIGIN = P { .x = 3 };` `ORIGIN.x` | `→ 3` | a constant of a plain struct type; each use is the value |
| `conf.const-exported` | `module m { export const i32 K = 7; }` `m::K` | `→ 7` | a `const` is an item: visibility and qualified paths as for any (`rule.module.resolve`) |
| `conf.const-shadowed-by-local` | `const i32 K = 7; fn f() : i32 { i32 K = 1; K }` `f()` | `→ 1` | a local binding is found before an item (`[Resolve-Unqualified]`) |
| `conf.const-cycle-rejected` | `const i32 A = B + 1; const i32 B = A;` | ✗ `diag.const-not-constant` (static) | `[Const-Cycle]` |
| `conf.const-not-constant-rejected` | `fn f() : i32 { 3 } const i32 A = f();` | ✗ `diag.const-not-constant` (static) | a call of a function is not a constant expression: `[Const-Not-Constant]` |
| `conf.const-resource-rejected` | `const Vec<i32> V = Vec::new();` | ✗ `diag.const-not-constant` (static) | a constant's type is not a resource: `[Const-Not-Constant]` |
| `conf.const-overflow-static` | `const i32 M = 2147483647; const i32 N = M + 1;` | ✗ `diag.arith-overflow` (static) | `[Const-Checked-Failure]`: the initializer's checked failure is a static rejection, at the constant |
| `conf.const-assign-rejected` | `const i32 A = 5;` `A = 6;` | ✗ `diag.type-mismatch` (static) | a constant's use is a value, not a place (`spec/22` `assign`) |
| `conf.static-assert-holds` | `struct H { u32 a; u16 b; u16 c; u64 d; } const usize BUF = 4096;` `static_assert(sizeof<H>() == 16); static_assert(BUF & (BUF - 1) == 0, "power of two");` | ok | D-0052 `[Static-Assert]`: true conditions; nothing happens at run time |
| `conf.static-assert-fails` | `static_assert(sizeof<u64>() == 4, "u64 is 8 bytes");` | ✗ `diag.static-assert-failed` (static) | `[Static-Assert-Failed]`, with the message |
| `conf.static-assert-uncalled` | `fn never() { static_assert(1 > 2); }` `` | ✗ `diag.static-assert-failed` (static) | every function is checked, called or not (`rule.fn.program`) |
| `conf.static-assert-generic` | `fn small<T>(T x) : T { static_assert(sizeof<T>() <= 8); x }` `small(5: i64); small(1: i128);` | ✗ `diag.static-assert-failed` (static) | checked per instantiation: `T = i64` holds, `T = i128` (16 bytes) fails |
| `conf.static-assert-not-constant` | `i32 k = 3; static_assert(k > 1);` | ✗ `diag.static-assert-not-constant` (static) | `[Static-Assert-Not-Constant]`: a local is not a constant expression |
| `conf.static-assert-overflow` | `static_assert(max_value<i32>() + 1 > 0);` | ✗ `diag.arith-overflow` (static) | `[Static-Assert-Checked-Failure]`: computed as at run time, before it |
| `conf.static-assert-not-bool` | `static_assert(5);` | ✗ `diag.static-assert-not-constant` (static) | `[Static-Assert-Not-Constant]`: the condition is a `bool` |
| `conf.view-basic` | `String line = String::from_str("  hello, world  "); StringView w = &line[2..7]; StringView r = &w[1..$]; StringView::len(w) == 5 && StringView::len(r) == 4 && StringView::len(&line[0..$]) == 16` | `→ true` | D-0053 `[View-Form]`: a view of a `String`, and of a view; `$` is the length in bytes |
| `conf.view-of-temporary-argument` | `impl/conformance/21-standard-library-semantics/view_of_temporary_argument_ok.cb` | ok | `[View-Form-Temporary-Argument]` (D-0135): a `String` a call returns, a field of a temporary, a nested call's result and a computed path, each viewed as an argument |
| `conf.view-of-call-result-argument` | `fn w() : String { String::from_str("abc") }` `StringView::len(&w()[0..$]) == 3 && String::find(&String::from_str("xabc"), &w()[1..$]) == Some(2)` | `→ true` | `[View-Form-Temporary-Argument]` (D-0135): the `String` lives to the statement's end |
| `conf.view-of-temporary-outside-argument-rejected` | `fn w() : String { String::from_str("abc") }` `StringView v = &w()[0..$];` | ✗ `diag.borrow-of-non-place` (static) | `[View-Form]` needs a place outside an argument (D-0135) |
| `conf.view-of-temporary-kept-faults` | `impl/conformance/21-standard-library-semantics/view_of_temporary_kept_faults.cb` | ✗ `diag.destroy-while-aliased` (dynamic) | a view returned from a temporary's view and kept past the statement: the `String` ends while the view holds it (D-0135, as D-0103's slices) |
| `conf.view-eq` | `String s = String::from_str("abcabc"); StringView a = &s[0..3]; StringView b = &s[3..6]; a == b && a == "abc" && "abc" == b && a != "abd"` | `→ true` | `[View-Eq]`: with a view or a `str`, either side |
| `conf.view-split-trim` | `String s = String::from_str(" a, bb ,c "); Vec<StringView> p = StringView::split(StringView::trim(&s[0..$]), ","); Vec::len(&p) == 3 && StringView::trim(p[1]) == "bb" && p[2] == "c"` | `→ true` | `split` and `trim`: every part is a view of the same `String` |
| `conf.view-parse` | `String s = String::from_str("x=42"); match (StringView::parse<i64>(&s[2..$])) { Ok(v) : v == 42, Err(_) : false }` | `→ true` | `String::parse<T>` of a view |
| `conf.view-push-while-held-rejected` | `String s = String::from_str("ab"); StringView v = &s[0..1]; String::push_ascii(&mut s, 33); StringView::len(v) == 1` | ✗ `diag.aliasing-conflict` (static) | a view borrows its `String` as a slice does |
| `conf.view-escape-rejected` | `fn bad() : StringView { String s = String::from_str("t"); &s[0..1] }` `bad();` | ✗ `diag.reference-escapes-scope` (static) | a view cannot outlive its `String` |
| `conf.view-char-boundary` | `String s = String::from_str("é!"); StringView h = &s[0..1];` | ✗ `diag.not-char-boundary` (dynamic) | `[View-Boundary]`: `é` is two bytes |
| `conf.view-exclusive-rejected` | `String s = String::from_str("ab"); auto m = &mut s[0..1];` | ✗ `diag.type-mismatch` (static) | `[View-Mode-Rejected]`: views are shared only |
| `conf.own-variant-wins` | `enum Shape { Dot(i32), Empty } Shape s = Empty; match (s) { Dot(x) : x, Empty : 5 }` | `→ 5` | `Empty` is a variant of the root's own `Shape` and of the imported `std::ParseError`: clause (4) finds `Shape` in the root, so (4b) is not consulted |
| `conf.qualified-variant-shared-name` | `enum Shape { Dot(i32), Empty } ParseError p = ParseError::Empty; match (p) { Empty : 1, _ : 2 }` | `→ 1` | `[Resolve-Qualified]`: `ParseError::Empty` names `std::ParseError`'s variant whatever else is called `Empty`; the arm matches by the scrutinee's enum |
| `conf.imported-variants-ambiguous` | `module a { export enum X { V } } module b { export enum Y { V } } import a; import b; auto x = V;` | ✗ `diag.ambiguous-name` (static) | no enum of the root has `V`, and clause (4b) finds two imported ones: `[Resolve-Ambiguous]` |
| `conf.main-exit-status` | `fn main() : u8 { 7 }` | `ok, exit status 7` | `main`'s value `7 : u8` (from the return type, `rule.type.expected`): `[Terminate-Ok]` gives `ok(7)` |
| `conf.main-exit-status-after-destructors` | `resource struct S { i32 x; } fn S::drop(ref<S, exclusive> self) { printf("dropped\n"); } fn main() : u8 { S s = S { .x = 1 }; return 3; }` | `ok, exit status 3`; prints `dropped` | `return 3` exits `main`'s frame through `[Block-Exit]`, which destroys `s` (`S::drop` prints); only then `[Terminate-Ok]` gives `ok(3)` |
| `conf.main-with-parameters-rejected` | `fn main(i32 x) { }` | ✗ `diag.no-main` (static) | `main` has a parameter: `[Program-No-Main]` |
| `conf.main-returns-i32-rejected` | `fn main() : i32 { 0 }` | ✗ `diag.no-main` (static) | `main`'s return type is neither `void` nor `u8`: `[Program-No-Main]` |
| `conf.const-local` | `impl/conformance/17-modules/const_local_ok.cb` | ok | `[Const-Local]`: `static_assert` on local constants; an inner block's constant shadows, as does an inner block's variable (D-0131); no capture in a closure; a pattern; in a loop body |
| `conf.const-field-element` | `impl/conformance/17-modules/const_field_element_ok.cb` | ok | a field of a constant struct and an element of a constant array are constant expressions: in a constant, in `static_assert`, in a local constant |
| `conf.const-local-names-variable` | `impl/conformance/17-modules/const_local_names_variable_rejected.cb` | ✗ `diag.const-not-constant` (static) | `[Const-Local-Not-Constant]`: `b` is a variable |
| `conf.const-local-names-type-parameter` | `impl/conformance/17-modules/const_local_names_type_parameter_rejected.cb` | ✗ `diag.const-not-constant` (static) | `[Const-Local-Not-Constant]`: `sizeof<T>()` |
| `conf.const-local-out-of-scope` | `impl/conformance/17-modules/const_local_out_of_scope_rejected.cb` | ✗ `diag.unbound-name` (static) | the constant's name ends with its block |
| `conf.local-bitstruct` | `impl/conformance/17-modules/local_bitstruct_ok.cb` | ok | `[Local-Type]` with a `bitstruct` (D-0132): `bits`/`from_bits` derived, fields read and written, a function beside it, an inner block's own `Pair` |
| `conf.redeclare-allowed-forms` | `impl/conformance/05-value-object-semantics/redeclare_allowed_forms_ok.cb` | ok | D-0131: nested blocks shadow; sibling blocks, successive loops and separate `match`es each declare their own; a closure's parameter and a local shadow outer names and items |
| `conf.redeclared-local-rejected` | `impl/conformance/05-value-object-semantics/redeclare_in_block_rejected.cb` | ✗ `diag.duplicate-local` (static) | `[Binding-Form-Redeclared]` |
| `conf.redeclared-for-variable-rejected` | `impl/conformance/05-value-object-semantics/redeclare_for_header_rejected.cb` | ✗ `diag.duplicate-local` (static) | a `for` header governs its body |
| `conf.redeclared-arm-binder-rejected` | `impl/conformance/05-value-object-semantics/redeclare_arm_binder_rejected.cb` | ✗ `diag.duplicate-local` (static) | an arm's binder governs the arm's block |
| `conf.redeclared-local-const-rejected` | `impl/conformance/05-value-object-semantics/redeclare_local_const_rejected.cb` | ✗ `diag.duplicate-local` (static) | a local constant and a variable share the region (D-0066) |
| `conf.const-auto` | `impl/conformance/17-modules/const_auto_ok.cb` | ok | `const auto` typed as `auto` types: a suffix or the default, an operand's type, struct, array and variant literals, fields, elements, intrinsics, a function; module and local |
| `conf.const-auto-undetermined` | `impl/conformance/17-modules/const_auto_undetermined_rejected.cb` | ✗ `diag.type-mismatch` (static) | `None` alone determines no type |
| `conf.static-assert-format-message` | `impl/conformance/17-modules/static_assert_format_message_rejected.cb` | ✗ `diag.static-assert-failed` (static) | the message formatted with the constants' values: "b is 201, not 202" |
| `conf.static-assert-message-variable` | `impl/conformance/17-modules/static_assert_message_variable_rejected.cb` | ✗ `diag.static-assert-not-constant` (static) | a message argument that is a variable |
| `conf.local-type` | `impl/conformance/17-modules/local_type_ok.cb` | ok | `[Local-Type]`: a struct with its function, an enum (qualified and bare variants), a recursive struct, a resource with its destructor, same-named types in sibling blocks |
| `conf.local-type-type-parameter` | `impl/conformance/17-modules/local_type_type_parameter_rejected.cb` | ✗ `diag.unbound-name` (static) | a local type cannot name the function's `T` |
| `conf.local-type-out-of-scope` | `impl/conformance/17-modules/local_type_out_of_scope_rejected.cb` | ✗ `diag.unbound-name` (static) | the type's name ends with its block |
| `conf.local-type-fn-no-variables` | `impl/conformance/17-modules/local_type_fn_no_variables_rejected.cb` | ✗ `diag.unbound-name` (static) | a function of a local type does not see the enclosing function's variables |
| `conf.local-fn-rejected` | `impl/conformance/17-modules/local_fn_rejected.cb` | ✗ `diag.syntax-error` (static) | a plain function in a block (D-0069) |
| `conf.local-fn-foreign-type-rejected` | `impl/conformance/17-modules/local_fn_foreign_type_rejected.cb` | ✗ `diag.syntax-error` (static) | a function in a block for a module's type (D-0069) |
| `conf.decl-unknown-type` | `impl/conformance/17-modules/decl_unknown_type_rejected.cb` | ✗ `diag.unbound-name` (static) | `int x = 10;` is a declaration (`spec/22` (4), CHG-0081) whose type names nothing |
| `conf.decl-type-parameter` | `impl/conformance/17-modules/decl_type_parameter_ok.cb` | ok | `T x = a;` in a generic function declares `x` of type `T` |

## 15. Syntax (`spec/22`)

| id | Fragment | Outcome | Derivation |
|---|---|---|---|
| `conf.syntax-stray-name` | `impl/conformance/22-surface-syntax/syntax_stray_name_rejected.cb` | ✗ `diag.syntax-error` (static) | `i32 x s= 10;`: expected `;`, found `s` |
| `conf.syntax-missing-semicolon` | `impl/conformance/22-surface-syntax/syntax_missing_semicolon_rejected.cb` | ✗ `diag.syntax-error` (static) | a statement without its `;` |
| `conf.syntax-unclosed-paren` | `impl/conformance/22-surface-syntax/syntax_unclosed_paren_rejected.cb` | ✗ `diag.syntax-error` (static) | a `(` never closed |
| `conf.syntax-unexpected-token` | `impl/conformance/22-surface-syntax/syntax_unexpected_token_rejected.cb` | ✗ `diag.syntax-error` (static) | `)` where an expression must start |
| `conf.syntax-c-array-declaration` | `impl/conformance/22-surface-syntax/syntax_c_array_declaration_rejected.cb` | ✗ `diag.syntax-error` (static) | `i32[4] xs;`: the message names `array<T, N>` (D-0072) |
| `conf.array-length-not-constant` | `impl/conformance/22-surface-syntax/array_length_not_constant_rejected.cb` | ✗ `diag.syntax-error` (static) | `array<i32, k>` with `k` a local variable: not a constant expression (D-0074, D-0126) |
| `conf.adjacent-string-literals` | `impl/conformance/22-surface-syntax/adjacent_string_literals_ok.cb` | ok | adjacent `str-literal`s are one literal, also as a `printf` format and a constant (D-0078) |
| `conf.adjacent-string-literals-inline` | `str s = "ab" "cd"; str_len(s)` | `→ 4` | joined before parsing (D-0078) |
| `conf.capitalized-builtin-type` | `impl/conformance/22-surface-syntax/capitalized_builtin_type_rejected.cb` | ✗ `diag.syntax-error` (static) | `Mutex<i32> m`: the message names `mutex<…>` (CHG-0093) |
| `conf.foreach-borrowed-temporary` | `impl/conformance/14-control-flow/foreach_borrowed_temporary_rejected.cb` | ✗ `diag.borrow-of-non-place` (static) | `foreach (x in &f())`: the message names `foreach (x in f())` (CHG-0093) |
| `conf.join-result-typed` | `impl/conformance/19-concurrency/join_result_typed_ok.cb` | ok | `spawn(f, …) : handle<R>`, `join(h) : R` (CHG-0093) |
| `conf.join-result-type-mismatch` | `impl/conformance/19-concurrency/join_result_type_mismatch_rejected.cb` | ✗ `diag.type-mismatch` (static) | `join(h)` of a `handle<u32>` does not initialize an `i64` |
| `conf.runtime-lock-after-join` | `impl/conformance/19-concurrency/runtime_lock_after_join_ok.cb` | ok; prints what its header gives | 200 rounds of spawn and join, a channel producer every 20th, single-threaded `Vec<String>` work between: the runtime state stays consistent as threads start and end (D-0192: `cbrt` stops locking once only the main thread runs, and takes shared holds for checks) |
| `conf.heaps-meet` | `impl/conformance/19-concurrency/heaps_meet_ok.cb` | ok; prints what its header gives | `rule.conc.spawn`, `rule.conc.channel`, `rule.conc.mutex` (D-0202: `cbrt` keeps each running thread's records in a heap of its own): three threads read one `Vec<String>` of the main thread at once through a shared reference, append under a mutex and send through a channel; then 260 threads are alive at once, more than there are heaps |
| `conf.mutex-bare-guard` | `impl/conformance/19-concurrency/mutex_bare_guard_ok.cb` | ok; prints what its header gives | `rule.conc.lock` (D-0202 phase 4: `cobc` locks and unlocks a mutex whose guard is used only as `*g` or `g.f` without a guard object): four threads with guards of every shape -- such blocks, an early `return` holding the guard, the interior lent to a function, a mutex holding a resource -- release every lock at its block's end, so every count is exact |
| `conf.mutex-bare-guard-reentrant` | `impl/conformance/19-concurrency/mutex_bare_guard_reentrant_rejected.cb` | ✗ `diag.mutex-reentrant-lock` (dynamic) | `[Lock-Reentrant]`: the same mutex reached through two references, locked through each in one block by guards used only as `*g`: still a fault, not a deadlock |
| `conf.spawn-threads-reused` | `impl/conformance/19-concurrency/spawn_threads_reused_ok.cb` | ok; prints what its header gives | `rule.conc.spawn` (D-0202 phase 4: `cbrt` runs a thread on an operating-system thread that finished an earlier one): 500 threads spawned and joined in turn, each moved a `String` and a `Vec`, each starting fresh |
| `conf.conc-race-detected` | `impl/conformance/19-concurrency/conc_race_detected_rejected.cb` | ✗ `diag.aliasing-conflict` (dynamic) | `rule.conc.guarantee` `[Conc-No-Race]` (D-0203): a thread holding an exclusive reference to an element waits; the main thread reads that element (chosen at run time): a fault in every run, however the threads are timed (a reference passed to `spawn` is held from the spawn) |
| `conf.conc-disjoint-elements` | `impl/conformance/19-concurrency/conc_disjoint_elements_ok.cb` | ok; prints what its header gives | `[Conc-No-Race]`, `[Conc-Interleaving]`: as `conf.conc-race-detected`, but another element is read: no conflict, the same result in every run |
| `conf.thread-failure-contained` | `impl/conformance/19-concurrency/thread_failure_contained_ok.cb` | ok; prints what its header gives | `rule.fail.fault-contain` `[Fault-Contain]`, `[Try-Join]` (D-0204): a spawned thread's index fault is contained; `try_join` gives it as a value and the program goes on |
| `conf.thread-failure-join` | `impl/conformance/19-concurrency/thread_failure_join_rejected.cb` | ✗ `diag.index-out-of-bounds` (dynamic) | `[Join-Failed]`: `join` raises the thread's failure again in the joiner |
| `conf.thread-failure-handle-end` | `impl/conformance/19-concurrency/thread_failure_handle_end_rejected.cb` | ✗ `diag.index-out-of-bounds` (dynamic) | `[Handle-Destructor]`, `[Join-Failed]`: a failure nobody took is raised as its handle ends |
| `conf.thread-failure-nested` | `impl/conformance/19-concurrency/thread_failure_nested_ok.cb` | ok; prints what its header gives | `[Handle-Destructor]`: a grandchild's failure raised at its handle's end makes the middle thread fail; the main thread takes it with `try_join` |
| `conf.thread-failure-deep-unwind` | `impl/conformance/19-concurrency/thread_failure_deep_unwind_ok.cb` | ok; prints what its header gives | `[Fault-Contain]`, `[Cancel]`: a fault 200 calls deep unwinds every destructor and cancels the sleeping thread it owns, at once |
| `conf.thread-failures-many` | `impl/conformance/19-concurrency/thread_failures_many_ok.cb` | ok; prints what its header gives | `[Fault-Contain]`: 200 threads, a third faulting, every failure contained and taken |
| `conf.thread-lent-exclusive` | `impl/conformance/19-concurrency/thread_lent_exclusive_not_contained_rejected.cb` | ✗ `diag.index-out-of-bounds` (dynamic) | `contained` (spec/18): a thread lent an exclusive reference is not contained; its fault ends the program despite `try_join` |
| `conf.thread-cancel-waits` | `impl/conformance/19-concurrency/thread_cancel_waits_ok.cb` | ok; prints what its header gives | `rule.conc.cancel` `[Cancelled-Wait]`: a channel wait and a minute's sleep end at once with `diag.thread-cancelled` |
| `conf.thread-cancel-lock` | `impl/conformance/19-concurrency/thread_cancel_lock_ok.cb` | ok; prints what its header gives | `[Cancelled-Wait]`, `[Deadlock]`: a thread waiting for a lock the main thread holds is cancelled and joined; a cancelled thread closes no cycle |
| `conf.thread-cancel-socket` | `impl/conformance/19-concurrency/thread_cancel_socket_ok.cb` | ok; prints what its header gives | `[Cancelled-Wait]`, `rule.stdlib.net`: waits in `accept` and in a read end with `diag.thread-cancelled` |
| `conf.thread-channel-exclusive` | `impl/conformance/19-concurrency/thread_channel_exclusive_not_contained_rejected.cb` | ✗ `diag.index-out-of-bounds` (dynamic) | `contained` (spec/18): a thread that received an exclusive reference through a channel is lent; its fault ends the program despite `try_join` |
| `conf.mutex-poisoned` | `impl/conformance/19-concurrency/mutex_poisoned_rejected.cb` | ✗ `diag.mutex-poisoned` (dynamic) | `[Lock-Poisoned]`: a thread faults holding a lock; the next `lock` of that mutex faults |
| `conf.deadlock-all-waiting` | `impl/conformance/19-concurrency/deadlock_all_waiting_rejected.cb` | ✗ `diag.deadlock` (dynamic) | `[Deadlock]` (b): the main thread joins a thread waiting on a channel nobody sends to |
| `conf.many-threads-owning-frames` | `impl/conformance/19-concurrency/many_threads_owning_frames_ok.cb` | ok; prints what its header gives | `[Spawn]`, `[Join]` (D-0202, D-0204): 300 threads alive at once, more than the runtime's heaps, each working on an owned vector through references: no false conflict |
| `conf.http-keep-alive-post-not-resent` | `impl/conformance/21-standard-library-semantics/http_keep_alive_post_not_resent_ok.cb` | ok; prints what its header gives | `[Http-Keep-Alive]` (D-0195, CHG-0235): a kept connection closed unanswered: a GET is sent again on a fresh connection, a POST is `ConnectionClosed` |
| `conf.result-ok` | `impl/conformance/21-standard-library-semantics/result_ok_ok.cb` | ok; prints what its header gives | `Result::ok` (D-0205): `Some` of the value, `None` for an error |
| `conf.slice-eq` | `impl/conformance/21-standard-library-semantics/slice_eq_ok.cb` | ok; prints what its header gives | `[Slice-Eq]` (D-0205): `Vec`s, an array, `String`s, text bytes; lengths differing |
| `conf.view-from-utf8` | `impl/conformance/21-standard-library-semantics/view_from_utf8_ok.cb` | ok; prints what its header gives | `[View-From-Utf8]` (D-0205): a view of UTF-8 bytes; the first bad byte's offset |
| `conf.udp-try-clone` | `impl/conformance/21-standard-library-semantics/udp_try_clone_ok.cb` | ok; prints what its header gives | `[Udp-Try-Clone]` (D-0205): one handle waits in a thread while the other sends, from the same address |
| `conf.child-kill-tree` | `impl/conformance/21-standard-library-semantics/child_kill_tree_ok.cb` | ok; prints what its header gives | `[Child-Kill-Tree]` (D-0205, Linux): a shell and the two `sleep`s it started all end |
| `conf.frozen-param-held-element` | `impl/conformance/08-alias-validity/frozen_param_held_element_rejected.cb` | ✗ `diag.aliasing-conflict` (dynamic) | `[Read-Conflict]` (CHG-0237): `&mut v[2]` held, a function reads `v` through a shared parameter: the checked body runs and element 2's read faults after the reads before it |
| `conf.frozen-param-released` | `impl/conformance/08-alias-validity/frozen_param_released_ok.cb` | ok; prints what its header gives | as above with the reference ended first: every read succeeds |
| `conf.frozen-param-held-field` | `impl/conformance/08-alias-validity/frozen_param_held_field_rejected.cb` | ✗ `diag.aliasing-conflict` (dynamic) | `[Borrow]` (CHG-0237): `&mut a.balance` held; the shared borrow of `a` made for a call to a function reading its fields clashes with it, before the function runs |
| `conf.frozen-param-threads` | `impl/conformance/08-alias-validity/frozen_param_threads_ok.cb` | ok; prints what its header gives | `[Spawn]`, `[Read]` (CHG-0237): four threads sum bands of one vector through shared parameters, rows handed to a reader: the one-thread total |
| `conf.thread-is-finished` | `impl/conformance/21-standard-library-semantics/thread_is_finished_ok.cb` | ok; prints what its header gives | `[Is-Finished]`, `[Try-Join]`, `[Failure-Text]`: looking without waiting; a failure's text names the diagnostic and the location |
| `conf.slice-parts` | `impl/conformance/21-standard-library-semantics/slice_parts_ok.cb` | ok; prints what its header gives | `[Slice-Parts]`: part lengths for 3, 12 and 0 parts; three threads write their parts at once |
| `conf.closure-capture-keeps-its-type` | `impl/conformance/15-function-semantics/closure_capture_keeps_its_type_ok.cb` | ok | a captured name has its binding's type in the body (CHG-0093) |
| `conf.literal-argument-typed-by-other-argument` | `impl/conformance/15-function-semantics/literal_arguments_typed_by_the_call_ok.cb` | ok | `pick(true, 0, x)` with `i64 x`: `T = i64` from `x`, and the `0` is an `i64` (D-0079) |
| `conf.literal-argument-typed-by-expected-type` | `fn middle<T>(T a, T b, T c) : T { b }` `u16 w = middle(1, 60000, 3); w` | `→ 60000` | only literals: the expected `u16` fixes `T` before they are typed (D-0079) |
| `conf.literal-arguments-only-default` | `fn pick<T>(bool f, T a, T b) : T { if (f) { a } else { b } }` `auto k = pick(false, 3, 4); i32 k32 = k; k32` | `→ 4` | nothing else fixes `T`: the literals' default `i32` (D-0079) |
| `conf.literal-argument-out-of-range-for-inferred-type` | `impl/conformance/15-function-semantics/literal_argument_out_of_range_for_inferred_type_rejected.cb` | ✗ `diag.literal-out-of-range` (static) | `x : u8` fixes `T`; 300 is not a `u8` |
| `conf.overwrite-destroys-old-value` | `impl/conformance/21-standard-library-semantics/overwrite_destroys_old_value_ok.cb` | ok | `overwrite(&mut a, b)` destroys `a`'s old value during the call (D-0080) |
| `conf.closure-result-type-written` | `impl/conformance/15-function-semantics/closure_result_type_written_ok.cb` | ok | `[](i64 a) : i64 { 0 }`: the body is checked against the written type (D-0081) |
| `conf.closure-result-type-conflict` | `impl/conformance/15-function-semantics/closure_result_type_conflict_rejected.cb` | ✗ `diag.type-mismatch` (static) | a written `: i32` where `fn(…) : bool` is expected (D-0081) |
| `conf.unwrap-some` | `impl/conformance/21-standard-library-semantics/unwrap_expect_clone_ok.cb` | ok | `unwrap`/`expect` give the payload; `Vec::clone`, `Vec::clone_by` (D-0082); also `conf.vec-clone` |
| `conf.vec-clone` | `impl/conformance/21-standard-library-semantics/unwrap_expect_clone_ok.cb` | ok | a clone is independent of its original (D-0082) |
| `conf.unwrap-none-faults` | `impl/conformance/21-standard-library-semantics/unwrap_none_faults.cb` | ✗ `diag.unwrap-failed` (dynamic) | `Option::unwrap` of `None` (D-0082) |
| `conf.expect-message` | `impl/conformance/21-standard-library-semantics/expect_message_faults.cb` | ✗ `diag.unwrap-failed` (dynamic) | `Result::expect` of `Err`, with the program's message (D-0082) |
| `conf.vec-clone-resource-rejected` | `impl/conformance/21-standard-library-semantics/vec_clone_resource_rejected.cb` | ✗ `diag.type-mismatch` (static) | `Vec::clone` of a `Vec<Handle>`, `Handle` a `resource` type with no `clone`: `[Bound-Unsatisfied]` (D-0082; `T: clone` since D-0116) |
| `conf.fault-unregistered-name` | `impl/conformance/21-standard-library-semantics/fault_unregistered_name_rejected.cb` | ✗ `diag.unbound-name` (static) | `fault(assertion_failed)`: no such run-time diagnostic (D-0083) |
| `conf.fault-with-message` | `impl/conformance/21-standard-library-semantics/fault_with_message.cb` | ✗ `diag.index-out-of-bounds` (dynamic) | a program's own container raises the standard fault with a message (D-0083) |

## Change Log

- 3.201.0 — `CHG-0245` (D-0207): three derivations cite rules by their
  current labels (`[Index-Checked]`, `[T-Deref]`, `[Hash]`), found by
  `impl/tools/speccheck.py`. No case changed.
- 3.200.0 — `CHG-0239`, `CHG-0240`, `CHG-0243` (D-0206): `conf.impl-defined-width-observed`, `conf.ref-byte-image-width`; `conf.safe-client-vec-trace`, `conf.safe-client-string-trace`, `conf.safe-client-box-trace`, `conf.safe-client-rc-trace`, `conf.safe-client-channel-trace`.
- 3.199.0 — `CHG-0237`, `CHG-0238` (D-0205): `conf.frozen-param-held-element`, `conf.frozen-param-released`, `conf.frozen-param-held-field`, `conf.frozen-param-threads`; `conf.result-ok`, `conf.slice-eq`, `conf.view-from-utf8`, `conf.udp-try-clone`, `conf.child-kill-tree`.
- 3.198.2 — `CHG-0235`: `conf.http-keep-alive-post-not-resent`.
- 3.198.1 — `CHG-0234` (D-0204): `conf.thread-channel-exclusive`;
  `conf.deadlock-all-waiting`'s comment (one outcome).
- 3.198.0 — `CHG-0233` (D-0204): `conf.generic-literal-argument-checked` (a checker fix found on the way), `conf.thread-failure-contained`, `conf.thread-failure-join`, `conf.thread-failure-handle-end`, `conf.thread-failure-nested`, `conf.thread-failure-deep-unwind`, `conf.thread-failures-many`, `conf.thread-lent-exclusive`, `conf.thread-cancel-waits`, `conf.thread-cancel-lock`, `conf.thread-cancel-socket`, `conf.mutex-poisoned`, `conf.deadlock-all-waiting`, `conf.many-threads-owning-frames`, `conf.thread-is-finished`, `conf.slice-parts`.
- 3.197.0 — `CHG-0232` (D-0203): `conf.conc-race-detected`, `conf.conc-disjoint-elements`.
- 3.196.0 — `CHG-0231` (D-0202 phase 4): `conf.mutex-bare-guard`, `conf.mutex-bare-guard-reentrant`, `conf.spawn-threads-reused` (no rule changed).
- 3.195.0 — `CHG-0230` (D-0202): `conf.heaps-meet` (no rule changed).
- 3.194.0 — `CHG-0229` (D-0201): `conf.file-write-buffer-visible`.
- 3.193.0 — `CHG-0228` (D-0200): `conf.objectless-lend-and-move`, `conf.objectless-moved-then-used`, `conf.raw-form-move-to-call`, `conf.fused-text-loops`, `conf.elem-ref-local`, `conf.elem-ref-local-conflict`, `conf.map-get-unwrap-read`, `conf.map-get-unwrap-none`, `conf.file-read-line-utf8-error`, `conf.spawn-in-raw-form`, `conf.binder-moved-into-literal` (no rule changed).
- 3.192.0 — `CHG-0227` (D-0199): `conf.text-search-edges`, `conf.text-append-view-of-self`, `conf.text-append-empty-view-of-self`, `conf.text-substring-not-char-boundary`, `conf.option-unwrap-ref-none` (no rule changed).
- 3.191.0 — `CHG-0226` (D-0198): `conf.std-encoding-hex-base64-paths`, `conf.std-text-has-no-hex`: hexadecimal and base64 moved to `std::encoding`.
- 3.190.0 — `CHG-0225` (D-0197): `conf.std-encoding-json-paths`: JSON moved to `std::encoding::json`.
- 3.189.0 — `CHG-0224` (D-0196): `conf.objectless-strings-by-binding`, `conf.string-builders-raw-return`.
- 3.188.0 — `CHG-0223` (D-0195): `conf.process-child-pipes`, `conf.http-keep-alive`.
- 3.187.0 — `CHG-0222` (D-0194): `conf.quiet-replace`, `conf.quiet-replace-ok`, `conf.overwrite-fault-destroys-pending`, `conf.quiet-not-channel`; `conf.reassign-maybe-live-dynamic` and `conf.overwrite-some-of-resource-option-dynamic` now use a `Channel`; `conf.overwrite-live-resource-rejected`, `conf.overwrite-field-of-live-object`, `conf.overwrite-through-exclusive-reference`, `conf.overwrite-optional-field-dynamic` now use a type declared `resource` (a `String` or `Vec<i32>` is quiet).
- 3.186.0 — `CHG-0221` (D-0193): `conf.closure-names-across-functions`, `conf.closure-captures-closure`, `conf.closure-captured-owner-destructors`, `conf.closure-combinator-loop`, `conf.text-split-trim-case`, `conf.vec-swap-remove`, `conf.priority-queue-order`, `conf.text-chars-count`, `conf.text-parse-int-edges`.
- 3.185.0 — `CHG-0220` (D-0192): `conf.vec-resource-elements`, `conf.vec-string-search`, `conf.vec-resource-predicates`, `conf.vec-string-clone`, `conf.vec-from-fn-literal`, `conf.vec-capturing-closure-values`, `conf.vec-search-key-places`, `conf.dual-body-bare-call`, `conf.runtime-lock-after-join` (no rule changed).
- 3.184.0 — `CHG-0219` (D-0191): `conf.vec-predicate-values`, `conf.vec-predicate-value-keeps-element`, `conf.vec-retain-fn-value-held-element`, `conf.vec-binary-search-by-held-element` (no rule changed).
- 3.183.0 — `CHG-0218` (D-0190): `conf.vec-struct-in-vec-loops`, `conf.vec-struct-in-vec-slice-held`, `conf.vec-struct-in-vec-inner-held`, `conf.vec-foreach-over-parameter`, `conf.vec-capturing-predicates`, `conf.vec-capturing-predicate-held-element` (no rule changed).
- 3.182.0 — `CHG-0217` (D-0189): `conf.vec-struct-field-loops`, `conf.vec-struct-field-held-element`, `conf.vec-local-calls`, `conf.vec-search-keys`, `conf.vec-sort-by-through-parameter`, `conf.vec-predicate-literals`, `conf.vec-retain-held-element`, `conf.vec-dedup-held-element`, `conf.vec-position-held-element` (no rule changed).
- 3.181.0 — `conf.slice-read-beside-held-element`, `conf.slice-write-over-held-shared-element`, `conf.vec-from-slice-held-element`, `conf.vec-extend-from-held-element`: an access through a slice meets the paths of the element objects in its range (`cobc` missed these until 2026-10-08; no rule changed).
- 3.180.0 — `conf.vec-element-moves`, `conf.vec-swap-held-element`, `conf.vec-swap-beside-held-element`, `conf.vec-insert-held-element`, `conf.vec-index-of-held-element`, `conf.vec-clone-held-element`, `conf.vec-reverse-held-element`: what `cobc`'s native `Vec` bodies must keep, an element with a live object among them (no rule changed).
- 3.179.0 — `conf.vec-filled-element-kinds`: the element kinds `cobc`'s native `Vec::filled` must fill as the body would (no rule changed).
- 3.178.0 — `CHG-0216` (D-0188): `conf.state-and-input-arguments-disjoint`, `conf.state-and-own-field-input-rejected`, `conf.state-and-field-of-state-rejected`, `conf.overlapping-slice-arguments-rejected`: the argument shapes a dual body's entry test must tell apart (no rule changed). The header's version, left at 3.175.0 by 3.176.0 and 3.177.0, is corrected.
- 3.177.0 — `CHG-0215` (D-0187): `conf.std-nested-submodule-path`, `conf.std-extensions-empty`: nested `std` submodules and the implementation-defined `std::extensions`.
- 3.176.0 — `conf.vec-extend-from-alias-rejected`, `conf.string-append-own-view-rejected`, `conf.view-sub-not-char-boundary`: shapes `cobc`'s native bodies must keep (found 2026-10-08 when a native `Vec::extend_from` and a shortcut past `String::append`'s binding lost the clash; no rule changed).
- 3.175.0 — `conf.match-arm-temporary-borrowing-binder`: the interpreter kept an expression arm's temporaries to the enclosing statement's end, past the arm's binding (a divergence from `cobc`, found 2026-10-07; no rule changed).
- 3.174.0 — `CHG-0209`, `CHG-0210`, `CHG-0211`, `CHG-0212`, `CHG-0213`, `CHG-0214` (D-0181 to D-0186): `conf.text-strip-prefix-suffix`, `conf.text-lines-words`, `conf.text-code-points`, `conf.parse-bool`, `conf.vec-retain`, `conf.vec-dedup`, `conf.vec-sort-floats`, `conf.math-clamp`, `conf.math-nan-finite`, `conf.math-gcd`, `conf.float-inverse-trig`, `conf.rng-shuffle`, `conf.uuid-v4`, `conf.time-iso-ms`, `conf.time-http-date`, `conf.file-create-new`, `conf.file-flush-position`, `conf.env-current-exe`, `conf.env-hostname`, `conf.process-id`, `conf.child-terminate`, `conf.child-read-error-line`, `conf.channel-try-recv`, `conf.channel-recv-timeout`, `conf.cpu-count`, `conf.tcp-try-clone`, `conf.tcp-keepalive`, `conf.udp-connect`, `conf.sha1`, `conf.hmac-sha1`, `conf.json-parse`, `conf.json-text`, `conf.json-tree`, `conf.json-errors`, `conf.deflate-inflate`, `conf.zlib`, `conf.gzip`, `conf.adler32`, `conf.inflate-errors`, `conf.args-flags-options`, `conf.args-finish`, `conf.args-help`, `conf.postgres-typed-cells`, `conf.tls-server`, `conf.tls-read-line`, `conf.tls-peer-certificate`, `conf.https-server`, `conf.http-forms`, `conf.http-content-encoding`.
- 3.173.0 — `CHG-0208` (D-0180): `conf.scram-sha256-vectors`, `conf.postgres-url`, `conf.postgres-values`, `conf.postgres-loopback`.
- 3.172.0 — `CHG-0207` (D-0179): `conf.view-arg-not-pending`; `conf.index-borrow-whole-element` (coby had checked an index borrow at the field reached, not the element); `conf.fault-unwind-first-fault` (coby had reported a destructor's fault raised during the unwind).
- 3.171.0 — `conf.eval-order-reads`, `conf.elem-write-rhs-grows`: two orders the implementations had diverged on (coby read an argument late; cobc kept an element's address across a call that moved it).
- 3.170.0 — `CHG-0206` (D-0178): `conf.stdout-buffer-order`, `conf.flush-stdout`, `conf.stdout-buffer-boundary`, `conf.stdout-buffer-child`, `conf.stdout-flush-private`.
- 3.169.0 — `CHG-0205` (D-0177): `conf.std-bytes-subjects`, `conf.std-text-has-no-bytes`.
- 3.168.0 — `CHG-0204` (D-0176): `conf.std-subjects-fs-env`, `conf.std-io-has-no-files`.
- 3.167.0 — `CHG-0203` (D-0175): `conf.while-true-never`, `conf.while-true-break-needs-value`, `conf.while-true-literal-only`.
- 3.166.0 — `conf.fault-with-live-thread` (spec/18 1.5.3, spec/19 1.8.1 clarification).
- 3.165.0 — `CHG-0202` (D-0174): `conf.http-server-keep-alive`, `conf.http-server-no-body`, `conf.http-server-refusals`.
- 3.164.0 — `CHG-0201` (D-0173): `conf.url-parse`, `conf.percent-encoding`, `conf.http-headers`, `conf.http-get-post`, `conf.http-redirects`, `conf.http-chunked`, `conf.http-until-close`, `conf.http-client-refusals`.
- 3.163.0 — `CHG-0199` (D-0171): `conf.key-formats`, `conf.sign-verify`, `conf.x509-ed25519`.
- 3.162.0 — `CHG-0198` (D-0170): `conf.blake2b-vectors`, `conf.pbkdf2-vectors`, `conf.argon2id-vectors`, `conf.password-hash`, `conf.kdf-parameters`.
- 3.161.0 — `CHG-0197` (D-0169): `conf.rsa-sign-vectors`, `conf.rsa-private-key-refusals`, `conf.rsa-generate-size`, `conf.bigint-mod-inverse`.
- 3.160.0 — `CHG-0196` (D-0168): `conf.ecdsa-sign-vectors`, `conf.ec-key-refusals`, `conf.ecdsa-sign-p384`, `conf.ecdh-p384`, `conf.ec-key-length`.
- 3.159.0 — `CHG-0195` (D-0167): `conf.ed25519-vectors`, `conf.ed25519-refusals`.
- 3.158.0 — `CHG-0194` (D-0166): `conf.hmac-generic`, `conf.hkdf-generic`.
- 3.157.0 — reserved: no rows (`CHG-0200`, D-0172, changes the TLS client, which the suite reaches only through the stress harness).
- 3.156.0 — `CHG-0193` (D-0165): `conf.aes-vectors`, `conf.aes-gcm-vectors`, `conf.p256-ecdh-vectors`, `conf.p256-ecdh-refusals`, `conf.aes-key-length`.
- 3.155.0 — `conf.ecdsa-p384-tamper`, `conf.rsa-pss-3072`; `conf.x509-refusals` in its own case: the heavy crypto cases split so each stays within the CPU limit under the interpreter's debug build.
- 3.154.0 — `CHG-0192` (D-0164): `conf.text-mixed-eq`, `conf.text-mixed-order-rejected`.
- 3.153.0 — `CHG-0191` (D-0163): `conf.stringview-as-bytes`.
- 3.152.0 — `CHG-0190` (D-0162): `conf.str-literal-generic-string`.
- 3.151.0 — `CHG-0189` (D-0161): `conf.hkdf-expand-label`, `conf.tls-not-tls`, `conf.tls-closed`.
- 3.150.0 — `CHG-0188` (D-0160): `conf.x509-parse`, `conf.x509-chain`, `conf.x509-refusals`.
- 3.149.0 — `CHG-0187` (D-0159): `conf.ecdsa-p256`, `conf.ecdsa-p384`.
- 3.148.0 — `CHG-0186` (D-0158): `conf.rsa-pkcs1v15`, `conf.rsa-pss`.
- 3.147.0 — `CHG-0185` (D-0157): `conf.bigint-arithmetic`, `conf.bigint-text`, `conf.bigint-sub-underflow`.
- 3.146.0 — `CHG-0184` (D-0156): `conf.sha512-vectors`, `conf.sha384-vectors`.
- 3.145.0 — `CHG-0183` (D-0155): `conf.str-literal-in-arms`.
- 3.144.0 — `CHG-0182` (D-0154): `conf.x25519-vectors`, `conf.x25519-exchange`, `conf.x25519-small-order`.
- 3.143.0 — `CHG-0181` (D-0153): `conf.chacha20-vector`, `conf.poly1305-vector`, `conf.aead-vector`, `conf.aead-tamper`, `conf.crypto-length`.
- 3.142.0 — `CHG-0180` (D-0152): `conf.hkdf-vectors`, `conf.hkdf-bound`, `conf.hkdf-too-long`; the two `exit` rows state their status as `ok, exit status n`.
- 3.141.0 — `CHG-0179` (D-0151): `conf.sha256-vectors`, `conf.sha256-streaming`, `conf.hmac-sha256-vectors`, `conf.digest-eq`, `conf.hex`, `conf.base64`.
- 3.140.0 — `CHG-0178` (D-0150): `conf.exit-runs-destructors`, `conf.exit-from-thread`.
- 3.139.0 — `CHG-0177` (D-0149): `conf.str-literal-as-string`, `conf.string-of-str-binding-rejected`.
- 3.138.0 — `CHG-0175` (D-0147): `conf.local-offset-range`, `conf.to-local-consistent`.
- 3.137.0 — A review of D-0143/D-0144: `conf.net-udp-truncate` (a datagram longer than `max`), `conf.process-closed-pipe` (a write to a child that has ended is `Err(Io)`).
- 3.136.0 — `CHG-0174` (D-0146): `conf.extern-without-unsafe-rejected`, `conf.unsafe-extern-code-rejected`; every fragment declaring a foreign function written `unsafe extern fn`.
- 3.135.0 — `CHG-0173` (D-0145): `conf.read-all`, `conf.read-all-bytes`, `conf.read-all-not-utf8`.
- 3.134.0 — `CHG-0172` (D-0144): `conf.net-bind-port`, `conf.net-echo`, `conf.net-read-exact`, `conf.net-shutdown-read-zero`, `conf.net-addr-in-use`, `conf.net-accept-timeout`, `conf.net-refused`, `conf.net-addr-parse`, `conf.net-addr-reject`, `conf.net-addr-constants`, `conf.net-udp`, `conf.net-resolve-localhost`, `conf.std-variant-names-unique`.
- 3.133.0 — `CHG-0171` (D-0143): `conf.process-output`, `conf.process-input`, `conf.process-status`, `conf.process-signal-status`, `conf.process-not-found`, `conf.process-dir-env`, `conf.process-spawn-lines`, `conf.process-kill`, `conf.process-try-wait`, `conf.interrupt-flag`.
- 3.132.0 — `CHG-0170` (D-0142): `conf.os-random-u64`, `conf.os-random-bytes`, `conf.os-random-empty`, `conf.rng-from-os`.
- 3.131.0 — `CHG-0169` (D-0141): `conf.path-join`, `conf.path-parts`, `conf.path-normalize`, `conf.file-info`, `conf.copy-file`, `conf.dir-all`, `conf.set-current-dir`, `conf.path-canonical`, `conf.temp-home-dir`; the `platform:` case header.
- 3.130.0 — `CHG-0168` (D-0140): `conf.datetime-from-unix`, `conf.datetime-to-unix-limits`, `conf.datetime-weekday`, `conf.datetime-iso`, `conf.datetime-new`, `conf.datetime-sort`, `conf.unix-ms`, `conf.datetime-invalid`.
- 3.129.0 — `CHG-0167` (D-0139): `conf.string-contains`, `conf.string-replace`, `conf.string-join`.
- 3.128.0 — `CHG-0166` (D-0138): `conf.hashset-union`, `conf.hashset-intersection`, `conf.hashset-difference`.
- 3.127.0 — `CHG-0165` (D-0137): `conf.queue-fifo`, `conf.queue-empty`, `conf.queue-both-ends`, `conf.queue-destroy`, `conf.queue-front-borrows`, `conf.priority-queue-key-order`, `conf.priority-queue-by`, `conf.priority-queue-by-any-type`, `conf.priority-queue-ties`, `conf.priority-queue-destroy`, `conf.priority-queue-not-key`, `conf.priority-queue-generic-key-checked`.
- 3.126.0 — `CHG-0163`, `CHG-0164` (D-0136): `conf.reexport-module`, `conf.reexport-item`, `conf.reexport-item-only`, `conf.reexport-chain`, `conf.reexport-same-item-twice`, `conf.reexport-conflict-rejected`, `conf.reexport-cycle`, `conf.reexport-private-not-passed`, `conf.reexport-unbound-rejected`, `conf.std-submodule-path`, `conf.std-import-one-subject`, `conf.std-import-one-subject-only`, `conf.std-message-short-path`, `conf.float-fn-needs-import`, `conf.float-fn-name-own-item`, `conf.module-import-no-submodule`.
- 3.125.0 — `CHG-0162` (D-0135): `conf.view-of-temporary-argument`, `conf.view-of-call-result-argument`, `conf.view-of-temporary-outside-argument-rejected`, `conf.view-of-temporary-kept-faults`.
- 3.124.0 — `CHG-0157`–`CHG-0161` (D-0134): `conf.str-literal-as-view`, `conf.view-of-str`, `conf.view-of-str-binding-rejected`, `conf.string-needle`, `conf.read-file-literal-path`, `conf.hashset-clear`, `conf.hashset-remove-keeps-order`, `conf.view-in-thread`; renamed `conf.hashmap-remove-swaps` → `conf.hashmap-swap-remove`, `conf.hashmap-remove-ordered` → `conf.hashmap-remove-keeps-order`, `conf.write-err-private` → `conf.stderr-write-private`; every row naming a renamed function or a path rewritten.
- 3.123.0 — `CHG-0156` (D-0133): `conf.ref-holding-binding-ends-at-last-use`,
  `conf.ref-holding-binding-used-later-rejected`.
- 3.122.0 — `CHG-0155` (D-0132): `conf.local-bitstruct`.
- 3.121.0 — `CHG-0154` (D-0131): `conf.shadowing` shadows from a nested
  block; `conf.drop-then-shadow-ok` becomes `conf.drop-then-redeclare-rejected`;
  new `conf.redeclared-local-in-block-rejected`, `conf.redeclared-param-rejected`,
  `conf.duplicate-param-rejected`, `conf.redeclared-foreach-name-rejected`,
  `conf.duplicate-destructure-binder-rejected`, and five file cases
  (`conf.redeclare-allowed-forms`, `conf.redeclared-local-rejected`,
  `conf.redeclared-for-variable-rejected`, `conf.redeclared-arm-binder-rejected`,
  `conf.redeclared-local-const-rejected`).
- 3.120.0 — `CHG-0152` (D-0129): `conf.vec-reserve`, `conf.vec-reserve-keeps`,
  `conf.vec-reserve-overflow`, `conf.vec-reserve-stale`, `conf.string-reserve`.
  `CHG-0153` (D-0130): `conf.propagate-option`,
  `conf.propagate-option-in-result-rejected`.
- 3.119.0 — `CHG-0151` (D-0128): `conf.string-chars`, `conf.stringview-chars`,
  `conf.string-chars-empty`, `conf.string-chars-borrows`.
- 3.118.1 — `conf.array-length-not-constant` now names a local variable as
  the length: D-0126 made its former `K = N + 1` a constant expression
  (`CHG-0149`; no rule changed).
- 3.118.0 — `CHG-0150` (D-0127): `conf.match-text-str`, `conf.match-text-literals`,
  `conf.match-text-non-exhaustive-rejected`, `conf.match-text-literal-type-rejected`,
  `conf.match-text-on-integer-rejected`, `conf.match-text-reference-rejected`,
  `conf.match-text-duplicate-rejected`, `conf.if-pattern-text`.
- 3.117.0 — `CHG-0144`–`CHG-0149` (D-0121–D-0126): `conf.volatile-read-write`,
  `conf.volatile-outside-unsafe-rejected`, `conf.volatile-resource-rejected`,
  `conf.checked-narrow-some`, `conf.checked-narrow-none`, `conf.bit-counts`,
  `conf.bit-rotate`, `conf.bit-count-non-integer-rejected`, `conf.sleep-at-least`,
  `conf.weak-upgrade-while-alive`, `conf.weak-upgrade-after-end`,
  `conf.weak-keeps-box`, `conf.length-constant-expression`,
  `conf.length-later-constant`, `conf.length-module-expression`,
  `conf.length-not-constant-rejected`, `conf.length-negative-rejected`.
- 3.116.0 — `CHG-0143` (D-0120): `conf.rng-check-values`, `conf.rng-below-range`,
  `conf.rng-below-zero-faults`, `conf.rng-unit-range`.
- 3.115.0 — `CHG-0142` (D-0119): `conf.bytes-le-be`, `conf.bytes-signed-roundtrip`,
  `conf.bytes-short-buffer-faults`, `conf.crc32-check`, `conf.crc32-empty`.
- 3.114.0 — `CHG-0141` (D-0118): `conf.bitstruct-fields`,
  `conf.bitstruct-bits-roundtrip`, `conf.bitstruct-raw-image`,
  `conf.bitstruct-through-reference`, `conf.bitstruct-key`,
  `conf.bitstruct-literal-overflow-rejected`,
  `conf.bitstruct-write-overflow-faults`, `conf.bitstruct-field-borrow-rejected`,
  `conf.bitstruct-fill-rejected`, `conf.bitstruct-destructure-rejected`.
- 3.113.0 — `CHG-0140` (D-0117): `conf.file-error-text`, `conf.parse-error-text`.
- 3.112.0 — `CHG-0139` (D-0116): `conf.clone-plain`, `conf.clone-derived-struct`,
  `conf.clone-derived-enum`, `conf.clone-generic-struct`,
  `conf.clone-std-containers`, `conf.clone-declared-called`,
  `conf.clone-bound-generic-fn`, `conf.clone-resource-without-clone-rejected`,
  `conf.clone-field-not-clone-rejected`,
  `conf.clone-unbounded-parameter-rejected`, `conf.clone-mutex-rejected`;
  `conf.vec-clone-resource-rejected`, `conf.vec-from-slice-resource-rejected`,
  `conf.vec-filled-resource-rejected`, `conf.vec-extend-from-resource-rejected`
  use a `resource` type without a `clone` (a `String` is `clone` now).
- 3.111.0 — `CHG-0138` (D-0115): `conf.if-pattern-some`,
  `conf.if-pattern-else-value`, `conf.if-pattern-nested`,
  `conf.if-pattern-literal`, `conf.if-pattern-qualified-variant`,
  `conf.if-pattern-binder-scope-rejected`,
  `conf.if-assignment-still-rejected`, `conf.while-pattern-pop-moves`,
  `conf.while-pattern-break-continue`.
- 3.110.0 — `CHG-0137` (D-0114): `conf.option-ok-or`,
  `conf.option-ok-or-propagates`, `conf.option-ok-or-error-unused-destroyed`.
- 3.109.0 — `CHG-0135` (D-0113): `conf.string-view-functions`,
  `conf.string-split`, `conf.map-lookup-by-str`, `conf.map-remove-str`,
  `conf.set-lookup-by-str`, `conf.map-get-str-non-string-key-rejected`;
  `CHG-0136`: `conf.generic-concrete-argument-checked`; `conf.vec-position`
  binds its closure first (the row assembler takes no closure in a
  trailing `match`).
- 3.108.0 — `CHG-0133` (D-0112): `conf.vec-filled`,
  `conf.vec-filled-resource-rejected`, `conf.vec-from-fn`,
  `conf.vec-append`, `conf.vec-extend-from`,
  `conf.vec-extend-from-resource-rejected`, `conf.vec-position`,
  `conf.vec-index-of`, `conf.vec-contains`,
  `conf.vec-contains-not-eq-rejected`, `conf.vec-reverse`; `CHG-0134`:
  `conf.closure-compares-captured-string`,
  `conf.slice-argument-held-across-push-rejected`.
- 3.107.0 — `CHG-0131` (D-0111): `conf.ref-binding-ends-at-last-use`,
  `conf.ref-binding-used-later-rejected`,
  `conf.ref-binding-used-in-loop-rejected`; seven rows whose reference
  was not used after the conflict now use it after
  (`conf.destroy-while-borrowed-rejected`,
  `conf.move-while-borrowed-rejected`,
  `conf.owner-write-while-shared-rejected`,
  `conf.owner-read-while-exclusive-rejected`,
  `conf.shared-then-exclusive-rejected`,
  `conf.projection-write-while-borrowed-rejected`,
  `conf.slice-push-while-borrowed-rejected`), as do six file cases.
- 3.106.0 — `CHG-0130`: `conf.first-error-in-source-order-rejected`,
  `conf.variant-temp-borrow-argument`, `conf.variant-borrow-outside-argument-rejected`.
- 3.105.0 — `CHG-0129` (D-0110): `conf.hashmap-struct-key`,
  `conf.hashmap-enum-key`, `conf.sort-struct-key`,
  `conf.hashmap-key-float-field-rejected`; `conf.hashmap-struct-key-rejected`
  now uses a struct with an `f32` field.
- 3.104.0 — `CHG-0128` (D-0109): `conf.match-through-ref-payload`,
  `conf.match-through-ref-literal`,
  `conf.match-through-ref-binder-borrows-rejected`,
  `conf.match-through-ref-non-exhaustive-rejected`.
- 3.103.0 — `CHG-0127` (D-0108): `conf.string-compare-operators` (was
  `conf.string-eq-operator-rejected`), `conf.order-bool` (was
  `conf.order-bool-rejected`), `conf.str-ordering` (was
  `conf.str-ordering-rejected`), `conf.view-ordering`,
  `conf.string-compare-in-place`,
  `conf.string-compare-while-exclusive-rejected`,
  `conf.bound-ordered-text`; `conf.bound-unsatisfied-rejected` uses
  `Option<i32>`.
- 3.102.0 — `CHG-0126` (D-0107): `conf.argument-borrow-then-shared-argument`,
  `conf.two-exclusive-arguments-fault-at-use`.
- 3.101.0 — `CHG-0125` (D-0106): `conf.enum-codes`, `conf.enum-from-code`,
  `conf.enum-code-duplicate-rejected`, `conf.enum-code-without-type-rejected`.
- 3.100.0 — `CHG-0124` (D-0105): `conf.array-length-qualified-constant`,
  `conf.array-length-qualified-private-rejected`.
- 3.99.0 — `CHG-0123` (D-0104): `conf.destructure-rest`,
  `conf.destructure-rename`, `conf.destructure-two-of-one-type`,
  `conf.destructure-missing-without-rest-rejected`,
  `conf.destructure-rest-destroys-at-block-end`.
- 3.98.0 — `CHG-0122` (D-0103): `conf.temp-field-borrow-argument`,
  `conf.temp-array-slice-argument`, `conf.temp-field-slice-argument`,
  `conf.move-out-of-temporary-field` (was `conf.move-out-of-temporary-field-rejected`),
  `conf.move-out-of-temporary-field-destructor-rejected`,
  `conf.temp-field-move-ends-rest`.
- 3.97.0 — `CHG-0121` (D-0102): `conf.byte-literal-u32-context`,
  `conf.byte-literal-default-u8`, `conf.byte-literal-signed-context-rejected`.
- 3.96.0 — `CHG-0120` (D-0101): `conf.float-transcendental`,
  `conf.float-transcendental-mixed-rejected`, `conf.ascii-helpers`,
  `conf.string-to-ascii-case`.
- 3.95.0 — `CHG-0119` (D-0100): `conf.format-star-width`,
  `conf.format-star-precision`, `conf.format-star-not-usize-rejected`,
  `conf.format-enum-name`.
- 3.94.0 — `CHG-0118` (D-0099): `conf.result-discarded-rejected`,
  `conf.result-discarded-void-rejected`, `conf.discard-underscore`,
  `conf.discard-underscore-destroys-now`.
- 3.93.0 — `CHG-0117` (D-0098): `conf.trailing-comma-call`,
  `conf.trailing-comma-params`, `conf.trailing-comma-empty-call-rejected`.
- 3.92.0 — `CHG-0116`: `conf.closure-calls-borrowed-capture`,
  `conf.closure-borrowed-capture-operations`,
  `conf.qualified-variant-clash`, `conf.map-err-resource-error`,
  `conf.return-bare-in-arm`, `conf.float-rem-rejected`,
  `conf.float-bitand-rejected`, `conf.bool-arith-rejected`,
  `conf.compare-resource-results-rejected`, `conf.logic-int-rejected`,
  `conf.neg-bool-rejected`, `conf.bitnot-bool-rejected`,
  `conf.order-bool-rejected`, `conf.index-string-rejected`,
  `conf.shift-float-rejected`, `conf.shift-amount-not-u32-rejected`,
  `conf.return-value-in-void-rejected`,
  `conf.return-bare-in-value-fn-rejected`, `conf.unit-equality`,
  `conf.struct-field-type-rejected`, `conf.struct-field-twice-rejected`,
  `conf.variant-payload-type-rejected`,
  `conf.array-element-types-rejected`,
  `conf.auto-untyped-variant-rejected`, `conf.assign-fn-item-rejected`,
  `conf.if-without-else-value-rejected`,
  `conf.private-enum-variant-qualified-rejected`,
  `conf.foreach-over-map-err-propagate`,
  `conf.generic-struct-lit-field-shape-rejected`,
  `conf.tail-recursion-stack-exhausted`,
  `conf.narrow-constant-static`, `conf.narrow-in-const-static`,
  `conf.to-int-constant-static` — the implementations enforce rules the
  specification already states (round-6 findings).
- 3.91.0 — `CHG-0113`: `conf.closure-borrow-escapes-to-caller`,
  `conf.closure-borrow-escapes-block`, `conf.closure-move-escape-ok` — a
  closure without `move` stores borrows of its captures for
  `[Ref-Escape-Rejected]`; `CHG-0114`: `conf.destroy-temporary-while-viewed`,
  `conf.destroy-local-while-viewed-outside`, `conf.spawn-closure-capturing-reference` — destruction at a statement's
  or block's end is `solitary`-checked.
- 3.90.0 — `CHG-0112` (D-0097): `conf.foreach-range`,
  `conf.foreach-range-position`, `conf.foreach-range-literal-takes-type`,
  `conf.foreach-range-empty`, `conf.foreach-range-output`,
  `conf.foreach-range-literals-index`,
  `conf.foreach-range-negative-literal-typed`,
  `conf.foreach-range-negative-literals-rejected`,
  `conf.foreach-range-mixed-rejected`, `conf.foreach-range-float-rejected`,
  `conf.foreach-range-three-names-rejected`,
  `conf.foreach-range-dollar-rejected`, `conf.range-not-a-value` —
  `foreach` over an integer range.
- 3.89.0 — `CHG-0110` (D-0095): `conf.overwrite-field-of-live-object`,
  `conf.overwrite-through-exclusive-reference` (a live part's overwrite,
  now static), `conf.overwrite-optional-field-dynamic`,
  `conf.overwrite-computed-index-bounds-first` (the two limits that stay
  dynamic), and `conf.own-generic-hiding-std-checked-at-own-types` (the
  instantiation-key defect this change exposed).
- 3.88.0 — `CHG-0109` (D-0094): `conf.spawn-ref-result`,
  `conf.spawn-ref-result-source-written-rejected` — a spawned
  thread's bare-reference result, now run by both implementations.
- 3.87.0 — `CHG-0108` (D-0093): `conf.generic-item-value-uninferable`,
  `conf.generic-item-value-wrong-expected` — a generic item as a
  value is instantiated by explicit arguments or the expected fn
  type; a binding with neither is rejected.
- 3.86.0 — `CHG-0107` (D-0092): `conf.read-line-lines` changed (a `\r`
  before the `\n` is consumed with it); `conf.file-read-line-crlf`
  and its file case added.
- 3.85.0 — `CHG-0106` (D-0091): `conf.min-max`, `conf.abs-min-overflows`,
  `conf.pow-checked`, `conf.float-fns`, `conf.float-fn-int-rejected`,
  `conf.hashmap-clear`.
- 3.84.0 — `CHG-0105` (D-0090): `conf.bound-ordered`,
  `conf.bound-number-literal-float`, `conf.bound-missing-rejected`,
  `conf.bound-unsatisfied-rejected`, `conf.bound-weaker-caller-rejected`.
- 3.83.0 — `CHG-0104` (D-0089): `conf.fn-value-copy-independent`,
  `conf.fn-value-owning-closure-moves`, `conf.fn-value-emptied-call`.
- 3.82.0 — `CHG-0103` (D-0088): `conf.map-value-held-across-insert-rejected`,
  `conf.map-key-written-while-value-held`.
- 3.81.0 — `CHG-0102` (D-0087): `conf.match-nested-move-consumes` and
  `conf.match-move-arm-consumes` expect `(static)`.
- 3.80.0 — `CHG-0101` (D-0086): `conf.shift-binds-tighter-than-bitor`,
  `conf.shift-binds-tighter-than-bitand`, `conf.shift-looser-than-additive`.
- 3.79.0 — `CHG-0100` (D-0084, D-0085): `conf.temp-borrow-operator-argument`,
  `conf.temp-borrow-literal-typed-by-parameter`,
  `conf.temp-borrow-literal-binding-rejected`,
  `conf.rc-clone-while-value-borrowed`, `conf.vec-from-slice`,
  `conf.vec-from-slice-resource-rejected`, `conf.vec-truncate`,
  `conf.vec-truncate-beyond-length`,
  `conf.generic-call-extra-type-arguments-rejected`,
  `conf.field-through-unwrapped-reference-borrowed`,
  `conf.field-of-enum-rejected`,
  `conf.closure-capture-written-while-held-rejected`.
- 3.78.0 — `CHG-0096` through `CHG-0099` (D-0080 through D-0083):
  `conf.overwrite-destroys-old-value`, `conf.closure-result-type-written`,
  `conf.closure-result-type-conflict`, `conf.unwrap-some`, `conf.vec-clone`,
  `conf.unwrap-none-faults`, `conf.expect-message`,
  `conf.vec-clone-resource-rejected`, `conf.fault-unregistered-name`,
  `conf.fault-with-message`.
- 3.77.0 — `CHG-0095` (D-0079): `conf.literal-argument-typed-by-other-argument`,
  `conf.literal-argument-typed-by-expected-type`,
  `conf.literal-arguments-only-default`,
  `conf.literal-argument-out-of-range-for-inferred-type`.
- 3.76.0 — `CHG-0093`, `CHG-0094` (D-0078): `conf.adjacent-string-literals`,
  `conf.adjacent-string-literals-inline`, `conf.capitalized-builtin-type`,
  `conf.foreach-borrowed-temporary`, `conf.join-result-typed`,
  `conf.join-result-type-mismatch`, `conf.closure-capture-keeps-its-type`.
- 3.75.0 — `CHG-0092`: `conf.literal-bound-local-overflow-static`,
  `conf.literal-bound-local-written`.
- 3.74.0 — `CHG-0091` (D-0077): `conf.recursion-ten-thousand-deep`,
  `conf.recursion-stack-exhausted`.
- 3.73.0 — `CHG-0090` (D-0076): `conf.string-less-sort`, `conf.string-truncate`,
  `conf.string-truncate-inside-character`.
- 3.72.0 — `CHG-0089`: `conf.temp-borrow-joined-result`,
  `conf.temporaries-across-thread-switches`.
- 3.71.0 — `CHG-0088` (D-0075): `conf.destructor-on-plain-type-rejected`.
- 3.70.0 — `CHG-0087` (D-0074): `conf.array-length-constant`,
  `conf.array-length-not-constant`, `conf.temp-borrow-escape-stale`,
  `conf.temp-borrow-in-condition`, `conf.move-out-of-temporary-field-rejected`.
- 3.69.0 — `CHG-0086` (D-0073): `conf.temp-borrow-argument`,
  `conf.temp-borrow-binding-rejected` (§4); `conf.slice-through-returned-ref`
  (§6); `conf.closure-result-checked`, `conf.closure-in-vec-callable`,
  `conf.closure-generic-inferred` (§10); `conf.vec-insert-remove`,
  `conf.option-result-predicates`, `conf.string-eq`,
  `conf.string-eq-operator-rejected` (§11).
- 3.68.0 — `CHG-0085` (D-0072): `conf.checked-neg`,
  `conf.checked-neg-unsigned` (§1); `conf.array-repeat`,
  `conf.array-repeat-inferred`, `conf.array-repeat-reference-rejected`,
  `conf.array-repeat-resource-rejected`, `conf.array-trailing-comma` (§6);
  `conf.syntax-c-array-declaration` (§15).
- 3.67.0 — `CHG-0084`: `conf.move-through-reference-rejected`,
  `conf.move-through-param-rejected` (§3), `conf.spawn-slice-arguments`
  (§12).
- 3.66.0 — `CHG-0083` (D-0071): `conf.parent-use-while-child-live-rejected`,
  `conf.match-by-ref-replace-while-bound-rejected`,
  `conf.cross-thread-write-conflict` and `conf.guard-interior-borrow-conflict`
  are refuted statically; seven cases added in §4, §11 and §12.
- 3.65.0 — `CHG-0082`: §15 (new, syntax errors); two §14 rows for
  local functions; `conf.deref-non-reference` in §2; the file cases that
  expected `parse-error` expect `diag.syntax-error (static)`.
- 3.64.0 — `CHG-0081`: two §14 cases for declarations by two names.
- 3.63.0 — `CHG-0080` (D-0070): five §6 cases for slices of ranges.
- 3.62.0 — `CHG-0079` (D-0069): four §14 cases for local types (and two
  file cases for syntax errors, which these rows do not express).
- 3.61.0 — `CHG-0078` (D-0068): four §14 cases for `const auto` and
  `static_assert` messages with values.
- 3.60.0 — `CHG-0077` (D-0067): `conf.static-assert-not-constant` and
  `conf.static-assert-not-bool` expect `diag.static-assert-not-constant`.
- 3.59.0 — `CHG-0076` (D-0066): five §14 cases for local constants and
  fields and elements of constants.
- 3.58.0 — `CHG-0075` (D-0065): four §8 cases for `assert`.
- 3.57.0 — Four §12 cases from the 2026-09-27 concurrency testing round
  (no rule changed; `coby` had typed a `spawn` argument without its
  parameter, kept a temporary guard locked, returned a `join`ed aggregate
  untyped, and missed a channel deadlock arising during a wait).
- 3.56.0 — `CHG-0074` (D-0064): two §12 cases for access through a guard.
- 3.55.0 — `CHG-0072`, `CHG-0073` (D-0063): §12 cases for channels and
  for a mutex inside a shared struct; a §3 case for a resource stored in
  a binding made from `None`.
- 3.54.0 — `CHG-0070` (D-0062): four §11 cases for sorting and searching.
- 3.53.0 — `CHG-0069` (D-0061): one §11 case for directories, the
  environment and the clocks.
- 3.52.0 — `CHG-0066`–`CHG-0068` (D-0058–D-0060): six §6 cases for
  binders and constants in patterns, two §11 cases for `File::printf`, one
  for `?` consuming a temporary.
- 3.51.0 — `CHG-0065` (D-0057): eight §6 cases for literal patterns (and a file case for a float pattern, a parse error).
- 3.50.0 — `CHG-0064` (D-0056): eight §6 cases for nested patterns.
- 3.49.0 — `CHG-0063` (D-0055): three §14 cases for items named as
  intrinsics.
- 3.48.0 — `CHG-0062` (D-0054): five cases for `File`.
- 3.47.0 — `CHG-0061` (D-0053): eight cases for `StringView`.
- 3.46.0 — `CHG-0060` (D-0052): seven cases for `static_assert`.
- 3.45.0 — `CHG-0059` (D-0051): six cases for the float conversions and
  limits; `conf.limits-not-integer` now uses `bool` (a float has limits).
- 3.44.0 — `CHG-0057` (D-0049): nine cases for literal branches, the
  shared reborrow at calls, overwriting a value that owns nothing, and
  a `match` consuming only what it moves.
- 3.43.0 — `CHG-0056` (D-0048): seven cases for `swap`, `replace`,
  `Vec::swap` and `slice_swap`.

- 3.42.0 — `CHG-0055` (D-0047): nineteen cases for slices, `$` and
  indexing a `Vec`.

- 3.41.0 — `CHG-0054` (D-0046): six cases for `match` through a
  reference.

- 3.40.0 — `CHG-0053` (D-0045): five cases for `[Type-Recursive]` and
  `Box<T>`.

- 3.39.0 — `CHG-0052` (D-0044): nine cases for `String::clone`,
  `push_ascii`, `Vec::clear`, full destructuring and a map loop's position.

- 3.38.0 — `conf.reborrow-call-evaluated-once` and
  `conf.return-ends-statement-temporaries`: two cases for faults in
  `coby` that writing the Tier 6 showcases found (`&mut *f(k)` evaluated
  `f(k)` twice; a statement left by `return` kept its temporaries). No
  rule changed; `cobc` already behaved as specified.

- 3.37.0 — `CHG-0051` (D-0043): §1 cases for digit separators.

- 3.36.0 — `CHG-0050` (D-0042): sixteen §11 cases for `foreach` and
  `printf` of a reference.

- 3.35.0 — `CHG-0049` (D-0041): twelve §11 cases for `HashMap` and
  `HashSet`.

- 3.34.0 — `CHG-0048` (D-0040): three §11 cases for `eprintf`.

- 3.33.0 — `CHG-0047` (D-0039): six §11 cases for `%v`, `sprintf` and
  `print` being private to `std`; derivations naming `print` as a
  program's call now name `printf`.

- 3.32.0 — `CHG-0046` (D-0038): eight §11 cases for
  `rule.stdlib.format`.

- 3.31.0 — `CHG-0045` (D-0037): four §1 cases for literal-expression
  typing.

- 3.30.0 — `CHG-0044` (D-0035, D-0036): §1 cases for radix literals and
  compound assignment, §3 cases for `for`, §14 cases for `const`.

- 3.29.0 — `CHG-0043` (D-0034): three §11 cases for
  `rule.stdlib.file`, and `conf.generic-void-argument` in §9. File
  cases run in their own directory; `$TMP` in `args:`.

- 3.28.0 — `CHG-0042` (D-0033): `conf.reassign-after-drop-rejected` is
  now `conf.reassign-after-drop-ok` (its outcome changed); four more
  §3 cases for `[Assign-Reestablish]`, seven §1 cases for float
  exponents, float literal range and `b'x'`, two §11 cases for
  `unwrap_or`.

- 3.27.0 — `CHG-0041` (D-0032): nine §11 cases for `rule.stdlib.text`,
  three §14 cases for variant resolution (`spec/17` clauses (4) and
  (4b)), and `conf.generic-assoc-fn-inferred` in §9.

- 3.26.0 — `CHG-0040` (D-0031): six §11 cases for `rule.stdlib.args`
  and four §14 cases for `main`'s signature and exit status. The
  outcome `ok, exit status n`, and the `args:` header of file cases.

- 3.25.0 — `CHG-0039` (D-0030): two §7 cases for
  `rule.trust.extern-code`. Cases an implementation may be unable to
  run are marked `cobc-only:` (the three `read_line` file cases too).

- 3.24.0 — `CHG-0038` (D-0029): five §11 cases for `rule.stdlib.read`.
  The three that read standard input are file cases with a
  `stdin-hex:` header giving their input.

- 3.23.0 — `conf.enum-literal-expected-type` added to §9: the positions
  where `rule.type.expected` reaches an enum literal's payload. No rule
  changed; both implementations had typed such a payload as `i32`.

- 3.22.0 — `CHG-0037` (D-0028): eight §11 cases for `rule.stdlib.print`.

- 3.21.0 — `CHG-0036` (D-0027): nine §1 cases for conversion
  directions, `[T-Call]`'s arity and callability, and intrinsic names
  yielding to a program's own.

- 3.20.0 — `CHG-0035` (D-0026): eleven §1 cases for `rule.arith.limits`,
  `[T-Alt]` and `[T-Convert]`, including their checking at an
  instantiation.

- 3.19.0 — `CHG-0034` (D-0025): seven §1 cases for `[Literal-Negated]`,
  `[T-Neg]` on unsigned operands, and `[Div-Overflow]` refuted
  statically. `conf.i32-div-min-neg-one`'s derivation no longer says
  `min(i32)` cannot be written as a literal.

- 3.18.0 — `CHG-0033`: fragments are assembled in a program beginning
  `import std;` (§0 conventions). `conf.duplicate-prelude-fn-rejected`
  and `conf.prelude-type-new-assoc-fn-ok` (3.17.0) removed: with the
  library in `std`, a root `fn Vec::drop` or `fn Vec::first_or` is
  `[Assoc-Fn-Foreign-Type]`, covered by the new
  `conf.foreign-assoc-fn-rejected`. Nine further §14 cases for `std`, module
  imports and private `std` fields, and
  `conf.module-struct-destructor-runs`. `conf.string-into-bytes-roundtrip`'s
  fragment builds its fallback with `String::from_str("")` instead of a
  `String` literal, whose private field the root can no longer name.
- 3.17.0 — `CHG-0032`: five `conf.duplicate-*` rejections and two
  positive cases (`conf.prelude-type-new-assoc-fn-ok`,
  `conf.same-name-other-module-ok`) added to §14 for `[Item-Duplicate]`.
- 3.16.0 — `CHG-0031`: `conf.vec-of-refs-usable`,
  `conf.vec-holds-exclusive-ref-conflict` and `conf.vec-pop-releases-ref`
  added to §7 — references stored in raw storage by a plain `Vec` element.
- 3.15.0 — `CHG-0030`: `conf.thread-vec-of-resources-dropped` and
  `conf.thread-vec-of-resources-returned` added to §12 — a `Vec` of
  resources destroyed by a thread other than the one that pushed its
  elements.
- 3.14.0 — `CHG-0027`, `CHG-0028`, `CHG-0029`: new §14 with the nine
  `conf.module-file-*` cases `CHG-0026` named (now homed here, as
  `spec/02` §2 requires), four `conf.field-*` cases for
  `[Field-Not-Visible]`, `conf.unbound-qualified-name-static`, and
  `conf.no-main`; the section's fragment convention (a path to a
  whole-program file) is new.
- 3.13.0 — `CHG-0025` (D-0020): eleven `str` rows added to §11
  (`conf.str-literal-type`, `conf.str-copy-no-move`, `conf.str-eq`,
  `conf.str-ordering-rejected`, `conf.str-byte`,
  `conf.str-byte-out-of-bounds`, `conf.byte-literal-array`,
  `conf.string-from-str`, `conf.str-in-vec`, `conf.str-not-ffi`,
  `conf.print-observed`) and `conf.e2e-hello-print` to §13, deriving
  `ex.e2e-hello-print` end to end. No existing row changed.
- 3.12.0 — `CHG-0021`: `conf.extern-write-observed` added, deriving
  `ex.extern-write` end to end through `[Rawptr-Of]`, `reinterpret_ptr`,
  and `[Extern-Call]` (`CHG-0020`'s `write`).
- 3.11.0 — `CHG-0019`: `conf.e2e-vec-pop-push-reuse-stale-ref` added,
  closing finding F-05.
- 3.10.0 — `CHG-0015`: `conf.spawn-join-resource-result` added,
  demonstrating the `spec/19` 1.5.0 authority-transfer fix (`F-06`).
- 3.9.0 — `CHG-0014`: `conf.string-into-bytes-roundtrip`,
  `conf.map-err-transforms`, and `conf.destroy-composite-multi-field-
  order` added; `conf.rc-clone-shares-allocation`/`conf.rc-last-drop-
  frees`'s derivations enriched to name `[Rawptr-Write]` (not
  `[Rawptr-Move-In]`) and `[Destroy-Plain]` (not `[Destroy]`)
  explicitly for a plain `T`. No outcome changed on any row. A
  soundness finding from the same pass (`F-05`,
  `spec/AUDIT-STATUS.md`) is recorded, not fixed here.
- 3.8.0 — `CHG-0013`: `conf.e2e-mutex-vec-resource-interior`,
  `conf.e2e-closure-move-rc`, `conf.e2e-thread-fault-abandons-guard`
  added — the remaining three `CHG-0011` revisit-condition programs,
  derived end to end. No rule changed.
- 3.7.0 — `CHG-0012`: `conf.e2e-vec-realloc-stale-ref` added — derives
  the first `CHG-0011` revisit-condition program end to end. No rule
  changed.
- 3.6.0 — `CHG-0011`: §13 added — four whole programs derived end to
  end (Master Instructions §20). The derivations exposed seven rule
  gaps, fixed by `CHG-0011`; no expected outcome changed.
- 3.5.0 — `CHG-0010`: every row re-derived step by step against the
  current rules. Fourteen derivations were found to rest on a rule
  that did not exist or did not apply as cited (shift-amount and index
  literal typing, expected-type propagation through `if`, a `never`
  block, cross-signedness `narrow`, `?` on `Err`, `*g` on a guard,
  built-in destructors, `represent` of handle/guard, the FA rows for
  consuming `match`/destructure/raw-write and for
  `¬live-resource-at`, the raw-access trusted condition); the rules
  were completed and the derivations rewritten to cite them. Two
  derivations mis-cited `[Store-Binding-Place-Copy]` for a
  value-position copy. No fragment's outcome changed.
- 3.4.0 — `CHG-0009`: `conf.reclaim-two-paths-clash`'s fragment
  re-based on a `Vec` allocation. The 3.3.1 fragment reclaimed the
  cells of a live fresh-storage local (`x`), which is exactly what
  `[Reclaim]`'s `discharge: trusted` side-condition asserts does not
  happen — the case's outcome was derivable only by assuming a false
  trusted claim. Outcome and diagnostic unchanged. Conventions now
  state what `c` in `if (c)` fragments denotes.
- 3.3.1 — Noted the single-line-fragment convention as this file's
  documented structural exception to `spec/22` §1's Allman brace
  convention (human-directed documentation pass). Non-normative per
  `spec/02-schema.md` §6 — no `CHG-XXXX` record; no row's content
  changed.
- 3.3.0 — `CHG-0005`: every `conf.match-*` row and
  `conf.vec-pop-resource`/`conf.string-from-utf8-ok` re-spelled with
  `:` instead of `=>` for match arms. No row's id, outcome, or
  derivation changed.
- 3.2.0 — `CHG-0003`: every `if`/`while`/`match` fragment re-spelled
  with mandatory condition parentheses (`spec/22` 2.2.0). No row's id,
  outcome, or derivation changed.
- 3.1.0 — `CHG-0002`: every `conf.closure-*` row and
  `conf.spawn-borrow-closure-rejected` re-spelled to `spec/22`
  2.1.0's C++11-style closure syntax; added
  `conf.closure-capture-list-mismatch-rejected` for the new
  `diag.capture-list-mismatch` check. No other row touched.
- 3.0.0 — Re-spelled to `spec/22` 2.0.0's C-style declarator syntax
  (`CHG-0001`): type-first local declarations and parameters, `auto`
  for inferred locals, `.f = e` struct-literal initializers,
  semicolon-terminated struct fields, `:`-introduced return types.
  Section 2's heading updated from "Values, `let`, and bindings" to
  "Values, local declarations, and bindings" to match. No expected
  outcome or Depends-on rule of any `conf.*` entity changed.
- 2.0.0 — Regenerated against the 1.0.0 rules with a derivation per
  case (`spec/AUDIT-2.md` §5 step 6). Three cases record where the
  derivation contradicted the intended fragment and the fragment was
  corrected rather than the rule (`conf.drop-then-reassign-ok`,
  `conf.reborrow-ok`, `conf.elision-conflict-dynamic`); 34 cases
  added covering B-01–B-09 and the library.
- 1.0.0 and earlier — superseded.
