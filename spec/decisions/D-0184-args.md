# D-0184 — `Args`: command-line options

Status: ACCEPTED (2026-10-07, the owner: "implement C1, C2, C3, C4, C5 and C6", on the completeness assessment of the same day)
Kind: Decision (`spec/02-schema.md`)
Governed by: `CobaltC_Master_Instructions.md` §1, §9
Depends on: D-0031 (`arg`, `arg_count`), D-0176 (`std::env`), D-0181 (`strip_prefix`)
Affects: `spec/21` §0, §2c (4.44.0), `spec/conformance.md` (3.174.0), the guide §21, `impl/std/env.cb`; `CHG-0212`

## Problem

Every command-line showcase loops over `arg(i)` by hand to find
`--flag`, `--name=value` and the files, and writes its own usage text;
the loops differ in what they accept (`--name value` or not, `--` or
not) and none refuses a misspelt option. "Read the program's options" is
one intent every tool has.

## Candidate mechanisms

1. **Declare while reading (Go's `flag`): `Args::flag`, `option`,
   `options` each name an option, take it from the arguments and add a
   line to `help`; `finish` gives the rest and refuses what no one
   declared.** Selected: no table to build first, no types to describe,
   and the help text falls out of the declarations.
2. **A declaration table then a parse** (Python's `argparse`). More
   machinery (option kinds, defaults, choices) for the same result.
3. **POSIX bundling (`-abc`), `-ovalue`, subcommands.** Fewer new
   tokens wins: long options, one-letter short forms, `--name=value`,
   `--name value`, `-s value`, `--`, a lone `-`. Bundling and
   subcommands are a later decision if a tool needs them.

## Selected design

`spec/21` §2c `rule.stdlib.args`, in `std::env`:

- `Args::new(usage)`: the program's arguments (`arg(0)` on), `Err` for
  one that is not UTF-8; `Args::from(items, usage)` over arguments given
  (a test, or another source).
- `[Args-Flag]` `Args::flag(a, name, short, help) : bool`: whether
  `--name` or `-s` (`short` one letter, or `""`) occurs; every
  occurrence is taken.
- `[Args-Option]` `Args::options(a, name, short, value_name, help)`:
  every value of `--name=value`, `--name value` or `-s value`, in order;
  `Args::option` the last one, or `None`. A `--name` with nothing after
  it is left for `finish` to refuse as missing its value.
- `[Args-Finish]` `Args::finish(a) : Result<Vec<String>, String>`: the
  arguments that are not options, in order, everything after `--` among
  them; `Err(message)` naming the first undeclared option or an option
  missing its value. `-` alone is an argument.
- `[Args-Help]` `Args::help(a)`: `Usage: ` and the usage line, then
  one aligned line per option declared so far (`-s, --name VALUE  help`).

## Compatibility impact

Additive: `Args` and its functions in `std::env`. No new tokens.

## Testing

`conf.args-flags-options`, `conf.args-finish`, `conf.args-help`
(`args_ok.cb`): a program run with flags, both option forms, a repeated
option, `--` and an option-looking argument after it; `Args::from` with
an undeclared option, an option missing its value, a lone `-` and two
`--`; the help text.

## Revisit conditions

- Bundled short flags, subcommands, or a counted flag (`-vv`).
