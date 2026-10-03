# CobaltC Standard Library Semantics

Status: normative artifact
Version: 4.5.0
Conforms to: `spec/02-schema.md` (Kind: Rule, `rule.stdlib.*`; Kind: Type, `type.str`)
Governed by: `CobaltC_Master_Instructions.md` §1, §23

## Purpose and scope

Master Instructions §1 scopes the design to the language. This
artifact fixes the **prelude**: the intrinsics every conforming
implementation provides (operations whose semantics are rules in
`spec/06`, `spec/07`, `spec/20`), the prelude types (`Option`,
`Result`), the text value type `str` (§2a), and the library types —
`Vec<T>`, `String`, `HashMap<K, V>`, `HashSet<K>`, `Queue<T>`, `PriorityQueue<T>`, `Rc<T>`, `Box<T>` — written in CobaltC
itself as evidence that the language as specified is sufficient. Their bodies are normative for
their observable behavior; an implementation may realize them
differently provided every conformance case in `spec/conformance.md`
holds.

**The module `std`** (`CHG-0033`, D-0024). Every declaration this
artifact gives in CobaltC — the types and functions of §0–§3d, the
externs `stdout_write`, `stdin_read` and `arg_bytes`, and
`Result::map_err` — is an item of the module `std`, declared at
the root of every program, or of one of its submodules (D-0136,
`CHG-0164`). Its names are `std::Vec`, `std::Vec::push`, `std::printf`,
…; a module uses them unqualified after `import std;` (or a single-name
import such as `import std::Vec;`), and any module may write them
qualified. Being a module, `std` keeps what it does not export:
`Vec`'s, `String`'s and `Rc`'s fields, `RcBox`, and `Vec::grow` are
reachable only from inside it. §0's tables say which names are which:
those headed **Language intrinsics** belong to the language and need no
import (with the type `str`); those headed **`std`** are items of `std`
(D-0134). An intrinsic is not an item and its name is not reserved: a
name that resolves to a local binding (`rule.value-object.binding-lookup`)
or to an item (`[Resolve-Unqualified]`, `spec/17`) means that binding or
item, and the intrinsic of the same name is reached only where nothing
else is (`CHG-0036`). An item of an intrinsic's name is found only in
its own module and where an `import` names it, never through an
enclosing module (`[Resolve-Unqualified]` (3), D-0055), so `std`'s own
code always reaches the intrinsics. A program may not declare its own
root-level `std` (`[Item-Duplicate]`).

**`std`'s submodules** (D-0136). `std` is divided by subject. Each
submodule declares the items of the sections listed, and `std`'s root
re-exports every one of them (`export import`, `spec/17` §3):

    export module std
    {
        -- the primitives private to std (§2b, §2e, §2e′, §3c)
        export module core "core.cb";
        export module collections "collections.cb";
        export module text "text.cb";
        export module io "io.cb";
        export module sys "sys.cb";
        export module memory "memory.cb";
        export module sync "sync.cb";
        export module random "random.cb";
        export module math "math.cb";
        export import std::core;
        export import std::collections;
        export import std::text;
        export import std::io;
        export import std::sys;
        export import std::memory;
        export import std::sync;
        export import std::random;
        export import std::math;
    }

| Submodule | Items | Sections |
|---|---|---|
| `std::core` | `Option`, `Result`, `AllocError`, `assert`, `swap`, `replace`, `overwrite`, `min`, `max`, `abs`, `pow` | §0, §3b |
| `std::collections` | `Vec` (with sorting, `Vec::push_le`/`push_be`), `HashMap`, `HashSet`, `Queue`, `PriorityQueue`, `slice_swap` | §1, §1b, §2g, §2i |
| `std::text` | `String`, `StringView`, `Utf8Error`, `ParseError`, the `ascii_` functions, `read_le`/`read_be`, `crc32`, `sprintf`, `String::appendf` | §2, §2a, §2d, §2f, §2h |
| `std::io` | `printf`, `eprintf`, `stdout_write`, `stdin_read`, `read_line`, `FileError`, `File`, `read_file`, `write_file`, `read_bytes`, `write_bytes` | §2b, §2e, §2f |
| `std::sys` | `arg_count`, `arg`, `arg_bytes`, the directory functions, `PathKind`, `env_var`, `current_dir`, `monotonic_ns`, `unix_seconds`, `sleep_ms` | §2c, §2e′ |
| `std::memory` | `Box`, `Rc`, `Weak` | §3, §3a |
| `std::sync` | `Channel` | §3c |
| `std::random` | `Rng` | §0 |
| `std::math` | `sqrt`, `floor`, `ceil`, `round`, `trunc`, `ln`, `exp`, `log2`, `log10`, `sin`, `cos`, `tan`, `atan2`, `powf` | §3d |

So `import std;` brings in the whole library, as it did before the
division, and `std::collections::Vec` is `std::Vec`, one type. A
program may import one subject alone (`import std::text;`). An item
with several paths is named by its shortest in every message
(`std::Vec`). An associated function is declared in the submodule that
declares its type (`[Assoc-Fn-Foreign-Type]`), which is why `std` is
divided by type.

`std`'s submodules share one privacy: a private item or field of any of
them is visible in all of `std`'s code, as it was when `std` was one
module. `File::read` fills a `Vec<u8>`'s spare room, and a path is
handed to the system as a `StringView`'s bytes, so `std::io` and
`std::sys` need what `std::collections` and `std::text` keep private.
No program is inside `std`, so no program can tell; to a program, each
submodule exports exactly what the tables below list.

## 0. Prelude

### `rule.stdlib.prelude`
**Status:** ACCEPTED

**Types in `std`** (unqualified after `import std;`):

    export enum Option<T>
    {
        Some(T),
        None
    }

    export enum Result<T, E>
    {
        Ok(T),
        Err(E)
    }

    export struct AllocError
    {
    }

    export struct Utf8Error
    {
        export usize offset;
    }

    export enum FileError                    -- rule.stdlib.file, rule.stdlib.read: files and standard input
    {
        NotFound,
        Denied,
        Io,
        Utf8(Utf8Error)
    }

    export enum PathKind                     -- rule.stdlib.fs
    {
        File(u64),
        Dir,
        Other
    }

    export enum ParseError                   -- rule.stdlib.text
    {
        Empty,
        Invalid(usize),
        OutOfRange
    }

**Conventions** (D-0134). `std`'s names follow a few rules, so a
function's name and signature can be guessed from its intent:

- A function is named under the type of its first parameter when that
  type is one of `std`'s (`Vec::push(&mut v, x)`, `Result::map_err(r, f)`,
  `String::parse<T>(&s)`); otherwise its name stands alone (`min`, `swap`,
  `crc32`, `read_le(s, at)`). A built-in type's few accessors carry its
  name as a prefix instead (`str_len`, `slice_len`), since a built-in
  type has no namespace.
- `T::new(…)` makes a `T` from its defining arguments; `from_X` makes one
  from an `X`, `as_X` borrows as an `X`, `into_X` consumes into an `X`,
  `to_X` makes a new derived value.
- `_mut` is the exclusive twin (`get`/`get_mut`), `_by` takes the
  caller's function (`sort_by`), `_at` reaches by position (`key_at`),
  `is_` asks without consuming (`Option::is_some(&o)`), `_str` is the twin
  that takes a `str` (`HashMap::get_str`).
- Text a function only reads is a `StringView` (a path, a needle, text to
  write): a literal is one already (`[Str-Literal-View]`, §2a), a `String`
  passes `&s[0..$]`. The exception is `split`'s separator, a `str`:
  `split` returns views of its first argument, so by
  `rule.temporal.elision` it takes no other borrowed parameter.
- A failure a program can handle is a `Result`, an absence an `Option`;
  misuse (an index out of bounds, `unwrap` of `None`) is a fault. Every
  input and output failure is a `FileError`.
- `remove` keeps the order of what remains, in every collection;
  `swap_remove` is the constant-time removal that does not.
- A unit of measure is part of a name (`monotonic_ns`, `unix_seconds`,
  `sleep_ms`).

**Intrinsics and functions** — an intrinsic's call is defined by the
named rule rather than by a body; each is in scope unqualified; type
arguments are written `name<τ>(…)` where the signature needs them. The
functions of `std` are in scope after `import std;`:

**Language intrinsics: values and arithmetic**

| Signature | Rule |
|---|---|
| `drop<T>(T x)` (argument in place position) | `rule.resauth.destroy` `[Destroy]` on the place; a temporary argument is destroyed directly |
| `wrapping_add/sub/mul<T>(T a, T b) : T`, `saturating_*` | `rule.arith.alt` |
| `checked_add/sub/mul/div/rem<T>(T a, T b) : Option<T>` | `rule.arith.alt` |
| `checked_neg<T>(T a) : Option<T>` | `checked_sub(0, a)` (D-0072): `None` for the most negative value of a signed type and for every non-zero unsigned value |
| `widen<U>(T x) : U`, `narrow<U>`, `narrow_wrapping<U>`, `reinterpret<U>`, `to_float<U>`, `to_int<U>` | `rule.arith.convert` |
| `checked_narrow<U>(T x) : Option<U>` (`T`, `U` integer types) | `rule.arith.convert` `[Checked-Narrow]` (D-0122) |
| `count_ones(T x) : u32`, `leading_zeros(T x) : u32`, `trailing_zeros(T x) : u32`, `rotate_left(T x, u32 k) : T`, `rotate_right(T x, u32 k) : T` (`T` an integer type) | `rule.arith.bitwise` `[Count-Ones]`, `[Leading-Zeros]`, `[Trailing-Zeros]`, `[Rotate]` (D-0123) |
| `sizeof<T>() : usize`, `alignof<T>() : usize` | `rule.arith.sizeof` |
| `min_value<T>() : T`, `max_value<T>() : T` (`T` an integer type, `f32` or `f64`) | `rule.arith.limits` |

**Language intrinsics: text, slices, checks and faults**

| Signature | Rule |
|---|---|
| `str_len(str s) : usize`, `str_byte(str s, usize i) : u8`, `str_ptr(str s) : rawptr<u8>` | `rule.stdlib.str` `[Str-Len]`, `[Str-Byte]`, `[Str-Ptr]` (§2a); all safe |
| `slice_len(slice<τ, m> s) : usize` (also through a reference) | a slice's length, `spec/16` `rule.agg.slice` (D-0047); safe |
| `static_assert(bool c)`, `static_assert(bool c, str message)` : unit (`c` a constant expression, the message a string literal) | `rule.module.static-assert` (`spec/17` §1b): computed before the program runs; nothing at run time |
| `fault(d) : never`, `fault(d, m) : never` (D-0083; `d` names a diagnostic of phase `dynamic` or `both` in `spec/registry/diagnostics.md`, written with `_` for `-`, such as `index_out_of_bounds`, `alloc_failure`, `assert_failed`, `unwrap_failed`; `m` a `str` message) | `[Fault]`: `⟨fault(d), Σ⟩ ↛ diag.d` (with `m` shown after the location), `disposition: checked`; typed `never` (`[T-Never]`, `spec/12` §5), so it may stand where any type is expected. `[Fault-Name]`: any other `d` is `diag.unbound-name` (static) |

**Language intrinsics: raw memory and threads**

| Signature | Rule |
|---|---|
| `dangling<T>() : rawptr<T>` | `outcome: impl-defined { any T-aligned non-null address }`, documented; never dereferenced by prelude code |
| `rawptr_of<T>(ref<T, shared> r) : rawptr<T>`, and the `exclusive` overload | `rule.trust.rawptr` `[Rawptr-Of]` |
| `reinterpret_ptr<U>(rawptr<T> p) : rawptr<U>` | identity on the address; safe |
| `reclaim<T>(rawptr<T> p) : place of T` | `[Reclaim]` (unsafe) |
| `release(rawptr<u8> p, usize n)` | `[Release]` (unsafe) |
| `copy_raw(rawptr<u8> dst, rawptr<u8> src, usize n)` | `[Copy-Raw]` (unsafe) |
| `allocate(usize n, usize align) : Result<rawptr<u8>, AllocError>` | `[Allocate]` below |
| `deallocate(rawptr<u8> p, usize n, usize align)` | `[Deallocate]` below (unsafe) |
| `read_volatile<T>(rawptr<T> p) : T`, `write_volatile<T>(rawptr<T> p, T v)` (`T` not a resource; inside `unsafe`) | `rule.trust.rawptr` `[Rawptr-Read-Volatile]`, `[Rawptr-Write-Volatile]` (D-0121) |
| `lock<T>(ref<mutex<T>, shared> m) : guard<T>` | `rule.conc.lock` |
| `Mutex::new<T>(T v) : mutex<T>` | `rule.conc.lock` (§2's construction) |
| `spawn(fn(τ1..τn) : τr f, τ1 a1, …, τn an) : handle<τr>` — argument count and types match `f`'s own signature, not a fixed arity | `rule.conc.spawn` `[Spawn]`, typed by `[T-Spawn]` (`spec/12`); `f` must be a `fn` value or a `move` closure |
| `join<T>(handle<T> h) : T` | `rule.conc.join` `[Join]`, typed by `[T-Join]` (`spec/12`) |

**`std::core`: `Option`, `Result` and the errors** (each error's `text` with its type: `std::io`, `std::text`)

| Signature | Rule |
|---|---|
| `Option::unwrap_or<T>(Option<T> o, T d) : T` | ordinary exported function of `std`: `match (o) { Some(v) : v, None : d }` (D-0033) |
| `Result::unwrap_or<T, E>(Result<T, E> r, T d) : T` | ordinary exported function of `std`: `match (r) { Ok(v) : v, Err(_) : d }` (D-0033); an unused `d` or error is destroyed at the call's end |
| `Option::ok_or<T, E>(Option<T> o, E e) : Result<T, E>` | ordinary exported function of `std`: `match (o) { Some(v) : Ok(v), None : Err(e) }` (D-0114) — the `Option` as a `Result` for `?`; `e` is evaluated before the call and, unused, ends with the statement |
| `Option::unwrap<T>(Option<T> o) : T`, `Option::expect<T>(Option<T> o, str m) : T`, `Result::unwrap<T, E>(Result<T, E> r) : T`, `Result::expect<T, E>(Result<T, E> r, str m) : T` | ordinary exported functions of `std` (D-0082): the payload of `Some`/`Ok`; on `None`/`Err`, `fault(unwrap_failed, m)` (`diag.unwrap-failed`), `m` naming the call for `unwrap`; an `Err`'s payload is destroyed |
| `Option::is_some<T>(ref<Option<T>, shared> o) : bool`, `Option::is_none`, `Result::is_ok<T, E>(ref<Result<T, E>, shared> r) : bool`, `Result::is_err` | ordinary exported functions of `std`: which variant, by a `match` through the reference; nothing is consumed (D-0073) |
| `Result::map_err<T, E1, E2>(Result<T, E1> r, fn(E1) : E2 f) : Result<T, E2>` | ordinary exported function of `std`: `match (r) { Ok(v) : Ok(v), Err(e) : Err(f(e)) }` (table cell — exempt from Allman, `spec/22` §1) |
| `FileError::text(ref<FileError, shared> e) : str`, `ParseError::text(ref<ParseError, shared> e) : str`, `Utf8Error::text(ref<Utf8Error, shared> e) : str` | ordinary exported functions of `std` (D-0117): the error as a sentence for a person — `FileError`: `NotFound` "not found", `Denied` "permission denied", `Io` "input/output error", `Utf8(_)` "not UTF-8"; `ParseError`: `Empty` "empty", `Invalid(_)` "not a number", `OutOfRange` "out of range"; `Utf8Error` "not UTF-8". A `str`: nothing is allocated; an offset inside the error is read by `match` |

**`std::collections`: `Vec`**

| Signature | Rule |
|---|---|
| `Vec::insert<T>(ref<Vec<T>, exclusive> v, usize i, T x)` (`i ≤ len`), `Vec::remove<T>(ref<Vec<T>, exclusive> v, usize i) : T` (`i < len`) | ordinary exported functions of `std`: `x` pushed then moved down to `i` by `Vec::swap`, or the element at `i` moved to the end by `Vec::swap` and popped; `diag.index-out-of-bounds` otherwise (D-0073) |
| `Vec::clone<T: clone>(ref<Vec<T>, shared> v) : Vec<T>`, `Vec::clone_by<T>(ref<Vec<T>, shared> v, fn(ref<T, shared>) : T copy) : Vec<T>` | ordinary exported functions of `std` (D-0082): a new `Vec` of the elements copied in order, each by `clone` (D-0116; a `T` that is not `clone` is `[Bound-Unsatisfied]` at the call) or by `copy` (`clone_by`) |
| `Vec::from_slice<T: clone>(slice<T, shared> s) : Vec<T>`, `Vec::truncate<T>(ref<Vec<T>, exclusive> v, usize n)` | ordinary exported functions of `std` (D-0085): a new `Vec` of the slice's elements, each copied by `clone` (D-0116); the first `n` elements kept and the rest destroyed, last first (`n` at or beyond the length leaves the `Vec` unchanged) |
| `Vec::filled<T: clone>(usize n, T x) : Vec<T>`, `Vec::from_fn<T>(usize n, fn(usize) : T f) : Vec<T>`, `Vec::append<T>(ref<Vec<T>, exclusive> a, Vec<T> b)`, `Vec::extend_from<T: clone>(ref<Vec<T>, exclusive> a, slice<T, shared> s)`, `Vec::position<T>(ref<Vec<T>, shared> v, fn(ref<T, shared>) : bool p) : Option<usize>`, `Vec::index_of<T: eq>(ref<Vec<T>, shared> v, ref<T, shared> x) : Option<usize>`, `Vec::contains<T: eq>(ref<Vec<T>, shared> v, ref<T, shared> x) : bool`, `Vec::reverse<T>(ref<Vec<T>, exclusive> v)` | ordinary exported functions of `std` (D-0112): `filled` — `n` copies of `x` by `clone` (D-0116); `from_fn` — `f(0), f(1), …, f(n-1)` pushed in that order, any `T`; `append` — `b`'s elements moved onto the end of `a` in order, `b` consumed; `extend_from` — the slice's elements copied by `clone` onto the end of `a` in order (a slice of `a` itself is `diag.aliasing-conflict`, `[Slice-Form]`); `position` — `Some(i)` for the least `i` with `p(&v[i])`, else `None`, `p` called in index order and not past the first hit; `index_of` — the least `i` with `v[i] == *x` (`[Cmp-*]`, in place for `String`), else `None`; `contains` — whether `index_of` finds one; `reverse` — the order inverted in place by `Vec::swap`, nothing destroyed, an empty or one-element `Vec` unchanged |
| `Vec::sort`, `Vec::sort_by`, `Vec::binary_search`, `Vec::binary_search_by` | exported functions of `std`, written in CobaltC over the std-only intrinsic `key_less`: `rule.stdlib.sort` (§1b) |
| `Vec::push_le<T: integer>(ref<Vec<u8>, exclusive> out, T x)`, `Vec::push_be<T: integer>(ref<Vec<u8>, exclusive> out, T x)`, `read_le<T: integer>(slice<u8, shared> s, usize at) : T`, `read_be<T: integer>(slice<u8, shared> s, usize at) : T` | ordinary exported functions of `std` (D-0119), written in CobaltC over `sizeof<T>()`, `>>`, `<<` and `narrow_wrapping`: `[Bytes-LE]`/`[Bytes-BE]` below — the `sizeof<T>()` bytes of `x`'s two's-complement image, least (`le`) or most (`be`) significant first, pushed onto `out`; the `T` whose image those bytes are, starting at `s[at]`; `diag.index-out-of-bounds` when fewer than `sizeof<T>()` bytes remain from `at` |

**`std::text`: `String`, `StringView`, numbers as text**

| Signature | Rule |
|---|---|
| `StringView` and its functions (`StringView::of`, `String::view`, `StringView::sub`, `len`, `eq`, `eq_str`, `find`, `starts_with`, `ends_with`, `trim`, `trim_start`, `trim_end`, `split`, `chars`, `contains`, `replace`, `join`, `parse<T>`, `String::from_view`) | exported items of `std`, written in CobaltC: `rule.stdlib.stringview` (§2h); `find`, `starts_with`, `ends_with`, `contains` and `replace` take the text sought as a `StringView`, `split` its separator as a `str` (D-0134); `contains`, `replace` and `join` are `[Contains]`, `[Replace]`, `[Join]` (D-0139); `&s[lo .. hi]` on a `String` and `==` with a view are its `[View-Form]` and `[View-Eq]` |
| `String::as_view(ref<String, shared> s) : StringView`, `String::find(ref<String, shared> s, StringView t) : Option<usize>`, `String::starts_with(ref<String, shared> s, StringView t) : bool`, `String::ends_with`, `String::trim(ref<String, shared> s) : StringView`, `String::trim_start`, `String::trim_end`, `String::split(ref<String, shared> s, str sep) : Vec<StringView>`, `String::chars(ref<String, shared> s) : Vec<StringView>` (D-0128), `String::contains(ref<String, shared> s, StringView t) : bool`, `String::replace(ref<String, shared> s, StringView from, StringView to) : String` (D-0139) | ordinary exported functions of `std` (D-0113): `as_view(&s)` is `&s[0..$]` (`[View-Form]`), and each other is the `StringView::` function of the same name applied to `&s[0..$]`; a result that is a view borrows `s` as a view does (`replace`'s `String` is new and borrows nothing) |
| `String::join(slice<String, shared> parts, StringView sep) : String`, `StringView::join(slice<StringView, shared> parts, StringView sep) : String` | ordinary exported functions of `std` (D-0139): `[Join]` (§2h), the texts of `parts` with `sep` between each two, in a new `String` |
| `String::eq(ref<String, shared> a, ref<String, shared> b) : bool`, `String::eq_str(ref<String, shared> a, str t) : bool` | ordinary exported functions of `std`: the texts' `[View-Eq]` (§2h) (D-0073) |
| `StringView::less(StringView a, StringView b) : bool`, `String::less(ref<String, shared> a, ref<String, shared> b) : bool` | ordinary exported functions of `std`: byte order (for UTF-8, code point order), a prefix first; `String::less` fits `Vec::sort_by` (D-0076) |
| `String::truncate(ref<String, exclusive> s, usize n)` | ordinary exported function of `std`: keeps the first `n` bytes; `diag.index-out-of-bounds` for `n` beyond the length, `diag.not-char-boundary` inside a character (D-0076) |
| `String::append<T>(ref<String, exclusive> s, T x)` for a printable `T` | exported function of `std`, realized natively: `rule.stdlib.text` `[Append]` (§2d) |
| `String::parse<T>(ref<String, shared> s) : Result<T, ParseError>` for an integer or float `T` | exported function of `std`, realized natively: `rule.stdlib.text` `[Parse]` (§2d) |
| `ascii_is_digit(u8 b) : bool`, `ascii_is_alpha`, `ascii_is_alnum`, `ascii_is_upper`, `ascii_is_lower`, `ascii_is_space`; `ascii_to_lower(u8 b) : u8`, `ascii_to_upper`; `String::to_ascii_lower(ref<String, shared> s) : String`, `String::to_ascii_upper` | exported functions of `std` (D-0101): ASCII only — a byte outside ASCII is none of the classes and is left unchanged, so a `String` stays valid UTF-8; `ascii_is_space` is space, tab, line feed, vertical tab, form feed and carriage return |

**`std::collections`: hash tables and queues**

| Signature | Rule |
|---|---|
| `HashMap<K, V>`, `HashSet<K>` and their functions, for a key type `K` | exported types of `std`, written in CobaltC over the std-only intrinsics `key_hash` and `key_eq`: `rule.stdlib.hashmap` (§2g); `remove` keeps the other entries' order, `swap_remove` moves the last entry into the hole (D-0134); `HashSet::key_at` and `HashSet::clear` as `HashMap`'s |
| `HashMap::get_str<V>(ref<HashMap<String, V>, shared> m, str k) : Option<ref<V, shared>>`, `HashMap::get_mut_str<V>(ref<HashMap<String, V>, exclusive> m, str k) : Option<ref<V, exclusive>>`, `HashMap::contains_str<V>(…, str k) : bool`, `HashMap::remove_str<V>(ref<HashMap<String, V>, exclusive> m, str k) : Option<V>`, `HashSet::contains_str(ref<HashSet<String>, shared> s, str k) : bool`, `HashSet::remove_str(ref<HashSet<String>, exclusive> s, str k) : bool` | ordinary exported functions of `std` (D-0113), for a map or set keyed by `String` only: each is the lookup of the same name with a `String` key of `k`'s bytes, without making one (`[Lookup-Str]`, §2g); `remove_str` keeps order as `remove` does |
| `HashSet::union<K: clone>(ref<HashSet<K>, shared> a, ref<HashSet<K>, shared> b) : HashSet<K>`, `HashSet::intersection<K: clone>(…)`, `HashSet::difference<K: clone>(…)` | ordinary exported functions of `std` (D-0138), written in CobaltC: `[Set-Ops]` (§2g) — a new set of copies (by `clone`) of the keys in `a` or `b`, in both, or in `a` and not `b`; `a` and `b` unchanged |
| `Queue<T>` and its functions (`Queue::new`, `len`, `push_back`, `pop_front`, `push_front`, `pop_back`, `front`, `back`, `clear`); `PriorityQueue<T>` and its functions (`PriorityQueue::new`, `new_by`, `len`, `push`, `pop`, `peek`, `clear`) | exported types of `std` (D-0137), written in CobaltC: `rule.stdlib.queue`, `rule.stdlib.priority-queue` (§2i) — a queue taken from at either end in constant time; a queue that gives the least element first, by the key types' order (`new`, as `Vec::sort`) or the caller's `less` (`new_by`, as `sort_by`), equal elements first in, first out |

**`std::memory`, `std::core`, `std::sync`: `Box`, `Rc`, `Weak`, values behind references, channels**

| Signature | Rule |
|---|---|
| `Box::clone<T: clone>(ref<Box<T>, shared> b) : Box<T>`, `HashMap::clone<K: clone, V: clone>(ref<HashMap<K, V>, shared> m) : HashMap<K, V>`, `HashSet::clone<K: clone>(ref<HashSet<K>, shared> s) : HashSet<K>` | ordinary exported functions of `std` (D-0116): a `Box` of a copy of the value; a map or set of copies of the keys and values, in the same insertion order (`String::clone` and `Rc::clone`, which share, complete the `std` types that are `clone`; `Option<T>` and `Result<T, E>` are `clone` by derivation, `rule.agg.derived-clone`) |
| `Weak<T>`, `Rc::downgrade<T>(ref<Rc<T>, shared> r) : Weak<T>`, `Weak::upgrade<T>(ref<Weak<T>, shared> w) : Option<Rc<T>>`, `Weak::clone<T>(ref<Weak<T>, shared> w) : Weak<T>` | exported type and functions of `std` (D-0125), written in CobaltC: `rule.stdlib.rc` — a handle that does not keep the value alive; `upgrade` gives a new strong handle while any exists, else `None` |
| `overwrite<T>(ref<T, exclusive> r, T v)` | ordinary exported function of `std` (D-0080): `drop(replace(r, v))`, §3b |
| `Channel<T>` and its functions (`Channel::new`, `send`, `recv`, `close`) | exported items of `std`, written in CobaltC over `mutex` and one primitive private to `std`: `rule.conc.channel` (`spec/19` §3), `rule.stdlib.channel` (§3c) |

**`std::io`, `std::sys`: input, output, files, the system**

| Signature | Rule |
|---|---|
| `printf(f, a1, …, an)`, `String::appendf(ref<String, exclusive> s, f, a1, …, an)`, `sprintf(f, a1, …, an) : String`, `eprintf(f, a1, …, an)` — `f` a string literal, any number of arguments | exported functions of `std`, realized natively: `rule.stdlib.format` `[Printf]`, `[Appendf]`, `[Sprintf]`, `[Eprintf]` (§2f) |
| `print<T>(T x)` for a printable `T` | function of `std`, not exported (D-0039), realized natively: `rule.stdlib.print` (§2a) |
| `assert(c)`, `assert(c, f, a1, …, an)` — `c` a `bool`, `f` a string literal, as `printf`'s | exported function of `std`, realized natively: `rule.fail.assert` (`spec/18` §1a) |
| `export extern fn stdout_write(rawptr<u8> buf, usize len) : isize;` | `rule.trust.extern-call` `[Extern-Call]` (`spec/20`); an instance declared in `std`, no dedicated rule |
| `export extern fn stdin_read(rawptr<u8> buf, usize len) : isize;` | `rule.stdlib.read` `[Read]`, an instance of `[Extern-Call]` declared in `std` |
| `read_line() : Result<Option<String>, FileError>` | ordinary exported function of `std`, written in CobaltC: `rule.stdlib.read` |
| `export extern fn arg_bytes(usize i, rawptr<u8> buf, usize len) : isize;` | `rule.stdlib.args` `[Arg-Bytes]`, an instance of `[Extern-Call]` declared in `std` |
| `arg_count() : usize`, `arg(usize i) : Result<String, Utf8Error>` | ordinary exported functions of `std`, written in CobaltC: `rule.stdlib.args` |
| `read_file(StringView path) : Result<String, FileError>`, `write_file(StringView path, StringView text) : Result<void, FileError>` | exported functions of `std`, realized natively: `rule.stdlib.file` `[Read-File]`, `[Write-File]` (§2e) |
| `File` and its functions (`File::open`, `create`, `append`, `open_rw`, `read`, `read_to_end`, `read_line`, `write`, `write_text`, `printf`, `seek`, `len`, `close`); `read_bytes(StringView path) : Result<Vec<u8>, FileError>`, `write_bytes(StringView path, slice<u8, shared> data) : Result<void, FileError>`; every path a `StringView` (D-0134) | exported items of `std`, written in CobaltC over two primitives private to `std`: `rule.stdlib.file-handle` (§2e) |
| `make_dir`, `remove_file`, `remove_dir`, `rename`, `list_dir`, `path_kind` (each path a `StringView`; all `Result<…, FileError>`); `env_var(StringView name) : Result<Option<String>, Utf8Error>`, `current_dir() : Result<String, FileError>`, `monotonic_ns() : u64`, `unix_seconds() : i64` | exported functions of `std`, written in CobaltC over three primitives private to `std`: `rule.stdlib.fs`, `rule.stdlib.env` (§2e′) |
| `sleep_ms(u64 ms)` | ordinary exported function of `std` (D-0124) over the std-private `extern fn sleep_ns(u64) : i64`: the calling thread does nothing for at least `ms` milliseconds (`[Sleep]`: how much longer is implementation-defined; other threads run meanwhile; a thread holding a guard keeps it; no fault) |

**`std::core`, `std::text`, `std::random`: numbers, bytes, checksums, random numbers**

| Signature | Rule |
|---|---|
| `min<T: ordered>(T a, T b) : T`, `max<T: ordered>(T a, T b) : T`, `abs<T: number>(T x) : T`, `pow<T: number>(T base, u32 exp) : T` | exported functions of `std`, written in CobaltC over D-0090's bounds (D-0091): `min`/`max` give `a` when the two are equal; `abs` and `pow` are checked (`diag.arith-overflow` for `abs` of a signed type's minimum, and for a power outside `T`), `abs(-0.0)` is `0.0`, `pow(x, 0)` is `1` |
| `crc32(slice<u8, shared> data) : u32` | ordinary exported function of `std` (D-0119): CRC-32/IEEE, `[CRC32]` below; `crc32` of the empty slice is 0, of the bytes of `"123456789"` is `0xCBF43926` |
| `Rng`, `Rng::new(u64 s) : Rng`, `Rng::next_u64(ref<Rng, exclusive> r) : u64`, `Rng::below(ref<Rng, exclusive> r, u64 n) : u64`, `Rng::unit_f64(ref<Rng, exclusive> r) : f64` | exported struct and functions of `std` (D-0120), written in CobaltC: a general-purpose pseudo-random generator, `[Rng]` below — the same seed gives the same sequence in every implementation; `below` is unbiased and `n = 0` is `diag.div-by-zero`; `unit_f64` is in `[0, 1)`. Not suitable for cryptographic purposes: its state is recoverable from its output, so it must not produce keys, tokens or nonces |

**`std::math`: floating-point functions** (D-0136; before it, language intrinsics)

| Signature | Rule |
|---|---|
| `sqrt(T x) : T`, `floor`, `ceil`, `round`, `trunc` (`T` `f32` or `f64`) | exported functions of `std`, realized natively: `rule.arith.float-fns` (§3d, D-0091) |
| `ln(T x) : T`, `exp`, `log2`, `log10`, `sin`, `cos`, `tan`; `atan2(T y, T x) : T`, `powf(T x, T y) : T` (`T` `f32` or `f64`) | exported functions of `std`, realized natively: `rule.arith.float-fns` `[Float-Transcendental]` (§3d, D-0101): the platform's, within 1 ulp |

    [Allocate]   disposition: fallible   outcome: unspecified { any fresh aligned range, or Err }
        n, align : usize
        ────────────────────────────────────────────
        ⟨allocate(n, align), Σ⟩ → ⟨Ok(p), Σ[ storage(x) := uninit for x ∈ [p, p+n) ]⟩   with [p, p+n) ∩ dom(Σ.storage) = ∅,
                                                                                          p mod align = 0
                                or ⟨Err(AllocError{}), Σ⟩

    [Deallocate]   disposition: trusted-unchecked
        ────────────────────────────────────────────
        ⟨deallocate(p, n, align), Σ⟩ → ⟨(), Σ' ⟩   where Σ' = release(p, n) then storage := Σ.storage \ [p, p+n)
        side-conditions: ⟦ (p, n, align) were returned by one allocate not yet deallocated ⟧ discharge: trusted

An allocation may fail (`Err`); succeeding is not guaranteed for any
size. `allocate(0, _)` returns `Ok(dangling)` without extending
storage.

**Depends on:** rule.arith.alt, rule.arith.convert, rule.arith.sizeof,
rule.resauth.destroy, rule.trust.rawptr, rule.conc.lock,
rule.conc.spawn, rule.conc.join, rule.agg.enum-construct,
rule.trust.extern-call
**Affects:** state.storage

## 1. `Vec<T>`

In `std::collections`.

### `rule.stdlib.vec`
**Status:** ACCEPTED

    export resource struct Vec<T>
    {
        rawptr<T> ptr;
        usize len;
        usize cap;
    }

    export fn Vec::new<T>() : Vec<T>
    {
        Vec { .ptr = dangling<T>(), .len = 0, .cap = 0 }
    }

    export fn Vec::len<T>(ref<Vec<T>, shared> v) : usize
    {
        v.len
    }

    export fn Vec::push<T>(ref<Vec<T>, exclusive> v, T x)
    {
        if (v.len == v.cap)
        {
            Vec::grow(v);
        }
        unsafe
        {
            *(v.ptr + reinterpret<isize>(v.len)) = x;      // [Rawptr-Write] or [Rawptr-Move-In]
        }
        v.len = v.len + 1;
    }

    export fn Vec::pop<T>(ref<Vec<T>, exclusive> v) : Option<T>
    {
        if (v.len == 0)
        {
            return None;
        }
        v.len = v.len - 1;
        unsafe
        {
            rawptr<T> p = v.ptr + reinterpret<isize>(v.len);
            auto x = *p;                                          // [Rawptr-Read] or [Rawptr-Move-Out]
            release(reinterpret_ptr<u8>(p), sizeof<T>());          // ends a reclaimed object left at this slot
                                                                     // (a no-op for a resource T: [Rawptr-Move-Out]
                                                                     // above already ended it) — see [Release]
            Some(x)
        }
    }

    export fn Vec::index_shared<T>(ref<Vec<T>, shared> v, usize i) : ref<T, shared>
    {
        if (i >= v.len)
        {
            fault(index_out_of_bounds);                    // see below
        }
        unsafe
        {
            &reclaim<T>(v.ptr + reinterpret<isize>(i))
        }
    }

    export fn Vec::index_exclusive<T>(ref<Vec<T>, exclusive> v, usize i) : ref<T, exclusive>
    {
        if (i >= v.len)
        {
            fault(index_out_of_bounds);
        }
        unsafe
        {
            &mut reclaim<T>(v.ptr + reinterpret<isize>(i))
        }
    }

    fn Vec::grow<T>(ref<Vec<T>, exclusive> v)
    {
        usize new_cap = if (v.cap == 0)
        {
            4
        }
        else
        {
            v.cap * 2
        };
        usize bytes = new_cap * sizeof<T>();
        rawptr<u8> p = match (allocate(bytes, alignof<T>()))
        {
            Ok(p) : p,
            Err(_) : fault(alloc_failure)
        };
        unsafe
        {
            copy_raw(p, reinterpret_ptr<u8>(v.ptr), v.len * sizeof<T>());   // reclaimed element objects are
                                                                             // released by the deallocate below and
                                                                             // re-attached lazily by reclaim
            if (v.cap > 0)
            {
                deallocate(reinterpret_ptr<u8>(v.ptr), v.cap * sizeof<T>(), alignof<T>());
            }
        }
        v.ptr = reinterpret_ptr<T>(p);
        v.cap = new_cap;
    }

    // Room for n more elements (D-0129).
    export fn Vec::reserve<T>(ref<Vec<T>, exclusive> v, usize n)
    {
        usize need = v.len + n;                          // overflowing: diag.arith-overflow
        if (need <= v.cap)
        {
            return;
        }
        usize new_cap = if (v.cap > need - v.cap)
        {
            v.cap * 2
        }
        else
        {
            need
        };
        usize bytes = new_cap * sizeof<T>();
        rawptr<u8> p = match (allocate(bytes, alignof<T>()))
        {
            Ok(p) : p,
            Err(_) : fault(alloc_failure)
        };
        unsafe
        {
            copy_raw(p, reinterpret_ptr<u8>(v.ptr), v.len * sizeof<T>());
            if (v.cap > 0)
            {
                deallocate(reinterpret_ptr<u8>(v.ptr), v.cap * sizeof<T>(), alignof<T>());
            }
        }
        v.ptr = reinterpret_ptr<T>(p);
        v.cap = new_cap;
    }

    export fn Vec::drop<T>(ref<Vec<T>, exclusive> self)
    {
        usize mut_i = 0;
        while (mut_i < self.len)
        {
            unsafe
            {
                drop(reclaim<T>(self.ptr + reinterpret<isize>(mut_i)));   // destroys element i if T is a
                                                                           // resource; ends the reclaimed object
            }
            mut_i = mut_i + 1;
        }
        if (self.cap > 0)
        {
            unsafe
            {
                deallocate(reinterpret_ptr<u8>(self.ptr), self.cap * sizeof<T>(), alignof<T>());
            }
        }
    }

`Vec::reserve(&mut v, n)` (D-0129) makes room for `n` more elements:
afterwards `cap >= len + n`. When the room is already there it changes
nothing; otherwise the elements move once, as `grow` moves them, to
cells for the larger of `len + n` and twice the capacity (doubling, as
`push` grows, so that small reserves in a loop stay amortized). It never
shrinks a `Vec` and never changes its length or elements; `len + n`
overflowing is `diag.arith-overflow`, before anything is allocated. A
reference into the old cells is stale afterwards, as after a `push`
that grows. No function reports a capacity, so `reserve` changes how
fast a program runs, not what it computes. `std`'s own builders
(`String::from_str`, `String::clone`, `Vec::from_slice`) reserve their
final length before filling.

`fault(d)` raises a checked fault with the named diagnostic
(`diag.index-out-of-bounds`, `diag.alloc-failure`), as `std` does here
and as a program implementing its own containers may (D-0083):
`[Fault]`: `⟨fault(d), Σ⟩ ↛ diag.d`, `disposition: checked`, typed `never`. `reinterpret_ptr<U>(rawptr<T> p) : rawptr<U>` is the
identity on addresses (an intrinsic, safe). `Vec` is declared
`resource` (its fields carry no authority; the allocation does), with
destructor `Vec::drop` invoked by `rule.resauth.destroy`. Element
references come from `reclaim`: a reclaimed element object persists
while the buffer does, so two references to element `i` are two
paths on one object and `clash` governs them; a reallocation
(`grow`) releases every reclaimed element, so a reference held across
a `push` that reallocates is caught as `diag.stale-binding` at its
next use (dynamic — `rule.control.flow-analysis` marks the referent
`escaped`). `push` of a resource moves it into raw storage as a
reclaimed object (`[Rawptr-Move-In]`); `pop` moves it back out
(`[Rawptr-Move-Out]`, which ends the reclaimed object as part of the
move) and, for a *plain* `T` — where the read that recovers the value
leaves the reclaimed object alone, correctly, since reading owns
nothing — additionally `release`s the popped slot explicitly, so a
reference into a popped-and-reused index is caught the same way as one
across a reallocation: `diag.stale-binding` at its next use, dynamic,
never a silent read of whatever a later `push` happens to write there
(`spec/AUDIT-STATUS.md` finding F-05, closed by `CHG-0019`). `drop`
destroys each element by reclaiming and destroying it (`[Destroy]` for
a resource `T`, `[Destroy-Plain]` otherwise — both end the reclaimed
object).

**Depends on:** rule.trust.rawptr, rule.stdlib.prelude,
rule.resauth.destroy, rule.type.is-resource, inv.spatial-validity,
D-0003, D-0008

## 1b. Sorting and searching

In `std::collections`.

### `rule.stdlib.sort`
**Status:** ACCEPTED

A `Vec` is sorted by a comparison, `less(a, b)` meaning that `a` goes
before `b`, or in its elements' own order when they are of a key type
(D-0062). Sorting is stable: equal elements (neither before the other)
keep their order. A search needs a `Vec` sorted by the same order.

    [Sort-By]         ⟨Vec::sort_by(v, less), Σ⟩ → the elements of v, each moved once, in an order
                      where no element is less than one before it, equal ones as they were;
                      `less` is called on references to elements (v is borrowed exclusively)
    [Sort]            ⟨Vec::sort(v), Σ⟩ is ⟨Vec::sort_by(v, key_before<T>), Σ⟩: integers by value,
                      `false` before `true`, `str` and `String` by their bytes (a prefix first);
                      a struct field by field in declaration order, an enum by the order its
                      variants are declared in, then by payload (D-0110)
    [Binary-Search]   ⟨Vec::binary_search_by(v, key, less), Σ⟩ → Ok(i), i the first position whose
                      element is equal to key, else Err(i), i the position key would take;
                      `binary_search(v, key)` in the key types' order
    [Sort-Not-Key]    disposition: rejected   `Vec::sort` or `Vec::binary_search` with T not a key
                      type (`[Key-Not-Hashable]`'s types)   ill-formed; diag.type-mismatch

`Vec::sort` compares in place, with the std-only intrinsic
`key_less_at(v, i, j)` (`key_less` of elements `i` and `j` of the `Vec`
`v` refers to, read while `v` is borrowed shared for the whole sort),
rather than through a reference to each element; the order, and every
other effect, is `[Sort]`'s.

`less` must be a strict weak order (never `less(a, a)`; transitive);
with one that is not, the elements still end as a permutation of what
they were, in an order this rule does not fix. An implementation may
realize `Vec::sort` of an integer or `bool` element type natively,
provided the elements end as `[Sort]` leaves them and the same checks
on `v` are made (`cobc` does, spec/21 §0's latitude).

The `std` source (`key_less` is an intrinsic private to `std`, the key
types' order):

    // Sorting and searching (`rule.stdlib.sort`, spec/21 §1b, D-0062). The
    // order `Vec::sort` and `Vec::binary_search` use: `key_less`, the key
    // types' own (integers by value, `false` before `true`, text by its
    // bytes), as a `fn` value.
    fn key_before<T>(ref<T, shared> a, ref<T, shared> b) : bool
    {
        key_less(a, b)
    }

    // The positions 0..n of `v` in the order `less` gives, equal elements in
    // their original order: a bottom-up merge sort of positions, which reads
    // the elements only through shared references.
    fn Vec::sorted_order<T>(ref<Vec<T>, shared> v, fn(ref<T, shared>, ref<T, shared>) : bool less) : Vec<usize>
    {
        usize n = v.len;
        Vec<usize> a = Vec::new();
        Vec<usize> b = Vec::new();
        usize i = 0;
        while (i < n)
        {
            Vec::push(&mut a, i);
            Vec::push(&mut b, 0);
            i += 1;
        }
        usize width = 1;
        while (width < n)
        {
            usize lo = 0;
            while (lo < n)
            {
                usize mid = if (n - lo > width)
                {
                    lo + width
                }
                else
                {
                    n
                };
                usize hi = if (n - mid > width)
                {
                    mid + width
                }
                else
                {
                    n
                };
                usize l = lo;
                usize r = mid;
                usize k = lo;
                while (k < hi)
                {
                    // From the left run unless the right one's element is
                    // strictly less: equal elements keep their order.
                    bool take_left = l < mid && (r >= hi || !less(Vec::index_shared(v, a[r]), Vec::index_shared(v, a[l])));
                    if (take_left)
                    {
                        b[k] = a[l];
                        l += 1;
                    }
                    else
                    {
                        b[k] = a[r];
                        r += 1;
                    }
                    k += 1;
                }
                lo = hi;
            }
            swap(&mut a, &mut b);
            width = if (width > n)
            {
                n
            }
            else
            {
                width * 2
            };
        }
        a
    }

    // `sorted_order` in the key types' own order, comparing elements in
    // place (`key_less_at`) rather than through a reference to each.
    fn Vec::sorted_order_keys<T>(ref<Vec<T>, shared> v) : Vec<usize>
    {
        usize n = v.len;
        Vec<usize> a = Vec::new();
        Vec<usize> b = Vec::new();
        usize i = 0;
        while (i < n)
        {
            Vec::push(&mut a, i);
            Vec::push(&mut b, 0);
            i += 1;
        }
        usize width = 1;
        while (width < n)
        {
            usize lo = 0;
            while (lo < n)
            {
                usize mid = if (n - lo > width)
                {
                    lo + width
                }
                else
                {
                    n
                };
                usize hi = if (n - mid > width)
                {
                    mid + width
                }
                else
                {
                    n
                };
                usize l = lo;
                usize r = mid;
                usize k = lo;
                while (k < hi)
                {
                    bool take_left = l < mid && (r >= hi || !key_less_at(v, a[r], a[l]));
                    if (take_left)
                    {
                        b[k] = a[l];
                        l += 1;
                    }
                    else
                    {
                        b[k] = a[r];
                        r += 1;
                    }
                    k += 1;
                }
                lo = hi;
            }
            swap(&mut a, &mut b);
            width = if (width > n)
            {
                n
            }
            else
            {
                width * 2
            };
        }
        a
    }

    // Moves every element of `v` once, into the order `order` gives.
    fn Vec::apply_order<T>(ref<Vec<T>, exclusive> v, ref<Vec<usize>, shared> order)
    {
        Vec<T> out = Vec::new();
        foreach (p in order)
        {
            Vec::push(&mut out, Vec::take_raw(v, *p));
        }
        v.len = 0;
        swap(v, &mut out);
    }

    // Sorts `v` by `less` (true when its first argument goes before its
    // second), keeping equal elements in their order.
    export fn Vec::sort_by<T>(ref<Vec<T>, exclusive> v, fn(ref<T, shared>, ref<T, shared>) : bool less)
    {
        if (v.len < 2)
        {
            return;
        }
        Vec<usize> order = Vec::sorted_order(v, less);
        Vec::apply_order(v, &order);
    }

    // Sorts `v` in its elements' own order; `T` a key type (integers, `bool`,
    // `str`, `String`).
    export fn Vec::sort<T>(ref<Vec<T>, exclusive> v)
    {
        if (v.len < 2)
        {
            return;
        }
        Vec<usize> order = Vec::sorted_order_keys(v);
        Vec::apply_order(v, &order);
    }

    // In a `v` sorted by `less`: Ok(i), i the first position holding an
    // element equal to `key` (neither goes before the other), or Err(i), the
    // position where `key` would go.
    export fn Vec::binary_search_by<T>(ref<Vec<T>, shared> v, ref<T, shared> key, fn(ref<T, shared>, ref<T, shared>) : bool less) : Result<usize, usize>
    {
        usize lo = 0;
        usize hi = v.len;
        while (lo < hi)
        {
            usize mid = lo + (hi - lo) / 2;
            if (less(Vec::index_shared(v, mid), key))
            {
                lo = mid + 1;
            }
            else
            {
                hi = mid;
            }
        }
        if (lo < v.len && !less(key, Vec::index_shared(v, lo)))
        {
            return Ok(lo);
        }
        Err(lo)
    }

    // `binary_search_by` in the elements' own order; `T` a key type.
    export fn Vec::binary_search<T>(ref<Vec<T>, shared> v, ref<T, shared> key) : Result<usize, usize>
    {
        Vec::binary_search_by(v, key, key_before<T>)
    }

**Depends on:** rule.stdlib.vec, rule.stdlib.hashmap, rule.stdlib.swap, D-0062
**Affects:** state.storage (through `Vec::push`)

## 2. `String`

In `std::text`.

### `rule.stdlib.string`
**Status:** ACCEPTED

    export struct String
    {
        Vec<u8> bytes;                   // is-resource by derivation
    }

    export fn String::from_utf8(Vec<u8> bytes) : Result<String, Utf8Error>
    {
        usize n = Vec::len(&bytes);
        usize i = 0;
        while (i < n)
        {
            u8 b = *Vec::index_shared(&bytes, i);
            usize width =
                if (b < 128)
                {
                    1
                }
                else if ((b & 224) == 192 && b >= 194)
                {
                    2
                }
                else if ((b & 240) == 224)
                {
                    3
                }
                else if ((b & 248) == 240 && b <= 244)
                {
                    4
                }
                else
                {
                    return Err(Utf8Error { .offset = i });
                };
            if (i + width > n)
            {
                return Err(Utf8Error { .offset = i });
            }
            usize k = 1;
            while (k < width)
            {
                u8 c = *Vec::index_shared(&bytes, i + k);
                if ((c & 192) != 128)
                {
                    return Err(Utf8Error { .offset = i + k });
                }
                k = k + 1;
            }
            if (width == 3)
            {
                u8 c1 = *Vec::index_shared(&bytes, i + 1);
                if ((b == 224 && c1 < 160) || (b == 237 && c1 >= 160))
                {
                    return Err(Utf8Error { .offset = i });
                }
            }
            if (width == 4)
            {
                u8 c1 = *Vec::index_shared(&bytes, i + 1);
                if ((b == 240 && c1 < 144) || (b == 244 && c1 >= 144))
                {
                    return Err(Utf8Error { .offset = i });
                }
            }
            i = i + width;
        }
        Ok(String { .bytes = bytes })
    }

    export fn String::len(ref<String, shared> s) : usize
    {
        Vec::len(&s.bytes)
    }

    export fn String::into_bytes(String s) : Vec<u8>
    {
        String { bytes } = s;   // rule.init.let [Let-Destructure]
        bytes
    }

    export fn String::from_str(str s) : String
    {
        Vec<u8> v = Vec::new();
        usize n = str_len(s);
        Vec::reserve(&mut v, n);
        usize i = 0;
        while (i < n)
        {
            Vec::push(&mut v, str_byte(s, i));
            i = i + 1;
        }
        String { .bytes = v }
    }

`from_utf8` is `[Trust-Transition]` instantiated: the byte vector is a
claim; the validator establishes `inv.string.utf8-validity` for the
resulting `String` (`spec/03`). `from_str` is the other constructor:
it copies the bytes of a `str` value, which are well-formed by
`inv.str.utf8-validity` (§2a), so the `String` invariant is
established without a validator — one invariant carried over into
another, not a trust transition. The only primitive that adds a
single byte, `String::push_ascii` (§2d), adds only ASCII, so the
invariant is preserved by construction. (`into_bytes` uses struct
destructuring, `spec/22` §2 — the one pattern form beyond `match` on
enums.)

**Depends on:** inv.string.utf8-validity, inv.str.utf8-validity,
inv.trust-transition, rule.stdlib.vec, rule.stdlib.str,
rule.trust.extern-call

## 2a. `str`

`str` is built in; its functions are intrinsics. `print` is private to `std`'s root.

### `type.str`
**Status:** ACCEPTED

`str` is the type of text values: its represented domain is the set
of finite byte sequences that are well-formed UTF-8. A `str` is a
value in the sense of `term.value` — an element of a domain,
independent of storage — exactly as an integer is: `is-resource(str)
= false` (`rule.type.is-resource`), so it is copied by `[Read]`,
never transferred, and carries no destroy obligation. It is not a
reference (`type.ref`): it has no target, no mode, no `held-by`, and
no temporal validity to check; the bytes it denotes are not cells of
any object in `Σ.objects`, and no access path reaches into them.
Equality is `[Eq-Str]` (`spec/12` §4) and order `[Cmp-Text]` (`spec/06`,
D-0108); arithmetic and logic are ill-typed on `str`. Its representation is `[Sizeof-Str]`
/`[Repr-Str]` (`spec/06` §7). `str ∉ FfiType` (`spec/20` §3).

The only way to write a `str` is a `str-literal` (`spec/22` §1); the
only operations on one are the three intrinsics below,
`String::from_str` (§2), and `StringView::of` (§2h, D-0134), a
read-only view of its bytes. There is no indexing syntax and no
slicing of a `str` itself.

**Depends on:** term.value, rule.type.is-resource, rule.type.eq,
rule.arith.sizeof, rule.arith.represent, inv.str.utf8-validity,
D-0006, D-0020

### `rule.stdlib.str`
**Status:** ACCEPTED

    [Str-Literal]
        L a str-literal (spec/22 §1) whose decoded bytes are b0..b_{n-1}
        ────────────────────────────────────────────
        Γ ⊢ L : str;   ⟨L, Σ⟩ → ⟨⟪b0..b_{n-1}⟫, Σ⟩
        side-conditions:
            ⟦ well-formed-utf8(b0..b_{n-1}) ⟧ discharge: static   -- by the grammar: str-char is a scalar value
                                                                    -- of UTF-8 source text, str-escape encodes one

    [Str-Literal-View]   (D-0134)
        L a str-literal, or a use of a constant whose value is one (rule.module.const),
        in a position whose declared type is StringView: an argument whose parameter is declared
        StringView, the initializer of a local declared StringView, a struct literal's field
        declared StringView
        ────────────────────────────────────────────
        L there  ≡  StringView::of(L)
        -- typing, not conversion: the literal has no type until its position gives it one, as an
        -- unsuffixed numeric literal has none (rule.arith.literal, D-0079). A str binding has type
        -- str and is not affected (StringView::of(t)); nor is a parameter of a type parameter.

    [Str-Len]
        ⟨s, Σ⟩ →* ⟨⟪b0..b_{n-1}⟫, Σ'⟩
        ────────────────────────────────────────────
        ⟨str_len(s), Σ⟩ → ⟨n : usize, Σ'⟩

    [Str-Byte]   disposition: checked
        ⟨s, Σ⟩ →* ⟨⟪b0..b_{n-1}⟫, Σ1⟩    ⟨i, Σ1⟩ →* ⟨k : usize, Σ2⟩    k < n
        ────────────────────────────────────────────
        ⟨str_byte(s, i), Σ⟩ → ⟨b_k : u8, Σ2⟩

    [Str-Byte-Out-Of-Bounds]   disposition: checked
        k ≥ n
        ────────────────────────────────────────────
        ⟨str_byte(s, i), Σ⟩ ↛ diag.index-out-of-bounds

    [Str-Ptr]   outcome: impl-defined { any address }
        ⟨s, Σ⟩ →* ⟨⟪b0..b_{n-1}⟫, Σ'⟩
        ────────────────────────────────────────────
        ⟨str_ptr(s), Σ⟩ → ⟨p : rawptr<u8>, Σ'[ storage(p+j) := byte(b_j) for j < n ]⟩
        side-conditions: [p, p+n) ∩ (⋃ extent(o,Σ') for o ∈ Σ'.objects) = ∅

`⟪…⟫` is the `str` value whose bytes are listed (`spec/06` §7's
`Value` grammar). `str_len` and `str_byte` are total over the value
alone; `str_byte` is `checked` exactly as `[Index-Checked]` is, and
its literal-index case is refuted statically by
`rule.control.flow-analysis`'s value-range row the same way. `str_ptr`
is safe, like `[Rawptr-Of]`: it yields an address whose cells hold the
value's bytes, outside every live object's extent, so no access path
can be invalidated by reading them; what a program then does with the
address is governed by `rule.trust.rawptr` and `rule.trust.extern-call`
inside `unsafe`. Which address (and whether two evaluations of the same
bytes share one) is implementation-defined and must be documented.

**Depends on:** type.str, inv.str.utf8-validity, rule.stdlib.prelude,
rule.trust.rawptr, rule.agg.index, rule.control.flow-analysis, D-0020
**Affects:** state.storage

### `rule.stdlib.print`
**Status:** ACCEPTED

`print<T>(T x)` is a function of `std` that `std` does not export
(D-0039): a program writes output with `printf` (§2f), and `print` is
what `printf`, `%v` and `String::append` are defined through. It is
realized natively by both implementations (§0's latitude, as `map_err`
is), because its behaviour depends on `T` and a CobaltC body cannot
(D-0010, D-0028). Its printable types are `str`, every integer type,
`f32`, `f64`, `bool` and `ref<String, shared>`.

    [Print]
        T printable    ⟨x, Σ⟩ →* ⟨v, Σ1⟩    text(v) = b0..b_{n-1}
        ────────────────────────────────────────────
        Γ ⊢ print(x) : void;   ⟨print(x), Σ⟩ → ⟨(), Σ1⟩, writing b0..b_{n-1} to standard output
                               as [Extern-Call] of write(p, n) does

    [Print-Not-Printable]   disposition: rejected
        T not printable, or not one argument
        ────────────────────────────────────────────
        ill-formed; diag.type-mismatch (static)

    text(⟪b0..b_{n-1}⟫ : str)            = b0..b_{n-1}
    text(v : ref<String, shared>)       = the bytes of the String v refers to
    text(v : τ integer)                 = [Print-Int]
    text(v : τ ∈ {f32, f64})            = [Print-Float]
    text(true), text(false)             = `true`, `false`

    [Print-Int]     the decimal digits of |v| with no leading zeros (`0` for zero), preceded by `-` if v < 0

    [Print-Float]
        NaN → `NaN`;   +∞ → `inf`;   −∞ → `-inf`;   otherwise `-` if v is negative (−0.0 included), then:
        let d1…dn and k be the shortest decimal digit string, and its exponent, with
            d1.d2…dn × 10^k read back (round to nearest, ties to even) as |v| in τ, d1 ≠ 0 unless v is zero;
            among equally short strings, the one nearest |v|, and on an exact tie the one whose dn is even;
        if −7 < k < 21:  positional, with at least one fraction digit:
            k ≥ 0:  d1…d_{k+1} (padded with 0s to k+1 digits) `.` d_{k+2}…dn (or `0` if none)
            k < 0:  `0.` followed by −k−1 zeros and d1…dn
        otherwise:       d1 `.` d2…dn (or `0` if n = 1) `e` k   (k in decimal, `-` if negative, no `+`)

For example `printf("%v", 0.1)` writes `0.1`, `printf("%v", 1.0)` `1.0`,
`printf("%v", 1.0 / 3.0)` `0.3333333333333333`, `printf("%v", 1.0e-7)` `1.0e-7`, and
`printf("%v", 0.1: f32)` `0.1` (the digits are chosen in `f32`, not in `f64`).
The shortest-digits rule is the one JavaScript's and Python's
number-to-text conversions use, so every printed float reads back as
exactly the value printed.

- **Where the type is checked.** A type parameter may be passed to
  `%v` (`rule.type.kind`); each instantiation is checked, and a `T`
  that is not printable is `[Print-Not-Printable]` there.
- **`String` by reference.** A `String` passed by value would be moved
  and destroyed; only `ref<String, shared>` is printable, so a program
  writes `printf("%v", &s)`.
- **`str` and `String`** are written through `stdout_write`, with the `unsafe`
  block inside `std`: a program that only prints never writes `unsafe`
  itself.
- **Failure.** `stdout_write`'s `isize` claim is discarded; a short or failed
  write is not observable (`feat.str-literal`; a fallible `printf` is a
  revisit condition of D-0020).
- **Not a program's name.** `print` is private to `std`
  (`spec/17`): in a program `print(x)` is `diag.unbound-name` and
  `std::print(x)` is `diag.name-not-visible`, and a program may
  declare its own `print`. Composite values (a `Vec`, an `Option`, a struct) are
  written by the program.

**Depends on:** type.str, rule.stdlib.string, rule.type.kind,
rule.trust.extern-call, rule.trust.unsafe, D-0020, D-0024, D-0028,
D-0039

## 2b. Standard input

In `std::io`.

### `rule.stdlib.read`
**Status:** ACCEPTED

    export extern fn stdin_read(rawptr<u8> buf, usize len) : isize;

    export fn read_line() : Result<Option<String>, FileError>
    {
        rawptr<u8> cell = match (allocate(1, 1))
        {
            Ok(p) : p,
            Err(_) : fault(alloc_failure),
        };
        Vec<u8> bytes = Vec::new();
        isize n = 0;
        bool any = false;
        while (true)
        {
            n = unsafe
            {
                stdin_read(cell, 1)
            };
            if (n <= 0)
            {
                break;
            }
            any = true;
            u8 c = unsafe
            {
                *cell
            };
            if (c == 10)
            {
                if (bytes.len > 0 && bytes[bytes.len - 1] == 13)
                {
                    Vec::pop(&mut bytes);
                }
                break;
            }
            Vec::push(&mut bytes, c);
        }
        unsafe
        {
            deallocate(cell, 1, 1);
        }
        if (n < 0)
        {
            return Err(Io);
        }
        if (!any)
        {
            return Ok(None);
        }
        match (String::from_utf8(bytes))
        {
            Ok(s) : Ok(Some(s)),
            Err(e) : Err(Utf8(e)),
        }
    }

    [Read]   an instance of [Extern-Call] (spec/20 §3)   disposition: trusted-unchecked
        p : rawptr<u8>    n : usize    the program's standard input supplies its next k bytes b0..b_{k-1}, 0 ≤ k ≤ n
        ────────────────────────────────────────────
        ⟨stdin_read(p, n), Σ⟩ → ⟨claim : isize, Σ[ storage(p+j) := byte(b_j) for j < k ]⟩
            claim = k;  k = 0 only when n = 0 or the input has ended;  claim = −1, storing nothing, when
            the input cannot be read
        side-conditions: ⟦ [p, p+n) is storage outside every live object, as [Extern-Call] requires ⟧
                         discharge: trusted

`stdin_read` is `stdout_write`'s counterpart: an `extern` declared in `std`, called
inside `unsafe`, reading the bytes that come next on the program's
standard input. Its result is a claim like any extern's.

`read_line` is the safe way to read: it returns the next line as a
`String`, with no `unsafe` in the calling program. Its error is a
`FileError`, the one error of input and output (D-0134); of its
variants, only `Io` and `Utf8` occur for standard input.

- **A line** is every byte up to the next `\n` (byte 10), which is
  consumed and not included, or up to the end of input. A `\r` (byte
  13) immediately before that `\n` is consumed with it and not
  included (D-0092): a Windows line ending reads as a Unix one, on
  every platform. Any other `\r` is part of the line, and a last line
  ended by the end of input keeps a final `\r`, since no `\n` was
  consumed.
- **`Ok(None)`** means the input ended before the line's first byte.
  A last line with no `\n` is still `Ok(Some(…))`, and every call after
  the end is `Ok(None)` again.
- **`Err(Utf8(e))`** means the line is not UTF-8; `e.offset` is the bad
  byte's offset within the line. The line has been consumed, so the
  next call reads the line after it.
- **`Err(Io)`** means `stdin_read` reported that the input cannot be read.
  The bytes of that line already read are discarded.

Standard input belongs to the program's environment, which is outside
this specification (Master Instructions §1). An implementation
documents whether it provides standard input. One that does not stops
the program at its first call of `stdin_read`, saying so; that stop is not a
diagnostic of this specification, and everything the program did
before it happened as usual. A program that never reads runs the same
either way.

**Depends on:** rule.trust.extern-call, rule.trust.unsafe,
rule.trust.rawptr, rule.stdlib.vec, rule.stdlib.string, D-0029
**Affects:** state.storage

## 2c. Program arguments

In `std::sys`.

### `rule.stdlib.args`
**Status:** ACCEPTED

    export extern fn arg_bytes(usize i, rawptr<u8> buf, usize len) : isize;

    export fn arg_count() : usize
    {
        usize i = 0;
        while (true)
        {
            isize n = unsafe
            {
                arg_bytes(i, dangling<u8>(), 0)
            };
            if (n < 0)
            {
                break;
            }
            i = i + 1;
        }
        i
    }

    export fn arg(usize i) : Result<String, Utf8Error>
    {
        isize n = unsafe
        {
            arg_bytes(i, dangling<u8>(), 0)
        };
        if (n < 0)
        {
            fault(index_out_of_bounds);
        }
        usize len = reinterpret<usize>(n);
        rawptr<u8> p = match (allocate(len, 1))
        {
            Ok(p) : p,
            Err(_) : fault(alloc_failure),
        };
        Vec<u8> bytes = Vec::new();
        unsafe
        {
            arg_bytes(i, p, len);
        }
        usize j = 0;
        while (j < len)
        {
            u8 b = unsafe
            {
                *(p + reinterpret<isize>(j))
            };
            Vec::push(&mut bytes, b);
            j = j + 1;
        }
        if (len > 0)
        {
            unsafe
            {
                deallocate(p, len, 1);
            }
        }
        String::from_utf8(bytes)
    }

    [Arg-Bytes]   an instance of [Extern-Call] (spec/20 §3)   disposition: trusted-unchecked
        i : usize    p : rawptr<u8>    n : usize    the program's arguments are A_0 … A_{c−1}, byte sequences
        ────────────────────────────────────────────
        ⟨arg_bytes(i, p, n), Σ⟩ → ⟨claim : isize, Σ[ storage(p+j) := byte(A_i[j]) for j < min(n, |A_i|) ]⟩
            claim = |A_i| when i < c;  claim = −1, storing nothing, when i ≥ c
        side-conditions: ⟦ [p, p+n) is storage outside every live object, as [Extern-Call] requires ⟧
                         discharge: trusted

The program's **arguments** are a sequence of byte sequences its
environment gives it before `main` begins (`rule.fn.program`), and
they do not change while it runs. Argument 0 is the first argument
after the program itself: the program's own name is not one of them.
How an implementation is given them is implementation-defined and
documented; one with no way to receive arguments gives none (`c = 0`).

`arg_bytes` is an `extern` declared in `std`, called inside `unsafe`,
like `stdin_read`. It copies at most `len` bytes and always returns the
argument's whole length, so a call with `len = 0` asks only whether
argument `i` exists and how long it is. Its result is a claim like any
extern's.

`arg_count` and `arg` are the safe way to read arguments, with no
`unsafe` in the calling program.

- **`arg_count()`** is the number of arguments, `c`.
- **`arg(i)`** is argument `i` as a `String`, or `Err(e)` when it is
  not UTF-8, `e.offset` being the bad byte's offset within it. An
  argument that is not UTF-8 does not affect any other.
- **`arg(i)` with `i ≥ arg_count()`** faults `diag.index-out-of-bounds`
  (`[Fault]`), as `Vec::index_shared` does.

A program that needs an argument's bytes whatever they are calls
`arg_bytes` itself.

**Depends on:** rule.trust.extern-call, rule.trust.unsafe,
rule.trust.rawptr, rule.stdlib.vec, rule.stdlib.string,
rule.fn.program, D-0031
**Affects:** state.storage

## 2d. Text conversions

In `std::text`.

### `rule.stdlib.text`
**Status:** ACCEPTED

    export fn String::new() : String
    {
        String { .bytes = Vec::new() }
    }

    export fn String::as_bytes(ref<String, shared> s) : ref<Vec<u8>, shared>
    {
        &s.bytes
    }

    // Room for n more bytes (D-0129): Vec::reserve on the bytes.
    export fn String::reserve(ref<String, exclusive> s, usize n)
    {
        Vec::reserve(&mut s.bytes, n);
    }

    // A copy of s: a new String with the same bytes (D-0044).
    export fn String::clone(ref<String, shared> s) : String
    {
        String c = String::new();
        String::append_string(&mut c, s);
        c
    }

    // Adds one ASCII byte (below 128) to s; any other byte would not be
    // UTF-8 on its own, and faults (D-0044). Text of arbitrary bytes is
    // built in a Vec<u8> and checked by String::from_utf8.
    export fn String::push_ascii(ref<String, exclusive> s, u8 b)
    {
        if (b >= 128)
        {
            fault(not_ascii);
        }
        Vec::push(&mut s.bytes, b);
    }

    // Empties s, keeping its buffer (D-0044).
    export fn String::clear(ref<String, exclusive> s)
    {
        Vec::clear(&mut s.bytes);
    }

    [Push-Ascii-Not-Ascii]   disposition: fault
        b ≥ 128
        ────────────────────────────────────────────
        ⟨String::push_ascii(s, b), Σ⟩ → fault diag.not-ascii (dynamic); s unchanged

`String::append<T>(ref<String, exclusive> s, T x)` and
`String::parse<T>(ref<String, shared> s) : Result<T, ParseError>` are exported
functions of `std`, realized natively by both implementations (§0's
latitude), because their behaviour depends on `T` and a CobaltC body
cannot (D-0010, D-0032).

    [Append]
        T printable (rule.stdlib.print)    ⟨s, Σ⟩ →* ⟨r, Σ1⟩    ⟨x, Σ1⟩ →* ⟨v, Σ2⟩    text(v) = b0..b_{n-1}
        ────────────────────────────────────────────
        Γ ⊢ String::append(s, x) : void;
        ⟨String::append(s, x), Σ⟩ → ⟨(), Σ3⟩, Σ3 as n calls Vec::push(&mut (*r).bytes, b_j), j = 0..n−1, give it

    [Append-Not-Printable]   disposition: rejected
        T not printable
        ────────────────────────────────────────────
        ill-formed; diag.type-mismatch (static)

    [Parse]
        T an integer type, f32 or f64    the bytes of the String s refers to are c0..c_{n−1}
        ────────────────────────────────────────────
        Γ ⊢ String::parse<T>(s) : Result<T, ParseError>;   ⟨String::parse<T>(s), Σ⟩ → ⟨r, Σ'⟩ with
            r = Err(Empty)            when n = 0;
            r = Err(Invalid(k))       otherwise, when c0..c_{n−1} is not a sentence of number(T):
                                      k is the length of the longest prefix of c0..c_{n−1} that is a
                                      prefix of some sentence of number(T)  (so k = n when it ends early);
            r = Err(OutOfRange)       otherwise, when value(T) is not a value of T;
            r = Ok(value(T))          otherwise

    [Parse-Not-Numeric]   disposition: rejected
        T is not an integer type, f32 or f64
        ────────────────────────────────────────────
        ill-formed; diag.type-mismatch (static)

    number(T integer, signed)     := sign? digit+
    number(T integer, unsigned)   := '+'? digit+
    number(T ∈ {f32, f64})        := sign? ( digit+ ('.' digit+)? (('e' | 'E') sign? digit+)? | 'inf' ) | 'NaN'
    sign := '+' | '-'        digit := '0' … '9'

    value(T integer)   = the decimal number the digits denote, negated after a '-'
    value(T float)     = 'inf' → +∞ (−∞ after '-');  'NaN' → a NaN;  otherwise the decimal number
                         d × 10^e the text denotes, rounded to the nearest value of T (ties to even);
                         a finite number that rounds beyond T's largest finite magnitude is not a value of T

`String::append(&mut s, x)` adds the bytes `printf("%v", x)` would write to
`s`; building a line and printing it writes what the separate `printf`
calls would. `String::parse<T>(&s)` accepts the whole `String` or nothing:

- **Strict.** No leading or trailing whitespace, `_`, other base, or
  `.5` / `5.` forms; `-` is invalid for an unsigned `T`, even for
  `-0`. Leading zeros are allowed. `Invalid(k)` points at the first
  byte that cannot continue a number (`"12x4"` → `Invalid(2)`), or past
  the end when the text stops too early (`"-"` → `Invalid(1)`,
  `"1e"` → `Invalid(2)`).
- **Order.** An invalid byte anywhere is `Invalid`, even when the
  digits before it are already out of range.
- **Floats** are correctly rounded in `T` itself (`String::parse<f32>` does
  not round through `f64`). Every text `%v` writes for a float `v`
  parses back to `v` in the same type (a NaN to a NaN).
- **Where `T` is checked.** At the call, as for `%v`; a type
  parameter passed through is checked at each instantiation.
- **Names.** `String::parse`, `ParseError` and its variants are items
  of `std` (D-0134: `parse` until then); a program's own of the same
  name takes precedence (D-0024).

**Depends on:** rule.stdlib.print, rule.stdlib.string, rule.stdlib.vec,
rule.type.kind, rule.arith.represent, D-0032
**Affects:** state.storage (through `Vec::push`)

## 2e. Files

In `std::io`.

### `rule.stdlib.file`
**Status:** ACCEPTED

`read_file` and `write_file` are exported functions of `std`, realized
natively by both implementations (§0's latitude), since they reach the
program's environment, as `stdin_read` and `arg_bytes` do. A path, and
`write_file`'s text, is a `StringView` (D-0134): a literal
(`[Str-Literal-View]`), or `&s[0..$]` of a `String`.

    [Read-File]
        the bytes of the view path name a file of the environment;
        its contents are the bytes c0..c_{n−1}
        ────────────────────────────────────────────
        ⟨read_file(path), Σ⟩ → ⟨r, Σ'⟩ with
            r = Ok(s)                   the String of c0..c_{n−1}, when they are UTF-8 (String::from_utf8);
            r = Err(Utf8(e))            when they are not, e as String::from_utf8 gives it;
            r = Err(NotFound)           when no such file exists;
            r = Err(Denied)             when the environment refuses to let the program read it;
            r = Err(Io)                 for any other failure (a directory, a device error, …)

    [Write-File]
        the bytes of the view path name a file of the environment
        ────────────────────────────────────────────
        ⟨write_file(path, text), Σ⟩ → ⟨r, Σ'⟩ with
            r = Ok(())                  the file now holds exactly the bytes of text, created if it did
                                        not exist, its previous contents replaced if it did;
            r = Err(NotFound)           when a directory on the path does not exist;
            r = Err(Denied)             when the environment refuses the write;
            r = Err(Io)                 for any other failure

Files belong to the program's environment, like its arguments and
standard input (`rule.fn.program`). How a path names a file is
implementation-defined and documented; both implementations pass it to
the operating system, which resolves a relative path against the
working directory. A file's contents are whatever the environment
holds when it is read, so `read_file` of a file another program
changes is not deterministic; every other part of the program is
unaffected.

- **Whole files.** `read_file` returns the whole file, `write_file`
  replaces the whole file; `File` (`rule.stdlib.file-handle`) reads and
  writes one in pieces.
- **Text.** Both work in text: `read_file` checks the bytes are
  UTF-8, reporting the offset of the first that is not, as `read_line`
  does; `write_file` can write only UTF-8 (a view's bytes are).
- **One error.** `FileError` is the error of every input and output
  operation of `std`, standard input's included (D-0134).

**Depends on:** rule.stdlib.string, rule.stdlib.vec, rule.fn.program,
D-0034
**Affects:** the program's environment

### `rule.stdlib.file-handle`
**Status:** ACCEPTED

A `File` (D-0054) is an open file of the environment, read and written
in order from a position, for files too large to hold, for bytes that
are not text, and for appending. It is a resource (`spec/07`): one
owner, moved into a thread with `spawn` like any other, destroyed
exactly once, and its destructor closes the file. Its fields are
private to `std`, so every `File` comes from opening one.

    [File-Open]
        the bytes of the String path refers to name a file of the environment
        ────────────────────────────────────────────
        ⟨File::open(path), Σ⟩ → ⟨r, Σ'⟩     for reading; the file must exist
        ⟨File::create(path), Σ⟩ → ⟨r, Σ'⟩   for writing: created, or its contents discarded
        ⟨File::append(path), Σ⟩ → ⟨r, Σ'⟩   for writing at its end, created if it does not exist
        ⟨File::open_rw(path), Σ⟩ → ⟨r, Σ'⟩  for reading and writing, created if it does not exist,
                                             its contents kept
            r = Ok(f)          f open, its position 0;
            r = Err(NotFound)  no such file (open), or a directory on the path does not exist;
            r = Err(Denied)    the environment refuses;
            r = Err(Io)        any other failure (the path names a directory, …)

    [File-Read]
        f open for reading, at position p, the file's bytes c0..c_{n−1}; k = min(max, 2^20)
        ────────────────────────────────────────────
        ⟨File::read(f, buf, max), Σ⟩ → ⟨Ok(j), Σ'⟩
            buf gains c_p..c_{p+j−1} at its end, f's position becomes p + j, with
            j = min(k, n − p) (fewer only at the end of the file, CHG-0067);
        ⟨File::read_to_end(f, buf), Σ⟩ → ⟨Ok(n − p), Σ'⟩
            buf gains c_p..c_{n−1}; f's position becomes n

    [File-Read-Line]
        f open for reading, at position p; q the position after the first byte 10 at or
        after p, or n when there is none
        ────────────────────────────────────────────
        ⟨File::read_line(f), Σ⟩ → ⟨r, Σ'⟩, f's position becomes q
            r = Ok(None)              when p = n;
            r = Ok(Some(s))           s the String of c_p..c_{q−1} without a final byte 10,
                                      when they are UTF-8 (String::from_utf8);
            r = Err(Utf8(e))          when they are not, e's offset from c_p

    [File-Write]
        f open for writing, at position p
        ────────────────────────────────────────────
        ⟨File::write(f, data), Σ⟩ → ⟨Ok(()), Σ'⟩
            the file holds data's bytes from p (from its end, for append), growing as needed;
            f's position becomes the byte after them;
        ⟨File::write_text(f, text), Σ⟩ is ⟨File::write(f, text's bytes), Σ⟩      -- text a StringView (D-0134)

    [File-Seek]
        ⟨File::seek(f, pos), Σ⟩ → ⟨Ok(()), Σ'⟩, f's position becomes pos
        (beyond the end is allowed: reading there gives 0 bytes, writing there extends the file)
        ⟨File::len(f), Σ⟩ → ⟨Ok(n), Σ⟩, n the file's length in bytes

    [File-Printf]   (D-0059)
        ⟨File::printf(f, format, a1, …, an), Σ⟩ is ⟨File::write_formatted(f, text), Σ⟩ with text the
        formatted text [Printf] would print (its format checked the same way, [Format-Invalid],
        [Format-Arg-Mismatch]); `write_formatted`, private to `std`, writes its bytes as [File-Write] does

    [File-Close]
        ⟨File::close(f), Σ⟩ → ⟨r, Σ'⟩, f closed and destroyed
            r = Ok(())  when f was open only for reading, or its bytes reached the
                        environment's storage (made durable);
            r = Err(e)  when the environment reports a failure; f is closed all the same

    [File-Drop]
        destroying an open f closes it, without making it durable and without reporting a failure

Any operation above gives `Err(Io)` (or `Err(Denied)`, `Err(NotFound)`)
when the environment reports a failure, as `[Read-File]` does. `read` or
`read_line` on a file opened only for writing, and `write` on one opened
only for reading, give exactly `Err(Io)`, the same on every platform
(`CHG-0071`).

    [Read-Bytes]
        ⟨read_bytes(path), Σ⟩ → ⟨r, Σ'⟩ with r = Ok(v), v the file's bytes, or the error
        [File-Open]'s open or [File-Read]'s read_to_end gives
    [Write-Bytes]
        ⟨write_bytes(path, data), Σ⟩ → ⟨r, Σ'⟩ with r = Ok(()), the file holding exactly
        data's bytes, or the error [File-Open]'s create or [File-Write] gives

- **Buffering.** Reading is buffered: `read_line` does not read the
  environment a byte at a time, and a `read` returns `max` bytes (at
  most a MiB), fewer only at the end of the file. Writing is not: each `write` is handed to the
  environment before it returns, so another program sees it. Mixing
  them on an `open_rw` file is exact: a write lands at the position
  reading reached.
- **Positions** are `u64`, not `usize`, so a file larger than the
  address space is still addressed.
- **Lines.** `read_line` removes the final `'\n'`, and one `'\r'`
  immediately before it, as `read_line` on standard input does
  (D-0092); any other `'\r'` stays.
- **Closing.** `close` reports a failure; the destructor cannot, and
  closes without waiting. A program that must know its bytes were
  stored calls `close`.
- **Handles.** A `File` holds the index of an entry in a table of the
  implementation's, not the operating system's handle; both
  implementations share one (`impl/src/fileio.rs`).

The `std` source:

    // `File` (`rule.stdlib.file-handle`, spec/21 §2e, D-0054) is written over
    // these two, private to `std`: `op` names the operation, `h` the open file
    // (or the mode, for opening), `buf`/`n` its bytes or its count; `file_at`
    // takes or gives a position. Each returns a handle, a count, a length or
    // 0, or a failure code as `file_read`'s (src/fileio.rs).
    extern fn file_op(usize op, usize h, rawptr<u8> buf, usize n) : isize;
    extern fn file_at(usize op, usize h, u64 pos) : i64;

    // An open file (`rule.stdlib.file-handle`, spec/21 §2e, D-0054): its
    // bytes read and written in order from a position, which `seek` moves.
    // Its destructor closes it; `close` closes it and reports a failure.
    export resource struct File
    {
        usize id;
        bool open;                          // false once close has closed it
    }

    // `file_error` for `file_at`'s codes.
    fn file_error_at(i64 code) : FileError
    {
        if (code == -1)
        {
            return FileError::NotFound;
        }
        if (code == -2)
        {
            return FileError::Denied;
        }
        FileError::Io
    }

    fn File::start(StringView path, usize mode) : Result<File, FileError>
    {
        isize r = unsafe
        {
            file_op(0, mode, StringView::ptr(path), StringView::len(path))
        };
        if (r < 0)
        {
            return Err(file_error(r));
        }
        Ok(File { .id = reinterpret<usize>(r), .open = true })
    }

    // For reading; the file must exist.
    export fn File::open(StringView path) : Result<File, FileError>
    {
        File::start(path, 0)
    }

    // For writing, from empty: created, or its contents discarded.
    export fn File::create(StringView path) : Result<File, FileError>
    {
        File::start(path, 1)
    }

    // For writing at its end, created if it does not exist.
    export fn File::append(StringView path) : Result<File, FileError>
    {
        File::start(path, 2)
    }

    // For reading and writing, created if it does not exist, its contents kept.
    export fn File::open_rw(StringView path) : Result<File, FileError>
    {
        File::start(path, 3)
    }

    // Appends exactly `want` bytes of what `file_op` reads, or fewer at the end.
    fn File::take(ref<File, exclusive> f, ref<Vec<u8>, exclusive> buf, usize want) : Result<usize, FileError>
    {
        while (buf.cap - buf.len < want)
        {
            Vec::grow(buf);
        }
        isize r = unsafe
        {
            file_op(3, f.id, reinterpret_ptr<u8>(buf.ptr + reinterpret<isize>(buf.len)), want)
        };
        if (r < 0)
        {
            return Err(file_error(r));
        }
        usize n = reinterpret<usize>(r);
        buf.len = buf.len + n;
        Ok(n)
    }

    // Appends up to `max` bytes to `buf` (at most a MiB, and fewer when fewer
    // are ready) and returns how many: 0 only at the end, when max > 0.
    export fn File::read(ref<File, exclusive> f, ref<Vec<u8>, exclusive> buf, usize max) : Result<usize, FileError>
    {
        usize want = if (max > 1_048_576)
        {
            1_048_576
        }
        else
        {
            max
        };
        File::take(f, buf, want)
    }

    // Appends the rest of the file to `buf` and returns how many bytes.
    export fn File::read_to_end(ref<File, exclusive> f, ref<Vec<u8>, exclusive> buf) : Result<usize, FileError>
    {
        usize total = 0;
        while (true)
        {
            usize n = match (File::read(f, buf, 65_536))
            {
                Ok(n) : n,
                Err(e) : return Err(e),
            };
            if (n == 0)
            {
                break;
            }
            total = total + n;
        }
        Ok(total)
    }

    // The next line without its '\n', nor a '\r' just before it (D-0092),
    // as `read_line` reads standard input; None at the end of the file.
    export fn File::read_line(ref<File, exclusive> f) : Result<Option<String>, FileError>
    {
        isize r = unsafe
        {
            file_op(4, f.id, dangling<u8>(), 0)
        };
        if (r < 0)
        {
            return Err(file_error(r));
        }
        if (r == 0)
        {
            return Ok(None);
        }
        Vec<u8> bytes = Vec::new();
        usize n = match (File::take(f, &mut bytes, reinterpret<usize>(r)))
        {
            Ok(n) : n,
            Err(e) : return Err(e),
        };
        if (n > 0 && bytes[n - 1] == 10)
        {
            Vec::pop(&mut bytes);
            if (n > 1 && bytes[n - 2] == 13)
            {
                Vec::pop(&mut bytes);
            }
        }
        match (String::from_utf8(bytes))
        {
            Ok(s) : Ok(Some(s)),
            Err(e) : Err(FileError::Utf8(e)),
        }
    }

    // Writes all of `data` at the file's position (at its end, for append).
    export fn File::write(ref<File, exclusive> f, slice<u8, shared> data) : Result<void, FileError>
    {
        usize n = slice_len(data);
        if (n == 0)
        {
            return Ok(());
        }
        isize r = unsafe
        {
            file_op(5, f.id, rawptr_of(&data[0]), n)
        };
        if (r < 0)
        {
            return Err(file_error(r));
        }
        Ok(())
    }

    // D-0134: writes the text at the file's position.
    export fn File::write_text(ref<File, exclusive> f, StringView text) : Result<void, FileError>
    {
        File::write(f, text.bytes)
    }

    // `File::printf(f, format, …)` (D-0059) is `File::write_formatted(f,
    // $fmt(format, …))` after `modres`, as `String::appendf` is
    // `String::append` of it.
    fn File::write_formatted(ref<File, exclusive> f, str text) : Result<void, FileError>
    {
        usize n = str_len(text);
        if (n == 0)
        {
            return Ok(());
        }
        isize r = unsafe
        {
            file_op(5, f.id, str_ptr(text), n)
        };
        if (r < 0)
        {
            return Err(file_error(r));
        }
        Ok(())
    }

    // Moves the position to byte `pos` from the start.
    export fn File::seek(ref<File, exclusive> f, u64 pos) : Result<void, FileError>
    {
        i64 r = unsafe
        {
            file_at(6, f.id, pos)
        };
        if (r < 0)
        {
            return Err(file_error_at(r));
        }
        Ok(())
    }

    // The file's length in bytes.
    export fn File::len(ref<File, shared> f) : Result<u64, FileError>
    {
        i64 r = unsafe
        {
            file_at(7, f.id, 0)
        };
        if (r < 0)
        {
            return Err(file_error_at(r));
        }
        Ok(reinterpret<u64>(r))
    }

    // Closes the file. A file open for writing is first made durable, so a
    // failure the environment reports late is reported here.
    export fn File::close(File f) : Result<void, FileError>
    {
        f.open = false;
        isize r = unsafe
        {
            file_op(1, f.id, dangling<u8>(), 0)
        };
        if (r < 0)
        {
            return Err(file_error(r));
        }
        Ok(())
    }

    export fn File::drop(ref<File, exclusive> self)
    {
        if (self.open)
        {
            isize r = unsafe
            {
                file_op(2, self.id, dangling<u8>(), 0)
            };
        }
    }

    // The whole file's bytes, as `read_file` reads its text.
    export fn read_bytes(StringView path) : Result<Vec<u8>, FileError>
    {
        File f = match (File::open(path))
        {
            Ok(f) : f,
            Err(e) : return Err(e),
        };
        Vec<u8> bytes = Vec::new();
        match (File::read_to_end(&mut f, &mut bytes))
        {
            Ok(_) : Ok(bytes),
            Err(e) : Err(e),
        }
    }

    // Makes the file hold exactly `data`, as `write_file` does its text.
    export fn write_bytes(StringView path, slice<u8, shared> data) : Result<void, FileError>
    {
        File f = match (File::create(path))
        {
            Ok(f) : f,
            Err(e) : return Err(e),
        };
        File::write(&mut f, data)
    }

**Depends on:** rule.stdlib.file, rule.stdlib.vec, rule.agg.slice,
rule.resauth.destroy, rule.conc.spawn, D-0054
**Affects:** the program's environment

## 2e′. Directories, the environment and the clocks

In `std::sys`.

### `rule.stdlib.fs`
**Status:** ACCEPTED

Paths are `String`s, resolved as `rule.stdlib.file`'s are. Failures are
`FileError`s, with `rule.stdlib.file`'s meanings (D-0061).

    [Make-Dir]      ⟨make_dir(p), Σ⟩ → Ok(true), the directory p made; Ok(false) when a directory
                    is already at p; Err(NotFound) when p's parent does not exist; Err(Io) when
                    something that is not a directory is at p
    [Remove-File]   ⟨remove_file(p), Σ⟩ → Ok(()), the file gone; Err(NotFound) when there is none
    [Remove-Dir]    ⟨remove_dir(p), Σ⟩ → Ok(()), the empty directory gone; Err(Io) when it is not
                    empty; Err(NotFound) when there is none
    [Rename]        ⟨rename(a, b), Σ⟩ → Ok(()), what was at a now at b (a file at b replaced)
    [List-Dir]      ⟨list_dir(p), Σ⟩ → Ok(names), the names of p's entries (never `.` or `..`)
                    sorted by their bytes; Err(Utf8(e)) for a name that is not UTF-8
    [Path-Kind]     ⟨path_kind(p), Σ⟩ → Ok(File(n)) a file of n bytes, Ok(Dir), Ok(Other) (a
                    device, a socket, …), following a symbolic link; Err(NotFound) when nothing
                    is at p

### `rule.stdlib.env`
**Status:** ACCEPTED

    [Env-Var]       ⟨env_var(name), Σ⟩ → Ok(Some(v)) the variable's value, Ok(None) when it is not
                    set, Err(e) when its value is not UTF-8 (e's offset, as String::from_utf8's)
    [Current-Dir]   ⟨current_dir(), Σ⟩ → Ok(d), the working directory; Err(Utf8(e)) when its
                    name is not UTF-8
    [Clocks]        ⟨monotonic_ns(), Σ⟩ → n, nanoseconds on a clock that never goes backwards,
                    from a start fixed for the whole program; ⟨unix_seconds(), Σ⟩ → s, whole
                    seconds since 1970-01-01 00:00 UTC by the wall clock (which may be changed)

The filesystem, the environment and the clocks belong to the program's
environment (`rule.fn.program`): their answers are whatever the
environment gives when asked, so a program using them is not
deterministic in them; every other part of it is unaffected.

The `std` source:

    // The filesystem, the environment and the clocks (`rule.stdlib.fs`,
    // `rule.stdlib.env`, D-0061), over three primitives private to `std`
    // (src/fileio.rs): `fs_op` acts on one or two paths; `fs_query` copies up
    // to `cap` bytes of an answer into `buf` and returns its whole length
    // (asked again with room for all of it when longer); `clock_read` reads a
    // clock. Failures are `file_read`'s codes.
    extern fn fs_op(usize op, rawptr<u8> a, usize an, rawptr<u8> b, usize bn) : isize;
    extern fn fs_query(usize op, rawptr<u8> a, usize an, rawptr<u8> buf, usize cap) : isize;
    extern fn clock_read(usize which) : i64;

    // What a path names (`rule.stdlib.fs`, D-0061): a file and its length in
    // bytes, a directory, or something else (a device, a socket, …).
    export enum PathKind
    {
        File(u64),
        Dir,
        Other,
    }

    fn fs_path_op(usize op, StringView a, StringView b) : isize
    {
        unsafe
        {
            fs_op(op, StringView::ptr(a), StringView::len(a), StringView::ptr(b), StringView::len(b))
        }
    }

    // The bytes of `fs_query`'s answer, or its failure code.
    fn fs_answer(usize op, rawptr<u8> a, usize an) : Result<Vec<u8>, isize>
    {
        Vec<u8> out = Vec::new();
        usize cap = 256;
        while (true)
        {
            while (out.cap < cap)
            {
                Vec::grow(&mut out);
            }
            isize n = unsafe
            {
                fs_query(op, a, an, reinterpret_ptr<u8>(out.ptr), cap)
            };
            if (n < 0)
            {
                return Err(n);
            }
            usize len = reinterpret<usize>(n);
            if (len <= cap)
            {
                out.len = len;
                return Ok(out);
            }
            cap = len;
        }
        Err(-3)
    }

    // Makes a directory: Ok(true) when it was made, Ok(false) when a
    // directory was already there (so it is safe to call again).
    export fn make_dir(StringView path) : Result<bool, FileError>
    {
        isize r = fs_path_op(0, path, "");
        if (r < 0)
        {
            return Err(file_error(r));
        }
        Ok(r == 1)
    }

    export fn remove_file(StringView path) : Result<void, FileError>
    {
        isize r = fs_path_op(1, path, "");
        if (r < 0)
        {
            return Err(file_error(r));
        }
        Ok(())
    }

    // Removes an empty directory.
    export fn remove_dir(StringView path) : Result<void, FileError>
    {
        isize r = fs_path_op(2, path, "");
        if (r < 0)
        {
            return Err(file_error(r));
        }
        Ok(())
    }

    // Renames (moves) `from` to `to`, replacing a file at `to`.
    export fn rename(StringView from, StringView to) : Result<void, FileError>
    {
        isize r = fs_path_op(3, from, to);
        if (r < 0)
        {
            return Err(file_error(r));
        }
        Ok(())
    }

    // The names in a directory (not `.` or `..`), sorted by their bytes.
    export fn list_dir(StringView path) : Result<Vec<String>, FileError>
    {
        Vec<u8> all = match (fs_answer(0, StringView::ptr(path), StringView::len(path)))
        {
            Ok(b) : b,
            Err(c) : return Err(file_error(c)),
        };
        Vec<String> names = Vec::new();
        Vec<u8> name = Vec::new();
        foreach (b in &all)
        {
            match (*b)
            {
                0 :
                {
                    Vec<u8> done = replace(&mut name, Vec::new());
                    match (String::from_utf8(done))
                    {
                        Ok(n) : Vec::push(&mut names, n),
                        Err(e) : return Err(FileError::Utf8(e)),
                    }
                },
                c : Vec::push(&mut name, c),
            }
        }
        Ok(names)
    }

    // What `path` names; `Err(NotFound)` when nothing does.
    export fn path_kind(StringView path) : Result<PathKind, FileError>
    {
        Vec<u8> b = match (fs_answer(1, StringView::ptr(path), StringView::len(path)))
        {
            Ok(b) : b,
            Err(c) : return Err(file_error(c)),
        };
        u64 len = 0;
        usize i = 8;
        while (i > 0)
        {
            len = (len << 8) | widen<u64>(b[i]);
            i -= 1;
        }
        match (b[0])
        {
            0 : Ok(File(len)),
            1 : Ok(Dir),
            _ : Ok(Other),
        }
    }

    // An environment variable's value; None when it is not set.
    export fn env_var(StringView name) : Result<Option<String>, Utf8Error>
    {
        match (fs_answer(2, StringView::ptr(name), StringView::len(name)))
        {
            Ok(b) : match (String::from_utf8(b))
            {
                Ok(v) : Ok(Some(v)),
                Err(e) : Err(e),
            },
            Err(_) : Ok(None),
        }
    }

    // The working directory, against which a relative path is resolved.
    export fn current_dir() : Result<String, FileError>
    {
        Vec<u8> b = match (fs_answer(3, dangling<u8>(), 0))
        {
            Ok(b) : b,
            Err(c) : return Err(file_error(c)),
        };
        match (String::from_utf8(b))
        {
            Ok(s) : Ok(s),
            Err(e) : Err(FileError::Utf8(e)),
        }
    }

    // A monotonic clock, in nanoseconds from an arbitrary start: for measuring
    // how long something takes; it never goes backwards.
    export fn monotonic_ns() : u64
    {
        reinterpret<u64>(unsafe
        {
            clock_read(0)
        })
    }

    // The wall clock: whole seconds since 1970-01-01 00:00 UTC.
    export fn unix_seconds() : i64
    {
        unsafe
        {
            clock_read(1)
        }
    }

**Depends on:** rule.stdlib.file, rule.stdlib.string, rule.fn.program, D-0061
**Affects:** the program's environment

## 2f. Formatted output

In `std::io` (`printf`, `eprintf`, `File::printf`) and `std::text` (`sprintf`, `String::appendf`).

### `rule.stdlib.format`
**Status:** ACCEPTED

`printf(f, a1, …, an)` writes the text `format(f, a1, …, an)` to
standard output, as `print` of that text would (§2a). `String::appendf(s, f,
a1, …, an)` adds the same text to the `String` `*s`, as
`String::append` of it would, and `sprintf(f, a1, …, an)` returns it as
a new `String`; `eprintf(f, a1, …, an)` writes it to standard error.
All four are exported functions of `std`, realized natively (D-0038,
D-0039, D-0040); each call is typed on its own, with any number of
arguments, as `spawn`'s is. `printf` is a program's only way to write
to standard output (D-0039), and `eprintf` to standard error.

    [Printf]    ⟨printf(f, a1..an), Σ⟩ ≡ ⟨print(t), Σ⟩                     t = format(f, v1..vn), a_i evaluated left to right
    [Appendf]   ⟨String::appendf(s, f, a1..an), Σ⟩ ≡ ⟨String::append(s, t), Σ⟩   s, then a1..an, left to right
    [Sprintf]   ⟨sprintf(f, a1..an), Σ⟩ ≡ ⟨String::from_str(t), Σ⟩        Γ ⊢ sprintf(f, a1..an) : String
    [Eprintf]   ⟨eprintf(f, a1..an), Σ⟩ → ⟨(), Σ1⟩, writing t's bytes to standard error
                as [Extern-Call] of std's private stderr_write(p, n) does    (Σ1 as for [Printf])

    [Format-Invalid]   disposition: rejected
        f is not a string literal, or not a sentence of format (below)
        ────────────────────────────────────────────
        ill-formed; diag.format-invalid

    [Format-Arg-Mismatch]   disposition: rejected
        the number of arguments f takes (one per specifier, and one `usize` before it for each `*`)
        is not n, or a_i's type is not one its place takes
        (a type parameter: at each instantiation, rule.type.kind)
        ────────────────────────────────────────────
        ill-formed; diag.type-mismatch

    format     := (byte other than '%' | '%%' | spec)*
    spec       := '%' flag* width? ('.' precision?)? conversion      -- width, precision: digits, at most 4096,
                                                                      -- or `*` (D-0100)
    flag       := '-' | '0' | '+' | ' ' | '#'

| Conversion | Argument | Text | Flags it takes |
|---|---|---|---|
| `d` `i` `u` | any integer type | the value in decimal | `- 0 + space`; a precision is a minimum digit count |
| `x` `X` `o` `b` | any integer type | the value's bits in its own width, in base 16 (lower/upper case), 8, 2 | `- 0 #` (`#`: prefix `0x`, `0X`, `0o`, `0b`); precision as for `d` |
| `f` `F` | `f32`, `f64` | fixed point, `precision` digits after the point (6 by default), correctly rounded | `- 0 + space #` |
| `e` `E` | `f32`, `f64` | `d.ddde±XX`, `precision` digits after the point (6), an exponent of at least two digits | as `f` |
| `g` `G` | `f32`, `f64` | `precision` significant digits (6; 0 is 1): `e` form when the exponent is below −4 or at least the precision, else `f` form; trailing zeros removed unless `#` | as `f` |
| `s` | `str`, `ref<String, shared>`, `bool` (`true`/`false`) | the text, at most `precision` characters | `-` |
| `v` | any printable type (§2a), or an enum (or a reference to one) | `text(v)`: what `String::append` adds — an integer in decimal, a float in the shortest form that reads back, text and `bool` as they are; an enum value is its variant's name (D-0100), its payload not shown | `-` (no precision) |

- **As C.** Every conversion prints what C's `printf` prints for the
  same value, but `%d` takes every integer type (there is no `%ld`),
  `%x`, `%o` and `%b` print a negative number's bits in its own width
  (`%x` of `-1: i8` is `ff`), and `%#o` writes CobaltC's octal prefix
  `0o` (a leading `0` alone is not octal here). `%b` is binary.
- **Padding.** A width pads with spaces on the left, or on the right
  with `-`; with `0`, a number is padded with zeros after its sign and
  prefix (not for an integer with a precision, nor for `inf`/`nan`).
  Widths and precisions count characters, not bytes, so text with
  multi-byte characters aligns and is never split.
- **Floats.** An `f32` is formatted from its exact value (as C does
  once it is promoted to `double`): `%f` of `0.1: f32` is `0.100000`.
  A NaN is `nan`, an infinity `inf` (`NAN`, `INF` for the upper-case
  conversions), with the sign and `+`/space flags as for a number.
- **Computed widths** (D-0100). `*` in place of a width or precision
  takes the next argument, a `usize`, before the value, as C's does:
  `printf("%-*s|", w, name)`. A computed width or precision above 4096
  counts as 4096.
- **Enums** (D-0100). `%v` of an enum value writes its variant's name
  (`Red`, `NotFound`, `Some`); print a payload with `match`.
- **Not included:** `%c` (a byte above 127 would not be UTF-8 in a
  `String`), `%p`, `%n`, and length modifiers.
- **References.** An argument that is a reference to a number, a
  `bool` or a `str` (either mode) is formatted as the value it refers
  to, and a `ref<String, exclusive>` as a `ref<String, shared>` (D-0042):
  in `foreach (x in &v)`, `printf("%d", x)` prints the element.
- **Checked when compiled.** The format is a literal, so a program
  whose `printf` would misprint is rejected; nothing about a format
  can fail when the program runs.
- **`%v`** is not C's: it is the conversion for "the value, as
  CobaltC writes it", so `printf("%v\n", x)` needs no thought about
  `x`'s type. A float written with `%v` reads back as the same value
  (`String::parse`, §2d); `%g` does not promise that.
- **Standard error.** `eprintf` is for messages that are not the
  program's output: errors, warnings, progress. Both streams are written
  at once, with nothing held back, so their text interleaves in the
  order the calls ran. `std` does not export `stderr_write`, the extern
  `eprintf` writes through; its failures are discarded as `stdout_write`'s are.
- **Names.** `printf`, `sprintf`, `eprintf` and `String::appendf` are items of
  `std`; a program's own of the same name takes precedence (D-0024),
  and the `std::` name always means this one.

**Depends on:** rule.stdlib.print, rule.stdlib.text, rule.stdlib.str,
rule.stdlib.string, rule.type.kind, rule.trust.extern-call, D-0038,
D-0039, D-0040
**Affects:** standard output; standard error (`eprintf`); `*s` (`String::appendf`); state.storage (`sprintf`'s `String`)

## 2g. Hash tables

In `std::collections`.

### `rule.stdlib.hashmap`
**Status:** ACCEPTED

`HashMap<K, V>` maps keys to values and `HashSet<K>` holds keys; both
are exported types of `std`, written in CobaltC below (D-0041). A **key
type** is an integer type, `bool`, `str` or `String`, or a struct whose
fields, or an enum whose payloads, are all key types (D-0110). Hashing and
equality depend on `K`, which a CobaltC body cannot inspect (D-0010),
so they are two std-only intrinsics realized natively, as
`append_native` is (§2d):

    [Key-Bytes]
        bytes(v : τ integer)   = v in two's complement, width(τ)/8 bytes, least significant first
        bytes(v : bool)        = one byte, 1 or 0
        bytes(v : str)         = its bytes;   bytes(v : String) = the bytes of v.bytes
        bytes(v : struct S)    = part(f1) … part(fn), its fields in declaration order   (D-0110)
        bytes(v : enum E)      = the variant's index as a u32 (as above), then part(payload) if any
            part(x) = bytes(x), except for text: its length as a u64, then its bytes

    [Key-Hash]    ⟨key_hash(r), Σ⟩ → ⟨h, Σ⟩     r : ref<K, shared> to v;  h : u64 = FNV-1a-64(bytes(v))
    [Key-Eq]      ⟨key_eq(r1, r2), Σ⟩ → ⟨b, Σ⟩   b = (bytes(v1) = bytes(v2))

        FNV-1a-64(b0..b_{n−1}):  h := 14695981039346656037;  for each bj: h := (h ⊕ bj) × 1099511628211 mod 2^64

`key_hash`'s value is not observable: it chooses only where an entry
sits in `slots`, and nothing a program can read depends on that.

**Bytes and checksums (D-0119).** For an integer type `T` of `n = sizeof<T>()`
bytes and a value `x`, let `u` be `x`'s two's-complement image as an
unsigned `8n`-bit number (`x` itself for an unsigned `T`; `x + 2^(8n)`
for a negative `x`), and `byte_k(u) = (u >> 8k) & 0xFF` for `k` in
`0 .. n`.

    [Bytes-LE]   Vec::push_le(&mut out, x)  pushes  byte_0(u), byte_1(u), …, byte_{n−1}(u)
                 read_le<T>(s, at)  =  the T whose image u has byte_k(u) = s[at + k]           -- at + n ≤ len(s), else diag.index-out-of-bounds
    [Bytes-BE]   Vec::push_be(&mut out, x)  pushes  byte_{n−1}(u), …, byte_0(u)
                 read_be<T>(s, at)  =  the T whose image u has byte_k(u) = s[at + n − 1 − k]
        -- so read_le<T>(&v[0..$], i) after Vec::push_le(&mut v, x) at position i gives x back, for every T and x; likewise be

    [CRC32]      crc32(data) : u32,  CRC-32/IEEE (IEEE 802.3; the checksum of zlib, gzip, PNG and Ethernet):
                     polynomial 0x04C11DB7, used reflected as 0xEDB88320; initial value 0xFFFFFFFF;
                     input reflected (bits taken least significant first); output reflected; final xor 0xFFFFFFFF.
                 Bit by bit, which is the definition and what std's body does:
                     crc := 0xFFFFFFFF
                     for each byte b of data, in order:
                         crc := crc xor b                       -- b as a u32, in the low eight bits
                         repeat 8 times:
                             if crc & 1 = 1 then crc := (crc >> 1) xor 0xEDB88320
                                            else crc := crc >> 1
                     result := crc xor 0xFFFFFFFF
                 Check values: crc32 of no bytes = 0x00000000; of the ASCII bytes "123456789" = 0xCBF43926.
                 An implementation may compute it by table or by hardware, but must give exactly this function.

    [Rng]        (D-0120)  state: four u64 words s0, s1, s2, s3.  All arithmetic below is on u64, wrapping (mod 2^64);
                 rotl(x, k) = (x << k) | (x >> (64 − k)).
                 Rng::new(seed): splitmix64 from `seed` gives s0, s1, s2, s3 in that order, where each step is
                     state := state + 0x9E3779B97F4A7C15
                     z := state;  z := (z xor (z >> 30)) × 0xBF58476D1CE4E5B9;  z := (z xor (z >> 27)) × 0x94D049BB133111EB
                     output z xor (z >> 31)
                 Rng::next_u64(r): xoshiro256**:
                     result := rotl(s1 × 5, 7) × 9
                     t := s1 << 17;  s2 := s2 xor s0;  s3 := s3 xor s1;  s1 := s1 xor s2;  s0 := s0 xor s3;  s2 := s2 xor t;  s3 := rotl(s3, 45)
                     output result
                 Rng::below(r, n): reject := (2^64 − n) mod n  (n = 0: diag.div-by-zero);  x := next_u64(r);
                     while x < reject: x := next_u64(r);   output x mod n        -- every value in 0 .. n equally likely
                 Rng::unit_f64(r): (next_u64(r) >> 11) as an f64, divided by 2^53          -- 0.0 ≤ x < 1.0, exact
                 Check values: seed 42 → next_u64: 1546998764402558742, 6990951692964543102, 12544586762248559009;
                     seed 0 → next_u64: 11091344671253066420;
                     seed 42 → below(10), below(10), below(6), below(1000000007), below(1): 2, 2, 5, 779106270, 0;
                     seed 42 → unit_f64: 0.083862971059882163 (printed `%.17g`).
                 The sequence is normative: an implementation computes exactly it. It is a general-purpose generator and
                 not suitable for cryptographic purposes: an observer of a few outputs can recover its state.

    [Lookup-Str]   (D-0113)
        m : HashMap<String, V> (or s : HashSet<String>),  k : str,  k' : String with bytes(k') = bytes(k)
        ────────────────────────────────────────────
        HashMap::get_str(&m, k)      ≡  HashMap::get(&m, &k')            and likewise get_mut_str, contains_str,
        HashMap::remove_str(&mut m, k) ≡  HashMap::remove(&mut m, &k')     HashSet::contains_str, HashSet::remove_str
        -- the same slot: [Key-Bytes] gives a str and a String of one text the same bytes, so [Key-Hash] the same
        -- hash; the entry is found by comparing bytes (String::eq_str); no String is constructed


    [Set-Ops]   (D-0138)   a, b : HashSet<K>, K a key type that is clone;  keys(s) = s's keys in insertion order
        keys(HashSet::union(&a, &b))        = keys(a), then the k ∈ keys(b) with k ∉ a, in keys(b)'s order
        keys(HashSet::intersection(&a, &b)) = the k ∈ keys(a) with k ∈ b, in keys(a)'s order
        keys(HashSet::difference(&a, &b))   = the k ∈ keys(a) with k ∉ b, in keys(a)'s order
        -- each key of the result a copy, clone(&k); a and b are read through shared references and unchanged


    [Key-Not-Hashable]   disposition: rejected
        a call of a HashMap or HashSet function whose K is not a key type
        (a type parameter: at each instantiation, rule.type.kind)
        ────────────────────────────────────────────
        ill-formed; diag.type-mismatch (static)

    // Puts x in slot i of v and returns what was there. Private to `std`:
    // `HashMap::insert` replaces a value with it.
    fn Vec::replace<T>(ref<Vec<T>, exclusive> v, usize i, T x) : T
    {
        if (i >= v.len)
        {
            fault(index_out_of_bounds);
        }
        unsafe
        {
            rawptr<T> p = v.ptr + reinterpret<isize>(i);
            auto old = *p;
            release(reinterpret_ptr<u8>(p), sizeof<T>());
            *p = x;
            old
        }
    }

    // Takes element i out of v; the elements after it keep their order
    // (they come off the end and go back on). Private to `std`:
    // `HashMap::remove` and `swap_remove` use it.
    fn Vec::remove_at<T>(ref<Vec<T>, exclusive> v, usize i) : T
    {
        if (i >= v.len)
        {
            fault(index_out_of_bounds);
        }
        Vec<T> later = Vec::new();
        while (v.len > i + 1)
        {
            match (Vec::pop(v))
            {
                Some(x) :
                {
                    Vec::push(&mut later, x);
                },
                None :
                {
                },
            }
        }
        auto taken = match (Vec::pop(v))
        {
            Some(x) : x,
            None : fault(index_out_of_bounds),
        };
        while (Vec::len(&later) > 0)
        {
            match (Vec::pop(&mut later))
            {
                Some(x) :
                {
                    Vec::push(v, x);
                },
                None :
                {
                },
            }
        }
        taken
    }

    // Moves element i out of v, leaving its slot empty. Private to `std`:
    // only a holder that sets `len` to 0 before the Vec is destroyed may use
    // it (`Drain`, `HashDrain`).
    fn Vec::take_raw<T>(ref<Vec<T>, exclusive> v, usize i) : T
    {
        unsafe
        {
            rawptr<T> p = v.ptr + reinterpret<isize>(i);
            auto x = *p;
            release(reinterpret_ptr<u8>(p), sizeof<T>());
            x
        }
    }

    // Empties v: its elements are destroyed in order, first to last, as
    // the Vec's own destructor would; its buffer is kept for reuse (D-0044).
    export fn Vec::clear<T>(ref<Vec<T>, exclusive> v)
    {
        usize i = 0;
        while (i < v.len)
        {
            drop(Vec::take_raw(v, i));
            i = i + 1;
        }
        v.len = 0;
    }

    // A consumed Vec that `foreach` takes elements from in order
    // (`rule.control.foreach`, spec/14, D-0042). The elements not yet taken
    // when the loop ends are destroyed with it.
    resource struct Drain<T>
    {
        Vec<T> v;
        usize next;
    }

    fn Vec::drain<T>(Vec<T> v) : Drain<T>
    {
        Drain { .v = v, .next = 0 }
    }

    fn Drain::total<T>(ref<Drain<T>, shared> d) : usize
    {
        d.v.len
    }

    fn Drain::take<T>(ref<Drain<T>, exclusive> d) : T
    {
        auto x = Vec::take_raw(&mut d.v, d.next);
        d.next = d.next + 1;
        x
    }

    fn Drain::drop<T>(ref<Drain<T>, exclusive> self)
    {
        while (self.next < self.v.len)
        {
            drop(Vec::take_raw(&mut self.v, self.next));
            self.next = self.next + 1;
        }
        self.v.len = 0;                 // every element is gone: the Vec frees only its buffer
    }

    // Hash tables (`rule.stdlib.hashmap`, spec/21 §1a, D-0041). A key type
    // is an integer type, `bool`, `str` or `String`; the std-only intrinsics
    // `key_hash` (FNV-1a over the key's bytes) and `key_eq` realize hashing
    // and equality for each, and the front end checks `K` at every call.
    // Keys and values are kept in insertion order, entry i in keys[i] and
    // values[i], and the table holds their indices, so iteration is in
    // insertion order.
    export struct HashMap<K, V>
    {
        Vec<K> keys;                        // in insertion order
        Vec<V> values;                      // values[i] belongs to keys[i]
        Vec<usize> slots;                   // an entry index, or max_value<usize>() when empty
    }

    export fn HashMap::new<K, V>() : HashMap<K, V>
    {
        Vec<usize> slots = Vec::new();
        usize i = 0;
        while (i < 8)
        {
            Vec::push(&mut slots, max_value<usize>());
            i = i + 1;
        }
        HashMap { .keys = Vec::new(), .values = Vec::new(), .slots = slots }
    }

    export fn HashMap::clear<K, V>(ref<HashMap<K, V>, exclusive> m)
    {
        Vec::clear(&mut m.values);
        Vec::clear(&mut m.keys);
        Vec::clear(&mut m.slots);
        usize i = 0;
        while (i < 8)
        {
            Vec::push(&mut m.slots, max_value<usize>());
            i = i + 1;
        }
    }

    export fn HashMap::len<K, V>(ref<HashMap<K, V>, shared> m) : usize
    {
        Vec::len(&m.keys)
    }

    // Entry i in insertion order, for i < len.
    export fn HashMap::key_at<K, V>(ref<HashMap<K, V>, shared> m, usize i) : ref<K, shared>
    {
        Vec::index_shared(&m.keys, i)
    }

    export fn HashMap::value_at<K, V>(ref<HashMap<K, V>, shared> m, usize i) : ref<V, shared>
    {
        Vec::index_shared(&m.values, i)
    }

    // The slot holding k, or the empty slot where it belongs (linear
    // probing; the table is never full).
    fn HashMap::find<K, V>(ref<HashMap<K, V>, shared> m, ref<K, shared> k) : usize
    {
        usize mask = Vec::len(&m.slots) - 1;
        usize i = narrow_wrapping<usize>(key_hash(k)) & mask;
        bool searching = true;
        while (searching)
        {
            usize e = *Vec::index_shared(&m.slots, i);
            if (e == max_value<usize>())
            {
                searching = false;
            }
            else if (key_eq(Vec::index_shared(&m.keys, e), k))
            {
                searching = false;
            }
            else
            {
                i = (i + 1) & mask;
            }
        }
        i
    }

    // The entry index for k, or max_value<usize>() when k is absent.
    fn HashMap::index_of<K, V>(ref<HashMap<K, V>, shared> m, ref<K, shared> k) : usize
    {
        *Vec::index_shared(&m.slots, HashMap::find(m, k))
    }

    // Places every entry again in a table of n slots (n a power of two, at
    // least the current size).
    fn HashMap::reslot<K, V>(ref<HashMap<K, V>, exclusive> m, usize n)
    {
        usize old = Vec::len(&m.slots);
        usize i = 0;
        while (i < n)
        {
            if (i < old)
            {
                *Vec::index_exclusive(&mut m.slots, i) = max_value<usize>();
            }
            else
            {
                Vec::push(&mut m.slots, max_value<usize>());
            }
            i = i + 1;
        }
        usize e = 0;
        while (e < Vec::len(&m.keys))
        {
            usize s = HashMap::find(&*m, Vec::index_shared(&m.keys, e));
            *Vec::index_exclusive(&mut m.slots, s) = e;
            e = e + 1;
        }
    }

    // Room for one more entry: the table stays at most three-quarters full.
    fn HashMap::reserve_one<K, V>(ref<HashMap<K, V>, exclusive> m)
    {
        usize n = Vec::len(&m.slots);
        if ((Vec::len(&m.keys) + 1) * 4 > n * 3)
        {
            HashMap::reslot(m, n * 2);
        }
    }

    // Adds k with value v. If k was present, its value is replaced where it
    // stands and the old one returned; the stored key stays and k is
    // destroyed.
    export fn HashMap::insert<K, V>(ref<HashMap<K, V>, exclusive> m, K k, V v) : Option<V>
    {
        HashMap::reserve_one(m);
        usize s = HashMap::find(&*m, &k);
        usize e = *Vec::index_shared(&m.slots, s);
        if (e == max_value<usize>())
        {
            *Vec::index_exclusive(&mut m.slots, s) = Vec::len(&m.keys);
            Vec::push(&mut m.keys, k);
            Vec::push(&mut m.values, v);
            return None;
        }
        Some(Vec::replace(&mut m.values, e, v))
    }

    // The value for k. If k is absent, it is added with `fresh` first; if it
    // is present, `k` and `fresh` are not needed and are destroyed here.
    export fn HashMap::entry<K, V>(ref<HashMap<K, V>, exclusive> m, K k, V fresh) : ref<V, exclusive>
    {
        HashMap::reserve_one(m);
        usize s = HashMap::find(&*m, &k);
        usize e = *Vec::index_shared(&m.slots, s);
        if (e == max_value<usize>())
        {
            e = Vec::len(&m.keys);
            *Vec::index_exclusive(&mut m.slots, s) = e;
            Vec::push(&mut m.keys, k);
            Vec::push(&mut m.values, fresh);
        }
        Vec::index_exclusive(&mut m.values, e)
    }

    export fn HashMap::get<K, V>(ref<HashMap<K, V>, shared> m, ref<K, shared> k) : Option<ref<V, shared>>
    {
        usize e = HashMap::index_of(m, k);
        if (e == max_value<usize>())
        {
            return None;
        }
        Some(Vec::index_shared(&m.values, e))
    }

    export fn HashMap::get_mut<K, V>(ref<HashMap<K, V>, exclusive> m, ref<K, shared> k) : Option<ref<V, exclusive>>
    {
        usize e = HashMap::index_of(&*m, k);
        if (e == max_value<usize>())
        {
            return None;
        }
        Some(Vec::index_exclusive(&mut m.values, e))
    }

    export fn HashMap::contains<K, V>(ref<HashMap<K, V>, shared> m, ref<K, shared> k) : bool
    {
        HashMap::index_of(m, k) != max_value<usize>()
    }

    // Empties slot s, moving later slots of its probe run back so that
    // every entry stays reachable from its home slot (backward-shift
    // deletion: the table needs no tombstones).
    fn HashMap::unslot<K, V>(ref<HashMap<K, V>, exclusive> m, usize s)
    {
        usize mask = Vec::len(&m.slots) - 1;
        usize i = s;
        usize j = s;
        bool shifting = true;
        while (shifting)
        {
            j = (j + 1) & mask;
            usize e = *Vec::index_shared(&m.slots, j);
            if (e == max_value<usize>())
            {
                shifting = false;
            }
            else
            {
                usize home = narrow_wrapping<usize>(key_hash(Vec::index_shared(&m.keys, e))) & mask;
                // The entry in j may move back to i unless its home lies in (i, j], cyclically.
                bool stays = if (i <= j)
                {
                    home > i && home <= j
                }
                else
                {
                    home > i || home <= j
                };
                if (!stays)
                {
                    *Vec::index_exclusive(&mut m.slots, i) = e;
                    i = j;
                }
            }
        }
        *Vec::index_exclusive(&mut m.slots, i) = max_value<usize>();
    }

    // Removes k and returns its value, in constant time: the last entry
    // moves into k's place, so insertion order holds only up to the first
    // such removal. `remove` keeps the order.
    export fn HashMap::swap_remove<K, V>(ref<HashMap<K, V>, exclusive> m, ref<K, shared> k) : Option<V>
    {
        usize s = HashMap::find(&*m, k);
        usize e = *Vec::index_shared(&m.slots, s);
        if (e == max_value<usize>())
        {
            return None;
        }
        usize last = Vec::len(&m.keys) - 1;
        if (e != last)
        {
            // The last entry's slot names e from now on.
            usize t = HashMap::find(&*m, Vec::index_shared(&m.keys, last));
            *Vec::index_exclusive(&mut m.slots, t) = e;
        }
        auto k_last = Vec::remove_at(&mut m.keys, last);
        auto v_last = Vec::remove_at(&mut m.values, last);
        auto v = if (e != last)
        {
            drop(Vec::replace(&mut m.keys, e, k_last));
            Vec::replace(&mut m.values, e, v_last)
        }
        else
        {
            drop(k_last);
            v_last
        };
        HashMap::unslot(m, s);
        Some(v)
    }

    // Removes k and returns its value; the other entries keep their order.
    // It takes time in proportion to the size of the map (`swap_remove`
    // takes constant time, and moves the last entry into k's place).
    export fn HashMap::remove<K, V>(ref<HashMap<K, V>, exclusive> m, ref<K, shared> k) : Option<V>
    {
        usize s = HashMap::find(&*m, k);
        HashMap::remove_slot(m, s)
    }

    // The entry slot s names taken out, the others keeping their order;
    // None when s is empty. `remove` and `remove_str` find s.
    fn HashMap::remove_slot<K, V>(ref<HashMap<K, V>, exclusive> m, usize s) : Option<V>
    {
        usize e = *Vec::index_shared(&m.slots, s);
        if (e == max_value<usize>())
        {
            return None;
        }
        HashMap::unslot(m, s);
        drop(Vec::remove_at(&mut m.keys, e));
        auto v = Vec::remove_at(&mut m.values, e);
        // Entries after e moved down by one.
        usize i = 0;
        while (i < Vec::len(&m.slots))
        {
            usize x = *Vec::index_shared(&m.slots, i);
            if (x != max_value<usize>() && x > e)
            {
                *Vec::index_exclusive(&mut m.slots, i) = x - 1;
            }
            i = i + 1;
        }
        Some(v)
    }

    // A consumed HashMap that `foreach (k, v in m)` takes entries from, key
    // then value, in order; the entries not yet taken are destroyed with it.
    resource struct HashDrain<K, V>
    {
        HashMap<K, V> m;
        usize next;
    }

    fn HashMap::drain<K, V>(HashMap<K, V> m) : HashDrain<K, V>
    {
        HashDrain { .m = m, .next = 0 }
    }

    fn HashDrain::total<K, V>(ref<HashDrain<K, V>, shared> d) : usize
    {
        d.m.keys.len
    }

    fn HashDrain::take_key<K, V>(ref<HashDrain<K, V>, exclusive> d) : K
    {
        Vec::take_raw(&mut d.m.keys, d.next)
    }

    fn HashDrain::take_value<K, V>(ref<HashDrain<K, V>, exclusive> d) : V
    {
        auto v = Vec::take_raw(&mut d.m.values, d.next);
        d.next = d.next + 1;
        v
    }

    fn HashDrain::drop<K, V>(ref<HashDrain<K, V>, exclusive> self)
    {
        while (self.next < self.m.keys.len)
        {
            drop(Vec::take_raw(&mut self.m.keys, self.next));
            drop(Vec::take_raw(&mut self.m.values, self.next));
            self.next = self.next + 1;
        }
        self.m.keys.len = 0;
        self.m.values.len = 0;
    }

    // A set of keys: a HashMap whose values are not used.
    export struct HashSet<K>
    {
        HashMap<K, bool> map;
    }

    export fn HashSet::new<K>() : HashSet<K>
    {
        HashSet { .map = HashMap::new() }
    }

    export fn HashSet::len<K>(ref<HashSet<K>, shared> s) : usize
    {
        HashMap::len(&s.map)
    }

    // Adds k; true if it was not already present.
    export fn HashSet::insert<K>(ref<HashSet<K>, exclusive> s, K k) : bool
    {
        match (HashMap::insert(&mut s.map, k, true))
        {
            Some(_) : false,
            None : true,
        }
    }

    export fn HashSet::contains<K>(ref<HashSet<K>, shared> s, ref<K, shared> k) : bool
    {
        HashMap::contains(&s.map, k)
    }

    // Removes k; true if it was present. As `HashMap::swap_remove`, the
    // last key takes k's place.
    export fn HashSet::swap_remove<K>(ref<HashSet<K>, exclusive> s, ref<K, shared> k) : bool
    {
        match (HashMap::swap_remove(&mut s.map, k))
        {
            Some(_) : true,
            None : false,
        }
    }

    // Removes k keeping the order of the others; true if it was present.
    export fn HashSet::remove<K>(ref<HashSet<K>, exclusive> s, ref<K, shared> k) : bool
    {
        match (HashMap::remove(&mut s.map, k))
        {
            Some(_) : true,
            None : false,
        }
    }

    // A consumed HashSet that `foreach (k in s)` takes keys from.
    struct SetDrain<K>
    {
        HashDrain<K, bool> d;
    }

    fn HashSet::drain<K>(HashSet<K> s) : SetDrain<K>
    {
        HashSet { map } = s;
        SetDrain { .d = HashMap::drain(map) }
    }

    fn SetDrain::total<K>(ref<SetDrain<K>, shared> s) : usize
    {
        HashDrain::total(&s.d)
    }

    fn SetDrain::take<K>(ref<SetDrain<K>, exclusive> s) : K
    {
        auto k = HashDrain::take_key(&mut s.d);
        HashDrain::take_value(&mut s.d);
        k
    }

    // Key i in insertion order, for i < len.
    export fn HashSet::key_at<K>(ref<HashSet<K>, shared> s, usize i) : ref<K, shared>
    {
        HashMap::key_at(&s.map, i)
    }

    // D-0134: every key removed, as `HashMap::clear`.
    export fn HashSet::clear<K>(ref<HashSet<K>, exclusive> s)
    {
        HashMap::clear(&mut s.map);
    }

    // D-0138: the set operations. Each makes a new set, copying the keys it
    // keeps with `clone`, and leaves `a` and `b` as they are. The keys keep
    // `a`'s order; `union` then adds `b`'s keys that are not in `a`, in `b`'s
    // order.
    export fn HashSet::union<K: clone>(ref<HashSet<K>, shared> a, ref<HashSet<K>, shared> b) : HashSet<K>
    {
        HashSet<K> out = HashSet::clone(a);
        foreach (i in 0..HashSet::len(b))
        {
            ref<K, shared> k = HashSet::key_at(b, i);
            if (!HashSet::contains(a, k))
            {
                HashSet::insert(&mut out, clone(k));
            }
        }
        out
    }

    // The keys of `a` that are also in `b`.
    export fn HashSet::intersection<K: clone>(ref<HashSet<K>, shared> a, ref<HashSet<K>, shared> b) : HashSet<K>
    {
        HashSet<K> out = HashSet::new();
        foreach (i in 0..HashSet::len(a))
        {
            ref<K, shared> k = HashSet::key_at(a, i);
            if (HashSet::contains(b, k))
            {
                HashSet::insert(&mut out, clone(k));
            }
        }
        out
    }

    // The keys of `a` that are not in `b`.
    export fn HashSet::difference<K: clone>(ref<HashSet<K>, shared> a, ref<HashSet<K>, shared> b) : HashSet<K>
    {
        HashSet<K> out = HashSet::new();
        foreach (i in 0..HashSet::len(a))
        {
            ref<K, shared> k = HashSet::key_at(a, i);
            if (!HashSet::contains(b, k))
            {
                HashSet::insert(&mut out, clone(k));
            }
        }
        out
    }

- **Order.** Entry `i` (`key_at`, `value_at`, `HashSet::key_at`, for
  `i < len`) is in insertion order. Replacing a key's value keeps its
  place. `remove` (and `remove_str`) keeps every other entry's place and
  takes time in proportion to the map's size; `swap_remove` takes
  constant time by moving the last entry into the removed one's place
  (D-0134). `HashSet::clear` empties a set as `HashMap::clear` empties a
  map. The set operations (`[Set-Ops]`, D-0138) keep the first set's
  order, `union` adding the second's new keys after it. The order depends
  only on the calls, never on hash values, so it is the same in every
  implementation.
- **Ownership.** The map owns its keys and values and destroys them
  with itself. `insert` of a present key returns the old value and
  destroys the key passed in; `entry` of a present key destroys both
  its key and `fresh`.
- **Lookups borrow the key** (`ref<K, shared>`), so a lookup copies
  nothing. In a `HashMap<String, V>`, a literal key is looked up with
  the `_str` functions (`[Lookup-Str]`), which make no `String`.
- **Not keys:** `f32`/`f64` (a NaN is not equal to itself; `0.0` and
  `-0.0` are equal but differ in bytes), references, and composite
  types; a revisit condition of D-0041.
- **Names.** `HashMap`, `HashSet` and their functions are items of
  `std`; `Vec::clear` (D-0044) is too; the fields, `Vec::replace`,
  `Vec::remove_at`, `Vec::take_raw`,
  the holders `Drain`, `HashDrain` and `SetDrain` that a consuming
  `foreach` uses (`rule.control.foreach`, spec/14), and the other helpers
  without `export` are `std`'s own.

**Depends on:** rule.stdlib.vec, rule.stdlib.string, rule.stdlib.str,
rule.type.kind, D-0041
**Affects:** state.storage (through `Vec`)

## 2h. `StringView`

In `std::text`.

### `rule.stdlib.stringview`
**Status:** ACCEPTED

A read-only view of part of a `String`'s text, without copying it
(D-0053). `std` defines it as a struct holding a `slice<u8, shared>` of
the `String`'s bytes; the language adds three forms over it.

    [View-Form]
        s a place of type String (through a reference too), lo, hi : usize, `$` = the length of s in bytes
        ────────────────────────────────────────────
        &s[lo .. hi] : StringView  ≡  String::view(&s, lo, hi)        -- s borrowed shared, as `&s` borrows it
        &v[lo .. hi] : StringView  ≡  StringView::sub(v, lo, hi)      -- v a StringView (any value); `$` its length

    [View-Form-Temporary-Argument]                                    -- D-0135
        &s[lo .. hi] an argument of a call whose result type is not a reference, s of type
        String a temporary (a call's result) or part of one (spec/16 [Temp-Root])
        ────────────────────────────────────────────
        the view is formed as for a place; the String is a temporary of the current statement
        and ends at its exit, after the call (spec/09 [Ref-Form-Temporary-Part-Argument])

    [View-Boundary]   disposition: checked
        lo or hi is not the start of a character (a byte 0b10xxxxxx), the end, or 0
        ────────────────────────────────────────────
        ⟨&s[lo .. hi], Σ⟩ ↛ diag.not-char-boundary
        (lo > hi or hi beyond the length: diag.index-out-of-bounds, as for a slice)

    [View-Eq]
        a : StringView, b : StringView or str (either side)
        ────────────────────────────────────────────
        a == b  ≡  StringView::eq(a, b) / StringView::eq_str(a, b);   a != b  ≡  !(a == b)

    [View-Mode-Rejected]   disposition: rejected
        &mut s[lo .. hi] with s a String or a StringView
        ────────────────────────────────────────────
        ill-formed; diag.type-mismatch (static)

A `String` a call returns may be viewed as an argument, as D-0103 lets
it be sliced (`[View-Form-Temporary-Argument]`, D-0135):
`read_file(&sprintf("%s/%s", dir, name)[0..$])` needs no binding.
Anywhere else a call's result is not a place and `&f()[lo .. hi]` is
`diag.borrow-of-non-place`; a view a call returns from such an argument
(`StringView::trim(&f()[0..$])`) and keeps past the statement is
`diag.destroy-while-aliased` when the `String` ends.

A view borrows its `String` as a `slice<u8, shared>` of it does: while
it lives the `String` cannot be changed (`diag.aliasing-conflict`), and
it cannot outlive it (`diag.reference-escapes-scope`, or
`diag.stale-binding` where the static analysis does not follow it). A
`StringView` counts as a borrowed type for `rule.temporal.elision`, so a
function returning one takes exactly one borrowed parameter — except
`StringView::of`. It is
printable (`%s`, `%v`, `String::append`), and no other operator applies
to it. Positions are byte positions; white space for `trim` is ASCII.

`StringView::of(t)` (D-0134) is a view of a `str`'s bytes. A `str` is a
value that never ends (`type.str`), so the object the view borrows never
ends either: such a view is valid for the rest of the program, and
borrowing it forbids nothing, since no program can change a `str`.
`StringView::of` is the one function returning a `StringView` with no
borrowed parameter. A literal where a `StringView` is expected is this
call (`[Str-Literal-View]`, §2a).

The text `find`, `starts_with` and `ends_with` look for is a
`StringView` (D-0134): a literal, a `String`'s `&s[0..$]`, or another
view. `split`'s separator is a `str`, because `split` returns views of
its first argument and so may take no other borrowed parameter.

`StringView::contains(v, t)` (D-0139, `[Contains]`) is whether
`StringView::find(v, t)` is `Some`; the empty text is in every text.
`StringView::replace(v, from, to)` (`[Replace]`) is a new `String`: `v`
with every occurrence of `from` replaced by `to`, the occurrences found
left to right and not overlapping (`"aaa"` with `"aa"` replaced by `"b"`
is `"ba"`); an empty `from` occurs nowhere, so the result is a copy of
`v`. Each occurrence is whole characters of `v`, since `from` is UTF-8,
so the result is UTF-8. `StringView::join(parts, sep)` (`[Join]`) is the
texts of the views in `parts`, in order, with `sep` between each two, in
a new `String`; no parts give an empty `String`; `String::join` is the
same for a slice of `String`s. `replace` and `join` return a `String`
that borrows nothing, so they may take several borrowed parameters.
`String::contains(&s, t)` and `String::replace(&s, from, to)` are the
`StringView::` functions of `&s[0..$]`.

`StringView::chars(v)` (D-0128) is the text's characters, in order, one
view per character: a character starts at byte 0 and at every byte that
is not a UTF-8 continuation byte (`0b10xxxxxx`), and runs to the next
start or the end, so each view is one to four bytes and together they
cover the text exactly once; empty text gives an empty `Vec`.
`String::chars(&s)` is `StringView::chars(&s[0..$])`. A character is a
Unicode scalar value (`"e\u{301}"` is two). The `Vec` is the caller's;
the views borrow what the argument borrows, as any view does. Building
the `Vec` is one allocation: `foreach` visits only the collections and
ranges the language defines, and a library type has no way to supply
the next element one step at a time (there is no iterator protocol), so
an allocation-free `foreach` over characters is not possible (D-0128
gives the reasons in full); a program that must not allocate walks
`String::as_bytes(&s)` instead.

    // D-0053: a borrowed, read-only view of part of a `String`'s text, made
    // by `&s[lo .. hi]` (`String::view`) and re-sliced by `&v[lo .. hi]`
    // (`StringView::sub`). Its bounds fall on character boundaries, so its
    // bytes are UTF-8. The `String` stays borrowed while a view of it lives.
    export struct StringView
    {
        slice<u8, shared> bytes;
    }

    // Whether byte position `i` of `b` starts a character (or is its end).
    fn StringView::boundary(slice<u8, shared> b, usize i) : bool
    {
        i == 0 || i == slice_len(b) || (b[i] & 0xC0: u8) != 0x80: u8
    }

    // D-0134: a view of a `str`'s text. A `str` is a value that never ends,
    // so the view is valid for the rest of the program; a literal where a
    // `StringView` is expected is this call (`[Str-Literal-View]`).
    export fn StringView::of(str s) : StringView
    {
        StringView { .bytes = str_slice(s) }
    }

    // The address of a view's first byte, for the file primitives; any
    // address when the view is empty (they then read none).
    fn StringView::ptr(StringView v) : rawptr<u8>
    {
        if (slice_len(v.bytes) == 0)
        {
            return dangling<u8>();
        }
        rawptr_of(&v.bytes[0])
    }

    export fn String::view(ref<String, shared> s, usize lo, usize hi) : StringView
    {
        slice<u8, shared> all = &s.bytes[0..$];
        if (lo > hi || hi > slice_len(all))
        {
            fault(index_out_of_bounds);
        }
        if (!StringView::boundary(all, lo) || !StringView::boundary(all, hi))
        {
            fault(not_char_boundary);
        }
        StringView { .bytes = &all[lo..hi] }
    }

    export fn StringView::sub(StringView v, usize lo, usize hi) : StringView
    {
        if (lo > hi || hi > slice_len(v.bytes))
        {
            fault(index_out_of_bounds);
        }
        if (!StringView::boundary(v.bytes, lo) || !StringView::boundary(v.bytes, hi))
        {
            fault(not_char_boundary);
        }
        StringView { .bytes = &v.bytes[lo..hi] }
    }

    export fn StringView::len(StringView v) : usize
    {
        slice_len(v.bytes)
    }

    export fn StringView::eq(StringView a, StringView b) : bool
    {
        usize n = slice_len(a.bytes);
        if (n != slice_len(b.bytes))
        {
            return false;
        }
        for (usize i = 0; i < n; i += 1)
        {
            if (a.bytes[i] != b.bytes[i])
            {
                return false;
            }
        }
        true
    }

    export fn StringView::eq_str(StringView a, str t) : bool
    {
        usize n = slice_len(a.bytes);
        if (n != str_len(t))
        {
            return false;
        }
        for (usize i = 0; i < n; i += 1)
        {
            if (a.bytes[i] != str_byte(t, i))
            {
                return false;
            }
        }
        true
    }

    // Whether `t` occurs in `v` at byte position `at`.
    fn StringView::at(StringView v, usize at, StringView t) : bool
    {
        usize m = slice_len(t.bytes);
        if (at + m > slice_len(v.bytes))
        {
            return false;
        }
        for (usize j = 0; j < m; j += 1)
        {
            if (v.bytes[at + j] != t.bytes[j])
            {
                return false;
            }
        }
        true
    }

    // The byte position of the first occurrence of `t` in `v`.
    export fn StringView::find(StringView v, StringView t) : Option<usize>
    {
        usize n = slice_len(v.bytes);
        usize m = slice_len(t.bytes);
        if (m > n)
        {
            return None;
        }
        for (usize i = 0; i <= n - m; i += 1)
        {
            if (StringView::at(v, i, t))
            {
                return Some(i);
            }
        }
        None
    }

    export fn StringView::starts_with(StringView v, StringView t) : bool
    {
        StringView::at(v, 0, t)
    }

    export fn StringView::ends_with(StringView v, StringView t) : bool
    {
        usize n = slice_len(v.bytes);
        usize m = slice_len(t.bytes);
        m <= n && StringView::at(v, n - m, t)
    }

    // Whether the text of `t` occurs in `v`.
    export fn StringView::contains(StringView v, StringView t) : bool
    {
        StringView::find(v, t) != None
    }

    // ASCII white space: space, tab, line feed, vertical tab, form feed, return.
    fn StringView::space(u8 b) : bool
    {
        b == b' ' || (b >= 9: u8 && b <= 13: u8)
    }

    export fn StringView::trim_start(StringView v) : StringView
    {
        usize n = slice_len(v.bytes);
        usize i = 0;
        while (i < n && StringView::space(v.bytes[i]))
        {
            i += 1;
        }
        StringView::sub(v, i, n)
    }

    export fn StringView::trim_end(StringView v) : StringView
    {
        usize n = slice_len(v.bytes);
        while (n > 0 && StringView::space(v.bytes[n - 1]))
        {
            n -= 1;
        }
        StringView::sub(v, 0, n)
    }

    export fn StringView::trim(StringView v) : StringView
    {
        StringView::trim_end(StringView::trim_start(v))
    }

    // The parts of `v` between occurrences of `sep`, each a view of the same
    // `String`; an empty `sep` gives `v` whole.
    export fn StringView::split(StringView v, str sep) : Vec<StringView>
    {
        Vec<StringView> out = Vec::new();
        usize n = slice_len(v.bytes);
        usize m = str_len(sep);
        if (m == 0)
        {
            Vec::push(&mut out, v);
            return out;
        }
        StringView sv = StringView::of(sep);
        usize start = 0;
        usize i = 0;
        while (i + m <= n)
        {
            if (StringView::at(v, i, sv))
            {
                Vec::push(&mut out, StringView::sub(v, start, i));
                i += m;
                start = i;
            }
            else
            {
                i += 1;
            }
        }
        Vec::push(&mut out, StringView::sub(v, start, n));
        out
    }

    // D-0128: the characters of `v`, in order, each a view of that one
    // character's bytes (one to four): a character starts at every byte that
    // is not a UTF-8 continuation byte (0b10xxxxxx). Empty text gives no
    // views. The `Vec` is new; the views borrow what `v` borrows.
    export fn StringView::chars(StringView v) : Vec<StringView>
    {
        Vec<StringView> out = Vec::new();
        usize n = slice_len(v.bytes);
        usize start = 0;
        foreach (i in 1..n + 1)
        {
            if (StringView::boundary(v.bytes, i))
            {
                Vec::push(&mut out, StringView { .bytes = &v.bytes[start..i] });
                start = i;
            }
        }
        out
    }

    // A new `String`: `v` with every occurrence of `from` replaced by `to`.
    // Occurrences are found left to right and do not overlap (`"aaa"` with
    // `"aa"` replaced is one replacement and an `a`); an empty `from`
    // occurs nowhere, so the result is a copy of `v`.
    export fn StringView::replace(StringView v, StringView from, StringView to) : String
    {
        usize n = slice_len(v.bytes);
        usize m = slice_len(from.bytes);
        if (m == 0)
        {
            return String::from_view(v);
        }
        String out = String::new();
        usize start = 0;
        usize i = 0;
        while (i + m <= n)
        {
            if (StringView::at(v, i, from))
            {
                String::append_view(&mut out, StringView { .bytes = &v.bytes[start..i] });
                String::append_view(&mut out, to);
                i += m;
                start = i;
            }
            else
            {
                i += 1;
            }
        }
        String::append_view(&mut out, StringView { .bytes = &v.bytes[start..n] });
        out
    }

    // The texts of `parts`, in order, with `sep` between each two, in a new
    // `String`; no parts give an empty `String`.
    export fn StringView::join(slice<StringView, shared> parts, StringView sep) : String
    {
        String out = String::new();
        foreach (i, p in parts)
        {
            if (i > 0)
            {
                String::append_view(&mut out, sep);
            }
            String::append_view(&mut out, *p);
        }
        out
    }

    // `StringView::join` for `String`s: the texts of `parts` joined by `sep`.
    export fn String::join(slice<String, shared> parts, StringView sep) : String
    {
        String out = String::new();
        foreach (i, p in parts)
        {
            if (i > 0)
            {
                String::append_view(&mut out, sep);
            }
            String::append_string(&mut out, p);
        }
        out
    }

    // A new `String` holding a copy of the view's text.
    export fn String::from_view(StringView v) : String
    {
        Vec<u8> b = Vec::new();
        foreach (x in v.bytes)
        {
            Vec::push(&mut b, *x);
        }
        String { .bytes = b }
    }

    export fn StringView::parse<T>(StringView v) : Result<T, ParseError>
    {
        String s = String::from_view(v);
        String::parse<T>(&s)
    }

    fn String::append_view(ref<String, exclusive> s, StringView v)
    {
        foreach (x in v.bytes)
        {
            Vec::push(&mut s.bytes, *x);
        }
    }

**Depends on:** rule.stdlib.string, rule.agg.slice, rule.temporal.elision, D-0047, D-0053
**Affects:** the `String` a view borrows

## 2i. Queues

In `std::collections` (D-0137).

### `rule.stdlib.queue`
**Status:** ACCEPTED

`Queue<T>` holds a sequence of elements and adds and takes them at
either end in constant time, amortized as a `Vec`'s `push` is. It is
written in CobaltC below as two `Vec`s used as stacks, so each step is
a `Vec`'s `push`, `pop` or index: `head` holds the front part, front
element last, and `tail` the back part, back element last.

    [Queue]   a queue's value is a sequence ⟨x1, …, xn⟩, x1 its front and xn its back
        Queue::new()            = ⟨⟩
        Queue::len(&q)          = n
        Queue::push_back(&mut q, x)    ⟨x1, …, xn⟩ → ⟨x1, …, xn, x⟩
        Queue::push_front(&mut q, x)   ⟨x1, …, xn⟩ → ⟨x, x1, …, xn⟩
        Queue::pop_front(&mut q)       ⟨x1, …, xn⟩ → ⟨x2, …, xn⟩, giving Some(x1);   ⟨⟩ → ⟨⟩, giving None
        Queue::pop_back(&mut q)        ⟨x1, …, xn⟩ → ⟨x1, …, xn−1⟩, giving Some(xn); ⟨⟩ → ⟨⟩, giving None
        Queue::front(&q) = Some(&x1),  Queue::back(&q) = Some(&xn);  None when n = 0
        Queue::clear(&mut q), and the queue's end: x1, …, xn destroyed in that order; clear leaves ⟨⟩

    // Queues (`rule.stdlib.queue`, spec/21 §2i, D-0137): two `Vec`s used
    // as stacks, so every step is a `Vec`'s `push`, `pop` or index. `head`
    // holds the front part with the front element last; `tail` holds the back
    // part in order, the back element last. Taking from an empty side first
    // moves elements across from the other (`Queue::shift`): all of them
    // while the queue is taken from at one end only, so each element moves
    // at most once; half of them once it has been taken from at both, so
    // alternating the ends still moves each element a bounded number of
    // times.
    export resource struct Queue<T>
    {
        Vec<T> head;                        // the front part, front element last
        Vec<T> tail;                        // the back part, back element last
        bool took_front;                    // `pop_front` has taken an element
        bool took_back;                     // `pop_back` has
    }

    export fn Queue::new<T>() : Queue<T>
    {
        Queue { .head = Vec::new(), .tail = Vec::new(), .took_front = false, .took_back = false }
    }

    export fn Queue::len<T>(ref<Queue<T>, shared> q) : usize
    {
        q.head.len + q.tail.len
    }

    // `to` is empty. With `half`, the half of `from`'s elements nearest `to`'s
    // end (its first ones, rounding up) move to `to`, the rest moving down to
    // the start of `from` in order; otherwise all of them move. Either way
    // they arrive in `to`'s order, `to`'s last being `from`'s first. Each
    // element moves by `copy_raw` into a slot that holds nothing, and the
    // slots left are released together.
    fn Queue::shift<T>(ref<Vec<T>, exclusive> from, ref<Vec<T>, exclusive> to, bool half)
    {
        usize n = from.len;
        usize stay = if (half)
        {
            n / 2
        }
        else
        {
            0
        };
        usize m = n - stay;
        usize size = sizeof<T>();
        Vec::reserve(to, m);
        rawptr<T> src = from.ptr;
        rawptr<T> dst = to.ptr;
        unsafe
        {
            for (usize k = 0; k < m; k += 1)
            {
                copy_raw(reinterpret_ptr<u8>(dst + reinterpret<isize>(m - 1 - k)), reinterpret_ptr<u8>(src + reinterpret<isize>(k)), size);
            }
            release(reinterpret_ptr<u8>(src), m * size);
            if (stay > 0)
            {
                // `m >= stay`: the ranges do not overlap.
                copy_raw(reinterpret_ptr<u8>(src), reinterpret_ptr<u8>(src + reinterpret<isize>(m)), stay * size);
                release(reinterpret_ptr<u8>(src + reinterpret<isize>(stay)), m * size);
            }
        }
        to.len = m;
        from.len = stay;
    }

    export fn Queue::push_back<T>(ref<Queue<T>, exclusive> q, T x)
    {
        Vec::push(&mut q.tail, x);
    }

    export fn Queue::push_front<T>(ref<Queue<T>, exclusive> q, T x)
    {
        Vec::push(&mut q.head, x);
    }

    export fn Queue::pop_front<T>(ref<Queue<T>, exclusive> q) : Option<T>
    {
        if (q.head.len == 0)
        {
            if (q.tail.len == 0)
            {
                return None;
            }
            q.took_front = true;
            Queue::shift(&mut q.tail, &mut q.head, q.took_back);
        }
        Vec::pop(&mut q.head)
    }

    export fn Queue::pop_back<T>(ref<Queue<T>, exclusive> q) : Option<T>
    {
        if (q.tail.len == 0)
        {
            if (q.head.len == 0)
            {
                return None;
            }
            q.took_back = true;
            Queue::shift(&mut q.head, &mut q.tail, q.took_front);
        }
        Vec::pop(&mut q.tail)
    }

    export fn Queue::front<T>(ref<Queue<T>, shared> q) : Option<ref<T, shared>>
    {
        if (q.head.len > 0)
        {
            return Some(Vec::index_shared(&q.head, q.head.len - 1));
        }
        if (q.tail.len > 0)
        {
            return Some(Vec::index_shared(&q.tail, 0));
        }
        None
    }

    export fn Queue::back<T>(ref<Queue<T>, shared> q) : Option<ref<T, shared>>
    {
        if (q.tail.len > 0)
        {
            return Some(Vec::index_shared(&q.tail, q.tail.len - 1));
        }
        if (q.head.len > 0)
        {
            return Some(Vec::index_shared(&q.head, 0));
        }
        None
    }

    // Empties q, destroying its elements front to back: `head` last first,
    // then `tail` first to last. The buffers are kept.
    export fn Queue::clear<T>(ref<Queue<T>, exclusive> q)
    {
        Vec::truncate(&mut q.head, 0);
        Vec::clear(&mut q.tail);
    }

    export fn Queue::drop<T>(ref<Queue<T>, exclusive> self)
    {
        Queue::clear(self);
    }

- **Ownership.** The queue owns its elements; one taken by `pop_front`
  or `pop_back` is the caller's. `front` and `back` give a shared
  reference that borrows the queue (`rule.temporal.elision`), so the
  queue cannot change while it is in use: a push may move every element
  to a new buffer.
- **Moving between the parts.** Taking from an empty part first moves
  elements across from the other, copying their bytes (`[Copy-Raw]`)
  and releasing the slots they left (`[Release]`): all of them while the
  queue has been taken from at one end only, so each element moves at
  most once; half of them once it has been taken from at both, so
  taking from the ends in turn still moves each element a bounded
  number of times. The buffers never shrink, and `clear` keeps them.
- **Not provided:** positional access (`q[i]`; `[Index-Vec]` is
  `Vec`'s), `foreach` over a queue (`rule.control.foreach` names its
  collections), `_mut` twins, `clone` and `reserve` — revisit conditions
  of D-0137.

**Depends on:** rule.stdlib.vec, rule.trust.rawptr, rule.resauth.destroy, D-0137
**Affects:** state.storage (through `Vec`)

### `rule.stdlib.priority-queue`
**Status:** ACCEPTED

`PriorityQueue<T>` gives its elements back least first, in an order
fixed when it is made: `PriorityQueue::new()` uses the key types' own
order (`key_less`, as `Vec::sort` does, §1b), `PriorityQueue::new_by(less)`
the caller's `less` (true when its first argument goes before its
second, as `sort_by`'s). Elements the order calls equal leave first in,
first out. It is a binary heap in a `Vec`, written in CobaltC below;
`push` and `pop` take time in proportion to the logarithm of the length.

    [Priority-Queue]   a queue's value is a set of pairs (x, k), k the element's arrival number:
                       0 for the first push after new or clear, then 1, 2, …
        (x, k) goes before (y, l)  ⟺  less(x, y)  ∨  (¬less(y, x) ∧ k < l)
            less = key_less (new) or the function given (new_by)
        PriorityQueue::push(&mut q, x)   adds (x, k), k the next arrival number
        PriorityQueue::pop(&mut q)       removes the pair that goes before every other, giving Some(x); None when empty
        PriorityQueue::peek(&q)          = Some(&x) for that pair, removing nothing; None when empty
        PriorityQueue::len(&q)           = the number of pairs
        PriorityQueue::clear(&mut q), and the queue's end: every element destroyed, in the order of the
            `Vec` that holds them (the heap's, which the body below determines)
        -- the body decides with one comparison, which is this order when less is a strict weak order
        -- a `less` that is not a strict weak order gives the order the body gives; nothing faults

    [Priority-Queue-Not-Key]   disposition: rejected
        PriorityQueue::new<T>() with T not a key type (§2g), at the call or at a generic function's instantiation
        ────────────────────────────────────────────
        ill-formed; diag.type-mismatch   (repair: PriorityQueue::new_by with a comparison)

    // Priority queues (`rule.stdlib.priority-queue`, spec/21 §2i, D-0137):
    // a binary heap in a `Vec`, each element's parent going before it. Each
    // element has an arrival number, kept at the same position of a second
    // `Vec`, so elements the order calls equal leave first in, first out.
    export struct PriorityQueue<T>
    {
        Vec<T> items;
        Vec<u64> seqs;                      // seqs[i] is items[i]'s arrival number
        bool keyed;                         // the key types' order, not `less`
        fn(ref<T, shared>, ref<T, shared>) : bool less;
        u64 next;                           // the next arrival number
    }

    // A queue in the key types' own order (as `Vec::sort`'s): smallest first.
    export fn PriorityQueue::new<T>() : PriorityQueue<T>
    {
        PriorityQueue { .items = Vec::new(), .seqs = Vec::new(), .keyed = true, .less = key_before<T>, .next = 0 }
    }

    // A queue in the order `less` gives (true when its first argument goes
    // before its second): the element no other goes before comes out first.
    export fn PriorityQueue::new_by<T>(fn(ref<T, shared>, ref<T, shared>) : bool less) : PriorityQueue<T>
    {
        PriorityQueue { .items = Vec::new(), .seqs = Vec::new(), .keyed = false, .less = less, .next = 0 }
    }

    export fn PriorityQueue::len<T>(ref<PriorityQueue<T>, shared> q) : usize
    {
        q.items.len
    }

    // Whether element i comes out before element j: the earlier arrival
    // unless the later goes before it, the later only if it goes before the
    // earlier. One comparison decides, the order being a strict weak order
    // (§2i); the key types' is compared in place (`key_less_at`, as
    // `Vec::sort` does).
    fn PriorityQueue::before<T>(ref<PriorityQueue<T>, shared> q, bool keyed, ref<Vec<T>, shared> items, rawptr<u64> sb, usize i, usize j) : bool
    {
        bool earlier = unsafe
        {
            *(sb + reinterpret<isize>(i)) < *(sb + reinterpret<isize>(j))
        };
        if (earlier)
        {
            if (keyed)
            {
                !key_less_at(items, j, i)
            }
            else
            {
                !(q.less)(Vec::index_shared(items, j), Vec::index_shared(items, i))
            }
        }
        else
        {
            if (keyed)
            {
                key_less_at(items, i, j)
            }
            else
            {
                (q.less)(Vec::index_shared(items, i), Vec::index_shared(items, j))
            }
        }
    }

    // The heap moves its elements by copying their bytes into a slot that
    // holds nothing (`copy_raw`) and releasing the slot they left, rather
    // than by `Vec::swap`, so no reference is formed for a move. Each loop
    // holds one shared reference to each `Vec` for its comparisons and
    // reaches the slots through `ib` and `sb`: the buffers do not move while
    // the loop runs.

    // Moves element i up past every ancestor it comes out before. The place
    // is found first, comparing with the element where it stands; then the
    // ancestors on the way move down one each and it takes the place.
    fn PriorityQueue::sift_up<T>(ref<PriorityQueue<T>, exclusive> q, usize i)
    {
        rawptr<u64> sb = q.seqs.ptr;
        usize t = i;
        {
            ref<PriorityQueue<T>, shared> s = &*q;
            ref<Vec<T>, shared> items = &s.items;
            bool keyed = s.keyed;
            while (t > 0)
            {
                usize p = (t - 1) / 2;
                if (!PriorityQueue::before(s, keyed, items, sb, i, p))
                {
                    break;
                }
                t = p;
            }
        }
        if (t == i)
        {
            return;
        }
        T x = Vec::take_raw(&mut q.items, i);
        u64 xs = q.seqs[i];
        usize size = sizeof<T>();
        rawptr<T> ib = q.items.ptr;
        usize j = i;
        while (j != t)
        {
            usize p = (j - 1) / 2;
            unsafe
            {
                copy_raw(reinterpret_ptr<u8>(ib + reinterpret<isize>(j)), reinterpret_ptr<u8>(ib + reinterpret<isize>(p)), size);
                release(reinterpret_ptr<u8>(ib + reinterpret<isize>(p)), size);
                *(sb + reinterpret<isize>(j)) = *(sb + reinterpret<isize>(p));
            }
            j = p;
        }
        unsafe
        {
            *(ib + reinterpret<isize>(t)) = x;
            *(sb + reinterpret<isize>(t)) = xs;
        }
    }

    export fn PriorityQueue::push<T>(ref<PriorityQueue<T>, exclusive> q, T x)
    {
        Vec::push(&mut q.items, x);
        Vec::push(&mut q.seqs, q.next);
        q.next = q.next + 1;
        PriorityQueue::sift_up(q, q.items.len - 1);
    }

    // The first element leaves a hole at the top. The hole moves down to a
    // leaf, the child that comes out first rising into it at each level (one
    // comparison per level); the last element fills the hole and moves up to
    // its place, which is seldom far.
    export fn PriorityQueue::pop<T>(ref<PriorityQueue<T>, exclusive> q) : Option<T>
    {
        usize n = q.items.len;
        if (n == 0)
        {
            return None;
        }
        T top = Vec::take_raw(&mut q.items, 0);
        usize size = sizeof<T>();
        rawptr<T> ib = q.items.ptr;
        rawptr<u64> sb = q.seqs.ptr;
        usize last = n - 1;
        usize hole = 0;
        {
            ref<PriorityQueue<T>, shared> s = &*q;
            ref<Vec<T>, shared> items = &s.items;
            bool keyed = s.keyed;
            while (2 * hole + 1 < last)
            {
                usize c = 2 * hole + 1;
                if (c + 1 < last && PriorityQueue::before(s, keyed, items, sb, c + 1, c))
                {
                    c = c + 1;
                }
                unsafe
                {
                    copy_raw(reinterpret_ptr<u8>(ib + reinterpret<isize>(hole)), reinterpret_ptr<u8>(ib + reinterpret<isize>(c)), size);
                    release(reinterpret_ptr<u8>(ib + reinterpret<isize>(c)), size);
                    *(sb + reinterpret<isize>(hole)) = *(sb + reinterpret<isize>(c));
                }
                hole = c;
            }
        }
        if (hole != last)
        {
            unsafe
            {
                copy_raw(reinterpret_ptr<u8>(ib + reinterpret<isize>(hole)), reinterpret_ptr<u8>(ib + reinterpret<isize>(last)), size);
                release(reinterpret_ptr<u8>(ib + reinterpret<isize>(last)), size);
                *(sb + reinterpret<isize>(hole)) = *(sb + reinterpret<isize>(last));
            }
        }
        q.items.len = last;
        q.seqs.len = last;
        if (hole != last)
        {
            PriorityQueue::sift_up(q, hole);
        }
        Some(top)
    }

    export fn PriorityQueue::peek<T>(ref<PriorityQueue<T>, shared> q) : Option<ref<T, shared>>
    {
        if (q.items.len == 0)
        {
            return None;
        }
        Some(Vec::index_shared(&q.items, 0))
    }

    // Empties q, destroying its elements in the order the heap holds them.
    export fn PriorityQueue::clear<T>(ref<PriorityQueue<T>, exclusive> q)
    {
        Vec::clear(&mut q.items);
        Vec::clear(&mut q.seqs);
        q.next = 0;
    }

- **Order among equals.** First in, first out: an event simulation's
  simultaneous events run in the order they were scheduled. This is
  `Vec::sort`'s stability carried over; it costs one `u64` per element,
  kept in a second `Vec` at the same position. One comparison decides
  which of two elements comes out first: the earlier arrival, unless the
  later goes before it — which, for a strict weak order, is
  `[Priority-Queue]`'s condition.
- **How it moves elements.** `pop` takes the first element and moves
  the hole it leaves down to a leaf, the child that comes out first
  rising at each level, then moves the last element into the hole and
  up to its place (one comparison per level on the way down, and seldom
  any on the way up). Elements move by `copy_raw` into a slot that holds
  nothing and `release` of the slot they left (`[Copy-Raw]`,
  `[Release]`), not by `Vec::swap`, and the key types' order compares
  them in place (`key_less_at`), so neither forms a reference.
- **Which end.** Least first: `new` and `sort` agree on one order. A
  largest-first queue passes a `less` that answers "greater".
- **Ownership.** The queue owns its elements; one `pop` gives is the
  caller's. `peek`'s reference borrows the queue.
- **Not provided:** `peek_mut` (a changed element would need moving;
  `pop`, change and `push` does it), `foreach`, `clone`, `reserve` —
  revisit conditions of D-0137.
- **Names.** `Queue`, `PriorityQueue` and the functions above with
  `export` are items of `std`; their fields, `Queue::shift`,
  `PriorityQueue::before` and `PriorityQueue::sift_up` are `std`'s own.

**Depends on:** rule.stdlib.vec, rule.stdlib.sort, rule.trust.rawptr, D-0137
**Affects:** state.storage (through `Vec`)

## 3. `Rc<T>`

In `std::memory`.

### `rule.stdlib.rc`
**Status:** ACCEPTED

    // D-0125: `count` strong handles keep `value`; `weak` handles keep the
    // box. The value ends when `count` reaches 0, the box when both do.
    struct RcBox<T>
    {
        usize count;
        usize weak;
        Option<T> value;
    }

    export resource struct Rc<T>
    {
        rawptr<RcBox<T>> ptr;
    }

    export fn Rc::new<T>(T v) : Rc<T>
    {
        rawptr<u8> p = match (allocate(sizeof<RcBox<T>>(), alignof<RcBox<T>>()))
        {
            Ok(p) : p,
            Err(_) : fault(alloc_failure),
        };
        rawptr<RcBox<T>> bp = reinterpret_ptr<RcBox<T>>(p);
        unsafe
        {
            *bp = RcBox { .count = 1, .weak = 0, .value = Some(v) };
        }
        Rc { .ptr = bp }
    }

    export fn Rc::clone<T>(ref<Rc<T>, shared> r) : Rc<T>
    {
        unsafe
        {
            // Only the count: a reference into the value (`Rc::get` of
            // another handle) may be live, and is disjoint from it.
            ref<usize, exclusive> c = &mut reclaim<RcBox<T>>(r.ptr).count;
            *c = *c + 1;
        }
        Rc { .ptr = r.ptr }
    }

    export fn Rc::get<T>(ref<Rc<T>, shared> r) : ref<T, shared>
    {
        unsafe
        {
            match (&reclaim<RcBox<T>>(r.ptr).value)
            {
                Some(v) : v,
                None : fault(unwrap_failed, "an Rc whose value has ended"),   // never: a live Rc keeps its value
            }
        }
    }

    export fn Rc::drop<T>(ref<Rc<T>, exclusive> self)
    {
        bool last = false;
        unsafe
        {
            ref<usize, exclusive> c = &mut reclaim<RcBox<T>>(self.ptr).count;
            *c = *c - 1;
            last = *c == 0;
        }
        if (last)
        {
            bool unwatched = false;
            unsafe
            {
                overwrite(&mut reclaim<RcBox<T>>(self.ptr).value, None);   // the value ends now (D-0125)
                unwatched = reclaim<RcBox<T>>(self.ptr).weak == 0;
            }
            if (unwatched)
            {
                unsafe
                {
                    drop(reclaim<RcBox<T>>(self.ptr));
                    deallocate(reinterpret_ptr<u8>(self.ptr), sizeof<RcBox<T>>(), alignof<RcBox<T>>());
                }
            }
        }
    }
    // D-0125: a handle that does not keep the value alive.
    export resource struct Weak<T>
    {
        rawptr<RcBox<T>> ptr;
    }
    export fn Rc::downgrade<T>(ref<Rc<T>, shared> r) : Weak<T>
    {
        unsafe
        {
            ref<usize, exclusive> w = &mut reclaim<RcBox<T>>(r.ptr).weak;
            *w = *w + 1;
        }
        Weak { .ptr = r.ptr }
    }
    export fn Weak::upgrade<T>(ref<Weak<T>, shared> w) : Option<Rc<T>>
    {
        bool alive = false;
        unsafe
        {
            ref<usize, exclusive> c = &mut reclaim<RcBox<T>>(w.ptr).count;
            if (*c > 0)
            {
                *c = *c + 1;
                alive = true;
            }
        }
        if (alive) { Some(Rc { .ptr = w.ptr }) } else { None }
    }
    export fn Weak::clone<T>(ref<Weak<T>, shared> w) : Weak<T>
    {
        unsafe
        {
            ref<usize, exclusive> k = &mut reclaim<RcBox<T>>(w.ptr).weak;
            *k = *k + 1;
        }
        Weak { .ptr = w.ptr }
    }
    export fn Weak::drop<T>(ref<Weak<T>, exclusive> self)
    {
        bool last = false;
        unsafe
        {
            ref<usize, exclusive> k = &mut reclaim<RcBox<T>>(self.ptr).weak;
            *k = *k - 1;
            last = *k == 0 && reclaim<RcBox<T>>(self.ptr).count == 0;
        }
        if (last)
        {
            unsafe
            {
                drop(reclaim<RcBox<T>>(self.ptr));
                deallocate(reinterpret_ptr<u8>(self.ptr), sizeof<RcBox<T>>(), alignof<RcBox<T>>());
            }
        }
    }
    export resource struct Rc<T>
    {
        rawptr<RcBox<T>> ptr;
    }

`Rc<T>` realizes shared destroy authority as a library pattern
(D-0003's allowance). A `Weak<T>` (D-0125) holds the box, not the
value: `count` strong handles keep the value, which ends (`overwrite`
to `None`) when the last of them does; `weak` handles keep the box,
which is deallocated when both counts are 0 — by the last `Rc::drop` if
no weak handle exists, else by the last `Weak::drop`. `Weak::upgrade`
makes a strong handle only while `count > 0`, so a value never comes
back once ended; `Rc::get` matches the `Option` and can never see
`None` through a live `Rc`. A cycle of `Rc`s still leaks; a parent
pointer, a cache or an observer list holds a `Weak` instead. `Rc::clone` and `Rc::drop` borrow only the box's
`count` (CHG-0100): a reference into the value that `Rc::get` gave
through another handle may be live, and it is disjoint from the count,
where a borrow of the whole box would clash with it. `Rc::drop` ends
that borrow (the block holding `c`) before destroying the box:
`[Destroy]` requires `solitary`, and a still-live `c` — or the
projection paths its own `if (*c == 0)` test would have formed inside
the same statement scope — would be a second path on the reclaimed box
object (`diag.destroy-while-aliased`, `CHG-0009`). More generally: `N` independent checked handles, one raw
allocation, a count arbitrating the final destroy. The `unsafe`
blocks assert what the count protects. `Rc::get` returns a shared
reference into the reclaimed box, so mutation through an `Rc` is
impossible without a `mutex` inside it. `Rc` is not thread-safe
(the count is not synchronized); sharing across threads is
`ref<mutex<T>, shared>` (`spec/19` §2).

**Depends on:** rule.trust.rawptr, rule.stdlib.prelude,
rule.resauth.destroy, D-0003

## 3a. `Box<T>`

In `std::memory`.

### `rule.stdlib.box`
**Status:** ACCEPTED

A value on the heap with one owner (D-0045): `Rc` (§3) without the
count. `Box<T>` holds a `rawptr<T>`, so a type may contain itself through
one (`[Type-Recursive]`, `spec/16`): `struct Node { i32 v;
Option<Box<Node>> next; }`. `get` and `get_mut` borrow the value;
`into_inner` moves it out and frees the memory; destroying the `Box`
destroys the value and frees the memory.

    export resource struct Box<T>
    {
        rawptr<T> ptr;
        bool full;                          // false once into_inner has taken the value
    }

    // The size Box allocates: at least one byte, so every Box has an address.
    fn Box::cells<T>() : usize
    {
        if (sizeof<T>() == 0)
        {
            1
        }
        else
        {
            sizeof<T>()
        }
    }

    export fn Box::new<T>(T v) : Box<T>
    {
        rawptr<u8> p = match (allocate(Box::cells<T>(), alignof<T>()))
        {
            Ok(p) : p,
            Err(_) : fault(alloc_failure),
        };
        rawptr<T> bp = reinterpret_ptr<T>(p);
        unsafe
        {
            *bp = v;
        }
        Box { .ptr = bp, .full = true }
    }

    export fn Box::get<T>(ref<Box<T>, shared> b) : ref<T, shared>
    {
        unsafe
        {
            &reclaim<T>(b.ptr)
        }
    }

    export fn Box::get_mut<T>(ref<Box<T>, exclusive> b) : ref<T, exclusive>
    {
        unsafe
        {
            &mut reclaim<T>(b.ptr)
        }
    }

    // The value, moved out; the Box's memory is freed.
    export fn Box::into_inner<T>(Box<T> b) : T
    {
        b.full = false;
        unsafe
        {
            auto v = *b.ptr;
            release(reinterpret_ptr<u8>(b.ptr), sizeof<T>());
            v
        }
    }

    export fn Box::drop<T>(ref<Box<T>, exclusive> self)
    {
        unsafe
        {
            if (self.full)
            {
                drop(reclaim<T>(self.ptr));
            }
            deallocate(reinterpret_ptr<u8>(self.ptr), Box::cells<T>(), alignof<T>());
        }
    }

- **One owner.** A `Box` is a resource: it moves, and is destroyed once.
  Changing the value through `get_mut` needs an exclusive borrow of the
  `Box`, as for any value.
- **Size.** A `Box` allocates `sizeof<T>()` bytes, or one for a
  zero-sized `T`, so that every `Box` has its own address.

**Depends on:** rule.stdlib.rc, rule.trust.unsafe, rule.agg.layout, D-0045
**Affects:** state.storage

## 3b. Exchanging values behind references

In `std::core`.

### `rule.stdlib.swap`
**Status:** ACCEPTED

A value cannot be moved out of a place reached through a reference
(`diag.move-out-of-field`), so a program cannot exchange two values it
holds only by reference — two fields of a borrowed struct, two elements
of a `Vec` or a slice of resources — itself (D-0048). `std` does it
with one std-only intrinsic:

    [Swap-Places]
        ⟨a, Σ⟩ →* ⟨ref to place p, Σ1⟩, ⟨b, Σ1⟩ →* ⟨ref to place q, Σ2⟩, both exclusive
        ────────────────────────────────────────────
        ⟨swap_places(a, b), Σ⟩ → ⟨(), Σ2[p := value(q), q := value(p)]⟩     (p = q: Σ2 unchanged)

A resource keeps its single owner: the place it now occupies, which is
destroyed as any place is. Nothing is copied or destroyed.

    export fn swap<T>(ref<T, exclusive> a, ref<T, exclusive> b)
    {
        swap_places(a, b);
    }

    // Puts v in *r and returns what was there.
    export fn replace<T>(ref<T, exclusive> r, T v) : T
    {
        auto old = v;
        swap_places(r, &mut old);
        old
    }

    // D-0080: puts v in *r and destroys what was there, where the call is
    // (`[Write-Resource-Overwrite-Rejected]`'s repair).
    export fn overwrite<T>(ref<T, exclusive> r, T v)
    {
        drop(replace(r, v));
    }

    // Exchanges elements i and j of v; nothing when i == j.
    export fn Vec::swap<T>(ref<Vec<T>, exclusive> v, usize i, usize j)
    {
        if (i >= v.len || j >= v.len)
        {
            fault(index_out_of_bounds);
        }
        if (i != j)
        {
            swap_places(&mut (*v)[i], &mut (*v)[j]);
        }
    }

    // Exchanges elements i and j of s; nothing when i == j.
    export fn slice_swap<T>(slice<T, exclusive> s, usize i, usize j)
    {
        if (i >= slice_len(s) || j >= slice_len(s))
        {
            fault(index_out_of_bounds);
        }
        if (i != j)
        {
            swap_places(&mut s[i], &mut s[j]);
        }
    }

- `Vec::swap` and `slice_swap` exist because `swap(&mut v[i], &mut v[j])`
  with `i = j` is two exclusive borrows of one place: they check the
  bounds and do nothing when `i = j`.
- `replace` is `swap` with a fresh value, returning the old one.

**Depends on:** rule.stdlib.vec, rule.agg.slice, rule.alias.borrow, D-0048
**Affects:** the places `swap`, `replace`, `Vec::swap` and `slice_swap` are given

## 3c. Channels

In `std::sync`.

### `rule.stdlib.channel`
**Status:** ACCEPTED

`Channel<T>` (D-0063) realizes `rule.conc.channel` (`spec/19` §3) in
CobaltC: a `mutex` around a ring of `cap` slots, and two counts that
waiting threads watch — one a `send` waits on, which `recv` and `close`
change, and one a `recv` waits on, which `send` and `close` change —
kept by one primitive private to `std`:

    [Event-Op]
        ⟨event_op(0, _, _), Σ⟩ → ⟨e, Σ[events(e) := 0]⟩, e fresh
        ⟨event_op(1, e, _), Σ⟩ → ⟨events(e), Σ⟩
        ⟨event_op(2, e, s), Σ⟩ → ⟨events(e), Σ⟩ when events(e) ≠ s; otherwise no step (it waits),
                                 or ↛ diag.channel-deadlock as [Channel-Deadlock] says
        ⟨event_op(3, e, _), Σ⟩ → ⟨events(e) + 1, Σ[events(e) := events(e) + 1]⟩ (wrapping)
        ⟨event_op(4, e, _), Σ⟩ → ⟨0, Σ[events(e) removed]⟩

A change to a channel's state adds one to the count of the threads it
may let proceed while the channel's lock is held, and a waiter reads
its count while holding it, then waits for it to differ once the lock is
released: no change made in between is missed, and a `send` wakes no
waiting sender.

    // Channels (`rule.conc.channel`, spec/19 §3, D-0063) wait on event
    // counts, private to `std`: `event_op(0, …)` makes one and returns its
    // id; 1 reads its count; 2 waits until its count differs from `seen` (a
    // fault when no other thread could change it); 3 adds one to it, waking
    // whoever waits on it; 4 frees it. A channel's counts change only under
    // its lock, so a waiter that read one under the lock misses no change.
    extern fn event_op(usize op, usize id, usize seen) : usize;

    // A bounded queue between threads (`rule.conc.channel`, spec/19 §3,
    // D-0063), shared by reference as a `mutex` is: `send` waits while it is
    // full, `recv` while it is empty; after `close`, `send` gives its value
    // back and `recv` drains what is left, then gives `None`.
    struct ChannelState<T>
    {
        Vec<Option<T>> slots;               // a ring of `cap` slots
        usize head;                         // the oldest value's slot
        usize count;
        bool closed;
    }

    // The oldest value, taken from its slot; `st.count` is not 0.
    fn ChannelState::pop<T>(ref<ChannelState<T>, exclusive> st) : Option<T>
    {
        usize at = st.head;
        Option<T> none = None;
        Option<T> x = replace(&mut st.slots[at], none);
        st.head = (at + 1) % Vec::len(&st.slots);
        st.count = st.count - 1;
        x
    }

    export resource struct Channel<T>
    {
        mutex<ChannelState<T>> state;
        usize cap;
        usize can_send;                     // counts a waiting `send` watches:
        usize can_recv;                     // `recv` adds to it; and `send` to this
    }

    // A channel holding at most `cap` values; `cap` is at least 1.
    export fn Channel::new<T>(usize cap) : Channel<T>
    {
        if (cap == 0)
        {
            fault(channel_zero_capacity);
        }
        Vec<Option<T>> slots = Vec::new();
        usize i = 0;
        while (i < cap)
        {
            Option<T> none = None;
            Vec::push(&mut slots, none);
            i = i + 1;
        }
        usize can_send = unsafe
        {
            event_op(0, 0, 0)
        };
        usize can_recv = unsafe
        {
            event_op(0, 0, 0)
        };
        ChannelState<T> st = ChannelState { .slots = slots, .head = 0, .count = 0, .closed = false };
        Channel { .state = Mutex::new(st), .cap = cap, .can_send = can_send, .can_recv = can_recv }
    }

    // Puts v at the back, waiting while the channel is full; `Err(v)` once it
    // is closed.
    export fn Channel::send<T>(ref<Channel<T>, shared> ch, T v) : Result<void, T>
    {
        while (true)
        {
            usize seen = 0;
            {
                auto g = lock(&ch.state);
                if (g.closed)
                {
                    return Err(v);
                }
                if (g.count < ch.cap)
                {
                    usize at = (g.head + g.count) % ch.cap;
                    g.slots[at] = Some(v);
                    g.count = g.count + 1;
                    unsafe
                    {
                        event_op(3, ch.can_recv, 0);
                    }
                    return Ok(());
                }
                seen = unsafe
                {
                    event_op(1, ch.can_send, 0)
                };
            }
            unsafe
            {
                event_op(2, ch.can_send, seen);
            }
        }
        Err(v)
    }

    // Takes the value at the front, waiting while the channel is empty;
    // `None` once it is closed and empty.
    export fn Channel::recv<T>(ref<Channel<T>, shared> ch) : Option<T>
    {
        while (true)
        {
            usize seen = 0;
            {
                auto g = lock(&ch.state);
                if (g.count > 0)
                {
                    Option<T> x = ChannelState::pop(&mut *g);
                    unsafe
                    {
                        event_op(3, ch.can_send, 0);
                    }
                    return x;
                }
                if (g.closed)
                {
                    return None;
                }
                seen = unsafe
                {
                    event_op(1, ch.can_recv, 0)
                };
            }
            unsafe
            {
                event_op(2, ch.can_recv, seen);
            }
        }
        None
    }

    // No more values: `send` fails from now on, and `recv` gives `None` once
    // the channel is empty. Closing a closed channel does nothing.
    export fn Channel::close<T>(ref<Channel<T>, shared> ch)
    {
        auto g = lock(&ch.state);
        g.closed = true;
        unsafe
        {
            event_op(3, ch.can_send, 0);
            event_op(3, ch.can_recv, 0);
        }
    }

    // The values still in the channel are destroyed oldest first.
    export fn Channel::drop<T>(ref<Channel<T>, exclusive> self)
    {
        {
            auto g = lock(&self.state);
            while (g.count > 0)
            {
                Option<T> x = ChannelState::pop(&mut *g);
                drop(x);
            }
        }
        unsafe
        {
            event_op(4, self.can_send, 0);
            event_op(4, self.can_recv, 0);
        }
    }

- `send` and `recv` wait in a loop: each wake-up is only a reason to
  look again, under the lock.
- `ChannelState::pop` serves `recv` and the destructor, which destroys
  the values left oldest first.

**Depends on:** rule.conc.channel, rule.conc.lock, rule.stdlib.vec,
rule.stdlib.swap, D-0063
**Affects:** the channel's state

## 3d. Floating-point functions

In `std::math` (D-0136). Before D-0136 these were language intrinsics,
in scope without an import; they are ordinary functions of a library in
everything but how they are typed. Each takes an `f32` or an `f64`
(`atan2` and `powf` two of one type) and gives that type, which no
bound can say (`rule.type.bound` has `number`, which admits integers),
so `[Float-Fn]` types them here and both implementations realize them
natively, as `printf` is. Without `import std;` (or an import naming
one of them) their names are unbound (`[Resolve-Unbound]`), and a
program's own `round` is an ordinary item, found as any other.

### `rule.arith.float-fns`
**Status:** ACCEPTED

    [Float-Fn]
        f ∈ {sqrt, floor, ceil, round, trunc}, resolved to std::math's (rule.module.resolve);  Γ ⊢ e : τ,  τ ∈ {f32, f64}
        ────────────────────────────────────────────
        Γ ⊢ f(e) : τ;   ⟨f(v), Σ⟩ → ⟨r, Σ⟩ with r IEEE-754's result at τ:
            sqrt  — the correctly rounded square root (a NaN for v < 0, -0.0 for -0.0)
            floor, ceil, trunc — v rounded to an integral value toward -∞, +∞, 0
            round — v rounded to the nearest integral value, halfway cases away from zero
        (each exact or correctly rounded, so every conforming implementation gives
        the same bits; a NaN in gives a NaN out)

    [Float-Fn-Operand]   disposition: rejected
        f(e) with e not of type f32 or f64 (a type parameter's value included)
        ────────────────────────────────────────────
        ill-formed; diag.type-mismatch

The transcendental functions (D-0101) are the platform's: their last bit
differs between platforms' libraries, so they are specified to within
one unit in the last place, and every implementation on one platform
uses that platform's C library, so `coby` and `cobc` give the same bits
there.

    [Float-Transcendental]
        f ∈ {ln, exp, log2, log10, sin, cos, tan};  Γ ⊢ e : τ,  τ ∈ {f32, f64}
        ────────────────────────────────────────────
        Γ ⊢ f(e) : τ;   ⟨f(v), Σ⟩ → ⟨r, Σ⟩, r within 1 ulp of the mathematical value at τ
            (ln: natural logarithm; a NaN for v < 0, -∞ for ±0; the angles in radians)

    [Float-Transcendental-2]
        f ∈ {atan2, powf};  Γ ⊢ e1 : τ,  Γ ⊢ e2 : τ,  τ ∈ {f32, f64}
        ────────────────────────────────────────────
        Γ ⊢ f(e1, e2) : τ;   atan2(y, x): the angle of (x, y) in (-π, π];  powf(x, y): x to the power y
            (C's atan2 and pow: within 1 ulp; `pow` is the integer-exponent power of spec/21)

    [Float-Transcendental-Operand]   disposition: rejected
        an operand not of type f32 or f64, or two operands of different types
        ────────────────────────────────────────────
        ill-formed; diag.type-mismatch

**Depends on:** rule.arith.represent, D-0091

## Change Log

- 4.5.0 — `CHG-0167` (D-0139): `StringView::contains`, `replace`, `join`
  and `String::contains`, `replace`, `join` (`[Contains]`, `[Replace]`,
  `[Join]`, §2h; §0's `std::text` table). Additive.
- 4.4.0 — `CHG-0166` (D-0138): `HashSet::union`, `HashSet::intersection`
  and `HashSet::difference` (`[Set-Ops]`, §2g; §0's `std::collections`
  table). Additive.
- 4.3.0 — `CHG-0165` (D-0137): §2i, `Queue<T>` (`rule.stdlib.queue`)
  and `PriorityQueue<T>` (`rule.stdlib.priority-queue`,
  `[Priority-Queue-Not-Key]`) in `std::collections`; §0's submodule
  table and `std::collections` table list them. Additive.
- 4.2.0 — `CHG-0164` (D-0136): `std` divided into submodules —
  `std::core`, `std::collections`, `std::text`, `std::io`, `std::sys`,
  `std::memory`, `std::sync`, `std::random`, `std::math` — each re-exported
  by `std`'s root, so `import std;` and every `std::` path are unchanged;
  `std`'s submodules share one privacy; each section names its
  submodule. §3d (new): `rule.arith.float-fns`, moved from `spec/06`:
  `sqrt`, `floor`, `ceil`, `round`, `trunc`, `ln`, `exp`, `log2`,
  `log10`, `sin`, `cos`, `tan`, `atan2` and `powf` are `std::math`'s,
  no longer intrinsics.
- 4.1.0 — `CHG-0162` (D-0135): `[View-Form-Temporary-Argument]`: as a
  call's argument, a `String` that is a call's result, or part of one,
  may be viewed (`&f()[0..$]`), as D-0103 lets it be sliced.
- 4.0.0 — `CHG-0157`–`CHG-0161` (D-0134), the alignment of `std`:
  text a function only reads is a `StringView` (paths, `write_file`'s
  text, the needles of `find`/`starts_with`/`ends_with`),
  `StringView::of`, `[Str-Literal-View]`, `File::write_str` →
  `File::write_text`; `Rng::seed` → `Rng::new`, `map_err` →
  `Result::map_err`, `parse` → `String::parse`, `HashSet::at` →
  `HashSet::key_at`, the externs `write`/`read` → `stdout_write`/`stdin_read`
  (and `std`'s private `write_err` → `stderr_write`); `remove` keeps
  order, `swap_remove` is the constant-time removal, `remove_ordered` is
  gone; `ReadError` is gone, `read_line`'s error is `FileError`;
  `HashSet::clear`; §0's table grouped by what it serves, with a
  conventions paragraph. Breaking: every renamed name, and a
  `ref<String, shared>` where a `StringView` is now taken.
- 3.51.0 — `CHG-0152` (D-0129): `Vec::reserve` (§1, with a paragraph on
  its meaning), `String::reserve` (§2); `String::from_str` reserves its
  length (§2), as `String::clone` and `Vec::from_slice` do inside `std`.
- 3.50.0 — `CHG-0151` (D-0128): `StringView::chars`, `String::chars` (§0,
  §2h `rule.stdlib.stringview`, and its listing).
- 3.49.0 — `CHG-0144`–`CHG-0148` (D-0121–D-0125): `read_volatile`/`write_volatile`,
  `checked_narrow`, the bit-counting and rotation intrinsics, `sleep_ms`,
  `Weak<T>` (the `Rc` source: a weak count and an `Option<T>` value).
- 3.48.0 — `CHG-0143` (D-0120): `Rng` (`[Rng]`: xoshiro256** seeded by
  splitmix64, with check values).
- 3.47.0 — `CHG-0142` (D-0119): `Vec::push_le`, `push_be`, `read_le`, `read_be`
  (`[Bytes-LE]`, `[Bytes-BE]`); `crc32` (`[CRC32]`, CRC-32/IEEE written out).
- 3.46.0 — `CHG-0140` (D-0117): `FileError::text`, `ReadError::text`,
  `ParseError::text`, `Utf8Error::text`.
- 3.45.0 — `CHG-0139` (D-0116): `Vec::clone`, `from_slice`, `filled`, `extend_from`
  take `T: clone` and copy by `clone`; `Box::clone`, `HashMap::clone`,
  `HashSet::clone`.
- 3.44.0 — `CHG-0137` (D-0114): `Option::ok_or`.
- 3.43.0 — `CHG-0135` (D-0113): `String::as_view`, `find`, `starts_with`,
  `ends_with`, `trim`, `trim_start`, `trim_end`, `split`; `HashMap::get_str`,
  `get_mut_str`, `contains_str`, `remove_str`, `HashSet::contains_str`,
  `remove_str`; `[Lookup-Str]`.
- 3.42.0 — `CHG-0133` (D-0112): `Vec::filled`, `from_fn`, `append`,
  `extend_from`, `position`, `index_of`, `contains`, `reverse`.
- 3.41.0 — `CHG-0129` (D-0110): a struct or enum of key types is a key
  type (`[Key-Bytes]`, `[Sort]`).
- 3.40.0 — `CHG-0127` (D-0108): `str` is ordered (`[Cmp-Text]`).
- 3.39.0 — `CHG-0120` (D-0101): `ln`, `exp`, `log2`, `log10`, `sin`,
  `cos`, `tan`, `atan2`, `powf`; the ASCII helpers and
  `String::to_ascii_lower`/`upper`.
- 3.38.0 — `CHG-0119` (D-0100): `*` widths and precisions (a `usize`
  argument); `%v` of an enum writes its variant's name.
- 3.37.0 — `CHG-0107` (D-0092): `read_line` — on standard input and on
  a `File` — consumes one `\r` immediately before the `\n` it
  consumes.
- 3.36.0 — `CHG-0106` (D-0091): `sqrt`, `floor`, `ceil`, `round`, `trunc`;
  `min`, `max`, `abs`, `pow`; `HashMap::clear`.
- 3.35.0 — `CHG-0100` (D-0085): `Vec::from_slice`, `Vec::truncate`.
- 3.34.0 — `CHG-0100`: `Rc::clone` and `Rc::drop` borrow the count only,
  not the whole box, so a live `Rc::get` reference into the value does
  not clash with them.

- 3.33.0 — `CHG-0096` (D-0080): `overwrite`. `CHG-0098` (D-0082):
  `Option::unwrap`, `Option::expect`, `Result::unwrap`, `Result::expect`,
  `Vec::clone`, `Vec::clone_by`. `CHG-0099` (D-0083): `fault` takes any
  run-time diagnostic and an optional message; any other name is rejected.
- 3.32.0 — `CHG-0090` (D-0076): `StringView::less`, `String::less`,
  `String::truncate`.

- 3.31.0 — `CHG-0086` (D-0073): `Option::is_some`/`is_none`,
  `Result::is_ok`/`is_err`, `String::eq`/`eq_str`, `Vec::insert`/`remove`.

- 3.30.0 — `CHG-0085` (D-0072): `checked_neg`.
- 3.29.0 — Non-normative: `Vec::sort`'s source compares keys in place
  (`key_less_at`, `Vec::sorted_order_keys`); `[Sort]` unchanged.

- 3.28.0 — `CHG-0075` (D-0065): §0's table: `assert`.

- 3.27.0 — `CHG-0074` (D-0064): `Channel`'s source reaches the guarded
  state as `g.count` (`[Guard-Auto-Deref]`).

- 3.26.0 — `CHG-0073` (D-0063): §3c (new) `rule.stdlib.channel`; §0's
  table; `fault`'s names.

- 3.25.0 — `CHG-0071`: a read or write against a `File`'s open mode is
  exactly `Err(Io)`.

- 3.24.0 — `CHG-0070` (D-0062): §1b (new) `rule.stdlib.sort`; §0's table.

- 3.23.0 — `CHG-0069` (D-0061): §2e′ (new) `rule.stdlib.fs` and
  `rule.stdlib.env`; `PathKind`; §0's table and scope paragraph.

- 3.22.0 — `CHG-0067` (D-0059): `[File-Printf]` and `File::write_text`;
  `[File-Read]` reads `max` bytes, fewer only at the end.

- 3.21.1 — Non-normative (`CHG-0063`, D-0055): the scope paragraph notes
  that an item of an intrinsic's name does not reach `std`'s code.

- 3.21.0 — `CHG-0062` (D-0054): §2e gains `rule.stdlib.file-handle`
  (`File`, `read_bytes`, `write_bytes`); §0's table and scope paragraph.

- 3.20.0 — `CHG-0061` (D-0053): §2h (new) `rule.stdlib.stringview`; §0's table.

- 3.19.0 — `CHG-0060` (D-0052): §0's table gains `static_assert`.

- 3.18.0 — `CHG-0059` (D-0051): §0's table gives `min_value`/`max_value`
  for `f32` and `f64`; the conversions' new forms are `spec/06`'s.

- 3.17.0 — `CHG-0056` (D-0048): `swap`, `replace`, `Vec::swap`,
  `slice_swap` (`rule.stdlib.swap`, §3b).

- 3.16.0 — `CHG-0055` (D-0047): `slice_len` in §0's table. A `Vec` is
  indexed `v[i]` and sliced `&v[lo .. hi]` (`spec/16` §3a).

- 3.15.0 — `CHG-0053` (D-0045): `Box<T>` (`rule.stdlib.box`, §3a).

- 3.14.0 — `CHG-0052` (D-0044): `String::clone`, `String::push_ascii`
  (`[Push-Ascii-Not-Ascii]`) and `String::clear` in §2d; `Vec::clear`
  among §2g's code.

- 3.13.0 — `CHG-0050` (D-0042): §2g gains the holders a consuming
  `foreach` uses (`Drain`, `HashDrain`, `SetDrain`, `Vec::take_raw`, all
  private to `std`); §2f formats a reference to a printable value as
  the value.

- 3.12.0 — `CHG-0049` (D-0041): hash tables. `std` gains `HashMap<K, V>`
  and `HashSet<K>` (`rule.stdlib.hashmap`, §2g), written in CobaltC over
  the std-only intrinsics `key_hash` and `key_eq`, and the private
  `Vec::replace` and `Vec::remove_at`; §0's type list and table list
  them.

- 3.11.0 — `CHG-0048` (D-0040): `eprintf` writes formatted text to
  standard error (`rule.stdlib.format` `[Eprintf]`); §0's table and
  scope paragraph list it.

- 3.10.0 — `CHG-0047` (D-0039): one way to write output. `print` is no
  longer exported (`rule.stdlib.print` now defines the text `printf`,
  `%v` and `String::append` write); `rule.stdlib.format` gains `%v` and
  `sprintf`; §0's table and scope paragraph follow.

- 3.9.0 — `CHG-0046` (D-0038): formatted output. `std` gains `printf`
  and `String::appendf` (`rule.stdlib.format`, §2f); §0's table and
  scope paragraph list them.

- 3.8.0 — `CHG-0043` (D-0034): files. `std` gains `read_file`,
  `write_file` and `FileError` (`rule.stdlib.file`, §2e); §0's type
  list, table and scope paragraph list them.

- 3.7.0 — `CHG-0042` (D-0033): `Option::unwrap_or` and
  `Result::unwrap_or` in §0's table.

- 3.6.0 — `CHG-0041` (D-0032): text conversions. `std` gains
  `String::new`, `String::as_bytes`, `String::append<T>`, `parse<T>`
  and `ParseError` (`rule.stdlib.text`, §2d); §0's type list, table and
  scope paragraph list them.

- 3.5.0 — `CHG-0040` (D-0031): program arguments. `std` gains the
  `extern` `arg_bytes`, `arg_count` and `arg` (`rule.stdlib.args`,
  §2c); §0's table and the scope paragraph list them.

- 3.4.0 — `CHG-0038` (D-0029): standard input. `std` gains the
  `extern` `read`, `read_line` and `ReadError` (`rule.stdlib.read`,
  §2b); §0's type list and table list them.

- 3.3.0 — `CHG-0037` (D-0028): `print` is `print<T>(T x)`, printing
  `str`, integers, floats, `bool` and `ref<String, shared>`
  (`rule.stdlib.print`, §2a), realized natively; its former body is the
  `str` case.

- 3.2.0 — `CHG-0036`: §0 says an intrinsic's name yields to a local
  binding or item of the same name.

- 3.1.0 — `CHG-0035` (D-0026): intrinsics `min_value<T>()` and
  `max_value<T>()` (`rule.arith.limits`).

- 3.0.0 — `CHG-0033` (D-0024): the prelude's declarations form the
  root-level module `std`, reached by `import std;` or `std::…` paths,
  no longer in scope unqualified in every module. `write` is declared
  `export` (programs call it, `CHG-0020`/`CHG-0021`), and so is
  `Utf8Error`'s `offset` field (it reports where validation failed; it
  had been reachable only because the prelude shared the root module).
  The intrinsics and `str` are unchanged and need no import.
- 2.9.1 — Non-normative (owner-directed, 2026-09-23): §2a states
  `print`'s intended scope — the prelude prints `str` only, and output
  of `u8`, `Vec<u8>` and `String` is program-written through `write`.
  No rule changes.
- 2.9.0 — `CHG-0025` (D-0020, human-directed): §2a added — `type.str`
  (a non-resource text value type whose domain is well-formed UTF-8
  byte sequences) and `rule.stdlib.str` (`[Str-Literal]`, `[Str-Len]`,
  `[Str-Byte]`, `[Str-Byte-Out-Of-Bounds]`, `[Str-Ptr]`, and the
  ordinary function `print`). §0's table gains the three `str_*`
  intrinsics. §2 gains `String::from_str`, a second `String`
  constructor that carries `inv.str.utf8-validity` over into
  `inv.string.utf8-validity` without a validator; the prose no longer
  calls `from_utf8` "the only constructor".
- 2.8.0 — `CHG-0020`: `write(rawptr<u8> buf, usize len) : isize` added
  to §0's prelude as a named `extern fn` instance, so a conformance
  case's outcome can in principle be observed by a real running
  program (`feat.minimal-io-extern-surface`, now `ACCEPTED`). No new
  rule — `rule.trust.extern-call`'s `[Extern-Call]` already governs
  any `FfiType` signature; `isize`, `usize`, and `rawptr<u8>` are all
  in `FfiType` (`spec/20` §3).
- 2.7.0 — `CHG-0019`: `Vec::pop` additionally `release`s the popped
  slot after reading/moving its value out. Under 2.6.0, a plain-typed
  element's reclaimed object (from an earlier `index_shared`) survived
  a `pop` untouched — reading owns nothing, correctly — so a
  subsequent `push` reusing the same slot could silently overwrite the
  value a live shared reference into it observed, with no diagnostic
  (`spec/AUDIT-STATUS.md` finding F-05). A resource `T`'s `pop` is
  unaffected (`[Rawptr-Move-Out]` already ends the reclaimed object;
  the added `release` finds nothing there). `rule.trust.rawptr`
  `[Rawptr-Write]`/`[Rawptr-Read]` tightened to match (no rule any
  longer needs the `reclaimed-at(addrs)` exemption they previously
  carried).
- 2.6.0 — `CHG-0009`: `Rc::drop` restructured so the exclusive borrow
  of the box (`b`) ends before `drop(reclaim<RcBox<T>>(self.ptr))`
  runs; the 2.5.0 body destroyed the box inside the block that still
  held `b`, which `[Destroy-Not-Solitary]` faults
  (`diag.destroy-while-aliased`) on every last drop —
  `conf.rc-last-drop-frees` did not derive. `fault(d)` declared
  `never`-typed in the §0 table (its match-arm and statement uses
  already required it). Observable behavior of every prelude item is
  otherwise unchanged.
- 2.5.0 — Reformatted every CobaltC code sample to Allman brace style
  (human-directed documentation convention, `spec/22` §1; non-
  normative per `spec/02-schema.md` §6, no `CHG-XXXX` record — no
  token sequence's meaning changed, only line breaks and indentation).
  The `[Allocate]`/`[Deallocate]` CFN blocks are metalanguage notation,
  not CobaltC surface syntax, and are unchanged; `map_err`'s inline
  `match` stays single-line since it sits inside a table cell (the
  same structural exemption documented for `spec/conformance.md`).
- 2.4.0 — `CHG-0008`: `spawn` and `join` added to the §0 intrinsics
  table, now that `spec/22` 2.6.0 demotes them from dedicated grammar.
  `rule.conc.spawn`/`rule.conc.join` added to this section's Depends
  on. No behavior changed — this documents where their existing rules
  now live in the classification, nothing more.
- 2.3.0 — `CHG-0006`: every `pub` re-spelled `export` (21 sites: the
  prelude types, `Vec`, `String`, `Rc`, and their public functions),
  matching `spec/22` 2.5.0. No visibility changed — every item that
  was `pub` is now `export` and nothing else moved.
- 2.2.0 — `CHG-0005`: every `match` in `map_err`, `Vec::grow`, and
  `Rc::new` re-spelled with `:` instead of `=>`. No behavior changed.
- 2.1.0 — `CHG-0003`: every `if`/`while`/`match` in `Vec`, `String`,
  `Rc`, and `map_err` re-spelled with mandatory condition parentheses
  (`spec/22` 2.2.0). No behavior changed.
- 2.0.0 — Re-spelled to `spec/22` 2.0.0's C-style declarator syntax
  (`CHG-0001`): every function/associated-function/destructor
  signature flipped to type-first parameters with `:`-introduced
  return types; every struct declaration flipped to type-first,
  semicolon-terminated fields; every struct-literal construction
  (`Vec`, `String`, `Utf8Error`, `RcBox`, `Rc`) changed to `.f = e`
  form; every local declaration flipped to type-first or `auto`. The
  `[Allocate]`/`[Deallocate]` CFN rule blocks are metalanguage
  notation, not CobaltC surface syntax, and are unchanged. No rule's
  behavior, and no conformance case in `spec/conformance.md`, changed.
- 1.0.0 — Rewritten (`spec/AUDIT-2.md` B-05, B-07, B-11, B-20): §0
  prelude declares every intrinsic and prelude type previously used
  without declaration; `Vec`, `String`, `Rc` written in the surface
  language against the 1.0.0 rules (destructors, `reclaim`
  re-attachment, `Rawptr-Move-In/Out`); `from_utf8` given an actual
  validator; `fault`, `reinterpret_ptr` added. All entities
  `ACCEPTED`.
- 0.6.0 and earlier — superseded.
