# D-0337 — The bug reports of Update 207

Status: decided by the implementer under the owner's instruction to resolve every report; for the owner's review
(2026-11-23). An agent testing Update 207 (`156c0e8`) filed 110 reports (240–349), and the owner asked for all of
them to be resolved in the next push. Most are plain defects and need no decision; this record holds the ones that
changed a rule, added to `std`, refused programs that were accepted before, added a warning, or were answered by a
message or a document instead of a change. Each was chosen as the smallest choice consistent with the earlier
decisions and with the owner's rule that code reads on its own: no implicit rule a reader cannot see on the line; a
diagnostic that gives the written form, `--doc` or a library function where possible. Implemented by `CHG-0480`
Kind: Decision (`spec/02-schema.md`)
Depends on: D-0035 (radix literals), D-0073 (`+` is arithmetic), D-0085 (`Vec::truncate`), D-0090 (built-in bounds,
`[T-Bound-Literal]`), D-0098 (trailing commas), D-0143 (interrupts are a polled flag), D-0151 (`from_hex`), D-0181
(`[From-Iso]`), D-0182 (`JsonObject`), D-0184 (`Args`, subcommands deferred), D-0204 and D-0210 (cancellation points),
D-0232 §1 (`max_redirects` 0), D-0267 (`std::stats`), D-0271 (key types), D-0287 (lazy `foreach` parts), D-0318
(several result sources), D-0332 §2.12 (error variants), D-0336 §2.1 (`diag.comparison-constant`), §2.7
(`[Ref-Binding-Last-Use]`), §2.12 (lookups by a key held), §2.14 (`[Print-Reader-Gone]`), §2.15 (an unwind runs every
destructor), §2.16 (threads at the end of `main`), §2.19 (the executable cache), §2.21 (`FileError`), §2.22
(`ParseError::describe`, `to_iso_local`)
Affects: `spec/05`, `spec/06`, `spec/12`, `spec/14`, `spec/15`, `spec/16`, `spec/18`, `spec/19`, `spec/21`, `spec/22`,
`spec/OVERVIEW.md`, `spec/registry/diagnostics.md`, `spec/conformance.md`

## 1. Problem

The reports fall into groups (`CHG-0480` has each one): the checker left to the run what it can refuse, or refused
correct programs; the default build and `--all-checks` disagreed; accepted programs that `cobc` or the C compiler
could not build; diagnostics that gave wrong or no advice, most of them for habits from C, Rust and Python; the
runtime's threads, interrupts and standard streams; the time of locks, line loops and JSON objects; `cobc`'s cache,
`--explore` and `cobfmt`; and `std`. Several asked for a new rule, a new warning or a new function, or showed that a
rule and the compiler disagreed.

## 2. Decision

1. **Three warnings, not errors (258, 302, 346).** Each is a warning of `--check`, like D-0336's `diag.no-effect`,
   so no program is newly refused; errors were not chosen, since each form is also written on purpose.
   - New `diag.match-arm-break` (258): `break;` as the last statement of a `match` arm's block, after other
     statements, inside a loop, when the arm's pattern is a bare literal (`1`, `b'q'`, `"quit"`; not `Some(true)`).
     That is C's `switch` case written in a loop, where the `break` leaves the loop, not the arm (`spec/14`
     rule.control.while). It was narrowed on real code: "every block arm ending in `break;`" fired on eight places
     in the compiler and `cobmodel` (`None : { …; break; }`); "a literal arm" still fired on three conformance cases
     (`b'.' : { break; }`, and arms that log and stop); a literal nested in a variant still fired in `gen.cb`
     (`Some(true)`) and a fuzz program (`Ok(3)`). An arm that is only `{ break; }`, a variant's arm, and `break`
     written as the block's value (`{ …; break }`) or the arm's value (`p : break,`) never warn; no `.cb` file of the
     repository gets it. `diag.break-outside-loop` no longer names `return` in its message.
   - New `diag.handle-discarded` (302): `_ = e;` where `e` is a `handle<T>` ends the handle at that statement, and a
     handle's end joins its thread (`rule.conc.join`), so `_ = spawn(f, x);` runs the thread to its end there; the
     message says so and names keeping the handle (or pushing it onto a `Vec` of handles) and joining it later
     (`spec/19` type.handle). Making `_ =` detach was not chosen: an implicit rule a reader cannot see, and a thread
     would outlive what it was lent.
   - New `diag.reference-identity` (346): `==` or `!=` between two `ref<T, _>` whose `T` is a type parameter of the
     body bound `eq` (or `ordered`). `[Eq-Ref]` stays identity, as `spec/09` and the guide's table have it; an
     implicit dereference would be an implicit rule. The message gives both written forms, `*x == *v` for the values
     and `rawptr_of(x) == rawptr_of(v)` for the place (which silences it). Concrete types are not warned (the type
     is on the line and identity is common there), nor is `std` (`spec/12`).
   - `diag.comparison-constant` (D-0336 §2.1) is not given for an operand whose type is a type parameter's in a
     generic body's instantiation, where the comparison is meant for the other instantiations (269), and for `a - b
     >= 0` it names `a >= b` (270) (`spec/06` rule.arith.cmp).
2. **A decimal literal does not start with `0` (259).** A decimal integer literal of two or more digits that starts
   with `0` (`0755`, `00`) is lexically ill-formed (`spec/22` §1). This supersedes D-0035's "`010` stays ten": C reads
   `0755` as 493, so the text does not show which number is meant, and a C file mode passed through `extern` got the
   wrong bits; Python 3 refuses it the same way. The message gives both readings ("the decimal number is written
   `755`, and C's octal reading, 493, is written `0o755`"). No program of the repository wrote one; floats are
   untouched.
3. **A `#!` first line is a comment (329).** A first line starting `#!` (`#!/usr/bin/env -S cobc --run`) is a
   comment, as a `//` line is (`spec/22` `comment`), so `cobfmt` keeps it as written. Anywhere else `#` is refused as
   before, and the message adds that a `#!` line is allowed only as the file's first line. The line is visibly an
   interpreter line, so nothing is hidden from a reader.
4. **The grammar of `match` follows the compiler (257).** `'{' arm (',' arm)* ','? '}'` (`spec/16` §4, `spec/22`):
   the comma after the last arm may be left out. The parser always accepted it; a build that refused it found 43
   files relying on it, all one-line matches (`match (k) { A : true, _ : false }`) in `std` (`time.cb`,
   `crypto/kdf.cb`), the back end, the showcase and the tests, so refusing it would have broken `std` and the
   compiler's own source. The separator form is what D-0098 and the struct and array literals already allow. A
   missing comma between two arms stays an error. No program is newly refused or accepted; the book and the guide say
   "a comma follows each arm; the one after the last may be left out".
5. **`+` on text is `diag.text-join` (260).** `+` with a `String`, `str` or `StringView` operand was reported as
   `diag.read-of-resource`, and `--explain` described copying a resource. A new id, whose message and `--explain`
   name the joining forms, is used for `str + str` too (`spec/06` rule.arith.checked, D-0073: `+` is arithmetic). The
   programs refused are the same.
6. **Literals where a type parameter is expected (345, 338).** `[T-Bound-Literal]` (D-0090) gives a literal the type
   `T` only when `T`'s bound implies `number`. The generic pass skipped the check, so `T s = 0;` with an unbounded
   `T` ran for `i32` and was refused only at a float instantiation; it is now refused in the body, naming the
   `number` bound (or `integer`). This is the spec's rule as written, and the book's "rejected where it is written";
   it refuses programs that were accepted. In the other direction (338), "where `T` is expected" includes a call's
   argument whose parameter the other arguments, or the expected result, fix as `T` (`max(x, 0)`, `Vec::push(v, 0)`,
   `Vec<T> v = Vec::filled(n, 0)`, `wrapping_sub(v, 1)`), so such a literal takes `T` in each instantiation, `f64`
   included (`spec/12` note).
7. **A type parameter's name is its own (281).** In `struct Stack<Item>` and its functions `Item` is the parameter
   even beside a `struct Item`, and a program's `struct K` (or `V`, `T`) is a type of its own where `std`'s
   `HashMap<K, V>` or `Queue<T>` names a parameter so (`spec/12` rule.type.kind). The resolver renames such a
   parameter in its declaration (`K'`, shown so in messages). Before, a program's `struct K` was the same type as
   `std`'s parameter, and the program crashed `cobc`.
8. **A view key's probe takes a literal (242).** A `StringView` key's probe is `ref<StringView, shared>`; a `str`
   there (a literal or a `str` value) is `&StringView::of(t)`, a view of it borrowed for the call, so
   `Vec::contains(&words, "fig")` works on a `Vec<StringView>` as it did on a `Vec<String>` (`spec/12`
   rule.type.probe). The call reads as the `Vec<String>` one does; only outside generic bodies, where the probe is one
   type for every instantiation.
9. **Refused at `--check`, not at the run (291, 313, 318).** Each was accepted and faulted at the run, always or
   depending on timing; refusing it loses no program that worked.
   - 291: a function returning a value that holds references in data (a `Vec<StringView>`, a `Vec` of structs holding
     views) whose facts reach an owner of the function's frame (`split_whitespace(&src)` of a local `src`) is
     `diag.reference-escapes-scope`, as returning one view-holding struct already was; `spec/10` §2 already says a
     result that holds references without being one is treated as a reference. Conformance
     `vec_of_refs_result_outlives_argument_checked_at_run` is now refused statically.
   - 313: a thread's handle (or a `Vec` of them) declared outside a loop, that lent a place on an earlier round,
     refutes `¬clash` at a later round's access when neither path has a computed index or range (`spec/14` §6): the
     outcome would depend on whether that thread had ended, which is timing. Two consecutive `spawn(worker, &mut
     count)` lines were already refused; Rust refuses the loop too. A handle that lends on some paths only stays the
     run's (`conf.spawn-join-in-branch-dynamic`). Conformance `ref_lent_in_loop_handle_kept_written_rejected` is now
     refused statically.
   - 318: a resource moved out through a lock guard (`Vec<String> batch = *g;`, `String n = g.name;`) is refused as
     the same move through a `ref` is, naming `replace(&mut *g, Vec::new())`, `replace(&mut g.name, v)` and
     `clone(&*g)`.
10. **A borrow ends inside a nested branch (292).** A waiting binding (a `let` reference or a `match`/`if` binder)
    whose last mention is an `if` statement, not named in its condition and named in exactly one branch block, also
    ends inside that block after its last mention there, and at the start of the other branch; the end after the whole
    statement stays, on every path, and is then nothing. So report 052's "copy, then advance" is accepted inside a
    nested `if`, in the checker and the run alike (`[Ref-Binding-Last-Use]`, D-0111, D-0336 §2.7). D-0336 §2.7's
    finding that finer granularity refused `cobc`'s own source is about expressions, not nested blocks; statement
    granularity is kept inside each block. Only `if` branches: loops and `else if` chains are not entered (a binding
    mentioned in a loop body is live across rounds). With it, a binding last named in an `else if` condition ends
    there (268) and a binding last mentioned by a block's last statement ends after it (265), as the static pass
    already took them.
11. **`spawn` of a closure capturing nothing (320).** A closure literal whose capture list is empty (`spawn([]() {
    … })`) is accepted without `move` (`spec/19` `rule.conc.spawn`, `spec/OVERVIEW.md`): `[]` is written on the line
    and borrows nothing, so the rule's reason (references into the spawning frame) does not apply, and `move` would
    change nothing. A closure *binding* given to `spawn` keeps the rule, since its captures are not on the line.
12. **Generic code by a key held (342, 241).** New `HashMap::entry_owned(m, K k, V fresh) : ref<V, exclusive>`, as
    `entry` given the key itself (kept only when absent, otherwise destroyed with `fresh`), `HashMap::get_mut_ref(m,
    ref<K>) : Option<ref<V, exclusive>> from m`, `HashMap::remove_ref(m, ref<K>) : Option<V>` and
    `HashSet::remove_ref(s, ref<K>) : bool`, completing D-0336 §2.12's `_ref` family. The report's `entry_ref(m, &k,
    fresh)` was not chosen: with two reference parameters it needs `from m`, and the result-source rule (D-0318) then
    refuses storing `fresh` (a `V` may hold references); `entry`'s own shape is the only one that can store it, so
    the generic form takes the key owned, and a generic count-by already holds `K k = key(x)`. A probe mismatch in an
    instantiation names the `_ref` functions and `entry_owned` in every instantiation alike (241).
13. **`split` is lazy only with a literal separator (280).** D-0287's lazy `foreach` over `StringView::split` gave
    the cursor a `str` separator, so a `StringView` separator was refused though `split` declares one. `Parts` keeps
    its `str`: a view separator would make the cursor hold a second borrowed text the static pass does not follow
    through the loop. The rewrite is made only when the separator is written as a text literal; another separator
    makes the `Vec`, as the call does (`spec/14` rule.control.foreach-parts).
14. **The end of `main` and threads (316, 317, 332, 319).**
    - 316: D-0336 §2.16 stands: the end of `main` waits for the threads still running, so their output is not lost.
      What was wrong is fixed by 317: `main`'s standard output is written before that wait (`spec/21` `[Print-Buffer]`
      already said "never later than `main` returns"). A thread blocked forever keeps the program running, as a
      non-daemon thread does in Java and Python; SIGINT and SIGTERM still end it by their default action, and with
      `watch_interrupts` a second interrupt does (§2.15). Not chosen: abandoning threads blocked in I/O (a rule a
      reader cannot see on the line, and their output would be lost again), or reporting a deadlock (the runtime
      cannot tell an `accept` that never returns from a slow one) (`spec/15` `[Terminate-Ok]`).
    - 332: a read of standard input that waits for its next bytes (`read_line`, `stdin_read`, `read_all`) is a
      cancellation point (`spec/19`, D-0210's list, which named a socket's wait but not standard input), so `cancel`
      ends a "press q to quit" helper and `main` can return. On Windows a console's read is seen to be cancelled at
      its next input event.
    - 319: during an `exit`'s unwind, a value another thread still holds a reference to is left as it is, not
      destroyed: the other threads take no further steps, and the storage is the process's until it ends (`spec/15`
      `[Terminate-Exit]`). The default build and `--all-checks` now agree (exit 64).
15. **A second unseen interrupt ends the program (331, 300).** D-0143 keeps interrupts a polled flag. Under
    `watch_interrupts`, an interrupt that arrives while the previous one is still unseen (the program has not called
    `interrupt_requested()` since) ends the program as without `watch_interrupts` (SIGINT: status 130), the common
    "Ctrl-C again to force quit"; so a program blocked in `read_line` or a long computation can be stopped. Not chosen:
    `read_line` or `sleep_ms` returning early (a new `FileError` variant, breaking exhaustive matches, and an early
    sleep is invisible on the line). A socket wait already waiting when an interrupt arrives returns early even if
    another thread has seen the flag since (300). `watch_interrupts`' documentation says both.
16. **Faults during an unwind (337, 277).**
    - 337: a stack exhausted while a destructor runs (the recursive end of a long `Rc` chain) ends the program with
      that one `diag.stack-exhausted` report, files and standard output flushed, and no destructor (`spec/18` §1), as
      Rust ends at a stack overflow: the unwind would end again the values whose destruction was under way (an `Rc`
      count taken below zero, 174,762 reports under `--all-checks`). Destroying a long chain iteratively was not done.
    - 277: a destructor that faults while a contained fault unwinds its thread still ends the program (D-0336 §2.15
      kept it: guarding that unwind resumed generated code out of step), but both faults are reported, the thread's
      own first, as the main thread's unwind orders them (`spec/18` §1).
17. **Standard output before a read (323, 324).** The flush before a read of standard input or before `eprintf` is
    a write like any other: one refused because the pipe's reader has gone ends the program (`[Print-Reader-Gone]`,
    D-0336 §2.14), so `yes | ./filter | head -n 1` ends (323). Standard output is written before a read of standard
    input only when that read waits for the operating system (nothing of the input buffered), and before every read
    when standard output is a terminal, so a prompt still shows before typed-ahead input (`spec/21` `[Print-Buffer]`,
    324). A line filter between pipes then writes in pieces: 100,000 lines in 97 writes (was 100,001).
18. **The executable cache and `--explore` (262, 263, 314).** The cache's key (D-0336 §2.19) adds each `extern` C
    file as `cc <flags> -E` preprocesses it (its headers; no cache when that fails), the C compiler's environment
    (`CPATH`, `C_INCLUDE_PATH`, `CPLUS_INCLUDE_PATH`, `LIBRARY_PATH`, `GCC_EXEC_PREFIX`, `COMPILER_PATH`) and the
    bytes of the compiler's own file (the `--cc` path, or the first match on `PATH`; no cache when none is found); a
    build for `-march=native` is not cached, since its code depends on the machine. Every `--explore` run has a
    wall-clock limit, the larger of 10 s and ten times the slowest run that ended; a run past it is killed and shown
    as "did not end in time" with its replay seed (314), so a schedule that never ends is reported instead of hanging.
19. **`std`.**
    - 286: `String::truncate(&mut s, n)` with `n` at or beyond the length leaves `s` as it is, as `Vec::truncate`
      does; `n` inside a character still faults `diag.not-char-boundary`. This revisits D-0085's last sentence
      ("`String::truncate` keeps its fault"): D-0085's own reason for `Vec::truncate`, "the call means at most `n`",
      holds for text, and the boundary check is what the fault was for.
    - 287: `from_hex("")` is `Ok` of no bytes (D-0151 had `Empty`), so an empty blob stored as hex reads back, as
      `base64_decode("")` and Python's `bytes.fromhex("")` do; PostgreSQL's `get_bytes` loses its special case.
    - 285: `median` and `percentile` pass over a NaN, as `minimum` and `maximum` do (D-0267), giving NaN only when
      every value is one (`[Stats-Median]`, `[Stats-Percentile]`).
    - 240, 304: `from_iso` and `unix_ms_from_iso` read `T`, `t` or a space between date and time, `Z` or `z`, and
      offsets `±HH:MM`, `±HH`, `±HHMM` (converted to UTC); a time with no zone is read as written (`[From-Iso]`,
      D-0181), so `to_iso_local`'s text (D-0336 §2.22) and PostgreSQL's `timestamptz` text read back. New
      `PgRow::get_datetime(&row, i) : Option<DateTime>` (fraction dropped; `unix_ms_from_iso` keeps milliseconds).
    - 301: `JsonObject(Vec<JsonMember>)` stays (public, ordered, duplicates kept, D-0182: changing it would break
      every program that builds or matches one). New `Json::push_member(&mut j, key, v)` (an append, no search, for
      keys known to be new) and `Json::key_index(&j) : HashMap<String, usize>` (each key's first member, for many
      lookups); `Json::set` and `get` say they search in order and point to both (`[Json-Set]`).
    - 303: `PgValue::Float(x)` with `x` whole within ±2^53 is sent without a fraction (`2`, `-0` for −0.0), so a JSON
      number goes into an integer column as into a float or numeric one; a fraction into an integer column is still
      refused by the server (a typed `float8` parameter would round silently). The documentation names
      `Int(Json::as_i64(v))` for the explicit form (`[Pg-Query]`).
    - 310: `max_redirects = 0` follows none: `send` gives the 3xx response itself, its `Location` and `Set-Cookie`
      readable; a positive limit keeps `TooManyRedirects` past it (`[Http-Redirect]`). D-0232 §1 already told a
      program to set it to 0 and follow redirects itself, which `TooManyRedirects` made impossible. No new field.
    - 326: `FileError` gains `CrossDevice` ("cannot rename across file systems: copy, then remove"), `NameTooLong`,
      `InvalidName` (a NUL in a name, a directory moved into itself), `NotOpenForThat` (a write to a file opened for
      reading) and `ArgsTooLong`, after `SameFile`, as D-0332 §2.12 and D-0336 §2.21 did; the runtime maps EXDEV,
      ENAMETOOLONG, EINVAL, EBADF and E2BIG (and their Windows codes) to them. A `match` on `FileError` naming every
      variant without `_` needs the new arms. A read or write against a `File`'s open mode, which `CHG-0071` made
      exactly `Io` on every platform, is now exactly `NotOpenForThat` on every platform (the runtime checks the mode
      itself, before the operating system). `read_line` with standard input a directory stays `Io` (its lowering is
      native; documented).
    - 328: no new `ParseError` variant (every exhaustive match of a number parse would need an arm that never
      occurs there). TOML keeps `Invalid(i)` at the key or date and records why; new `Toml::describe_error(&e, text)`
      gives "line 3, column 1: name is defined twice" or "… 2026-13-01 is not a date or time", else
      `ParseError::describe`'s text (D-0336 §2.22) (`[Toml-Error]`).
    - 330: a second declaration of an option's name or letter (`-v, --verbose` and `-v, --version`) faults
      `diag.assert-failed` at its call through a `requires` ("`Args`: -v is declared twice (the second time for
      --version)"): a programming error, found the first time the program runs (`[Args-Flag]`). `-o=x` is advised
      `-o x`.
    - 333: new `Args::command(&mut a, "-C --config") : Vec<String>`, called before the global declarations: the
      command is the first argument (before `--`, or just after it) that is not an option and not the value of a
      named value-taking global option; it and everything after are taken and returned, command first, and the
      command's options are read with `Args::from(cmd, usage)` (`[Args-Command]`). D-0184 left subcommands "a later
      decision if a tool needs them". The value-taking globals are named because declarations read at once (this API
      has no second phase); the list is on the line.
    - 334: new `File::read_line_with_end(&mut f)`: the line with its `\n` (and `\r`) kept, so a line not yet ended is
      told apart (Python's `readline`); `read_line` is unchanged and points to it.
    - 298, 299, 327 are defects with a visible change: an idle keep-alive connection whose timeout passes before a
      byte of a next request ends `HttpConnection::request` with `Ok(None)` and closes (a timeout inside a request
      stays `Err`); the client never copies a request's own `Transfer-Encoding`, `Content-Length`, `Host` or
      `Connection` (it frames the body itself, `[Http-Send]`); `copy_file` of a source that is neither a regular file
      nor a directory (a pipe, `/dev/stdin`) reads it to its end, as `cp` does (`[Copy-File]`).
20. **Messages and documents, not new rules.** These reports asked for something a reader could not see on the line,
    or showed that a document promised what the language does not do; each got a message or a document that gives
    the written form.
    - 343: `==` on a struct holding text compares it as a key type (D-0271), and a `f64` field has no equivalence; the
      rule is unchanged (`spec/12`), and the refusal names the field (`price` is `f64`) instead of "a `Vec`, a `Box`,
      a hash table, a handle".
    - 344: a function value has no `clone`; the message for a moved `move` closure names writing it without `move`,
      making a new closure for each use, or using it before the move, not `clone(&f)`.
    - 349: a type parameter has no functions of its own (no traits); `T::speak(x)` is told to pass the operation as a
      `fn` parameter and call it, `T::clone` to write `clone(x)`, `T::eq`/`cmp` to write `==`/`<` with the bound.
    - 347: `o == None` on an `Option` of a function and `f == g` say a function value has no `==` (`[Eq-Fn]`) and
      name `Option::is_none` or `match`.
    - 283: `[Call-Multi-Ref-Return-Rejected]` (a missing `from` clause on a function with two or more reference
      parameters) is checked before the bodies, as `[Result-Source-Form]` was, so it is reported at the function, not
      as a consequence at its caller. A function returning a reference and taking none is still reported after the
      bodies (conformance `ref_escape_plain_param_return_rejected` expects the body's error first).
    - 247: in a loop's fixed-point rounds, when the condition and the body both fail, the condition's (earlier) error
      is reported, as D-0323 says.
    - 254: the index fault's repair no longer names "checked accessors" that do not exist; it says to check the index
      first or take the last element with `Vec::pop`, an `Option`; `Vec::get`, `is_empty`, `last` and `v.first()`
      are each given the CobaltC form.
    - 252: a run-time fault's caret is under the operation when the fault names it and the line has it once (the
      operator of `diag.arith-overflow`, the indexed name of `diag.index-out-of-bounds`), else the line's first token
      (D-0228); `0 - 1` of an unsigned type adds the count-down form (registry, "How a diagnostic is shown").
    - 284: `pow` is exact for integer types; for a float base each multiplication rounds, and `powf` is within one
      unit. `pow` stays repeated squaring (it is generic over `number`, and its integer exactness and overflow check
      are its point); `--doc pow` and `--doc powf` say so.
    - 296, 305, 306: `--doc` lists an enum's variants and a struct's exported fields, attaches a unit test to an
      exported item only when no private item came between, and documents a derived `T::clone` (D-0116).
    - 322: the guide's threads section showed a "statics section" that did not exist; it now gives the form
      (`static Atomic<u64> HITS = 0;`, used through `&HITS`, `spec/17` §1a′) with a runnable example.
    - 278: the book's description of `cobfmt` (one-line forms, `--width`) and `spec/15`'s `[Terminate-Ok]` and
      `[Terminate-Exit]` comments, which still said "no other thread is live", now describe what happens.

## 3. Not defects, or documented limits

- **311.** Divide-and-conquer `spawn` on `&mut s[0..mid]` and `&mut s[mid..n]` is slow because the static pass cannot
  prove two ranges with non-constant bounds disjoint, so the slice is tracked and every `s[i]` is a run-time check
  shared between threads. Proving `[a..m)` and `[m..b)` disjoint needs symbolic range bounds in the static pass, a
  feature; the guide's performance chapter says so and names `slice_parts(s, 2)` (decided; 7 ms on the report's case)
  and `--report-tracking`.
- **315.** An uncontended `lock` is 18% fewer instructions and a contended one spins before the shared wait, but it
  does not reach C's 10 ns: a lock still takes its stripe's mutex twice and two hash operations. A fast path in the
  mutex's own cells would need the deadlock finder and poisoning to read holders from there, a record redesign the
  owner reserved (runtime optimisation, after Update 200). The guide no longer says a locked block "costs a lock and
  an unlock and nothing more".
- **316.** A thread that never ends keeps the program running (§2.14); chosen, not a defect.
- **284.** `pow` of a float base (§2.20): documented.
- **326.** `read_line` from a standard input that is a directory stays `Io` (§2.19): documented.
- **337.** A long `Rc` chain still exhausts the stack when it is destroyed; the program now ends with one report
  (§2.16).

## 4. Consequences

- Programs newly refused: a decimal literal with a leading `0` (§2.2, D-0035's "`010` stays ten" superseded); a
  literal where an unbounded type parameter is expected (§2.6, the spec's rule, which the generic pass skipped); a
  returned value holding views of the function's own data (291), a place lent to a thread on an earlier round of a
  loop and used again (313), and a resource moved out through a lock guard (318) (§2.9), each of which faulted at the
  run.
- Programs newly accepted: a `#!` first line (§2.3); `spawn` of `[]` (§2.11); a literal looked up in a
  `Vec<StringView>` (§2.8); integer literals in generic call arguments at a float `T` (§2.6); a program's own `K`, `V`
  or `T` types beside `std`'s generics (§2.7); a borrow ending inside a nested `if` (§2.10); `split` with a view
  separator in a `foreach` (§2.13); and the correct programs `CHG-0480` lists (267, 273, 288, 290, 293, 307, 339, 340,
  341, and generic programs that `cobc` or the C compiler could not build).
- New warnings: `diag.match-arm-break`, `diag.handle-discarded`, `diag.reference-identity` (§2.1). New error id
  `diag.text-join` (§2.5), for programs that were refused under another id.
- `std` gains `HashMap::entry_owned`, `HashMap::get_mut_ref`, `HashMap::remove_ref`, `HashSet::remove_ref`,
  `Json::push_member`, `Json::key_index`, `Toml::describe_error`, `Args::command`, `File::read_line_with_end`,
  `PgRow::get_datetime`; `FileError` gains `CrossDevice`, `NameTooLong`, `InvalidName`, `NotOpenForThat`,
  `ArgsTooLong`.
- Behaviour that changes for running programs: `String::truncate` past the end does nothing (§2.19); `from_hex("")`
  is `Ok`; `median` and `percentile` pass over NaNs; `from_iso` reads offsets, a space and no zone; a whole
  `PgValue::Float` is sent without a fraction; `max_redirects = 0` gives the 3xx; an idle keep-alive connection's
  timeout is `Ok(None)`; a second declaration of an `Args` option faults; `copy_file` copies a pipe; errors once `Io`
  are named (§2.19); a read of standard input is a cancellation point (§2.14); a second unseen interrupt ends the
  program (§2.15); `main`'s output is written before it waits for threads (§2.14); an `exit` leaves values lent to
  running threads (§2.14); a stack exhausted in a destructor ends the program without destructors (§2.16); a line
  filter between pipes writes in pieces and ends when its reader goes (§2.17); the cache keys on headers, the
  compiler's file and its environment, and `--explore` runs have a time limit (§2.18).

## 5. Verification

See `CHG-0480`.
