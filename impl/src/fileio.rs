// `read_file`'s and `write_file`'s file-system access (`spec/21` §2e
// `rule.stdlib.file`). One source file for both implementations: the
// interpreter uses it as a module and the compiler's runtime (`cbrt`)
// includes this same file, so `coby` and `cobc` read, write and fail
// alike. A path is the bytes of a `String`, so it is UTF-8; it is
// resolved as the operating system resolves it (relative to the working
// directory).
//
// Failures are codes: -1 the file does not exist, -2 access is denied,
// -3 anything else (a directory, a device error, ...).
//
// `File` (`rule.stdlib.file-handle`, D-0054) is a table of open files,
// shared by every thread: a `File`'s handle is its index. Each entry
// keeps a read-ahead buffer, so `read_line` does not read a byte at a
// time; a write or a seek first gives back what was read ahead but not
// taken, so the file's position is always the one the program sees.
//
// D-0201 (`[File-Buffer]`): writes are buffered too, `OUT_CAP` bytes per
// entry. The bytes reach the operating system before any other operation
// on the same file, before any other file-system operation of the
// program (`flush_all`, at every path-taking entry point here and in
// `procio` before a child starts), and at the program's end (both tools
// call `flush_all` there). A failure found while writing them out is kept
// (`failed`) and reported by the file's next operation.

pub const NOT_FOUND: i64 = -1;
pub const DENIED: i64 = -2;
pub const OTHER: i64 = -3;

fn code(e: &std::io::Error) -> i64 {
    match e.kind() {
        std::io::ErrorKind::NotFound => NOT_FOUND,
        std::io::ErrorKind::PermissionDenied => DENIED,
        _ => OTHER,
    }
}

fn path(p: &[u8]) -> Result<&str, i64> {
    std::str::from_utf8(p).map_err(|_| OTHER)
}

pub fn read(p: &[u8]) -> Result<Vec<u8>, i64> {
    flush_all();
    std::fs::read(path(p)?).map_err(|e| code(&e))
}

pub fn write(p: &[u8], data: &[u8]) -> Result<(), i64> {
    flush_all();
    std::fs::write(path(p)?, data).map_err(|e| code(&e))
}


// How `File::open`, `create`, `append` and `open_rw` open a file.
pub const MODE_READ: u64 = 0;
pub const MODE_CREATE: u64 = 1;
pub const MODE_APPEND: u64 = 2;
pub const MODE_READ_WRITE: u64 = 3;
// D-0181: created only if nothing is there; `EXISTS` when something is.
pub const MODE_CREATE_NEW: u64 = 4;
pub const EXISTS: i64 = -4;

// What is read from the operating system at a time.
const CHUNK: usize = 8192;

// D-0201: the write buffer's size; a write as large is not buffered.
const OUT_CAP: usize = 8192;

struct Entry {
    file: std::fs::File,
    readable: bool,
    writable: bool,
    ahead: Vec<u8>,
    at: usize,
    // D-0201: bytes written but not yet handed to the operating system,
    // and a failure found handing them over, for the next operation.
    out: Vec<u8>,
    failed: Option<i64>,
}

static TABLE: std::sync::Mutex<Vec<Option<Entry>>> = std::sync::Mutex::new(Vec::new());
// D-0201: how many entries hold buffered bytes; `flush_all` with none
// costs one load.
static DIRTY: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);

// Hands the entry's buffered bytes to the operating system.
fn drain_out(e: &mut Entry) -> Result<(), i64> {
    use std::io::Write;
    if e.out.is_empty() {
        return Ok(());
    }
    let r = e.file.write_all(&e.out).map_err(|x| code(&x));
    e.out.clear();
    DIRTY.fetch_sub(1, std::sync::atomic::Ordering::AcqRel);
    r
}

// The entry's buffered bytes written out, and any failure so far taken:
// what every operation on a file does first.
fn settle(e: &mut Entry) -> Result<(), i64> {
    let r = drain_out(e);
    if let Some(c) = e.failed.take() {
        return Err(c);
    }
    r
}

/// D-0201: every file's buffered bytes handed to the operating system (a
/// failure is kept for that file's next operation). Called before any
/// file-system operation, before a child process starts and at the
/// program's end.
pub fn flush_all() {
    if DIRTY.load(std::sync::atomic::Ordering::Acquire) == 0 {
        return;
    }
    let mut t = TABLE.lock().unwrap_or_else(|e| e.into_inner());
    for e in t.iter_mut().flatten() {
        if let Err(c) = drain_out(e) {
            e.failed.get_or_insert(c);
        }
    }
}

fn with<T>(h: u64, f: impl FnOnce(&mut Entry) -> Result<T, i64>) -> Result<T, i64> {
    let mut t = TABLE.lock().unwrap_or_else(|e| e.into_inner());
    match t.get_mut(h as usize).and_then(|e| e.as_mut()) {
        Some(e) => {
            settle(e)?;
            f(e)
        }
        None => Err(OTHER),
    }
}

// `with` for a write: the buffer stays; only a failure kept from an
// earlier hand-over is reported.
fn with_buffered<T>(h: u64, f: impl FnOnce(&mut Entry) -> Result<T, i64>) -> Result<T, i64> {
    let mut t = TABLE.lock().unwrap_or_else(|e| e.into_inner());
    match t.get_mut(h as usize).and_then(|e| e.as_mut()) {
        Some(e) => {
            if let Some(c) = e.failed.take() {
                return Err(c);
            }
            f(e)
        }
        None => Err(OTHER),
    }
}

fn fill(e: &mut Entry) -> Result<usize, i64> {
    use std::io::Read;
    if e.at > 0 {
        e.ahead.drain(..e.at);
        e.at = 0;
    }
    let old = e.ahead.len();
    e.ahead.resize(old + CHUNK, 0);
    let r = loop {
        match e.file.read(&mut e.ahead[old..]) {
            Err(x) if x.kind() == std::io::ErrorKind::Interrupted => continue,
            other => break other,
        }
    };
    let n = r.as_ref().map_or(0, |n| *n);
    e.ahead.truncate(old + n);
    r.map_err(|x| code(&x))
}

// Gives back the bytes read ahead but not taken: the operating system's
// position becomes the program's.
fn unread(e: &mut Entry) -> Result<(), i64> {
    use std::io::{Seek, SeekFrom};
    let rest = (e.ahead.len() - e.at) as i64;
    e.ahead.clear();
    e.at = 0;
    if rest > 0 {
        e.file.seek(SeekFrom::Current(-rest)).map_err(|x| code(&x))?;
    }
    Ok(())
}

pub fn open(p: &[u8], mode: u64) -> Result<u64, i64> {
    flush_all();
    let mut o = std::fs::OpenOptions::new();
    match mode {
        MODE_READ => o.read(true),
        MODE_CREATE => o.write(true).create(true).truncate(true),
        MODE_APPEND => o.append(true).create(true),
        MODE_READ_WRITE => o.read(true).write(true).create(true),
        MODE_CREATE_NEW => o.write(true).create_new(true),
        _ => return Err(OTHER),
    };
    let file = o.open(path(p)?).map_err(|e| if mode == MODE_CREATE_NEW && e.kind() == std::io::ErrorKind::AlreadyExists { EXISTS } else { code(&e) })?;
    // A directory opens on some systems; it is not a file to read.
    if file.metadata().map_or(true, |m| m.is_dir()) {
        return Err(OTHER);
    }
    let readable = mode == MODE_READ || mode == MODE_READ_WRITE;
    let e = Entry { file, readable, writable: mode != MODE_READ, ahead: Vec::new(), at: 0, out: Vec::new(), failed: None };
    let mut t = TABLE.lock().unwrap_or_else(|e| e.into_inner());
    match t.iter().position(|s| s.is_none()) {
        Some(i) => {
            t[i] = Some(e);
            Ok(i as u64)
        }
        None => {
            t.push(Some(e));
            Ok((t.len() - 1) as u64)
        }
    }
}

// Closes the file. With `sync`, a file open for writing is first made
// durable, so an error the operating system reports late (a full disk,
// a lost network file) is reported here; without it (a destructor),
// nothing is reported.
pub fn close(h: u64, sync: bool) -> Result<(), i64> {
    let mut e = {
        let mut t = TABLE.lock().unwrap_or_else(|e| e.into_inner());
        match t.get_mut(h as usize).and_then(|e| e.take()) {
            Some(e) => e,
            None => return Err(OTHER),
        }
    };
    // D-0201: the buffered bytes first; their failure is reported by
    // `close`, not by a destructor.
    let settled = settle(&mut e);
    if sync {
        settled?;
        if e.writable {
            e.file.sync_all().map_err(|x| code(&x))?;
        }
    }
    Ok(())
}

// Up to `cap` bytes, from the read-ahead first and then from the file
// directly, fewer only at the end of the file; none at the end.
pub fn read_some(h: u64, cap: u64) -> Result<Vec<u8>, i64> {
    use std::io::Read;
    with(h, |e| {
        let cap = cap as usize;
        // A file not opened for reading: `Io` on every platform, rather
        // than whatever the operating system would say (EBADF on Linux,
        // access denied on Windows).
        if !e.readable {
            return Err(OTHER);
        }
        if cap == 0 {
            return Ok(Vec::new());
        }
        if e.at == e.ahead.len() && cap < CHUNK {
            fill(e)?;
        }
        let k = (e.ahead.len() - e.at).min(cap);
        let mut out = e.ahead[e.at..e.at + k].to_vec();
        e.at += k;
        // The rest straight from the file (the read-ahead is empty now,
        // or `out` is already full).
        while out.len() < cap {
            let old = out.len();
            out.resize(cap, 0);
            let r = loop {
                match e.file.read(&mut out[old..]) {
                    Err(x) if x.kind() == std::io::ErrorKind::Interrupted => continue,
                    other => break other,
                }
            };
            match r {
                Ok(0) => {
                    out.truncate(old);
                    break;
                }
                Ok(n) => out.truncate(old + n),
                Err(x) => {
                    out.truncate(old);
                    if old == 0 {
                        return Err(code(&x));
                    }
                    break;
                }
            }
        }
        Ok(out)
    })
}

// The length of the next line, its '\n' included (the rest of the file
// when there is none); 0 at the end. Its bytes are then in the
// read-ahead, and `read_some` takes them.
pub fn line_len(h: u64) -> Result<u64, i64> {
    with(h, |e| {
        if !e.readable {
            return Err(OTHER);
        }
        let mut from = e.at;
        loop {
            if let Some(i) = e.ahead[from..].iter().position(|b| *b == b'\n') {
                return Ok((from + i + 1 - e.at) as u64);
            }
            let seen = e.ahead.len() - e.at;
            if fill(e)? == 0 {
                return Ok((e.ahead.len() - e.at) as u64);
            }
            from = e.at + seen;
        }
    })
}

pub fn write_all(h: u64, data: &[u8]) -> Result<(), i64> {
    use std::io::Write;
    with_buffered(h, |e| {
        // Not opened for writing: `Io`, the same everywhere (as reading).
        if !e.writable {
            return Err(OTHER);
        }
        unread(e)?;
        // D-0201: into the buffer; one as large as the buffer, after what
        // it holds, straight through.
        if e.out.len() + data.len() > OUT_CAP {
            drain_out(e)?;
        }
        if data.len() >= OUT_CAP {
            return e.file.write_all(data).map_err(|x| code(&x));
        }
        if e.out.is_empty() && !data.is_empty() {
            DIRTY.fetch_add(1, std::sync::atomic::Ordering::AcqRel);
        }
        e.out.extend_from_slice(data);
        Ok(())
    })
}

pub fn seek(h: u64, pos: u64) -> Result<(), i64> {
    use std::io::{Seek, SeekFrom};
    with(h, |e| {
        e.ahead.clear();
        e.at = 0;
        e.file.seek(SeekFrom::Start(pos)).map(|_| ()).map_err(|x| code(&x))
    })
}

pub fn len(h: u64) -> Result<u64, i64> {
    with(h, |e| e.file.metadata().map(|m| m.len()).map_err(|x| code(&x)))
}

// D-0181: the program's position: the system's, less what was read ahead
// and not yet taken.
pub fn position(h: u64) -> Result<u64, i64> {
    use std::io::{Seek, SeekFrom};
    with(h, |e| {
        let sys = e.file.seek(SeekFrom::Current(0)).map_err(|x| code(&x))?;
        Ok(sys - (e.ahead.len() - e.at) as u64)
    })
}

// D-0181: everything written made durable (`fsync`).
pub fn flush(h: u64) -> Result<(), i64> {
    with(h, |e| {
        if !e.writable {
            return Err(OTHER);
        }
        e.file.sync_all().map_err(|x| code(&x))
    })
}

// `std::file_op(op, h, buf, n)`, the one primitive `File` is written
// over; both tools call `call`. `OPEN` and `WRITE` take the `n` bytes at
// `buf` as `input` (`takes_input`); `READ` puts its bytes at `buf`. The
// result is the operation's number (a handle, a count, a length, 0) or
// a failure code.
pub const OP_OPEN: u64 = 0;
pub const OP_CLOSE: u64 = 1;
pub const OP_DROP: u64 = 2;
pub const OP_READ: u64 = 3;
pub const OP_LINE_LEN: u64 = 4;
pub const OP_WRITE: u64 = 5;
pub const OP_SEEK: u64 = 6;
pub const OP_LEN: u64 = 7;
pub const OP_FLUSH: u64 = 8; // D-0181, through `file_op`
pub const OP_POSITION: u64 = 9; // D-0181, through `file_at`

pub fn takes_input(op: u64) -> bool {
    op == OP_OPEN || op == OP_WRITE
}

pub fn call(op: u64, h: u64, input: &[u8], n: u64) -> (i64, Vec<u8>) {
    let r = match op {
        OP_OPEN => open(input, h).map(|h| h as i64),
        OP_CLOSE | OP_DROP => close(h, op == OP_CLOSE).map(|_| 0),
        OP_READ => {
            return match read_some(h, n) {
                Ok(v) => (v.len() as i64, v),
                Err(c) => (c, Vec::new()),
            }
        }
        OP_LINE_LEN => line_len(h).map(|n| n as i64),
        OP_WRITE => write_all(h, input).map(|_| 0),
        OP_SEEK => seek(h, n).map(|_| 0),
        OP_LEN => len(h).map(|n| n as i64),
        OP_FLUSH => flush(h).map(|_| 0),
        OP_POSITION => position(h).map(|n| n as i64),
        _ => Err(OTHER),
    };
    (r.unwrap_or_else(|c| c), Vec::new())
}

// ---- the filesystem, the environment and the clocks (D-0061) ----
//
// `std::fs_op(op, a, b)`: 0 `make_dir(a)` (1 created, 0 a directory was
// already there), 1 `remove_file(a)`, 2 `remove_dir(a)` (empty only),
// 3 `rename(a, b)` (replacing a file at b). 0 or a failure code.
pub fn fs_op(op: u64, a: &[u8], b: &[u8]) -> i64 {
    flush_all();
    let r: Result<i64, i64> = (|| {
        let pa = path(a)?;
        match op {
            0 => match std::fs::create_dir(pa) {
                Ok(()) => Ok(1),
                Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {
                    if std::path::Path::new(pa).is_dir() {
                        Ok(0)
                    } else {
                        Err(OTHER)
                    }
                }
                Err(e) => Err(code(&e)),
            },
            1 => std::fs::remove_file(pa).map(|_| 0).map_err(|e| code(&e)),
            2 => std::fs::remove_dir(pa).map(|_| 0).map_err(|e| code(&e)),
            3 => std::fs::rename(pa, path(b)?).map(|_| 0).map_err(|e| code(&e)),
            _ => Err(OTHER),
        }
    })();
    r.unwrap_or_else(|c| c)
}

// `std::fs_query(op, a)`: the bytes of an answer, or a failure code:
// 0 `list_dir(a)`, its entries' names sorted by their bytes, each followed
// by a 0 byte; 1 `path_kind(a)`, a kind byte (0 file, 1 directory, 2
// other) and the length as 8 little-endian bytes; 2 `env_var(a)`, the
// value's bytes (NOT_FOUND when unset); 3 `current_dir()`. Names and
// values are the platform's bytes (`into_encoded_bytes`), which `std`
// checks are UTF-8.
pub fn fs_query(op: u64, a: &[u8]) -> Result<Vec<u8>, i64> {
    flush_all();
    match op {
        0 => {
            let rd = std::fs::read_dir(path(a)?).map_err(|e| code(&e))?;
            let mut names: Vec<Vec<u8>> = Vec::new();
            for ent in rd {
                let ent = ent.map_err(|e| code(&e))?;
                names.push(ent.file_name().into_encoded_bytes());
            }
            names.sort();
            let mut out = Vec::new();
            for n in names {
                out.extend_from_slice(&n);
                out.push(0);
            }
            Ok(out)
        }
        1 => {
            let m = std::fs::metadata(path(a)?).map_err(|e| code(&e))?;
            let kind: u8 = if m.is_file() {
                0
            } else if m.is_dir() {
                1
            } else {
                2
            };
            let mut out = vec![kind];
            out.extend_from_slice(&m.len().to_le_bytes());
            Ok(out)
        }
        2 => match std::env::var_os(path(a)?) {
            Some(v) => Ok(v.into_encoded_bytes()),
            None => Err(NOT_FOUND),
        },
        3 => std::env::current_dir().map(|d| d.into_os_string().into_encoded_bytes()).map_err(|e| code(&e)),
        _ => Err(OTHER),
    }
}

// ---- paths and file metadata (D-0141) ----
//
// `std::path_op(op, h, a, b, out, cap)`: one or two paths in (`a`, `b`),
// the bytes of an answer out, or a number; `h` is unused. A path's
// grammar is the platform's (`std::path`): `C:\` and `\\server\share` on
// Windows. The `PATH_*` part functions and `PATH_NORMALIZE` never touch
// the file system. An absent answer (no parent, no extension, no home)
// is NOT_FOUND.
pub const PATH_JOIN: u64 = 0;
pub const PATH_PARENT: u64 = 1;
pub const PATH_FILE_NAME: u64 = 2;
pub const PATH_STEM: u64 = 3;
pub const PATH_EXTENSION: u64 = 4;
pub const PATH_IS_ABSOLUTE: u64 = 5;
pub const PATH_NORMALIZE: u64 = 6;
pub const PATH_CANONICAL: u64 = 7;
pub const FILE_INFO: u64 = 8;
pub const COPY_FILE: u64 = 9;
pub const MAKE_DIR_ALL: u64 = 10;
pub const REMOVE_DIR_ALL: u64 = 11;
pub const SET_CURRENT_DIR: u64 = 12;
pub const TEMP_DIR: u64 = 13;
pub const HOME_DIR: u64 = 14;
pub const CURRENT_EXE: u64 = 15; // D-0181
pub const HOSTNAME: u64 = 16; // D-0181

fn os_bytes(p: &std::path::Path) -> Vec<u8> {
    p.as_os_str().to_os_string().into_encoded_bytes()
}

// A lexical normalization: `.` parts removed, `x/..` removed where `x`
// is a name, `..` at the start kept, `/..` is `/`; nothing becomes `.`.
fn normalize(p: &std::path::Path) -> std::path::PathBuf {
    use std::path::Component;
    let mut parts: Vec<Component> = Vec::new();
    for c in p.components() {
        match c {
            Component::CurDir => {}
            Component::ParentDir => match parts.last() {
                Some(Component::Normal(_)) => {
                    parts.pop();
                }
                Some(Component::RootDir) | Some(Component::Prefix(_)) => {}
                _ => parts.push(c),
            },
            _ => parts.push(c),
        }
    }
    let out: std::path::PathBuf = parts.iter().collect();
    if out.as_os_str().is_empty() {
        std::path::PathBuf::from(".")
    } else {
        out
    }
}

// The 18-byte record `file_info` decodes: a kind byte (0 file, 1
// directory, 2 other), the length (8 bytes), the last modification in
// seconds since 1970 (8 bytes, rounded down), and a read-only byte; the
// numbers little-endian.
fn info_record(m: &std::fs::Metadata) -> Vec<u8> {
    let kind: u8 = if m.is_file() {
        0
    } else if m.is_dir() {
        1
    } else {
        2
    };
    let modified = match m.modified() {
        Ok(t) => match t.duration_since(std::time::UNIX_EPOCH) {
            Ok(d) => d.as_secs() as i64,
            Err(e) => {
                let d = e.duration();
                -(d.as_secs() as i64) - if d.subsec_nanos() > 0 { 1 } else { 0 }
            }
        },
        Err(_) => 0,
    };
    let mut out = vec![kind];
    out.extend_from_slice(&m.len().to_le_bytes());
    out.extend_from_slice(&modified.to_le_bytes());
    out.push(m.permissions().readonly() as u8);
    out
}

// D-0181: the machine's host name, by `gethostname` on Unix and
// `GetComputerNameExW` (the DNS host name) on Windows.
#[cfg(unix)]
fn hostname() -> Result<Vec<u8>, i64> {
    extern "C" {
        fn gethostname(name: *mut u8, len: usize) -> i32;
    }
    let mut buf = vec![0u8; 256];
    // SAFETY: the buffer is as long as the length passed.
    let r = unsafe { gethostname(buf.as_mut_ptr(), buf.len()) };
    if r != 0 {
        return Err(OTHER);
    }
    let n = buf.iter().position(|b| *b == 0).unwrap_or(buf.len());
    buf.truncate(n);
    Ok(buf)
}

#[cfg(windows)]
fn hostname() -> Result<Vec<u8>, i64> {
    #[link(name = "kernel32")]
    extern "system" {
        fn GetComputerNameExW(kind: i32, buf: *mut u16, size: *mut u32) -> i32;
    }
    const DNS_HOSTNAME: i32 = 5;
    let mut buf = vec![0u16; 256];
    let mut size = buf.len() as u32;
    // SAFETY: `size` is the buffer's length in characters.
    let r = unsafe { GetComputerNameExW(DNS_HOSTNAME, buf.as_mut_ptr(), &mut size) };
    if r == 0 {
        return Err(OTHER);
    }
    buf.truncate(size as usize);
    Ok(String::from_utf16_lossy(&buf).into_bytes())
}

#[cfg(not(any(unix, windows)))]
fn hostname() -> Result<Vec<u8>, i64> {
    Err(OTHER)
}

pub fn path_op(op: u64, a: &[u8], b: &[u8]) -> Result<(i64, Vec<u8>), i64> {
    flush_all();
    use std::path::Path;
    let pa = Path::new(path(a)?);
    let bytes = |v: Vec<u8>| Ok((v.len() as i64, v));
    let some = |p: Option<&std::ffi::OsStr>| match p {
        Some(x) => Ok((x.len() as i64, x.to_os_string().into_encoded_bytes())),
        None => Err(NOT_FOUND),
    };
    match op {
        PATH_JOIN => bytes(os_bytes(&pa.join(path(b)?))),
        PATH_PARENT => some(pa.parent().map(|p| p.as_os_str())),
        PATH_FILE_NAME => some(pa.file_name()),
        PATH_STEM => some(pa.file_stem()),
        PATH_EXTENSION => some(pa.extension()),
        PATH_IS_ABSOLUTE => Ok((pa.is_absolute() as i64, Vec::new())),
        PATH_NORMALIZE => bytes(os_bytes(&normalize(pa))),
        PATH_CANONICAL => {
            let c = std::fs::canonicalize(pa).map_err(|e| code(&e))?;
            // Windows answers in the verbatim form (`\\?\C:\…`); the
            // ordinary form names the same file and is what people write.
            #[cfg(windows)]
            let c = {
                let t = c.to_string_lossy().into_owned();
                match t.strip_prefix(r"\\?\") {
                    Some(rest) if !rest.starts_with("UNC\\") => std::path::PathBuf::from(rest),
                    _ => c,
                }
            };
            bytes(os_bytes(&c))
        }
        FILE_INFO => bytes(info_record(&std::fs::metadata(pa).map_err(|e| code(&e))?)),
        COPY_FILE => std::fs::copy(pa, path(b)?).map(|n| (n as i64, Vec::new())).map_err(|e| code(&e)),
        MAKE_DIR_ALL => std::fs::create_dir_all(pa).map(|_| (0, Vec::new())).map_err(|e| code(&e)),
        REMOVE_DIR_ALL => std::fs::remove_dir_all(pa).map(|_| (0, Vec::new())).map_err(|e| code(&e)),
        SET_CURRENT_DIR => std::env::set_current_dir(pa).map(|_| (0, Vec::new())).map_err(|e| code(&e)),
        TEMP_DIR => bytes(std::env::temp_dir().to_string_lossy().into_owned().into_bytes()),
        CURRENT_EXE => bytes(os_bytes(&std::env::current_exe().map_err(|e| code(&e))?)),
        HOSTNAME => bytes(hostname()?),
        HOME_DIR => {
            let v = std::env::var_os("HOME").filter(|v| !v.is_empty());
            #[cfg(windows)]
            let v = v.or_else(|| std::env::var_os("USERPROFILE").filter(|v| !v.is_empty()));
            match v {
                Some(h) => bytes(h.to_string_lossy().into_owned().into_bytes()),
                None => Err(NOT_FOUND),
            }
        }
        _ => Err(OTHER),
    }
}

pub type ByteCall = fn(u64, u64, &[u8], &[u8], u64) -> (i64, Vec<u8>);

// The shape every byte-answering primitive shares (`path_op`, and
// `proc_op`, `net_op` in their modules): a number, or a failure code, and
// the bytes of an answer, of which the caller copies up to `cap`.
pub fn path_call(op: u64, _h: u64, a: &[u8], b: &[u8], _cap: u64) -> (i64, Vec<u8>) {
    match path_op(op, a, b) {
        Ok(r) => r,
        Err(c) => (c, Vec::new()),
    }
}

// `std::sleep_ns(ns)` (D-0124): the calling thread sleeps at least `ns`
// nanoseconds.
pub fn sleep_ns(ns: u64) {
    std::thread::sleep(std::time::Duration::from_nanos(ns));
}

// `std::clock_read(which)`: 0 a monotonic clock's nanoseconds (from an
// arbitrary start, the same for the whole program); 1 the wall clock's
// seconds since 1970-01-01 UTC (negative before it); 2 the wall clock's
// milliseconds since then (D-0140), rounded down.
pub fn clock_read(which: u64) -> i64 {
    use std::sync::OnceLock;
    static START: OnceLock<std::time::Instant> = OnceLock::new();
    match which {
        0 => START.get_or_init(std::time::Instant::now).elapsed().as_nanos() as i64,
        2 => match std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH) {
            Ok(d) => d.as_millis() as i64,
            Err(e) => -(e.duration().as_nanos().div_ceil(1_000_000) as i64),
        },
        _ => match std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH) {
            Ok(d) => d.as_secs() as i64,
            Err(e) => -(e.duration().as_secs() as i64),
        },
    }
}

// ---- the machine's local time (D-0147) ----
//
// `std::tz_offset(unix)`: the seconds the machine's local time is ahead
// of UTC at that moment (negative when behind), daylight saving included,
// by the operating system's rule for the zone the program runs in. Rust's
// standard library has no local time, so the platform is asked directly;
// the same code in both tools. For a moment the platform has no rule for
// (before its records, beyond its range), the offset at the current
// moment, so a program never gets 0 masquerading as UTC.

// Days since 1970-01-01 of a civil date (H. Hinnant's `days_from_civil`).
fn days_from_civil(y: i64, m: i64, d: i64) -> i64 {
    let y = if m <= 2 { y - 1 } else { y };
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let mp = (m + 9) % 12;
    let doy = (153 * mp + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146097 + doe - 719468
}

// The civil date of a day count (`civil_from_days`): (year, month, day).
#[cfg(windows)]
fn civil_from_days(z: i64) -> (i64, i64, i64) {
    let z = z + 719468;
    let era = z.div_euclid(146097);
    let doe = z - era * 146097;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    (yoe + era * 400 + if m <= 2 { 1 } else { 0 }, m, d)
}

// The seconds since 1970 of a broken-down local time, read as if it were
// UTC; its difference from the moment is the offset.
fn local_seconds(y: i64, m: i64, d: i64, h: i64, mi: i64, s: i64) -> i64 {
    days_from_civil(y, m, d) * 86400 + h * 3600 + mi * 60 + s
}

#[cfg(unix)]
fn platform_offset(unix: i64) -> Option<i64> {
    // `struct tm`'s nine `int`s are the same on every Unix; what follows
    // them (`tm_gmtoff`, `tm_zone` on glibc, musl and the BSDs) is given
    // room and not read, so only the standard fields are relied on.
    #[repr(C)]
    struct Tm {
        sec: i32,
        min: i32,
        hour: i32,
        mday: i32,
        mon: i32,
        year: i32,
        wday: i32,
        yday: i32,
        isdst: i32,
        spare: [u64; 8],
    }
    #[cfg(target_pointer_width = "64")]
    type TimeT = i64;
    #[cfg(not(target_pointer_width = "64"))]
    type TimeT = i32;
    extern "C" {
        fn localtime_r(t: *const TimeT, out: *mut Tm) -> *mut Tm;
    }
    let t = TimeT::try_from(unix).ok()?;
    let mut tm = Tm { sec: 0, min: 0, hour: 0, mday: 0, mon: 0, year: 0, wday: 0, yday: 0, isdst: 0, spare: [0; 8] };
    // SAFETY: `localtime_r` writes the standard fields of `tm`, which has
    // room for the platform's whole `struct tm`; it reads only `t`.
    let p = unsafe { localtime_r(&t, &mut tm) };
    if p.is_null() {
        return None;
    }
    let local = local_seconds(tm.year as i64 + 1900, tm.mon as i64 + 1, tm.mday as i64, tm.hour as i64, tm.min as i64, tm.sec as i64);
    local.checked_sub(unix)
}

#[cfg(windows)]
fn platform_offset(unix: i64) -> Option<i64> {
    #[repr(C)]
    #[derive(Clone, Copy, Default)]
    struct SystemTime {
        year: u16,
        month: u16,
        day_of_week: u16,
        day: u16,
        hour: u16,
        minute: u16,
        second: u16,
        milliseconds: u16,
    }
    #[repr(C)]
    struct TimeZoneInformation {
        bias: i32,
        standard_name: [u16; 32],
        standard_date: SystemTime,
        standard_bias: i32,
        daylight_name: [u16; 32],
        daylight_date: SystemTime,
        daylight_bias: i32,
    }
    #[link(name = "kernel32")]
    extern "system" {
        fn GetTimeZoneInformationForYear(year: u16, dynamic: *mut std::ffi::c_void, out: *mut TimeZoneInformation) -> i32;
        fn SystemTimeToTzSpecificLocalTime(tz: *const TimeZoneInformation, utc: *const SystemTime, local: *mut SystemTime) -> i32;
    }
    let days = unix.div_euclid(86400);
    let secs = unix.rem_euclid(86400);
    let (y, m, d) = civil_from_days(days);
    // A SYSTEMTIME's year is 1601 ..= 30827.
    if !(1601..=30827).contains(&y) {
        return None;
    }
    let utc = SystemTime { year: y as u16, month: m as u16, day_of_week: 0, day: d as u16, hour: (secs / 3600) as u16, minute: (secs % 3600 / 60) as u16, second: (secs % 60) as u16, milliseconds: 0 };
    let mut tz = TimeZoneInformation { bias: 0, standard_name: [0; 32], standard_date: SystemTime::default(), standard_bias: 0, daylight_name: [0; 32], daylight_date: SystemTime::default(), daylight_bias: 0 };
    let mut local = SystemTime::default();
    // SAFETY: the system fills `tz` and `local`, both fully initialized
    // records of the documented layout; `utc` is read.
    unsafe {
        if GetTimeZoneInformationForYear(y as u16, std::ptr::null_mut(), &mut tz) == 0 {
            return None;
        }
        if SystemTimeToTzSpecificLocalTime(&tz, &utc, &mut local) == 0 {
            return None;
        }
    }
    let l = local_seconds(local.year as i64, local.month as i64, local.day as i64, local.hour as i64, local.minute as i64, local.second as i64);
    l.checked_sub(unix)
}

#[cfg(not(any(unix, windows)))]
fn platform_offset(_unix: i64) -> Option<i64> {
    None
}

pub fn tz_offset(unix: i64) -> i64 {
    let o = platform_offset(unix).or_else(|| platform_offset(clock_read(1))).unwrap_or(0);
    o.clamp(-64800, 64800)
}

#[cfg(test)]
mod tz_tests {
    use super::*;

    #[test]
    fn civil_round_trips() {
        assert_eq!(days_from_civil(1970, 1, 1), 0);
        assert_eq!(days_from_civil(2000, 2, 29), 11016);
        assert_eq!(days_from_civil(1969, 12, 31), -1);
        assert_eq!(local_seconds(2026, 10, 3, 10, 17, 0), 1791022620);
    }

    #[test]
    fn offset_is_sane() {
        for unix in [0i64, 951782400, 1791022620, clock_read(1), i64::MAX, i64::MIN] {
            let o = tz_offset(unix);
            assert!((-64800..=64800).contains(&o), "{} -> {}", unix, o);
            assert_eq!(o % 60, 0, "{} -> {}", unix, o);
        }
        assert_eq!(tz_offset(1791022620), tz_offset(1791022620));
    }
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    use std::os::unix::fs::PermissionsExt;

    #[test]
    fn codes() {
        let d = std::env::temp_dir().join(format!("cobaltc-fileio-{}", std::process::id()));
        std::fs::create_dir_all(&d).unwrap();
        let f = d.join("x.txt");
        let fp = f.to_str().unwrap().as_bytes();
        assert_eq!(write(fp, b"abc"), Ok(()));
        assert_eq!(read(fp), Ok(b"abc".to_vec()));
        assert_eq!(read(d.join("none").to_str().unwrap().as_bytes()), Err(NOT_FOUND));
        // A directory is neither missing nor denied.
        assert_eq!(read(d.to_str().unwrap().as_bytes()), Err(OTHER));
        // Unreadable, unless this runs as root (which reads anything).
        std::fs::set_permissions(&f, std::fs::Permissions::from_mode(0o000)).unwrap();
        let root = std::fs::read(&f).is_ok();
        if !root {
            assert_eq!(read(fp), Err(DENIED));
            assert_eq!(write(fp, b"x"), Err(DENIED));
        }
        std::fs::set_permissions(&f, std::fs::Permissions::from_mode(0o644)).unwrap();
        let _ = std::fs::remove_dir_all(&d);
    }
}

#[cfg(test)]
mod handle_tests {
    use super::*;

    // The handle table is the process's, and a closed handle's slot is
    // reused by the next open: a test that closes a handle twice (to see
    // the second fail) must not run while another opens, or its second
    // close lands on the other test's file. These tests take turns.
    static SERIAL: std::sync::Mutex<()> = std::sync::Mutex::new(());

    fn serial() -> std::sync::MutexGuard<'static, ()> {
        SERIAL.lock().unwrap_or_else(|e| e.into_inner())
    }

    fn tmp(name: &str) -> std::path::PathBuf {
        let d = std::env::temp_dir().join(format!("cobaltc-handle-{}-{}", std::process::id(), name));
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    fn p(f: &std::path::Path) -> Vec<u8> {
        f.to_str().unwrap().as_bytes().to_vec()
    }

    #[test]
    fn lines_reads_writes_seeks() {
        let _turn = serial();
        let d = tmp("a");
        let f = p(&d.join("log.txt"));
        assert_eq!(open(&p(&d.join("none")), MODE_READ), Err(NOT_FOUND));
        let h = open(&f, MODE_CREATE).unwrap();
        write_all(h, b"one\ntwo\nlast").unwrap();
        close(h, true).unwrap();
        let h = open(&f, MODE_APPEND).unwrap();
        assert_eq!(read_some(h, 1), Err(OTHER));
        assert_eq!(line_len(h), Err(OTHER));
        write_all(h, b"!").unwrap();
        close(h, false).unwrap();
        let h = open(&f, MODE_READ).unwrap();
        assert_eq!(len(h), Ok(13));
        assert_eq!(line_len(h), Ok(4));
        assert_eq!(read_some(h, 4).unwrap(), b"one\n");
        assert_eq!(line_len(h), Ok(4));
        assert_eq!(read_some(h, 2).unwrap(), b"tw");
        assert_eq!(read_some(h, 100).unwrap(), b"o\nlast!");
        assert_eq!(line_len(h), Ok(0));
        assert_eq!(read_some(h, 100).unwrap(), b"");
        seek(h, 4).unwrap();
        assert_eq!(read_some(h, 3).unwrap(), b"two");
        assert_eq!(write_all(h, b"x"), Err(OTHER));
        close(h, true).unwrap();
        assert_eq!(close(h, true), Err(OTHER));
        // Read-write: a write after reading lands where reading stopped.
        let h = open(&f, MODE_READ_WRITE).unwrap();
        assert_eq!(read_some(h, 4).unwrap(), b"one\n");
        write_all(h, b"TWO").unwrap();
        close(h, true).unwrap();
        assert_eq!(std::fs::read(d.join("log.txt")).unwrap(), b"one\nTWO\nlast!");
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn long_lines_cross_chunks() {
        let _turn = serial();
        let d = tmp("b");
        let f = p(&d.join("big.txt"));
        let mut data = vec![b'a'; CHUNK * 2 + 5];
        data.push(b'\n');
        data.extend_from_slice(b"tail");
        let h = open(&f, MODE_CREATE).unwrap();
        write_all(h, &data).unwrap();
        close(h, true).unwrap();
        let h = open(&f, MODE_READ).unwrap();
        let n = line_len(h).unwrap();
        assert_eq!(n as usize, CHUNK * 2 + 6);
        assert_eq!(read_some(h, n).unwrap().len() as u64, n);
        assert_eq!(line_len(h), Ok(4));
        // A large read gets all that is there, not one chunk.
        seek(h, 0).unwrap();
        assert_eq!(read_some(h, 1 << 20).unwrap().len(), data.len());
        close(h, false).unwrap();
        // A directory is not a file (Windows refuses to open one).
        if cfg!(unix) {
            assert_eq!(open(&p(&d), MODE_READ), Err(OTHER));
        }
        let _ = std::fs::remove_dir_all(&d);
    }
}

#[cfg(test)]
mod fs_tests {
    use super::*;

    #[test]
    fn dirs_files_env_clock() {
        let d = std::env::temp_dir().join(format!("cobaltc-fs-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        let dp = d.to_str().unwrap().as_bytes().to_vec();
        assert_eq!(fs_op(0, &dp, b""), 1);
        assert_eq!(fs_op(0, &dp, b""), 0);
        let f = d.join("b.txt");
        std::fs::write(&f, b"abc").unwrap();
        std::fs::write(d.join("a.txt"), b"").unwrap();
        assert_eq!(fs_query(0, &dp), Ok(b"a.txt\0b.txt\0".to_vec()));
        let fp = f.to_str().unwrap().as_bytes().to_vec();
        let mut k = vec![0u8];
        k.extend_from_slice(&3u64.to_le_bytes());
        assert_eq!(fs_query(1, &fp), Ok(k));
        assert_eq!(fs_query(1, &dp).map(|v| v[0]), Ok(1));
        // A file where a directory is asked for is not "already there".
        assert_eq!(fs_op(0, &fp, b""), OTHER);
        // Not empty.
        assert_eq!(fs_op(2, &dp, b""), OTHER);
        let g = d.join("c.txt");
        let gp = g.to_str().unwrap().as_bytes().to_vec();
        assert_eq!(fs_op(3, &fp, &gp), 0);
        assert_eq!(fs_query(1, &fp), Err(NOT_FOUND));
        assert_eq!(fs_op(1, &gp, b""), 0);
        assert_eq!(fs_op(1, &gp, b""), NOT_FOUND);
        assert_eq!(fs_op(1, d.join("a.txt").to_str().unwrap().as_bytes(), b""), 0);
        assert_eq!(fs_op(2, &dp, b""), 0);
        assert_eq!(fs_query(2, b"COBALTC_SURELY_UNSET_VARIABLE"), Err(NOT_FOUND));
        let a = clock_read(0);
        let b = clock_read(0);
        assert!(b >= a);
        assert!(clock_read(1) > 1_700_000_000);
    }
}


// `std::stdin_read_line` (`read_line`'s primitive): standard input's next
// bytes up to and including the next `\n`, at most `buf.len()` of them,
// from the same buffered `stdin` every other read of it uses; 0 at its
// end, -1 on an error. Standard output's buffer is written out first
// (D-0178), so a prompt is on the screen before the wait.
pub fn read_line_chunk(buf: &mut [u8]) -> i64 {
    use std::io::BufRead;
    let _ = crate::outbuf::flush();
    if buf.is_empty() {
        return 0;
    }
    let mut input = std::io::stdin().lock();
    loop {
        let avail = match input.fill_buf() {
            Ok(a) => a,
            Err(e) if e.kind() == std::io::ErrorKind::Interrupted => continue,
            Err(_) => return -1,
        };
        if avail.is_empty() {
            return 0;
        }
        let k = avail.iter().position(|&b| b == b'\n').map_or(avail.len(), |i| i + 1).min(buf.len());
        buf[..k].copy_from_slice(&avail[..k]);
        input.consume(k);
        return k as i64;
    }
}
