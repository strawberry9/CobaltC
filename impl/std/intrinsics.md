# The language's built-in functions

These functions are not items of `std`: they are the language's intrinsics, in scope in every program
without an `import`, and a call of one is defined by its rule in the specification rather than by a body
(`spec/21` §0, "Language intrinsics"). `--doc NAME` shows each, and `--doc language` lists them (CHG-0451);
`impl/tools/cobdoc.py --check` fails when the tools' list of the intrinsics a program may call
(`impl/cobc/summary.cb`'s `INTRINSIC_NAMES`, less those only `std` may call and those `builtins.md`
documents) and the entries below differ.

An entry is a heading naming the function, a `Signature:` line, and Markdown text whose first sentence is
the summary. `T` and `U` stand for the types the text names.

## `drop` in `the language`
Signature: `fn drop<T>(T x)`

Destroys `x` now, before the end of its scope (`rule.resauth.destroy`). The argument is a place, so `drop(v)`
ends the binding `v`; a temporary is destroyed directly. A resource's destructor runs, its memory is freed,
and any later use of the binding is `diag.stale-binding`.

## `clone` in `the language`
Signature: `fn clone<T: clone>(ref<T, shared> x) : T`

A copy of the value `x` refers to, made by its type's `clone` (D-0116). A struct or enum whose fields all
clone has one derived; a `String`, `Vec` or `Box` copies what it owns.

## `sizeof` in `the language`
Signature: `fn sizeof<T>() : usize`

The size of a value of type `T` in bytes, as the implementation lays it out (`rule.arith.sizeof`). Known
when the program is compiled, so it may stand in a constant.

## `alignof` in `the language`
Signature: `fn alignof<T>() : usize`

The alignment of type `T` in bytes (`rule.arith.sizeof`).

## `min_value` in `the language`
Signature: `fn min_value<T>() : T`

The least value of `T`, an integer type, `f32` or `f64` (`rule.arith.limits`): `min_value<i32>()` is
-2147483648. For a float it is the most negative finite value.

## `max_value` in `the language`
Signature: `fn max_value<T>() : T`

The greatest value of `T`, an integer type, `f32` or `f64` (`rule.arith.limits`): `max_value<u8>()` is 255.
For a float it is the greatest finite value.

## `widen` in `the language`
Signature: `fn widen<U>(T x) : U`

`x` as a value of the wider type `U`, which never fails (`rule.arith.convert`). It is accepted only where
`U` holds every value of `T`: `u8` to `u32` or `i16`, `i32` to `i64`, `f32` to `f64`. The other way is
`narrow`.

## `narrow` in `the language`
Signature: `fn narrow<U>(T x) : U`

`x` as a value of the integer type `U`, checked: a value `U` cannot hold faults with
`diag.arith-overflow` (`rule.arith.convert`). `checked_narrow` gives an `Option` instead, and
`narrow_wrapping` keeps the low bits.

## `narrow_wrapping` in `the language`
Signature: `fn narrow_wrapping<U>(T x) : U`

`x` as a value of the integer type `U`, keeping its low bits, as C's cast does (`rule.arith.convert`).
Never fails.

## `checked_narrow` in `the language`
Signature: `fn checked_narrow<U>(T x) : Option<U>`

`Some(x)` as a value of the integer type `U` when `U` holds it, else `None` (`[Checked-Narrow]`, D-0122).

## `reinterpret` in `the language`
Signature: `fn reinterpret<U>(T x) : U`

The bits of `x` read as a value of `U`, a type of the same size: an integer's bits as the other signedness
(`u32` to `i32`), a float's bits as an integer or back (`rule.arith.convert`). Never fails.

## `to_float` in `the language`
Signature: `fn to_float<U>(T x) : U`

The integer `x` as the float type `U` (`f32` or `f64`), rounded to the nearest representable value
(`rule.arith.convert`).

## `to_int` in `the language`
Signature: `fn to_int<U>(T x) : U`

The float `x` as the integer type `U`, its fraction dropped (toward zero), checked: a NaN, an infinity or a
value `U` cannot hold faults with `diag.arith-overflow` (`rule.arith.convert`).

## `wrapping_add` in `the language`
Signature: `fn wrapping_add<T>(T a, T b) : T`

`a + b` kept to the width of `T`, wrapping around instead of faulting (`rule.arith.alt`). Also lane by lane
on two `simd` values of integer lanes.

## `wrapping_sub` in `the language`
Signature: `fn wrapping_sub<T>(T a, T b) : T`

`a - b` kept to the width of `T`, wrapping around instead of faulting (`rule.arith.alt`).

## `wrapping_mul` in `the language`
Signature: `fn wrapping_mul<T>(T a, T b) : T`

`a * b` kept to the width of `T`, wrapping around instead of faulting (`rule.arith.alt`).

## `saturating_add` in `the language`
Signature: `fn saturating_add<T>(T a, T b) : T`

`a + b`, or the nearest of `min_value<T>()` and `max_value<T>()` when it does not fit (`rule.arith.alt`).

## `saturating_sub` in `the language`
Signature: `fn saturating_sub<T>(T a, T b) : T`

`a - b`, or the nearest limit of `T` when it does not fit (`rule.arith.alt`).

## `saturating_mul` in `the language`
Signature: `fn saturating_mul<T>(T a, T b) : T`

`a * b`, or the nearest limit of `T` when it does not fit (`rule.arith.alt`).

## `checked_add` in `the language`
Signature: `fn checked_add<T>(T a, T b) : Option<T>`

`Some(a + b)`, or `None` when it does not fit in `T` (`rule.arith.alt`).

## `checked_sub` in `the language`
Signature: `fn checked_sub<T>(T a, T b) : Option<T>`

`Some(a - b)`, or `None` when it does not fit in `T` (`rule.arith.alt`).

## `checked_mul` in `the language`
Signature: `fn checked_mul<T>(T a, T b) : Option<T>`

`Some(a * b)`, or `None` when it does not fit in `T` (`rule.arith.alt`).

## `checked_div` in `the language`
Signature: `fn checked_div<T>(T a, T b) : Option<T>`

`Some(a / b)`, or `None` when `b` is zero or the quotient does not fit in `T` (`rule.arith.alt`).

## `checked_rem` in `the language`
Signature: `fn checked_rem<T>(T a, T b) : Option<T>`

`Some(a % b)`, or `None` when `b` is zero or the remainder does not fit in `T` (`rule.arith.alt`).

## `checked_neg` in `the language`
Signature: `fn checked_neg<T>(T a) : Option<T>`

`Some(-a)`, or `None` when it does not fit: the most negative value of a signed type, and every non-zero
unsigned value (`checked_sub(0, a)`, D-0072).

## `count_ones` in `the language`
Signature: `fn count_ones(T x) : u32`

How many bits of the integer `x` are 1 (`[Count-Ones]`, D-0123).

## `leading_zeros` in `the language`
Signature: `fn leading_zeros(T x) : u32`

How many 0 bits the integer `x` has above its highest 1, in its own width (`[Leading-Zeros]`, D-0123).

## `trailing_zeros` in `the language`
Signature: `fn trailing_zeros(T x) : u32`

How many 0 bits the integer `x` has below its lowest 1, in its own width (`[Trailing-Zeros]`, D-0123).

## `rotate_left` in `the language`
Signature: `fn rotate_left(T x, u32 k) : T`

The bits of the integer `x` rotated left by `k`, those that leave at the top coming back at the bottom
(`[Rotate]`, D-0123).

## `rotate_right` in `the language`
Signature: `fn rotate_right(T x, u32 k) : T`

The bits of the integer `x` rotated right by `k` (`[Rotate]`, D-0123).

## `str_len` in `the language`
Signature: `fn str_len(str s) : usize`

The length of the text constant `s` in bytes (`[Str-Len]`). A `StringView`'s is `StringView::len(v)`.

## `str_byte` in `the language`
Signature: `fn str_byte(str s, usize i) : u8`

Byte `i` of the text constant `s`, checked: `diag.index-out-of-bounds` past its end (`[Str-Byte]`).

## `str_ptr` in `the language`
Signature: `fn str_ptr(str s) : rawptr<u8>`

The address of the text constant `s`'s first byte, for a C function (`[Str-Ptr]`); safe to take.

## `slice_len` in `the language`
Signature: `fn slice_len(slice<T, m> s) : usize`

The number of elements of the slice `s` (`rule.agg.slice`, D-0047), also through a reference to one. A
`Vec`'s is `Vec::len(&v)`, a `StringView`'s `StringView::len(v)`.

## `static_assert` in `the language`
Signature: `fn static_assert(bool c, str message)`

Rejects the program, before it runs, when the constant expression `c` is false, with `message` if given
(`rule.module.static-assert`). Nothing happens at run time.

## `fault` in `the language`
Signature: `fn fault(d, str message) : never`

Ends the program (or the thread) with the dynamic diagnostic `d`, written with `_` for `-`
(`index_out_of_bounds`, `unwrap_failed`), and the message if given (`[Fault]`, D-0083). Typed `never`, so
it stands where any type is expected; `assert(c)` is the usual way to fault.

## `simd_load` in `the language`
Signature: `fn simd_load<V>(slice<T, m> s, usize i) : V`

The `simd` value `V` whose lanes are `s[i]` to `s[i+N-1]`, checked against the slice's length (D-0254,
`rule.agg.simd`); any alignment.

## `simd_store` in `the language`
Signature: `fn simd_store(slice<T, exclusive> s, usize i, V v)`

Writes the lanes of `v` to `s[i]` to `s[i+N-1]`, checked against the slice's length (D-0254).

## `simd_eq` in `the language`
Signature: `fn simd_eq(V a, V b) : M`

The mask whose lane `i` is -1 when `a[i] == b[i]` and 0 otherwise (D-0254).

## `simd_ne` in `the language`
Signature: `fn simd_ne(V a, V b) : M`

The mask whose lane `i` is -1 when `a[i] != b[i]` and 0 otherwise (D-0254).

## `simd_lt` in `the language`
Signature: `fn simd_lt(V a, V b) : M`

The mask whose lane `i` is -1 when `a[i] < b[i]` and 0 otherwise (D-0254).

## `simd_le` in `the language`
Signature: `fn simd_le(V a, V b) : M`

The mask whose lane `i` is -1 when `a[i] <= b[i]` and 0 otherwise (D-0254).

## `simd_gt` in `the language`
Signature: `fn simd_gt(V a, V b) : M`

The mask whose lane `i` is -1 when `a[i] > b[i]` and 0 otherwise (D-0254).

## `simd_ge` in `the language`
Signature: `fn simd_ge(V a, V b) : M`

The mask whose lane `i` is -1 when `a[i] >= b[i]` and 0 otherwise (D-0254).

## `simd_select` in `the language`
Signature: `fn simd_select(M m, V a, V b) : V`

The value whose lane `i` is `a[i]` where the mask's lane is not 0, else `b[i]` (D-0254).

## `simd_any` in `the language`
Signature: `fn simd_any(M m) : bool`

Whether some lane of the mask `m` is not 0 (D-0254).

## `simd_all` in `the language`
Signature: `fn simd_all(M m) : bool`

Whether every lane of the mask `m` is not 0 (D-0254).

## `simd_min` in `the language`
Signature: `fn simd_min(V a, V b) : V`

The value whose lane `i` is the lesser of `a[i]` and `b[i]`; a NaN in `a` gives `b[i]` (D-0254).

## `simd_max` in `the language`
Signature: `fn simd_max(V a, V b) : V`

The value whose lane `i` is the greater of `a[i]` and `b[i]`; a NaN in `a` gives `b[i]` (D-0254).

## `simd_sum` in `the language`
Signature: `fn simd_sum(V v) : T`

The sum of the lanes, left to right, as `+` adds them (an integer sum is checked) (D-0254).

## `simd_reduce_min` in `the language`
Signature: `fn simd_reduce_min(V v) : T`

The least lane, taken left to right by `simd_min`'s rule (D-0254).

## `simd_reduce_max` in `the language`
Signature: `fn simd_reduce_max(V v) : T`

The greatest lane, taken left to right by `simd_max`'s rule (D-0254).

## `dangling` in `the language`
Signature: `fn dangling<T>() : rawptr<T>`

A non-null address aligned for `T` that is never read or written: a placeholder for an empty allocation
(spec/21 §0).

## `null_rawptr` in `the language`
Signature: `fn null_rawptr<T>() : rawptr<T>`

Address 0, C's `NULL`, for a C function that takes or returns one (D-0252); safe. No object has it.

## `rawptr_of` in `the language`
Signature: `fn rawptr_of<T>(ref<T, m> r) : rawptr<T>`

The address a reference holds, as a raw pointer (`[Rawptr-Of]`); safe to take, used inside `unsafe`.

## `reinterpret_ptr` in `the language`
Signature: `fn reinterpret_ptr<U>(rawptr<T> p) : rawptr<U>`

The same address as a pointer to `U` (spec/21 §0); safe.

## `rawptr_from_address` in `the language`
Signature: `fn rawptr_from_address<T>(usize a) : rawptr<T>`

The integer `a` as an address, inside `unsafe`; nothing is checked (`[Rawptr-From-Address]`, D-0258).

## `rawptr_address` in `the language`
Signature: `fn rawptr_address<T>(rawptr<T> p) : usize`

The address `p` holds, as an integer, inside `unsafe` (`[Rawptr-Address]`, D-0258).

## `reclaim` in `the language`
Signature: `fn reclaim<T>(rawptr<T> p) : T`

The place of type `T` at `p`, inside `unsafe` (`[Reclaim]`): what `*` on a reference is, for a raw pointer
the program vouches for.

## `release` in `the language`
Signature: `fn release(rawptr<u8> p, usize n)`

Ends the `n` bytes at `p` as objects, without freeing their memory, inside `unsafe` (`[Release]`).

## `copy_raw` in `the language`
Signature: `fn copy_raw(rawptr<u8> dst, rawptr<u8> src, usize n)`

Copies `n` bytes from `src` to `dst`, which do not overlap, inside `unsafe` (`[Copy-Raw]`).

## `allocate` in `the language`
Signature: `fn allocate(usize n, usize align) : Result<rawptr<u8>, AllocError>`

`n` bytes of new memory aligned to `align`, or the error when there is none (`[Allocate]`). Safe; using
the memory needs `unsafe`.

## `deallocate` in `the language`
Signature: `fn deallocate(rawptr<u8> p, usize n, usize align)`

Frees memory `allocate` gave, with the same size and alignment, inside `unsafe` (`[Deallocate]`).

## `read_volatile` in `the language`
Signature: `fn read_volatile<T>(rawptr<T> p) : T`

Reads the value at `p` exactly once, never merged or removed by the compiler, inside `unsafe`
(`[Rawptr-Read-Volatile]`, D-0121); `T` is not a resource.

## `write_volatile` in `the language`
Signature: `fn write_volatile<T>(rawptr<T> p, T v)`

Writes `v` at `p` exactly once, never merged or removed by the compiler, inside `unsafe`
(`[Rawptr-Write-Volatile]`, D-0121); `T` is not a resource.

## `Mutex::new` in `the language`
Signature: `fn Mutex::new<T>(T v) : mutex<T>`

A new mutex holding `v` (`rule.conc.lock`). Threads share it by reference and reach its value through
`lock`.

## `lock` in `the language`
Signature: `fn lock<T>(ref<mutex<T>, shared> m) : guard<T>`

Waits until no other thread holds `m`, then holds it: the guard reads and writes the value (`g.f`, `*g`)
and lets the mutex go when it is destroyed, at the end of its scope (`rule.conc.lock`). A thread that
faults while holding it poisons it.

## `spawn` in `the language`
Signature: `fn spawn(fn(T1, …, Tn) : R f, T1 a1, …, Tn an) : handle<R>`

Runs `f(a1, …, an)` on a new thread and returns at once with its handle (`rule.conc.spawn`). The arguments
are moved to the thread or lent to it until `join`; `f` is a function or a `move` closure. `join(h)` waits
for the result.

## `join` in `the language`
Signature: `fn join<T>(handle<T> h) : T`

Waits for the thread `h` to end and gives its result; a fault in the thread is raised again here
(`rule.conc.join`, D-0204). `try_join` gives the fault as a value instead.

## `cancel` in `the language`
Signature: `fn cancel<T>(ref<handle<T>, shared> h)`

Asks the thread `h` to stop at its next wait (`rule.conc.cancel`, D-0204); `join` then raises
`diag.thread-cancelled`.

## `join_status` in `the language`
Signature: `fn join_status<T>(ref<handle<T>, shared> h, u8 mode) : u64`

The thread's state, the primitive under `try_join` and `is_finished` (D-0204): 0 while it runs, 1 when it
ended with a result, `k + 2` when it failed with failure `k`. Mode 0 first waits for the end, mode 1 only
looks.
