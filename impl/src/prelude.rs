// The CobaltC prelude, transcribed faithfully from
// spec/21-standard-library-semantics.md §0-3 (CHG-0020's `write`,
// CHG-0025's `str` surface, CHG-0038's `read`/`read_line`, CHG-0040's
// `arg_bytes`/`arg_count`/`arg` and CHG-0043's files included).
// Every intrinsic named here without a body (drop, sizeof, allocate,
// rawptr_of, reclaim, spawn, join, str_len, str_byte, str_ptr, ...) is recognized natively by
// `Interp::try_intrinsic`, per CHG-0023: a body-less prelude entry is
// never a `Σ.items`/parsed-item entry. `map_err` and every `Vec`/`String`/
// `Rc` function ARE real parsed CobaltC source, run through the same
// parser/evaluator as any user function, exactly as the spec directs.

pub const PRELUDE_SRC: &str = r#"
export module std
{
export enum Option<T>
{
    Some(T),
    None,
}

export enum Result<T, E>
{
    Ok(T),
    Err(E),
}

export struct AllocError
{
}

export struct Utf8Error
{
    export usize offset;
}

export enum ReadError
{
    Io,
    Utf8(Utf8Error),
}

// `std` has two enums with `Io` and `Utf8`: its own code names them
// `ReadError::…` and `FileError::…` (`spec/17` clause (4)).
export enum FileError
{
    NotFound,
    Denied,
    Io,
    Utf8(Utf8Error),
}

export enum ParseError
{
    Empty,
    Invalid(usize),
    OutOfRange,
}

// D-0033 (6): the value, or `d` when there is none; an unused `d` (or
// `Err` payload) is destroyed like any other.
export fn Option::unwrap_or<T>(Option<T> o, T d) : T
{
    match (o)
    {
        Some(v) : v,
        None : d,
    }
}

export fn Result::unwrap_or<T, E>(Result<T, E> r, T d) : T
{
    match (r)
    {
        Ok(v) : v,
        Err(_) : d,
    }
}

export extern fn write(rawptr<u8> buf, usize len) : isize;
export extern fn read(rawptr<u8> buf, usize len) : isize;
export extern fn arg_bytes(usize i, rawptr<u8> buf, usize len) : isize;

// Standard error (`rule.stdlib.format` `[Eprintf]`, D-0040): `write`'s
// counterpart, private to `std`; `eprintf` writes through `eprint_str`.
extern fn write_err(rawptr<u8> buf, usize len) : isize;

// Files (`rule.stdlib.file`, spec/21 §2e, CHG-0043): `read_file` and
// `write_file` are realized over these two, private to `std`. The path
// is the bytes of a `String`. `file_read` copies up to `cap` bytes of
// the file into `buf` and returns its whole length; `file_write`
// replaces the file's contents with `len` bytes and returns 0. Both
// return -1 when the file does not exist, -2 when access is denied, -3
// for any other failure (src/fileio.rs).
extern fn file_read(rawptr<u8> path, usize path_len, rawptr<u8> buf, usize cap) : isize;
extern fn file_write(rawptr<u8> path, usize path_len, rawptr<u8> buf, usize len) : isize;

// `File` (`rule.stdlib.file-handle`, spec/21 §2e, D-0054) is written over
// these two, private to `std`: `op` names the operation, `h` the open file
// (or the mode, for opening), `buf`/`n` its bytes or its count; `file_at`
// takes or gives a position. Each returns a handle, a count, a length or
// 0, or a failure code as `file_read`'s (src/fileio.rs).
extern fn file_op(usize op, usize h, rawptr<u8> buf, usize n) : isize;
extern fn file_at(usize op, usize h, u64 pos) : i64;

export resource struct Vec<T>
{
    rawptr<T> ptr;
    usize len;
    usize cap;
}

export fn Vec::new<T>() : Vec<T>
{
    Vec { .ptr = dangling<T>(), .len = 0, .cap = 0 }
}

export fn Vec::len<T>(ref<Vec<T>, shared> v) : usize
{
    v.len
}

export fn Vec::push<T>(ref<Vec<T>, exclusive> v, T x)
{
    if (v.len == v.cap)
    {
        Vec::grow(v);
    }
    unsafe
    {
        *(v.ptr + reinterpret<isize>(v.len)) = x;
    }
    v.len = v.len + 1;
}

export fn Vec::pop<T>(ref<Vec<T>, exclusive> v) : Option<T>
{
    if (v.len == 0)
    {
        return None;
    }
    v.len = v.len - 1;
    unsafe
    {
        rawptr<T> p = v.ptr + reinterpret<isize>(v.len);
        auto x = *p;
        release(reinterpret_ptr<u8>(p), sizeof<T>());
        Some(x)
    }
}

export fn Vec::index_shared<T>(ref<Vec<T>, shared> v, usize i) : ref<T, shared>
{
    if (i >= v.len)
    {
        fault(index_out_of_bounds);
    }
    unsafe
    {
        &reclaim<T>(v.ptr + reinterpret<isize>(i))
    }
}

export fn Vec::index_exclusive<T>(ref<Vec<T>, exclusive> v, usize i) : ref<T, exclusive>
{
    if (i >= v.len)
    {
        fault(index_out_of_bounds);
    }
    unsafe
    {
        &mut reclaim<T>(v.ptr + reinterpret<isize>(i))
    }
}

fn Vec::grow<T>(ref<Vec<T>, exclusive> v)
{
    usize new_cap = if (v.cap == 0)
    {
        4
    }
    else
    {
        v.cap * 2
    };
    usize bytes = new_cap * sizeof<T>();
    rawptr<u8> p = match (allocate(bytes, alignof<T>()))
    {
        Ok(p) : p,
        Err(_) : fault(alloc_failure),
    };
    unsafe
    {
        copy_raw(p, reinterpret_ptr<u8>(v.ptr), v.len * sizeof<T>());
        if (v.cap > 0)
        {
            deallocate(reinterpret_ptr<u8>(v.ptr), v.cap * sizeof<T>(), alignof<T>());
        }
    }
    v.ptr = reinterpret_ptr<T>(p);
    v.cap = new_cap;
}

export fn Vec::drop<T>(ref<Vec<T>, exclusive> self)
{
    usize mut_i = 0;
    while (mut_i < self.len)
    {
        unsafe
        {
            drop(reclaim<T>(self.ptr + reinterpret<isize>(mut_i)));
        }
        mut_i = mut_i + 1;
    }
    if (self.cap > 0)
    {
        unsafe
        {
            deallocate(reinterpret_ptr<u8>(self.ptr), self.cap * sizeof<T>(), alignof<T>());
        }
    }
}

export struct String
{
    Vec<u8> bytes;
}

export fn String::from_utf8(Vec<u8> bytes) : Result<String, Utf8Error>
{
    usize n = Vec::len(&bytes);
    usize i = 0;
    while (i < n)
    {
        u8 b = *Vec::index_shared(&bytes, i);
        usize width =
            if (b < 128)
            {
                1
            }
            else if ((b & 224) == 192 && b >= 194)
            {
                2
            }
            else if ((b & 240) == 224)
            {
                3
            }
            else if ((b & 248) == 240 && b <= 244)
            {
                4
            }
            else
            {
                return Err(Utf8Error { .offset = i });
            };
        if (i + width > n)
        {
            return Err(Utf8Error { .offset = i });
        }
        usize k = 1;
        while (k < width)
        {
            u8 c = *Vec::index_shared(&bytes, i + k);
            if ((c & 192) != 128)
            {
                return Err(Utf8Error { .offset = i + k });
            }
            k = k + 1;
        }
        if (width == 3)
        {
            u8 c1 = *Vec::index_shared(&bytes, i + 1);
            if ((b == 224 && c1 < 160) || (b == 237 && c1 >= 160))
            {
                return Err(Utf8Error { .offset = i });
            }
        }
        if (width == 4)
        {
            u8 c1 = *Vec::index_shared(&bytes, i + 1);
            if ((b == 240 && c1 < 144) || (b == 244 && c1 >= 144))
            {
                return Err(Utf8Error { .offset = i });
            }
        }
        i = i + width;
    }
    Ok(String { .bytes = bytes })
}

export fn String::len(ref<String, shared> s) : usize
{
    Vec::len(&s.bytes)
}

export fn String::into_bytes(String s) : Vec<u8>
{
    String { bytes } = s;
    bytes
}

export fn String::from_str(str s) : String
{
    Vec<u8> v = Vec::new();
    usize n = str_len(s);
    usize i = 0;
    while (i < n)
    {
        Vec::push(&mut v, str_byte(s, i));
        i = i + 1;
    }
    String { .bytes = v }
}

export fn read_line() : Result<Option<String>, ReadError>
{
    rawptr<u8> cell = match (allocate(1, 1))
    {
        Ok(p) : p,
        Err(_) : fault(alloc_failure),
    };
    Vec<u8> bytes = Vec::new();
    isize n = 0;
    bool any = false;
    while (true)
    {
        n = unsafe
        {
            read(cell, 1)
        };
        if (n <= 0)
        {
            break;
        }
        any = true;
        u8 c = unsafe
        {
            *cell
        };
        if (c == 10)
        {
            break;
        }
        Vec::push(&mut bytes, c);
    }
    unsafe
    {
        deallocate(cell, 1, 1);
    }
    if (n < 0)
    {
        return Err(ReadError::Io);
    }
    if (!any)
    {
        return Ok(None);
    }
    match (String::from_utf8(bytes))
    {
        Ok(s) : Ok(Some(s)),
        Err(e) : Err(ReadError::Utf8(e)),
    }
}

export fn arg_count() : usize
{
    usize i = 0;
    while (true)
    {
        isize n = unsafe
        {
            arg_bytes(i, dangling<u8>(), 0)
        };
        if (n < 0)
        {
            break;
        }
        i = i + 1;
    }
    i
}

export fn arg(usize i) : Result<String, Utf8Error>
{
    isize n = unsafe
    {
        arg_bytes(i, dangling<u8>(), 0)
    };
    if (n < 0)
    {
        fault(index_out_of_bounds);
    }
    usize len = reinterpret<usize>(n);
    rawptr<u8> p = match (allocate(len, 1))
    {
        Ok(p) : p,
        Err(_) : fault(alloc_failure),
    };
    Vec<u8> bytes = Vec::new();
    unsafe
    {
        arg_bytes(i, p, len);
    }
    usize j = 0;
    while (j < len)
    {
        u8 b = unsafe
        {
            *(p + reinterpret<isize>(j))
        };
        Vec::push(&mut bytes, b);
        j = j + 1;
    }
    if (len > 0)
    {
        unsafe
        {
            deallocate(p, len, 1);
        }
    }
    String::from_utf8(bytes)
}

// `print<T>(T x)` is realized natively (`rule.stdlib.print`, spec/21
// §2a). Its `str` and `ref<String, shared>` cases are these two
// functions, which it calls; the others format in the implementation.
fn print_str(str s)
{
    unsafe
    {
        write(str_ptr(s), str_len(s));
    }
}

fn print_string(ref<String, shared> s)
{
    unsafe
    {
        write(reinterpret_ptr<u8>(s.bytes.ptr), s.bytes.len);
    }
}

fn eprint_str(str s)
{
    unsafe
    {
        write_err(str_ptr(s), str_len(s));
    }
}

// Text conversions (`rule.stdlib.text`, spec/21 §2d, CHG-0041).
export fn String::new() : String
{
    String { .bytes = Vec::new() }
}

export fn String::as_bytes(ref<String, shared> s) : ref<Vec<u8>, shared>
{
    &s.bytes
}

// A copy of s: a new String with the same bytes (D-0044).
export fn String::clone(ref<String, shared> s) : String
{
    String c = String::new();
    String::append_string(&mut c, s);
    c
}

// Adds one ASCII byte (below 128) to s; any other byte would not be
// UTF-8 on its own, and faults (D-0044). Text of arbitrary bytes is
// built in a Vec<u8> and checked by String::from_utf8.
export fn String::push_ascii(ref<String, exclusive> s, u8 b)
{
    if (b >= 128)
    {
        fault(not_ascii);
    }
    Vec::push(&mut s.bytes, b);
}

// Empties s, keeping its buffer (D-0044).
export fn String::clear(ref<String, exclusive> s)
{
    Vec::clear(&mut s.bytes);
}

// D-0053: a borrowed, read-only view of part of a `String`'s text, made
// by `&s[lo .. hi]` (`String::view`) and re-sliced by `&v[lo .. hi]`
// (`StringView::sub`). Its bounds fall on character boundaries, so its
// bytes are UTF-8. The `String` stays borrowed while a view of it lives.
export struct StringView
{
    slice<u8, shared> bytes;
}

// Whether byte position `i` of `b` starts a character (or is its end).
fn StringView::boundary(slice<u8, shared> b, usize i) : bool
{
    i == 0 || i == slice_len(b) || (b[i] & 0xC0: u8) != 0x80: u8
}

export fn String::view(ref<String, shared> s, usize lo, usize hi) : StringView
{
    slice<u8, shared> all = &s.bytes[0..$];
    if (lo > hi || hi > slice_len(all))
    {
        fault(index_out_of_bounds);
    }
    if (!StringView::boundary(all, lo) || !StringView::boundary(all, hi))
    {
        fault(not_char_boundary);
    }
    StringView { .bytes = &all[lo..hi] }
}

export fn StringView::sub(StringView v, usize lo, usize hi) : StringView
{
    if (lo > hi || hi > slice_len(v.bytes))
    {
        fault(index_out_of_bounds);
    }
    if (!StringView::boundary(v.bytes, lo) || !StringView::boundary(v.bytes, hi))
    {
        fault(not_char_boundary);
    }
    StringView { .bytes = &v.bytes[lo..hi] }
}

export fn StringView::len(StringView v) : usize
{
    slice_len(v.bytes)
}

export fn StringView::eq(StringView a, StringView b) : bool
{
    usize n = slice_len(a.bytes);
    if (n != slice_len(b.bytes))
    {
        return false;
    }
    for (usize i = 0; i < n; i += 1)
    {
        if (a.bytes[i] != b.bytes[i])
        {
            return false;
        }
    }
    true
}

export fn StringView::eq_str(StringView a, str t) : bool
{
    usize n = slice_len(a.bytes);
    if (n != str_len(t))
    {
        return false;
    }
    for (usize i = 0; i < n; i += 1)
    {
        if (a.bytes[i] != str_byte(t, i))
        {
            return false;
        }
    }
    true
}

// Whether `t` occurs in `v` at byte position `at`.
fn StringView::at(StringView v, usize at, str t) : bool
{
    usize m = str_len(t);
    if (at + m > slice_len(v.bytes))
    {
        return false;
    }
    for (usize j = 0; j < m; j += 1)
    {
        if (v.bytes[at + j] != str_byte(t, j))
        {
            return false;
        }
    }
    true
}

// The byte position of the first occurrence of `t` in `v`.
export fn StringView::find(StringView v, str t) : Option<usize>
{
    usize n = slice_len(v.bytes);
    usize m = str_len(t);
    if (m > n)
    {
        return None;
    }
    for (usize i = 0; i <= n - m; i += 1)
    {
        if (StringView::at(v, i, t))
        {
            return Some(i);
        }
    }
    None
}

export fn StringView::starts_with(StringView v, str t) : bool
{
    StringView::at(v, 0, t)
}

export fn StringView::ends_with(StringView v, str t) : bool
{
    usize n = slice_len(v.bytes);
    usize m = str_len(t);
    m <= n && StringView::at(v, n - m, t)
}

// ASCII white space: space, tab, line feed, vertical tab, form feed, return.
fn StringView::space(u8 b) : bool
{
    b == b' ' || (b >= 9: u8 && b <= 13: u8)
}

export fn StringView::trim_start(StringView v) : StringView
{
    usize n = slice_len(v.bytes);
    usize i = 0;
    while (i < n && StringView::space(v.bytes[i]))
    {
        i += 1;
    }
    StringView::sub(v, i, n)
}

export fn StringView::trim_end(StringView v) : StringView
{
    usize n = slice_len(v.bytes);
    while (n > 0 && StringView::space(v.bytes[n - 1]))
    {
        n -= 1;
    }
    StringView::sub(v, 0, n)
}

export fn StringView::trim(StringView v) : StringView
{
    StringView::trim_end(StringView::trim_start(v))
}

// The parts of `v` between occurrences of `sep`, each a view of the same
// `String`; an empty `sep` gives `v` whole.
export fn StringView::split(StringView v, str sep) : Vec<StringView>
{
    Vec<StringView> out = Vec::new();
    usize n = slice_len(v.bytes);
    usize m = str_len(sep);
    if (m == 0)
    {
        Vec::push(&mut out, v);
        return out;
    }
    usize start = 0;
    usize i = 0;
    while (i + m <= n)
    {
        if (StringView::at(v, i, sep))
        {
            Vec::push(&mut out, StringView::sub(v, start, i));
            i += m;
            start = i;
        }
        else
        {
            i += 1;
        }
    }
    Vec::push(&mut out, StringView::sub(v, start, n));
    out
}

// A new `String` holding a copy of the view's text.
export fn String::from_view(StringView v) : String
{
    Vec<u8> b = Vec::new();
    foreach (x in v.bytes)
    {
        Vec::push(&mut b, *x);
    }
    String { .bytes = b }
}

export fn StringView::parse<T>(StringView v) : Result<T, ParseError>
{
    String s = String::from_view(v);
    parse<T>(&s)
}

fn String::append_view(ref<String, exclusive> s, StringView v)
{
    foreach (x in v.bytes)
    {
        Vec::push(&mut s.bytes, *x);
    }
}

// `String::append<T>` is realized natively, as `print<T>` is: the
// std-only intrinsic `append_native` sends a `str` to `append_str`, a
// `ref<String, shared>` to `append_string`, a `StringView` to
// `append_view` (D-0053), and a number or `bool` to
// `append_text`, which has `text_write` format it into a buffer. The
// front end checks `T` at the call.
export fn String::append<T>(ref<String, exclusive> s, T x)
{
    append_native(s, x);
}

fn String::append_str(ref<String, exclusive> s, str t)
{
    usize n = str_len(t);
    usize i = 0;
    while (i < n)
    {
        Vec::push(&mut s.bytes, str_byte(t, i));
        i = i + 1;
    }
}

fn String::append_string(ref<String, exclusive> s, ref<String, shared> t)
{
    usize n = t.bytes.len;
    usize i = 0;
    while (i < n)
    {
        u8 b = unsafe
        {
            *(reinterpret_ptr<u8>(t.bytes.ptr) + reinterpret<isize>(i))
        };
        Vec::push(&mut s.bytes, b);
        i = i + 1;
    }
}

fn String::append_text<T>(ref<String, exclusive> s, T x)
{
    rawptr<u8> p = match (allocate(64, 1))
    {
        Ok(p) : p,
        Err(_) : fault(alloc_failure),
    };
    usize n = unsafe
    {
        text_write(x, p)
    };
    usize i = 0;
    while (i < n)
    {
        u8 b = unsafe
        {
            *(p + reinterpret<isize>(i))
        };
        Vec::push(&mut s.bytes, b);
        i = i + 1;
    }
    unsafe
    {
        deallocate(p, 64, 1);
    }
}

// `parse<T>` is realized natively: the std-only intrinsics `parse_check`
// (-1 for a number, -2 empty, -3 out of range, else the invalid offset)
// and `parse_value` read the bytes (src/numtext.rs). The front end
// checks `T` at the call.
export fn parse<T>(ref<String, shared> s) : Result<T, ParseError>
{
    rawptr<u8> p = reinterpret_ptr<u8>(s.bytes.ptr);
    usize n = s.bytes.len;
    isize code = unsafe
    {
        parse_check<T>(p, n)
    };
    if (code == -1)
    {
        auto v = unsafe
        {
            parse_value<T>(p, n)
        };
        return Ok(v);
    }
    if (code == -2)
    {
        return Err(Empty);
    }
    if (code == -3)
    {
        return Err(OutOfRange);
    }
    Err(Invalid(reinterpret<usize>(code)))
}

fn file_error(isize code) : FileError
{
    if (code == -1)
    {
        return FileError::NotFound;
    }
    if (code == -2)
    {
        return FileError::Denied;
    }
    FileError::Io
}

export fn read_file(ref<String, shared> path) : Result<String, FileError>
{
    rawptr<u8> p = reinterpret_ptr<u8>(path.bytes.ptr);
    usize pn = path.bytes.len;
    usize cap = 4096;
    while (true)
    {
        rawptr<u8> buf = match (allocate(cap, 1))
        {
            Ok(b) : b,
            Err(_) : fault(alloc_failure),
        };
        isize n = unsafe
        {
            file_read(p, pn, buf, cap)
        };
        if (n >= 0 && reinterpret<usize>(n) <= cap)
        {
            usize len = reinterpret<usize>(n);
            Vec<u8> bytes = Vec::new();
            usize i = 0;
            while (i < len)
            {
                u8 b = unsafe
                {
                    *(buf + reinterpret<isize>(i))
                };
                Vec::push(&mut bytes, b);
                i = i + 1;
            }
            unsafe
            {
                deallocate(buf, cap, 1);
            }
            return match (String::from_utf8(bytes))
            {
                Ok(s) : Ok(s),
                Err(e) : Err(FileError::Utf8(e)),
            };
        }
        unsafe
        {
            deallocate(buf, cap, 1);
        }
        if (n < 0)
        {
            return Err(file_error(n));
        }
        // Longer than `cap`: again, with room for all of it.
        cap = reinterpret<usize>(n);
    }
    Err(FileError::Io)
}

export fn write_file(ref<String, shared> path, ref<String, shared> text) : Result<void, FileError>
{
    isize r = unsafe
    {
        file_write(reinterpret_ptr<u8>(path.bytes.ptr), path.bytes.len, reinterpret_ptr<u8>(text.bytes.ptr), text.bytes.len)
    };
    if (r < 0)
    {
        return Err(file_error(r));
    }
    Ok(())
}

// An open file (`rule.stdlib.file-handle`, spec/21 §2e, D-0054): its
// bytes read and written in order from a position, which `seek` moves.
// Its destructor closes it; `close` closes it and reports a failure.
export resource struct File
{
    usize id;
    bool open;                          // false once close has closed it
}

// `file_error` for `file_at`'s codes.
fn file_error_at(i64 code) : FileError
{
    if (code == -1)
    {
        return FileError::NotFound;
    }
    if (code == -2)
    {
        return FileError::Denied;
    }
    FileError::Io
}

fn File::start(ref<String, shared> path, usize mode) : Result<File, FileError>
{
    isize r = unsafe
    {
        file_op(0, mode, reinterpret_ptr<u8>(path.bytes.ptr), path.bytes.len)
    };
    if (r < 0)
    {
        return Err(file_error(r));
    }
    Ok(File { .id = reinterpret<usize>(r), .open = true })
}

// For reading; the file must exist.
export fn File::open(ref<String, shared> path) : Result<File, FileError>
{
    File::start(path, 0)
}

// For writing, from empty: created, or its contents discarded.
export fn File::create(ref<String, shared> path) : Result<File, FileError>
{
    File::start(path, 1)
}

// For writing at its end, created if it does not exist.
export fn File::append(ref<String, shared> path) : Result<File, FileError>
{
    File::start(path, 2)
}

// For reading and writing, created if it does not exist, its contents kept.
export fn File::open_rw(ref<String, shared> path) : Result<File, FileError>
{
    File::start(path, 3)
}

// Appends exactly `want` bytes of what `file_op` reads, or fewer at the end.
fn File::take(ref<File, exclusive> f, ref<Vec<u8>, exclusive> buf, usize want) : Result<usize, FileError>
{
    while (buf.cap - buf.len < want)
    {
        Vec::grow(buf);
    }
    isize r = unsafe
    {
        file_op(3, f.id, reinterpret_ptr<u8>(buf.ptr + reinterpret<isize>(buf.len)), want)
    };
    if (r < 0)
    {
        return Err(file_error(r));
    }
    usize n = reinterpret<usize>(r);
    buf.len = buf.len + n;
    Ok(n)
}

// Appends up to `max` bytes to `buf` (at most a MiB, and fewer when fewer
// are ready) and returns how many: 0 only at the end, when max > 0.
export fn File::read(ref<File, exclusive> f, ref<Vec<u8>, exclusive> buf, usize max) : Result<usize, FileError>
{
    usize want = if (max > 1_048_576)
    {
        1_048_576
    }
    else
    {
        max
    };
    File::take(f, buf, want)
}

// Appends the rest of the file to `buf` and returns how many bytes.
export fn File::read_to_end(ref<File, exclusive> f, ref<Vec<u8>, exclusive> buf) : Result<usize, FileError>
{
    usize total = 0;
    while (true)
    {
        usize n = match (File::read(f, buf, 65_536))
        {
            Ok(n) : n,
            Err(e) : return Err(e),
        };
        if (n == 0)
        {
            break;
        }
        total = total + n;
    }
    Ok(total)
}

// The next line without its '\n', as `read_line` reads standard input;
// None at the end of the file.
export fn File::read_line(ref<File, exclusive> f) : Result<Option<String>, FileError>
{
    isize r = unsafe
    {
        file_op(4, f.id, dangling<u8>(), 0)
    };
    if (r < 0)
    {
        return Err(file_error(r));
    }
    if (r == 0)
    {
        return Ok(None);
    }
    Vec<u8> bytes = Vec::new();
    usize n = match (File::take(f, &mut bytes, reinterpret<usize>(r)))
    {
        Ok(n) : n,
        Err(e) : return Err(e),
    };
    if (n > 0 && bytes[n - 1] == 10)
    {
        Vec::pop(&mut bytes);
    }
    match (String::from_utf8(bytes))
    {
        Ok(s) : Ok(Some(s)),
        Err(e) : Err(FileError::Utf8(e)),
    }
}

// Writes all of `data` at the file's position (at its end, for append).
export fn File::write(ref<File, exclusive> f, slice<u8, shared> data) : Result<void, FileError>
{
    usize n = slice_len(data);
    if (n == 0)
    {
        return Ok(());
    }
    isize r = unsafe
    {
        file_op(5, f.id, rawptr_of(&data[0]), n)
    };
    if (r < 0)
    {
        return Err(file_error(r));
    }
    Ok(())
}

export fn File::write_str(ref<File, exclusive> f, ref<String, shared> text) : Result<void, FileError>
{
    File::write(f, &text.bytes[0..$])
}

// Moves the position to byte `pos` from the start.
export fn File::seek(ref<File, exclusive> f, u64 pos) : Result<void, FileError>
{
    i64 r = unsafe
    {
        file_at(6, f.id, pos)
    };
    if (r < 0)
    {
        return Err(file_error_at(r));
    }
    Ok(())
}

// The file's length in bytes.
export fn File::len(ref<File, shared> f) : Result<u64, FileError>
{
    i64 r = unsafe
    {
        file_at(7, f.id, 0)
    };
    if (r < 0)
    {
        return Err(file_error_at(r));
    }
    Ok(reinterpret<u64>(r))
}

// Closes the file. A file open for writing is first made durable, so a
// failure the environment reports late is reported here.
export fn File::close(File f) : Result<void, FileError>
{
    f.open = false;
    isize r = unsafe
    {
        file_op(1, f.id, dangling<u8>(), 0)
    };
    if (r < 0)
    {
        return Err(file_error(r));
    }
    Ok(())
}

export fn File::drop(ref<File, exclusive> self)
{
    if (self.open)
    {
        isize r = unsafe
        {
            file_op(2, self.id, dangling<u8>(), 0)
        };
    }
}

// The whole file's bytes, as `read_file` reads its text.
export fn read_bytes(ref<String, shared> path) : Result<Vec<u8>, FileError>
{
    File f = match (File::open(path))
    {
        Ok(f) : f,
        Err(e) : return Err(e),
    };
    Vec<u8> bytes = Vec::new();
    match (File::read_to_end(&mut f, &mut bytes))
    {
        Ok(_) : Ok(bytes),
        Err(e) : Err(e),
    }
}

// Makes the file hold exactly `data`, as `write_file` does its text.
export fn write_bytes(ref<String, shared> path, slice<u8, shared> data) : Result<void, FileError>
{
    File f = match (File::create(path))
    {
        Ok(f) : f,
        Err(e) : return Err(e),
    };
    File::write(&mut f, data)
}

// Puts x in slot i of v and returns what was there. Private to `std`:
// `HashMap::insert` replaces a value with it.
fn Vec::replace<T>(ref<Vec<T>, exclusive> v, usize i, T x) : T
{
    if (i >= v.len)
    {
        fault(index_out_of_bounds);
    }
    unsafe
    {
        rawptr<T> p = v.ptr + reinterpret<isize>(i);
        auto old = *p;
        release(reinterpret_ptr<u8>(p), sizeof<T>());
        *p = x;
        old
    }
}

// Takes element i out of v; the elements after it keep their order
// (they come off the end and go back on). Private to `std`:
// `HashMap::remove` uses it.
fn Vec::remove_at<T>(ref<Vec<T>, exclusive> v, usize i) : T
{
    if (i >= v.len)
    {
        fault(index_out_of_bounds);
    }
    Vec<T> later = Vec::new();
    while (v.len > i + 1)
    {
        match (Vec::pop(v))
        {
            Some(x) :
            {
                Vec::push(&mut later, x);
            },
            None :
            {
            },
        }
    }
    auto taken = match (Vec::pop(v))
    {
        Some(x) : x,
        None : fault(index_out_of_bounds),
    };
    while (Vec::len(&later) > 0)
    {
        match (Vec::pop(&mut later))
        {
            Some(x) :
            {
                Vec::push(v, x);
            },
            None :
            {
            },
        }
    }
    taken
}

// Moves element i out of v, leaving its slot empty. Private to `std`:
// only a holder that sets `len` to 0 before the Vec is destroyed may use
// it (`Drain`, `HashDrain`).
fn Vec::take_raw<T>(ref<Vec<T>, exclusive> v, usize i) : T
{
    unsafe
    {
        rawptr<T> p = v.ptr + reinterpret<isize>(i);
        auto x = *p;
        release(reinterpret_ptr<u8>(p), sizeof<T>());
        x
    }
}

// Empties v: its elements are destroyed in order, first to last, as
// the Vec's own destructor would; its buffer is kept for reuse (D-0044).
export fn Vec::clear<T>(ref<Vec<T>, exclusive> v)
{
    usize i = 0;
    while (i < v.len)
    {
        drop(Vec::take_raw(v, i));
        i = i + 1;
    }
    v.len = 0;
}

// A consumed Vec that `foreach` takes elements from in order
// (`rule.control.foreach`, spec/14, D-0042). The elements not yet taken
// when the loop ends are destroyed with it.
resource struct Drain<T>
{
    Vec<T> v;
    usize next;
}

fn Vec::drain<T>(Vec<T> v) : Drain<T>
{
    Drain { .v = v, .next = 0 }
}

fn Drain::total<T>(ref<Drain<T>, shared> d) : usize
{
    d.v.len
}

fn Drain::take<T>(ref<Drain<T>, exclusive> d) : T
{
    auto x = Vec::take_raw(&mut d.v, d.next);
    d.next = d.next + 1;
    x
}

fn Drain::drop<T>(ref<Drain<T>, exclusive> self)
{
    while (self.next < self.v.len)
    {
        drop(Vec::take_raw(&mut self.v, self.next));
        self.next = self.next + 1;
    }
    self.v.len = 0;                 // every element is gone: the Vec frees only its buffer
}

// Hash tables (`rule.stdlib.hashmap`, spec/21 §1a, D-0041). A key type
// is an integer type, `bool`, `str` or `String`; the std-only intrinsics
// `key_hash` (FNV-1a over the key's bytes) and `key_eq` realize hashing
// and equality for each, and the front end checks `K` at every call.
// Keys and values are kept in insertion order, entry i in keys[i] and
// values[i], and the table holds their indices, so iteration is in
// insertion order.
export struct HashMap<K, V>
{
    Vec<K> keys;                        // in insertion order
    Vec<V> values;                      // values[i] belongs to keys[i]
    Vec<usize> slots;                   // an entry index, or max_value<usize>() when empty
}

export fn HashMap::new<K, V>() : HashMap<K, V>
{
    Vec<usize> slots = Vec::new();
    usize i = 0;
    while (i < 8)
    {
        Vec::push(&mut slots, max_value<usize>());
        i = i + 1;
    }
    HashMap { .keys = Vec::new(), .values = Vec::new(), .slots = slots }
}

export fn HashMap::len<K, V>(ref<HashMap<K, V>, shared> m) : usize
{
    Vec::len(&m.keys)
}

// Entry i in insertion order, for i < len.
export fn HashMap::key_at<K, V>(ref<HashMap<K, V>, shared> m, usize i) : ref<K, shared>
{
    Vec::index_shared(&m.keys, i)
}

export fn HashMap::value_at<K, V>(ref<HashMap<K, V>, shared> m, usize i) : ref<V, shared>
{
    Vec::index_shared(&m.values, i)
}

// The slot holding k, or the empty slot where it belongs (linear
// probing; the table is never full).
fn HashMap::find<K, V>(ref<HashMap<K, V>, shared> m, ref<K, shared> k) : usize
{
    usize mask = Vec::len(&m.slots) - 1;
    usize i = narrow_wrapping<usize>(key_hash(k)) & mask;
    bool searching = true;
    while (searching)
    {
        usize e = *Vec::index_shared(&m.slots, i);
        if (e == max_value<usize>())
        {
            searching = false;
        }
        else if (key_eq(Vec::index_shared(&m.keys, e), k))
        {
            searching = false;
        }
        else
        {
            i = (i + 1) & mask;
        }
    }
    i
}

// The entry index for k, or max_value<usize>() when k is absent.
fn HashMap::index_of<K, V>(ref<HashMap<K, V>, shared> m, ref<K, shared> k) : usize
{
    *Vec::index_shared(&m.slots, HashMap::find(m, k))
}

// Places every entry again in a table of n slots (n a power of two, at
// least the current size).
fn HashMap::reslot<K, V>(ref<HashMap<K, V>, exclusive> m, usize n)
{
    usize old = Vec::len(&m.slots);
    usize i = 0;
    while (i < n)
    {
        if (i < old)
        {
            *Vec::index_exclusive(&mut m.slots, i) = max_value<usize>();
        }
        else
        {
            Vec::push(&mut m.slots, max_value<usize>());
        }
        i = i + 1;
    }
    usize e = 0;
    while (e < Vec::len(&m.keys))
    {
        usize s = HashMap::find(&*m, Vec::index_shared(&m.keys, e));
        *Vec::index_exclusive(&mut m.slots, s) = e;
        e = e + 1;
    }
}

// Room for one more entry: the table stays at most three-quarters full.
fn HashMap::reserve_one<K, V>(ref<HashMap<K, V>, exclusive> m)
{
    usize n = Vec::len(&m.slots);
    if ((Vec::len(&m.keys) + 1) * 4 > n * 3)
    {
        HashMap::reslot(m, n * 2);
    }
}

// Adds k with value v. If k was present, its value is replaced where it
// stands and the old one returned; the stored key stays and k is
// destroyed.
export fn HashMap::insert<K, V>(ref<HashMap<K, V>, exclusive> m, K k, V v) : Option<V>
{
    HashMap::reserve_one(m);
    usize s = HashMap::find(&*m, &k);
    usize e = *Vec::index_shared(&m.slots, s);
    if (e == max_value<usize>())
    {
        *Vec::index_exclusive(&mut m.slots, s) = Vec::len(&m.keys);
        Vec::push(&mut m.keys, k);
        Vec::push(&mut m.values, v);
        return None;
    }
    Some(Vec::replace(&mut m.values, e, v))
}

// The value for k. If k is absent, it is added with `fresh` first; if it
// is present, `k` and `fresh` are not needed and are destroyed here.
export fn HashMap::entry<K, V>(ref<HashMap<K, V>, exclusive> m, K k, V fresh) : ref<V, exclusive>
{
    HashMap::reserve_one(m);
    usize s = HashMap::find(&*m, &k);
    usize e = *Vec::index_shared(&m.slots, s);
    if (e == max_value<usize>())
    {
        e = Vec::len(&m.keys);
        *Vec::index_exclusive(&mut m.slots, s) = e;
        Vec::push(&mut m.keys, k);
        Vec::push(&mut m.values, fresh);
    }
    Vec::index_exclusive(&mut m.values, e)
}

export fn HashMap::get<K, V>(ref<HashMap<K, V>, shared> m, ref<K, shared> k) : Option<ref<V, shared>>
{
    usize e = HashMap::index_of(m, k);
    if (e == max_value<usize>())
    {
        return None;
    }
    Some(Vec::index_shared(&m.values, e))
}

export fn HashMap::get_mut<K, V>(ref<HashMap<K, V>, exclusive> m, ref<K, shared> k) : Option<ref<V, exclusive>>
{
    usize e = HashMap::index_of(&*m, k);
    if (e == max_value<usize>())
    {
        return None;
    }
    Some(Vec::index_exclusive(&mut m.values, e))
}

export fn HashMap::contains<K, V>(ref<HashMap<K, V>, shared> m, ref<K, shared> k) : bool
{
    HashMap::index_of(m, k) != max_value<usize>()
}

// Empties slot s, moving later slots of its probe run back so that
// every entry stays reachable from its home slot (backward-shift
// deletion: the table needs no tombstones).
fn HashMap::unslot<K, V>(ref<HashMap<K, V>, exclusive> m, usize s)
{
    usize mask = Vec::len(&m.slots) - 1;
    usize i = s;
    usize j = s;
    bool shifting = true;
    while (shifting)
    {
        j = (j + 1) & mask;
        usize e = *Vec::index_shared(&m.slots, j);
        if (e == max_value<usize>())
        {
            shifting = false;
        }
        else
        {
            usize home = narrow_wrapping<usize>(key_hash(Vec::index_shared(&m.keys, e))) & mask;
            // The entry in j may move back to i unless its home lies in (i, j], cyclically.
            bool stays = if (i <= j)
            {
                home > i && home <= j
            }
            else
            {
                home > i || home <= j
            };
            if (!stays)
            {
                *Vec::index_exclusive(&mut m.slots, i) = e;
                i = j;
            }
        }
    }
    *Vec::index_exclusive(&mut m.slots, i) = max_value<usize>();
}

// Removes k and returns its value, in constant time: the last entry
// moves into k's place, so insertion order holds only up to the first
// removal. `remove_ordered` keeps the order.
export fn HashMap::remove<K, V>(ref<HashMap<K, V>, exclusive> m, ref<K, shared> k) : Option<V>
{
    usize s = HashMap::find(&*m, k);
    usize e = *Vec::index_shared(&m.slots, s);
    if (e == max_value<usize>())
    {
        return None;
    }
    usize last = Vec::len(&m.keys) - 1;
    if (e != last)
    {
        // The last entry's slot names e from now on.
        usize t = HashMap::find(&*m, Vec::index_shared(&m.keys, last));
        *Vec::index_exclusive(&mut m.slots, t) = e;
    }
    auto k_last = Vec::remove_at(&mut m.keys, last);
    auto v_last = Vec::remove_at(&mut m.values, last);
    auto v = if (e != last)
    {
        drop(Vec::replace(&mut m.keys, e, k_last));
        Vec::replace(&mut m.values, e, v_last)
    }
    else
    {
        drop(k_last);
        v_last
    };
    HashMap::unslot(m, s);
    Some(v)
}

// Removes k and returns its value; the other entries keep their order.
// It takes time in proportion to the size of the map.
export fn HashMap::remove_ordered<K, V>(ref<HashMap<K, V>, exclusive> m, ref<K, shared> k) : Option<V>
{
    usize s = HashMap::find(&*m, k);
    usize e = *Vec::index_shared(&m.slots, s);
    if (e == max_value<usize>())
    {
        return None;
    }
    HashMap::unslot(m, s);
    drop(Vec::remove_at(&mut m.keys, e));
    auto v = Vec::remove_at(&mut m.values, e);
    // Entries after e moved down by one.
    usize i = 0;
    while (i < Vec::len(&m.slots))
    {
        usize x = *Vec::index_shared(&m.slots, i);
        if (x != max_value<usize>() && x > e)
        {
            *Vec::index_exclusive(&mut m.slots, i) = x - 1;
        }
        i = i + 1;
    }
    Some(v)
}

// A consumed HashMap that `foreach (k, v in m)` takes entries from, key
// then value, in order; the entries not yet taken are destroyed with it.
resource struct HashDrain<K, V>
{
    HashMap<K, V> m;
    usize next;
}

fn HashMap::drain<K, V>(HashMap<K, V> m) : HashDrain<K, V>
{
    HashDrain { .m = m, .next = 0 }
}

fn HashDrain::total<K, V>(ref<HashDrain<K, V>, shared> d) : usize
{
    d.m.keys.len
}

fn HashDrain::take_key<K, V>(ref<HashDrain<K, V>, exclusive> d) : K
{
    Vec::take_raw(&mut d.m.keys, d.next)
}

fn HashDrain::take_value<K, V>(ref<HashDrain<K, V>, exclusive> d) : V
{
    auto v = Vec::take_raw(&mut d.m.values, d.next);
    d.next = d.next + 1;
    v
}

fn HashDrain::drop<K, V>(ref<HashDrain<K, V>, exclusive> self)
{
    while (self.next < self.m.keys.len)
    {
        drop(Vec::take_raw(&mut self.m.keys, self.next));
        drop(Vec::take_raw(&mut self.m.values, self.next));
        self.next = self.next + 1;
    }
    self.m.keys.len = 0;
    self.m.values.len = 0;
}

// A set of keys: a HashMap whose values are not used.
export struct HashSet<K>
{
    HashMap<K, bool> map;
}

export fn HashSet::new<K>() : HashSet<K>
{
    HashSet { .map = HashMap::new() }
}

export fn HashSet::len<K>(ref<HashSet<K>, shared> s) : usize
{
    HashMap::len(&s.map)
}

// Adds k; true if it was not already present.
export fn HashSet::insert<K>(ref<HashSet<K>, exclusive> s, K k) : bool
{
    match (HashMap::insert(&mut s.map, k, true))
    {
        Some(_) : false,
        None : true,
    }
}

export fn HashSet::contains<K>(ref<HashSet<K>, shared> s, ref<K, shared> k) : bool
{
    HashMap::contains(&s.map, k)
}

// Removes k; true if it was present. As `HashMap::remove`, the last key
// takes k's place.
export fn HashSet::remove<K>(ref<HashSet<K>, exclusive> s, ref<K, shared> k) : bool
{
    match (HashMap::remove(&mut s.map, k))
    {
        Some(_) : true,
        None : false,
    }
}

// Removes k keeping the order of the others; true if it was present.
export fn HashSet::remove_ordered<K>(ref<HashSet<K>, exclusive> s, ref<K, shared> k) : bool
{
    match (HashMap::remove_ordered(&mut s.map, k))
    {
        Some(_) : true,
        None : false,
    }
}

// A consumed HashSet that `foreach (k in s)` takes keys from.
struct SetDrain<K>
{
    HashDrain<K, bool> d;
}

fn HashSet::drain<K>(HashSet<K> s) : SetDrain<K>
{
    HashSet { map } = s;
    SetDrain { .d = HashMap::drain(map) }
}

fn SetDrain::total<K>(ref<SetDrain<K>, shared> s) : usize
{
    HashDrain::total(&s.d)
}

fn SetDrain::take<K>(ref<SetDrain<K>, exclusive> s) : K
{
    auto k = HashDrain::take_key(&mut s.d);
    HashDrain::take_value(&mut s.d);
    k
}

// Key i in insertion order, for i < len.
export fn HashSet::at<K>(ref<HashSet<K>, shared> s, usize i) : ref<K, shared>
{
    HashMap::key_at(&s.map, i)
}

struct RcBox<T>
{
    usize count;
    T value;
}

export resource struct Rc<T>
{
    rawptr<RcBox<T>> ptr;
}

export fn Rc::new<T>(T v) : Rc<T>
{
    rawptr<u8> p = match (allocate(sizeof<RcBox<T>>(), alignof<RcBox<T>>()))
    {
        Ok(p) : p,
        Err(_) : fault(alloc_failure),
    };
    rawptr<RcBox<T>> bp = reinterpret_ptr<RcBox<T>>(p);
    unsafe
    {
        *bp = RcBox { .count = 1, .value = v };
    }
    Rc { .ptr = bp }
}

export fn Rc::clone<T>(ref<Rc<T>, shared> r) : Rc<T>
{
    unsafe
    {
        ref<RcBox<T>, exclusive> b = &mut reclaim<RcBox<T>>(r.ptr);
        b.count = b.count + 1;
    }
    Rc { .ptr = r.ptr }
}

export fn Rc::get<T>(ref<Rc<T>, shared> r) : ref<T, shared>
{
    unsafe
    {
        &reclaim<RcBox<T>>(r.ptr).value
    }
}

export fn Rc::drop<T>(ref<Rc<T>, exclusive> self)
{
    bool last = false;
    unsafe
    {
        ref<RcBox<T>, exclusive> b = &mut reclaim<RcBox<T>>(self.ptr);
        b.count = b.count - 1;
        last = b.count == 0;
    }
    if (last)
    {
        unsafe
        {
            drop(reclaim<RcBox<T>>(self.ptr));
            deallocate(reinterpret_ptr<u8>(self.ptr), sizeof<RcBox<T>>(), alignof<RcBox<T>>());
        }
    }
}

// A value on the heap with one owner (`rule.stdlib.box`, spec/21 §3a,
// D-0045): `Rc` without the count. It gives recursive types their
// indirection (`struct Node { i32 v; Option<Box<Node>> next; }`) and
// moves a large value as a pointer.
export resource struct Box<T>
{
    rawptr<T> ptr;
    bool full;                          // false once into_inner has taken the value
}

// The size Box allocates: at least one byte, so every Box has an address.
fn Box::cells<T>() : usize
{
    if (sizeof<T>() == 0)
    {
        1
    }
    else
    {
        sizeof<T>()
    }
}

export fn Box::new<T>(T v) : Box<T>
{
    rawptr<u8> p = match (allocate(Box::cells<T>(), alignof<T>()))
    {
        Ok(p) : p,
        Err(_) : fault(alloc_failure),
    };
    rawptr<T> bp = reinterpret_ptr<T>(p);
    unsafe
    {
        *bp = v;
    }
    Box { .ptr = bp, .full = true }
}

export fn Box::get<T>(ref<Box<T>, shared> b) : ref<T, shared>
{
    unsafe
    {
        &reclaim<T>(b.ptr)
    }
}

export fn Box::get_mut<T>(ref<Box<T>, exclusive> b) : ref<T, exclusive>
{
    unsafe
    {
        &mut reclaim<T>(b.ptr)
    }
}

// The value, moved out; the Box's memory is freed.
export fn Box::into_inner<T>(Box<T> b) : T
{
    b.full = false;
    unsafe
    {
        auto v = *b.ptr;
        release(reinterpret_ptr<u8>(b.ptr), sizeof<T>());
        v
    }
}

export fn Box::drop<T>(ref<Box<T>, exclusive> self)
{
    unsafe
    {
        if (self.full)
        {
            drop(reclaim<T>(self.ptr));
        }
        deallocate(reinterpret_ptr<u8>(self.ptr), Box::cells<T>(), alignof<T>());
    }
}

// Exchanging values behind references (`rule.stdlib.swap`, spec/21 §3b,
// D-0048). Moving out of a borrowed place is refused, so a program cannot
// exchange two values it holds only by reference; these do it inside
// `std`. `swap_places` is a std-only intrinsic: it exchanges the two
// places' contents (a resource stays owned by the place it is now in).
export fn swap<T>(ref<T, exclusive> a, ref<T, exclusive> b)
{
    swap_places(a, b);
}

// Puts v in *r and returns what was there.
export fn replace<T>(ref<T, exclusive> r, T v) : T
{
    auto old = v;
    swap_places(r, &mut old);
    old
}

// Exchanges elements i and j of v; nothing when i == j.
export fn Vec::swap<T>(ref<Vec<T>, exclusive> v, usize i, usize j)
{
    if (i >= v.len || j >= v.len)
    {
        fault(index_out_of_bounds);
    }
    if (i != j)
    {
        swap_places(&mut (*v)[i], &mut (*v)[j]);
    }
}

// Exchanges elements i and j of s; nothing when i == j.
export fn slice_swap<T>(slice<T, exclusive> s, usize i, usize j)
{
    if (i >= slice_len(s) || j >= slice_len(s))
    {
        fault(index_out_of_bounds);
    }
    if (i != j)
    {
        swap_places(&mut s[i], &mut s[j]);
    }
}
}
"#;

/// The number of source lines `PRELUDE_SRC` itself occupies inside the
/// combined `"{prelude}\n{user src}"` text every parse/typecheck/eval
/// pass actually runs on (`lib.rs`'s `build_interp`/
/// `check_source_static_raw` -- one token stream, deliberately, so the
/// disambiguation prescan sees the prelude's own struct/enum names).
/// Every `Expr::line`/fault-tagged line number is a line number *in
/// that combined text*, not in the user's own file, until this offset
/// is subtracted back out -- exactly what `main.rs` does before
/// rendering a diagnostic, so "at line N" means N in the file the user
/// actually wrote, never a line inside invisible prelude source they
/// never see.
pub fn line_count() -> usize {
    // Counting '\n' characters (not `.lines().count()`) so this exactly
    // mirrors `build_interp`'s actual join, `format!("{}\n{}", PRELUDE_SRC,
    // src)`, character for character: PRELUDE_SRC's own content already
    // ends in `\n`, and the join adds one *more* `\n` as a separator --
    // together that's one real blank line between the prelude and the
    // user's own source, which `.lines().count()` alone (283, not 284)
    // would silently omit, undercounting the offset by exactly one line.
    PRELUDE_SRC.matches('\n').count() + 1
}
