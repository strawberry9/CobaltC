# CobaltC showcases

Fifty-seven small, complete programs, each chosen to show something
that is particular to CobaltC rather than to re-solve a textbook
exercise. None reads standard input, each prints deterministic output,
and each is run under **both** implementations — the interpreter `coby` and the
native compiler `cobc` — against a checked-in expectation:

    ./run_all.sh          # runs every showcase under coby, and cobc with GCC and with Clang, and reports the totals

Each program's header comment says what it demonstrates; its expected
output is the `.out` file beside it (the last line is the exit status).
A Tier 5, 6 or 7 program is a command-line tool: its `.args` file lists the
runs, one per line, with the arguments each gets, and its `.out` holds
every run's output (standard output and standard error together) and
exit status in turn. `$TMP` in an `.args` line is a new, empty directory
for that run, for a tool that writes files.
Run one yourself from its directory:

    cd tier2
    ../../impl/target/release/coby scope_destruction.cb
    ../../impl/target/release/cobc --run scope_destruction.cb

Every program begins `import std;` to use the standard library by
its short names (`printf`, `Vec`, `Some`, …), and writes its output with
`printf`, C's formatted output checked when compiled, whose `%v` writes
any number, text, `bool` or `&String` as CobaltC does (D-0038, D-0039).
`hex_printer` alone writes raw bytes, through `std`'s `write` extern.

## Tier 1 — essentials

| Showcase | What it shows |
|---|---|
| `hello.cb` | A string literal is a `str`: a plain value, UTF-8 by construction; `printf` needs no `unsafe` |
| `array_sum.cb` | Bounds-checked fixed arrays; overflow never wraps — `checked_add` turns it into an `Option` |
| `mutable_vector.cb` | Mutation only through an exclusive borrow (`&mut`); no `mut` on variables; `pop` returns `Option` |
| `fibonacci.cb` | Checked arithmetic finds the largest Fibonacci number a `u64` holds, exactly |
| `fizzbuzz.cb` | An enum with a payload and an exhaustive `match` |
| `generic_function.cb` | Bound-free generics, inference from arguments and expected types, an elided reference return |
| `enum_calculator.cb` | Data-carrying enums; division by zero and overflow as `Result` values |
| `result_propagation.cb` | `?` propagation through a parser; the error enum records where it failed |
| `struct_geometry.cb` | Plain structs copy on assignment and compare field by field with `==`; `fn Type::name` |
| `string_processing.cb` | `str` versus `String`; UTF-8 validation is a `Result` naming the bad byte |

## Tier 2 — the language's identity

| Showcase | What it shows |
|---|---|
| `disjoint_borrows.cb` | Two exclusive borrows at once into different fields or literal array elements |
| `sort_vec.cb` | In-place insertion sort under the aliasing rules; sorting records by a key |
| `vec_of_vecs.cb` | Resources inside resources: rows move into the outer `Vec`, all freed together |
| `expression_tree.cb` | A recursive structure without pointers: an arena `Vec` indexed by `usize`, errors as `Result` |
| `closure_capture.cb` | Checked capture lists; borrowing versus `move` capture; a closure owning a resource destroys it |
| `reference_counting.cb` | `Rc<T>` is library code over raw memory; the value is destroyed once, by the last owner |
| `mutex_counter.cb` | Shared mutation only through a `mutex`; the guard unlocks when its block ends |
| `parallel_computation.cb` | Threads with no shared state, results through `join`; parallel under `cobc` |
| `resource_return_threads.cb` | Owned data moves into a thread and owned `String`s come back through `join` |
| `tree_traversal.cb` | A binary search tree in an arena, `Option<usize>` instead of null, recursive traversals |
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
| `tiny_serializer.cb` | Explicit width conversions for a wire format; typed decode errors; `\|` binds tighter than `<<` |
| `stack_abstraction.cb` | Encapsulation: an exported generic type with a private field, holding values or resources |
| `hex_printer.cb` | Defined representations — little-endian integers, IEEE floats, struct layout — seen byte by byte |
| `hash_map.cb` | How a hash table is built (programs use `std`'s `HashMap`): `String` keys, any value type, `String`s included; FNV-1a needs `wrapping_mul`, and an empty slot is `max_value<usize>()` |
| `big_integers.cb` | Exact `100!`, `2^256` and `fib(300)` in `u32` limbs, every carry an explicit `widen`/`narrow_wrapping`; a resource updated in place |
| `shortest_paths.cb` | Dijkstra with a hand-written heap: swapping two elements under the aliasing rules; `Option<u64>` distances, `checked_add` |
| `sudoku.cb` | Arrays are values: each guess copies the board, so backtracking needs no undo; candidate digits as bit sets |
| `crc_rle.cb` | CRC-32 and run-length coding with fully defined bit operations; the checksum catches a corrupted copy |
| `parallel_sort.cb` | Merge sort whose halves move into threads with `spawn` and back with `join`: parallel with nothing shared |
| `tinyos.cb` | A small kernel on simulated hardware (`tinyos/`): page allocator, page tables, a CPU running machine code, round-robin scheduling, system calls, a blocking pipe, a filesystem; the kernel owns everything, and a use-after-free is a page fault |

## Tier 4 — unsafe and foreign functions

What CobaltC lets you assert when it cannot check — and how little of
the language that switches off. These call real C functions from libc
and libm (x86-64 Linux), which both implementations now do.

| Showcase | What it shows |
|---|---|
| `ffi_math.cb` | `extern fn` declarations of libm functions; results are claims, checked where it matters |
| `ffi_claims.cb` | The trust transition: values from C's `rand` become typed dice only through a validator |
| `raw_linked_list.cb` | Manual memory: `allocate` as a `Result`, nodes written and walked through raw pointers, freed by hand |
| `arena_allocator.cb` | A bump allocator as a `resource struct` whose destructor frees the block; alignment, `copy_raw`, trusted pointer arithmetic |
| `vec_from_scratch.cb` | A growable vector written from raw memory, used beside `std::Vec` with the same results; why `cobc` gives `std::Vec` native code but compiles this one as written |
| `float_anatomy.cb` | An `f64`'s defined bits read through a raw pointer, taken apart, and rebuilt exactly with C's `ldexp` |
| `checked_write.cb` | `std`'s one real extern, `write`, called directly with its claim checked |

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
| `stats.cb` | Numbers from the arguments: count, min, max, sum, mean, median; a bad argument named with the byte where it stops being a number; exit 1 for bad input, 2 for none |
| `calc.cb` | A recursive-descent evaluator over a `String`'s bytes, compared with byte characters (`b'('`); `?` carries every error up; a caret under the failing column (from `parse`'s `Invalid` offset); a different exit status per kind of failure |
| `convert.cb` | An integer in decimal, hex and binary, and exactly which integer types hold it: one generic `parse<T>` call per type decides, with nothing to overflow, even at `i128`'s minimum |
| `wc.cb` | Lines, words and bytes of the files named, as Unix `wc` counts them: `read_file` over checked-in `wc_data/`, a missing file and a file that is not UTF-8 each reported by its `FileError` without stopping the rest, exit 1 if any failed; aligned columns with `printf` |
| `table.cb` | CSV rows as arguments, split at `b','` through `String::as_bytes` into `Vec<Vec<String>>`, one buffer handing its value on and taking a new one; `parse`'s strictness rejects `-2` as a `u32` and ` 1.25` as a number; `checked_add` guards the total; aligned output |

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
| `wordfreq.cb` | Word counts of a file with `*HashMap::entry(&mut counts, w, 0) += 1`; words taken from a `Vec<String>` by a consuming `foreach`, each moving into the map or dropped; a stable sort keeps first-appearance order among equal counts; a bar chart with `printf` widths |
| `anagrams.cb` | A `HashMap<String, Vec<String>>` whose values are resources, found or created by `entry`; removing while looping over a map is a borrow conflict, so singletons are collected, then removed with `remove_ordered` to keep the groups in order |
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
| `pngchunks.cb` | Random access in a binary format: the chunk list found by seeking over each chunk's data without reading it, then every chunk's CRC-32 checked by reading it; a copy with one byte changed, written with `write_bytes`, is caught by the check |
| `logscan.cb` | Logs read a line at a time with `File::read_line`; `Ok(Some(line))`, `Ok(None)` and `Err(Utf8(e))` in one `match`, a line that is not UTF-8 reported and skipped; ERROR lines written to a report with `File::create`/`write_str` and closed with `File::close`, each log's summary added to a history opened with `File::append` |
| `records.cb` | A file as a database of 32-byte records at `32 * k`, opened with `File::open_rw`: records encoded little-endian and written, a sale that rewrites 4 bytes in the middle of the file and nothing else, a new record at `File::len`; lookups return `Result<Option<Record>, FileError>` |

Writing Tier 7 found one fault in `coby`, fixed with a conformance
case: `match (f()?)` faulted, because the value `?` takes out of `Ok`
had no type for `match` to read its variant from; `cobc` was right.

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
