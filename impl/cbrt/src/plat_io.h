/* What the I/O modules (outbuf.c, fileio.c, procio.c, netio.c, osrand.c)
   need of the operating system: what Rust's `std::fs`, `std::process`,
   `std::net` and `std::io` do underneath them. Implemented by plat_posix.c
   (glibc) and plat_win.c (Windows 7 and later, MinGW-w64), which keep the
   standard library's own behaviour: the flags it opens files and sockets
   with (close-on-exec everywhere on POSIX; share modes and no handle
   inheritance on Windows), its retries on EINTR where it retries, its
   argument quoting and program search on Windows, its mapping of the
   system's error numbers onto `ErrorKind`s (`os_kind` below).

   Paths and other text cross this interface as the bytes the program holds
   (UTF-8; on Windows WTF-8 coming back, Rust's `into_encoded_bytes`). A
   NUL inside a path is `OS_INVALID`, as Rust's "file name contained an
   unexpected NUL byte". Every function answers an `os_kind`: OS_OK (0) or
   the kind of the failure. */

#ifndef CBRT_PLAT_IO_H
#define CBRT_PLAT_IO_H

#include "common.h"

/* Rust's `std::io::ErrorKind`, as far as the modules tell kinds apart;
   OS_UNREACHABLE is a network or host that is down or unreachable by its
   raw error number (netio's `unreachable`). */
enum os_kind
{
    OS_OK = 0,
    OS_NOT_FOUND,
    OS_DENIED,
    OS_EXISTS,
    OS_INTERRUPTED,
    OS_WOULD_BLOCK,
    OS_TIMED_OUT,
    OS_REFUSED,
    OS_RESET,
    OS_ABORTED,
    OS_ADDR_IN_USE,
    OS_ADDR_NOT_AVAILABLE,
    OS_BROKEN_PIPE,
    OS_NOT_CONNECTED,
    OS_UNREACHABLE,
    OS_INVALID,
    OS_WRITE_ZERO,
    /* Issue 041: told apart so that std's errors can name them, where they
       were all "input/output error". */
    OS_IS_DIR,
    OS_NOT_DIR,
    OS_NO_SPACE,
    OS_TOO_MANY_OPEN,
    OS_MESSAGE_TOO_LONG,
    OS_OTHER
};

/* 1 on Windows, 0 elsewhere: the path grammar (`std::path`) and the few
   rules that differ by platform in otherwise portable code. */
extern const int os_windows;

/* ---- standard streams ---- */

/* `std::io::stdout()`/`stderr()`'s `write_all`: 1 standard output, 2
   standard error. A stream that is not open counts as written (Rust's
   `handle_ebadf`); on a Windows console the bytes are written as UTF-16,
   and bytes that are not UTF-8 are an error, as Rust's console writer. */
int os_std_write(int which, const uint8_t *p, size_t n);
/* One read of standard input's handle (no buffering): bytes in `*got`, 0
   at its end; a stream that is not open reads as ended. */
int os_stdin_read(uint8_t *p, size_t n, size_t *got);
int os_stdout_is_tty(void);

/* ---- files ---- */

typedef intptr_t os_file;

/* fileio's MODE_* (READ 0, CREATE 1, APPEND 2, READ_WRITE 3, CREATE_NEW 4)
   as Rust's `OpenOptions` for each. */
int os_file_open(const uint8_t *path, size_t n, int mode, os_file *f);
/* One read or write call; OS_INTERRUPTED for the caller to retry. */
int os_file_read(os_file f, uint8_t *p, size_t n, size_t *got);
int os_file_write(os_file f, const uint8_t *p, size_t n, size_t *put);
/* `Seek`: whence 0 start, 1 current; the new position. */
int os_file_seek(os_file f, int whence, int64_t off, uint64_t *pos);
/* `File::metadata`: the length and whether it is a directory. */
int os_file_stat(os_file f, uint64_t *len, int *is_dir);
int os_file_sync(os_file f);
void os_file_close(os_file f);

/* `std::fs::read` and `std::fs::write`. */
int os_read_file(const uint8_t *path, size_t n, cb_buf *out);
int os_write_file(const uint8_t *path, size_t n, const uint8_t *d, size_t dn);

/* `std::fs::metadata` (following links). */
typedef struct
{
    int kind; /* 0 file, 1 directory, 2 other */
    uint64_t len;
    int has_modified;
    int64_t mod_secs; /* seconds since 1970, rounded down */
    int readonly;
} os_meta;
int os_metadata(const uint8_t *path, size_t n, os_meta *m);
/* `Path::is_dir`: metadata succeeds and says directory. */
int os_is_dir(const uint8_t *path, size_t n);

int os_mkdir(const uint8_t *path, size_t n);
int os_remove_file(const uint8_t *path, size_t n);
int os_remove_dir(const uint8_t *path, size_t n);
int os_rename(const uint8_t *a, size_t an, const uint8_t *b, size_t bn);
/* `read_dir`'s names (without `.` and `..`), each followed by a 0 byte, in
   the order the system gives them. */
int os_list_dir(const uint8_t *path, size_t n, cb_buf *names, size_t *count);
int os_canonicalize(const uint8_t *path, size_t n, cb_buf *out);
/* `std::fs::copy`: the bytes copied (on Windows, the copy's size). */
int os_copy_file(const uint8_t *a, size_t an, const uint8_t *b, size_t bn, uint64_t *copied);
/* `set_permissions(b, metadata(a).permissions())`. */
int os_copy_permissions(const uint8_t *a, size_t an, const uint8_t *b, size_t bn);
int os_remove_dir_all(const uint8_t *path, size_t n);
int os_set_current_dir(const uint8_t *path, size_t n);

/* ---- the environment ---- */

/* `env::var_os`: 1 and the value's bytes when set, 0 when not. */
int os_env_var(const uint8_t *name, size_t n, cb_buf *out);
int os_current_dir(cb_buf *out);
/* `env::temp_dir().to_string_lossy()`. */
void os_temp_dir(cb_buf *out);
int os_current_exe(cb_buf *out);
int os_hostname(cb_buf *out);

/* ---- clocks ---- */

/* `SystemTime::now()` since 1970: whole seconds (negative before it) and
   the nanoseconds after them (always 0..1e9, toward the future). */
void os_wall_time(int64_t *secs, uint32_t *nanos);
/* fileio's `platform_offset`, the system's part: 1 and the local time at
   the moment `t` (year, month 1-12, day, hour, minute, second) when the
   system has a rule for it. */
int os_local_time(int64_t t, int64_t tm[6]);

/* ---- child processes ---- */

typedef intptr_t os_pipe;
#define OS_PIPE_NONE ((os_pipe)-1)

typedef struct os_child os_child;

enum
{
    OS_STDIO_INHERIT = 0,
    OS_STDIO_NULL = 1,
    OS_STDIO_PIPED = 2
};

/* A command as `std::process::Command` is given it (procio's `Spec`). */
typedef struct
{
    size_t argc;
    const uint8_t **argv;
    const size_t *argl;
    int clear_env;
    int has_dir;
    const uint8_t *dir;
    size_t dirn;
    size_t nenv;
    const uint8_t **env_k;
    const size_t *env_kn;
    const uint8_t **env_v;
    const size_t *env_vn;
} os_cmd;

/* `Command::spawn` with the three streams as given; a piped stream's
   parent end in `*in`, `*out`, `*err` (OS_PIPE_NONE otherwise). */
int os_spawn(const os_cmd *c, int in_mode, int out_mode, int err_mode, os_child **child, os_pipe *in, os_pipe *out, os_pipe *err);
/* `Child::wait` (one call: OS_INTERRUPTED for the caller to retry) and
   `try_wait`; procio's `status_of`: the exit code, or minus the signal. */
int os_child_wait(os_child *c, int *status);
int os_child_try_wait(os_child *c, int *done, int *status);
int os_child_kill(os_child *c);
/* SIGTERM on POSIX (OS_OTHER when `kill` fails), `kill` on Windows. */
int os_child_terminate(os_child *c);
int os_child_kill_tree(os_child *c);
uint32_t os_child_id(os_child *c);
/* The child's handle given back (it is not waited for). */
void os_child_free(os_child *c);

/* Rust's `read2` (`wait_with_output`): both pipes read to their ends at
   once, appended to `va` and `vb`. */
int os_read2(os_pipe a, cb_buf *va, os_pipe b, cb_buf *vb);
int os_pipe_read(os_pipe p, uint8_t *buf, size_t n, size_t *got);
int os_pipe_write(os_pipe p, const uint8_t *buf, size_t n, size_t *put);
void os_pipe_close(os_pipe p);

uint32_t os_process_id(void);
/* procio's `watch`: SIGINT and SIGTERM (Ctrl-C and the console's close on
   Windows) set `*flag` to 1, and a blocking call they arrive in returns
   EINTR instead of resuming. */
void os_watch_interrupts(int *flag);
void os_ignore_sigpipe(void);

/* ---- sockets ---- */

typedef intptr_t os_sock;

/* A socket address (netio's 19-byte record, plus what the system keeps). */
typedef struct
{
    int family; /* 4 or 6 */
    uint8_t ip[16];
    uint16_t port;
} os_addr;

int os_tcp_bind(const os_addr *a, os_sock *s);
int os_udp_bind(const os_addr *a, os_sock *s);
int os_sock_local(os_sock s, os_addr *a);
int os_sock_peer(os_sock s, os_addr *a);
int os_sock_nonblocking(os_sock s, int on);
int os_accept(os_sock l, os_sock *s);
/* `TcpStream::connect` (ms 0) or `connect_timeout`. */
int os_connect(const os_addr *a, uint64_t ms, os_sock *s);
/* SO_RCVTIMEO / SO_SNDTIMEO: set (`ms` 0: none) and read back, in
   nanoseconds (0: none), as `set_read_timeout`/`read_timeout`. */
int os_set_timeout(os_sock s, int write, uint64_t ms);
int os_get_timeout(os_sock s, int write, uint64_t *ns);
int os_set_nodelay(os_sock s, int on);
/* OS_OTHER on any failure (netio's `set_keepalive`). */
int os_set_keepalive(os_sock s, int on);
int os_recv(os_sock s, uint8_t *p, size_t n, size_t *got);
int os_send(os_sock s, const uint8_t *p, size_t n, size_t *put);
int os_shutdown_write(os_sock s);
void os_sock_close(os_sock s);
int os_sendto(os_sock s, const uint8_t *p, size_t n, const os_addr *a, size_t *put);
int os_recvfrom(os_sock s, uint8_t *p, size_t n, size_t *got, os_addr *from);
int os_udp_connect(os_sock s, const os_addr *a);
int os_sock_dup(os_sock s, os_sock *out);
/* `(host, port).to_socket_addrs()` past the literal forms: the system's
   resolver; the addresses in its order. */
int os_resolve(const char *host, uint16_t port, os_addr **out, size_t *count);
/* netio's readiness test: 1 in `*ready` when readable (or writable); `ms` -1
   waits without limit. OS_INTERRUPTED when a signal (a wake) ended it. */
int os_ready(os_sock s, int read, int ms, int *ready);
/* netio's `sys` wake machinery (D-0204): Linux only; elsewhere the
   functions do nothing and `os_wake_enabled` is 0. */
void os_wake_init(void);
int os_wake_enabled(void);
uint64_t os_wake_thread_self(void);
void os_wake_thread(uint64_t t);

/* ---- random bytes ---- */

/* osrand's `fill_os`: 1 when `p` holds `n` secure bytes. */
int os_random(uint8_t *p, size_t n);

#endif
