# CobaltC Standard Library Semantics

Status: normative artifact
Version: 3.21.1
Conforms to: `spec/02-schema.md` (Kind: Rule, `rule.stdlib.*`; Kind: Type, `type.str`)
Governed by: `CobaltC_Master_Instructions.md` §1, §23

## Purpose and scope

Master Instructions §1 scopes the design to the language. This
artifact fixes the **prelude**: the intrinsics every conforming
implementation provides (operations whose semantics are rules in
`spec/06`, `spec/07`, `spec/20`), the prelude types (`Option`,
`Result`), the text value type `str` (§2a), and the library types —
`Vec<T>`, `String`, `HashMap<K, V>`, `HashSet<K>`, `Rc<T>`, `Box<T>` — written in CobaltC
itself as evidence that the language as specified is sufficient. Their bodies are normative for
their observable behavior; an implementation may realize them
differently provided every conformance case in `spec/conformance.md`
holds.

**The module `std`** (`CHG-0033`, D-0024). Every declaration this
artifact gives in CobaltC — the types and functions of §0–§3, `write`,
`read`, and `map_err` — is an item of one module, `std`, declared at
the root of every program (`export module std { … }`, in the order given here).
Its names are `std::Vec`, `std::Vec::push`, `std::printf`, …; a module
uses them unqualified after `import std;` (or a single-name import such
as `import std::Vec;`), and any module may write them qualified.
Being a module, `std` keeps what it does not export: `Vec`'s, `String`'s
and `Rc`'s fields, `RcBox`, and `Vec::grow` are reachable only from
inside it. The intrinsics of §0's table other than `write`, `read`,
`read_line`, `arg_bytes`, `arg_count`, `arg`, `parse`, `printf`, `sprintf`, `eprintf`, `read_file`, `write_file`, `read_bytes`, `write_bytes` and `map_err`, and the types `str` and `File`, belong to the language
and need no import. An intrinsic is not an item and its name is not reserved: a
name that resolves to a local binding (`rule.value-object.binding-lookup`)
or to an item (`[Resolve-Unqualified]`, `spec/17`) means that binding or
item, and the intrinsic of the same name is reached only where nothing
else is (`CHG-0036`). An item of an intrinsic's name is found only in
its own module and where an `import` names it, never through an
enclosing module (`[Resolve-Unqualified]` (3), D-0055), so `std`'s own
code always reaches the intrinsics. A program may not declare its own root-level `std`
(`[Item-Duplicate]`).

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

    export enum ReadError                    -- rule.stdlib.read
    {
        Io,
        Utf8(Utf8Error)
    }

    export enum FileError                    -- rule.stdlib.file
    {
        NotFound,
        Denied,
        Io,
        Utf8(Utf8Error)
    }

    export enum ParseError                   -- rule.stdlib.text
    {
        Empty,
        Invalid(usize),
        OutOfRange
    }

**Intrinsics** — functions whose call is defined by the named rule
rather than by a body; each is in scope unqualified; type arguments
written `name<τ>(…)` where the signature needs them:

| Signature | Rule |
|---|---|
| `drop<T>(T x)` (argument in place position) | `rule.resauth.destroy` `[Destroy]` on the place; a temporary argument is destroyed directly |
| `wrapping_add/sub/mul<T>(T a, T b) : T`, `saturating_*` | `rule.arith.alt` |
| `checked_add/sub/mul/div/rem<T>(T a, T b) : Option<T>` | `rule.arith.alt` |
| `widen<U>(T x) : U`, `narrow<U>`, `narrow_wrapping<U>`, `reinterpret<U>`, `to_float<U>`, `to_int<U>` | `rule.arith.convert` |
| `sizeof<T>() : usize`, `alignof<T>() : usize` | `rule.arith.sizeof` |
| `min_value<T>() : T`, `max_value<T>() : T` (`T` an integer type, `f32` or `f64`) | `rule.arith.limits` |
| `StringView` and its functions (`String::view`, `StringView::sub`, `len`, `eq`, `eq_str`, `find`, `starts_with`, `ends_with`, `trim`, `trim_start`, `trim_end`, `split`, `parse<T>`, `String::from_view`) | exported items of `std`, written in CobaltC: `rule.stdlib.stringview` (§2h); `&s[lo .. hi]` on a `String` and `==` with a view are its `[View-Form]` and `[View-Eq]` |
| `static_assert(bool c)`, `static_assert(bool c, str message)` : unit (`c` a constant expression, the message a string literal) | `rule.module.static-assert` (`spec/17` §1b): computed before the program runs; nothing at run time |
| `dangling<T>() : rawptr<T>` | `outcome: impl-defined { any T-aligned non-null address }`, documented; never dereferenced by prelude code |
| `rawptr_of<T>(ref<T, shared> r) : rawptr<T>`, and the `exclusive` overload | `rule.trust.rawptr` `[Rawptr-Of]` |
| `reclaim<T>(rawptr<T> p) : place of T` | `[Reclaim]` (unsafe) |
| `release(rawptr<u8> p, usize n)` | `[Release]` (unsafe) |
| `copy_raw(rawptr<u8> dst, rawptr<u8> src, usize n)` | `[Copy-Raw]` (unsafe) |
| `allocate(usize n, usize align) : Result<rawptr<u8>, AllocError>` | `[Allocate]` below |
| `deallocate(rawptr<u8> p, usize n, usize align)` | `[Deallocate]` below (unsafe) |
| `lock<T>(ref<mutex<T>, shared> m) : guard<T>` | `rule.conc.lock` |
| `Mutex::new<T>(T v) : mutex<T>` | `rule.conc.lock` (§2's construction) |
| `spawn(fn(τ1..τn) : τr f, τ1 a1, …, τn an) : handle<τr>` — argument count and types match `f`'s own signature, not a fixed arity | `rule.conc.spawn` `[Spawn]`, typed by `[T-Spawn]` (`spec/12`); `f` must be a `fn` value or a `move` closure |
| `join<T>(handle<T> h) : T` | `rule.conc.join` `[Join]`, typed by `[T-Join]` (`spec/12`) |
| `export extern fn write(rawptr<u8> buf, usize len) : isize;` | `rule.trust.extern-call` `[Extern-Call]` (`spec/20`); an instance declared in `std`, no dedicated rule |
| `export extern fn read(rawptr<u8> buf, usize len) : isize;` | `rule.stdlib.read` `[Read]`, an instance of `[Extern-Call]` declared in `std` |
| `read_line() : Result<Option<String>, ReadError>` | ordinary exported function of `std`, written in CobaltC: `rule.stdlib.read` |
| `export extern fn arg_bytes(usize i, rawptr<u8> buf, usize len) : isize;` | `rule.stdlib.args` `[Arg-Bytes]`, an instance of `[Extern-Call]` declared in `std` |
| `arg_count() : usize`, `arg(usize i) : Result<String, Utf8Error>` | ordinary exported functions of `std`, written in CobaltC: `rule.stdlib.args` |
| `String::append<T>(ref<String, exclusive> s, T x)` for a printable `T` | exported function of `std`, realized natively: `rule.stdlib.text` `[Append]` (§2d) |
| `parse<T>(ref<String, shared> s) : Result<T, ParseError>` for an integer or float `T` | exported function of `std`, realized natively: `rule.stdlib.text` `[Parse]` (§2d) |
| `printf(f, a1, …, an)`, `String::appendf(ref<String, exclusive> s, f, a1, …, an)`, `sprintf(f, a1, …, an) : String`, `eprintf(f, a1, …, an)` — `f` a string literal, any number of arguments | exported functions of `std`, realized natively: `rule.stdlib.format` `[Printf]`, `[Appendf]`, `[Sprintf]`, `[Eprintf]` (§2f) |
| `read_file(ref<String, shared> path) : Result<String, FileError>`, `write_file(ref<String, shared> path, ref<String, shared> text) : Result<void, FileError>` | exported functions of `std`, realized natively: `rule.stdlib.file` `[Read-File]`, `[Write-File]` (§2e) |
| `File` and its functions (`File::open`, `create`, `append`, `open_rw`, `read`, `read_to_end`, `read_line`, `write`, `write_str`, `seek`, `len`, `close`); `read_bytes(ref<String, shared> path) : Result<Vec<u8>, FileError>`, `write_bytes(ref<String, shared> path, slice<u8, shared> data) : Result<void, FileError>` | exported items of `std`, written in CobaltC over two primitives private to `std`: `rule.stdlib.file-handle` (§2e) |
| `print<T>(T x)` for a printable `T` | function of `std`, not exported (D-0039), realized natively: `rule.stdlib.print` (§2a) |
| `HashMap<K, V>`, `HashSet<K>` and their functions, for a key type `K` | exported types of `std`, written in CobaltC over the std-only intrinsics `key_hash` and `key_eq`: `rule.stdlib.hashmap` (§2g) |
| `Option::unwrap_or<T>(Option<T> o, T d) : T` | ordinary exported function of `std`: `match (o) { Some(v) : v, None : d }` (D-0033) |
| `Result::unwrap_or<T, E>(Result<T, E> r, T d) : T` | ordinary exported function of `std`: `match (r) { Ok(v) : v, Err(_) : d }` (D-0033); an unused `d` or error is destroyed at the call's end |
| `map_err<T, E1, E2>(Result<T, E1> r, fn(E1) : E2 f) : Result<T, E2>` | ordinary exported function of `std`: `match (r) { Ok(v) : Ok(v), Err(e) : Err(f(e)) }` (table cell — exempt from Allman, `spec/22` §1) |
| `reinterpret_ptr<U>(rawptr<T> p) : rawptr<U>` | identity on the address; safe |
| `str_len(str s) : usize`, `str_byte(str s, usize i) : u8`, `str_ptr(str s) : rawptr<u8>` | `rule.stdlib.str` `[Str-Len]`, `[Str-Byte]`, `[Str-Ptr]` (§2a); all safe |
| `slice_len(slice<τ, m> s) : usize` (also through a reference) | a slice's length, `spec/16` `rule.agg.slice` (D-0047); safe |
| `fault(d) : never` (prelude-internal; `d` one of `index_out_of_bounds`, `alloc_failure`) | `[Fault]`: `⟨fault(d), Σ⟩ ↛ diag.d`, `disposition: checked`; typed `never` (`[T-Never]`, `spec/12` §5), so it may stand where any type is expected |

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

`fault(d)` denotes the prelude's way to raise a checked fault with the
named diagnostic (`diag.index-out-of-bounds`, `diag.alloc-failure`):
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

## 2. `String`

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
Equality is `[Eq-Str]` (`spec/12` §4); `<`/`<=`/`>`/`>=`, arithmetic,
and logic are ill-typed on `str`. Its representation is `[Sizeof-Str]`
/`[Repr-Str]` (`spec/06` §7). `str ∉ FfiType` (`spec/20` §3).

The only way to write a `str` is a `str-literal` (`spec/22` §1); the
only operations on one are the three intrinsics below and
`String::from_str` (§2). There is no indexing syntax, no slicing, and
no borrowed view into a `str`'s bytes (D-0020's revisit conditions).

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
- **`str` and `String`** are written through `write`, with the `unsafe`
  block inside `std`: a program that only prints never writes `unsafe`
  itself.
- **Failure.** `write`'s `isize` claim is discarded; a short or failed
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

### `rule.stdlib.read`
**Status:** ACCEPTED

    export extern fn read(rawptr<u8> buf, usize len) : isize;

    export fn read_line() : Result<Option<String>, ReadError>
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
                read(cell, 1)
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
        ⟨read(p, n), Σ⟩ → ⟨claim : isize, Σ[ storage(p+j) := byte(b_j) for j < k ]⟩
            claim = k;  k = 0 only when n = 0 or the input has ended;  claim = −1, storing nothing, when
            the input cannot be read
        side-conditions: ⟦ [p, p+n) is storage outside every live object, as [Extern-Call] requires ⟧
                         discharge: trusted

`read` is `write`'s counterpart: an `extern` declared in `std`, called
inside `unsafe`, reading the bytes that come next on the program's
standard input. Its result is a claim like any extern's.

`read_line` is the safe way to read: it returns the next line as a
`String`, with no `unsafe` in the calling program.

- **A line** is every byte up to the next `\n` (byte 10), which is
  consumed and not included, or up to the end of input. Only `\n`
  ends a line: a `\r` before it is part of the line.
- **`Ok(None)`** means the input ended before the line's first byte.
  A last line with no `\n` is still `Ok(Some(…))`, and every call after
  the end is `Ok(None)` again.
- **`Err(Utf8(e))`** means the line is not UTF-8; `e.offset` is the bad
  byte's offset within the line. The line has been consumed, so the
  next call reads the line after it.
- **`Err(Io)`** means `read` reported that the input cannot be read.
  The bytes of that line already read are discarded.

Standard input belongs to the program's environment, which is outside
this specification (Master Instructions §1). An implementation
documents whether it provides standard input. One that does not stops
the program at its first call of `read`, saying so; that stop is not a
diagnostic of this specification, and everything the program did
before it happened as usual. A program that never reads runs the same
either way.

**Depends on:** rule.trust.extern-call, rule.trust.unsafe,
rule.trust.rawptr, rule.stdlib.vec, rule.stdlib.string, D-0029
**Affects:** state.storage

## 2c. Program arguments

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
like `read`. It copies at most `len` bytes and always returns the
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
`parse<T>(ref<String, shared> s) : Result<T, ParseError>` are exported
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
        Γ ⊢ parse<T>(s) : Result<T, ParseError>;   ⟨parse<T>(s), Σ⟩ → ⟨r, Σ'⟩ with
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
calls would. `parse<T>(&s)` accepts the whole `String` or nothing:

- **Strict.** No leading or trailing whitespace, `_`, other base, or
  `.5` / `5.` forms; `-` is invalid for an unsigned `T`, even for
  `-0`. Leading zeros are allowed. `Invalid(k)` points at the first
  byte that cannot continue a number (`"12x4"` → `Invalid(2)`), or past
  the end when the text stops too early (`"-"` → `Invalid(1)`,
  `"1e"` → `Invalid(2)`).
- **Order.** An invalid byte anywhere is `Invalid`, even when the
  digits before it are already out of range.
- **Floats** are correctly rounded in `T` itself (`parse<f32>` does
  not round through `f64`). Every text `%v` writes for a float `v`
  parses back to `v` in the same type (a NaN to a NaN).
- **Where `T` is checked.** At the call, as for `%v`; a type
  parameter passed through is checked at each instantiation.
- **Names.** `parse`, `ParseError` and its variants are items of `std`;
  a program's own of the same name takes precedence (D-0024).

**Depends on:** rule.stdlib.print, rule.stdlib.string, rule.stdlib.vec,
rule.type.kind, rule.arith.represent, D-0032
**Affects:** state.storage (through `Vec::push`)

## 2e. Files

### `rule.stdlib.file`
**Status:** ACCEPTED

`read_file` and `write_file` are exported functions of `std`, realized
natively by both implementations (§0's latitude), since they reach the
program's environment, as `read` and `arg_bytes` do.

    [Read-File]
        the bytes of the String path refers to name a file of the environment;
        its contents are the bytes c0..c_{n−1}
        ────────────────────────────────────────────
        ⟨read_file(path), Σ⟩ → ⟨r, Σ'⟩ with
            r = Ok(s)                   the String of c0..c_{n−1}, when they are UTF-8 (String::from_utf8);
            r = Err(Utf8(e))            when they are not, e as String::from_utf8 gives it;
            r = Err(NotFound)           when no such file exists;
            r = Err(Denied)             when the environment refuses to let the program read it;
            r = Err(Io)                 for any other failure (a directory, a device error, …)

    [Write-File]
        the bytes of the String path refers to name a file of the environment
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
- **Text.** Both work in `String`s: `read_file` checks the bytes are
  UTF-8, reporting the offset of the first that is not, as `read_line`
  does; `write_file` can write only UTF-8.
- **Names.** `std` has two enums with `Io` and `Utf8` variants,
  `ReadError` and `FileError`: a `match` on either names its variants
  unqualified, since the scrutinee's type fixes the enum, but a
  program constructing one writes `FileError::Io` (`spec/17`
  `[Resolve-Ambiguous]`).

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
            0 < j ≤ min(k, n − p) when p < n and k > 0, and j = 0 otherwise;
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
        ⟨File::write_str(f, text), Σ⟩ is ⟨File::write(f, &text's bytes[0..$]), Σ⟩

    [File-Seek]
        ⟨File::seek(f, pos), Σ⟩ → ⟨Ok(()), Σ'⟩, f's position becomes pos
        (beyond the end is allowed: reading there gives 0 bytes, writing there extends the file)
        ⟨File::len(f), Σ⟩ → ⟨Ok(n), Σ⟩, n the file's length in bytes

    [File-Close]
        ⟨File::close(f), Σ⟩ → ⟨r, Σ'⟩, f closed and destroyed
            r = Ok(())  when f was open only for reading, or its bytes reached the
                        environment's storage (made durable);
            r = Err(e)  when the environment reports a failure; f is closed all the same

    [File-Drop]
        destroying an open f closes it, without making it durable and without reporting a failure

Any operation above gives `Err(Io)` (or `Err(Denied)`, `Err(NotFound)`)
when the environment reports a failure, as `[Read-File]` does; `read`
on a file opened only for writing, or `write` on one opened only for
reading, is such a failure.

    [Read-Bytes]
        ⟨read_bytes(path), Σ⟩ → ⟨r, Σ'⟩ with r = Ok(v), v the file's bytes, or the error
        [File-Open]'s open or [File-Read]'s read_to_end gives
    [Write-Bytes]
        ⟨write_bytes(path, data), Σ⟩ → ⟨r, Σ'⟩ with r = Ok(()), the file holding exactly
        data's bytes, or the error [File-Open]'s create or [File-Write] gives

- **Buffering.** Reading is buffered: `read_line` does not read the
  environment a byte at a time, and a `read` returns what is ready (at
  most a MiB). Writing is not: each `write` is handed to the
  environment before it returns, so another program sees it. Mixing
  them on an `open_rw` file is exact: a write lands at the position
  reading reached.
- **Positions** are `u64`, not `usize`, so a file larger than the
  address space is still addressed.
- **Lines.** `read_line` removes the final `'\n'` only, as `read_line`
  on standard input does; a `'\r'` before it stays.
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

    fn File::start(ref<String, shared> path, usize mode) : Result<File, FileError>
    {
        isize r = unsafe
        {
            file_op(0, mode, reinterpret_ptr<u8>(path.bytes.ptr), path.bytes.len)
        };
        if (r < 0)
        {
            return Err(file_error(r));
        }
        Ok(File { .id = reinterpret<usize>(r), .open = true })
    }

    // For reading; the file must exist.
    export fn File::open(ref<String, shared> path) : Result<File, FileError>
    {
        File::start(path, 0)
    }

    // For writing, from empty: created, or its contents discarded.
    export fn File::create(ref<String, shared> path) : Result<File, FileError>
    {
        File::start(path, 1)
    }

    // For writing at its end, created if it does not exist.
    export fn File::append(ref<String, shared> path) : Result<File, FileError>
    {
        File::start(path, 2)
    }

    // For reading and writing, created if it does not exist, its contents kept.
    export fn File::open_rw(ref<String, shared> path) : Result<File, FileError>
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

    // The next line without its '\n', as `read_line` reads standard input;
    // None at the end of the file.
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

    export fn File::write_str(ref<File, exclusive> f, ref<String, shared> text) : Result<void, FileError>
    {
        File::write(f, &text.bytes[0..$])
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
    export fn read_bytes(ref<String, shared> path) : Result<Vec<u8>, FileError>
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
    export fn write_bytes(ref<String, shared> path, slice<u8, shared> data) : Result<void, FileError>
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

## 2f. Formatted output

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
                as [Extern-Call] of std's private write_err(p, n) does    (Σ1 as for [Printf])

    [Format-Invalid]   disposition: rejected
        f is not a string literal, or not a sentence of format (below)
        ────────────────────────────────────────────
        ill-formed; diag.format-invalid

    [Format-Arg-Mismatch]   disposition: rejected
        the number of specifiers in f is not n, or a_i's type is not one its specifier takes
        (a type parameter: at each instantiation, rule.type.kind)
        ────────────────────────────────────────────
        ill-formed; diag.type-mismatch

    format     := (byte other than '%' | '%%' | spec)*
    spec       := '%' flag* width? ('.' precision?)? conversion      -- width, precision: digits, at most 4096
    flag       := '-' | '0' | '+' | ' ' | '#'

| Conversion | Argument | Text | Flags it takes |
|---|---|---|---|
| `d` `i` `u` | any integer type | the value in decimal | `- 0 + space`; a precision is a minimum digit count |
| `x` `X` `o` `b` | any integer type | the value's bits in its own width, in base 16 (lower/upper case), 8, 2 | `- 0 #` (`#`: prefix `0x`, `0X`, `0o`, `0b`); precision as for `d` |
| `f` `F` | `f32`, `f64` | fixed point, `precision` digits after the point (6 by default), correctly rounded | `- 0 + space #` |
| `e` `E` | `f32`, `f64` | `d.ddde±XX`, `precision` digits after the point (6), an exponent of at least two digits | as `f` |
| `g` `G` | `f32`, `f64` | `precision` significant digits (6; 0 is 1): `e` form when the exponent is below −4 or at least the precision, else `f` form; trailing zeros removed unless `#` | as `f` |
| `s` | `str`, `ref<String, shared>`, `bool` (`true`/`false`) | the text, at most `precision` characters | `-` |
| `v` | any printable type (§2a) | `text(v)`: what `String::append` adds — an integer in decimal, a float in the shortest form that reads back, text and `bool` as they are | `-` (no precision) |

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
- **Not included:** `%c` (a byte above 127 would not be UTF-8 in a
  `String`), `%p`, `%n`, `*` widths, and length modifiers.
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
  (`parse`, §2d); `%g` does not promise that.
- **Standard error.** `eprintf` is for messages that are not the
  program's output: errors, warnings, progress. Both streams are written
  at once, with nothing held back, so their text interleaves in the
  order the calls ran. `std` does not export `write_err`, the extern
  `eprintf` writes through; its failures are discarded as `write`'s are.
- **Names.** `printf`, `sprintf`, `eprintf` and `String::appendf` are items of
  `std`; a program's own of the same name takes precedence (D-0024),
  and the `std::` name always means this one.

**Depends on:** rule.stdlib.print, rule.stdlib.text, rule.stdlib.str,
rule.stdlib.string, rule.type.kind, rule.trust.extern-call, D-0038,
D-0039, D-0040
**Affects:** standard output; standard error (`eprintf`); `*s` (`String::appendf`); state.storage (`sprintf`'s `String`)

## 2g. Hash tables

### `rule.stdlib.hashmap`
**Status:** ACCEPTED

`HashMap<K, V>` maps keys to values and `HashSet<K>` holds keys; both
are exported types of `std`, written in CobaltC below (D-0041). A **key
type** is an integer type, `bool`, `str` or `String`. Hashing and
equality depend on `K`, which a CobaltC body cannot inspect (D-0010),
so they are two std-only intrinsics realized natively, as
`append_native` is (§2d):

    [Key-Bytes]
        bytes(v : τ integer)   = v in two's complement, width(τ)/8 bytes, least significant first
        bytes(v : bool)        = one byte, 1 or 0
        bytes(v : str)         = its bytes;   bytes(v : String) = the bytes of v.bytes

    [Key-Hash]    ⟨key_hash(r), Σ⟩ → ⟨h, Σ⟩     r : ref<K, shared> to v;  h : u64 = FNV-1a-64(bytes(v))
    [Key-Eq]      ⟨key_eq(r1, r2), Σ⟩ → ⟨b, Σ⟩   b = (bytes(v1) = bytes(v2))

        FNV-1a-64(b0..b_{n−1}):  h := 14695981039346656037;  for each bj: h := (h ⊕ bj) × 1099511628211 mod 2^64

`key_hash`'s value is not observable: it chooses only where an entry
sits in `slots`, and nothing a program can read depends on that.

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
    // `HashMap::remove` uses it.
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
    // removal. `remove_ordered` keeps the order.
    export fn HashMap::remove<K, V>(ref<HashMap<K, V>, exclusive> m, ref<K, shared> k) : Option<V>
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
    // It takes time in proportion to the size of the map.
    export fn HashMap::remove_ordered<K, V>(ref<HashMap<K, V>, exclusive> m, ref<K, shared> k) : Option<V>
    {
        usize s = HashMap::find(&*m, k);
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

    // Removes k; true if it was present. As `HashMap::remove`, the last key
    // takes k's place.
    export fn HashSet::remove<K>(ref<HashSet<K>, exclusive> s, ref<K, shared> k) : bool
    {
        match (HashMap::remove(&mut s.map, k))
        {
            Some(_) : true,
            None : false,
        }
    }

    // Removes k keeping the order of the others; true if it was present.
    export fn HashSet::remove_ordered<K>(ref<HashSet<K>, exclusive> s, ref<K, shared> k) : bool
    {
        match (HashMap::remove_ordered(&mut s.map, k))
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
    export fn HashSet::at<K>(ref<HashSet<K>, shared> s, usize i) : ref<K, shared>
    {
        HashMap::key_at(&s.map, i)
    }

- **Order.** Entry `i` (`key_at`, `value_at`, `HashSet::at`, for
  `i < len`) is in insertion order. Replacing a key's value keeps its
  place. `remove` takes constant time by moving the last entry into the
  removed one's place; `remove_ordered` keeps every other entry's place
  and takes time in proportion to the map's size. The order depends
  only on the calls, never on hash values, so it is the same in every
  implementation.
- **Ownership.** The map owns its keys and values and destroys them
  with itself. `insert` of a present key returns the old value and
  destroys the key passed in; `entry` of a present key destroys both
  its key and `fresh`.
- **Lookups borrow the key** (`ref<K, shared>`), so a lookup copies
  nothing. In a `HashMap<String, V>`, a program looking up a literal
  binds a `String` first (`[Ref-Form-Temporary]` rejects `&` of a
  temporary).
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

A view borrows its `String` as a `slice<u8, shared>` of it does: while
it lives the `String` cannot be changed (`diag.aliasing-conflict`), and
it cannot outlive it (`diag.reference-escapes-scope`, or
`diag.stale-binding` where the static analysis does not follow it). A
`StringView` counts as a borrowed type for `rule.temporal.elision`, so a
function returning one takes exactly one borrowed parameter. It is
printable (`%s`, `%v`, `String::append`), and no other operator applies
to it. Positions are byte positions; white space for `trim` is ASCII.

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
    fn StringView::at(StringView v, usize at, str t) : bool
    {
        usize m = str_len(t);
        if (at + m > slice_len(v.bytes))
        {
            return false;
        }
        for (usize j = 0; j < m; j += 1)
        {
            if (v.bytes[at + j] != str_byte(t, j))
            {
                return false;
            }
        }
        true
    }

    // The byte position of the first occurrence of `t` in `v`.
    export fn StringView::find(StringView v, str t) : Option<usize>
    {
        usize n = slice_len(v.bytes);
        usize m = str_len(t);
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

    export fn StringView::starts_with(StringView v, str t) : bool
    {
        StringView::at(v, 0, t)
    }

    export fn StringView::ends_with(StringView v, str t) : bool
    {
        usize n = slice_len(v.bytes);
        usize m = str_len(t);
        m <= n && StringView::at(v, n - m, t)
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
        usize start = 0;
        usize i = 0;
        while (i + m <= n)
        {
            if (StringView::at(v, i, sep))
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
        parse<T>(&s)
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

## 3. `Rc<T>`

### `rule.stdlib.rc`
**Status:** ACCEPTED

    struct RcBox<T>
    {
        usize count;
        T value;
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
            Err(_) : fault(alloc_failure)
        };
        rawptr<RcBox<T>> bp = reinterpret_ptr<RcBox<T>>(p);
        unsafe
        {
            *bp = RcBox { .count = 1, .value = v };          // [Rawptr-Move-In] if T is a resource
        }
        Rc { .ptr = bp }
    }

    export fn Rc::clone<T>(ref<Rc<T>, shared> r) : Rc<T>
    {
        unsafe
        {
            ref<RcBox<T>, exclusive> b = &mut reclaim<RcBox<T>>(r.ptr);
            b.count = b.count + 1;
        }
        Rc { .ptr = r.ptr }
    }

    export fn Rc::get<T>(ref<Rc<T>, shared> r) : ref<T, shared>
    {
        unsafe
        {
            &reclaim<RcBox<T>>(r.ptr).value
        }
    }

    export fn Rc::drop<T>(ref<Rc<T>, exclusive> self)
    {
        bool last = false;
        unsafe
        {
            ref<RcBox<T>, exclusive> b = &mut reclaim<RcBox<T>>(self.ptr);
            b.count = b.count - 1;
            last = b.count == 0;
        }                                                    // b's object ends here, and with it its borrow of the box
        if (last)
        {
            unsafe
            {
                drop(reclaim<RcBox<T>>(self.ptr));              // destroys value if T is a resource; solitary: no
                                                                // path into the box survives the block above
                deallocate(reinterpret_ptr<u8>(self.ptr), sizeof<RcBox<T>>(), alignof<RcBox<T>>());
            }
        }
    }

`Rc<T>` realizes shared destroy authority as a library pattern
(D-0003's allowance). `Rc::drop` ends its exclusive borrow of the box
(the block holding `b`) before destroying the box: `[Destroy]`
requires `solitary`, and a still-live `b` — or the projection paths
its own `if (b.count == 0)` test would have formed inside the same
statement scope — would be a second path on the reclaimed box object
(`diag.destroy-while-aliased`, `CHG-0009`). More generally: `N` independent checked handles, one raw
allocation, a count arbitrating the final destroy. The `unsafe`
blocks assert what the count protects. `Rc::get` returns a shared
reference into the reclaimed box, so mutation through an `Rc` is
impossible without a `mutex` inside it. `Rc` is not thread-safe
(the count is not synchronized); sharing across threads is
`ref<mutex<T>, shared>` (`spec/19` §2).

**Depends on:** rule.trust.rawptr, rule.stdlib.prelude,
rule.resauth.destroy, D-0003

## 3a. `Box<T>`

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

## Change Log

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
