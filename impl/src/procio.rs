// Child processes and interrupt requests (`spec/21` §2k
// `rule.stdlib.process`, D-0143). One source file for both
// implementations: the interpreter uses it as a module and the compiler's
// runtime (`cbrt`) includes this same file, so `coby` and `cobc` run
// programs, and fail, alike.
//
// `std::proc_op(op, h, a, b, out, cap)` is the one primitive. A command
// arrives in `a` as `std` encodes it (`decode`); `b` is bytes for the
// child's standard input. No shell is involved: the program is found as
// the system finds programs (a name without a separator on `PATH`) and
// its arguments reach it exactly as given.
//
// A running child is an entry of a process-wide table, shared by every
// thread; its handle is its index. Each entry is locked on its own, and
// the table only long enough to find it, so one thread waiting for its
// child's output does not stop another starting a child or waiting for
// its own. Finished runs (`OUTPUT`) wait in a second table until `TAKE`
// copies them out.
//
// Failures are codes: -1 the program was not found, -2 permission was
// denied, -3 anything else.

use std::io::{Read, Write};
use std::sync::{Arc, Mutex};

pub const NOT_FOUND: i64 = -1;
pub const DENIED: i64 = -2;
pub const OTHER: i64 = -3;

pub const OUTPUT: u64 = 0;
pub const TAKE: u64 = 1;
pub const STATUS: u64 = 2;
pub const SPAWN: u64 = 3;
pub const CHILD_ID: u64 = 4;
pub const WRITE_INPUT: u64 = 5;
pub const CLOSE_INPUT: u64 = 6;
pub const READ_OUTPUT: u64 = 7;
pub const OUTPUT_LINE_LEN: u64 = 8;
pub const READ_ERROR: u64 = 9;
pub const WAIT: u64 = 10;
pub const TRY_WAIT: u64 = 11;
pub const KILL: u64 = 12;
pub const DROP: u64 = 13;
pub const WATCH_INTERRUPTS: u64 = 14;
pub const INTERRUPT_REQUESTED: u64 = 15;
pub const PROCESS_ID: u64 = 16; // D-0181
pub const TERMINATE: u64 = 17; // D-0181
pub const ERROR_LINE_LEN: u64 = 18; // D-0181
pub const CPU_COUNT: u64 = 19; // D-0185
// D-0195: a child's standard input or output taken out as a pipe of its
// own (`h` the child for the TAKE_*s, the pipe for the PIPE_*s).
pub const TAKE_INPUT: u64 = 20;
pub const TAKE_OUTPUT: u64 = 21;
pub const PIPE_WRITE: u64 = 22;
pub const PIPE_READ: u64 = 23;
pub const PIPE_LINE_LEN: u64 = 24;
pub const PIPE_DROP: u64 = 25;
pub const KILL_TREE: u64 = 26; // D-0205

fn code(e: &std::io::Error) -> i64 {
    match e.kind() {
        std::io::ErrorKind::NotFound => NOT_FOUND,
        std::io::ErrorKind::PermissionDenied => DENIED,
        _ => OTHER,
    }
}

// The command `std` sends: strings as an 8-byte little-endian length and
// the bytes. First the program and its arguments, as a count and the
// strings; then a byte, 1 to clear the environment; a byte, 1 when a
// directory follows, and the directory; then a count of variables, and
// each name and value.
struct Spec {
    argv: Vec<String>,
    clear_env: bool,
    dir: Option<String>,
    env: Vec<(String, String)>,
}

struct Reader<'a> {
    b: &'a [u8],
    at: usize,
}

impl Reader<'_> {
    fn n(&mut self) -> Result<u64, i64> {
        let s = self.b.get(self.at..self.at + 8).ok_or(OTHER)?;
        self.at += 8;
        Ok(u64::from_le_bytes(s.try_into().unwrap()))
    }
    fn byte(&mut self) -> Result<u8, i64> {
        let x = *self.b.get(self.at).ok_or(OTHER)?;
        self.at += 1;
        Ok(x)
    }
    fn text(&mut self) -> Result<String, i64> {
        let n = self.n()? as usize;
        let s = self.b.get(self.at..self.at + n).ok_or(OTHER)?;
        self.at += n;
        String::from_utf8(s.to_vec()).map_err(|_| OTHER)
    }
}

fn decode(a: &[u8]) -> Result<Spec, i64> {
    let mut r = Reader { b: a, at: 0 };
    let n = r.n()?;
    let mut argv = Vec::new();
    for _ in 0..n {
        argv.push(r.text()?);
    }
    let clear_env = r.byte()? == 1;
    let dir = if r.byte()? == 1 { Some(r.text()?) } else { None };
    let k = r.n()?;
    let mut env = Vec::new();
    for _ in 0..k {
        let name = r.text()?;
        let value = r.text()?;
        env.push((name, value));
    }
    if argv.is_empty() {
        return Err(OTHER);
    }
    Ok(Spec { argv, clear_env, dir, env })
}

fn command(s: &Spec) -> std::process::Command {
    // D-0201: a child sees every byte this program wrote to its files.
    crate::fileio::flush_all();
    let mut c = std::process::Command::new(&s.argv[0]);
    c.args(&s.argv[1..]);
    if s.clear_env {
        c.env_clear();
    }
    for (k, v) in &s.env {
        c.env(k, v);
    }
    if let Some(d) = &s.dir {
        c.current_dir(d);
    }
    c
}

// The exit code, or minus the signal that ended the child (`-9` for
// SIGKILL) on Unix.
fn status_of(st: std::process::ExitStatus) -> i32 {
    if let Some(c) = st.code() {
        return c;
    }
    #[cfg(unix)]
    {
        use std::os::unix::process::ExitStatusExt;
        if let Some(s) = st.signal() {
            return -s;
        }
    }
    -1
}

// A spawn's failure: a program that is not there is NotFound, whether or
// not a working directory given for it exists (the system says NotFound
// for both; a missing directory is Io here, checked first).
fn spawn_error(s: &Spec, e: &std::io::Error) -> i64 {
    if let Some(d) = &s.dir {
        if !std::path::Path::new(d).is_dir() {
            return OTHER;
        }
    }
    code(e)
}

// ---- runs to the end (`Command::output`, `Command::status`) ----

static DONE: Mutex<Vec<Option<Vec<u8>>>> = Mutex::new(Vec::new());

fn keep(v: Vec<u8>) -> i64 {
    let mut t = DONE.lock().unwrap_or_else(|e| e.into_inner());
    match t.iter().position(|s| s.is_none()) {
        Some(i) => {
            t[i] = Some(v);
            i as i64
        }
        None => {
            t.push(Some(v));
            (t.len() - 1) as i64
        }
    }
}

// The record `TAKE` hands over: the status (4 bytes), the standard
// output's length (8 bytes), its bytes, then the standard error's bytes.
fn run_output(s: &Spec, input: &[u8]) -> Result<Vec<u8>, i64> {
    let mut c = command(s);
    c.stdout(std::process::Stdio::piped()).stderr(std::process::Stdio::piped());
    let out = if input.is_empty() {
        c.stdin(std::process::Stdio::null());
        c.output().map_err(|e| spawn_error(s, &e))?
    } else {
        c.stdin(std::process::Stdio::piped());
        let mut child = c.spawn().map_err(|e| spawn_error(s, &e))?;
        let mut stdin = child.stdin.take().unwrap();
        // Written from its own thread, while this one drains both output
        // pipes, so a child that answers before reading all of its input
        // cannot deadlock against us. A child that stops reading early
        // breaks the pipe; that is not this run's failure.
        let data = input.to_vec();
        let writer = std::thread::spawn(move || {
            let _ = stdin.write_all(&data);
        });
        let out = child.wait_with_output().map_err(|e| code(&e))?;
        let _ = writer.join();
        out
    };
    let mut v = Vec::with_capacity(12 + out.stdout.len() + out.stderr.len());
    v.extend_from_slice(&status_of(out.status).to_le_bytes());
    v.extend_from_slice(&(out.stdout.len() as u64).to_le_bytes());
    v.extend_from_slice(&out.stdout);
    v.extend_from_slice(&out.stderr);
    Ok(v)
}

// ---- children with pipes (`Command::spawn`, `Child`) ----

// What is read from a pipe at a time.
const CHUNK: usize = 8192;

struct Entry {
    child: std::process::Child,
    stdin: Option<std::process::ChildStdin>,
    stdout: Option<std::process::ChildStdout>,
    stderr: Option<std::process::ChildStderr>,
    // Standard output read ahead for `read_output_line`, and how much of
    // it is taken; the same for standard error (`read_error_line`).
    ahead: Vec<u8>,
    at: usize,
    ahead_err: Vec<u8>,
    at_err: usize,
    status: Option<i32>,
}

type Slot = Arc<Mutex<Entry>>;

static TABLE: Mutex<Vec<Option<Slot>>> = Mutex::new(Vec::new());

fn slot(h: u64) -> Result<Slot, i64> {
    let t = TABLE.lock().unwrap_or_else(|e| e.into_inner());
    t.get(h as usize).and_then(|s| s.clone()).ok_or(OTHER)
}

fn with<T>(h: u64, f: impl FnOnce(&mut Entry) -> Result<T, i64>) -> Result<T, i64> {
    let s = slot(h)?;
    let mut e = s.lock().unwrap_or_else(|e| e.into_inner());
    f(&mut e)
}

fn spawn(s: &Spec) -> Result<i64, i64> {
    use std::process::Stdio;
    let mut child = command(s).stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::piped()).spawn().map_err(|e| spawn_error(s, &e))?;
    let e = Entry { stdin: child.stdin.take(), stdout: child.stdout.take(), stderr: child.stderr.take(), child, ahead: Vec::new(), at: 0, ahead_err: Vec::new(), at_err: 0, status: None };
    let mut t = TABLE.lock().unwrap_or_else(|e| e.into_inner());
    let slot = Some(Arc::new(Mutex::new(e)));
    match t.iter().position(|s| s.is_none()) {
        Some(i) => {
            t[i] = slot;
            Ok(i as i64)
        }
        None => {
            t.push(slot);
            Ok((t.len() - 1) as i64)
        }
    }
}

// ---- pipes taken out of a child (D-0195) ----

// One end, owned by a `ChildInput` or `ChildOutput`: each has its own lock,
// so a thread writing one child's input and another reading a second
// child's output never wait for each other, nor for the `Child`s.
enum Pipe {
    In(std::process::ChildStdin),
    Out { pipe: std::process::ChildStdout, ahead: Vec<u8>, at: usize },
}

static PIPES: Mutex<Vec<Option<Arc<Mutex<Pipe>>>>> = Mutex::new(Vec::new());

fn add_pipe(p: Pipe) -> i64 {
    let mut t = PIPES.lock().unwrap_or_else(|e| e.into_inner());
    let slot = Some(Arc::new(Mutex::new(p)));
    match t.iter().position(|s| s.is_none()) {
        Some(i) => {
            t[i] = slot;
            i as i64
        }
        None => {
            t.push(slot);
            (t.len() - 1) as i64
        }
    }
}

fn with_pipe<T>(h: u64, f: impl FnOnce(&mut Pipe) -> Result<T, i64>) -> Result<T, i64> {
    let s = {
        let t = PIPES.lock().unwrap_or_else(|e| e.into_inner());
        t.get(h as usize).and_then(|s| s.clone()).ok_or(OTHER)?
    };
    let mut p = s.lock().unwrap_or_else(|e| e.into_inner());
    f(&mut p)
}

fn read_pipe(p: &mut impl Read, buf: &mut [u8]) -> Result<usize, i64> {
    loop {
        match p.read(buf) {
            Err(x) if x.kind() == std::io::ErrorKind::Interrupted => continue,
            Ok(n) => return Ok(n),
            Err(x) => return Err(code(&x)),
        }
    }
}

// Up to `cap` bytes of a pipe: what was read ahead (`ahead`, taken to
// `at`) first, then one read of the pipe; none at its end.
fn read_ahead_pipe(pipe: Option<&mut impl Read>, ahead: &mut Vec<u8>, at: &mut usize, cap: usize) -> Result<Vec<u8>, i64> {
    if cap == 0 {
        return Ok(Vec::new());
    }
    if *at < ahead.len() {
        let k = (ahead.len() - *at).min(cap);
        let v = ahead[*at..*at + k].to_vec();
        *at += k;
        return Ok(v);
    }
    let Some(p) = pipe else { return Ok(Vec::new()) };
    let mut v = vec![0u8; cap];
    let n = read_pipe(p, &mut v)?;
    v.truncate(n);
    Ok(v)
}

// The length of a pipe's next line, its '\n' included (the rest when
// there is none); 0 at the end. Its bytes are then read ahead, and
// `read_ahead_pipe` takes them.
fn pipe_line_len(pipe: Option<&mut impl Read>, ahead: &mut Vec<u8>, at: &mut usize) -> Result<u64, i64> {
    let mut from = *at;
    let mut pipe = pipe;
    loop {
        if let Some(i) = ahead[from..].iter().position(|b| *b == b'\n') {
            return Ok((from + i + 1 - *at) as u64);
        }
        if *at > 0 {
            ahead.drain(..*at);
            *at = 0;
        }
        let seen = ahead.len();
        let Some(p) = pipe.as_mut() else { return Ok(seen as u64) };
        let old = ahead.len();
        ahead.resize(old + CHUNK, 0);
        let n = read_pipe(*p, &mut ahead[old..]);
        let got = *n.as_ref().unwrap_or(&0);
        ahead.truncate(old + got);
        n?;
        if got == 0 {
            return Ok(ahead.len() as u64);
        }
        from = seen;
    }
}

// D-0205: `Child::kill_tree`. Linux: the child's descendants, found from
// /proc (each process's parent), are killed after the child -- which is
// killed first so it starts no more -- one level at a time. Windows:
// `taskkill /T /F`, which ends the tree. Elsewhere: the child alone.
#[cfg(target_os = "linux")]
fn kill_tree(child: &mut std::process::Child) -> std::io::Result<()> {
    fn parents() -> Vec<(u32, u32)> {
        let mut v = Vec::new();
        if let Ok(d) = std::fs::read_dir("/proc") {
            for e in d.flatten() {
                let Some(pid) = e.file_name().to_str().and_then(|n| n.parse::<u32>().ok()) else { continue };
                if let Ok(st) = std::fs::read_to_string(format!("/proc/{}/stat", pid)) {
                    // "pid (comm) state ppid ...": comm may hold spaces and
                    // parentheses, so read after the last ')'.
                    if let Some(rest) = st.rsplit_once(')').map(|(_, r)| r) {
                        if let Some(ppid) = rest.split_whitespace().nth(1).and_then(|x| x.parse::<u32>().ok()) {
                            v.push((pid, ppid));
                        }
                    }
                }
            }
        }
        v
    }
    extern "C" {
        fn kill(pid: i32, sig: i32) -> i32;
    }
    let root = child.id();
    let table = parents();
    let mut level = vec![root];
    let mut all = Vec::new();
    while !level.is_empty() {
        let next: Vec<u32> = table.iter().filter(|(_, pp)| level.contains(pp)).map(|(p, _)| *p).collect();
        all.extend(next.iter().copied());
        level = next;
    }
    child.kill()?;
    for p in all {
        unsafe { kill(p as i32, 9) };
    }
    Ok(())
}

#[cfg(windows)]
fn kill_tree(child: &mut std::process::Child) -> std::io::Result<()> {
    let st = std::process::Command::new("taskkill").args(["/T", "/F", "/PID", &child.id().to_string()]).stdout(std::process::Stdio::null()).stderr(std::process::Stdio::null()).status();
    match st {
        Ok(s) if s.success() => Ok(()),
        _ => child.kill(),
    }
}

#[cfg(not(any(target_os = "linux", windows)))]
fn kill_tree(child: &mut std::process::Child) -> std::io::Result<()> {
    child.kill()
}

fn read_output(e: &mut Entry, cap: usize) -> Result<Vec<u8>, i64> {
    read_ahead_pipe(e.stdout.as_mut(), &mut e.ahead, &mut e.at, cap)
}

fn output_line_len(e: &mut Entry) -> Result<u64, i64> {
    pipe_line_len(e.stdout.as_mut(), &mut e.ahead, &mut e.at)
}

// D-0181: standard error read the same way, so `read_error_line` exists.
fn read_error(e: &mut Entry, cap: usize) -> Result<Vec<u8>, i64> {
    read_ahead_pipe(e.stderr.as_mut(), &mut e.ahead_err, &mut e.at_err, cap)
}

fn error_line_len(e: &mut Entry) -> Result<u64, i64> {
    pipe_line_len(e.stderr.as_mut(), &mut e.ahead_err, &mut e.at_err)
}

// D-0181: SIGTERM to a child on Unix, so it may end cleanly; on other
// systems, as `kill`.
fn terminate(e: &mut Entry) -> Result<(), i64> {
    if e.status.is_some() {
        return Ok(());
    }
    #[cfg(unix)]
    {
        extern "C" {
            fn kill(pid: i32, sig: i32) -> i32;
        }
        const SIGTERM: i32 = 15;
        // SAFETY: a signal to a process this program started and has not
        // yet reaped (its id is still the child's).
        let r = unsafe { kill(e.child.id() as i32, SIGTERM) };
        return if r == 0 { Ok(()) } else { Err(OTHER) };
    }
    #[cfg(not(unix))]
    {
        e.child.kill().map_err(|x| code(&x))
    }
}

fn wait(e: &mut Entry) -> Result<i32, i64> {
    if let Some(s) = e.status {
        return Ok(s);
    }
    // Standard input closed first, so a child reading to its end ends.
    e.stdin = None;
    let st = loop {
        match e.child.wait() {
            Err(x) if x.kind() == std::io::ErrorKind::Interrupted => continue,
            other => break other.map_err(|x| code(&x))?,
        }
    };
    let s = status_of(st);
    e.status = Some(s);
    Ok(s)
}

fn try_wait(e: &mut Entry) -> Result<Option<i32>, i64> {
    if let Some(s) = e.status {
        return Ok(Some(s));
    }
    match e.child.try_wait().map_err(|x| code(&x))? {
        Some(st) => {
            let s = status_of(st);
            e.status = Some(s);
            Ok(Some(s))
        }
        None => Ok(None),
    }
}

// The child's pipes closed and its entry gone; a finished child is
// reaped. A running one keeps running, as Rust's `Child` does when
// dropped.
fn drop_child(h: u64) -> Result<(), i64> {
    let s = {
        let mut t = TABLE.lock().unwrap_or_else(|e| e.into_inner());
        match t.get_mut(h as usize).and_then(|e| e.take()) {
            Some(s) => s,
            None => return Err(OTHER),
        }
    };
    let mut e = s.lock().unwrap_or_else(|e| e.into_inner());
    e.stdin = None;
    e.stdout = None;
    e.stderr = None;
    let _ = e.child.try_wait();
    Ok(())
}

// ---- interrupt requests ----

static INTERRUPTED: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

#[cfg(unix)]
fn watch() {
    extern "C" fn on_signal(_sig: i32) {
        INTERRUPTED.store(true, std::sync::atomic::Ordering::SeqCst);
    }
    extern "C" {
        fn siginterrupt(sig: i32, flag: i32) -> i32;
    }
    const SIGINT: i32 = 2;
    const SIGTERM: i32 = 15;
    // SAFETY: the handler only stores to an atomic, which is
    // async-signal-safe. `siginterrupt` makes a blocking call the signal
    // arrives in return EINTR rather than resume, so a loop can see the
    // flag.
    unsafe {
        let handler = on_signal as extern "C" fn(i32) as usize;
        signal(SIGINT, handler);
        signal(SIGTERM, handler);
        siginterrupt(SIGINT, 1);
        siginterrupt(SIGTERM, 1);
    }
}

// `signal(2)`, the handler as a number so that `SIG_IGN` can be passed.
#[cfg(unix)]
extern "C" {
    fn signal(sig: i32, handler: usize) -> usize;
}

// A write to a pipe whose reader is gone raises SIGPIPE on Unix, which
// ends a C program at once. Rust's start-up ignores the signal, so under
// `coby` such a write fails and `std` reports it (`Err(Io)`); the
// compiled program's runtime calls this at start-up so that it does the
// same (D-0143).
#[cfg(unix)]
pub fn ignore_sigpipe() {
    const SIGPIPE: i32 = 13;
    const SIG_IGN: usize = 1;
    // SAFETY: a disposition for one signal, set before any other thread
    // exists.
    unsafe {
        signal(SIGPIPE, SIG_IGN);
    }
}

#[cfg(not(unix))]
pub fn ignore_sigpipe() {}

#[cfg(windows)]
fn watch() {
    extern "system" fn on_ctrl(_kind: u32) -> i32 {
        INTERRUPTED.store(true, std::sync::atomic::Ordering::SeqCst);
        1
    }
    #[link(name = "kernel32")]
    extern "system" {
        fn SetConsoleCtrlHandler(handler: Option<extern "system" fn(u32) -> i32>, add: i32) -> i32;
    }
    // SAFETY: the handler only stores to an atomic.
    unsafe {
        SetConsoleCtrlHandler(Some(on_ctrl), 1);
    }
}

#[cfg(not(any(unix, windows)))]
fn watch() {}

pub fn interrupt_requested() -> bool {
    INTERRUPTED.load(std::sync::atomic::Ordering::SeqCst)
}

// ---- the primitive ----

// A status as an answer: its 4 bytes, little-endian, and their count.
fn status_bytes(s: i32) -> (i64, Vec<u8>) {
    (4, s.to_le_bytes().to_vec())
}

pub fn call(op: u64, h: u64, a: &[u8], b: &[u8], cap: u64) -> (i64, Vec<u8>) {
    let r: Result<(i64, Vec<u8>), i64> = (|| match op {
        OUTPUT => {
            let s = decode(a)?;
            Ok((keep(run_output(&s, b)?), Vec::new()))
        }
        TAKE => {
            // The kept record's length; its bytes when they fit in `cap`,
            // and then it is gone.
            let mut t = DONE.lock().unwrap_or_else(|e| e.into_inner());
            let v = t.get_mut(h as usize).ok_or(OTHER)?;
            let n = v.as_ref().ok_or(OTHER)?.len();
            if n as u64 <= cap {
                Ok((n as i64, v.take().unwrap()))
            } else {
                Ok((n as i64, Vec::new()))
            }
        }
        STATUS => {
            let s = decode(a)?;
            // The child writes to this program's standard output, after
            // what the program has written so far (D-0178).
            let _ = crate::outbuf::flush();
            let st = command(&s).status().map_err(|e| spawn_error(&s, &e))?;
            Ok(status_bytes(status_of(st)))
        }
        SPAWN => Ok((spawn(&decode(a)?)?, Vec::new())),
        CHILD_ID => with(h, |e| Ok((e.child.id() as i64, Vec::new()))),
        WRITE_INPUT => with(h, |e| match e.stdin.as_mut() {
            Some(p) => p.write_all(b).and_then(|_| p.flush()).map(|_| (0, Vec::new())).map_err(|x| code(&x)),
            None => Err(OTHER),
        }),
        CLOSE_INPUT => with(h, |e| {
            e.stdin = None;
            Ok((0, Vec::new()))
        }),
        READ_OUTPUT => with(h, |e| read_output(e, cap as usize).map(|v| (v.len() as i64, v))),
        OUTPUT_LINE_LEN => with(h, |e| output_line_len(e).map(|n| (n as i64, Vec::new()))),
        READ_ERROR => with(h, |e| read_error(e, cap as usize).map(|v| (v.len() as i64, v))),
        WAIT => with(h, |e| wait(e).map(status_bytes)),
        TRY_WAIT => with(h, |e| {
            Ok(match try_wait(e)? {
                Some(s) => (1, s.to_le_bytes().to_vec()),
                None => (0, Vec::new()),
            })
        }),
        KILL => with(h, |e| {
            if e.status.is_some() {
                return Ok((0, Vec::new()));
            }
            e.child.kill().map(|_| (0, Vec::new())).map_err(|x| code(&x))
        }),
        // D-0205: the child and every process it started, and they in turn.
        KILL_TREE => with(h, |e| {
            if e.status.is_some() {
                return Ok((0, Vec::new()));
            }
            kill_tree(&mut e.child).map(|_| (0, Vec::new())).map_err(|x| code(&x))
        }),
        DROP => drop_child(h).map(|_| (0, Vec::new())),
        WATCH_INTERRUPTS => {
            watch();
            crate::netio::watching();
            Ok((0, Vec::new()))
        }
        INTERRUPT_REQUESTED => Ok((interrupt_requested() as i64, Vec::new())),
        PROCESS_ID => Ok((std::process::id() as i64, Vec::new())),
        TERMINATE => with(h, |e| terminate(e).map(|_| (0, Vec::new()))),
        ERROR_LINE_LEN => with(h, |e| error_line_len(e).map(|n| (n as i64, Vec::new()))),
        CPU_COUNT => Ok((std::thread::available_parallelism().map(|n| n.get() as i64).unwrap_or(1), Vec::new())),
        // The pipe's number, or NOT_FOUND when it was taken or closed.
        TAKE_INPUT => {
            let p = with(h, |e| Ok(e.stdin.take()))?;
            match p {
                Some(p) => Ok((add_pipe(Pipe::In(p)), Vec::new())),
                None => Err(NOT_FOUND),
            }
        }
        TAKE_OUTPUT => {
            let p = with(h, |e| Ok(e.stdout.take().map(|p| (p, std::mem::take(&mut e.ahead), std::mem::replace(&mut e.at, 0)))))?;
            match p {
                Some((pipe, ahead, at)) => Ok((add_pipe(Pipe::Out { pipe, ahead, at }), Vec::new())),
                None => Err(NOT_FOUND),
            }
        }
        PIPE_WRITE => with_pipe(h, |p| match p {
            Pipe::In(w) => w.write_all(b).and_then(|_| w.flush()).map(|_| (0, Vec::new())).map_err(|x| code(&x)),
            Pipe::Out { .. } => Err(OTHER),
        }),
        PIPE_READ => with_pipe(h, |p| match p {
            Pipe::Out { pipe, ahead, at } => read_ahead_pipe(Some(pipe), ahead, at, cap as usize).map(|v| (v.len() as i64, v)),
            Pipe::In(_) => Err(OTHER),
        }),
        PIPE_LINE_LEN => with_pipe(h, |p| match p {
            Pipe::Out { pipe, ahead, at } => pipe_line_len(Some(pipe), ahead, at).map(|n| (n as i64, Vec::new())),
            Pipe::In(_) => Err(OTHER),
        }),
        PIPE_DROP => {
            let mut t = PIPES.lock().unwrap_or_else(|e| e.into_inner());
            if let Some(s) = t.get_mut(h as usize) {
                *s = None;
            }
            Ok((0, Vec::new()))
        }
        _ => Err(OTHER),
    })();
    match r {
        Ok(x) => x,
        Err(c) => (c, Vec::new()),
    }
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;

    fn enc(argv: &[&str]) -> Vec<u8> {
        let mut v = Vec::new();
        v.extend_from_slice(&(argv.len() as u64).to_le_bytes());
        for a in argv {
            v.extend_from_slice(&(a.len() as u64).to_le_bytes());
            v.extend_from_slice(a.as_bytes());
        }
        v.push(0);
        v.push(0);
        v.extend_from_slice(&0u64.to_le_bytes());
        v
    }

    #[test]
    fn output_status_and_children() {
        let (h, _) = call(OUTPUT, 0, &enc(&["/bin/sh", "-c", "echo out; echo err >&2; exit 3"]), b"", 0);
        assert!(h >= 0);
        let (n, v) = call(TAKE, h as u64, b"", b"", 1 << 20);
        assert_eq!(n as usize, v.len());
        assert_eq!(&v[..4], &3i32.to_le_bytes());
        assert_eq!(&v[12..16], b"out\n");
        assert_eq!(&v[16..], b"err\n");
        let (h, _) = call(OUTPUT, 0, &enc(&["/bin/cat"]), b"fed", 0);
        let (_, v) = call(TAKE, h as u64, b"", b"", 1 << 20);
        assert_eq!(&v[12..], b"fed");
        assert_eq!(call(OUTPUT, 0, &enc(&["/surely/not/a/program"]), b"", 0).0, NOT_FOUND);
        let (_, v) = call(STATUS, 0, &enc(&["/bin/sh", "-c", "kill -9 $$"]), b"", 0);
        assert_eq!(v, (-9i32).to_le_bytes());
        let (c, _) = call(SPAWN, 0, &enc(&["/bin/cat"]), b"", 0);
        assert!(c >= 0);
        let c = c as u64;
        assert_eq!(call(WRITE_INPUT, c, b"", b"one\ntwo", 0).0, 0);
        call(CLOSE_INPUT, c, b"", b"", 0);
        assert_eq!(call(OUTPUT_LINE_LEN, c, b"", b"", 0).0, 4);
        assert_eq!(call(READ_OUTPUT, c, b"", b"", 4).1, b"one\n");
        assert_eq!(call(OUTPUT_LINE_LEN, c, b"", b"", 0).0, 3);
        assert_eq!(call(READ_OUTPUT, c, b"", b"", 100).1, b"two");
        assert_eq!(call(OUTPUT_LINE_LEN, c, b"", b"", 0).0, 0);
        assert_eq!(call(WAIT, c, b"", b"", 0).1, 0i32.to_le_bytes());
        assert_eq!(call(DROP, c, b"", b"", 0).0, 0);
    }
}
