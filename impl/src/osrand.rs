// `std::os_random` (`spec/21` `[Os-Random]`, D-0142): bytes from the
// operating system's cryptographically secure source. One source file
// for both implementations: the interpreter uses it as a module and the
// compiler's runtime (`cbrt`) includes this same file. Never seeded from
// a clock: when the system has no source, the answer is -1 and `std`
// faults (`diag.entropy-unavailable`).
//
// Linux: the `getrandom(2)` system call (by number, so an old C library
// does not matter), falling back to `/dev/urandom` on a kernel without
// it; other Unix systems: `/dev/urandom`; Windows: `BCryptGenRandom`
// with the system's preferred generator.

#[cfg(all(target_os = "linux", any(target_arch = "x86_64", target_arch = "aarch64")))]
fn getrandom(buf: &mut [u8]) -> Option<bool> {
    extern "C" {
        fn syscall(num: i64, ...) -> i64;
    }
    #[cfg(target_arch = "x86_64")]
    const SYS_GETRANDOM: i64 = 318;
    #[cfg(target_arch = "aarch64")]
    const SYS_GETRANDOM: i64 = 278;
    let mut at = 0;
    while at < buf.len() {
        let rest = &mut buf[at..];
        // SAFETY: the kernel writes at most `rest.len()` bytes at `rest`.
        let n = unsafe { syscall(SYS_GETRANDOM, rest.as_mut_ptr(), rest.len(), 0u32) };
        if n < 0 {
            let e = std::io::Error::last_os_error();
            if e.kind() == std::io::ErrorKind::Interrupted {
                continue;
            }
            // ENOSYS: a kernel before 3.17; the device will do.
            return if at == 0 { None } else { Some(false) };
        }
        at += n as usize;
    }
    Some(true)
}

#[cfg(not(all(target_os = "linux", any(target_arch = "x86_64", target_arch = "aarch64"))))]
#[allow(dead_code)]
fn getrandom(_buf: &mut [u8]) -> Option<bool> {
    None
}

#[cfg(unix)]
fn fill_os(buf: &mut [u8]) -> bool {
    use std::io::Read;
    match getrandom(buf) {
        Some(ok) => ok,
        None => match std::fs::File::open("/dev/urandom") {
            Ok(mut f) => f.read_exact(buf).is_ok(),
            Err(_) => false,
        },
    }
}

#[cfg(windows)]
fn fill_os(buf: &mut [u8]) -> bool {
    #[link(name = "bcrypt")]
    extern "system" {
        fn BCryptGenRandom(alg: *mut std::ffi::c_void, buf: *mut u8, n: u32, flags: u32) -> i32;
    }
    const BCRYPT_USE_SYSTEM_PREFERRED_RNG: u32 = 2;
    for chunk in buf.chunks_mut(u32::MAX as usize) {
        // SAFETY: the system writes exactly `chunk.len()` bytes at `chunk`.
        let r = unsafe { BCryptGenRandom(std::ptr::null_mut(), chunk.as_mut_ptr(), chunk.len() as u32, BCRYPT_USE_SYSTEM_PREFERRED_RNG) };
        if r != 0 {
            return false;
        }
    }
    true
}

#[cfg(not(any(unix, windows)))]
fn fill_os(_buf: &mut [u8]) -> bool {
    false
}

// Fills `buf`: 0, or -1 when the system gives no secure bytes.
pub fn fill(buf: &mut [u8]) -> i64 {
    if buf.is_empty() || fill_os(buf) {
        0
    } else {
        -1
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn differs_and_fills() {
        let mut a = [0u8; 32];
        let mut b = [0u8; 32];
        assert_eq!(super::fill(&mut a), 0);
        assert_eq!(super::fill(&mut b), 0);
        assert_ne!(a, b);
        assert_eq!(super::fill(&mut []), 0);
    }
}
