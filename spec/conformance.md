# CobaltC Conformance Cases

Status: normative artifact
Version: 3.51.0
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
`rule.arith.literal`; `c` denotes a `bool` binding already in scope
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
| `conf.parse-rejects-digit-separator` | `String s = String::from_str("1_000"); Result::unwrap_or(parse<u32>(&s), 7) == 7` | `→ true` | `rule.stdlib.text` `[Parse]`: `_` is not part of `number(T)`; a separator is source syntax, not text a program reads |
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
| `conf.i32-add-overflow-dynamic` | `fn add(i32 a, i32 b) : i32 { a + b }` `add(2147483647, 1)` | ✗ `diag.arith-overflow` (dynamic) | operands are parameters → FA unknown → runtime guard fails |
| `conf.i32-div-zero-dynamic` | `fn d(i32 a, i32 b) : i32 { a / b }` `d(10, 0)` | ✗ `diag.div-by-zero` (dynamic) | `[Div-By-Zero]` |
| `conf.i32-div-min-neg-one` | `d(-2147483647 - 1, -1)` | ✗ `diag.div-overflow` (dynamic) | `[Div-Overflow]` (`min(i32)` is computed, not a literal, so FA does not refute the guard; the literal form is `conf.div-min-neg-one-static`) |
| `conf.u8-wrapping-add` | `wrapping_add(255: u8, 1: u8)` | `→ 0` | `[Wrapping]` |
| `conf.checked-add-none` | `checked_add(255: u8, 1: u8)` | `→ None` | `[Checked-None]`; type `Option<u8>` |
| `conf.shift-in-range` | `1: u32 << 31: u32` | `→ 2147483648` | `[Shl-Checked]` |
| `conf.shift-out-of-range` | `1: u32 << 32: u32` | ✗ `diag.shift-amount-out-of-range` (static) | literals → FA refuted |
| `conf.shr-arithmetic` | `fn s(i32 x) : i32 { x >> 1 }` `s(-8)` | `→ -4` | `1 : u32` from the shift-amount position (`rule.type.expected`); `[Shr-Checked]`: floor(-8/2) |
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

## 2. Values, local declarations, and bindings (`spec/05`, `spec/11`)

| id | Fragment | Outcome | Derivation |
|---|---|---|---|
| `conf.let-copy` | `i32 x = 1; auto y = x; y + x` | `→ 2` | `[Let]`→`[Store-Binding-Value]` for x; y's initializer is value position (non-resource) → `[LValue-To-RValue]` reads `a_x` → `[Store-Binding-Value]` into fresh `o_y`; both readable |
| `conf.let-uninit-then-assign-ok` | `i32 x; x = 5; x` | `→ 5` | `[Let-Uninit]`; whole-object `[Write]` (`init-ok`: uninitialized, whole); FA `init(x) = T` at the read |
| `conf.definite-assignment-both-branches` | `i32 x; if (c) { x = 1; } else { x = 2; } x` | `ok` | FA: `init(x) = T` on both branches → join T |
| `conf.definite-assignment-missing-branch` | `i32 x; if (c) { x = 1; } x` | ✗ `diag.use-of-uninitialized` (static) | join of T and F = ? → `rule.init.definite-assignment` rejects on unknown |
| `conf.partial-init-rejected` | `struct P { i32 a; i32 b; } P p; p.a = 1;` | ✗ `diag.use-of-uninitialized` (static) | projection write with `init = uninitialized` → `[Write-Partial-Init]`; FA `init(p) = F` refuted |
| `conf.shadowing` | `i32 x = 1; i32 x = 2; x` | `→ 2` | second `[Binding-Form]` rebinds the name; the first object remains owned until block exit |
| `conf.unbound-name` | `fn f() : i32 { y }` | ✗ `diag.unbound-name` (static) | `[Binding-Lookup-Unbound]`, `[Resolve-Unbound]` |

## 3. Resource authority (`spec/07`, `spec/14`)

| id | Fragment | Outcome | Derivation |
|---|---|---|---|
| `conf.transfer-invalidates-source` | `Vec<i32> v = Vec::new(); auto w = v; Vec::push(&mut v, 1);` | ✗ `diag.stale-binding` (static) | `auto w = v`: `[Store-Binding-Place-Transfer]` → `[Authority-Transfer]` invalidates `a_v`; FA `valid(v) = F` → `[Binding-Lookup-Stale]` refuted |
| `conf.double-destroy-rejected` | `Vec<i32> v = Vec::new(); drop(v); drop(v);` | ✗ `diag.stale-binding` (static) | first `drop`: `[Destroy]` → `[Object-End]` invalidates `a_v`; FA `valid(v) = F` |
| `conf.leak-free-by-construction` | `{ Vec<i32> v = Vec::new(); }` | `ok`; `Vec::drop` runs once | `Vec::new` → `[Struct-Construct]` temp with authority (resource marker); `[Store-Binding-Temp]` adopts; BE: `owned-by-frame(o_v)` → `[Destroy]` → `[Run-Destructor]` → `[Object-End]` |
| `conf.unstored-temporary-destroyed-at-stmt-end` | `fn make() : Vec<i32> { Vec::new() }` `make();` | `ok`; `Vec::drop` runs at the `;` | the call result is `temp o` with `temp-scope` re-stamped to the caller's statement; SE (`discard`): `Temps ∋ o` → `[Destroy]` |
| `conf.returned-resource-destroyed-at-caller-exit` | `fn make_vec() : Vec<i32> { Vec<i32> v = Vec::new(); Vec::push(&mut v, 1); v }` `{ auto r = make_vec(); }` | `ok`; destroyed at the caller's BE | trailing `v` (keep): `[Store-Result-Place-Release]` clears `holder(o_v)`, invalidates `a_v`; BE of `make_vec`'s body and `[Call]`'s BE exempt `objs-in(temp o_v)`; `[Let]` adopts; caller BE destroys |
| `conf.early-return-destroys-locals` | `fn f() : i32 { Vec<i32> v = Vec::new(); if (true) { return 1; } 2 }` `f()` | `→ 1`; `Vec::drop` runs | `[Return]`: `unwind-to(f_b, 1)` runs SE/BE of the `if` branch and the body frame → `o_v` destroyed |
| `conf.destroy-while-borrowed-rejected` | `Vec<i32> v = Vec::new(); auto r = &v; drop(v);` | ✗ `diag.destroy-while-aliased` (static) | `a_r'` valid on `o_v` → `[Destroy-Not-Solitary]`; FA `deriv(r, v.ε, shared) = T` → `solitary` refuted |
| `conf.destroy-after-borrow-scope-ends-ok` | `{ Vec<i32> v = Vec::new(); auto r = &v; }` | `ok` | BE: `Owned = {o_v, o_r}` descending origin → `o_r` ends first (`[Object-End]`: `a_r'` `held-by = {o_r}` → Dead → invalid) → `[Destroy]` on `a_v`: solitary ✓ |
| `conf.move-while-borrowed-rejected` | `Vec<i32> v = Vec::new(); auto r = &v; auto w = v;` | ✗ `diag.move-while-aliased` (static) | `[Authority-Transfer-Aliased]`: `a_r'` ∉ {a_v, a_w}; FA `deriv` T → refuted |
| `conf.overwrite-live-resource-rejected` | `Vec<i32> v = Vec::new(); v = Vec::new();` | ✗ `diag.overwrite-of-live-resource` (static) | `[Write-Resource-Overwrite-Rejected]`: `o_v ∈ obligations`; FA `¬live-resource-at` row: `valid(v) = T ∧ init(v) = T` → refuted |
| `conf.reassign-after-drop-ok` | `Vec<i32> v = Vec::new(); drop(v); v = Vec::new(); Vec::push(&mut v, 1); Vec::len(&v)` | `→ 1` | `drop` ends `o_v`; `v = Vec::new()` is `[Assign-Reestablish]` (`spec/11` §2, D-0033): a new object bound to `v` in `main`'s frame; FA `valid(v) := T` again |
| `conf.reassign-after-move-ok` | `Vec<i32> v = Vec::new(); auto w = v; v = Vec::new(); Vec::push(&mut v, 1); Vec::len(&v) + Vec::len(&w)` | `→ 1` | `auto w = v` transfers `o_v` to `w`; `[Assign-Reestablish]` gives `v` a new object; `w` keeps the old one |
| `conf.reassign-from-own-move-ok` | `fn grow(Vec<i32> v) : Vec<i32> { Vec<i32> w = v; Vec::push(&mut w, 7); w }` `Vec<i32> v = Vec::new(); v = grow(v); v = grow(v); Vec::len(&v)` | `→ 2` | `grow(v)` moves `v`'s value away while the right side is evaluated; the assignment then re-establishes `v` with the result |
| `conf.reassign-in-inner-block-ok` | `Vec<i32> v = Vec::new(); { auto w = v; v = Vec::new(); } Vec::push(&mut v, 3); Vec::len(&v)` | `→ 1` | the new object is bound in the frame that declared `v`, not the inner block's, so it outlives the block; `w`'s object ends at the block's BE |
| `conf.reassign-maybe-live-dynamic` | `Vec<i32> v = Vec::new(); if (!c) { drop(v); } v = Vec::new();` | ✗ `diag.overwrite-of-live-resource` (dynamic) | FA `valid(v) = unknown` after the `if`, so `¬live-resource-at` is checked at run time; `c` is true, `v` was not dropped, and it still holds a live `Vec` |
| `conf.drop-then-shadow-ok` | `Vec<i32> v = Vec::new(); drop(v); Vec<i32> v = Vec::new(); Vec::len(&v)` | `→ 0` | second `[Let]` forms a fresh object and binding |
| `conf.read-of-resource-rejected` | `Vec<i32> v = Vec::new(); Vec<i32> w = Vec::new(); v == w` | ✗ `diag.read-of-resource` (static) | comparison operands are value position → `[Read-Resource-Rejected]` (before `[T-Cmp]`'s own rejection) |
| `conf.move-out-of-field-rejected` | `struct P { Vec<i32> a; } auto p = P { .a = Vec::new() }; auto q = p.a;` | ✗ `diag.move-out-of-field` (static) | `[Store-Binding-Place-Transfer-Sub]` |
| `conf.while-body-frame-per-iteration` | `i32 i = 0; while (i < 3) { Vec<i32> v = Vec::new(); i = i + 1; }` | `ok`; `Vec::drop` runs 3 times, each before the next test | `[While-True]`: `loop-body(b) ; while (…) …` — the body block's BE runs before `[Seq-Value]` reaches the re-test |
| `conf.for-sum-continue` | `u64 s = 0; for (u64 i = 0; i < 10; i += 1) { if (i % 3 == 0) { continue; } s += i; } s` | `→ 27` | `[For]` (`spec/14`, D-0035): `continue` runs the step before the next test; 1 + 2 + 4 + 5 + 7 + 8 |
| `conf.for-empty-parts` | `i32 n = 0; for (; n < 5; ) { n += 2; } for (;;) { break; } n` | `→ 6` | each part may be left out; a missing condition is `true` |
| `conf.return-ends-statement-temporaries` | `fn look(ref<Vec<String>, shared> v) : i32 { match (Some(Vec::index_shared(v, 0))) { Some(r) : { return 1; }, None : {}, } 0 }` `Vec<String> v = Vec::new(); Vec::push(&mut v, String::from_str("x")); look(&v) == 1` | `→ true` | `rule.control.stmt`: the `return` leaves the `match` statement, whose temporary scrutinee (holding a reference into `v`) ends then; `v` is later destroyed with no reference left |
| `conf.for-variable-scoped` | `for (i32 i = 0; i < 1; i += 1) { } i` | ✗ `diag.unbound-name` (static) | the initializer's binding belongs to the `for`: `{ i32 i = 0; while … }` |
| `conf.break-destroys-body-locals` | `i32 i = 0; while (true) { Vec<i32> v = Vec::new(); break; } i` | `→ 0`; `Vec::drop` once | `[Break]`: `unwind-to(f_w, ())` runs the body frame's BE |

## 4. Aliasing (`spec/08`, `spec/16`)

| id | Fragment | Outcome | Derivation |
|---|---|---|---|
| `conf.two-shared-borrows-ok` | `i32 x = 1; auto r1 = &x; auto r2 = &x; *r1 + *r2` | `→ 2` | `[Borrow]` twice: `clash(a_x, shared)`: `a_r1'` shared → permitted; reads: `a_x` is an ancestor of each, siblings shared |
| `conf.owner-read-while-shared-ok` | `i32 x = 1; auto r = &x; x + *r` | `→ 2` | `[Read]` through `a_x`: `clash(a_x, shared)` — `a_r'` is a descendant, shared → permitted |
| `conf.owner-write-while-shared-rejected` | `i32 x = 1; auto r = &x; x = 2;` | ✗ `diag.aliasing-conflict` (static) | `[Write-Conflict]`: `a_r'` shared vs exclusive; FA `deriv(r, x.ε, shared) = T` refuted |
| `conf.owner-read-while-exclusive-rejected` | `i32 x = 1; auto r = &mut x; x` | ✗ `diag.aliasing-conflict` (static) | `[Read-Conflict]`: `a_r'` exclusive |
| `conf.shared-then-exclusive-rejected` | `i32 x = 1; auto r1 = &x; auto r2 = &mut x;` | ✗ `diag.aliasing-conflict` (static) | `[Borrow-Denied]`: `clash(a_x, exclusive)` with `a_r1'` |
| `conf.sequential-borrows-ok` | `i32 x = 1; { auto r1 = &mut x; *r1 = 2; } auto r2 = &x; *r2` | `→ 2` | inner BE ends `o_r1` → `a_r1'` Dead; second `[Borrow]` finds no live conflicting path |
| `conf.reborrow-ok` | `i32 x = 1; auto r1 = &mut x; { auto r2 = &*r1; *r2; } *r1 = 3; *r1` | `→ 3` | `&*r1`: `[Ref-Deref-Place]` → `[Borrow]` from `a_r1'` (shared from exclusive; `a_x` is an ancestor); after the block `a_r2'` is Dead; `*r1 = 3` and `*r1` clash with nothing (reading `x` itself here would be `[Read-Conflict]`, since `a_r1'` is a live exclusive descendant) |
| `conf.parent-use-while-child-live-rejected` | `i32 x = 1; auto r1 = &mut x; auto r2 = &*r1; *r1 = 2;` | ✗ `diag.aliasing-conflict` (dynamic) | `[Write-Conflict]`: `a_r2'` is a shared descendant of `a_r1'`; FA: the write's root is `*r1` → unknown → dynamic |
| `conf.projection-write-while-borrowed-rejected` | `struct P { i32 a; i32 b; } auto p = P { .a = 1, .b = 2 }; auto r = &mut p.a; p.a = 5;` | ✗ `diag.aliasing-conflict` (static) | fresh projection `a_pa2` (base `a_p`); `clash(a_pa2, exclusive)`: `a_r'` valid, not an ancestor, overlapping; FA `deriv(r, p.a, exclusive)` overlaps `p.a` → refuted |
| `conf.disjoint-field-borrows-ok` | same `P`; `auto r1 = &mut p.a; auto r2 = &mut p.b; *r1 + *r2` | `→ 3` | `clash(a_pb, exclusive)`: `a_r1'` targets `a`'s cells, no overlap; FA paths `a`, `b` do not overlap → proven |
| `conf.exclusive-from-shared-rejected` | `fn g(ref<P, shared> p) { auto w = &mut p.a; }` | ✗ `diag.borrow-exceeds-source` (static) | `[Field-Access-Auto-Deref]` gives a shared projection; `[Borrow-Exceeds-Source]` |
| `conf.write-through-shared-rejected` | `fn g(ref<i32, shared> p) { *p = 1; }` | ✗ `diag.write-through-shared` (static) | `[Write-Not-Exclusive]` |
| `conf.sequential-exclusive-borrows-ok` | `Vec<i32> nums = Vec::new(); Vec::push(&mut nums, 10); Vec::push(&mut nums, 20); Vec::len(&nums)` | `→ 2` | each `&mut nums` is held only by the callee's parameter object; `[Call]`'s BE ends it → the borrow is Dead before the next statement |
| `conf.reference-in-field-survives-statement` | `struct H { ref<i32, shared> r; } i32 x = 1; auto h = H { .r = &x }; *h.r` | `→ 1` | `[Store-Sub-Value]` adds `o_h` to the borrow's `held-by`; SE does not touch held paths; `h.r` read → value `a` → `[Ref-Deref-Place]` → read |
| `conf.borrow-of-temporary-rejected` | `struct P { i32 a; } auto r = &P { .a = 1 }.a;` | ✗ `diag.borrow-of-temporary` (static) | `[Temp-Root]` then `[Ref-Form-Temporary]` |

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
| `conf.match-nested-move-consumes` | `impl/conformance/16-aggregates/match_nested_move_consumes_rejected.cb` | ✗ `diag.stale-binding` (dynamic) | the arm binding a nested `String` consumes the scrutinee |
| `conf.match-literals` | `impl/conformance/16-aggregates/match_literal_patterns_ok.cb` | `ok` | D-0057: `match` on integers, bytes and `bool`s; `Ok(0)`, `Err(-5)`, `Some(Some(true))`; `u128`'s largest and `i128`'s smallest literals; a computed scrutinee; a literal through a reference; arms that `break`, `continue`, `return` |
| `conf.match-literal-non-exhaustive` | `impl/conformance/16-aggregates/match_literal_non_exhaustive_rejected.cb` | ✗ `diag.non-exhaustive-match` (static) | literals never cover an integer type; the message is `_` |
| `conf.match-bool-non-exhaustive` | `impl/conformance/16-aggregates/match_bool_non_exhaustive_rejected.cb` | ✗ `diag.non-exhaustive-match` (static) | `Some(false)` is not covered, and the message says so |
| `conf.match-literal-duplicate` | `impl/conformance/16-aggregates/match_literal_duplicate_rejected.cb` | ✗ `diag.unreachable-arm` (static) | a second `3` arm |
| `conf.match-literal-wrong-type` | `impl/conformance/16-aggregates/match_literal_wrong_type_rejected.cb` | ✗ `diag.type-mismatch` (static) | `[Pattern-Type]`: `1` against a `bool` |
| `conf.match-literal-out-of-range` | `impl/conformance/16-aggregates/match_literal_out_of_range_rejected.cb` | ✗ `diag.literal-out-of-range` (static) | `300` against a `u8` |
| `conf.match-literal-on-enum` | `impl/conformance/16-aggregates/match_literal_on_enum_rejected.cb` | ✗ `diag.type-mismatch` (static) | `[Pattern-Type]`: a literal where a variant is |
| `conf.match-literal-through-reference` | `impl/conformance/16-aggregates/match_literal_through_reference_rejected.cb` | ✗ `diag.type-mismatch` (static) | `[Pattern-Type]`: `match (&x)` on an `i32` |
| `conf.match-resource-payload-transfers` | `enum E { Has(Vec<i32>), Empty } auto e = E::Has(Vec::new()); match (e) { Has(x) : Vec::len(&x), Empty : 0 }` | `→ 0`; `e` stale afterwards | scrutinee `a_e` root; `[Relocate-Out]` → temp adopted by `x` in the arm frame; consume (the `Has` arm moved the payload out): `[Destroy]` on `a_e` (no obligations left); arm BE destroys `o_x`; FA: only the `Has` arm consumes (D-0049), so after the match `valid(e) = unknown` and a later `drop(e)` is `[Binding-Lookup-Stale]` at run time (`diag.stale-binding`, dynamic) |
| `conf.match-through-ref-rejected` | `fn f(ref<E, shared> e) : usize { match (*e) { Has(x) : Vec::len(&x), Empty : 0 } }` | ✗ `diag.move-out-of-field` (static) | `[Match-Move-Through-Ref]`: `base(a) ≠ None` |
| `conf.let-destructure` | `struct W { Vec<i32> inner; } auto w = W { .inner = Vec::new() }; W { inner } = w; Vec::len(&inner)` | `→ 0` | `[T-Let-Destructure]`; `[Let-Destructure]`: `[Relocate-Out]` then `[Destroy]` of `o_w`; FA: `w` consumed |

## 7. Trust boundaries (`spec/20`)

| id | Fragment | Outcome | Derivation |
|---|---|---|---|
| `conf.rawptr-deref-outside-unsafe-rejected` | `fn f(rawptr<i32> p) : i32 { *p }` | ✗ `diag.trusted-outside-unsafe` (static) | `[Unsafe-Rejected]` |
| `conf.rawptr-deref-inside-unsafe-ok` | `fn f(rawptr<i32> p) : i32 { unsafe { *p } }` `i32 x = 7; f(rawptr_of(&x))` | `→ 7` | `[Rawptr-Of]` gives `min(target)`; `[T-Deref]` (rawptr case); `[Rawptr-Read]` reads the cells `[Repr-Int]` wrote — its trusted condition holds: `a_x` is a root, the argument borrow is shared |
| `conf.reclaim-two-paths-clash` | `Vec<i32> v = Vec::new(); Vec::push(&mut v, 7); auto p = rawptr_of(Vec::index_shared(&v, 0)); unsafe { auto r1 = &mut reclaim<i32>(p); auto r2 = &reclaim<i32>(p); }` | ✗ `diag.aliasing-conflict` (dynamic) | `index_shared` established element 0 as a reclaimed object (its borrow died at that statement's SE); each `reclaim` re-attaches it as a fresh root path (`[Reclaim]`'s trusted condition holds: the cells belong to `v`'s allocation, not to a fresh-storage object); the second `[Borrow]` (shared) sees `a_r1'` (exclusive, not an ancestor) → `[Borrow-Denied]`; FA: root is a reclaim → unknown → dynamic |
| `conf.extern-non-ffi-type-rejected` | `extern fn g(Vec<i32> v);` | ✗ `diag.extern-non-ffi-type` (static) | `[Extern-Non-Ffi-Type]` |
| `conf.extern-write-observed` | `ex.extern-write` | `ok; → claim : isize` (`Σ.trust(claim) = unchecked-claim`; the concrete value is outside this specification's scope, `feat.minimal-io-extern-surface`) | `[Array-Construct]` builds `message : array<u8,6>`, a fresh plain (non-resource) object; `&message` is a shared borrow of its root path `a_message`; `[Rawptr-Of]` (safe) gives `p0 = min(target(a_message)) : rawptr<array<u8,6>>`; `reinterpret_ptr<u8>` (safe, identity on the address) gives `p : rawptr<u8>`; `send(p, 6)` evaluates `p, 6` left to right (`u8`/`usize`/`rawptr<u8>` args, none resources or places) and enters `send`'s body; `write(p, n)` occurs lexically inside `unsafe { }`, satisfying `[Unsafe-Rejected]`'s guard; `[Extern-Call]`'s side-conditions are the author's trusted assertion (`p` is reachable storage from a live object, `write` only reads those 6 cells) — `disposition: trusted-unchecked`, so no rule checks it; conclusion: `claim : isize` with `Σ.trust(claim) := unchecked-claim`, `storage` widened only outside any live object's cells; `send` returns `claim`; no diagnostic, terminates `ok` |
| `conf.extern-code-library-name` | `impl/conformance/20-trust-boundaries/extern_code_library_name_ok.cb` | ok | `extern "m";` (`rule.trust.extern-code`) declares no name; `sqrt` is an ordinary `[Extern-Call]` whether or not the implementation links foreign code |
| `conf.extern-code-c-file` | `impl/conformance/20-trust-boundaries/extern_code_c_file_ok.cb` | ok | a program's own C file, named relative to the declaring file (the case, and a module in `extern_code/`); `[Extern-Call]` into it, the count it returns checked before use. Only an implementation that links foreign code runs it (`cobc-only:`) |
| `conf.vec-of-refs-usable` | `i32 a = 10; i32 b = 20; Vec<ref<i32, shared>> v = Vec::new(); Vec::push(&mut v, &a); Vec::push(&mut v, &b); **Vec::index_shared(&v, 0) + **Vec::index_shared(&v, 1)` | `→ 30` | each `push`: `[Rawptr-Write]` of a reference-bearing `ref<i32,shared>` establishes the element's reclaimed object as holder (`CHG-0031`), so `&a`/`&b` are held, not `Unheld`, at the push statement's SE; `index_shared` re-attaches the element object and borrows it; `**` reads through the stored reference (valid) |
| `conf.vec-holds-exclusive-ref-conflict` | `i32 x = 1; Vec<ref<i32, exclusive>> v = Vec::new(); Vec::push(&mut v, &mut x); x = 5;` | ✗ `diag.aliasing-conflict` (dynamic) | `&mut x` is held by the element's reclaimed object (`CHG-0031`); `x = 5`: `[Write]`'s `¬clash(a_x, exclusive)` finds that held, non-ancestor exclusive path; FA: `&mut x` passed to a call → `escaped(x)` → dynamic |
| `conf.vec-pop-releases-ref` | `i32 x = 1; Vec<ref<i32, exclusive>> v = Vec::new(); Vec::push(&mut v, &mut x); Vec::pop(&mut v); x = 5; x` | `→ 5` | `pop` reads the element (`[Rawptr-Read]`: a copy, the discarded `Option` ends at SE) and `release`s the slot: `[Release]` → `[Object-End]` of the element object, so `&mut x` is held by nothing and invalid; `x = 5` meets no conflicting path |

## 8. Failure semantics (`spec/18`)

| id | Fragment | Outcome | Derivation |
|---|---|---|---|
| `conf.checked-fault-unwinds-and-terminates` | `fn d(i32 a, i32 b) : i32 { a / b }` `{ Vec<i32> v = Vec::new(); d(1, 0); }` | terminates `diag.div-by-zero`; `Vec::drop` ran | `[Fault-Unwind]`: `unwind-to(f_0, ())` runs the block's BE |
| `conf.propagate-ok` | `fn f() : Result<i32, i32> { Result<i32, i32> r = Ok(1); auto v = r?; Ok(v + 1) }` `f()` | `→ Ok(2)` | `?` → `[Propagate]` → `[Match]`, arm `Ok(v)` copies the payload (`[Store-Binding-Place-Copy]`) into the arm frame |
| `conf.propagate-err` | `fn f() : Result<i32, i32> { Result<i32, i32> r = Err(5); auto v = r?; Ok(v) }` `f()` | `→ Err(5)` | `?` → `[Propagate]` → `[Match]`, arm `Err(err) : return Err(err)` → `[Return]` (`never`-typed arm, `[T-Never]`) |
| `conf.propagate-mismatch-rejected` | `fn f() : i32 { Result<i32, i32> r = Ok(1); r? }` | ✗ `diag.propagate-outside-fallible-context` (static) | `[Propagate-Err-Mismatch]` |

## 9. Generics and inference (`spec/12`, `spec/15`, `spec/16`)

| id | Fragment | Outcome | Derivation |
|---|---|---|---|
| `conf.generic-fn-explicit` | `fn id<T>(T x) : T { x }` `id<i32>(5)` | `→ 5` | `[Generic-Call-Explicit]`; `x` non-resource: returned by copy |
| `conf.generic-call-inferred` | `id(5)` | `→ 5` | `[Generic-Call-Inferred]`: `T := i32` (literal default) |
| `conf.generic-assoc-fn-inferred` | `struct W { i32 a; } fn W::put<T>(ref<W, exclusive> w, T x) { } W w = W { .a = 1 }; W::put(&mut w, 42);` | ok | `[Generic-Call-Inferred]`: `T := i32` from the second argument; `W` in the first parameter's type is the struct, not a type parameter |
| `conf.generic-void-argument` | `fn id<T>(T x) : T { x }` `id(());` | ok | `[Generic-Call-Inferred]`: `T := void` from `()`; a parameter may have type `void` |
| `conf.generic-call-expected-type-inferred` | `Vec<i32> nums = Vec::new(); Vec::len(&nums)` | `→ 0` | `[Generic-Call-Expected-Type]` matches `Vec<T>` to the annotation |
| `conf.generic-call-uninferable-rejected` | `auto bad = Vec::new();` | ✗ `diag.cannot-infer-type-parameter` (static) | no argument, no expected type |
| `conf.generic-struct-monomorphize-resource` | `struct Box<T> { T value; } Box<Vec<i32>> c = Box { .value = Vec::new() };` | `ok`; `is-resource(Box<Vec<i32>>) = true`; `Vec::drop` at BE | `rule.type.expected` (D-0014) fixes the field's expected type; `[Struct-Construct]` relocates the temp Vec in; BE → `[Destroy-Composite]` |
| `conf.unbounded-type-parameter-rejected` | `fn add<T>(T a, T b) : T { a + b }` | ✗ `diag.unbounded-type-parameter` (static) | `rule.type.kind` |
| `conf.literal-default-i32` | `auto x = 5;` | `x : i32` | `[Literal-Default]` |
| `conf.literal-context-u8` | `u8 x = 200;` | `x : u8` | `[Literal-Type-From-Context]` |
| `conf.let-synthesis` | `auto x = 3: i64 + 4: i64;` | `x : i64` | `[T-Let]` from `[T-Arith]` |
| `conf.enum-literal-expected-type` | `impl/conformance/12-type-system/enum_literal_takes_expected_type_ok.cb` | ok | `rule.type.expected`: `Some(5000000000)`'s `T` from the expected `Option<u64>` in a declaration, an assignment, a return, an argument (generic: `T` fixed by the arguments to the left), a field, a nested literal, and either operand of `==`/`!=` |

## 10. Closures (`spec/15` §6)

| id | Fragment | Outcome | Derivation |
|---|---|---|---|
| `conf.closure-borrow-capture` | `i32 x = 10; auto f = [x](i32 y) { x + y }; f(5)` | `→ 15` | `[Closure-Form-Borrow]`: capture mode shared; struct temp adopted by `f`; `[Closure-Call]`: exclusive self-borrow of `o_f`, body `*self.f1 + y` |
| `conf.closure-capture-list-mismatch-rejected` | `i32 x = 10; auto f = [](i32 y) { x + y };` | ✗ `diag.capture-list-mismatch` (static) | `[Closure-Capture-List-Rejected]`: written list `∅` ≠ free variables `{x}` |
| `conf.closure-move-capture-invalidates-source` | `Vec<i32> v = Vec::new(); auto f = move [v]() { Vec::push(&mut v, 1); }; f(); Vec::push(&mut v, 2);` | ✗ `diag.stale-binding` (static) | `[Closure-Form-Move]` relocates `o_v` into the closure; FA: move capture → `valid(v) = F` |
| `conf.closure-drop-through-self-rejected` | `Vec<i32> v = Vec::new(); auto f = move [v]() { drop(v); }; f();` | ✗ `diag.move-out-of-field` (static) | body's `drop(v)` is `drop(self.f1)` → `[Destroy-Projection]` |
| `conf.closure-owns-moved-resource` | `{ Vec<i32> v = Vec::new(); auto f = move [v]() { Vec::len(&v); }; }` | `ok`; `Vec::drop` at BE | closure object is a resource by derivation; BE → `[Destroy-Composite]` on its field |

## 11. Library (`spec/21`)

| id | Fragment | Outcome | Derivation |
|---|---|---|---|
| `conf.vec-push-len` | `Vec<i32> v = Vec::new(); Vec::push(&mut v, 1); Vec::push(&mut v, 2); Vec::len(&v)` | `→ 2` | `grow` on first push (`[Allocate]`; its `if` branches take `usize` from the declaration, `rule.type.expected`; `[T-Rawptr-Offset]`), `[Rawptr-Write]` twice; each `&mut v` argument is held only by the callee's parameter object and is Dead after the call |
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
| `conf.result-unwrap-or-resource` | `Result<Vec<i32>, i32> r = Err(3); Vec<i32> v = Result::unwrap_or(r, Vec::new()); Vec::len(&v)` | `→ 0` | `Err(_)` arm: the payload ends with the match; `d` is moved out as the result |
| `conf.map-err-transforms` | `fn scale(i32 e) : i32 { e * 10 }` `Result<i32, i32> r = Err(4); match (map_err(r, scale)) { Ok(v) : v, Err(e) : e }` | `→ 40` | `map_err<i32,i32,i32>(r, scale)`: `T,E1,E2 := i32` inferred from `r`'s and `scale`'s types (`[Generic-Call-Inferred]`); body `match (r) { Ok(v) : Ok(v), Err(e) : Err(f(e)) }` types under `[T-Match]`: `r : Result<i32,i32>`; arm `Err(e)` binds `e : i32`; `f : fn(i32):i32` is `callable` (`[Callable-Fn]`), so `f(e)` types via `[T-Call]` as `i32`; `Err(f(e)) : Result<i32,i32>` matches the arm above's `Ok(v) : Result<i32,i32>` (`rule.type.expected` propagates the function's declared return type into both arms) — exhaustive, `[T-Match]` gives `Result<i32,i32>`, matching `map_err`'s declared return type. Evaluated: `r = Err(4)` selects the `Err` arm, `e := 4`; `scale(4) = 40` (`[Arith-Checked]`, no overflow); `Err(40)` returned; outer `match`'s `Err(e) : e` → `40` |
| `conf.str-literal-type` | `str s = "hi"; str_len(s)` | `→ 2` | `[Str-Literal]`: `"hi"` decodes to `⟪104, 105⟫`, type `str` (`[T-Lit-Str]`; the annotation is compared, not propagated); `[Let]` → `[Store-Binding-Value]` (`¬is-resource(str)`); `str_len(s)`: `s` in value position → `[Read]` copies; `[Str-Len]` → 2 |
| `conf.str-copy-no-move` | `str s = "hi"; str t = s; str_len(s) + str_len(t)` | `→ 4` | `str t = s`: `s` in place position, `¬is-resource(str)` → `[Store-Binding-Place-Copy]`, not `-Transfer`: no `[Authority-Transfer]`, `a_s` stays valid, FA `valid(s) = T`; both `str_len` calls read their argument; `[Arith-Checked]` 2 + 2 |
| `conf.str-eq` | `("ab" == "ab") && ("ab" != "ba")` | `→ true` | `[T-Cmp]`: τ = `str` has `=_str` and is not a resource; `[Eq-Str]` bytewise: equal lengths and bytes → `true`; `[Cmp-Ne]` on unequal bytes → `true`; `[T-Logic]` |
| `conf.str-ordering-rejected` | `"a" < "b"` | ✗ `diag.type-mismatch` (static) | `[T-Cmp]`: `<` additionally requires τ numeric; `str` is not |
| `conf.str-byte` | `str_byte("hi", 1)` | `→ 105` | `1 : usize` from the parameter type (`rule.type.expected`); `[Str-Byte]`: `k = 1 < n = 2` → `b_1 = 105 : u8` |
| `conf.print-int` | `printf("%v", -42)` | ok | `[Print]`, `[Print-Int]`: writes `-42` |
| `conf.print-float` | `printf("%v", 0.1)` | ok | `[Print-Float]`: the shortest digits reading back as the f64 nearest 0.1 are `1`, k = −1 → `0.1` |
| `conf.print-string-ref` | `String s = String::from_str("x"); printf("%v", &s)` | ok | `ref<String, shared>` is printable; its bytes are written |
| `conf.print-string-by-value-rejected` | `String s = String::from_str("x"); printf("%v", s)` | ✗ `diag.type-mismatch` (static) | `[Print-Not-Printable]`: a `String` value is not printable (it would be moved into `print`) |
| `conf.print-not-printable` | `printf("%v", Some(1))` | ✗ `diag.type-mismatch` (static) | `[Print-Not-Printable]`: `Option<i32>` |
| `conf.print-generic-ok` | `fn show<T>(T x) { printf("%v", x); }` `show(2.5)` | ok | `rule.type.kind`: `%v` checked at the instantiation `T = f64`, printable |
| `conf.print-generic-not-printable` | `fn show<T>(T x) { printf("%v", x); }` `show(Some(1))` | ✗ `diag.type-mismatch` (static) | at the instantiation `T = Option<i32>`: `[Print-Not-Printable]` |
| `conf.print-output` | `impl/conformance/21-standard-library-semantics/print_output_ok.cb` | ok | `[Print-Int]`, `[Print-Float]` and the other cases of `text`; its exact output is checked by `impl/tests/print_output.rs` |
| `conf.read-line-typed` | `fn next() : Result<Option<String>, ReadError> { read_line() }` | ok | `read_line`'s type (`rule.stdlib.read`); never called, so no input is read |
| `conf.read-outside-unsafe-rejected` | `fn f(rawptr<u8> p) : isize { read(p, 1) }` | ✗ `diag.trusted-outside-unsafe` (static) | `read` is an `extern` (`[Read]`, an instance of `[Extern-Call]`): `[Unsafe-Rejected]` |
| `conf.read-line-lines` | `impl/conformance/21-standard-library-semantics/read_line_lines_ok.cb` | ok | input `ab\r\n\nc`: lines `ab\r` (only `\n` ends a line), `` (empty), `c` (ended by the end of input), then `Ok(None)` |
| `conf.read-line-end-of-input` | `impl/conformance/21-standard-library-semantics/read_line_end_of_input_ok.cb` | ok | empty input: `Ok(None)`, and `Ok(None)` again |
| `conf.read-line-invalid-utf8` | `impl/conformance/21-standard-library-semantics/read_line_invalid_utf8_ok.cb` | ok | input `a\xff\nok\n`: `Err(Utf8(e))`, `e.offset = 1`; the line was consumed, so the next call is `Ok(Some(ok))` |
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
| `conf.parse-int` | `String s = String::from_str("-42"); match (parse<i32>(&s)) { Ok(v) : v, Err(_) : 0 }` | `→ -42` | `[Parse]`: `-42` is a sentence of `number(i32)`, and −42 is a value of `i32` |
| `conf.parse-invalid-offset` | `String s = String::from_str("12x4"); usize k = match (parse<i32>(&s)) { Ok(_) : 9, Err(e) : match (e) { Invalid(k) : k, _ : 8 } }; k` | `→ 2` | `[Parse]`: the longest prefix that begins a number is `12`, so `Invalid(2)` |
| `conf.parse-out-of-range` | `String s = String::from_str("128"); bool r = match (parse<i8>(&s)) { Ok(_) : false, Err(e) : match (e) { OutOfRange : true, _ : false } }; r` | `→ true` | `[Parse]`: `128` is a sentence of `number(i8)`, but 128 is not a value of `i8` |
| `conf.parse-not-numeric` | `String s = String::new(); auto r = parse<bool>(&s);` | ✗ `diag.type-mismatch` (static) | `[Parse-Not-Numeric]`: `T = bool` |
| `conf.text-round-trip` | `impl/conformance/21-standard-library-semantics/text_append_parse_ok.cb` | ok | every integer type's limits and floats in each of `%v`'s forms: `parse<T>` of `text(v)` is `v`; each `ParseError`, with `Invalid`'s offsets |
| `conf.read-file-not-found` | `String p = String::from_str("/nonexistent-cobaltc-dir/x.txt"); bool nf = match (read_file(&p)) { Ok(_) : false, Err(e) : match (e) { NotFound : true, _ : false } }; nf` | `→ true` | `[Read-File]`: no such file |
| `conf.write-file-missing-dir` | `String p = String::from_str("/nonexistent-cobaltc-dir/x.txt"); String t = String::from_str("x"); bool nf = match (write_file(&p, &t)) { Ok(_) : false, Err(e) : match (e) { NotFound : true, _ : false } }; nf` | `→ true` | `[Write-File]`: a directory on the path does not exist |
| `conf.file-round-trip` | `impl/conformance/21-standard-library-semantics/files_round_trip_ok.cb` | ok | `write_file` then `read_file` give back the text; a second write replaces it; `NotFound` for a missing file and a missing directory; a fixture whose third byte is 0xFF is `Utf8(e)`, `e.offset = 2` |
| `conf.file-handle-round-trip` | `impl/conformance/21-standard-library-semantics/file_handle_round_trip_ok.cb` | ok | `[File-Open]` in each mode, `[File-Write]` of text and of bytes that are not text, `[File-Read-Line]` (and `Utf8(e)`, `e.offset = 2`, on the fixture), `[File-Read]` after `[File-Seek]`, `[File-Close]`, an `open_rw` write where reading stopped, `read_bytes`/`write_bytes`, `NotFound` |
| `conf.file-destructor-closes` | `impl/conformance/21-standard-library-semantics/file_destructor_closes_ok.cb` | ok | `[File-Drop]`: 2000 files opened and never closed by the program are each closed by the destructor; two open files keep their own positions |
| `conf.file-moved-to-thread` | `impl/conformance/21-standard-library-semantics/file_moved_to_thread_ok.cb` | ok | a `File` moves into a spawned thread, which writes and closes it |
| `conf.file-use-after-close-rejected` | `impl/conformance/21-standard-library-semantics/file_use_after_close_rejected.cb` | ✗ `diag.stale-binding` (static) | `close` takes the `File`: writing to it afterwards names a moved binding |
| `conf.file-forged-rejected` | `File f = File { .id = 0, .open = true };` | ✗ `diag.name-not-visible` (static) | a `File`'s fields are private to `std` (`[Field-Not-Visible]`) |
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
| `conf.write-err-private` | `unsafe { std::write_err(str_ptr("x"), 1); }` | ✗ `diag.name-not-visible` (static) | `[Resolve-Not-Visible]`: the extern `eprintf` writes through is private to `std` |
| `conf.hashmap-basic` | `HashMap<i32, bool> m = HashMap::new(); HashMap::insert(&mut m, 5, true); i32 k = 5; i32 j = 6; HashMap::len(&m) == 1 && HashMap::contains(&m, &k) && !HashMap::contains(&m, &j)` | `→ true` | `rule.stdlib.hashmap`: one entry; 5 found, 6 not |
| `conf.hashmap-insert-replaces` | `HashMap<str, i32> m = HashMap::new(); HashMap::insert(&mut m, "a", 1); i32 old = Option::unwrap_or(HashMap::insert(&mut m, "a", 2), 0); str k = "a"; i32 now = match (HashMap::get(&m, &k)) { Some(v) : *v, None : 0 }; old == 1 && now == 2 && HashMap::len(&m) == 1` | `→ true` | the second `insert` returns `Some(1)`; `get` gives 2; `len` stays 1 |
| `conf.hashmap-entry-counts` | `HashMap<u8, u32> m = HashMap::new(); for (u8 i = 0; i < 100; i += 1) { *HashMap::entry(&mut m, i % 7, 0) += 1; } HashMap::len(&m) == 7 && *HashMap::value_at(&m, 0) == 15 && *HashMap::key_at(&m, 6) == 6` | `→ true` | seven keys in insertion order 0..6; key 0 counted 15 times |
| `conf.hashmap-remove-swaps` | `HashMap<i32, i32> m = HashMap::new(); HashMap::insert(&mut m, 1, 10); HashMap::insert(&mut m, 2, 20); HashMap::insert(&mut m, 3, 30); i32 k = 1; i32 v = Option::unwrap_or(HashMap::remove(&mut m, &k), 0); v == 10 && *HashMap::key_at(&m, 0) == 3 && *HashMap::key_at(&m, 1) == 2` | `→ true` | returns `Some(10)`; the last entry (3) takes position 0: order 3, 2 |
| `conf.hashmap-remove-ordered` | `HashMap<i32, i32> m = HashMap::new(); HashMap::insert(&mut m, 1, 10); HashMap::insert(&mut m, 2, 20); HashMap::insert(&mut m, 3, 30); i32 k = 1; i32 v = Option::unwrap_or(HashMap::remove_ordered(&mut m, &k), 0); v == 10 && *HashMap::key_at(&m, 0) == 2 && *HashMap::key_at(&m, 1) == 3` | `→ true` | returns `Some(10)`; order 2, 3 |
| `conf.hashset-basic` | `HashSet<String> s = HashSet::new(); bool a = HashSet::insert(&mut s, String::from_str("x")); bool b = HashSet::insert(&mut s, String::from_str("x")); String k = String::from_str("x"); a && !b && HashSet::len(&s) == 1 && HashSet::remove(&mut s, &k) && HashSet::len(&s) == 0` | `→ true` | `insert` is true, then false; `remove` is true; `len` 1 then 0 |
| `conf.hashmap-float-key-rejected` | `HashMap<f64, i32> m = HashMap::new();` | ✗ `diag.type-mismatch` (static) | `[Key-Not-Hashable]`: `f64` is not a key type |
| `conf.hashmap-struct-key-rejected` | `struct P { i32 x; }` `HashSet<P> s = HashSet::new();` | ✗ `diag.type-mismatch` (static) | `[Key-Not-Hashable]`: a struct is not a key type |
| `conf.hashmap-generic-key-checked` | `fn keep<K>(K k) { HashSet<K> s = HashSet::new(); HashSet::insert(&mut s, k); }` `keep(1.5)` | ✗ `diag.type-mismatch` (static) | `[Key-Not-Hashable]` at the instantiation `K = f64` |
| `conf.hashmap-fields-private` | `HashMap<i32, i32> m = HashMap::new(); usize n = Vec::len(&m.keys);` | ✗ `diag.name-not-visible` (static) | `HashMap`'s fields are private to `std` |
| `conf.key-hash-std-only` | `i32 k = 1; key_hash(&k)` | ✗ `diag.unbound-name` (static) | `key_hash` is a std-only intrinsic |
| `conf.hashmap-output` | `impl/conformance/21-standard-library-semantics/hashmap_ok.cb` | ok | insertion order, both removals, growth and resource destruction, output pinned |
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
| `conf.match-by-ref-replace-while-bound-rejected` | `Option<i32> o = Some(1); match (&o) { Some(n) : { o = None; i32 k = *n; }, None : {}, }` | ✗ `diag.aliasing-conflict` (dynamic) | `o` is written while `n`, a reference into it, is still used |
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
| `conf.slice-push-while-borrowed-rejected` | `Vec<i32> v = Vec::new(); Vec::push(&mut v, 1); auto s = &v[0..1]; Vec::push(&mut v, 2);` | ✗ `diag.aliasing-conflict` (static) | the slice keeps `v` borrowed: no push while it lives; a slice binding is a borrow of its source to `rule.control.flow-analysis`, as `auto r = &v;` is |
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
| `conf.overwrite-some-of-resource-option-dynamic` | `Option<String> o = Some(String::from_str("a")); o = Some(String::from_str("b"));` | ✗ `diag.overwrite-of-live-resource` (dynamic) | a `Some(String)` owns its `String`; `Option<String>` has a variant owning nothing, so the static pass leaves it to the value |
| `conf.overwrite-owner-type-still-static` | `struct W { Option<String> s; } fn W::drop(ref<W, exclusive> self) { }` `W w = W { .s = None }; w = W { .s = None };` | ✗ `diag.overwrite-of-live-resource` (static) | a type with a destructor always owns (its destructor is the obligation), whatever its fields hold |
| `conf.match-wildcard-keeps-scrutinee` | `Option<String> o = Some(String::from_str("abc")); match (o) { None : {}, _ : {}, } match (&o) { Some(s) : String::len(s) == 3, None : false }` | `→ true` | D-0049: an arm that moves nothing out leaves the scrutinee with its owner |
| `conf.match-move-arm-consumes` | `Option<String> o = Some(String::from_str("a")); match (o) { Some(s) : {}, None : {}, } match (&o) { Some(s) : {}, None : {}, }` | ✗ `diag.stale-binding` (dynamic) | the `Some(s)` arm moved the payload out and consumed `o`; after the match `o` is valid on one path only, so the check is dynamic |
| `conf.str-byte-out-of-bounds` | `fn at(str s, usize i) : u8 { str_byte(s, i) }` `at("hi", 2)` | ✗ `diag.index-out-of-bounds` (dynamic) | `i` is a parameter → FA unknown → the `checked` guard runs; `k = 2 ≥ n = 2` → `[Str-Byte-Out-Of-Bounds]` |
| `conf.byte-literal-array` | `array<u8, 3> b = b"hi\n"; b[2]` | `→ 10` | `b"hi\n"` is `[104:u8, 105:u8, 10:u8]` (`spec/16` §3) → `[Array-Construct]`, type `array<u8,3>` matching the annotation; temp adopted; `2 : usize` from the index position; `[Index-Checked]` → 10 |
| `conf.string-from-str` | `String s = String::from_str("hi"); String::len(&s)` | `→ 2` | `[Str-Len]` → `n = 2`; loop: `str_byte(s, 0)`, `str_byte(s, 1)` (`[Str-Byte]`, in range) each pushed (`grow` 0→4 on the first: `[Allocate]`; `[Rawptr-Write]` twice); `String { .bytes = v }` → `[Store-Sub-Relocate]` (`v`'s obligation re-keyed); `inv.string.utf8-validity` holds by `inv.str.utf8-validity` — no validator ran; result temp adopted by `s`; `String::len(&s)` → `Vec::len` → 2 |
| `conf.str-in-vec` | `Vec<str> v = Vec::new(); Vec::push(&mut v, "a"); Vec::push(&mut v, "bc"); str_len(*Vec::index_shared(&v, 1))` | `→ 2` | `T := str` from the annotation (`[Generic-Call-Expected-Type]`); each `push`: `[Rawptr-Write]` of `represent(str, ·)` (`[Repr-Str]`, `[Sizeof-Str]`; `¬is-resource(str)`, so a write not a move-in); `index_shared(&v, 1)` → `[Reclaim]` establishes element 1 (plain: no obligation) → `[Borrow]` shared; `*` reads `value-at(str, …)` back → `⟪98, 99⟫`; `[Str-Len]` → 2 |
| `conf.str-not-ffi` | `extern fn put(str s);` | ✗ `diag.extern-non-ffi-type` (static) | `str ∉ FfiType` (`spec/20` §3) → `[Extern-Non-Ffi-Type]` on the declaration |
| `conf.print-observed` | `printf("hi\n")` | `ok`; the bytes `104 105 10` written | `[Printf]`: no specifiers, so `format` is the literal's bytes and the call is `print(s)` of them; `[Str-Literal]` → `⟪104, 105, 10⟫`; `[Call]` binds `s` by `[Store-Binding-Value]`; `str_ptr(s)` → `[Str-Ptr]`: `p : rawptr<u8>` with `storage(p..p+3)` holding the bytes outside every object's extent (safe: no path is affected); `str_len(s)` → 3; `write(p, 3)` occurs lexically inside `unsafe { }`, satisfying `[Unsafe-Rejected]`'s guard; `[Extern-Call]` is `disposition: trusted-unchecked` — the author's assertion that `write` reads exactly those 3 cells is discharged nowhere; `claim : isize`, `Σ.trust(claim) := unchecked-claim`, discarded at the statement's SE (plain value); `print`, and so `printf`, returns `()`; the caller's SE discards it; terminates `ok` |
| `conf.rc-last-drop-frees` | `… drop(a); drop(b);` | `count` reaches 0; `deallocate` called once | `drop(a)` → `Rc::drop`: `reclaim<RcBox<i32>>(self.ptr)` re-attaches `o'` (established by `clone`'s reclaim above); `count = 2 − 1 = 1`; `last = false`; `b`'s block ends. `drop(b)` → `Rc::drop`: reclaim re-attaches `o'` again; `count = 0`; `last = true`; the counting block's borrow ends; `drop(reclaim<RcBox<i32>>(self.ptr))`: reclaim re-attaches `o'` once more (still alive, solitary — nothing else reaches it); `¬is-resource(RcBox<i32>)` → `[Destroy-Plain]` (not `[Destroy]`: no authority to consume, no composite obligations to run — unlike `RcBox<Vec<i32>>`'s `[Destroy]` in `conf.e2e-rc-resource-payload`, which does both): `solitary(a')` holds, `end-object(o')` runs directly; `deallocate(self.ptr, sizeof<RcBox<i32>>(), alignof<RcBox<i32>>())` → `[Release]` finds no reclaimed object left (just ended) |

## 12. Concurrency (`spec/19`)

| id | Fragment | Outcome | Derivation |
|---|---|---|---|
| `conf.spawn-join-value` | `fn work(i32 n) : i32 { n * 2 }` `auto h = spawn(work, 21); join(h)` | `→ 42` | `[Spawn]` temp handle adopted (`[Repr-Handle]`); `join(h)`: `h` in place position; `[Join]` waits, marks the result `taken`, `[Destroy]` the handle (`[Handle-Destructor]` discards nothing), returns `r_b` |
| `conf.unjoined-handle-waits` | `{ auto h = spawn(work, 1); }` | `ok`; thread finished before BE completes | `[Handle-Destructor]` |
| `conf.spawn-borrow-closure-rejected` | `i32 x = 1; auto h = spawn([x]() { x + 1 });` | ✗ `diag.spawn-borrow-closure` (static) | `rule.conc.spawn` |
| `conf.cross-thread-write-conflict` | `fn w(ref<i32, shared> r) : i32 { *r }` `i32 x = 1; auto h = spawn(w, &x); x = 2; join(h)` | ✗ `diag.aliasing-conflict` (dynamic) | `x = 2`: the spawned thread's parameter object holds the borrow (`a'` valid, shared, not an ancestor) until it finishes; FA `escaped(x)` → unknown; outcome depends on interleaving only in *which* step reports it — every interleaving in which the write precedes the thread's exit faults, and one in which the thread has already finished succeeds: `outcome: unspecified { ok, diag.aliasing-conflict }` per `[Thread-Step]` |
| `conf.mutex-lock-unlock` | `auto m = Mutex::new(0); { auto g = lock(&m); *g = *g + 1; } *lock(&m)` | `→ 1` | `[Lock]` path `base := a_m'`; `*g`: `g` in place position → `[Guard-Deref]`; write: `a_m`, `a_m'` ancestors; BE → `[Destroy]` of the guard → `[Guard-Drop]` (built-in `destructor`, `spec/07` §1); second lock succeeds; `*lock(&m)` derefs the temporary guard, which ends at SE |
| `conf.mutex-reentrant-rejected` | `auto m = Mutex::new(0); auto g = lock(&m); auto g2 = lock(&m);` | ✗ `diag.mutex-reentrant-lock` (dynamic) | `[Lock-Reentrant]` |
| `conf.thread-vec-of-resources-dropped` | `fn consume(Vec<Vec<i32>> vv) : usize { Vec::len(&vv) }` `Vec<Vec<i32>> vv = Vec::new(); Vec<i32> inner = Vec::new(); Vec::push(&mut inner, 1); Vec::push(&mut vv, inner); auto h = spawn(consume, vv); join(h)` | `→ 1` | `Vec::push(&mut vv, inner)`: `[Rawptr-Move-In]` makes element 0 a top-level reclaimed object, its authority re-keyed from `inner` — main's. `spawn`: `[Authority-Transfer]` moves `vv`'s own authority to `ℓ'` (not the element's: nothing in `Σ` links them). In `ℓ'`, `vv`'s BE → `Vec::drop` → `drop(reclaim<Vec<i32>>(…))`: `[Reclaim]` re-attaches element 0 and takes its authority for `ℓ'` (`CHG-0030`), so `[Destroy]`'s `authority(ℓ', destroy, o)` holds; `inner`'s buffer deallocated, then `vv`'s |
| `conf.thread-vec-of-resources-returned` | `fn make() : Vec<Vec<i32>> { Vec<Vec<i32>> vv = Vec::new(); Vec::push(&mut vv, Vec::new()); vv }` `auto h = spawn(make); auto vv = join(h); Vec::len(&vv)` | `→ 1` | the element is established in `ℓ'`; `[Join]` re-keys `vv`'s top-level authority to main (`CHG-0015`), not the element's; main's BE → `Vec::drop` → `[Reclaim]` re-attaches the element and takes its authority (`CHG-0030`) → `[Destroy]` holds |
| `conf.mutex-shared-across-threads` | `fn inc(ref<mutex<i32>, shared> m) { auto g = lock(m); *g = *g + 1; }` `auto m = Mutex::new(0); auto h = spawn(inc, &m); { auto g = lock(&m); *g = *g + 1; } join(h); *lock(&m)` | `→ 2` | both threads hold shared references to `o_m`; each lock path's writes are `sync-exempt` from the other thread's shared reference; `[Lock]` serializes |

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
| `conf.e2e-hello-print` | `ex.e2e-hello-print` | `ok`; the bytes of `"Hello, CobaltC!\n"` written, in three `write` calls | `[Program]` → `main`. `str who = "CobaltC";`: `[Str-Literal]` → `⟪67, 111, 98, 97, 108, 116, 67⟫ : str`; `[Let]` → `[Store-Binding-Value]` into `o_who` (plain). `String::from_str(who)`: `who` in value position → `[Read]` copies (`¬is-resource(str)`; `a_who` untouched); as `conf.string-from-str` with `n = 7`: `grow` 0→4 at the first push, 4→8 at the fifth (`[Allocate]`, `copy_raw` of 4 × 1 byte, `[Deallocate]` → `[Release]` ends the four reclaimed element objects `push` never established — vacuous), seven `[Rawptr-Write]`s; `String { .bytes = v }` relocates; temp adopted by `owned` (`is-resource(String)` by derivation: authority and obligation `{o_owned, (o_owned, bytes)}`). `String::len(&owned)` → 7; `str_len(who)` → 7 (`[Read]` again, still no move); `[Cmp-Ne]` → `false` → `[If-No-Else]` skips the never-reached fault. Three `printf` calls, each as `conf.print-observed`: `[Str-Ptr]` widens `storage` with 7, 7, and 2 bytes respectively (the second call's `who` is `[Read]` a third time), `[Extern-Call]` inside `std`'s `unsafe` block, three `isize` claims discarded. Main BE: `Owned = {o_who, o_owned}` descending origin → `o_owned` first: `[Destroy]` (solitary: every `&owned` borrow died with `len`'s parameter object) → `destructor(String) = None` → `[Run-Destructor-None]` → `[Destroy-Composite]` on `(o_owned, bytes)` → `Vec::drop`: reclaims and reads seven plain `u8` elements (no obligations), `[Deallocate]` of the 8-slot buffer; `[Object-End]` of `o_owned`; then `o_who` ends (plain, `[Object-End]`). `[Terminate-Ok]`. Observable behavior (`rule.fn.program`): outcome `ok` and the `extern` sequence `write(·,7), write(·,7), write(·,2)` whose cells hold the sixteen bytes of `"Hello, CobaltC!\n"` in order |
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
| `conf.static-assert-not-constant` | `i32 k = 3; static_assert(k > 1);` | ✗ `diag.const-not-constant` (static) | `[Static-Assert-Not-Constant]`: a local is not a constant expression |
| `conf.static-assert-overflow` | `static_assert(max_value<i32>() + 1 > 0);` | ✗ `diag.arith-overflow` (static) | `[Static-Assert-Checked-Failure]`: computed as at run time, before it |
| `conf.static-assert-not-bool` | `static_assert(5);` | ✗ `diag.type-mismatch` (static) | the condition is a `bool` |
| `conf.view-basic` | `String line = String::from_str("  hello, world  "); StringView w = &line[2..7]; StringView r = &w[1..$]; StringView::len(w) == 5 && StringView::len(r) == 4 && StringView::len(&line[0..$]) == 16` | `→ true` | D-0053 `[View-Form]`: a view of a `String`, and of a view; `$` is the length in bytes |
| `conf.view-eq` | `String s = String::from_str("abcabc"); StringView a = &s[0..3]; StringView b = &s[3..6]; a == b && a == "abc" && "abc" == b && a != "abd"` | `→ true` | `[View-Eq]`: with a view or a `str`, either side |
| `conf.view-split-trim` | `String s = String::from_str(" a, bb ,c "); Vec<StringView> p = StringView::split(StringView::trim(&s[0..$]), ","); Vec::len(&p) == 3 && StringView::trim(p[1]) == "bb" && p[2] == "c"` | `→ true` | `split` and `trim`: every part is a view of the same `String` |
| `conf.view-parse` | `String s = String::from_str("x=42"); match (StringView::parse<i64>(&s[2..$])) { Ok(v) : v == 42, Err(_) : false }` | `→ true` | `parse<T>` of a view |
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

## Change Log

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
