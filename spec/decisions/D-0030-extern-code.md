# D-0030 — Naming Foreign Code in the Program: `extern "…";`

Status: ACCEPTED (2026-09-25, owner-chosen)
Kind: Decision (`spec/02-schema.md`)
Governed by: `CobaltC_Master_Instructions.md` §1, §5, §17
Depends on: D-0017, D-0021, rule.trust.extern-call
Affects: rule.trust.extern-code (new), spec/22 §3 (`extern-code`), spec/22 §5

## Problem

An `extern fn` could call only what the implementation happened to link:
libc and libm. A program with its own C code, or using an installed C
library, had no way to say so. The build needed options outside the
program, which the compiler did not have.

## Constraints

- No new tokens.
- Linking stays outside the core semantics (`spec/22` §5): the program
  says *what* to link, each implementation says *how*.
- A program stays buildable with nothing but its entry file:
  `cobc prog.cb`.
- The interpreter need not link foreign code (the owner, 2026-09-25).

## Candidate mechanisms

1. **A bare `extern "…";` item**, anywhere an item may be. **Selected.**
2. **An `extern "…" { fn …; }` block** grouping the functions that come
   from that code.
3. **A new keyword**, e.g. `link "…";`.
4. **Compiler options only** (`-l`, `-L`, extra input files), no
   language change.

## Selected design

Candidate 1, with four choices made:

- **(a) The string names code, the declaration names nothing.** It adds
  no name to any module, takes no `export`, and changes no rule of name
  resolution, typing or evaluation. Declaring the same code twice, in
  one module or several, is not an error.
- **(b) Its meaning is implementation-defined and documented**, as
  calling convention and linking already are. The specification fixes
  the syntax and that the string names foreign code.
- **(c) `cobc`'s reading:** a string beginning `./`, `../` or `/` is a
  file, relative to the file that declares it (as `module m "./m.cb";`
  is): `.c` (compiled separately, with warnings shown), `.o`, `.a`, or
  `.so` (linked by absolute path, with an rpath, so the executable
  runs from any directory). Any other string is a library name, as
  `cc -l` takes it; a name that could read as another option is
  refused. Each file and name is linked once. A missing file or library
  is reported at its declaration.
- **(d) The interpreter accepts and ignores it.** A call of a function
  that only the named code provides stops as any unavailable `extern fn`
  does (`unsupported: …`, exit 3), and the message names the declared
  code, which only the compiler links.

## Rejected alternatives

- **2:** a second way to declare an `extern fn`, and the grouping is not
  checked: the linker sees one list of symbols, so a function "in" one
  block may come from another library.
- **3:** a new keyword for what `extern` plus a string already says.
- **4:** the program no longer builds from its own source; it needs its
  build command beside it. Options can still be added later for build
  systems that pass them.
- **Only file paths, no library names:** an installed library (zlib,
  sqlite) could be named only by where it happens to be on one machine.
- **Compiling `.c` files together with the generated C:** the
  generated C is compiled with warnings off; the program's own C should
  show its warnings.

## Semantic rationale

Nothing in the core changes: an `extern fn` call is still
`[Extern-Call]`, whose callee's behaviour is the author's trusted
assertion. `extern "…";` only tells the implementation where that
callee is.

## Usability

    // main.cb
    import std;

    module geometry "./geometry.cb";

    fn main()
    {
        print(geometry::rect_area(6, 7));
    }

    // geometry.cb: the module that needs the C code names it
    extern "./geometry/shapes.c";            // relative to geometry.cb
    extern "m";                              // an installed library

    extern fn area(i32 w, i32 h) : i32;

    export fn rect_area(u8 w, u8 h) : i32
    {
        unsafe { area(widen<i32>(w), widen<i32>(h)) }
    }

`cobc --run main.cb` builds and runs it, with no other options.

## Explainability

"`extern "./x.c";` or `extern "z";` says which C code the program's
`extern fn`s come from; a path is relative to the file it is written
in" is two sentences.

## Implementation-feasibility

The parser gains one item form; the item table records each string and
its line; the source map of file-backed modules gives the declaring
file. `cobc` resolves the strings, compiles `.c` files, and passes the
results to the C compiler after the generated program and before its
runtime. About 150 lines.

## Compatibility impact

Extension. `extern "` could not begin an item before.

## Prior-art status

- **Rust:** `#[link(name = "z")] extern "C" { … }`; build scripts for
  paths.
- **D:** `pragma(lib, "z");`
- **Odin:** `foreign import z "system:z"`, then `foreign z { … }`.
- **Go (cgo):** `// #cgo LDFLAGS: -lz` in a comment.
- **Nim / C#:** `{.dynlib: "z".}`, `[DllImport("z")]`, per function.
- **MSVC C:** `#pragma comment(lib, "z")`.

## Invariant traceability

`inv.trust-transition`: unchanged. What the named code returns is a
claim, as every extern's result is.

## Revisit conditions

- Build systems needing compiler options (`-l`, `-L`, extra inputs).
- Calling CobaltC from C, or a second calling convention (would
  suggest qualifying `extern`).
- A C symbol whose name is not a valid CobaltC identifier (would
  suggest a link-name mapping).
- Letting a C function write into a live object (the pattern today is
  `allocate`, let C fill it, copy out).
