# D-0277 — No object cache: a program's C is one file, compiled every time

Status: ACCEPTED (2026-10-24, the owner's decision: "To be honest, this cache is always causing issues. I
wouldn't mind if we stop using it completely", choosing option 1 of three: remove it; fix its key and size;
keep it off by default)
Kind: Decision (`spec/02-schema.md`)
Governed by: `CobaltC_Master_Instructions.md` §9
Depends on: D-0276 (`cbc` replaces `cobc`), D-0274 (the bootstrap form)
Affects: `impl/cobc/src/cache.rs` (removed; its C compiler helpers kept in `cc.rs`), `impl/cobc/src/main.rs`,
`impl/cobc/src/lower.rs` and `lower/program.rs` (`Unit::cacheable` named `pure_std`),
`impl/cobc/tests/cache_identity.rs` (removed), `driver.rs`, `tracking.rs`; `impl/selfhost/cbc/cache.cb`
(removed; `cc.cb`), `driver.cb`, `main.cb`, `impl/selfhost/back/state.cb`, `gen.cb`, `passes.cb`;
`impl/tools/suites.sh`, `impl/fuzz/run.py`, `impl/STATUS.md`

## Problem

Since 2026-10-05 `cobc` (and, since D-0276, `cbc`) compiled each pure-`std` function — a `std` function at
`std` types only, whose C is the same in every program — in a translation unit of its own and kept the
object in `cobc-cache` beside the executable, keyed by the unit's text, the C compiler, its options, the
runtime's stamp and the compiler's own executable (size and modification time). The cache made a program
that reaches much of `std` compile several times faster (measured 2026-10-24: `dns_over_http.cb` 6.1 s with
a warm cache, 35.9 s without; `tinyos.cb` 4.3 s and 4.4 s). It also cost: a rebuilt compiler matched no
entry, so every development build of `cobc` started cold and left a full set of objects behind, never
pruned (the cache had grown to 1.2 million files, 7.7 GB); the inline-only copies of cached functions it
needed for speed had their own bug (`hands_runtime_an_address`); concurrent builds needed an atomic
placement protocol, and Windows a fallback for it (CHG-0340); and the mirror in `cbc` doubled all of it.

## Options

1. Remove the cache: the program's C — its own code and the `std` it reaches — is one file, compiled
   whole every time, as `COBC_CACHE=off` did.
2. Keep it, keyed by the runtime header's text instead of the compiler's executable, and with a size cap.
3. Keep it, off by default, on with `COBC_CACHE=DIR`.

## Decision

Option 1. `cobc` and `cbc` write the program's single C file (`Generated::combined`, what `--keep-c` kept
before) and compile it every time, a small program's with one C compiler invocation; `--lib` compiles it to
one object for the archive.
`COBC_CACHE` is gone. A large program's C is compiled in parts, all at once, then linked: one part a
megabyte of its functions' C, at most eight, so a program is always split alike (`cc::part_count`,
`Generated::parts`); every part has the declarations, a function given external linkage where it was
`static`, but for the `static inline` ones each part defines, statics and addressed constants defined in
part 0 and declared in the rest, thunks and descriptors each part's own, the program's tail in part 0.
Link-time optimisation lets the link inline across the parts: GCC's (`--version` names the Free Software
Foundation) above `-O0`, `-flto` per part and `-flto=auto` to link, or `-flto=auto` alone for a program in
one part; Clang's ThinLTO for a program in parts, through the first of the Clang installation's own `lld`
(the one `-print-prog-name=ld.lld` names, of Clang's major version) and `gold` that links a probe program
against the runtime; none for `--lib`, whose archive a C build without LTO must be able to link. Without
LTO a program in parts runs a little slower (`cbc`: 2-4%). The kept C (`--keep-c`, and `--emit-c`, which
`cobc` gains as `cbc` has it) is the one file, as before. The C itself is
unchanged, byte for byte: the units that were cacheable are still told apart (`Unit::pure_std`), since
`localize` leaves them external and `line_directives` gives them no `#line`; whether they too become
`static` is a change to the C of its own.

## Consequences

- Measured on four processors, `-O2` (build time, then the built `cbc`'s time on `tinyos.cb`): `cbc`'s own
  C, 92 MB, as one file 357 s with GCC, 351 s with Clang; GCC's LTO of one file 170 s; in four parts 147 s
  (GCC), 145 s (GCC, LTO), 102 s (Clang), 128 s (Clang, ThinLTO), the built `cbc` 8.9-9.3 s in every form,
  writing the same C. `dns_over_http.cb`'s C, 4.7 MB: 32.8 s as one file, 12.4 s in four parts with
  GCC's LTO, 12.6 s with Clang's ThinLTO (16.8 s with a warm cache before). `tinyos.cb`'s, 0.9 MB: one part,
  3.0 s.
- Clang's ThinLTO needs an `lld` of its own LLVM, or a `gold` plugin that reads the runtime: clang 14's
  cannot read the LLVM bitcode rustc's standard library leaves in `libcbrt.a` (LLVM 22), so on such a
  machine a Clang build is split without LTO.
- No cache directory beside the compilers (the one this machine had, 7.7 GB, was deleted).
- `cbc`'s C stays `cobc`'s, byte for byte (`compare_c.sh`), and the fixed point is unaffected.
