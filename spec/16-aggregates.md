# CobaltC Aggregates

Status: normative artifact
Version: 1.22.0
Conforms to: `spec/02-schema.md` (Kind: Type `type.struct`,
`type.array`, `type.enum`; Kind: Rule, `rule.agg.*`)
Governed by: `CobaltC_Master_Instructions.md` §23
Realizes: D-0006, D-0010, D-0014, D-0018, D-0019

## Purpose

Defines struct, array, and enum types; their layout; construction as
temporaries; field/index projections; `match`; and generic
declarations.

## 1. Layout

### `rule.agg.layout`
**Status:** ACCEPTED

    [Layout-Struct]   outcome: impl-defined { any offset assignment satisfying the side-conditions }
        τ = struct { f1: τ1, …, fn: τn }
        ────────────────────────────────────────────
        off(f1) < … < off(fn);   off(fi) mod alignof(τi) = 0;
        off(fi) + sizeof(τi) ≤ off(f_{i+1});   off(fn) + sizeof(τn) ≤ sizeof(τ);
        alignof(τ) = max(alignof(τi), 1);   sizeof(τ) mod alignof(τ) = 0;
        sizeof(τ) is the least value admitting such offsets (no trailing padding beyond alignment)

    [Layout-Array]    off(i) = i · sizeof(τ);  sizeof = N · sizeof(τ);  alignof = alignof(τ)

    [Layout-Enum]     outcome: impl-defined { discriminant width DW ∈ {1,2,4,8} }   (spec/06 §7 [Repr-Enum])
        τ = enum { V1(τ1), …, Vk(τk) }
        ────────────────────────────────────────────
        discriminant at offset 0, width DW;   payload-off = least multiple of max(alignof(τi)) ≥ DW;
        sizeof(τ) = least multiple of alignof(τ) ≥ payload-off + max(sizeof(τi));
        alignof(τ) = max(DW, alignof(τi))

    sub-range(o, ε, Σ) = extent(o,Σ);
    sub-range(o, f·p, Σ) = sub-range(o', p, ·) relative to the cells [base + off(f), base + off(f) + sizeof(τf)) of the struct at sub-range(o, ·)
    sub-range(o, i·p, Σ)  likewise with off(i);
    sub-range(o, Vi·p, Σ) likewise with payload-off;   sub-type accordingly (τf, τ, τi)

A conforming implementation commits to one offset assignment and one
`DW` and documents them (`spec/06` §7's FFI condition). Declaration
order is preserved; padding is the only freedom.

    [Type-Recursive]   disposition: rejected
        a struct or enum τ contains τ by value: through a field, a payload, an array element or a
        `mutex`'s value, with the type arguments of each generic on the way substituted, and no
        `ref`, `rawptr`, `fn`, `handle` or `guard` on the way
        ────────────────────────────────────────────
        ill-formed; diag.recursive-type (static)

Such a τ has no `sizeof`: the equations above have no solution.
Recursion goes through something that holds a pointer: a `Box<T>`,
`Rc<T>` or `Vec<T>` (`spec/21`), which are built on `rawptr<T>`.
`struct Node { i32 v; Option<Node> next; }` is rejected; `Option<Box<Node>>`
in its place is accepted (D-0045).

**Depends on:** rule.arith.sizeof, rule.arith.represent, D-0019, D-0045

## 2. Struct

### `type.struct`
**Status:** ACCEPTED

`struct Name<T…> { τ1 f1; …; τn fn; }`, optionally marked `resource`
(`spec/22` §3). Values: `⟨f1: v1, …⟩`. `is-resource` per
`rule.type.is-resource`. A struct with no fields is permitted
(`sizeof = 0`).

### `rule.agg.struct-construct`
**Status:** ACCEPTED

    [Struct-Construct]
        Name<σ>{ .f1 = e1, …, .fn = en }, fields in declaration order (spec/22: any written order;
            evaluation follows written order, D-0007; each field exactly once, rule.type.typing)
        ⟨e_i, Σ_{i-1}⟩ →* ⟨r_i, Σ_i⟩,  e_i in place position iff is-resource(τi)
        ⟨establish(Name<σ>), Σ_n⟩ →^ℓ ⟨o, Σ'_0⟩                              -- rule.value-object.object-establish
        ⟨store(sub(o, f_i, τi), r_i), Σ'_{i-1}⟩ →^ℓ ⟨(), Σ'_i⟩  for i = 1..n     -- rule.value-object.store
        ────────────────────────────────────────────
        ⟨Name<σ>{…}, Σ⟩ →^ℓ ⟨temp o, Σ'_n[ init(o) := valid ]⟩

Construction yields a temporary (D-0019): a live object with its own
storage, authority and obligation if `is-resource(Name<σ>)`
(granted by establishment — D-0016), and no holder until the
enclosing local declaration/argument/field/return stores it. Resource-bearing
field initializers are moved in (`[Store-Sub-Relocate]`: a binding
source is invalidated, a temporary source ends); plain ones are
copied; references are recorded as held by `o`.

**Depends on:** rule.value-object.object-establish,
rule.value-object.store, rule.resauth.relocate-in, rule.type.typing,
D-0016, D-0019

### `rule.agg.field-access`
**Status:** ACCEPTED

    [Field-Access]
        ⟨e, Σ⟩ →* ⟨place a0, Σ1⟩;  type(a0,Σ1) = τ_struct with field f: τf
        temporally-valid(a0, Σ1);  a ∉ dom(Σ1.access-paths)
        ────────────────────────────────────────────
        ⟨e.f, Σ⟩ →^ℓ ⟨place a, Σ1[ access-paths(a) := { target: sub-range(of(a0), path(a0)·f, Σ1), of: of(a0,Σ1),
                                       type: τf, mode: mode(a0,Σ1), thread: ℓ, valid: true,
                                       formed-at: this-event, frame: current-frame(ℓ,Σ1), temp-scope: current-scope(ℓ,Σ1),
                                       base: a0, held-by: ∅ } ]⟩

    [Field-Access-Auto-Deref]
        ⟨e, Σ⟩ →* ⟨v, Σ1⟩,  v = a_r : ref<τ_struct, m>          -- e's static type is a reference: read it
        ────────────────────────────────────────────
        ⟨e.f, Σ⟩ ≡ ⟨(*e).f, Σ⟩   i.e. [Field-Access] with a0 = a_r

    [Guard-Auto-Deref]   (D-0064)
        e's static type is guard<τ>                              -- e in place position: the guard is not read
        ────────────────────────────────────────────
        ⟨e.f, Σ⟩ ≡ ⟨(*e).f, Σ⟩,   ⟨e[i], Σ⟩ ≡ ⟨(*e)[i], Σ⟩   i.e. through the lock path, spec/19 [Guard-Deref]

    [Temp-Root]
        ⟨e, Σ⟩ →* ⟨temp o, Σ1⟩;  e is the base of .f or [i]
        ────────────────────────────────────────────
        ⟨e, Σ⟩ →* ⟨place a_o, Σ1[ access-paths(a_o) := root record for o: target extent(o), type type-of(o),
                                    mode exclusive, base None, held-by ∅, frame current-frame, temp-scope current-scope ]⟩

    [Field-Access-Stale]   disposition: checked   ¬temporally-valid(a0,Σ1)   ⟨e.f, Σ⟩ ↛ diag.stale-binding

A field or element of a guarded value is reached through its guard as
through a reference: `g.head`, `g.v[0]`, `g[i]` for a guarded array or
`Vec` (`[Guard-Auto-Deref]`, D-0064). `*g` is still written for the
whole value (`*g = *g + 1`).

A projection narrows an existing path: same object, sub-range target,
the field's type, the base's mode, `base := a0`. No conflict check is
performed at formation — a projection is checked at use like any path
(D-0018, `spec/08` §3). Projecting from a temporary forms a root path
for it that dies with the temporary at statement exit; borrowing such
a projection is rejected (`rule.ref.form` `[Ref-Form-Temporary]`:
`diag.borrow-of-temporary`) except as a call's argument
(`[Ref-Form-Temporary-Part-Argument]`, D-0103: `String::len(&t().name)`,
`sum(&[1, 2, 3][0..$])`). `path(a0)` is the projection
path from `a0`'s root (`ε` for a root).

    [Temp-Field-Move]                                                 -- D-0103
        f().x (a chain of fields of a temporary of struct type S, S without a destructor)
        in a position that moves its value, the field of a resource type
        ────────────────────────────────────────────
        ⟨f().x, Σ⟩ ≡ ⟨{ S { x, y1, …, yk } = f(); x }, Σ⟩       -- spec/11 [Let-Destructure]

Moving a resource field out of a temporary takes the temporary apart
there (D-0103): the field is the value, and the temporary's other
fields end at once, in declaration order, as a destructuring's unused
bindings would. A struct with a destructor needs all its fields, so it
is not taken apart (`diag.move-out-of-field`, as for `[Let-Destructure]`).

**Depends on:** term.access-path, inv.spatial-validity,
inv.temporal-validity, rule.agg.layout, rule.ref.deref, D-0018,
D-0019

## 2a. Bitstruct (D-0118)

### `type.bitstruct`
**Status:** ACCEPTED

    bitstruct Name : uN { f1 : w1; …; fk : wk; }          -- spec/22 §3; uN one of u8 .. u128; Σ wi = N; each wi ≥ 1

A `bitstruct` is a struct whose fields are runs of bits of one unsigned
integer, its **backing type**, low bit first in declaration order:
`f1` is bits `0 .. w1`, `f2` the next `w2` bits, and so on, filling
`uN` exactly (unused bits are a named field). Field `fi` has the
smallest unsigned type holding `wi` bits: `u8` up to 8, `u16` up to 16,
`u32` up to 32, `u64` up to 64, else `u128`. A bitstruct takes no type
parameters, is never `resource`, and is declared at item level or, as
a local type, in a block (`spec/17` §1c, D-0132), where `Name::bits` and
`Name::from_bits` are derived for it as at item level. It
is a plain type (`is-resource = false`): copied by a read, `clone`, `eq`
by its bits, a key type (`[Key-Bytes]` gives its backing integer's
bytes), and its `%v` is not defined (`Name::bits(v)` is).

    [Repr-Bitstruct]   represent(Name, v) = represent(uN, bits(v)),  bits(v) = Σ_i (v.fi << off_i), off_1 = 0, off_{i+1} = off_i + w_i
                       sizeof(Name) = sizeof(uN);  alignof(Name) = alignof(uN)      -- rule.agg.layout: no field has an offset

    [Bitstruct-Construct]   Name { .f1 = e1, …, .fk = ek }  as [Struct-Construct], every field named once, each ei : type(fi), then [Bitfield-Overflow] for each

    [Bitfield-Read]    v.fi  as [Field-Access] (through a reference or guard too): the value (bits(v) >> off_i) & (2^wi − 1), of type(fi)

    [Bitfield-Write]   p.fi = e  as [Write] to the whole p, after [Bitfield-Overflow]: bits'(p) = (bits(p) & ¬(mask_i << off_i)) | (e << off_i)

    [Bitfield-Overflow]   disposition: checked
        a value e written into (or constructed for) field fi of width wi;  e ≥ 2^wi
        ────────────────────────────────────────────
        ⟨…, Σ⟩ ↛ diag.narrowing-overflow        -- static when e is a literal or literal-bound (rule.control.flow-analysis), else dynamic

    [Bitfield-No-Place]   disposition: rejected
        &p.fi, &mut p.fi, or &p.fi[..]  for a bitstruct field
        ────────────────────────────────────────────
        ill-formed; diag.borrow-of-non-place       -- a run of bits has no address; borrow the whole value

    [Bitstruct-Whole]   disposition: rejected
        Name { … } = e  (destructuring, spec/22 decl-stmt)
        ────────────────────────────────────────────
        ill-formed; diag.type-mismatch             -- read its fields instead

    [Bitstruct-Bits]        Name::bits(Name v) : uN         ≡  bits(v)
    [Bitstruct-From-Bits]   Name::from_bits(uN b) : Name    ≡  Name { .f1 = (b >> off_1) & mask_1, … }     -- total: every uN is a Name

`bits` and `from_bits` are functions the program has as if declared
beside the type (as a derived `clone` is, §4a). A write to a field is
a write to the whole value for the aliasing rules (`spec/08`): an
exclusive path to `p` is what it needs, and a shared reference to `p`
may read any field. Byte order is not a bitstruct's concern: a value
crosses into bytes by `Vec::push_le`/`push_be` (D-0119) or a raw
pointer to the whole (`[Rawptr-Read]`, whose image is `[Repr-Bitstruct]`).
A bitstruct is not an `FfiType`: an `extern fn` takes or returns
`Name::bits(v)` / `Name::from_bits(x)`.

**Depends on:** rule.agg.struct-construct, rule.agg.field-access, rule.agg.layout, rule.arith.convert `[Narrow-Overflow]`, rule.type.is-resource, D-0118

## 3. Array

### `type.array`
**Status:** ACCEPTED

`array<τ, N>`, `N` a constant expression of `usize` (`spec/22` `length`,
D-0126). Values `[v0, …, v_{N-1}]`. One
object identity for the whole array; elements are projections.
`is-resource(array<τ,N>) = is-resource(τ)`.

### `rule.agg.array-construct`
**Status:** ACCEPTED

    [Array-Construct]
        [e0, …, e_{N-1}] : array<τ,N>;   ⟨e_i, Σ_{i-1}⟩ →* ⟨r_i, Σ_i⟩ left to right
        ⟨establish(array<τ,N>), Σ_N⟩ →^ℓ ⟨o, Σ'_0⟩
        ⟨store(sub(o, i, τ), r_i), Σ'_{i-1}⟩ →^ℓ ⟨(), Σ'_i⟩  for i = 0..N-1
        ────────────────────────────────────────────
        ⟨[e0, …], Σ⟩ →^ℓ ⟨temp o, Σ'_N[ init(o) := valid ]⟩

    [Array-Repeat]                                                   -- D-0072
        [e; N] : array<τ,N>   (N an integer literal);   ⟨e, Σ⟩ →* ⟨v, Σ1⟩ by [Read]
        τ plain: ¬is-resource(τ), and τ holds no reference, slice or function value
        ────────────────────────────────────────────
        ⟨[e; N], Σ⟩ →^ℓ ⟨temp o, …⟩   as [Array-Construct] with r_i = v for every i

    [Array-Repeat-Not-Plain]   disposition: rejected   τ not plain   ill-formed; diag.type-mismatch
                                                       (a resource `e` in place position: diag.read-of-resource)

`e` is evaluated once; its value is copied into every element, which is
why its type must be plain: a copy of a resource or an exclusive
reference would be a second owner or a second exclusive path.

A `byte-literal` `b"…"` (`spec/22` §1) whose decoded bytes are
`b0..b_{N-1}` denotes exactly `[b0:u8, …, b_{N-1}:u8]` — this rule on
`N` suffixed `u8` literals, type `array<u8, N>`; it is concrete syntax
only and introduces no separate rule or value.

**Depends on:** rule.value-object.object-establish,
rule.value-object.store, D-0019

### `rule.agg.index`
**Status:** ACCEPTED

    [Index-Checked]
        ⟨e, Σ⟩ →* ⟨place a0, Σ1⟩ (or via [Field-Access-Auto-Deref]/[Temp-Root]);  type(a0,Σ1) = array<τ,N>
        ⟨i, Σ1⟩ →* ⟨n, Σ2⟩;  n : usize;  0 ≤ n < N
        temporally-valid(a0, Σ2)
        ────────────────────────────────────────────
        ⟨e[i], Σ⟩ →^ℓ ⟨place a, Σ2[ access-paths(a) := as [Field-Access] with target sub-range(…·n), type τ ]⟩
        side-conditions:
            ⟦ 0 ≤ n < N ⟧ discharge: dynamic (static, disposition: rejected, when n is a literal —
                                             rule.control.flow-analysis)

    [Index-Out-Of-Bounds]   disposition: checked   ¬(0 ≤ n < N)   ⟨e[i], Σ⟩ ↛ diag.index-out-of-bounds

`inv.spatial-validity`'s only dynamic check: the bound guarantees
`target ⊆ extent`.

**Depends on:** inv.spatial-validity, type.usize, rule.agg.layout,
rule.control.flow-analysis, D-0018

## 3a. Slices (D-0047)

### `type.slice`
**Status:** ACCEPTED

`slice<τ, m>` (`m` shared or exclusive) is a borrowed run of consecutive
elements of an array, a `Vec<τ>` (`spec/21` §1) or another slice. It is
a borrow, as `ref<τ', m>` is: it is not a resource, a shared one is
copied, and it holds its **source** — the array, `Vec` or underlying
source of a slice — borrowed in mode `m` for as long as it lives
(`rule.alias.*`). Its value is that borrow, the position where the run
starts in the source, and the run's length. `sizeof(slice<τ, m>)` is
three words (impl-defined, as `[Sizeof-Str]`).

### `rule.agg.slice`
**Status:** ACCEPTED

    [Slice-Form]
        ⟨e, Σ⟩ →* ⟨place a0, Σ1⟩ in place position, or a0 through a reference (auto-deref);
        type(a0) = array<τ,N> (len = N) | Vec<τ> (len = its length) | slice<τ,m'> (len = its length; m ≠ exclusive if m' = shared)
        ⟨lo, Σ1⟩ →* ⟨i, Σ2⟩ and ⟨hi, Σ2⟩ →* ⟨j, Σ3⟩ with `$` = len (rule.agg.dollar);   0 ≤ i ≤ j ≤ len
        the source s: a0 itself, or a0's own source for a slice (the run offset by a0's start);
        r = the elements start(a0) + i up to start(a0) + j of s (D-0070);
        ¬clash(r, m, Σ3) with the borrows `e` was reached through as ancestors
        ────────────────────────────────────────────
        ⟨&m e[lo .. hi], Σ⟩ →^ℓ ⟨slice⟨borrow of r in mode m, start(a0) + i, j − i⟩ : slice<τ, m>, Σ3 + that borrow⟩

    [Slice-Index]
        ⟨e, Σ⟩ →* ⟨v : slice<τ,m>, Σ1⟩ (or a place holding one, or a reference to one);  ⟨k, Σ1⟩ →* ⟨n, Σ2⟩ with `$` = v's length
        0 ≤ n < length(v);  v's borrow is live
        ────────────────────────────────────────────
        ⟨e[k], Σ⟩ →^ℓ ⟨place: element start(v) + n of v's source, reached through v's borrow, Σ2⟩

    [Slice-Out-Of-Bounds]   disposition: checked   ¬(0 ≤ i ≤ j ≤ len), or ¬(0 ≤ n < length)
        ⟨…⟩ ↛ diag.index-out-of-bounds     (static, disposition: rejected, when every bound is a literal and len is an array's N)

    [Slice-Not-Borrowed]   disposition: rejected
        `e[lo .. hi]` not the operand of `&` or `&mut`
        ────────────────────────────────────────────
        diag.syntax-error (static): a slice is a borrow

A slice borrows its **range** of elements, not its whole source
(D-0070): two slices of one array or `Vec` conflict only where their
ranges meet, so `&mut v[0 .. m]` and `&mut v[m .. $]` live together
(divide and conquer on one buffer, one half per thread), while anything
that reaches the whole source — `&mut v`, `Vec::push`, a read of `v`
whole — conflicts with every live slice of it. An empty range meets
nothing.

`slice_len(s) : usize` (an intrinsic, `spec/21` §0, as `str_len`) is a
slice's length. A slice's elements are read and written through it as
an array's are (`[Index-Checked]`'s place, reached through the slice's
borrow): through a shared slice only read (`diag.write-through-shared`),
and a resource element is not moved out (`diag.move-out-of-field`). A
function returning a slice counts a slice parameter as a reference
parameter (`rule.temporal.elision`), and a slice of a local does not
escape its scope (`diag.reference-escapes-scope`). A slice is otherwise an ordinary
value: it is stored in a variable, a struct field or a `Vec`, passed,
returned and given to a generic `T`. There is no `==` on slices
(`diag.type-mismatch`), and no implicit conversion to one: an array or
`Vec` is passed as a slice by writing `&a[0..$]`.

### `rule.agg.index-vec`
**Status:** ACCEPTED

    [Index-Vec]
        type(e) = Vec<τ> (a place, or through a reference)
        ────────────────────────────────────────────
        e[k] ≡ *Vec::index_exclusive(&mut e, k)   where the place is written: an assignment's target, `&mut`'s operand
        e[k] ≡ *Vec::index_shared(&e, k)          elsewhere
        with `$` = Vec::len(&e) inside the brackets;
        the borrow of e so formed is checked as an access to e's element k (D-0070): a live
        slice of e whose range holds k conflicts with it, one of other elements does not

### `rule.agg.dollar`
**Status:** ACCEPTED

`$` is written only inside the brackets of an index `e[k]` or a slice
`&e[lo .. hi]`, and is the length of `e` — the innermost such brackets'
— taken after `e` is evaluated and before the bracketed expressions
are: `N` for an array, the length of a `Vec` or a slice. It is a
`usize`, so `a[$ − 1]` of an empty `a` is `diag.arith-overflow`, not an
index. Outside brackets it is a syntax error (`diag.syntax-error`).

On a `String` or a `StringView`, `&base[lo .. hi]` makes a `StringView` instead (D-0053, `spec/21` §2h `[View-Form]`).

**Depends on:** rule.agg.index, rule.stdlib.vec, rule.alias.borrow,
rule.temporal.elision, D-0047

## 4. Enum and `match`

### `type.enum`
**Status:** ACCEPTED

`enum Name<T…> { V1(τ1), …, Vk(τk) }`; a payload-less `Vi` is
`Vi(unit)`. Values `Vi(v)`. Variants are numbered from 0 in
declaration order (`spec/06` §7). `is-resource` per
`rule.type.is-resource`. `discriminant(o, Σ)` reads the `DW`-cell
unsigned image at `extent(o)`'s start.

### `rule.agg.enum-construct`
**Status:** ACCEPTED

    [Enum-Construct]
        Vi(e) : Name<σ>;  ⟨e, Σ⟩ →* ⟨r, Σ1⟩, e in place position iff is-resource(τi)
        ⟨establish(Name<σ>), Σ1⟩ →^ℓ ⟨o, Σ2⟩
        ⟨store(sub(o, Vi, τi), r), Σ2⟩ →^ℓ ⟨(), Σ3⟩
        ────────────────────────────────────────────
        ⟨Vi(e), Σ⟩ →^ℓ ⟨temp o, Σ3[ storage(discriminant cells of o) := DW-cell image of i, init(o) := valid ]⟩

    [Enum-Construct-Unit]   ⟨Vi, Σ⟩ → ⟨Vi(()), Σ⟩

**Codes** (D-0106). `enum Name : τ { V1 = c1, V2, … }`, τ an integer
type, gives each variant a code of type τ: the one written, or one more
than the previous variant's (0 for the first). Such an enum has no type
parameters and no payloads, its codes are distinct and each fits τ, and
it is declared at module level. It gains two associated functions,
exported when the enum is:

    [Enum-Code]        Name::code(Name v) : τ               -- ≡ match (v) { Name::V1 : c1, … }
    [Enum-From-Code]   Name::from_code(τ c) : Option<Name>  -- ≡ match (c) { c1 : Some(Name::V1), …, _ : None }

    [Enum-Code-Invalid]   disposition: rejected
        a code on an enum without `: τ`; τ not an integer type; type parameters or a payload;
        two variants with one code; a code outside τ (diag.literal-out-of-range)
        ────────────────────────────────────────────
        ill-formed; diag.syntax-error

The code is not the variant's discriminant (`rule.agg.layout` is
unchanged) and there is no implicit conversion either way (D-0006).

**Depends on:** rule.value-object.object-establish,
rule.value-object.store, rule.agg.layout, D-0019, D-0106

### `rule.agg.match`
**Status:** ACCEPTED

    match ::= 'match' '(' expr ')' '{' (arm ',')+ '}';   arm ::= pat ':' expr
    pat   ::= '_' | x | lit | V | V '(' pat ')'           -- D-0056, D-0057, D-0058
    lit   ::= '-'? integer-literal | byte-char-literal | 'true' | 'false' | C
            | string-literal                                  -- text, by its bytes (D-0127)

    A pattern is a chain V1(V2(…Vk(q)…)) with q a binder x, `_`, a literal, or nothing (Vk alone);
    with k = 0 it is `_`, a binder or a literal, testing the scrutinee itself. An identifier (or a
    qualified path) in a pattern that names a variant of an enum is that variant; one that names a
    constant C (`rule.module.const`) of an integer type or `bool` is its value, a literal by name;
    any other identifier is a binder. The pattern matches a value v when v's variant is V1, its
    payload's variant is V2, …, down to Vk, and Vk's payload (v itself when k = 0) equals the
    literal if there is one (a text literal: the level's text — `str`, `String` or `StringView` —
    has exactly the literal's bytes, D-0127; the text is read where it is, never moved); `_` and a
    binder alone match every value. x binds Vk's payload (v itself when k = 0: moved out when it
    is a resource, as a payload is; through a reference, the reference). These forms are the whole
    pattern language (D-0057; text literals added by D-0127): there are no range, struct, slice,
    or-patterns or guards.

    [Pattern-Type]   disposition: rejected   (D-0056, D-0057)
        V1 is not a variant of the scrutinee's enum, or some Vj+1 is not a variant of the enum
        that is Vj's payload type, or that a reference payload `ref<E, m>` refers to
        ([Match-Through-Ref], D-0109; a payload whose type is not an enum or a reference to
        one, a type parameter included, has no variants to match); or a literal's level type τ
        (Vk's payload type, or the referent's when it is `ref<τ, m>`, k ≥ 1; or
        the scrutinee's when k = 0) is not an integer type, `bool` or a text type (`str`,
        `String`, `StringView`; D-0127), or the literal, typed with τ expected
        (rule.arith.literal), is not of type τ (a reference to an integer or to text is not one;
        a text literal is of every text type and of no other, an integer or `bool` literal of no
        text type; a constant must fold to a literal, `rule.module.const`, and keeps its declared
        type)
        ────────────────────────────────────────────
        ill-formed; diag.type-mismatch (a literal outside τ's range: diag.literal-out-of-range)

    [Match]
        ⟨e, Σ⟩ →* ⟨place a, Σ1⟩  (a temp scrutinee first gets a root path, [Temp-Root])
        o = of(a,Σ1);  temporally-valid(a,Σ1);  init-state(o,Σ1) = valid;  ¬clash(a, shared, Σ1)
        the arm taken is the first whose pattern matches of(a,Σ1) (D-0056); its chain V1…Vk,
        a_k = a's projection through k payloads (path(a)·V1·…·Vk), τ its type, x its binder (or _)
        f_arm fresh;  Σ1 := Σ1[ frame-stack(ℓ) := f_arm :: Σ1.frame-stack(ℓ) ]     -- the arm frame, pushed here
        binder:
          x = _ :                      Σ2 = Σ1
          ¬is-resource(τ):             ⟨store(binding(x), place a_k), Σ1⟩ →^ℓ ⟨(), Σ2⟩      -- bound in f_arm
          is-resource(τ):              base(a,Σ1) = None (a whole owned or temporary object — else [Match-Move-Through-Ref])
                                       ⟨relocate-out(o, a_k), Σ1⟩ →^ℓ ⟨temp o_x, Σ'⟩;
                                       ⟨store(binding(x), temp o_x), Σ'⟩ →^ℓ ⟨(), Σ2⟩      -- bound in f_arm
        consume: if x ≠ _ ∧ is-resource(τ) (the payload was relocated out):  ⟨destroy(a), Σ2⟩ →^ℓ ⟨(), Σ3⟩
                 (each variant has one payload, so nothing else of o owns anything)
                 (what is left of o; solitary(a) as for any move, [Authority-Transfer-Aliased])   else Σ3 = Σ2
                 -- D-0049: a scrutinee no arm moves out of stays with its owner (a temporary one
                 -- ends with its statement, [Stmt-Exit]); `match (o) { None : …, _ : … }` leaves `o` usable
        ────────────────────────────────────────────
        ⟨match (e) {…}, Σ⟩ →^ℓ ⟨block'(f_arm, stmt-scoped(e_i, keep)), Σ3⟩
            -- the arm body runs as the block whose frame is f_arm (spec/14 [Block-Step]/[Block-Exit]); no second
            -- [Block-Enter] occurs, so `x` and the body share one frame, which [Block-Exit] ends

    [Match-By-Ref]   (D-0046)
        ⟨e, Σ⟩ →* ⟨v, Σ1⟩ with v : ref<E, m> for an enum E, v's path a (`match (&x)`, `match (&mut x)`,
        or any expression of reference type);  temporally-valid(a,Σ1)
        the arm taken and a_k, τ as in [Match], on the referent;  f_arm pushed as in [Match]
        binder:
          x = _ :                      Σ2 = Σ1
          otherwise:                   ¬clash(a_k, m, Σ1) with v's token as ancestor;
                                       ⟨store(binding(x), &m a_k), Σ1⟩ →^ℓ ⟨(), Σ2⟩   -- x : ref<τ, m>, derived from v
        nothing is consumed
        ────────────────────────────────────────────
        ⟨match (e) {…}, Σ⟩ →^ℓ ⟨block'(f_arm, stmt-scoped(e_i, keep)), Σ2⟩

    [Match-Through-Ref]   (D-0109)
        as [Match] or [Match-By-Ref], with some level j ≥ 2 of the arm's chain (or the literal
        below Vk) reached through a payload v_j : ref<E_j, m_j>: the chain continues at v_j's
        referent, temporally-valid(v_j's path) (else diag.stale-binding), and a_k is a path in
        that referent
        binder:
          x = _ :                      Σ2 = Σ1
          otherwise:                   m = shared if any m_j (or [Match-By-Ref]'s m) is shared, else exclusive;
                                       ¬clash(a_k, m, Σ1) with the last v_j's token as ancestor;
                                       ⟨store(binding(x), &m a_k), Σ1⟩ →^ℓ ⟨(), Σ2⟩   -- x : ref<τ, m>, derived from v_j
        nothing is moved or consumed
        ────────────────────────────────────────────
        ⟨match (e) {…}, Σ⟩ →^ℓ ⟨block'(f_arm, stmt-scoped(e_i, keep)), Σ2⟩
        -- `match (HashMap::get(&m, &k)) { Some(Num(v)) : …, … }`: `v : ref<i64, shared>`, and
        -- the map stays borrowed while `v` lives (D-0088). Coverage (`[Match-Non-Exhaustive]`)
        -- splits the referent's variants.

    [Match-Move-Through-Ref]   disposition: rejected
        the scrutinee is not a reference, an arm's binder x ≠ _ with is-resource(τ) ∧ base(a,Σ1) ≠ None
        (matching a resource payload by value through `*r` or a projection)
        ────────────────────────────────────────────
        ill-formed; diag.move-out-of-field

    [Match-Non-Exhaustive]   disposition: rejected
        some value of the scrutinee's type matches no arm's pattern (decided from the declared
        variant sets, level by level: a chain P is covered when an arm's chain is a prefix of P,
        or the level after P is an enum every variant W of which extends P to a covered chain
        P·W, or `bool` with P·true and P·false both covered; an integer or text level is covered
        only by an arm whose chain is a prefix of P — `_` or a binder — never by literals)
        ────────────────────────────────────────────
        ill-formed; diag.non-exhaustive-match, with a pattern not covered as its message

    [Match-Unreachable]   disposition: rejected   (D-0056)
        every value arm k's pattern matches is matched by an arm before it (arm k's chain is
        covered, as above, by the earlier arms' chains)
        ────────────────────────────────────────────
        ill-formed; diag.unreachable-arm, at arm k

    [Match-Conflict]   disposition: checked   clash(a, shared, Σ1)   ⟨match (e) {…}, Σ⟩ ↛ diag.aliasing-conflict

`match` tries the arms in order and takes the first whose pattern
matches (D-0056). A pattern names a variant and, in parentheses, what
its payload must be: a binder, `_`, or another pattern, so
`Ok(Some(line))`, `Ok(None)` and `Err(Utf8(e))` test two levels at once.
Since a variant has one payload, a pattern is a chain with at most one
binder, which binds the innermost payload it reaches. A chain can end in
a literal (D-0057) — `Ok(0)`, `Some(b'\n')`, `Some(true)` — and a
`match` on an integer or `bool` is a list of literal arms and `_`: the
checked multi-way branch CobaltC has in place of `switch`, where a
duplicated case is an unreachable arm. A `match` on text — a `str`, a
`String` or a `StringView`, or `*r` through a reference to one — is the
same with text literals (D-0127): `"get" : …`, `Some("quit")`; the text
is compared byte for byte where it is, so a `String` scrutinee stays in
place unless a binder arm takes it whole, and `_` is needed as for an
integer. A named constant stands where a
literal does (`PUSH : …`, D-0058); since a name that is a constant is
not a binder, mistaking one for the other shows as a missing `_`. A
binder alone (`n : …`, `other : …`) takes the whole value, as the last
arm usually does. An arm no value
reaches is rejected, as is a set of arms some value escapes; the
diagnostic names such a value's pattern (`Ok(None)`).

`match` reads the discriminant, binds the matched payload — by copy
for a plain payload, by moving it out into a fresh object for a
resource payload — and evaluates the arm in a fresh frame that owns
`x`. An arm that binds a resource payload by value **consumes** the
enum: the payload is moved out and the enum object destroyed (its
remaining obligations are none), so the scrutinee binding is invalid
afterwards exactly as after a move. An arm that moves nothing out —
`_`, a payload-less variant, `Vi(_)`, or a plain payload copied —
leaves the scrutinee where it was (D-0049): `match (o) { None : …, _ :
o }` may still use `o`, and a temporary scrutinee ends with its
statement as any temporary does. To inspect an enum without consuming it,
match a reference to it (`[Match-By-Ref]`, D-0046): `match (&e)` binds
each payload as a `ref<τi, shared>`, `match (&mut e)` as a
`ref<τi, exclusive>` through which the payload can be changed, and a
scrutinee that already is a reference binds in its own mode. The form
is written, as `foreach (x in &c)`'s is: `match (e)` and `match (*r)`
bind by value as before, so a resource payload reached by value through
a reference is still `[Match-Move-Through-Ref]`. All of this holds at
any depth of a nested pattern: binding a resource payload however deep
consumes the scrutinee (its other levels own nothing else), and through
a reference every binder is a reference. Literal and struct patterns are
not provided (`spec/22` §5).
Exhaustiveness is decidable from the declared variant set (D-0006).

**Depends on:** rule.agg.field-access, rule.resauth.relocate-out,
rule.resauth.destroy, rule.value-object.store, rule.control.block,
D-0006, D-0018, D-0019, D-0056, D-0057, D-0058

## 4a. Derived `clone` (D-0116)

### `rule.agg.derived-clone`
**Status:** ACCEPTED

For every struct or enum `N` declared without `resource` and without
its own `N::clone`, whose fields (payloads) are not all copied by a
read, the program has, as if written beside it in `N`'s module:

    fn N::clone<P1: clone, …, Pk: clone>(ref<N<P1..Pk>, shared> self) : N<P1..Pk>
    {
        N { .f1 = clone(&self.f1), …, .fn = clone(&self.fn) }              -- a struct
    }
    fn N::clone<…>(ref<N<…>, shared> self) : N<…>
    {
        match (self) { V1(p) : N::V1(clone(p)), V2 : N::V2, … }             -- an enum; `p` a reference to the payload
    }

    [Derived-Clone]
        clone-holds(N<τ1..τk>) by its fields (spec/12 rule.type.bound)
        ────────────────────────────────────────────
        clone(r) for r : ref<N<τ1..τk>, m>  ≡  N::clone(r) at τ1..τk, the function above

The derived function is checked and run as any function: each field
is copied by `clone`, in declaration order, into a fresh `N`; a
resource field is copied by its own type's `clone`, never moved. Its
type parameters carry `clone`, so `Pair<String, File>` is not `clone`
(`File` has none), and neither is a struct with such a field: the
rejection is at the use, naming the field. A struct declared inside a
function (D-0069) derives nothing; it declares its own `clone` if it
needs one. A type marked `resource` derives nothing either: its
destructor says it manages something beyond its fields, which only its
own `clone` can copy.

**Depends on:** rule.type.bound, rule.agg.struct-construct, rule.agg.match, D-0116

## 5. Generic declarations

### `rule.agg.generic-decl`
**Status:** ACCEPTED

`struct Name<T1..Tn>{…}` / `enum Name<T1..Tn>{…}` have kind
`Type^n → Type` (`rule.type.kind`); `Name<σ1..σn>` substitutes and
every rule above applies to the result as to a concrete declaration
(D-0010). Type arguments may be omitted where the expected type fixes
them (`rule.type.expected`; D-0013/D-0014): `Box { .value = Vec::new() }`
against expected `Box<Vec<i32>>` infers both. `Option<T>` and
`Result<T,E>` are ordinary library enums (`spec/21` §0).

**Depends on:** D-0010, D-0013, D-0014, rule.type.kind,
rule.type.expected

## 6. Composite destruction

Handled entirely by `rule.resauth.destroy` → `destroy-composite`
(`spec/07` §3–§4): every resource-bearing field, element, or live
payload is registered as `(o, path)` at construction
(`[Store-Sub-Relocate]`) and destroyed innermost-first before `o`
ends. No aggregate-specific rule exists.

## Change Log

- 1.22.0 — `CHG-0155` (D-0132): §2a: a `bitstruct` may be declared in a
  block, as a local type, with its derived functions.
- 1.21.0 — `CHG-0150` (D-0127): `rule.agg.match` takes text literals (`string-literal`) against `str`, `String` and `StringView` levels.
- 1.20.0 — `CHG-0149` (D-0126): `type.array`'s `N` is a constant expression.
- 1.19.0 — `CHG-0141` (D-0118): §2a `type.bitstruct`, `[Repr-Bitstruct]`,
  `[Bitfield-Read]`, `[Bitfield-Write]`, `[Bitfield-Overflow]`,
  `[Bitfield-No-Place]`, `[Bitstruct-Whole]`, `bits`/`from_bits`.
- 1.18.0 — `CHG-0139` (D-0116): `rule.agg.derived-clone`.
- 1.17.0 — `CHG-0128` (D-0109): `[Match-Through-Ref]`; `[Pattern-Type]`
  descends through a reference to an enum.
- 1.16.0 — `CHG-0125` (D-0106): enum codes, `enum Name : τ { V = c }`,
  `Name::code`, `Name::from_code`.
- 1.15.0 — `CHG-0122` (D-0103): `[Temp-Field-Move]`; a field of a
  temporary as a borrowed argument.
- 1.14.0 — `CHG-0085` (D-0072): `[Array-Repeat]`, `[v; N]`.
- 1.13.1 — Non-normative (`CHG-0082`): a syntax error is named
  `diag.syntax-error`.

- 1.13.0 — `CHG-0080` (D-0070): a slice borrows its range (`[Slice-Form]`);
  `[Index-Vec]`'s borrow is checked at the element.

- 1.12.0 — `CHG-0074` (D-0064): `[Guard-Auto-Deref]`: `g.f` and `g[i]`
  through a guard.

- 1.11.0 — `CHG-0066` (D-0058): a binder alone at the top of a pattern
  (the whole value); a constant as a literal; `pat`'s grammar folded.

- 1.10.0 — `CHG-0065` (D-0057): literal patterns (`lit`), a `match` on an
  integer or `bool`, their typing in `[Pattern-Type]` and coverage in
  `[Match-Non-Exhaustive]`; the pattern language is closed at four forms.

- 1.9.0 — `CHG-0064` (D-0056): `rule.agg.match` gains nested patterns
  (`[Pattern-Type]`, arms in order, `[Match-Unreachable]`, the binder at
  any depth, `[Match-Non-Exhaustive]` over chains).

- 1.8.0 — `CHG-0061` (D-0053): `rule.agg.slice` points to `[View-Form]` for a `String` or `StringView` base.

- 1.7.0 — `CHG-0057` (D-0049): `[Match]` consumes the scrutinee only in an
  arm that moves a resource payload out.
- 1.6.0 — `CHG-0055` (D-0047): §3a slices (`type.slice`,
  `rule.agg.slice`), `[Index-Vec]`, `$` (`rule.agg.dollar`).

- 1.5.0 — `CHG-0054` (D-0046): `[Match-By-Ref]` in `rule.agg.match`;
  `[Match-Move-Through-Ref]` applies to a by-value match only.

- 1.4.0 — `CHG-0053` (D-0045): `[Type-Recursive]` in `rule.agg.layout`.

- 1.3.1 — Non-normative (`CHG-0025`): `rule.agg.array-construct`
  notes that `spec/22`'s `byte-literal` is sugar for `[Array-Construct]`
  on suffixed `u8` literals. No rule changed.
- 1.3.0 — `CHG-0011`: `[Match]` states where the arm frame is pushed
  and that the arm body reduces as `block'(f_arm, …)` directly. The
  1.2.1 text said "in a fresh arm frame" and "a block whose frame is
  the arm frame" without saying which rule pushed it, leaving
  `rule.control.unwind`'s fold through a `return` inside an arm
  ambiguous (found deriving `ex.e2e-propagate-chain`).
- 1.2.1 — Non-normative (consistency pass, `CHG-0009` §"Hygiene"):
  `type.struct`'s declaration shape, `[Struct-Construct]`'s literal
  shape, `rule.agg.generic-decl`'s example, and one `let` mention
  re-spelled to the current surface syntax (missed by `CHG-0001`); a
  citation of `[Ref-Form-Not-Place]` where `[Ref-Form-Temporary]` was
  meant corrected.
- 1.2.0 — `CHG-0005`: `match`'s grammar illustration and `[Match]`'s
  arm-shape prose re-spelled with `:` instead of `=>`, matching
  `spec/22` 2.4.0. No rule semantics changed.
- 1.1.0 — `CHG-0003`: `match`'s grammar illustration and `[Match]`/
  `[Match-Conflict]`'s reduction subjects restated with mandatory
  parentheses around the scrutinee (`match (e) {…}`), matching
  `spec/22` 2.2.0. No rule semantics changed.
- 1.0.0 — Rewritten per D-0018/D-0019 (`spec/AUDIT-2.md` B-01, B-04,
  B-05, B-07, B-17): construction yields temporaries via `store`;
  projections set every record field with `base := a0` and no
  formation check; `[Temp-Root]`; `[Match]` stated over `Σ` with
  copy/move-out binding, arm frames, consumption of resource enums,
  and `[Match-Move-Through-Ref]`; `[Layout-Enum]` and `sub-range`
  defined; `[Enum-Destroy]` subsumed by `destroy-composite`. All
  entities `ACCEPTED`.
- 0.9.0 and earlier — superseded.
