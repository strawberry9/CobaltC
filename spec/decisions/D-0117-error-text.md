# D-0117 — The `std` errors as text

Status: ACCEPTED (2026-09-30, the owner: "proceed with F8 as D-0117" — the findings report's F8; round-6 friction 19)
Kind: Decision (`spec/02-schema.md`)
Governed by: `CobaltC_Master_Instructions.md` §1, §8 (item 10), §9 ("Library helpers")
Depends on: D-0054 (`File`, `FileError`), D-0029 (`ReadError`), D-0032 (`ParseError`), D-0100 (`%v` prints a variant's name)
Affects: `spec/21` §0 (3.46.0), `spec/conformance.md`, the guide §21, `impl/src/prelude.rs`

## Problem

`std`'s error enums can be matched but not printed as a sentence:
`%v` gives the variant's name (`NotFound`, D-0100), right in a log
line and wrong in a message to a person. So nine programs of the
corpus wrote the same four-arm `describe(FileError) : str`, and
others its twin for `ParseError`.

## Candidate mechanisms

1. **Leave the tables to programs.** Nine copies and counting.
2. **A `text` function per `std` error type**, returning a `str`.
   Selected: it names one common intent — this error, as a sentence —
   and without it the reader must read the table to learn that a
   program merely reports the error (§9's test).
3. **`%v` printing a payload-bearing error as a sentence.** Needs a
   "text of a value" hook for any type — a trait-shaped question,
   larger than this; not adopted.

## Selected design

    FileError::text(ref<FileError, shared> e) : str
    ReadError::text(ref<ReadError, shared> e) : str
    ParseError::text(ref<ParseError, shared> e) : str
    Utf8Error::text(ref<Utf8Error, shared> e) : str

Fixed sentences (`spec/21` §0), so nothing is allocated; the offsets in
`Utf8Error` and `ParseError::Invalid` stay for a `match`. A program's
own error enum keeps its own text function, with its payloads.

## Compatibility impact

Additive; a program's own `FileError::text` shadows it (D-0024).

## Revisit conditions

If a general "text of a value" mechanism is ever adopted, these are
its instances for the `std` errors.
