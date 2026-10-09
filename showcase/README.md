# CobaltC showcases

Eighty-four small, complete programs, each chosen to show something
that is particular to CobaltC rather than to re-solve a textbook
exercise. Each prints deterministic output (only Tier 8 and `hashsum` read standard
input, and then from a checked-in file),
and each is run under **both** implementations — the interpreter `coby` and the
native compiler `cobc` — against a checked-in expectation:

    ./run_all.sh          # runs every showcase under coby, and cobc with GCC and with Clang, and reports the totals

Each program's header comment says what it demonstrates; its expected
output is the `.out` file beside it (the last line is the exit status).
A Tier 5 to 9 program, `logrotate` in Tier 10 and `hashsum` in Tier 11, is a command-line tool: its `.args` file lists the
runs, one per line, with the arguments each gets, and its `.out` holds
every run's output (standard output and standard error together) and
exit status in turn. `$TMP` in an `.args` line is a new, empty directory
for that run, for a tool that writes files, and a line ending in
`< FILE` gives that run FILE as its standard input, and one beginning
with `NAME=value` words gives it those environment variables.
Run one yourself from its directory:

    cd tier2
    ../../impl/target/release/coby scope_destruction.cb
    ../../impl/target/release/cobc --run scope_destruction.cb

Every program begins `import std;` to use the standard library by
its short names (`printf`, `Vec`, `Some`, …), and writes its output with
`printf`, C's formatted output checked when compiled, whose `%v` writes
any number, text (`str`, `&String`, `StringView`), `bool`, or an enum's
variant name as CobaltC does (D-0038, D-0039, D-0100). `hex_printer` and
`ascii_mandelbrot` also write raw bytes, and `checked_write` calls the
`stdout_write` extern directly to check its claim.

## Tier 1 — essentials

| Showcase | What it shows |
|---|---|
| `hello.cb` | A string literal is a `str`: a plain value, UTF-8 by construction; `printf` needs no `unsafe` |
| `array_sum.cb` | Bounds-checked fixed arrays; overflow never wraps — `checked_add` turns it into an `Option` |
| `mutable_vector.cb` | Mutation only through an exclusive borrow (`&mut`); no `mut` on variables; `pop` returns `Option` |
| `fibonacci.cb` | Checked arithmetic finds the largest Fibonacci number a `u64` holds, exactly: `while (Some(next) = checked_add(a, b))` runs until it would not fit |
| `fizzbuzz.cb` | An enum with a payload and an exhaustive `match` |
| `generic_function.cb` | Bound-free generics, inference from arguments and expected types, an elided reference return |
| `enum_calculator.cb` | Data-carrying enums; division by zero and overflow as `Result` values |
| `result_propagation.cb` | `?` propagation through a parser, with `Option::ok_or` turning checked arithmetic's `None` into an error; the error enum records where it failed |
| `struct_geometry.cb` | Plain structs copy on assignment and compare field by field with `==`; `fn Type::name` |
| `string_processing.cb` | `str` versus `String`; UTF-8 validation is a `Result` naming the bad byte; bytes versus characters (`String::chars`) |

## Tier 2 — the language's identity

| Showcase | What it shows |
|---|---|
| `disjoint_borrows.cb` | Two exclusive borrows at once into different fields or literal array elements |
| `sort_vec.cb` | In-place insertion sort under the aliasing rules; sorting records by a key |
| `vec_of_vecs.cb` | Resources inside resources: rows move into the outer `Vec`, all freed together; `printf`'s `*` width centres the triangle |
| `expression_tree.cb` | A recursive structure without pointers: an arena `Vec` indexed by `usize`, errors as `Result` |
| `closure_capture.cb` | Checked capture lists; borrowing versus `move` capture; a closure owning a resource destroys it |
| `reference_counting.cb` | `Rc<T>` is library code over raw memory; the value is destroyed once, by the last owner, and a `Weak<T>` observer finds it alive until then and gone after |
| `mutex_counter.cb` | Shared mutation only through a `mutex`; the guard unlocks when its block ends |
| `parallel_computation.cb` | Threads with no shared state, results through `join`; parallel under `cobc` |
| `resource_return_threads.cb` | Owned data moves into a thread and owned `String`s come back through `join` |
| `tree_traversal.cb` | A binary search tree in an arena, `Option<usize>` instead of null, recursive traversals with `if (Some(i) = at)` |
| `scope_destruction.cb` | Deterministic destruction: reverse order at block end, `drop`, moves, statement temporaries |
| `file_modules.cb` | Modules whose bodies live in their own files (`file_modules/`), nested file modules, privacy across files, `import` of a module |

## Tier 3 — a real language

| Showcase | What it shows |
|---|---|
| `matrix_multiplication.cb` | Arrays as values with sizes in their types; matrix powers with overflow caught by `?` on `Option` |
| `prime_sieve.cb` | A sieve over `Vec<bool>`: built through `&mut`, read through `&` |
| `ascii_mandelbrot.cb` | IEEE-754 floats with explicit conversions; identical output from interpreter and native code |
| `ascii_sierpinski.cb` | CobaltC's operator precedence: `x & y == 0` means `(x & y) == 0` |
| `mini_interpreter.cb` | A bytecode VM whose every failure (underflow, overflow, bad jump) is a value |
| `tiny_serializer.cb` | A wire format with its byte order spelled out (`Vec::push_le`, `read_le`); every read checked first, so a short record is a typed decode error, not a fault |
| `stack_abstraction.cb` | Encapsulation: an exported generic type with a private field, holding values or resources |
| `hex_printer.cb` | Defined representations — little-endian integers, IEEE floats, struct layout — seen byte by byte |
| `hash_map.cb` | How a hash table is built (programs use `std`'s `HashMap`): `String` keys, any value type, `String`s included; FNV-1a needs `wrapping_mul`, and an empty slot is `max_value<usize>()` |
| `big_integers.cb` | Exact `100!`, `2^256` and `fib(300)` in `u32` limbs, every carry an explicit `widen`/`narrow_wrapping`; a resource updated in place |
| `shortest_paths.cb` | Dijkstra with `std`'s `PriorityQueue`: a struct of keys is a key, so the queue needs no comparison function; `Option<u64>` distances, `checked_add` |
| `maze.cb` | Breadth-first search with `std`'s `Queue` (`push_back`, `pop_front`): squares reached in order of distance, so the first path found is a shortest one; each square's predecessor an `Option<usize>`, and no way out a `None` |
| `sudoku.cb` | Arrays are values: each guess copies the board, so backtracking needs no undo; candidate digits as bit sets, counted with `count_ones` |
| `crc_rle.cb` | CRC-32 and run-length coding with fully defined bit operations, checked against `std`'s `crc32`; the checksum catches a corrupted copy |
| `parallel_sort.cb` | Merge sort whose halves move into threads with `spawn` and back with `join`: parallel with nothing shared; the data from a seeded `Rng` |
| `parallel_quicksort.cb` | Several threads sorting one `Vec` in place: after partitioning, the two sides are two exclusive slices of the same buffer, sorted by two threads at once — a slice borrows only its own range (D-0070) |
| `pipeline.cb` | Threads passing owned values through bounded `Channel`s: a `String` sent is moved in and out, never shared; `close` ends each stage's loop, and a late `send` gives its value back |
| `tinyos.cb` | A small kernel on simulated hardware (`tinyos/`): page allocator, page tables, a CPU running machine code (its op codes an `enum Op : u8`, decoded by `Op::from_code`), round-robin scheduling, system calls numbered by an `enum Syscall : u8`, a blocking pipe, a filesystem; the kernel owns everything, and a use-after-free is a page fault |

## Tier 4 — unsafe and foreign functions

What CobaltC lets you assert when it cannot check — and how little of
the language that switches off. These call real C functions from libc
and libm (x86-64 Linux), which both implementations now do.

| Showcase | What it shows |
|---|---|
| `ffi_math.cb` | `unsafe extern fn` declarations of libm functions; results are claims, checked where it matters |
| `ffi_claims.cb` | The trust transition: values from C's `rand` become typed dice only through a validator (`checked_narrow`, and `Die::from_code` for an `enum Die : u8`) |
| `raw_linked_list.cb` | Manual memory: `allocate` as a `Result`, nodes written and walked through raw pointers, freed by hand |
| `arena_allocator.cb` | A bump allocator as a `resource struct` whose destructor frees the block; alignment, `copy_raw`, trusted pointer arithmetic |
| `vec_from_scratch.cb` | A growable vector written from raw memory, used beside `std::Vec` with the same results; why `cobc` gives `std::Vec` native code but compiles this one as written |
| `float_anatomy.cb` | An `f64`'s defined bits read through a raw pointer, taken apart by a `bitstruct`, and rebuilt exactly with C's `ldexp` |
| `checked_write.cb` | `std`'s one real extern, `stdout_write`, called directly with its claim checked |

The interpreter's memory is simulated, so it cannot hand a pointer to
C; these showcases pass only integers and floats across the boundary.
The compiler can pass pointers too.

## Tier 5 — command-line tools

Programs that take arguments and report through their exit status, as
tools do: reading arguments with `arg`, numbers with `parse`, files
with `read_file`, building output with `String::append`, writing
errors to standard error with `eprintf`, and returning `u8` from
`main`. Each runs
several times (`<name>.args`), including runs that fail on purpose,
under both implementations:

    cd tier5
    ../../impl/target/release/coby calc.cb "2 * (3 + 4.5) / -0.5"
    ../../impl/target/release/cobc --run table.cb "item,qty,price" "apple,3,0.5"

| Showcase | What it shows |
|---|---|
| `stats.cb` | Numbers from the arguments: count, min, max, sum, mean, median, sorted by `Vec::sort_by` with the order as a closure (an `f64` has none of its own); a bad argument named with the byte where it stops being a number; exit 1 for bad input, 2 for none |
| `calc.cb` | A recursive-descent evaluator over a `String`'s bytes, compared with byte characters (`b'('`); `?` carries every error up; a caret under the failing column (from `parse`'s `Invalid` offset); a different exit status per kind of failure |
| `convert.cb` | An integer in decimal, hex and binary, and exactly which integer types hold it: one generic `String::parse<T>` call per type decides, with nothing to overflow, even at `i128`'s minimum |
| `wc.cb` | Lines, words and bytes of the files named, as Unix `wc` counts them: `read_file` over checked-in `wc_data/`, a missing file and a file that is not UTF-8 each reported by its `FileError` (`FileError::text`) without stopping the rest, exit 1 if any failed; aligned columns with `printf` |
| `table.cb` | CSV rows as arguments, split with `String::split` into `Vec<Vec<String>>`; `parse`'s strictness rejects `-2` as a `u32` and ` 1.25` as a number; `checked_add` guards the total; columns sized in characters (`String::chars`) and aligned with `printf`'s `%-*s`, so accented text lines up |

## Tier 6 — collections at work

Tools built on `std`'s `HashMap` and `HashSet`, `foreach`, and
`printf`/`sprintf`/`eprintf`, reading checked-in files under `data/`.
Maps and sets keep insertion order, so every output is the same on
every run and in both implementations. Each runs several times
(`<name>.args`), including runs that fail on purpose:

    cd tier6
    ../../impl/target/release/coby wordfreq.cb data/harbour.txt 5
    ../../impl/target/release/cobc --run build_order.cb data/project.deps

| Showcase | What it shows |
|---|---|
| `wordfreq.cb` | Word counts of a file with `*HashMap::entry(&mut counts, w, 0) += 1`; words taken from a `Vec<String>` by a consuming `foreach`, each moving into the map or dropped; `Vec::sort_by` with a closure that borrows the map, stable, so equal counts keep first-appearance order; a bar chart with `printf` widths |
| `anagrams.cb` | A `HashMap<String, Vec<String>>` whose values are resources, found or created by `entry`, keyed by each word's characters sorted (`String::chars`); removing while looping over a map is a borrow conflict, so singletons are collected, then removed with `remove`, which keeps the groups in order |
| `inventory.cb` | A ledger applied to a `HashMap<String, i64>`: a malformed line or an oversell reported on standard error with its line number, the rest applied; an item at zero removed and remembered in a `HashSet`; exit 1 when anything was skipped |
| `build_order.cb` | A dependency graph as `HashMap<String, Vec<String>>`, depth-first with two `HashSet`s as marks, printing an order in which everything follows its dependencies; a cycle reported on standard error with the modules on it |

Writing Tier 6 found two faults in `coby`, both fixed, each with a
conformance case; `cobc` already behaved correctly:

- `&mut *f(k)` evaluated `f(k)` twice (once to check whether it crossed
  a shared reference), so `*HashMap::entry(&mut m, key, 0) += 1` moved
  `key` twice.
- A statement left by `return` kept its temporaries: after
  `match (Some(r)) { Some(x) : { return 1; }, … }`, the temporary
  `Option` still counted as a reference, and destroying the collection
  it pointed into later faulted.

It also showed where the language was awkward, and each of those was
then resolved (D-0044): `String::clone` copies a `String` (it had
taken `sprintf("%s", &s)`); `String::push_ascii` adds one ASCII byte;
`Vec::clear` and `String::clear` empty a buffer in place (a `Vec` had to
be dropped and replaced); destructuring takes every field of a struct
apart, `Entry { item, change } = entry;` (only a single-field struct
could be before, so a resource field could not leave a larger one); and
`foreach (i, k, v in &m)` gives a map entry's position with its key and
value. The programs above use them.

## Tier 7 — files as byte streams

Tools built on `File` (D-0054): files opened, read and written in
pieces, at any position, as bytes or as lines, and closed by their
destructors or by `close` when the result matters, with nested and literal patterns
(D-0056, D-0057) taking every `Result<Option<…>>` apart in one `match`. They
read checked-in files under `data/` and write into `$TMP`:

    cd tier7
    ../../impl/target/release/coby hexdump.cb data/tiny.png
    ../../impl/target/release/cobc --run records.cb /tmp

| Showcase | What it shows |
|---|---|
| `hexdump.cb` | A binary file (a real PNG) as `xxd` shows it: 16 bytes at a time into one reused `Vec<u8>` (`File::read` appends, `Vec::clear` keeps the buffer), from an offset reached with `File::seek`, sized with `File::len`; `Ok(0)` for the end of the file, `Err(NotFound)` and `Err(Denied)` as arms of one `match` |
| `pngchunks.cb` | Random access in a binary format: the chunk list found by seeking over each chunk's data without reading it, then every chunk's CRC-32 checked by reading it (`crc32`, lengths by `read_be`); a copy with one byte changed, written with `write_bytes`, is caught by the check |
| `logscan.cb` | Logs read a line at a time with `File::read_line`; `Ok(Some(line))`, `Ok(None)` and `Err(Utf8(e))` in one `match`, a line that is not UTF-8 reported and skipped; ERROR lines written to a report with `File::create`/`write_text` and closed with `File::close`, each log's summary added to a history opened with `File::append` |
| `records.cb` | A file as a database of 32-byte records at `32 * k`, opened with `File::open_rw`: records encoded little-endian (`Vec::push_le`, `read_le`) and written, a sale that rewrites 4 bytes in the middle of the file and nothing else, a new record at `File::len`; lookups return `Result<Option<Record>, FileError>` |
| `wavsynth.cb` | Sound saved as WAV files any player opens: three tones synthesised as `i16` samples (a sine by recurrence, no trigonometry library), mixed with `saturating_add` (the clipped samples counted), smoothed by a Q15 fixed-point low-pass filter (an arithmetic `>>` of a negative `i32`); the 16-bit file streamed through a `File` with its RIFF sizes left zero and patched afterwards by `File::seek`, the 8-bit one written whole by `write_bytes`; read back by walking its chunks (`read_le`), the samples on disk compared with those in memory |

Writing Tier 7 found one fault in `coby`, fixed with a conformance
case: `match (f()?)` faulted, because the value `?` takes out of `Ok`
had no type for `match` to read its variant from; `cobc` was right.

## Tier 8 — filters on standard input

Tools that read standard input, as filters in a pipeline do: a line at
a time with `read_line`, whose `Result<Option<String>, FileError>` is
taken apart in one `match` — a line, the end of the input, a line that
is not UTF-8 (reported and stepped over) — or as raw bytes with the
`stdin_read` extern beneath it. A line ending in `\r\n` reads exactly as
one ending in `\n` (D-0092). Each `.args` line ends with `< FILE`, a
checked-in file under `data/` that the run reads as its standard input:

    cd tier8
    ../../impl/target/release/coby uniq_count.cb -i < data/fruit.txt
    ../../impl/target/release/cobc --run grep_lite.cb -n ferry < data/harbour.txt

| Showcase | What it shows |
|---|---|
| `uniq_count.cb` | `sort \| uniq -c \| sort -rn` in one filter: lines counted in a `HashMap` as they arrive, the end of input a value (`Ok(None)`), not a sentinel; a file with CRLF line endings counts exactly as one without; lines that are not UTF-8 skipped with their line number, and the exit status says so |
| `grep_lite.cb` | grep's contract — exit 0 when a line was selected, 1 when none was, 2 on trouble even if lines were also selected — with `-i`, `-v`, `-n` and `-c`; options gathered into a struct first, where filling a `String` field that already holds one is `overwrite`, never a silent `=` |
| `rot13.cb` | Standard input as bytes, not lines: `stdin_read` into a raw buffer owned by a `resource struct`, its claim checked before any byte is trusted, and `stdout_write`'s claim checked on the way out; `\r`, bytes that are not UTF-8 and a missing last newline pass through unchanged, whatever the chunk size |
| `rpn.cb` | A reverse-Polish calculator that survives bad input: each line's failure — an unknown word, too few values, division by zero, an overflow caught by `checked_*` and `Option::ok_or` — is an error enum carrying its column, printed under a caret, and the next line is read as if nothing happened |

Writing Tier 8 found no fault in either implementation. It did show one
small friction: a `String` does not compare with a string literal, so an
option is tested as `&a[0..$] == "-i"`, a view of it, rather than
`a == "-i"`.

## Tier 9 — directories and the environment

Tools that work with the filesystem beyond one file, and with the
environment they run in: `list_dir`, whose names come sorted by their
bytes so every listing is the same everywhere; `path_kind`, whose
`File(n)` carries a file's size; `make_dir`, `rename`, `remove_file`
and `remove_dir`, each failure a `FileError`; and `env_var` and
`current_dir`. A run's `.args` line may begin with `NAME=value` words,
its environment variables:

    cd tier9
    ../../impl/target/release/coby tree.cb -s data/site
    BANNER_CHAR=═ ../../impl/target/release/cobc --run banner.cb "Harbour notes"

| Showcase | What it shows |
|---|---|
| `tree.cb` | `tree` and `du` in one: a walk of `data/site/` builds a tree of `Node`s, each directory owning a `Vec` of its children, so a directory's total is known before its line is printed; `-s` sorts by size with a stable sort, so ties keep `list_dir`'s order; sizes are `u64`, every sum a `checked_add` |
| `safe_save.cb` | Saving without ever leaving a half-written file: write `NAME.tmp`, then `rename` it over the old file; a save interrupted before the rename leaves the old contents whole, and the stray `.tmp` is found with `list_dir` and removed when the program starts again; `make_dir` tells "made" (`true`) from "already there" (`false`), and `remove_dir` refuses a directory that is not empty |
| `banner.cb` | A tool configured by environment variables with defaults: `env_var`'s `Ok(Some(v))`, `Ok(None)` (not set) and `Err(e)` (not UTF-8) in one `match`, so "not set" is never "set to nothing"; every setting checked and a bad one named with its value; widths counted in characters, so a `═` border lines up; with no text, the name of the current directory |

Writing Tier 9 found no fault in either implementation.

## Tier 10 — deployed

What a program needs once it runs as a service rather than a one-off
tool: dates for its logs and file names (`std::time`, UTC only), paths
built and taken apart by the platform's rules and files asked about
(`path_join`, `file_info`, `copy_file`, `make_dir_all`,
`remove_dir_all`), other programs run without a shell (`std::process`),
connections served by a thread each (`std::net`), and secrets drawn from
the operating system (`os_random_bytes`). Each prints only what is the
same on every run — fixed dates, loopback port 0, facts about random
bytes rather than the bytes. They use Unix programs (`/bin/sh`, `sort`,
`rev`, `sleep`) and Unix paths, so they run on Unix systems:

    cd tier10
    ../../impl/target/release/coby echo_server.cb
    ../../impl/target/release/cobc --run logrotate.cb /tmp/somewhere-empty

| Showcase | What it shows |
|---|---|
| `logrotate.cb` | A day's log named from a `DateTime` and placed with `path_join`; five days written into a scratch directory, the newest three kept and the rest copied into `archive/` (`make_dir_all`, `copy_file` and its byte count) and removed; `path_extension` and `path_stem` read names back, `file_info` gives sizes, `remove_dir_all` cleans up |
| `echo_server.cb` | A TCP server and three clients in one program: loopback port 0, a thread per accepted connection with the `TcpStream` moved into it, client threads reporting through a `Channel` and printed in client order; every blocking call lets the other threads run, under `coby` too |
| `pipeline.cb` | Programs run with no shell between: a pipeline asks for `/bin/sh -c` by name and passes the user's text as `$1`, where `; echo injected` stays text; `output_with_input` feeds standard input; statuses as exit codes (`3`, `7`) and minus the signal (`-9` after `kill` of a running `sleep`, found running by `try_wait`); a missing program is `NotFound` |
| `token.cb` | Secrets from `os_random_bytes`, never from `Rng`; the program prints only what holds of every run (a token's length, that two differ); `Rng::new(42)` repeats, `Rng::from_os()` shuffles differently each run |

Writing Tier 10 found no fault in either implementation. It showed two
small frictions: `Vec::sort` does not take `StringView`s (their order
is passed as a two-line function of references to `Vec::sort_by`), and
text for a child's standard input goes through a `String` to become the
bytes `output_with_input` takes.

## Tier 11 — secured

Hashes, message authentication, encryption, key agreement, signatures,
password hashing, big numbers, keys and certificates, all from `std`
and all written in CobaltC (`std::crypto`, `std::math`'s `BigUint`,
`std::x509`), the pieces a TLS 1.3 connection (`std::tls`) is made of.
Where an RFC publishes a worked example, the program reproduces it byte
for byte, and every other signature and hash was checked against
openssl or Python's `cryptography` and `argon2-cffi`. Every key, salt
and date is fixed so the output is; real keys come from
`os_random_bytes` (`ed25519_private_key`, `ec_private_key`,
`rsa_generate_key`), real password hashes from `hash_password`, and real
certificates from the network.

    cd tier11
    ../../impl/target/release/cobc --run sealed_box.cb
    ../../impl/target/release/coby hashsum.cb -a sha384 < data/abc.txt

| Showcase | What it shows |
|---|---|
| `hashsum.cb` | A checksum tool over standard input (`read_all_bytes`): SHA-256, SHA-384 or SHA-512 through one `hash(kind, data)`, in hex or base64; a bad option is a message and `exit(2)` from where it is found |
| `webhook.cb` | Webhook signatures as payment services send them: `sha256=` and the hex of HMAC-SHA-256; checked with `digest_eq`, which takes the same time however much of a forged signature is right; a changed body, another secret, a missing prefix, non-hex and a short signature each refused with the reason |
| `sealed_box.cb` | Two parties agree on a secret with X25519 (RFC 7748's example keys), derive a key per direction with HKDF, and exchange ChaCha20-Poly1305-sealed messages with counter nonces and a clear-text header; a flipped bit, a changed header, a replay under another counter and the wrong key are refused |
| `bignum.cb` | `BigUint`: 52! and the 300th Fibonacci number exactly, a Fermat test of Mersenne numbers up to 2^521 - 1, and a textbook RSA round trip; `overwrite` replaces a running value, a `BigDivision` is destructured |
| `jwt.cb` | JSON Web Tokens signed four ways: HS256 and EdDSA reproduce RFC 7515 A.1 and RFC 8037 A.4 exactly, ES256 is deterministic (RFC 6979) with ECDSA's DER converted to and from JWS's raw r ‖ s by `der_read`, RS256 uses an RSA key read from openssl's PEM; expiry, a changed claim, `"alg": "none"`, HS256 keyed with the public key, and a token of another algorithm are refused because the verifier knows its key's kind |
| `release_signing.cb` | Signing a release manifest of SHA-256 digests with an Ed25519 key from openssl's PKCS #8 PEM, its public key exported as the PEM users pin; a replaced file, a manifest rewritten to match, and a missing file are caught. A backup P-256 key in SEC 1 form is converted to PKCS #8 byte for byte as `openssl pkcs8 -topk8` does and countersigns with ECDSA; each signature fails under the other's key |
| `password_store.cb` | Argon2id password records as PHC strings that carry their own parameters and salt, checked with `verify_password`; a legacy PBKDF2 record upgraded at its next good login (the `overwrite` placed after the view of the old record ends, which CobaltC's alias check enforces); unknown names do the same work as wrong passwords; a hash written by Python's argon2-cffi verifies; API tokens stored as keyed BLAKE2b digests |
| `token_gate.cb` | An authenticated API over `std::http`: Argon2id account records checked with `verify_password`, a login that issues an Ed25519-signed JWT, and a `/secret` route that admits a `Bearer` token only when its signature verifies and it has not expired, answering 401 with a `WWW-Authenticate` reason otherwise; a token with edited claims, an expired one and a second user walk through it |
| `certcheck.cb` | An openssl-made certificate authority embedded as PEM (EC P-384 root, RSA-PSS-signing intermediate, two server certificates): each summarized, then the chain checked at a fixed date for DNS names, a wildcard, IP addresses, an expired certificate, a missing intermediate and no trusted roots |

Writing Tier 11 found no fault in either implementation. It found two
frictions, both fixed before the tier was finished: a program could not
take the bytes of a `StringView` or a literal as a slice to hash
(`StringView::as_bytes`, D-0163), and a `String` could not be compared
with `==` to a `str` (D-0164). A `str` value, unlike a literal, still
needs `String::from_str` to become a `String`, as D-0006 intends.

The four later programs (`jwt`, `release_signing`, `password_store`,
`token_gate`, D-0166–D-0174) found no fault either. Under the interpreter they take
about a minute each, most of it P-256, RSA and Argon2 arithmetic, which
the compiled programs do in milliseconds.

## Tier 12 — connected

HTTP, both ends, from `std::http` (D-0173, D-0174): a client that fetches
and posts, follows redirects and decodes chunked bodies, and a server that
parses requests and writes responses, keeping a connection as the client
asks. Each program is its own client and server on loopback port 0, so it
needs no network and prints the same thing every run; pointed at a real
host, the same client code speaks `https` through `std::tls`. Under the
interpreter each takes a few seconds; `static_site` takes the scratch
directory it is given:

    cd tier12
    ../../impl/target/release/cobc --run tiny_api.cb
    ../../impl/target/release/coby static_site.cb /tmp/somewhere-empty

| Showcase | What it shows |
|---|---|
| `tiny_api.cb` | A key-value service: an accept thread, a handler thread per connection with the `HttpConnection` moved into it, the store a `mutex<HashMap<String, String>>` shared by reference and reachable only through `lock`'s guard; PUT, GET, DELETE and a listing routed on method and path, 201/200/204/404 told apart, 405 with an `Allow` header; the client is `http_get`, `http_post` and an `HttpClient` for the rest |
| `static_site.cb` | Files served without serving the wrong ones: the target percent-decoded and split, any `..` segment refused with 400, a directory serving its `index.html`, the content type from the extension, `HEAD` with headers only, 404 for the rest; paths built with `path_join` so the same program serves on Windows |
| `dns_over_http.cb` | A binary protocol over HTTP, RFC 8484 style: DNS messages (RFC 1035 wire format) by `POST` as `application/dns-message` or by `GET ?dns=` in base64url, a tiny authoritative resolver answering A records and NXDOMAIN with name-compression pointers, and a client encoding questions with `Vec::push_be` and decoding answers with `read_be`; a text body is 400 and the wrong content type 415. The same codec against a public resolver over `https` is a DNS-over-HTTPS client |

Writing Tier 12 found no fault in either implementation, and one thing to
know: `handle` is a reserved word (the thread handle type), so a function
cannot be called that.

## Tier 13 — supervised

Threads the way D-0204 means them to be used: a fault in one thread ends
that thread only, its failure is a value the supervisor takes with
`try_join`, `cancel` stops a waiting thread, and a deadlock is a
diagnostic instead of a hang. Each program prints the same thing every
run, in both tools:

    cd tier13
    ../../impl/target/release/cobc --run supervised_dns.cb
    ../../impl/target/release/coby --explore 20 deadlock_found.cb

| Showcase | What it shows |
|---|---|
| `supervised_dns.cb` | A DNS server over TCP, framed as DNS over TLS frames it (RFC 7766), one handler thread per connection, supervised: a handler with a bug (it faults on an `ANY` query) fails alone, the supervisor logs the failure from `ThreadFailure::text` and keeps serving the other clients; at shutdown the idle handlers are cancelled in their socket reads |
| `first_answer.cb` | "First answer wins" without `select`: one thread per resolver sending to one channel, `cancel` for the rest once an answer is in, each cancelled wait ending at once |
| `deadlock_found.cb` | Two threads locking two mutexes in opposite orders: the wait that would close the cycle faults with `diag.deadlock`, contained; the other then finds the failed thread's mutex poisoned (`diag.mutex-poisoned`); `--explore` shows every schedule ends the same way |
| `parallel_chunks.cb` | `slice_parts` gives each thread a disjoint exclusive run of one vector; the threads write at once with no lock, and the checker proves they cannot overlap |

## Found along the way

Writing these exercised both implementations in ways the conformance
suite had not, and turned up defects that are now fixed, each with a
conformance case (`impl/STATUS.md`):

- `coby` never destroyed the value inside an `Rc` (or a resource field
  of any `Vec` element): a reclaimed object's value was read from its
  record, which holds a placeholder, instead of its raw cells.
- `coby` never ended a temporary that was read through (`make().name`);
  it now ends such temporaries with their statement, as `[Stmt-Exit]`
  requires.
- `coby` gave the elements of a nested array literal no expected type,
  so `-1` in `array<array<i64, 3>, 3>` became an `i32`.
- `cobc` did not compile `?` applied to an `Option`.
- Neither tool called real C functions: `cobc` refused every extern but
  `write`, and `coby` silently returned zero. Both now call libc and
  libm (Tier 4).
- `coby` could not access a field through a raw pointer, `(*p).f`.

Tier 5 was written with the then-new arguments, exit status, `parse`
and `String::append`, and changed the language itself (D-0033): a
variable whose value was moved away or dropped can now be given a new
one (`cur = Vec::new();`, and `v = f(v)`, which used to fault); `b'x'`
is a `u8` byte literal, in place of `44` for a comma; float literals
take an exponent (`1.0e-7`, as `%v` writes them); and
`Option::unwrap_or` / `Result::unwrap_or` replace a four-line `match`.
Along the way: `coby` did not round a float literal typed `f32` by its
context, and both tools let a float literal round to infinity.

Bringing every showcase onto the newer features (`foreach` ranges,
pattern conditions, `ok_or`, enum codes, `bitstruct`, the byte helpers,
`String::chars`, …) found two more faults in `coby`, both fixed with a
conformance case; `cobc` was right each time:

- A variant of a generic enum passed where the parameter is a type
  parameter — `None`, `Ok(5)`, `Some(None)` — took a defaulted type
  instead of the one its context gives, so `Vec::filled(3, None)` for a
  `Vec<Option<usize>>` laid its elements out at the wrong size and they
  overlapped (`shortest_paths`).
- A slice taken out of an `Option` or `Result` by `?` had no type to be
  indexed by, so `need(b, at, n)?[0]` faulted (`tiny_serializer`).
