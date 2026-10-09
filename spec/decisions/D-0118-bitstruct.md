# D-0118 — `bitstruct`: named bit fields of one integer

Status: ACCEPTED (2026-09-30, the owner: "i think I want bitstruct type as well"; the design decisions delegated: "I delegate these decisions to you, but let me know what you decide before proceeding" — decided and reported the same day; syntax the owner's, `bitstruct Name : u32 { … }`)
Kind: Decision (`spec/02-schema.md`)
Governed by: `CobaltC_Master_Instructions.md` §1, §4, §5, §9, §18
Depends on: rule.agg.struct-construct, rule.agg.field-access, rule.agg.layout, rule.arith.convert (`[Narrow-Overflow]`), rule.type.is-resource, D-0110 (key types), D-0116 (derived functions by the front end), D-0119 (byte order lives in `push_le`/`push_be`)
Affects: `spec/16` §2a (1.19.0), `spec/22` §1 and §3 (2.41.0), `spec/registry/diagnostics.md` (1.41.0), `spec/conformance.md`, the guide §16 and §22, the shared front end, both tools

## Problem

A value made of fields at fixed bit positions — a float's sign,
exponent and mantissa; a bytecode instruction's opcode and operands; a
UTF-8 lead byte; a device register — is taken apart and put together
by hand: `(b >> 52) & 2047`, `(c & 0x3F) << 6 | …`. The field's
position and width live only in the programmer's head, a wrong shift
or mask is silent, and each program repeats the arithmetic per width.
The corpus does this in float anatomy, UTF-8 encoders and decoders, a
VM's instruction words, LEB128, Q15 audio and PNG headers. This is the
Master Instructions' §4 test exactly: a fact correct code depends on,
represented nowhere.

## Candidate mechanisms

1. **Leave it to shifts and masks**, with D-0119's byte functions for
   the byte order. No new feature; every program keeps its own.
2. **C bit fields** (`unsigned op : 6;` inside a struct). Their layout
   is implementation-defined in C; a language whose point is a fixed
   representation cannot inherit that.
3. **A `bitstruct` declaration** naming the backing integer and each
   field's width, with the layout fixed by the language, reads and
   writes as ordinary field access, every write checked against the
   width, and two functions to and from the integer. Selected.

## Selected design (the decisions delegated by the owner)

    bitstruct Instr : u32 { op : 6; rd : 5; rs : 5; imm : 16; }

1. **A write that does not fit faults**, `diag.narrowing-overflow`
   (`[Bitfield-Overflow]`), refuted statically for a literal or a
   literal-bound value; never truncation.
2. **Low bit first**, in declaration order: the first field is bits
   `0..w1`. Byte order is not the type's concern (D-0119).
3. **Fields are unsigned**, each the smallest unsigned type holding
   its width. No signed, `bool` or enum fields in this decision; each
   is an extension that changes nothing decided here.
4. **The widths fill the backing type exactly**; unused bits are a
   named field. A declaration that does not fill it is a syntax error
   naming the shortfall. So `bits` is total and `from_bits` is total.
5. **Backing type: `u8` to `u128`.**
6. **Forms.** Item level only, `export` as a struct, not generic, never
   `resource`. Construction by a literal naming every field;
   `Name::bits(v) : uN` and `Name::from_bits(uN) : Name`, present as if
   declared beside the type (the front end derives them, as it derives
   `clone`); `x.f` reads and `x.f = v` writes under the ordinary
   aliasing rules (a write is a write to the whole). Plain: copied by a
   read, `clone`, `eq` by its bits, a key type by its bits. `%v` is not
   defined for it; `Name::bits(v)` prints.
7. **No nesting, no arrays of fields, no patterns, no destructuring**
   (`[Bitstruct-Whole]`), **no borrowing a field** (`[Bitfield-No-Place]`:
   a run of bits has no address).
8. **Not an `FfiType`** — a refinement of what was first reported: a
   C call takes `Name::bits(v)` and gives back `Name::from_bits(x)`, so
   no C ABI question about a one-member struct arises. A `rawptr<Name>`
   reads and writes the integer's image (`[Repr-Bitstruct]`), which is
   what device memory needs once a volatile access exists.

Implementation: one keyword; a `StructDecl` with a backing type and
field widths, so name resolution, typing, field access, literals,
equality and keys are the struct's; `coby` keeps the fields unpacked
and packs them at every byte boundary (raw memory, keys); `cobc`
represents the value as the integer and lowers reads, writes and
literals to shifts and masks, never to C bit fields.

## Why it earns its place

Two facts, position and width, move from the programmer's head into
the declaration, and every write is checked (§4, §9). The
hand-written form stays available.

## Compatibility impact

Additive: `bitstruct` becomes a keyword (a program using it as a name
is rejected at the lexer; none in the repository did). Nothing that
ran changes meaning.

## Revisit conditions

Signed fields, `bool` fields, an enum with codes (D-0106) as a field
type, and a high-bit-first order for datasheet-shaped registers, each
as its own decision if a program needs it.
