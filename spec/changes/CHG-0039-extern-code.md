# CHG-0039 — `extern "…";`: naming foreign code to link

Kind: Change record (`spec/02-schema.md` §2)
Status: ACCEPTED (2026-09-25, owner-chosen)
Governed by: `CobaltC_Master_Instructions.md` §1, §19, §21
Depends on: D-0030, rule.trust.extern-call
Affects: rule.trust.extern-code (new), spec/22 §3 `item`, spec/22 §5, the §7 cases listed below

## Problem / motivation

D-0030 has the details. In short, an `extern fn` could call only what
the implementation linked by itself, and a program could not name its
own C code or an installed library.

## Decision

D-0030: an `extern "…";` item naming foreign code, whose resolution and
linking are implementation-defined.

## What changed

- **`spec/20` 1.9.0:** `rule.trust.extern-code` (new).
- **`spec/22` 2.11.0:** `item` gains `extern-code ::= 'extern'
  str-literal ';'`; §5's out-of-scope list is qualified.
- **`spec/conformance.md` 3.25.0:** two §7 cases; the `cobc-only:`
  convention.
- **`spec/02-schema.md` 1.0.15:** §5's "in use" ranges.
- **Implementations:**
  - the parser reads `extern "…";` (an `export` before it is a parse
    error), and the item table records each string with its line;
  - `cobc` resolves each string (`cobc/src/link.rs`): a path beginning
    `./`, `../` or `/` against the declaring file's directory — a `.c`
    file compiled on its own with warnings shown, a `.o`, `.a`, or a
    `.so` linked by absolute path with an rpath — and any other string
    as a library name (`-lNAME`). Duplicates are linked once; a missing
    file, a file of another kind, a malformed string, or a library the
    linker cannot find is reported at its declaration (exit 2);
  - `coby` accepts and ignores the declaration; its message for a
    missing C function names the declared code, which only `cobc`
    links.
- **Conformance runners:** a case only `cobc` can run is marked
  `cobc-only: <reason>`. `coby`'s runners require it to be refused
  (exit 3, `unsupported: …`); the compiled runners run it, feeding its
  `stdin-hex:` bytes if it has any, and compare its output with
  `expect-stdout-hex:`. This replaces CHG-0038's use of `stdin-hex:` as
  the marker: the three `read_line` cases now carry `cobc-only: reads
  standard input` as well.

## Affected entities

`rule.trust.extern-code` (new); `spec/22` §3 `item`, §5.

## Previous semantics

None: `extern "` could not begin an item.

## New semantics

    extern "c";   -- an item naming foreign code; no name, no visibility; meaning implementation-defined

## Affected invariants

None.

## Dependency impact

`rule.trust.extern-code` depends on `rule.trust.extern-call`.

## Compatibility classification

Extension.

## Migration implications

None.

## Example changes

None in `spec/examples.md`.

## Conformance changes

**Added:**
- `conf.extern-code-library-name` (`extern "m";`, run by both
  implementations)
- `conf.extern-code-c-file` (a C file named by the case and by a
  module in a subdirectory, relative to each; `cobc-only:`)

Also added, without a row (a parse error, which the rows do not state):
`20-trust-boundaries/extern_code_export_rejected.cb`.
`impl/cobc/tests/extern_code.rs` checks `.o`, `.a` and `.so` files, an
executable using a `.so` run from another directory, and every
reported error.

## Future implementation implications

- A `.c` file is compiled with the program's `-O` and `-march` and the
  C compiler's default warnings, not the generated C's options.

## Prior-art status

See D-0030.

## Revisit conditions

See D-0030.
