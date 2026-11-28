# D-0326 — The toolchain without Rust, built on a new machine with a C compiler alone

Status: ACCEPTED (2026-11-15, the owner: "implement /home/…/private/RUSTFREE-PLAN-2026-11-15.md in full,
autonomously, using all your recommendations"; Q2 decided by the owner earlier the same day: "I agree, rewrite
Q2 and phase E that way"); implemented by `CHG-0457` onward
Kind: Decision (`spec/02-schema.md`); the implementations, not the language
Governed by: `CobaltC_Master_Instructions.md` §9
Depends on: D-0274 (the bootstrap form), D-0276 (`cbc` replaces `cobc`), D-0278 (which toolchain leads; its
option 2, retiring `cobc`, and its questions 2 and 3 left open), D-0279 (no untracked build), D-0296 (both
toolchains kept in step), D-0255 (the Windows target)
Affects: everything under `impl/`; `README.md`; `impl/STATUS.md`; `spec/ASSURANCE.md`; the guide; the book;
`CARD.md`; `bench/`. No rule of the language changes, and the edition changes only as the records do.

## 1. Problem

The toolchain exists twice. The Rust one (`coby`, `cobc`, `cbrt`, `cobfmt`, `cobmodel`) is the reference; the
CobaltC one (`cbc`, `cbparse`, `cobfmt-cb`, in `impl/selfhost/`) writes the same C byte for byte and compiles
itself to a fixed point. Every change to the static pass or the lowering is written in both (D-0296). Building
CobaltC needs Rust, and on Windows 7 a Rust pinned at 1.77.2. A newcomer meets two ways to run a program,
`coby` and `cobc`, and is not told which to use.

The owner's goal (2026-11-15): every tool written in CobaltC, nothing in the repository written in Rust, and
CobaltC built on a new PC with no Rust installed.

## 2. Decision

The plan of 2026-11-15 (kept privately; its decisions are these) is carried out in seven phases, A to G, each
ending in a gate and a push, each Rust part removed only after its replacement agrees with it.

1. **The runtime is C.** `cbrt` is ported to GNU C11, function for function, behind the unchanged `cbrt.h`, so
   the generated C does not change. CobaltC lacks what a runtime needs (mutable statics, thread-local storage,
   a lowering that calls no runtime: D-0256 parked a freestanding mode for the same reasons); C is what CobaltC
   compiles to, so it is not a second toolchain. "Self-hosting in full" means every tool is CobaltC; the
   runtime, the tools' host shims and the bootstrap driver are C.
2. **The interpreter is retired, not ported** (the owner's decision). No user needs it: `cobc` checks, runs,
   tests, explores schedules and prints documentation. It leaves what a newcomer reads at once; it stays the
   suites' internal checker while the runtime is ported and the Rust compiler goes; then every case's output,
   on which it and the compiler agree, is recorded beside the case, and it is removed.
3. **The bootstrap seed is `cbc`'s own C**, in the tree, compressed, without `#line` directives: the C the
   compiler's source in the same commit produces. One seed serves Linux and Windows.
4. **The first build is one C program**: `gcc impl/bootstrap/boot.c -o boot && ./boot` builds the runtime,
   unpacks the seed, compiles it, and proves the result by having the new compiler write its own C again,
   which must be the seed byte for byte.
5. **Names.** The CobaltC tools take the names everything already uses: `cobc`, `cobfmt`, `cobmodel`, in
   `impl/target/release/`. `cbc`, `cbparse` and `cobfmt-cb` end as names.
6. **Tests** become files with headers, as the conformance suite's are, run by a runner written in CobaltC; the
   gate stays Python and stays the orchestrator.
7. **`cobmodel` is ported to CobaltC**, importing `std` only, never the tools' front end or code generator, so
   that it still shares no code with them.
8. **The Rust sources leave `main`.** The last tree that has them is kept as one ref, `rust-reference`, never
   updated: the historical oracle for a suspected miscompile.
9. **`cbc`'s speed against `cobc` is not a gate** for this work; it is measured again after the C runtime.
10. **A fresh-clone build on every push**, on GitHub's Linux and Windows runners, as well as in clean containers
    on the development machine.
11. **`bench/` keeps its comparison with checked Rust**: `rustc` is needed only to re-measure that column,
    never to build, test or gate CobaltC.
12. **One D-record, this one**; one change record for each piece of work, `CHG-0457` onward.

## 3. What replaces the two-implementation checks

Two kinds of evidence go: two toolchains written separately that must agree byte for byte, and two
implementations of the run-time rules that must print the same. They are replaced by: the fixed point and the
seed; the compiler built with every check against the default build; the C compilers against each other; every
case's recorded output; the compiler against itself, in its forms, on random programs; the conformance headers
and the specification's rows; `cobmodel`; the Python readings (`factcheck.py`, `validate_c.py`), the
certificates and the replays against the Lean machine. What none of these gives, and is accepted: a second
opinion on a new feature's output outside the model's fragment, and a second front end.

## 4. Consequences

- From the end of phase E a change to the language is written once.
- Until a Rust part is removed, D-0296 holds for it.
- D-0278's option 2 and questions 2 and 3 are answered: `cobc` (Rust) is retired; the seed is the compiler's own
  C, in the tree; `cbparse` ends with the comparison it existed for.

## 5. Amended by the owner, 2026-11-15

- **Three Updates, not one for each phase**: Update 192 is phase A; Update 193 is phases B and C (the runtime
  in C, the seed and the first build: a new PC builds the compiler with a C compiler alone); Update 194 is
  phases D to G (the tests off Cargo, the outputs recorded, the interpreter, the Rust compiler and the Rust
  model removed, the cut-over). The rule that a Rust part goes only after its checks, against it as the
  oracle, is unchanged.
- **The Windows checkpoints are GitHub's Windows runners** (`.github/workflows/bootstrap.yml`: a fresh clone,
  `boot.exe` with MinGW-w64's GCC from a command prompt, no Rust on `PATH`, then the conformance suite), in
  place of the owner's runs on the Windows 7 machine. A runner is Windows Server, not Windows 7: the runtime
  keeps to the Windows calls Windows 7 has (`_WIN32_WINNT 0x0601`), which the runner cannot show.

## 6. Done (2026-11-16)

Update 192 (phase A, `CHG-0457`), Update 193 (phases B and C, `CHG-0458`) and Update 194 (phases D to G,
`CHG-0459`). No tool is Rust, nor anything that builds, tests or gates CobaltC; the only Rust left is `bench/`'s
comparison programs, which item 11 keeps as a measurement. A new machine builds CobaltC with a C compiler alone
(`impl/bootstrap/boot.c`), checked on every push on GitHub's Linux and Windows runners. The last tree with the
Rust tools is the tag `rust-reference`.

## 7. Amended by D-0328, 2026-11-17

Item 8's second sentence no longer holds: the tag `rust-reference` was removed from the published repository
(the owner: "not needed"), and no published ref holds the Rust tools.
