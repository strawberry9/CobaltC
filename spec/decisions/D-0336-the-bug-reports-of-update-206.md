# D-0336 — The bug reports of Update 206

Status: decided by the implementer under the owner's instruction to resolve every report; for the owner's review
(2026-11-22). An agent testing Update 206 (`28818f0`) filed 194 reports (046–239), and the owner asked for all of
them to be resolved in the next push. Most are plain defects and need no decision; this record holds the ones that
changed a rule, added to `std`, refused programs that were accepted before, or were answered by a message or a
document instead of a change. Each was chosen as the smallest choice consistent with the earlier decisions and with
the owner's rule that code reads on its own: no implicit rule a reader cannot see on the line; a diagnostic that
gives the written form, `--doc` or a library function where possible. Implemented by `CHG-0479`
Kind: Decision (`spec/02-schema.md`)
Depends on: D-0042 (`foreach`), D-0089 (`fn` values own their closures), D-0100 (`%v`), D-0111 (a reference binding's
last use), D-0125 (`Weak`), D-0135 (a view of a temporary argument), D-0143 (processes and interrupts), D-0178 and
D-0332 §2.4 (standard output), D-0181 (`[Sort-Float]`), D-0194 (quiet types), D-0204 (supervised tasks), D-0290
(frame-held aggregates), D-0315 (decided programs), D-0323 (several errors in a body), D-0324 (variants with named
fields), D-0332 §2.12 (error variants)
Affects: `spec/05`, `spec/06`, `spec/13`, `spec/14`, `spec/18`, `spec/20`, `spec/21`, `spec/22`,
`spec/registry/diagnostics.md`, `spec/conformance.md`

## 1. Problem

The reports fall into groups (`CHG-0479` has each one): the checker left to the run what it can refuse, or refused
correct programs; accepted programs that `cobc` or the C compiler could not build; diagnostics that gave wrong or
no advice; time and memory of checks, lowering and the run; the runtime's faults, threads and standard streams;
`cobc`, `cobfmt` and the C header; security of `std`'s HTTP, TLS, X.509 and PostgreSQL; and the rest of `std`.
Several of them asked for a new rule or a new function, or showed that two rules disagreed.

## 2. Decision

1. **Two warnings, not errors (068, 125).** New `diag.comparison-constant`: an unsigned value compared with `0` so
   that the comparison cannot be false (`i >= 0`, `0 <= i`) or cannot be true (`i < 0`); the message gives the
   count-down form (`spec/06` rule.arith.cmp). New `diag.no-effect`: an expression statement that is only a
   comparison, a name, a field or a literal (`count == count + 1;`) (`spec/13` §3). Both are warnings of `--check`, so
   no program is newly refused. Arithmetic statements are not warned: one may fault, and tests write such statements
   to make it fault. Errors were not chosen, so that no existing program is refused.
2. **Two disambiguation rules of `spec/22` §2 (055, 127).** (1) `identifier '<' type, … '>'` in an expression is type
   arguments unless the token after `>` begins an operand (an identifier, a type name, a literal, a prefix `-` `!`
   `~` `&` `*`), C#'s rule, so `f(a < b, c > d)` is two comparisons. (4) A path `m::Name` followed by an identifier
   and `=` or `;` begins a declaration, whatever `m::Name` resolves to, so a misspelt or unimported type is reported
   by name ("did you mean `std::String`") instead of as a syntax error at the variable. Only programs that were
   syntax errors change meaning.
3. **No `resource enum`; an enum has no destructor (081).** The grammar listed `'resource'?` before `enum` and the
   messages advised it, but the parser refused it, and the back end runs no destructor of an enum (a `fn E::drop` on
   an enum holding a `String` was accepted and never ran). `resource enum` is removed from the grammar, the registry,
   the guide and every message; it is a syntax error that names the `resource struct` wrapper, and a `drop` on any
   enum is `diag.destructor-on-plain-type` ("an enum has no destructor"). Refusing a destructor that never ran
   loses nothing that worked.
4. **A byte-literal range is `u8` (182).** A `foreach` range whose bounds are both byte literals (`b'a' .. b'z'`,
   `b'0' ..= b'9'`) has `u8` as its type (`spec/14` rule.control.foreach-range); a `: u8` after one is told so. The
   bounds say the type on the line; no existing program used such a range.
5. **A literal compared through a shared reference takes the number's type (162).** In `count > 5` with
   `count: ref<u32, shared>`, the literal is a `u32`, so `[Cmp-Through-Shared-Ref]` applies. It only accepts
   programs that were refused, and the comparison reads as written.
6. **Messages, not new rules.** These reports asked for something a reader could not see on the line, or an
   addition the language does not need; each got a message or a document that gives the written form.
   - 046: `%v` stays as D-0100 has it (an enum's variant name; no structs, `Vec`s or payloads). Printing a struct
     would need field names and a layout description in every program. The book no longer says `%v` prints whatever
     it is given, and the refusal says what to print: each field, each element, or an enum's payload by `match`.
   - 159: `String::append` of an enum is refused, naming `String::appendf(&mut s, "%v", x)`; an implicit write of the
     variant's name was not added.
   - 067: `main` stays `void` or `u8`; `?` in `main` names the `fn run() : Result<void, E>` form and shows the `main`
     that matches on it. A `main` returning `Result` would be an exit path the reader cannot see.
   - 058: `==` keeps what it accepts; on a `Vec` the message names `Vec::eq(&a, &b)` or a field-by-field comparison.
   - 071: `&[]` stays `array<i32, 0>`; typing it from the parameter would be a rule change in argument typing. The
     message and `PgConnection::query`'s documentation show `array<PgValue, 0> none = [];` and `&none`.
   - 089: `foreach` over a `Queue` or `PriorityQueue` names the `while (Some(x) = Queue::pop_front(&mut q))` form; a
     walk over a queue was not added.
   - 101: `&if …`, `&match …` and `&{ … }` as an argument stay refused; the message shows binding the value first.
   - 188: a library root checked alone has no name, so `export import inventory::items` cannot resolve; the message
     says how to check it (through a program that mounts it, or its submodules by their names). Mounting a root under
     its file's name was not chosen: an implicit name.
   - 189: `rule.type.expected` is unchanged (an `if` of literals as an operand is `i32`); the message says to type one
     of the literals (`0: usize`), not to `narrow`.
   - 120: `cobc --check` of a file with `unittest` blocks and no `main` checks it as `--unittest` does; elsewhere a
     missing `main` is reported last, after the other errors, never alone.
7. **A reference binder ends at its last use (052, 114).** A `match` arm's binder of reference type, or one of
   `if (P = &x)`, ends after the last statement of the arm's block that mentions it, as a `let` reference does
   (`[Ref-Binding-Last-Use]`, D-0111), in the checker and in the run (`spec/14`, the flow table's `match` binder row).
   A reference whose last use is an `if`'s condition, mentioned in neither branch, ends right after the condition, in
   both (`spec/14` `[Ref-Binding-Last-Use]`). The two passes had disagreed: the static pass ended a reference at its
   last use, the run at the end of its last statement, so such a program was decided and faulted under
   `--all-checks`. Making the static liveness statement-granular instead was tried and refused `cobc`'s own source
   (`match (vo) { ViewOp::SliceString : return lower_slice_string(g, …) }`), a common shape; the run now ends earlier
   where the checker did. A conflict later in the same expression statement is left (`CHG-0479`).
8. **A known value is not folded behind a guard that names it (056).** A local's known value (the flow's literal
   facts) is not used inside the branches of an `if` whose condition names the local, nor in the right operand of
   `&&` or `||`, so code that the guard keeps from running is not refused for what the value would do there. A
   `const` operand keeps the static refusal `spec/06` allows for compile-time constants.
9. **What a binder or a closure borrows stays where it can be (136, 109).**
   - 136: the binder of a `match` (or `if (P = e)`) holding a view of a scrutinee temporary is free inside its arm,
     since the temporary lives through the whole `match` (D-0135); any binding made from it (`kept = n`, a `let` from
     it, a derived value) is refused when it is used after the `match` ends. The `if` and `match` forms now agree, and
     the `match` form, which let a view outlive a released guard, is closed. Letting the `if` form's binder pass
     alone was tried and let `kept = n` escape the guard.
   - 109: a non-`move` closure literal given to `Vec::push`/`insert`, `HashMap::insert` or `Queue::push_back`/
     `push_front` when one of its captures is declared in a block strictly inside the container's is refused,
     naming both and the `move [i](…)` form. This is stronger than the better run-time message the report asked
     for; the run faulted on every such program in both forms, so no accepted program is lost. A capture declared
     outside the container's block stays accepted (080's shape).
10. **Quiet: weak handles and shared references (092, 161).** quiet(τ) (`spec/05`, D-0194) means destroying a value
    runs nothing observable. `Weak<T>` and `ArcWeak<T>` are quiet for any `T` (they never keep the value; destroying
    one frees at most the box), so `n.parent = None` overwrites the old handle. A `shared` reference or slice, so a
    `StringView`, is quiet (its end runs nothing), so a `Vec<StringView>` can be reassigned. An `exclusive`
    reference or slice stays non-quiet. The decided objectless lowerings keep their behaviour for types holding
    references.
11. **`StringView` is `eq` and `ordered`, not a key (098).** It compares by its text, as `==` and `<` on it already
    did, so `Vec::contains`, `Vec::sort` and `Vec::index_of` take a `Vec<StringView>`. It is still not a key type: a
    table of views would hold borrows of text it does not own, and `HashSet<StringView>`'s message is unchanged.
12. **Lookups by a key held (086).** New `HashMap::contains_ref(m, ref<K>)`, `HashMap::get_ref(m, ref<K>) :
    Option<ref<V, shared>> from m`, `HashSet::contains_ref(s, ref<K>)` and `Vec::contains_ref<T: eq>(v, ref<T>)`. A
    `probe<K>` cannot be formed from a `ref<K>` for every `K` in generic code; library functions were chosen over an
    implicit conversion.
13. **Two expected types (181, 190).** `?` directly on a call gives that call's deferred literal argument the
    enclosing function's `Result` error type, so `Option::ok_or(o, "msg")?` in a `Result<_, String>` function makes
    the literal a `String`, as `return Err("…")` does. A temporary may be borrowed as an argument where the call's
    reference result cannot reach it (a `probe<K>` parameter, or one a `from` clause leaves out), so
    `HashMap::entry(&mut m, &Red)` is accepted as `get` was.
14. **Standard output whose reader has gone (102, 167, 171).** New `[Print-Reader-Gone]` (`spec/21`): a write to
    standard output refused because it is a pipe whose reader has gone ends the program as `exit(1)` does
    (`[Terminate-Exit]`: the thread's destructors run, nothing is reported). Every pipeline depends on the writer
    stopping (`prog | head` must end); D-0332 §2.4 made the failure seen but kept the program running. D-0143 (`Err`
    on a pipe) still holds for a child's pipes and sockets. Standard streams closed when the program starts are
    opened on the null device, so a later `open` cannot reuse descriptor 1 and receive the program's output; a write
    to a standard output closed at start is `[Print-Failure]` ("it was closed when the program started"). The
    `[Print-Failure]` line follows a fault's report when the program ends by a fault.
15. **An unwind runs every destructor (166, 170).** A destructor that faults during the program's unwind (a fault's
    or an `exit`'s) does not stop it: its fault is reported after the first, what it left open is folded, and the
    unwind goes on with the next destructor (`spec/18` §1); `conf.fault-unwind-first-fault` now expects both faults
    and `main`'s destructor. A stack-exhaustion unwind lifts the stack floor first, so its destructors run as any
    other unwind's (`spec/18`). During a contained thread's unwind the earlier behaviour stays (the first fault ends
    the program): guarding there resumed generated drop code with scopes out of step.
16. **Threads at the end of `main`; a failure is a resource (134, 238).** A handle destroyed on the thread it names
    marks the thread self-owned instead of waiting on itself; at `[Terminate-Ok]` the runtime waits for every thread
    still running and raises a self-owned thread's failure. `[Terminate-Ok]` assumed no other thread was live; this
    makes it true, and output is never lost. `ThreadFailure` is a `resource struct` whose `drop` frees the runtime's
    record (a failure raised again by `join` or a handle's end is freed too). Ownership bounds the record's life with
    no new function; its documentation says the record is kept while the value lives. A program that iterated
    failures by value in an array literal must take them one at a time (`showcase/tier13/deadlock_found.cb`).
17. **Copies of a closure share its box where nothing can tell (155).** A `fn` box caches whether its closure owns a
    resource, writes its captures or holds a reference. A copy of a closure that does none of these, and whose
    captured closures do none, shares the box, which ends with its last copy, and such a closure's `self` is borrowed
    shared, so copies may be called at once. D-0089's "a copy owns its own closure" is unobservable for those
    closures; others are still copied deeply (a self-writing closure's copy stays independent).
18. **`unwrap` names the error (066).** `Result::unwrap` and `expect` call a new `std`-only intrinsic
    `unwrap_failed_with(&e, head)`: an error that is a number, `bool`, `str`, `String`, `StringView` or an enum is
    formatted as `sprintf("%s: %v", head, e)` formats it; any other type gives `head: a \`T\``. The native
    `Result::unwrap` body, which could not name the error, is no longer used.
19. **An executable build cache (059).** Most of a small HTTP program's build is the C compiler under LTO (4 s of
    27 s was the front end and lowering), and an edit changes the C, so per-part objects would save little.
    `cobc` keeps the executables it builds, keyed by SHA-256 of the generated C, the compiler's identity, version
    and flags, `-O`, the architecture, debug and strip, the target, the runtime's bytes and the program's own C
    files. The cache is `COBALTC_CACHE_DIR`, else `$XDG_CACHE_HOME/cobc`, `%LOCALAPPDATA%\cobc` or `$HOME/.cache/cobc`;
    `COBALTC_CACHE=off` turns it off; it is not used with `-v` or `--lib`; the 32 most recent are kept. A rebuild of
    an unchanged program costs the front end only.
20. **The C header (233, 140, 228).** The `--lib` header can be included from C++: an `extern "C"` block and
    `COBALT_STATIC_ASSERT`/`COBALT_ALIGNOF` macros. A C program links one CobaltC library: each carries its program's
    start-up and type table for the one runtime, so two cannot share it; the header and the guide (§20) say so, and
    the other CobaltC files are modules of the one library. A C library function with a variable argument list is
    declared to C with its fixed parameters and `...` (a table in `gen.cb`), and a variadic function of the program's
    own C is called through a fixed-signature wrapper (`spec/20` rule.trust.extern-call).
21. **The file system and processes.**
    - `FileError` gains `NotEmpty` ("directory not empty"), `LinkLoop` ("too many levels of symbolic links") and
      `SameFile` ("source and target are the same file"), after `TooManyOpen`, as D-0332 §2.12 did (076, 095, 096).
      A missing working directory of a `Command` is `NotFound` (was `Io`), and `copy_file` of a directory is
      `IsDirectory`. A `match` on `FileError` naming every variant without `_` needs the new arms.
    - 096: new `read_link(path) : Result<Option<String>, FileError>` (`None` when the path is not a link; the link is
      not followed). No `PathKind::Link` (it would break exhaustive matches, and `path_kind` follows links by its
      definition) and no directory walker.
    - 103: new `list_dir_bytes(path) : Result<Vec<Vec<u8>>, FileError>` beside `list_dir`, which keeps refusing a name
      that is not UTF-8 and points to it. Path functions still take `StringView`, so such a name can be reported or
      skipped, not opened.
    - 224: new `File::create_private(path) : Result<Option<File>, FileError>` (as `create_new`, mode 0600, no link
      followed) and `make_temp_dir(prefix) : Result<String, FileError>` (a new directory under `temp_dir()`, the
      prefix and 32 hex digits from the secure source, mode 0700, made only if nothing was there). On Windows both take
      the directory's ACL. A mode parameter or `set_permissions` was not added: modes are platform-specific, and the
      two functions cover secrets and temporary files.
    - 168: `fsync`'s EINVAL, EROFS and ENOTSUP mean there is nothing to sync (`Ok`); Windows skips
      `FlushFileBuffers` for a handle that is not a disk file.
    - 223: an environment name that is empty or holds `=` is refused as one holding NUL (`Err(Io)`).
    - 239: dropping a `Child` without `wait` reaps it on POSIX: at once after a `kill`/`kill_tree`'s SIGKILL,
      otherwise without blocking, with a pending list swept at the next spawn or drop. No new function.
    - 175: the interrupt flag is a count; a socket wait returns early only for an interrupt the program has not yet
      seen through `interrupt_requested()`, and is still `TimedOut` then (no new `NetError` variant).
22. **Text, time and arguments.**
    - 051: `ParseError::text` of `Invalid(_)` is "invalid syntax" (was "not a number", wrong for JSON, TOML, dates and
      addresses); new `ParseError::describe(&e, text) : String` gives "line L, column C: unexpected 'x'" or "the text
      ends too early".
    - 057: new `DateTime::to_iso_local(&t) : String`, local wall time with its `+HH:MM` offset (cut to the minute), the
      same moment as `to_iso`. No `LocalDateTime` type.
    - 217: `from_iso` and `unix_ms_from_iso` give `OutOfRange` for a moment whose seconds do not fit an `i64`, so a
      parsed `DateTime` always converts (`[From-Iso]`).
    - 218: each part of a TOML header or dotted key counts as one level against the 512-level limit (`OutOfRange`).
    - 054: `Args` still reads only the documented forms; a declared option written in another form is named with the
      form to write ("`-I` takes its value as a separate argument: `-I inc`", "short options are written one at a
      time: `-v -o`", "`--verbose` takes no value").
23. **HTTP and TLS.**
    - 203: `form_decode`, and so `Url::query_pairs`, splits at `&` only; `;` is part of a name or value, as CPython's
      fix for CVE-2021-23336 and the WHATWG form encoding have it. No opt-in `;` mode: nothing needs it, and a program
      that wants it can split the text itself.
    - 202: the client accepts at most 16 interim (1xx) responses to one request; the 17th is `BadMessage`. A count
      was chosen over an overall deadline, which would be a new setting.
    - Outgoing messages `std` refuses (a bad header, a non-token method, a status outside 100–599) reuse
      `HttpError::BadMessage`, whose documentation says so; no new `HttpError` variant.
    - 237: `HttpServer::accept` no longer does the TLS handshake; the connection keeps the key and chain, and its
      first `request` or `respond` runs `TlsStream::server` (`Conn::Gone` if it fails). Chosen over a separate
      `accept_tcp` and `handshake` (new API, two steps), so that existing programs that accept and then spawn get the
      fix unchanged. The handshake keeps the server's `timeout_ms`; a separate handshake timeout would be a new
      setting, and a serial accept loop is held up by an idle client in plain HTTP too.
24. **X.509 (212, 213, 214).** Name constraints are enforced, the trust anchor's path length and an issuer's EKU
    apply, and chains are found by a depth-first search within 100 signature checks (Go's budget). Violations reuse
    `CertError::NotAuthority` (name constraints, path length; its text is now "an issuer may not issue the
    certificate: not a CA, or beyond its path length or name constraints") and `WrongUsage` (an issuer's EKU), so
    exhaustive matches over `CertError` need no new arm. Wildcard SANs are read literally against a constraint, as
    OpenSSL does; the check of the host closes the wildcard-into-excluded case.
25. **PostgreSQL (205, 206, 208, 209).**
    - New `PgError::Unsupported(str)`, for `COPY` (in either direction; the connection stays usable) and for a
      `client_encoding` other than UTF8 (the connection is closed, as JDBC does, so no text in another encoding is
      read or stored as UTF-8). `ProtocolViolation` was not reused: its text, "the server's message is malformed", is
      what report 208 complained of. An exhaustive `match` on `PgError` needs the new arm.
    - 206: a reply read that times out sends a `CancelRequest` and waits once more, so the outcome is the server's
      (57014 for a cancelled statement, or the result of one that completed first) and the connection stays usable;
      only a second timeout closes it with `PgNetFailure(TimedOut)`, documented as "the statement may have completed
      or not". New `PgConnection::set_timeout_ms(c, ms)` (0 for none), over new `TlsStream::set_read_timeout_ms` and
      `set_write_timeout_ms`; `connect`'s 30 s is per reply. Chosen over libpq's unlimited default because a bounded
      wait that cancels instead of misreporting keeps existing programs bounded and truthful; a statement-timeout
      argument would change `connect_with`'s signature.
    - 209: new `saslprep(StringView) : Option<String>` (`std::database`), RFC 4013 over Unicode 3.2, unassigned code
      points prohibited (checked against PostgreSQL, which stores such a password raw); SCRAM uses it, and the raw
      password when it gives `None`, as libpq does. Its tables are generated by `impl/tools/gen_saslprep.py` from
      Python's `unicodedata.ucd_3_2_0` and `stringprep`, with no library linked.
26. **Password hashes (215).** `verify_password` gives `false` for a stored hash with `t` above 2^32−1 or `m` above
    1,048,576 KiB (libsodium's largest preset) instead of faulting, and `argon2id` itself faults `kdf_parameters`
    above RFC 9106's 32-bit bounds (`[Password-Hash]`, `[Kdf-Parameters]`).
27. **No change for users.** `Rc`'s strong handles together hold one weak count, as `ArcBox` does, and
    `RcBox::release_weak` frees the box (111). The runtime's mutexes held and channel event counts are in 64 striped
    locks; the waiting paths and the deadlock detector are unchanged (132).

## 3. Not defects, or documented limits

- **153.** The test's label is printed when it starts, deliberately (a slow test shows which it is); the book said
  otherwise and now describes it.
- **083.** `[Retain]` was right (removed elements are destroyed by a truncate from the last place); the documentation
  of `retain` and `dedup` said "last first" of the original order. Reordering would need a permutation pass for a
  low-severity case.
- **172.** `Child::wait` does not drain the child's pipes; `wait`, `Command::spawn`, `Child`, `[Wait]` and the guide
  say so.
- **173.** `kill_tree` cannot reach a double-forked daemon. A process group was not used: `setpgid` would take the
  child out of the terminal's foreground group, so Ctrl-C would stop reaching it and a child reading the terminal
  would get SIGTTIN. The documentation no longer says "every process".
- **140.** Two `--lib` libraries in one C program (§2.20).

## 4. Consequences

- Programs newly refused: `resource enum` and a `drop` on an enum (§2.3, which never ran); `String::append` of an enum
  (§2.6, which crashed `cobc`); a closure stored in a container that outlives a capture (§2.9, which faulted at the
  run); a binding made from a view of a `match` temporary used after the `match` (§2.9, unsound); a resource part moved
  out through a reference, a variant called with the wrong number of values, a missing `return` after a diverging
  branch, two fields of one name, a wrong `extern fn` argument and a suffixed literal too large for its parameter
  (`CHG-0479`), each of which faulted, wrapped or could not be built.
- Programs newly accepted: those of §2.2, §2.4, §2.5, §2.7, §2.8, §2.10, §2.11, §2.13, and the correct programs
  `CHG-0479` lists.
- New warnings: `diag.comparison-constant`, `diag.no-effect` (§2.1).
- `std` gains `HashMap::contains_ref`, `HashMap::get_ref`, `HashSet::contains_ref`, `Vec::contains_ref`,
  `read_link`, `list_dir_bytes`, `File::create_private`, `make_temp_dir`, `DateTime::to_iso_local`,
  `ParseError::describe`, `saslprep`, `PgConnection::set_timeout_ms`, `TlsStream::set_read_timeout_ms`,
  `TlsStream::set_write_timeout_ms`; `FileError` gains `NotEmpty`, `LinkLoop`, `SameFile` and `PgError` gains
  `Unsupported`; `ThreadFailure` is a resource.
- Behaviour that changes for running programs: a writer to a pipe whose reader has gone ends (§2.14); `main`'s end
  waits for the threads still running (§2.16); an unwind continues past a faulting destructor (§2.15); `;` no longer
  splits a query string (§2.23); a timed-out PostgreSQL statement is cancelled (§2.25); `cobc` keeps a cache of
  executables (§2.19).

## 5. Verification

See `CHG-0479`.
