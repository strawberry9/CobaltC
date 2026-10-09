// Standard output's buffer (D-0178, `rule.stdlib.print` [Print-Buffer]),
// shared by `coby` and `cbrt`. Every byte a program writes to standard
// output passes through `write`; it reaches the operating system when the
// buffer fills, at a newline when standard output is a terminal, and at
// `flush`, which both implementations call before a write to standard
// error, a read of standard input, a child that shares standard output,
// a fault's report and the program's end. Within the program, standard
// output and standard error therefore stay in the order it wrote them.
use std::io::{IsTerminal, Write};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Mutex, MutexGuard, OnceLock};

// The buffer's size: a full buffer is one `write(2)`.
const CAP: usize = 8192;

static BUF: Mutex<Vec<u8>> = Mutex::new(Vec::new());
static TTY: OnceLock<bool> = OnceLock::new();
// Whether the buffer may hold bytes: `flush` with nothing buffered (before
// each read of standard input, a byte at a time for `read_line`) then
// costs one load. Set by the writing thread itself, so a thread always
// sees its own writes.
static PENDING: AtomicBool = AtomicBool::new(false);

fn buf() -> MutexGuard<'static, Vec<u8>> {
    BUF.lock().unwrap_or_else(|e| e.into_inner())
}

// A terminal is written a line at a time, so a person sees each line as
// the program finishes it.
fn tty() -> bool {
    *TTY.get_or_init(|| std::io::stdout().is_terminal())
}

fn put(bytes: &[u8]) -> std::io::Result<()> {
    if bytes.is_empty() {
        return Ok(());
    }
    let mut out = std::io::stdout().lock();
    out.write_all(bytes).and_then(|_| out.flush())
}

fn drain(b: &mut Vec<u8>) -> std::io::Result<()> {
    let r = put(b);
    b.clear();
    PENDING.store(false, Ordering::Release);
    r
}

/// Writes `bytes` to standard output through the buffer. An error is the
/// operating system's, from this call's own write or from an earlier
/// call's bytes written now.
pub fn write(bytes: &[u8]) -> std::io::Result<()> {
    let mut b = buf();
    if b.len() + bytes.len() > CAP {
        drain(&mut b)?;
        if bytes.len() >= CAP {
            return put(bytes);
        }
    }
    b.extend_from_slice(bytes);
    PENDING.store(true, Ordering::Release);
    if b.len() == CAP || (tty() && bytes.contains(&b'\n')) {
        return drain(&mut b);
    }
    Ok(())
}

/// Writes out whatever the buffer holds.
pub fn flush() -> std::io::Result<()> {
    if !PENDING.load(Ordering::Acquire) {
        return Ok(());
    }
    drain(&mut buf())
}

/// Writes `bytes` to standard error, after standard output's buffer.
pub fn write_err(bytes: &[u8]) -> std::io::Result<()> {
    let _ = flush();
    let mut out = std::io::stderr().lock();
    out.write_all(bytes).and_then(|_| out.flush())
}
