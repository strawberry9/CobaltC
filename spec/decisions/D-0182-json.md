# D-0182 — `std::json`: JSON read into a value tree and written back

Status: ACCEPTED (2026-10-07, the owner: "implement C1, C2, C3, C4, C5 and C6", on the completeness assessment of the same day)
Kind: Decision (`spec/02-schema.md`)
Governed by: `CobaltC_Master_Instructions.md` §1, §9, §18
Depends on: D-0136 (submodules), D-0134 (names), D-0181 (`push_code_point`, `is_finite`), D-0045 (recursive types through `Vec`), D-0164 (`String == str`), D-0180 (unique variant names across `std`)
Affects: `spec/21` §0, §2r (4.44.0), `spec/conformance.md` (3.174.0), the guide §21, `impl/std/json.cb`, `impl/std/std.cb`, `src/prelude.rs`; `CHG-0210`

## Problem

Every HTTP API, configuration file and structured log a deployed program
meets is JSON, and `std` had no reader or writer for it: three stress
rounds hand-rolled `json.cb` (`stress/round5`, `round6`), each with its
own escape handling and its own bugs. "Read this JSON" and "write this as
JSON" are among the most common intents a program has, so a library
reader and writer pass the admission test of Master Instructions §9 as
plainly as anything in `std`.

## Candidate mechanisms

1. **A value tree: `Json`, one enum of the six kinds, with `parse`,
   `text`, `pretty`, lookups and builders.** Selected. Every program can
   use it; it needs no language change; it is what the hand-rolled
   parsers were.
2. **Typed (de)serialization: a struct read from or written as JSON by
   its fields.** Needs derivation (a language mechanism the owner has
   not accepted) or per-struct code; a later decision, over the tree.
3. **A streaming (event) reader.** For documents larger than memory;
   not the common case. Deferred.
4. **Numbers kept as text, or as an integer-or-float union.** More
   faithful for integers above 2^53, at the cost of two number kinds in
   every match. Not adopted: `f64`, as JavaScript and most readers; a
   whole value is written without a fraction, and `as_i64` reads one
   back exactly within 2^53.

## Selected design

`spec/21` §2r `rule.stdlib.json`, in `std::json`:

- `Json`: `JsonNull`, `JsonBool(bool)`, `JsonNumber(f64)`,
  `JsonText(String)`, `JsonArray(Vec<Json>)`,
  `JsonObject(Vec<JsonMember>)`; `JsonMember` (`key`, `value`). The
  variant names carry `Json` because `std`'s bare variant names are
  unique (`PgValue::Null`, `PgValue::Text` exist). An object keeps its
  members in order; duplicate keys are kept as read, `get` finding the
  first.
- `[Json-Parse]` `Json::parse(text)`: RFC 8259, one value with
  whitespace (space, tab, LF, CR) around it; `Empty` for none;
  `Invalid(i)` at the first byte that does not fit (the length when the
  text stops short); `OutOfRange` for a number beyond `f64` or nesting
  deeper than 512. Strings: every escape, `\uXXXX` with a surrogate pair
  joined and a lone surrogate U+FFFD (`push_code_point`'s rule), a raw
  control character refused. Numbers by the JSON grammar then
  `parse<f64>`.
- `[Json-Text]` `Json::text(j)`: compact; `[Json-Pretty]`
  `Json::pretty(j)`: two spaces per level, `: ` after a key, no trailing
  newline. A string is written with `"`, `\` and the control characters
  escaped (`\n`, `\r`, `\t`, `\b`, `\f`, else `\u00XX`), everything else
  as it is; a number that is whole and within 2^53 without a fraction,
  else the shortest text that reads back (`String::append`'s); a NaN or
  an infinity, which JSON cannot write, as `null`. `parse(text(j))` is
  `j`.
- `[Json-Get]` `Json::get(j, key)`, `Json::at(j, i)`: a reference into
  the tree, `None` for another kind or nothing there; `is_null`,
  `as_bool`, `as_f64`, `as_i64` (`Some` only for a whole value within
  `i64`), `as_text`, `as_array`, `as_object`.
- `[Json-Set]` `Json::set(j, key, v)` replaces the first member of that
  name or appends; `Json::push(j, v)` appends to an array; on a value of
  another kind each is `diag.assert-failed` (a programmer's error, as an
  index out of bounds is).

Written in CobaltC over `std::text`; nothing native.

## Compatibility impact

Additive: a new module, re-exported by `std`. New names: `Json`,
`JsonMember` and the six variants. No new tokens.

## Testing

`conf.json-parse`, `conf.json-text`, `conf.json-tree`, `conf.json-errors`
(`json_ok.cb`): a document with every kind of value and every escape
read and written back compact and indented, the round trip, lookups and
builders, nineteen refusals with their positions, nesting at and past
the limit.

## Revisit conditions

- Typed (de)serialization, if derivation enters the language.
- A streaming reader, if a program must read documents larger than
  memory.
- Integers beyond 2^53, if a protocol in use carries them.
