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
    let e = Entry { file, writable: mode != MODE_READ, ahead: Vec::new(), at: 0 };
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

// At most `cap` bytes, from the read-ahead first; none at the end.
pub fn read_some(h: u64, cap: u64) -> Result<Vec<u8>, i64> {
    with(h, |e| {
        if cap == 0 {
            return Ok(Vec::new());
        }
        if e.at == e.ahead.len() {
            fill(e)?;
        }
        let k = (e.ahead.len() - e.at).min(cap as usize);
        let out = e.ahead[e.at..e.at + k].to_vec();
        e.at += k;
        Ok(out)
    })
}

// The length of the next line, its '\n' included (the rest of the file
// when there is none); 0 at the end. Its bytes are then in the
// read-ahead, and `read_some` takes them.
pub fn line_len(h: u64) -> Result<u64, i64> {
    with(h, |e| {
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
        let d = tmp("a");
        let f = p(&d.join("log.txt"));
        assert_eq!(open(&p(&d.join("none")), MODE_READ), Err(NOT_FOUND));
        let h = open(&f, MODE_CREATE).unwrap();
        write_all(h, b"one\ntwo\nlast").unwrap();
        close(h, true).unwrap();
        let h = open(&f, MODE_APPEND).unwrap();
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
        assert_eq!(write_all(h, b"x").is_err(), true);
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
        close(h, false).unwrap();
        // A directory is not a file (Windows refuses to open one).
        if cfg!(unix) {
            assert_eq!(open(&p(&d), MODE_READ), Err(OTHER));
        }
        let _ = std::fs::remove_dir_all(&d);
    }
}
