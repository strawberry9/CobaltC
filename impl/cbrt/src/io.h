/* The runtime's I/O modules as lib.c calls them: standard output's buffer
   (outbuf.c, D-0178), files, the file system, the environment and the
   clocks (fileio.c: D-0054, D-0061, D-0141, D-0147, D-0181, D-0201),
   child processes (procio.c, D-0143, D-0195, D-0205), sockets (netio.c,
   D-0144, D-0204) and the OS's random bytes (osrand.c, D-0142). Their OS
   calls are plat_io.h's.

   Results are numbers: a count, a handle or 0 on success, a negative
   failure code otherwise. Every `cb_buf *out` is cleared by the callee
   before it writes its answer there; the caller owns the buffer and may
   reuse it between calls. */

#ifndef CBRT_IO_H
#define CBRT_IO_H

#include "common.h"

/* ---- outbuf: 0 for Ok, -1 for Err ---- */

void outbuf_prepare(void);
int outbuf_write(const uint8_t *p, size_t n);
int outbuf_flush(void);
int outbuf_write_err(const uint8_t *p, size_t n);
/* Issue 028: the program's exit status once its end has flushed standard
   output, `status` or 1 for a run whose standard output lost bytes. */
int outbuf_end_status(int status);

/* ---- fileio ---- */

#define FILEIO_NOT_FOUND (-1)
#define FILEIO_DENIED (-2)
#define FILEIO_OTHER (-3)
#define FILEIO_EXISTS (-4)
/* Issue 041: `FileError`'s `IsDirectory`, `NotDirectory`, `NoSpace` and
   `TooManyOpen` (std's `file_error`, `code_error`). */
#define FILEIO_IS_DIR (-5)
#define FILEIO_NOT_DIR (-6)
#define FILEIO_NO_SPACE (-7)
#define FILEIO_TOO_MANY_OPEN (-8)

/* `read`: 0 and the bytes in `out`, or the code. */
int64_t fileio_read(const uint8_t *path, size_t pn, cb_buf *out);
int64_t fileio_write(const uint8_t *path, size_t pn, const uint8_t *d, size_t dn);
int fileio_takes_input(uint64_t op);
/* `call`: OP_READ's bytes in `out` (which holds nothing for the others). */
int64_t fileio_call(uint64_t op, uint64_t h, const uint8_t *in, size_t inn, uint64_t n, cb_buf *out);
int64_t fileio_fs_op(uint64_t op, const uint8_t *a, size_t an, const uint8_t *b, size_t bn);
/* `fs_query`: 0 and the answer in `out`, or the code. */
int64_t fileio_fs_query(uint64_t op, const uint8_t *a, size_t an, cb_buf *out);
void fileio_flush_all(void);
void fileio_sleep_ns(uint64_t ns);
int64_t fileio_clock_read(uint64_t which);
int64_t fileio_tz_offset(int64_t t);
/* `read_line_chunk` and `std::stdin_read`: both over the one buffered
   standard input (Rust's `std::io::stdin()`, an 8 KiB `BufReader`). */
int64_t fileio_read_line_chunk(uint8_t *buf, size_t n);
int64_t io_stdin_read(uint8_t *buf, size_t n);

/* ---- the byte calls: (op, h, a, b, cap) -> the number, and the answer's
   bytes in `out`, of which the caller copies up to `cap` ---- */

typedef int64_t (*io_byte_call)(uint64_t op, uint64_t h, const uint8_t *a, size_t an, const uint8_t *b, size_t bn, uint64_t cap, cb_buf *out);

int64_t fileio_path_call(uint64_t op, uint64_t h, const uint8_t *a, size_t an, const uint8_t *b, size_t bn, uint64_t cap, cb_buf *out);
int64_t procio_call(uint64_t op, uint64_t h, const uint8_t *a, size_t an, const uint8_t *b, size_t bn, uint64_t cap, cb_buf *out);
int64_t netio_call(uint64_t op, uint64_t h, const uint8_t *a, size_t an, const uint8_t *b, size_t bn, uint64_t cap, cb_buf *out);

/* ---- procio ---- */

/* D-0143: SIGPIPE ignored, so a write to a pipe whose reader is gone fails
   instead of ending the program (`cb_set_args`). */
void procio_ignore_sigpipe(void);
int procio_interrupt_requested(void);

/* ---- netio ---- */

#define NETIO_CANCELLED (-11)

/* D-0204: the runtime's question while a socket waits (has this thread
   been asked to stop?), set once by `cb_init`. */
void netio_set_cancel_probe(int (*f)(void));
void netio_wake_init(void);
uint64_t netio_os_thread(void);
void netio_wake_thread(uint64_t t);
/* Set by procio's WATCH_INTERRUPTS. */
void netio_watching(void);

/* ---- osrand: 0, or -1 when the system gives no secure bytes ---- */

int64_t osrand_fill(uint8_t *p, size_t n);

#endif
