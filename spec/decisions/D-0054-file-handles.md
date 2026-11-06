# D-0054 — `File`: files as byte streams

Status: ACCEPTED (2026-09-26, owner-chosen)
Kind: Decision (`spec/02-schema.md`)
Governed by: `CobaltC_Master_Instructions.md` §1, §9, §17
Depends on: D-0034, D-0045, D-0047, rule.stdlib.file, rule.resauth.destroy
Affects: rule.stdlib.file-handle (new), rule.stdlib.prelude

## Problem

`read_file` and `write_file` (D-0034) read or write a whole file as
UTF-8 text. A program cannot read a file that is not text (an image,
an archive, a database page), stream a file too large to hold, or
append to a log. D-0034's revisit conditions named all three.

## Constraints

- No new tokens.
- No `unsafe` in the program; what is read is a checked value
  (`inv.trust-transition`).
- Both tools behave alike, sharing one implementation of the file
  access, as D-0034's do; `coby` stays portable to Windows.
- The resource model does the bookkeeping: a file is closed exactly
  once, never used after it is closed.

## Candidate mechanisms

The type is fixed by the resource model: `export resource struct File`
in `std`, its destructor closing it (as `Box`'s frees its memory,
D-0045), its fields private, so every `File` comes from opening one.
It holds an index into a table of the implementation's, not the
operating system's handle. `FileError` is reused unchanged; the end of
the file is a read of 0 bytes, not an error.

Opening:

1. **Separate constructors** `File::open` (read), `create` (write,
   truncate), `append`, `open_rw` (read and write, created, kept).
   **Selected.**
2. `File::open(&path, OpenMode::Append)` with a new enum.
3. An options builder, as Rust's `OpenOptions`.

Reading:

4. **Append to a `Vec<u8>`:** `File::read(&mut f, &mut buf, max)`
   appends up to `max` bytes. **Selected.**
5. Fill a slice, as C's `read`: needs a filled buffer, which the
   language has no short way to make (no array-repeat form, no
   `Vec::resize`).

Writing:

6. **Write all of it:** `File::write(&mut f, slice<u8, shared>)`,
   short writes retried inside. **Selected.**
7. Return the count, as C's `write`: every caller then needs a loop.

Closing:

8. **Both:** `File::close(File f)` reports a failure; the destructor
   closes a file the program did not. **Selected.**
9. The destructor only: a failed close is lost.

Positions:

10. **`File::seek(&mut f, u64 pos)` and `File::len(&f)`.** **Selected.**
11. No seeking.

And:

12. **Whole-file `read_bytes` and `write_bytes`**, the byte siblings of
    `read_file` and `write_file`. **Selected.**
13. **`File::write_str`** for text (writing a `String`'s bytes
    otherwise reads `&(*String::as_bytes(&s))[0..$]`). **Selected.**
14. **`File::read_line`**, as `read_line` reads standard input (the
    owner, 2026-09-26: the way a log is read in other languages).
    **Selected.**

## Selected design

Candidates 1, 4, 6, 8, 10, 12, 13 and 14:

    File::open / create / append / open_rw(&path) : Result<File, FileError>
    File::read(&mut f, &mut buf, usize max)       : Result<usize, FileError>
    File::read_to_end(&mut f, &mut buf)           : Result<usize, FileError>
    File::read_line(&mut f)                       : Result<Option<String>, FileError>
    File::write(&mut f, slice<u8, shared> data)   : Result<void, FileError>
    File::write_str(&mut f, &text)                : Result<void, FileError>
    File::seek(&mut f, u64 pos)                   : Result<void, FileError>
    File::len(&f)                                 : Result<u64, FileError>
    File::close(File f)                           : Result<void, FileError>
    read_bytes(&path)                             : Result<Vec<u8>, FileError>
    write_bytes(&path, slice<u8, shared> data)    : Result<void, FileError>

- **Reading is buffered, writing is not.** `read_line` needs a
  read-ahead; it lives in the implementation's table, so `read`,
  `read_line`, `seek` and `write` agree on the position (a write on an
  `open_rw` file lands where reading stopped). Each `write` reaches the
  environment before it returns.
- **`read`** appends up to `max` bytes, at most a MiB a call, fewer when
  fewer are ready; 0 only at the end, when `max > 0`.
- **`read_line`** removes the final `'\n'` only, as standard input's
  does; a line that is not UTF-8 is `Utf8(e)`, `e` from the line's
  start.
- **`close`** makes a written file durable before closing it, so a
  failure the environment reports late (a full disk, a lost network
  file) is reported; the destructor closes without waiting and cannot
  report. A file closed by `close` is closed even when it fails.
- **Positions and lengths are `u64`**, so a file larger than the
  address space is still addressed.

## Rejected alternatives

- **2:** an enum and a mode argument to say what the name can say.
- **3:** a builder for combinations no program here needs.
- **5:** usable only after `Vec::resize` or an array-repeat form exists.
- **7:** a loop at every call, which programs forget.
- **9:** a lost write error, silently.
- **11:** database pages and binary formats need positions.
- **The operating system's handle in `File`:** not portable (a Windows
  `HANDLE` is not an fd), and the read-ahead needs a home.
- **Unbuffered `read_line`,** a byte a call, as standard input's: slow
  for the logs it is for.

## Semantic rationale

`File` is a resource like `Vec` and `Box`: `spec/07`'s rules alone
guarantee one owner, no use after `close` (it takes the `File`), and one
close, by `close` or by the destructor. No rule of the core changes;
files remain part of the environment (`rule.fn.program`). Bytes reach a
`String` only through `String::from_utf8` (`read_line`).

## Usability

    String path = Result::unwrap_or(arg(0), String::new());
    File log = match (File::append(&path))
    {
        Ok(f) : f,
        Err(_) : return 1,
    };
    String line = sprintf("started %v\n", 3);
    Result::unwrap_or(File::write_str(&mut log, &line), ());

    File input = match (File::open(&path)) { … };
    while (true)
    {
        match (File::read_line(&mut input))
        {
            Ok(o) : match (o)
            {
                Some(s) : { … },
                None : { break; },
            },
            Err(_) : { break; },
        }
    }

## Explainability

"A `File` is an open file: open it, read or write it in order, seek
it; it closes when it goes away, or `close` it to hear whether that
worked."

## Implementation-feasibility

`std` gains about 250 lines of CobaltC over two primitives private to
`std`, `file_op(op, h, buf, n)` and `file_at(op, h, pos)` (for `seek`
and `len`, whose positions are `u64`). `std` converts with
`reinterpret` only: a program's own item named `widen` or `narrow`
would take the name from `std`'s code (`[Resolve-Unqualified]`), which
`conf.item-shadows-intrinsic` exercises. `impl/src/fileio.rs` holds the table of
open files (one mutex; each entry a `std::fs::File`, its read-ahead,
whether it was opened for writing) for both tools; `coby` realizes the
primitives in `call_extern`, `cobc` compiles them to `cb_file_op` and
`cb_file_at`. No
change to the checker.

## Compatibility impact

Extension. A program's own `File`, `read_bytes` or `write_bytes` takes
precedence (D-0024).

## Prior-art status

- **Rust:** `std::fs::File`, `OpenOptions`, `BufReader::read_line`,
  `Read::read_to_end`, `Write::write_all`, `File::sync_all`; `Drop`
  closes and ignores the error.
- **Go:** `os.Open`/`Create`/`OpenFile`, `bufio.Scanner`, `f.Close()`
  returning the error, `defer`.
- **C:** `fopen` modes `"r"`, `"w"`, `"a"`, `"r+"`; `fgets`, `fread`,
  `fwrite`, `fseek`, `fclose`.
- **Zig:** `std.fs.File`, `reader().readUntilDelimiter`, explicit
  `close` (no error), `sync`.

## Invariant traceability

`inv.resource-authority`: a `File` is closed exactly once.
`inv.trust-transition`, `inv.string.utf8-validity`: `read_line`'s bytes
become a `String` only through `String::from_utf8`.

## Revisit conditions

- Buffered writing (a `flush`), formatted writing to a file (`fprintf`).
- Standard input and output as `File`s.
- Directories, removing and renaming files, permissions.
- A fill-a-slice `read`, once `Vec::resize` or an array-repeat form exists.
