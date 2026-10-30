# D-0231 — No project or build file

Status: ACCEPTED (2026-10-14; Q11 of the 2026-10-12 plan, recommendation accepted by the owner)
Kind: Decision (`spec/02-schema.md`); a deliberate absence
Governed by: `CobaltC_Master_Instructions.md` §21
Affects: the guide's "Running a program"

## Decision

A CobaltC program has no project file and no build file. A program is its root file, together with what the
root file names:

- `module m "./file.cb";` loads another file as a module, its path relative to the file that declares it
  (`spec/17` §5);
- `extern "./x.c";` (or a `.o`, `.a` or `.so`) links the program's own C code, and `extern "lib";` links an
  installed library, as `cc -llib` (`spec/20`, `rule.trust.extern-code`).

`coby FILE.cb` runs the program and `cobc FILE.cb` builds it. Each finds every other file from those
declarations, so the source is the whole build description, and `cobc` is the build.

## Why

Everything such a file would hold is already in the source: the files, the C code and the libraries. A
manifest beside it would only repeat that, and the two could disagree. With no package ecosystem, there is
nothing else for it to name.

## Revisit

Revisit when programs depend on other people's CobaltC packages: versions, a place to fetch them from, and
more than one package in a build. A manifest would then have something of its own to say. Adding one then is
easier than removing one now would be.

## Alternatives considered

- **A minimal `cobalt.toml`** (name, entry file, C sources, flags). Declined, for the reasons above.
