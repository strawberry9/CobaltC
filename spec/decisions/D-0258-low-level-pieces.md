# D-0258 — The low-level pieces: assembly files, addresses, `static` atomics

Status: ACCEPTED (2026-10-17, the owner: "go with your recommendations"; part C implemented as `CHG-0317`,
part A as `CHG-0318`, part B as `CHG-0320`)
Kind: Decision (`spec/02-schema.md`)
Depends on: D-0121, D-0146, D-0249, D-0250, D-0251, D-0253, D-0255
Affects: part C — `impl/cobc/src/link.rs`, the guide, `CHG-0317`; part A — `spec/20` (`[Rawptr-Offset]`'s
side condition, two trusted intrinsics); part B — `spec/04`, `spec/10` (a program extent), `spec/19` (a
static reached by every thread), `spec/22` (`static`), both tools, `cobmodel`. The full text is
`private/D-0258-low-level-pieces-proposal.md`.

## Problem

Hardware-facing code (`CobaltC_Master_Instructions.md` §5) needs instructions C cannot express, a way to name
a device register's address, and global state that no `main` holds — a library (D-0257) has none. CobaltC
had none of the three: foreign code was `.c`, `.o`, `.a`, `.so` or a library name; a `rawptr` came only from
`rawptr_of`, `allocate` or C; `static` was a C-ism error, and D-0253 left it to a decision of its own.

## Options

A: two trusted intrinsics (chosen); extending `reinterpret` (blurs its same-width rule); only through C
helpers. B: `static` for atomics with a literal initializer plus addressable `const` (chosen); general
immutable statics (an indexed integer-array `const` is already one C table, so only its address was
missing); `static mut` inside `unsafe` (the language's one unchecked shared mutable state outside raw
pointers; declined). C: `.S`/`.s` files assembled by the C compiler (chosen); inline assembly (a sub-language
in the checker; declined).

## Decision

C: `extern "./x.S";` (preprocessed, so `#ifdef _WIN32` chooses the calling convention) and `extern "./x.s";`
are foreign code, assembled as a `.c` file is compiled; `coby` refuses the first call into them as for C.
A: `unsafe { rawptr_from_address<τ>(usize) }` and `unsafe { rawptr_address(p) }`; `[Rawptr-Offset]`'s side
condition admits a region the program obtained this way, its bounds the caller's (`discharge: trusted`);
`coby`, whose storage is simulated, makes such a pointer only from an address it gave out (so a round trip
through `rawptr_address` works there) and refuses any other at the call (`unsupported`). B: `static Atomic<T>
NAME = literal;` (later `Mutex<T>`), the literal the atomic's first value, `&NAME` a shared reference with
no end, each static registered with the tracker at start-up; `&CONST` of a `const` of plain data gives one
object with program extent in read-only data; no `static mut`, no static of plain data; `spec/19` says a
static atomic is reached by every thread, and the model is extended and compared. Order: C, A, then B with
D-0257.

## Results

C: `conf.extern-code-asm-file`, `cobc/tests/extern_code.rs` `assembly_files_are_assembled_and_linked` (`.S`
and `.s`, GCC and Clang), the case as a Windows executable under Wine. A: `conf.rawptr-address-roundtrip`,
`conf.rawptr-from-address-outside-unsafe`; an allocation's bytes written through a pointer made from its
address read back alike in `coby`, GCC and Clang; a device address refused by `coby`, compiled by `cobc`. B:
`conf.static-atomic` (workers and a waiter on statics, a module's static, `&NAME` held), five rejections,
`conf.const-address` (array, nested array, struct and number constants with one address, checked with
`rawptr_address`), `conf.const-exclusive-borrow`; `stress/round10/statics/` (four threads on a static
counter, `--explore 12` one outcome) under `coby`, GCC and Clang. As implemented: a static of a mutex is not
yet admitted; `coby` makes the static's object on its first use, `cobc` a C global with a token-0 reference
(no runtime registration was needed: no path ever writes the data). `cobmodel` gained `static Atomic<i32>`
(an object established in `Σ_0` with a shared root path, borrowed by name from closures with no capture; the
enumeration's alphabet gained its uses): hand-written programs and a random enumeration compared with both
tools, nothing outside the model's set.

## Revisit conditions

Inline assembly; `static Mutex<T>` once its start-up is defined; linker sections and interrupt entry points
(with D-0256).
