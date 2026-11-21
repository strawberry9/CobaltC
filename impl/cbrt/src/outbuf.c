/* Standard output's buffer (D-0178, `rule.stdlib.print` [Print-Buffer]).
   Every byte a program writes to standard output passes through
   `outbuf_write`; it reaches the operating system when the buffer fills,
   at a newline when standard output is a terminal, and at `outbuf_flush`,
   which the runtime calls before a write to standard error, a read of
   standard input, a child that shares standard output, a fault's report
   and the program's end. Within the program, standard output and standard
   error therefore stay in the order it wrote them. */

#include "io.h"
#include "plat.h"
#include "plat_io.h"

#include <stdio.h>

/* The buffer's size: a full buffer is one `write(2)`. */
#define CAP 8192

/* The buffer's lock. Uncontended (every program with one thread, and most
   writes in one with many) it is one atomic exchange to take and one to give
   back, inline, as Rust's `Mutex` is: a `printf` a line costs no call into
   the C library's mutex. Contended, a thread waits on `ob_cond` under
   `ob_mutex`. `ob_state`: 0 free, 1 held, 2 held with a thread waiting (or
   one that has waited). */
static int ob_state;
static plat_mutex ob_mutex = PLAT_MUTEX_INIT;
static plat_cond ob_cond = PLAT_COND_INIT;

static inline void ob_lock(void)
{
    int free_ = 0;
    if (__atomic_compare_exchange_n(&ob_state, &free_, 1, 0, __ATOMIC_ACQUIRE, __ATOMIC_RELAXED))
    {
        return;
    }
    plat_mutex_lock(&ob_mutex);
    while (__atomic_exchange_n(&ob_state, 2, __ATOMIC_ACQUIRE) != 0)
    {
        plat_cond_wait(&ob_cond, &ob_mutex);
    }
    plat_mutex_unlock(&ob_mutex);
}

static inline void ob_unlock(void)
{
    if (__atomic_exchange_n(&ob_state, 0, __ATOMIC_RELEASE) == 2)
    {
        /* A waiter either has not yet set the state (and finds it free) or
           waits on the condition, holding `ob_mutex` until it does. */
        plat_mutex_lock(&ob_mutex);
        plat_cond_broadcast(&ob_cond);
        plat_mutex_unlock(&ob_mutex);
    }
}
static uint8_t *buf;
static size_t buf_n;
/* -1 not yet known, else whether standard output is a terminal. */
static int tty_state = -1;
/* Whether the buffer may hold bytes: `outbuf_flush` with nothing buffered
   (before each read of standard input, a byte at a time for `read_line`)
   then costs one load. Set by the writing thread itself, so a thread
   always sees its own writes. */
static int pending;

/* A terminal is written a line at a time, so a person sees each line as
   the program finishes it. */
static int tty(void)
{
    int t = __atomic_load_n(&tty_state, __ATOMIC_ACQUIRE);
    if (t < 0)
    {
        t = os_stdout_is_tty() ? 1 : 0;
        __atomic_store_n(&tty_state, t, __ATOMIC_RELEASE);
    }
    return t;
}

static void ensure_buf(void)
{
    if (!buf)
    {
        buf = (uint8_t *)cb_xmalloc(CAP);
    }
}

/* Issue 028: the first failure to write standard output (an `os_kind`, 0
   for none). `printf` cannot fail, so the program learns nothing of it; at
   its end (`outbuf_end_status`) one line on standard error says the output
   was lost, and the exit status is not 0, as C's `close_stdout` idiom
   makes it. */
static int lost_kind;

static int put(const uint8_t *p, size_t n)
{
    if (n == 0)
    {
        return 0;
    }
    int k = os_std_write(1, p, n);
    if (k == OS_OK)
    {
        return 0;
    }
    int none = 0;
    __atomic_compare_exchange_n(&lost_kind, &none, k, 0, __ATOMIC_RELAXED, __ATOMIC_RELAXED);
    return -1;
}

static int drain(void)
{
    int r = put(buf, buf_n);
    buf_n = 0;
    __atomic_store_n(&pending, 0, __ATOMIC_RELEASE);
    return r;
}

/* Makes the buffer and standard output's own state before the program runs
   (`cb_init`): made at the first write instead, their blocks are among the
   first large ones the program asks for, and glibc then consolidates every
   small block freed before (a tree just dropped). */
void outbuf_prepare(void)
{
    (void)tty();
    ob_lock();
    ensure_buf();
    ob_unlock();
}

/* Writes `p` to standard output through the buffer. An error is the
   operating system's, from this call's own write or from an earlier call's
   bytes written now. */
int outbuf_write(const uint8_t *p, size_t n)
{
    int r = 0;
    ob_lock();
    ensure_buf();
    if (buf_n + n > CAP)
    {
        if (drain() != 0)
        {
            ob_unlock();
            return -1;
        }
        if (n >= CAP)
        {
            r = put(p, n);
            ob_unlock();
            return r;
        }
    }
    if (n)
    {
        memcpy(buf + buf_n, p, n);
        buf_n += n;
    }
    __atomic_store_n(&pending, 1, __ATOMIC_RELEASE);
    if (buf_n == CAP || (tty() && n && memchr(p, '\n', n)))
    {
        r = drain();
    }
    ob_unlock();
    return r;
}

/* Writes out whatever the buffer holds. */
int outbuf_flush(void)
{
    if (!__atomic_load_n(&pending, __ATOMIC_ACQUIRE))
    {
        return 0;
    }
    ob_lock();
    int r = drain();
    ob_unlock();
    return r;
}

/* `[Print-Failure]`: the status a program ends with, given the one it chose,
   once standard output is written out. A pipe whose reader has gone (a
   program piped into `head`) ends it quietly; any other failure is named on
   standard error. Either way a status of 0 becomes 1. */
int outbuf_end_status(int status)
{
    (void)outbuf_flush();
    int k = __atomic_load_n(&lost_kind, __ATOMIC_RELAXED);
    if (k == 0)
    {
        return status;
    }
    if (k != OS_BROKEN_PIPE)
    {
        const char *why = k == OS_NO_SPACE ? "no space left on the device"
                        : k == OS_DENIED   ? "permission denied"
                        : k == OS_WRITE_ZERO ? "the device took no bytes"
                                           : "input/output error";
        char line[160];
        int m = snprintf(line, sizeof line, "error: standard output could not be written (%s): the program's output is incomplete\n", why);
        if (m > 0)
        {
            (void)os_std_write(2, (const uint8_t *)line, (size_t)m < sizeof line ? (size_t)m : sizeof line - 1);
        }
    }
    return status == 0 ? 1 : status;
}

/* Writes `p` to standard error, after standard output's buffer. */
int outbuf_write_err(const uint8_t *p, size_t n)
{
    (void)outbuf_flush();
    if (n == 0)
    {
        return 0;
    }
    return os_std_write(2, p, n) == OS_OK ? 0 : -1;
}
