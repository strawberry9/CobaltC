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
    std::fs::read(path(p)?).map_err(|e| code(&e))
}

pub fn write(p: &[u8], data: &[u8]) -> Result<(), i64> {
    std::fs::write(path(p)?, data).map_err(|e| code(&e))
}


// How `File::open`, `create`, `append` and `open_rw` open a file.
pub const MODE_READ: u64 = 0;
pub const MODE_CREATE: u64 = 1;
pub const MODE_APPEND: u64 = 2;
pub const MODE_READ_WRITE: u64 = 3;

// What is read from the operating system at a time.
const CHUNK: usize = 8192;

struct Entry {
    file: std::fs::File,
    readable: bool,
    writable: bool,
    ahead: Vec<u8>,
    at: usize,
}

static TABLE: std::sync::Mutex<Vec<Option<Entry>>> = std::sync::Mutex::new(Vec::new());

fn with<T>(h: u64, f: impl FnOnce(&mut Entry) -> Result<T, i64>) -> Result<T, i64> {
    let mut t = TABLE.lock().unwrap_or_else(|e| e.into_inner());
    match t.get_mut(h as usize).and_then(|e| e.as_mut()) {
        Some(e) => f(e),
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
    let mut o = std::fs::OpenOptions::new();
    match mode {
        MODE_READ => o.read(true),
        MODE_CREATE => o.write(true).create(true).truncate(true),
        MODE_APPEND => o.append(true).create(true),
        MODE_READ_WRITE => o.read(true).write(true).create(true),
        _ => return Err(OTHER),
    };
    let file = o.open(path(p)?).map_err(|e| code(&e))?;
    // A directory opens on some systems; it is not a file to read.
    if file.metadata().map_or(true, |m| m.is_dir()) {
        return Err(OTHER);
    }
    let readable = mode == MODE_READ || mode == MODE_READ_WRITE;
    let e = Entry { file, readable, writable: mode != MODE_READ, ahead: Vec::new(), at: 0 };
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
    let e = {
        let mut t = TABLE.lock().unwrap_or_else(|e| e.into_inner());
        match t.get_mut(h as usize).and_then(|e| e.take()) {
            Some(e) => e,
            None => return Err(OTHER),
        }
    };
    if sync && e.writable {
        e.file.sync_all().map_err(|x| code(&x))?;
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
    with(h, |e| {
        // Not opened for writing: `Io`, the same everywhere (as reading).
        if !e.writable {
            return Err(OTHER);
        }
        unread(e)?;
        e.file.write_all(data).map_err(|x| code(&x))
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

// `std::sleep_ns(ns)` (D-0124): the calling thread sleeps at least `ns`
// nanoseconds.
pub fn sleep_ns(ns: u64) {
    std::thread::sleep(std::time::Duration::from_nanos(ns));
}

// `std::clock_read(which)`: 0 a monotonic clock's nanoseconds (from an
// arbitrary start, the same for the whole program); 1 the wall clock's
// seconds since 1970-01-01 UTC (negative before it).
pub fn clock_read(which: u64) -> i64 {
    use std::sync::OnceLock;
    static START: OnceLock<std::time::Instant> = OnceLock::new();
    match which {
        0 => START.get_or_init(std::time::Instant::now).elapsed().as_nanos() as i64,
        _ => match std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH) {
            Ok(d) => d.as_secs() as i64,
            Err(e) => -(e.duration().as_secs() as i64),
        },
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

