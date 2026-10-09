// Sockets: TCP, UDP and name resolution (`spec/21` §2l
// `rule.stdlib.net`, D-0144). One source file for both implementations:
// the interpreter uses it as a module and the compiler's runtime (`cbrt`)
// includes this same file, so `coby` and `cobc` connect, read, write and
// fail alike. Rust's `std::net` underneath, so Windows needs nothing
// more than it links already (`ws2_32`).
//
// `std::net_op(op, h, a, b, out, cap)` is the one primitive. `h` is a
// socket's handle: its index in a process-wide table, shared by every
// thread. Each entry is locked on its own, and the table only long enough
// to find one, so a thread blocked in `accept` or `read` holds no other
// thread up. An address travels as a 19-byte record (`addr_bytes`). For
// an operation that answers no bytes, `cap` is its number (a timeout in
// milliseconds, a flag).
//
// Sockets block, each with its own timeouts. A timeout is `TIMED_OUT`,
// and the socket stays usable. Under `watch_interrupts` (D-0143), an
// interrupt that arrives while a call blocks makes it return `TIMED_OUT`
// too, so a loop sees the flag. D-0204: a wait is made in slices
// (`wait_ready`), so a thread asked to stop (`cancel`) leaves it with
// `CANCELLED`.

use std::io::{Read, Write};
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr, TcpListener, TcpStream, UdpSocket};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

pub const NOT_FOUND: i64 = -1;
pub const DENIED: i64 = -2;
pub const OTHER: i64 = -3;
pub const REFUSED: i64 = -4;
pub const RESET: i64 = -5;
pub const TIMED_OUT: i64 = -6;
pub const UNREACHABLE: i64 = -7;
pub const ADDR_IN_USE: i64 = -8;
pub const ADDR_NOT_AVAILABLE: i64 = -9;
pub const CLOSED: i64 = -10;
// D-0204: the waiting thread was asked to stop (`cancel`); each tool
// raises `diag.thread-cancelled`.
pub const CANCELLED: i64 = -11;

pub const RESOLVE: u64 = 0;
pub const TCP_BIND: u64 = 1;
pub const LOCAL_ADDR: u64 = 2;
pub const PEER_ADDR: u64 = 3;
pub const SET_ACCEPT_TIMEOUT: u64 = 4;
pub const ACCEPT: u64 = 5;
pub const CONNECT: u64 = 6;
pub const SET_READ_TIMEOUT: u64 = 7;
pub const SET_WRITE_TIMEOUT: u64 = 8;
pub const SET_NODELAY: u64 = 9;
pub const READ: u64 = 10;
pub const LINE_LEN: u64 = 11;
pub const WRITE: u64 = 12;
pub const SHUTDOWN_WRITE: u64 = 13;
pub const CLOSE: u64 = 14;
pub const UDP_BIND: u64 = 15;
pub const SEND_TO: u64 = 16;
pub const RECV_FROM: u64 = 17;
pub const PARSE_IP: u64 = 18;
pub const ADDR_TEXT: u64 = 19;
pub const SET_KEEPALIVE: u64 = 20; // D-0181
pub const UDP_CONNECT: u64 = 21; // D-0181
pub const UDP_SEND: u64 = 22; // D-0181
pub const UDP_RECV: u64 = 23; // D-0181
pub const TRY_CLONE: u64 = 24; // D-0185

fn code(e: &std::io::Error) -> i64 {
    use std::io::ErrorKind as K;
    match e.kind() {
        K::NotFound => NOT_FOUND,
        K::PermissionDenied => DENIED,
        K::ConnectionRefused => REFUSED,
        K::ConnectionReset | K::ConnectionAborted => RESET,
        K::TimedOut | K::WouldBlock => TIMED_OUT,
        K::AddrInUse => ADDR_IN_USE,
        K::AddrNotAvailable => ADDR_NOT_AVAILABLE,
        K::BrokenPipe | K::NotConnected | K::UnexpectedEof => CLOSED,
        K::Interrupted => TIMED_OUT,
        _ if unreachable(e) => UNREACHABLE,
        _ => OTHER,
    }
}

// A network that is down or unreachable, or a host that is unreachable,
// by the system's own error number. `ErrorKind::NetworkDown`,
// `NetworkUnreachable` and `HostUnreachable` say the same, but are
// stable only since Rust 1.83, and Rust 1.77.2 is the last release that
// runs on Windows 7, where CobaltC is also built.
fn unreachable(e: &std::io::Error) -> bool {
    #[cfg(windows)]
    const CODES: [i32; 3] = [10050, 10051, 10065]; // WSAENETDOWN, WSAENETUNREACH, WSAEHOSTUNREACH
    #[cfg(any(target_os = "linux", target_os = "android"))]
    const CODES: [i32; 3] = [100, 101, 113]; // ENETDOWN, ENETUNREACH, EHOSTUNREACH
    #[cfg(all(unix, not(any(target_os = "linux", target_os = "android"))))]
    const CODES: [i32; 3] = [50, 51, 65]; // ENETDOWN, ENETUNREACH, EHOSTUNREACH (macOS, the BSDs)
    #[cfg(not(any(unix, windows)))]
    const CODES: [i32; 0] = [];
    e.raw_os_error().map_or(false, |n| CODES.contains(&n))
}

// Whether a call interrupted by a signal should give up: only when it was
// an interrupt the program watches for; otherwise it is resumed.
fn give_up(e: &std::io::Error) -> bool {
    e.kind() != std::io::ErrorKind::Interrupted || crate::procio::interrupt_requested()
}

// The record of an address: the family (4 or 6), 16 bytes of address
// (a v4 address in the first 4), the port (2 bytes, little-endian).
pub const ADDR_LEN: usize = 19;

pub fn addr_bytes(a: &SocketAddr) -> Vec<u8> {
    let mut v = vec![0u8; ADDR_LEN];
    match a.ip() {
        IpAddr::V4(ip) => {
            v[0] = 4;
            v[1..5].copy_from_slice(&ip.octets());
        }
        IpAddr::V6(ip) => {
            v[0] = 6;
            v[1..17].copy_from_slice(&ip.octets());
        }
    }
    v[17..19].copy_from_slice(&a.port().to_le_bytes());
    v
}

fn addr_of(b: &[u8]) -> Result<SocketAddr, i64> {
    if b.len() < ADDR_LEN {
        return Err(OTHER);
    }
    let port = u16::from_le_bytes([b[17], b[18]]);
    let ip = match b[0] {
        4 => IpAddr::V4(Ipv4Addr::new(b[1], b[2], b[3], b[4])),
        6 => IpAddr::V6(Ipv6Addr::from(<[u8; 16]>::try_from(&b[1..17]).unwrap())),
        _ => return Err(OTHER),
    };
    Ok(SocketAddr::new(ip, port))
}

// What is read from a stream at a time, for `read_line`.
const CHUNK: usize = 8192;

struct Stream {
    s: TcpStream,
    ahead: Vec<u8>,
    at: usize,
}

enum Sock {
    Listener(TcpListener, u64),
    Stream(Stream),
    Udp(UdpSocket),
}

type Slot = Arc<Mutex<Sock>>;

static TABLE: Mutex<Vec<Option<Slot>>> = Mutex::new(Vec::new());

fn add(s: Sock) -> i64 {
    let mut t = TABLE.lock().unwrap_or_else(|e| e.into_inner());
    let slot = Some(Arc::new(Mutex::new(s)));
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

fn with<T>(h: u64, f: impl FnOnce(&mut Sock) -> Result<T, i64>) -> Result<T, i64> {
    let s = {
        let t = TABLE.lock().unwrap_or_else(|e| e.into_inner());
        t.get(h as usize).and_then(|s| s.clone()).ok_or(OTHER)?
    };
    let mut e = s.lock().unwrap_or_else(|e| e.into_inner());
    f(&mut e)
}

fn with_stream<T>(h: u64, f: impl FnOnce(&mut Stream) -> Result<T, i64>) -> Result<T, i64> {
    with(h, |s| match s {
        Sock::Stream(st) => f(st),
        _ => Err(OTHER),
    })
}

fn timeout(ms: u64) -> Option<Duration> {
    if ms == 0 {
        None
    } else {
        Some(Duration::from_millis(ms))
    }
}

// D-0204: whether the calling thread has been asked to stop, as the tool
// running the program knows (`set_cancel_probe`).
static CANCEL_PROBE: std::sync::OnceLock<fn() -> bool> = std::sync::OnceLock::new();

pub fn set_cancel_probe(f: fn() -> bool) {
    let _ = CANCEL_PROBE.set(f);
}

fn cancelled() -> bool {
    CANCEL_PROBE.get().map_or(false, |f| f())
}

// Waits until `sock` can be read (or written, `!read`), the deadline
// passes (`TIMED_OUT`), an interrupt arrives while watched (`TIMED_OUT`),
// or the thread is asked to stop (`CANCELLED`). In slices, growing from
// 50 ms to a second: a thousand idle connections do not wake the program
// ten thousand times a second.
// D-0204: on Linux a socket wait blocks until the socket is ready, its
// deadline passes or a signal (`wake_thread`, sent by `cancel`) interrupts
// it -- no wake-ups to look for a cancellation, which with ten thousand
// idle connections were most of a server's system time. Elsewhere it waits
// in slices, asking between them.
pub fn wake_init() {
    sys::wake_init();
}

// This OS thread, for `wake_thread`.
pub fn os_thread() -> u64 {
    sys::os_thread()
}

// Interrupts `t`'s socket wait, if it is in one (a pending wake ends its
// next). The caller makes sure `t` is still running.
pub fn wake_thread(t: u64) {
    sys::wake_thread(t);
}

fn wait_ready(sock: &impl sys::AsSock, read: bool, deadline: Option<Instant>) -> Result<(), i64> {
    let watched = INTERRUPTS_WATCHED.load(std::sync::atomic::Ordering::SeqCst);
    let wake = sys::wake_enabled();
    let mut slice = 50u64;
    loop {
        if cancelled() {
            return Err(CANCELLED);
        }
        if watched && crate::procio::interrupt_requested() {
            return Err(TIMED_OUT);
        }
        // Milliseconds to wait; none (`u64::MAX`) for no limit.
        let mut ms = if wake { u64::MAX } else { slice };
        if let Some(d) = deadline {
            let now = Instant::now();
            if now >= d {
                return Err(TIMED_OUT);
            }
            ms = ms.min((d - now).as_millis() as u64 + 1);
        }
        if watched {
            ms = ms.min(50);
        }
        let ms = if ms == u64::MAX { -1 } else { ms.min(i32::MAX as u64) as i32 };
        match sys::ready(sock.sock(), read, ms) {
            Ok(true) => return Ok(()),
            Ok(false) => {}
            Err(e) if e.kind() == std::io::ErrorKind::Interrupted => {}
            Err(e) => return Err(code(&e)),
        }
        slice = (slice * 2).min(1000);
    }
}

// `poll`, on one socket.
#[cfg(unix)]
mod sys {
    use std::os::unix::io::AsRawFd;

    pub trait AsSock {
        fn sock(&self) -> i32;
    }
    impl<T: AsRawFd> AsSock for T {
        fn sock(&self) -> i32 {
            self.as_raw_fd()
        }
    }

    #[repr(C)]
    struct PollFd {
        fd: i32,
        events: i16,
        revents: i16,
    }

    #[cfg(target_os = "linux")]
    type Nfds = std::os::raw::c_ulong;
    #[cfg(not(target_os = "linux"))]
    type Nfds = std::os::raw::c_uint;

    extern "C" {
        fn poll(fds: *mut PollFd, n: Nfds, timeout: i32) -> i32;
    }

    extern "C" {
        fn listen(fd: i32, backlog: i32) -> i32;
    }

    // D-0204: room for 4096 connections waiting to be accepted (the
    // standard library asks for 128): a burst of clients larger than that
    // had connections dropped, and retried a second later.
    pub fn backlog(l: &std::net::TcpListener) {
        unsafe { listen(l.as_raw_fd(), 4096) };
    }

    // Readable (or writable), an error or a hang-up included: the call
    // that follows reports those.
    pub fn ready(fd: i32, read: bool, ms: i32) -> std::io::Result<bool> {
        let mut p = PollFd { fd, events: if read { 1 } else { 4 }, revents: 0 };
        #[cfg(target_os = "linux")]
        if wake_enabled() {
            return wake::ready(&mut p, ms);
        }
        let n = unsafe { poll(&mut p, 1, ms) };
        if n < 0 {
            return Err(std::io::Error::last_os_error());
        }
        Ok(n > 0)
    }

    static WAKE: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

    pub fn wake_enabled() -> bool {
        WAKE.load(std::sync::atomic::Ordering::Relaxed)
    }

    #[cfg(target_os = "linux")]
    pub fn wake_init() {
        if wake::init() {
            WAKE.store(true, std::sync::atomic::Ordering::Relaxed);
        }
    }
    #[cfg(not(target_os = "linux"))]
    pub fn wake_init() {}

    #[cfg(target_os = "linux")]
    pub fn os_thread() -> u64 {
        wake::os_thread()
    }
    #[cfg(not(target_os = "linux"))]
    pub fn os_thread() -> u64 {
        0
    }

    pub fn wake_thread(t: u64) {
        #[cfg(target_os = "linux")]
        if wake_enabled() && t != 0 {
            wake::kill(t);
        }
        let _ = t;
    }

    // `SIGURG` (which nothing else here uses) is blocked in every thread
    // from the start -- threads inherit it -- and let through only inside
    // `ppoll`: one sent after the waiter last looked for a cancellation is
    // pending, and interrupts the `ppoll` as it begins.
    #[cfg(target_os = "linux")]
    mod wake {
        use super::{Nfds, PollFd};
        use std::os::raw::{c_int, c_long};

        #[repr(C)]
        #[derive(Clone, Copy)]
        struct SigSet([u64; 16]);

        #[repr(C)]
        struct Timespec {
            s: c_long,
            ns: c_long,
        }

        const SIGURG: c_int = 23;
        const SIG_BLOCK: c_int = 0;

        extern "C" {
            fn ppoll(fds: *mut PollFd, n: Nfds, ts: *const Timespec, mask: *const SigSet) -> c_int;
            fn sigemptyset(s: *mut SigSet) -> c_int;
            fn sigaddset(s: *mut SigSet, sig: c_int) -> c_int;
            fn sigdelset(s: *mut SigSet, sig: c_int) -> c_int;
            fn pthread_sigmask(how: c_int, set: *const SigSet, old: *mut SigSet) -> c_int;
            // As `procio` declares it (one declaration's signature).
            fn signal(sig: i32, handler: usize) -> usize;
            fn pthread_self() -> usize;
            fn pthread_kill(t: usize, sig: c_int) -> c_int;
        }

        extern "C" fn woken(_: c_int) {}

        pub fn init() -> bool {
            unsafe {
                let mut set = SigSet([0; 16]);
                sigemptyset(&mut set);
                sigaddset(&mut set, SIGURG);
                if pthread_sigmask(SIG_BLOCK, &set, std::ptr::null_mut()) != 0 {
                    return false;
                }
                signal(SIGURG, woken as extern "C" fn(c_int) as usize);
            }
            true
        }

        pub fn os_thread() -> u64 {
            unsafe { pthread_self() as u64 }
        }

        pub fn kill(t: u64) {
            unsafe { pthread_kill(t as usize, SIGURG) };
        }

        pub fn ready(p: &mut PollFd, ms: i32) -> std::io::Result<bool> {
            let ts = Timespec { s: (ms.max(0) / 1000) as c_long, ns: ((ms.max(0) % 1000) as c_long) * 1_000_000 };
            let tp: *const Timespec = if ms < 0 { std::ptr::null() } else { &ts };
            let mut mask = SigSet([0; 16]);
            let n = unsafe {
                pthread_sigmask(SIG_BLOCK, std::ptr::null(), &mut mask);
                sigdelset(&mut mask, SIGURG);
                ppoll(p, 1, tp, &mask)
            };
            if n < 0 {
                return Err(std::io::Error::last_os_error());
            }
            Ok(n > 0)
        }
    }
}

#[cfg(windows)]
mod sys {
    use std::os::windows::io::AsRawSocket;

    pub trait AsSock {
        fn sock(&self) -> u64;
    }
    impl<T: AsRawSocket> AsSock for T {
        fn sock(&self) -> u64 {
            self.as_raw_socket()
        }
    }

    #[repr(C)]
    struct WsaPollFd {
        fd: usize,
        events: i16,
        revents: i16,
    }

    #[link(name = "ws2_32")]
    extern "system" {
        fn WSAPoll(fds: *mut WsaPollFd, n: u32, timeout: i32) -> i32;
    }

    pub fn backlog(_l: &std::net::TcpListener) {}

    pub fn wake_enabled() -> bool {
        false
    }
    pub fn wake_init() {}
    pub fn os_thread() -> u64 {
        0
    }
    pub fn wake_thread(_t: u64) {}

    pub fn ready(fd: u64, read: bool, ms: i32) -> std::io::Result<bool> {
        // POLLRDNORM, POLLWRNORM
        let mut p = WsaPollFd { fd: fd as usize, events: if read { 0x100 } else { 0x10 }, revents: 0 };
        let n = unsafe { WSAPoll(&mut p, 1, ms) };
        if n < 0 {
            return Err(std::io::Error::last_os_error());
        }
        Ok(n > 0)
    }
}

// A socket's own timeout, as a deadline from now.
fn deadline_of(t: std::io::Result<Option<Duration>>) -> Option<Instant> {
    t.ok().flatten().map(|d| Instant::now() + d)
}

// `accept`, waiting at most `ms` (0: for ever), in `wait_ready`'s slices.
fn accept(l: &TcpListener, ms: u64) -> Result<TcpStream, i64> {
    l.set_nonblocking(true).map_err(|e| code(&e))?;
    let deadline = timeout(ms).map(|d| Instant::now() + d);
    loop {
        match l.accept() {
            Ok((s, _)) => {
                // Some systems hand on the listener's mode.
                s.set_nonblocking(false).map_err(|e| code(&e))?;
                return Ok(s);
            }
            Err(e) if e.kind() == std::io::ErrorKind::WouldBlock || e.kind() == std::io::ErrorKind::Interrupted => {
                wait_ready(l, true, deadline)?;
            }
            Err(e) => return Err(code(&e)),
        }
    }
}

// Set by `watch_interrupts` (src/procio.rs) through `watching`.
static INTERRUPTS_WATCHED: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

pub fn watching() {
    INTERRUPTS_WATCHED.store(true, std::sync::atomic::Ordering::SeqCst);
}

fn read_into(s: &mut TcpStream, buf: &mut [u8]) -> Result<usize, i64> {
    wait_ready(s, true, deadline_of(s.read_timeout()))?;
    loop {
        match s.read(buf) {
            Ok(n) => return Ok(n),
            Err(e) if !give_up(&e) => continue,
            Err(e) => return Err(code(&e)),
        }
    }
}

// Up to `cap` bytes: what was read ahead first, then one read of the
// socket (as many as have arrived, at least one); none when the peer has
// closed its side.
fn read(st: &mut Stream, cap: usize) -> Result<Vec<u8>, i64> {
    if cap == 0 {
        return Ok(Vec::new());
    }
    if st.at < st.ahead.len() {
        let k = (st.ahead.len() - st.at).min(cap);
        let v = st.ahead[st.at..st.at + k].to_vec();
        st.at += k;
        return Ok(v);
    }
    let mut v = vec![0u8; cap];
    let n = read_into(&mut st.s, &mut v)?;
    v.truncate(n);
    Ok(v)
}

// The length of the next line, its '\n' included (what is left when the
// peer closes without one); 0 at the end. Its bytes are then read ahead.
fn line_len(st: &mut Stream) -> Result<u64, i64> {
    let mut from = st.at;
    loop {
        if let Some(i) = st.ahead[from..].iter().position(|b| *b == b'\n') {
            return Ok((from + i + 1 - st.at) as u64);
        }
        if st.at > 0 {
            st.ahead.drain(..st.at);
            st.at = 0;
        }
        let seen = st.ahead.len();
        let old = seen;
        st.ahead.resize(old + CHUNK, 0);
        let r = read_into(&mut st.s, &mut st.ahead[old..]);
        let got = *r.as_ref().unwrap_or(&0);
        st.ahead.truncate(old + got);
        r?;
        if got == 0 {
            return Ok(st.ahead.len() as u64);
        }
        from = seen;
    }
}

fn write_all(s: &mut TcpStream, mut data: &[u8]) -> Result<(), i64> {
    let deadline = deadline_of(s.write_timeout());
    while !data.is_empty() {
        wait_ready(s, false, deadline)?;
        match s.write(data) {
            Ok(0) => return Err(CLOSED),
            Ok(n) => data = &data[n..],
            Err(e) if !give_up(&e) => continue,
            Err(e) => return Err(code(&e)),
        }
    }
    Ok(())
}

fn connect(a: SocketAddr, ms: u64) -> Result<TcpStream, i64> {
    match timeout(ms) {
        Some(d) => TcpStream::connect_timeout(&a, d).map_err(|e| code(&e)),
        None => TcpStream::connect(a).map_err(|e| code(&e)),
    }
}

fn resolve(host: &[u8], port: &[u8]) -> Result<Vec<u8>, i64> {
    use std::net::ToSocketAddrs;
    let host = std::str::from_utf8(host).map_err(|_| OTHER)?;
    let port = u16::from_le_bytes([*port.first().unwrap_or(&0), *port.get(1).unwrap_or(&0)]);
    // Any failure to resolve is NotFound: the system's reasons (no such
    // name, no resolver reachable) do not map onto I/O errors.
    let addrs = (host, port).to_socket_addrs().map_err(|_| NOT_FOUND);
    let mut out = Vec::new();
    for a in addrs? {
        out.extend_from_slice(&addr_bytes(&a));
    }
    if out.is_empty() {
        return Err(NOT_FOUND);
    }
    Ok(out)
}

fn none() -> Vec<u8> {
    Vec::new()
}

// D-0181: `SO_KEEPALIVE`, which Rust's `std::net` does not expose: set
// through the system's `setsockopt` on the stream's own descriptor.
#[cfg(unix)]
fn set_keepalive(s: &TcpStream, on: bool) -> Result<(), i64> {
    use std::os::fd::AsRawFd;
    extern "C" {
        fn setsockopt(fd: i32, level: i32, name: i32, value: *const u8, len: u32) -> i32;
    }
    #[cfg(any(target_os = "linux", target_os = "android"))]
    const SOL_SOCKET: i32 = 1;
    #[cfg(any(target_os = "linux", target_os = "android"))]
    const SO_KEEPALIVE: i32 = 9;
    #[cfg(not(any(target_os = "linux", target_os = "android")))]
    const SOL_SOCKET: i32 = 0xffff;
    #[cfg(not(any(target_os = "linux", target_os = "android")))]
    const SO_KEEPALIVE: i32 = 8;
    let v: i32 = on as i32;
    // SAFETY: a 4-byte option value with its length.
    let r = unsafe { setsockopt(s.as_raw_fd(), SOL_SOCKET, SO_KEEPALIVE, &v as *const i32 as *const u8, 4) };
    if r == 0 {
        Ok(())
    } else {
        Err(OTHER)
    }
}

#[cfg(windows)]
fn set_keepalive(s: &TcpStream, on: bool) -> Result<(), i64> {
    use std::os::windows::io::AsRawSocket;
    #[link(name = "ws2_32")]
    extern "system" {
        fn setsockopt(s: usize, level: i32, name: i32, value: *const u8, len: i32) -> i32;
    }
    const SOL_SOCKET: i32 = 0xffff;
    const SO_KEEPALIVE: i32 = 8;
    let v: i32 = on as i32;
    // SAFETY: a 4-byte option value with its length.
    let r = unsafe { setsockopt(s.as_raw_socket() as usize, SOL_SOCKET, SO_KEEPALIVE, &v as *const i32 as *const u8, 4) };
    if r == 0 {
        Ok(())
    } else {
        Err(OTHER)
    }
}

#[cfg(not(any(unix, windows)))]
fn set_keepalive(_s: &TcpStream, _on: bool) -> Result<(), i64> {
    Err(OTHER)
}

pub fn call(op: u64, h: u64, a: &[u8], b: &[u8], cap: u64) -> (i64, Vec<u8>) {
    let r: Result<(i64, Vec<u8>), i64> = (|| match op {
        RESOLVE => resolve(a, b).map(|v| (v.len() as i64, v)),
        TCP_BIND => {
            let l = TcpListener::bind(addr_of(a)?).map_err(|e| code(&e))?;
            sys::backlog(&l);
            Ok((add(Sock::Listener(l, 0)), none()))
        }
        LOCAL_ADDR | PEER_ADDR => with(h, |s| {
            let r = match (s, op) {
                (Sock::Listener(l, _), LOCAL_ADDR) => l.local_addr(),
                (Sock::Stream(st), LOCAL_ADDR) => st.s.local_addr(),
                (Sock::Stream(st), _) => st.s.peer_addr(),
                (Sock::Udp(u), LOCAL_ADDR) => u.local_addr(),
                _ => return Err(OTHER),
            };
            let v = addr_bytes(&r.map_err(|e| code(&e))?);
            Ok((v.len() as i64, v))
        }),
        SET_ACCEPT_TIMEOUT => with(h, |s| match s {
            Sock::Listener(_, ms) => {
                *ms = cap;
                Ok((0, none()))
            }
            _ => Err(OTHER),
        }),
        ACCEPT => {
            let s = with(h, |s| match s {
                Sock::Listener(l, ms) => accept(l, *ms),
                _ => Err(OTHER),
            })?;
            Ok((add(Sock::Stream(Stream { s, ahead: Vec::new(), at: 0 })), none()))
        }
        CONNECT => {
            let s = connect(addr_of(a)?, cap)?;
            Ok((add(Sock::Stream(Stream { s, ahead: Vec::new(), at: 0 })), none()))
        }
        SET_READ_TIMEOUT => with(h, |s| {
            match s {
                Sock::Stream(st) => st.s.set_read_timeout(timeout(cap)),
                Sock::Udp(u) => u.set_read_timeout(timeout(cap)),
                _ => return Err(OTHER),
            }
            .map(|_| (0, none()))
            .map_err(|e| code(&e))
        }),
        SET_WRITE_TIMEOUT => with_stream(h, |st| st.s.set_write_timeout(timeout(cap)).map(|_| (0, none())).map_err(|e| code(&e))),
        SET_NODELAY => with_stream(h, |st| st.s.set_nodelay(cap != 0).map(|_| (0, none())).map_err(|e| code(&e))),
        READ => with_stream(h, |st| read(st, cap as usize).map(|v| (v.len() as i64, v))),
        LINE_LEN => with_stream(h, |st| line_len(st).map(|n| (n as i64, none()))),
        WRITE => with_stream(h, |st| write_all(&mut st.s, b).map(|_| (0, none()))),
        SHUTDOWN_WRITE => with_stream(h, |st| st.s.shutdown(std::net::Shutdown::Write).map(|_| (0, none())).map_err(|e| code(&e))),
        CLOSE => {
            let mut t = TABLE.lock().unwrap_or_else(|e| e.into_inner());
            match t.get_mut(h as usize).and_then(|s| s.take()) {
                Some(_) => Ok((0, none())),
                None => Err(OTHER),
            }
        }
        UDP_BIND => {
            let u = UdpSocket::bind(addr_of(a)?).map_err(|e| code(&e))?;
            Ok((add(Sock::Udp(u)), none()))
        }
        SEND_TO => with(h, |s| match s {
            Sock::Udp(u) => loop {
                match u.send_to(b, addr_of(a)?) {
                    Ok(n) => return Ok((n as i64, none())),
                    Err(e) if !give_up(&e) => continue,
                    Err(e) => return Err(code(&e)),
                }
            },
            _ => Err(OTHER),
        }),
        RECV_FROM => with(h, |s| match s {
            Sock::Udp(u) => {
                // The answer is the sender's record and then the bytes, and
                // must fit `cap`: the datagram's room is what is left after
                // the record (a longer datagram's rest is lost, `[Udp]`).
                let mut v = vec![0u8; (cap as usize).saturating_sub(ADDR_LEN)];
                wait_ready(u, true, deadline_of(u.read_timeout()))?;
                let (n, from) = loop {
                    match u.recv_from(&mut v) {
                        Ok(x) => break x,
                        Err(e) if !give_up(&e) => continue,
                        Err(e) => return Err(code(&e)),
                    }
                };
                v.truncate(n);
                let mut out = addr_bytes(&from);
                out.extend_from_slice(&v);
                Ok((out.len() as i64, out))
            }
            _ => Err(OTHER),
        }),
        SET_KEEPALIVE => with_stream(h, |st| set_keepalive(&st.s, cap != 0).map(|_| (0, none()))),
        // D-0205: a UDP socket too, so one thread can wait for datagrams
        // while others send on the same socket.
        TRY_CLONE => {
            let c = with(h, |s| match s {
                Sock::Stream(st) => st.s.try_clone().map(|x| Sock::Stream(Stream { s: x, ahead: Vec::new(), at: 0 })).map_err(|e| code(&e)),
                Sock::Udp(u) => u.try_clone().map(Sock::Udp).map_err(|e| code(&e)),
                Sock::Listener(..) => Err(OTHER),
            })?;
            Ok((add(c), none()))
        }
        UDP_CONNECT => with(h, |s| match s {
            Sock::Udp(u) => u.connect(addr_of(a)?).map(|_| (0, none())).map_err(|e| code(&e)),
            _ => Err(OTHER),
        }),
        UDP_SEND => with(h, |s| match s {
            Sock::Udp(u) => loop {
                match u.send(b) {
                    Ok(n) => return Ok((n as i64, none())),
                    Err(e) if !give_up(&e) => continue,
                    Err(e) => return Err(code(&e)),
                }
            },
            _ => Err(OTHER),
        }),
        UDP_RECV => with(h, |s| match s {
            Sock::Udp(u) => {
                let mut v = vec![0u8; cap as usize];
                wait_ready(u, true, deadline_of(u.read_timeout()))?;
                let n = loop {
                    match u.recv(&mut v) {
                        Ok(x) => break x,
                        Err(e) if !give_up(&e) => continue,
                        Err(e) => return Err(code(&e)),
                    }
                };
                v.truncate(n);
                Ok((v.len() as i64, v))
            }
            _ => Err(OTHER),
        }),
        PARSE_IP => {
            let t = std::str::from_utf8(a).map_err(|_| OTHER)?;
            let ip: IpAddr = t.parse().map_err(|_| OTHER)?;
            let v = addr_bytes(&SocketAddr::new(ip, 0));
            Ok((v.len() as i64, v))
        }
        ADDR_TEXT => {
            let v = addr_of(a)?.ip().to_string().into_bytes();
            Ok((v.len() as i64, v))
        }
        _ => Err(OTHER),
    })();
    match r {
        Ok(x) => x,
        Err(c) => (c, Vec::new()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lo(port: u16) -> Vec<u8> {
        addr_bytes(&SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), port))
    }

    #[test]
    fn unreachable_by_number() {
        #[cfg(target_os = "linux")]
        for n in [100, 101, 113] {
            assert_eq!(code(&std::io::Error::from_raw_os_error(n)), UNREACHABLE);
        }
    }

    #[test]
    fn echo_once() {
        let (l, _) = call(TCP_BIND, 0, &lo(0), b"", 0);
        assert!(l >= 0);
        let (_, la) = call(LOCAL_ADDR, l as u64, b"", b"", 0);
        let port = u16::from_le_bytes([la[17], la[18]]);
        assert_ne!(port, 0);
        let t = std::thread::spawn(move || {
            let (s, _) = call(ACCEPT, l as u64, b"", b"", 0);
            assert!(s >= 0);
            assert_eq!(call(LINE_LEN, s as u64, b"", b"", 0).0, 6);
            let (_, line) = call(READ, s as u64, b"", b"", 6);
            assert_eq!(line, b"hello\n");
            call(WRITE, s as u64, b"", b"HELLO\n", 0);
            call(CLOSE, s as u64, b"", b"", 0);
        });
        let (c, _) = call(CONNECT, 0, &lo(port), b"", 0);
        assert!(c >= 0);
        call(WRITE, c as u64, b"", b"hello\n", 0);
        assert_eq!(call(LINE_LEN, c as u64, b"", b"", 0).0, 6);
        assert_eq!(call(READ, c as u64, b"", b"", 100).1, b"HELLO\n");
        assert_eq!(call(READ, c as u64, b"", b"", 100).0, 0);
        t.join().unwrap();
        call(CLOSE, c as u64, b"", b"", 0);
        call(SET_ACCEPT_TIMEOUT, l as u64, b"", b"", 50);
        assert_eq!(call(ACCEPT, l as u64, b"", b"", 0).0, TIMED_OUT);
        call(CLOSE, l as u64, b"", b"", 0);
        // Refused: port 1, which only a privileged process may listen on.
        // (The port just closed was once asked here, but a test running
        // beside this one can be given it at once, and then it answers.)
        assert_eq!(call(CONNECT, 0, &lo(1), b"", 0).0, REFUSED);
    }
}
