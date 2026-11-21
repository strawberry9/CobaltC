/* Sockets: TCP, UDP and name resolution (`spec/21` §2l `rule.stdlib.net`,
   D-0144).

   `std::net_op(op, h, a, b, out, cap)` is the one primitive. `h` is a
   socket's handle: its index in a process-wide table, shared by every
   thread. Each entry is locked on its own, and the table only long enough
   to find one, so a thread blocked in `accept` or `read` holds no other
   thread up (Rust's `Arc<Mutex<Sock>>`: a socket closed while a call uses
   it stays open until that call ends). An address travels as a 19-byte
   record (`addr_bytes`). For an operation that answers no bytes, `cap` is
   its number (a timeout in milliseconds, a flag).

   Sockets block, each with its own timeouts. A timeout is `TIMED_OUT`, and
   the socket stays usable. Under `watch_interrupts` (D-0143), an interrupt
   that arrives while a call blocks makes it return `TIMED_OUT` too, so a
   loop sees the flag. D-0204: a wait is made in slices (`wait_ready`), so a
   thread asked to stop (`cancel`) leaves it with `CANCELLED`. */

#include "io.h"
#include "plat.h"
#include "plat_io.h"

#define NOT_FOUND (-1)
#define DENIED (-2)
#define OTHER (-3)
#define REFUSED (-4)
#define RESET (-5)
#define TIMED_OUT (-6)
#define UNREACHABLE (-7)
#define ADDR_IN_USE (-8)
#define ADDR_NOT_AVAILABLE (-9)
#define CLOSED (-10)
/* Issue 041: `NetError`'s `NoDescriptors` and `MessageTooLong`. */
#define TOO_MANY_OPEN (-12)
#define MESSAGE_TOO_LONG (-13)
/* D-0204: the waiting thread was asked to stop (`cancel`); the runtime
   raises `diag.thread-cancelled`. */
#define CANCELLED NETIO_CANCELLED

#define RESOLVE 0
#define TCP_BIND 1
#define LOCAL_ADDR 2
#define PEER_ADDR 3
#define SET_ACCEPT_TIMEOUT 4
#define ACCEPT 5
#define CONNECT 6
#define SET_READ_TIMEOUT 7
#define SET_WRITE_TIMEOUT 8
#define SET_NODELAY 9
#define READ 10
#define LINE_LEN 11
#define WRITE 12
#define SHUTDOWN_WRITE 13
#define CLOSE 14
#define UDP_BIND 15
#define SEND_TO 16
#define RECV_FROM 17
#define PARSE_IP 18
#define ADDR_TEXT 19
#define SET_KEEPALIVE 20 /* D-0181 */
#define UDP_CONNECT 21   /* D-0181 */
#define UDP_SEND 22      /* D-0181 */
#define UDP_RECV 23      /* D-0181 */
#define TRY_CLONE 24     /* D-0185 */

/* The largest UDP datagram (65,535 bytes less the IPv4 and UDP headers); a
   datagram is received whole into this much room, then cut. */
#define DATAGRAM_MAX 65536

static int64_t code(int kind)
{
    switch (kind)
    {
    case OS_NOT_FOUND:
        return NOT_FOUND;
    case OS_DENIED:
        return DENIED;
    case OS_REFUSED:
        return REFUSED;
    case OS_RESET:
    case OS_ABORTED:
        return RESET;
    case OS_TIMED_OUT:
    case OS_WOULD_BLOCK:
        return TIMED_OUT;
    case OS_ADDR_IN_USE:
        return ADDR_IN_USE;
    case OS_ADDR_NOT_AVAILABLE:
        return ADDR_NOT_AVAILABLE;
    case OS_BROKEN_PIPE:
    case OS_NOT_CONNECTED:
        return CLOSED;
    case OS_INTERRUPTED:
        return TIMED_OUT;
    case OS_UNREACHABLE:
        /* A network that is down or unreachable, or a host that is
           unreachable, by the system's own error number. */
        return UNREACHABLE;
    case OS_TOO_MANY_OPEN:
        return TOO_MANY_OPEN;
    case OS_MESSAGE_TOO_LONG:
        return MESSAGE_TOO_LONG;
    default:
        return OTHER;
    }
}

/* Whether a call interrupted by a signal should give up: only when it was
   an interrupt the program watches for; otherwise it is resumed. */
static int give_up(int kind)
{
    return kind != OS_INTERRUPTED || procio_interrupt_requested();
}

/* The record of an address: the family (4 or 6), 16 bytes of address (a v4
   address in the first 4), the port (2 bytes, little-endian). */
#define ADDR_LEN 19

static void addr_bytes(const os_addr *a, cb_buf *out)
{
    uint8_t v[ADDR_LEN];
    memset(v, 0, sizeof v);
    v[0] = (uint8_t)a->family;
    memcpy(v + 1, a->ip, a->family == 4 ? 4 : 16);
    v[17] = (uint8_t)a->port;
    v[18] = (uint8_t)(a->port >> 8);
    cb_buf_put(out, v, ADDR_LEN);
}

static int addr_of(const uint8_t *b, size_t n, os_addr *a)
{
    if (n < ADDR_LEN)
    {
        return 0;
    }
    memset(a, 0, sizeof *a);
    a->port = (uint16_t)(b[17] | b[18] << 8);
    if (b[0] == 4)
    {
        a->family = 4;
        memcpy(a->ip, b + 1, 4);
        return 1;
    }
    if (b[0] == 6)
    {
        a->family = 6;
        memcpy(a->ip, b + 1, 16);
        return 1;
    }
    return 0;
}

/* ---- Rust's address parser (`core::net::parser`) ---- */

typedef struct
{
    const uint8_t *s;
    size_t n, at;
} parser;

static int p_char(parser *p, uint8_t c)
{
    if (p->at < p->n && p->s[p->at] == c)
    {
        p->at++;
        return 1;
    }
    return 0;
}

static int digit_of(uint8_t c, unsigned radix)
{
    int d;
    if (c >= '0' && c <= '9')
    {
        d = c - '0';
    }
    else if (c >= 'a' && c <= 'z')
    {
        d = c - 'a' + 10;
    }
    else if (c >= 'A' && c <= 'Z')
    {
        d = c - 'A' + 10;
    }
    else
    {
        return -1;
    }
    return (unsigned)d < radix ? d : -1;
}

/* `read_number`: at most `max` digits, a value at most `limit`, a leading
   zero only alone unless `zero_ok`; restores the position on failure. */
static int p_number(parser *p, unsigned radix, size_t max, uint32_t limit, int zero_ok, uint32_t *v)
{
    size_t start = p->at;
    uint32_t r = 0;
    size_t count = 0;
    int lead0 = p->at < p->n && p->s[p->at] == '0';
    while (p->at < p->n)
    {
        int d = digit_of(p->s[p->at], radix);
        if (d < 0)
        {
            break;
        }
        p->at++;
        r = r * radix + (uint32_t)d;
        count++;
        if (r > limit || count > max)
        {
            p->at = start;
            return 0;
        }
    }
    if (count == 0 || (!zero_ok && lead0 && count > 1))
    {
        p->at = start;
        return 0;
    }
    *v = r;
    return 1;
}

static int p_ipv4(parser *p, uint8_t out[4])
{
    size_t start = p->at;
    for (int i = 0; i < 4; i++)
    {
        size_t at = p->at;
        uint32_t v;
        if ((i > 0 && !p_char(p, '.')) || !p_number(p, 10, 3, 255, 0, &v))
        {
            p->at = start;
            (void)at;
            return 0;
        }
        out[i] = (uint8_t)v;
    }
    return 1;
}

/* `read_groups`: colon-separated groups into `g[0..limit)`, an embedded
   IPv4 address allowed where two groups are left; the count, and whether
   it ended with one. */
static size_t p_groups(parser *p, uint16_t *g, size_t limit, int *v4)
{
    *v4 = 0;
    for (size_t i = 0; i < limit; i++)
    {
        if (i + 1 < limit)
        {
            size_t at = p->at;
            uint8_t a4[4];
            if ((i == 0 || p_char(p, ':')) && p_ipv4(p, a4))
            {
                g[i] = (uint16_t)(a4[0] << 8 | a4[1]);
                g[i + 1] = (uint16_t)(a4[2] << 8 | a4[3]);
                *v4 = 1;
                return i + 2;
            }
            p->at = at;
        }
        size_t at = p->at;
        uint32_t v;
        if ((i == 0 || p_char(p, ':')) && p_number(p, 16, 4, 0xFFFF, 1, &v))
        {
            g[i] = (uint16_t)v;
            continue;
        }
        p->at = at;
        return i;
    }
    return limit;
}

static int p_ipv6(parser *p, uint8_t out[16])
{
    size_t start = p->at;
    uint16_t head[8] = {0};
    int v4;
    size_t hn = p_groups(p, head, 8, &v4);
    if (hn < 8)
    {
        /* An IPv4 part is not allowed before `::`. */
        if (v4 || !p_char(p, ':') || !p_char(p, ':'))
        {
            p->at = start;
            return 0;
        }
        uint16_t tail[7] = {0};
        size_t limit = 8 - (hn + 1);
        size_t tn = p_groups(p, tail, limit, &v4);
        memcpy(head + (8 - tn), tail, tn * sizeof(uint16_t));
    }
    for (int i = 0; i < 8; i++)
    {
        out[2 * i] = (uint8_t)(head[i] >> 8);
        out[2 * i + 1] = (uint8_t)head[i];
    }
    return 1;
}

/* `s.parse::<Ipv4Addr>()`, `parse::<Ipv6Addr>()`, `parse::<IpAddr>()`: the
   whole text or nothing. */
static int parse_v4(const uint8_t *s, size_t n, os_addr *a)
{
    parser p = {s, n, 0};
    memset(a, 0, sizeof *a);
    if (p_ipv4(&p, a->ip) && p.at == n)
    {
        a->family = 4;
        return 1;
    }
    return 0;
}

static int parse_v6(const uint8_t *s, size_t n, os_addr *a)
{
    parser p = {s, n, 0};
    memset(a, 0, sizeof *a);
    if (p_ipv6(&p, a->ip) && p.at == n)
    {
        a->family = 6;
        return 1;
    }
    return 0;
}

static int parse_ip(const uint8_t *s, size_t n, os_addr *a)
{
    parser p = {s, n, 0};
    memset(a, 0, sizeof *a);
    if (p_ipv4(&p, a->ip))
    {
        a->family = 4;
        return p.at == n;
    }
    p.at = 0;
    if (p_ipv6(&p, a->ip))
    {
        a->family = 6;
        return p.at == n;
    }
    return 0;
}

static void put_dec(cb_buf *o, unsigned v)
{
    char t[8];
    int k = 0;
    do
    {
        t[k++] = (char)('0' + v % 10);
        v /= 10;
    } while (v);
    while (k)
    {
        cb_buf_byte(o, (uint8_t)t[--k]);
    }
}

static void put_hex(cb_buf *o, unsigned v)
{
    static const char hx[] = "0123456789abcdef";
    char t[8];
    int k = 0;
    do
    {
        t[k++] = hx[v & 15];
        v >>= 4;
    } while (v);
    while (k)
    {
        cb_buf_byte(o, (uint8_t)t[--k]);
    }
}

static void put_v4(cb_buf *o, const uint8_t *ip)
{
    for (int i = 0; i < 4; i++)
    {
        if (i)
        {
            cb_buf_byte(o, '.');
        }
        put_dec(o, ip[i]);
    }
}

/* `Display for IpAddr`: an IPv4-mapped address as `::ffff:a.b.c.d`, else
   the groups in hex with the longest run of two or more zero groups (the
   first of equals) as `::`. */
static void ip_text(const os_addr *a, cb_buf *o)
{
    if (a->family == 4)
    {
        put_v4(o, a->ip);
        return;
    }
    uint16_t g[8];
    for (int i = 0; i < 8; i++)
    {
        g[i] = (uint16_t)(a->ip[2 * i] << 8 | a->ip[2 * i + 1]);
    }
    if (g[0] == 0 && g[1] == 0 && g[2] == 0 && g[3] == 0 && g[4] == 0 && g[5] == 0xFFFF)
    {
        cb_buf_str(o, "::ffff:");
        put_v4(o, a->ip + 12);
        return;
    }
    int best_s = 0, best_l = 0, cur_s = 0, cur_l = 0;
    for (int i = 0; i < 8; i++)
    {
        if (g[i] == 0)
        {
            if (cur_l == 0)
            {
                cur_s = i;
            }
            cur_l++;
            if (cur_l > best_l)
            {
                best_l = cur_l;
                best_s = cur_s;
            }
        }
        else
        {
            cur_l = 0;
        }
    }
    if (best_l > 1)
    {
        for (int i = 0; i < best_s; i++)
        {
            if (i)
            {
                cb_buf_byte(o, ':');
            }
            put_hex(o, g[i]);
        }
        cb_buf_str(o, "::");
        for (int i = best_s + best_l; i < 8; i++)
        {
            if (i > best_s + best_l)
            {
                cb_buf_byte(o, ':');
            }
            put_hex(o, g[i]);
        }
        return;
    }
    for (int i = 0; i < 8; i++)
    {
        if (i)
        {
            cb_buf_byte(o, ':');
        }
        put_hex(o, g[i]);
    }
}

/* ---- the table ---- */

/* What is read from a stream at a time, for `read_line`. */
#define CHUNK 8192

enum
{
    SOCK_LISTENER,
    SOCK_STREAM_,
    SOCK_UDP
};

typedef struct
{
    int refs;
    plat_mutex lock;
    int kind;
    os_sock s;
    uint64_t accept_ms; /* a listener's */
    cb_buf ahead;       /* a stream's */
    size_t at;
} sock;

static plat_mutex table_lock = PLAT_MUTEX_INIT;
static sock **table;
static size_t table_n, table_cap;

static void sock_put(sock *s)
{
    if (__atomic_sub_fetch(&s->refs, 1, __ATOMIC_ACQ_REL) == 0)
    {
        os_sock_close(s->s);
        cb_buf_free(&s->ahead);
        free(s);
    }
}

static int64_t add(int kind, os_sock os)
{
    sock *s = (sock *)cb_xcalloc(1, sizeof(sock));
    s->refs = 1;
    plat_mutex_init(&s->lock);
    s->kind = kind;
    s->s = os;
    plat_mutex_lock(&table_lock);
    size_t i = 0;
    while (i < table_n && table[i])
    {
        i++;
    }
    if (i == table_n)
    {
        if (table_n == table_cap)
        {
            table_cap = table_cap ? table_cap * 2 : 8;
            table = (sock **)cb_xrealloc(table, table_cap * sizeof(sock *));
        }
        table_n++;
    }
    table[i] = s;
    plat_mutex_unlock(&table_lock);
    return (int64_t)i;
}

/* The socket under `h`, locked and held (`done` releases it), or NULL. */
static sock *with(uint64_t h)
{
    plat_mutex_lock(&table_lock);
    sock *s = h < table_n ? table[h] : NULL;
    if (s)
    {
        __atomic_add_fetch(&s->refs, 1, __ATOMIC_ACQ_REL);
    }
    plat_mutex_unlock(&table_lock);
    if (s)
    {
        plat_mutex_lock(&s->lock);
    }
    return s;
}

static void done(sock *s)
{
    plat_mutex_unlock(&s->lock);
    sock_put(s);
}

/* ---- waiting (D-0204) ---- */

/* Whether the calling thread has been asked to stop, as the runtime knows
   (`netio_set_cancel_probe`). */
static int (*cancel_probe)(void);

void netio_set_cancel_probe(int (*f)(void))
{
    if (!__atomic_load_n(&cancel_probe, __ATOMIC_ACQUIRE))
    {
        __atomic_store_n(&cancel_probe, f, __ATOMIC_RELEASE);
    }
}

static int cancelled(void)
{
    int (*f)(void) = __atomic_load_n(&cancel_probe, __ATOMIC_ACQUIRE);
    return f ? f() : 0;
}

/* D-0204: on Linux a socket wait blocks until the socket is ready, its
   deadline passes or a signal (`netio_wake_thread`, sent by `cancel`)
   interrupts it -- no wake-ups to look for a cancellation, which with ten
   thousand idle connections were most of a server's system time.
   Elsewhere it waits in slices, asking between them. */
void netio_wake_init(void)
{
    os_wake_init();
}

/* This OS thread, for `netio_wake_thread`. */
uint64_t netio_os_thread(void)
{
    return os_wake_thread_self();
}

/* Interrupts `t`'s socket wait, if it is in one (a pending wake ends its
   next). The caller makes sure `t` is still running. */
void netio_wake_thread(uint64_t t)
{
    os_wake_thread(t);
}

/* Set by `watch_interrupts` (procio.c) through `netio_watching`. */
static int interrupts_watched;

void netio_watching(void)
{
    __atomic_store_n(&interrupts_watched, 1, __ATOMIC_SEQ_CST);
}

/* Waits until `s` can be read (or written, `!read`), the deadline passes
   (`TIMED_OUT`), an interrupt arrives while watched (`TIMED_OUT`), or the
   thread is asked to stop (`CANCELLED`). In slices, growing from 50 ms to
   a second: a thousand idle connections do not wake the program ten
   thousand times a second. `deadline` is a `plat_now_ns` time, 0 for
   none. */
static int64_t wait_ready(os_sock s, int read, int has_deadline, uint64_t deadline)
{
    int watched = __atomic_load_n(&interrupts_watched, __ATOMIC_SEQ_CST);
    int wake = os_wake_enabled();
    uint64_t slice = 50;
    for (;;)
    {
        if (cancelled())
        {
            return CANCELLED;
        }
        if (watched && procio_interrupt_requested())
        {
            return TIMED_OUT;
        }
        /* Milliseconds to wait; none (`UINT64_MAX`) for no limit. */
        uint64_t ms = wake ? UINT64_MAX : slice;
        if (has_deadline)
        {
            uint64_t now = plat_now_ns();
            if (now >= deadline)
            {
                return TIMED_OUT;
            }
            uint64_t left = (deadline - now) / 1000000u + 1;
            if (left < ms)
            {
                ms = left;
            }
        }
        if (watched && ms > 50)
        {
            ms = 50;
        }
        int msi = ms == UINT64_MAX ? -1 : (int)(ms > 2147483647u ? 2147483647u : ms);
        int ready = 0;
        int k = os_ready(s, read, msi, &ready);
        if (k == OS_OK && ready)
        {
            return 0;
        }
        if (k != OS_OK && k != OS_INTERRUPTED)
        {
            return code(k);
        }
        /* Where no signal wakes a waiter (Windows), the slice is the
           longest a `cancel` waits to be seen: a quarter second there
           (D-0255: a second was over the 3-second bound of
           `conf.thread-cancel-socket` on a loaded Windows 7 machine), a
           second elsewhere. */
        slice *= 2;
        uint64_t cap = os_windows ? 250 : 1000;
        if (slice > cap)
        {
            slice = cap;
        }
    }
}

/* A socket's own timeout, as a deadline from now (`deadline_of`). */
static int deadline_of(os_sock s, int write, uint64_t *deadline)
{
    uint64_t ns;
    if (os_get_timeout(s, write, &ns) != OS_OK || ns == 0)
    {
        return 0;
    }
    *deadline = plat_now_ns() + ns;
    return 1;
}

/* `accept`, waiting at most `ms` (0: for ever), in `wait_ready`'s slices. */
static int64_t accept_on(sock *l, os_sock *out)
{
    int k = os_sock_nonblocking(l->s, 1);
    if (k != OS_OK)
    {
        return code(k);
    }
    uint64_t ms = l->accept_ms;
    uint64_t deadline = ms ? plat_now_ns() + ms * 1000000u : 0;
    for (;;)
    {
        os_sock s;
        k = os_accept(l->s, &s);
        if (k == OS_OK)
        {
            /* Some systems hand on the listener's mode. */
            k = os_sock_nonblocking(s, 0);
            if (k != OS_OK)
            {
                os_sock_close(s);
                return code(k);
            }
            *out = s;
            return 0;
        }
        if (k == OS_WOULD_BLOCK || k == OS_INTERRUPTED)
        {
            int64_t r = wait_ready(l->s, 1, ms != 0, deadline);
            if (r != 0)
            {
                return r;
            }
            continue;
        }
        return code(k);
    }
}

static int64_t read_into(os_sock s, uint8_t *buf, size_t n, size_t *got)
{
    uint64_t dl = 0;
    int has = deadline_of(s, 0, &dl);
    int64_t r = wait_ready(s, 1, has, dl);
    if (r != 0)
    {
        return r;
    }
    for (;;)
    {
        int k = os_recv(s, buf, n, got);
        if (k == OS_OK)
        {
            return 0;
        }
        if (!give_up(k))
        {
            continue;
        }
        return code(k);
    }
}

/* Up to `cap` bytes: what was read ahead first, then one read of the socket
   (as many as have arrived, at least one); none when the peer has closed
   its side. */
static int64_t stream_read(sock *st, size_t cap, cb_buf *out)
{
    if (cap == 0)
    {
        return 0;
    }
    if (st->at < st->ahead.n)
    {
        size_t k = st->ahead.n - st->at;
        if (k > cap)
        {
            k = cap;
        }
        cb_buf_put(out, st->ahead.p + st->at, k);
        st->at += k;
        return (int64_t)k;
    }
    cb_buf_reserve(out, cap);
    size_t got = 0;
    int64_t r = read_into(st->s, out->p, cap, &got);
    if (r != 0)
    {
        return r;
    }
    out->n = got;
    return (int64_t)got;
}

/* The length of the next line, its '\n' included (what is left when the
   peer closes without one); 0 at the end. Its bytes are then read ahead. */
static int64_t stream_line_len(sock *st)
{
    size_t from = st->at;
    for (;;)
    {
        if (from < st->ahead.n)
        {
            const uint8_t *nl = (const uint8_t *)memchr(st->ahead.p + from, '\n', st->ahead.n - from);
            if (nl)
            {
                return (int64_t)((size_t)(nl - st->ahead.p) + 1 - st->at);
            }
        }
        if (st->at > 0)
        {
            memmove(st->ahead.p, st->ahead.p + st->at, st->ahead.n - st->at);
            st->ahead.n -= st->at;
            st->at = 0;
        }
        size_t seen = st->ahead.n;
        size_t old = seen;
        cb_buf_reserve(&st->ahead, CHUNK);
        size_t got = 0;
        int64_t r = read_into(st->s, st->ahead.p + old, CHUNK, &got);
        st->ahead.n = old + (r == 0 ? got : 0);
        if (r != 0)
        {
            return r;
        }
        if (got == 0)
        {
            return (int64_t)st->ahead.n;
        }
        from = seen;
    }
}

static int64_t stream_write_all(os_sock s, const uint8_t *d, size_t n)
{
    uint64_t dl = 0;
    int has = deadline_of(s, 1, &dl);
    while (n > 0)
    {
        int64_t r = wait_ready(s, 0, has, dl);
        if (r != 0)
        {
            return r;
        }
        size_t put = 0;
        int k = os_send(s, d, n, &put);
        if (k == OS_OK)
        {
            if (put == 0)
            {
                return CLOSED;
            }
            d += put;
            n -= put;
            continue;
        }
        if (!give_up(k))
        {
            continue;
        }
        return code(k);
    }
    return 0;
}

/* `std::str::from_utf8`: strict UTF-8. */
static int utf8_ok(const uint8_t *p, size_t n)
{
    size_t i = 0;
    while (i < n)
    {
        uint8_t c = p[i];
        if (c < 0x80)
        {
            i++;
            continue;
        }
        size_t w;
        uint8_t lo = 0x80, hi = 0xBF;
        if (c >= 0xC2 && c <= 0xDF)
        {
            w = 2;
        }
        else if (c >= 0xE0 && c <= 0xEF)
        {
            w = 3;
            lo = c == 0xE0 ? 0xA0 : 0x80;
            hi = c == 0xED ? 0x9F : 0xBF;
        }
        else if (c >= 0xF0 && c <= 0xF4)
        {
            w = 4;
            lo = c == 0xF0 ? 0x90 : 0x80;
            hi = c == 0xF4 ? 0x8F : 0xBF;
        }
        else
        {
            return 0;
        }
        if (n - i < w || p[i + 1] < lo || p[i + 1] > hi)
        {
            return 0;
        }
        for (size_t k = 2; k < w; k++)
        {
            if (p[i + k] < 0x80 || p[i + k] > 0xBF)
            {
                return 0;
            }
        }
        i += w;
    }
    return 1;
}

static int64_t resolve(const uint8_t *host, size_t hn, const uint8_t *port, size_t pn, cb_buf *out)
{
    /* The host as a `&str`. */
    if (!utf8_ok(host, hn))
    {
        return OTHER;
    }
    uint16_t p = (uint16_t)((pn > 0 ? port[0] : 0) | (pn > 1 ? port[1] : 0) << 8);
    /* `(host, port).to_socket_addrs()`: a literal address first. */
    os_addr a;
    if (parse_v4(host, hn, &a) || parse_v6(host, hn, &a))
    {
        a.port = p;
        addr_bytes(&a, out);
        return (int64_t)out->n;
    }
    /* Any failure to resolve is NotFound: the system's reasons (no such
       name, no resolver reachable) do not map onto I/O errors. */
    if (hn && memchr(host, 0, hn))
    {
        return NOT_FOUND;
    }
    char *h = (char *)cb_xmalloc(hn + 1);
    memcpy(h, host, hn);
    h[hn] = 0;
    os_addr *list = NULL;
    size_t count = 0;
    int k = os_resolve(h, p, &list, &count);
    free(h);
    if (k != OS_OK)
    {
        free(list);
        return NOT_FOUND;
    }
    for (size_t j = 0; j < count; j++)
    {
        addr_bytes(&list[j], out);
    }
    free(list);
    if (out->n == 0)
    {
        return NOT_FOUND;
    }
    return (int64_t)out->n;
}

static int64_t netio_op(uint64_t op, uint64_t h, const uint8_t *a, size_t an, const uint8_t *b, size_t bn, uint64_t cap, cb_buf *out)
{
    os_addr ad;
    os_sock os;
    int k;
    int64_t r;
    sock *s;
    switch (op)
    {
    case RESOLVE:
        return resolve(a, an, b, bn, out);
    case TCP_BIND:
        if (!addr_of(a, an, &ad))
        {
            return OTHER;
        }
        k = os_tcp_bind(&ad, &os);
        if (k != OS_OK)
        {
            return code(k);
        }
        return add(SOCK_LISTENER, os);
    case UDP_BIND:
        if (!addr_of(a, an, &ad))
        {
            return OTHER;
        }
        k = os_udp_bind(&ad, &os);
        if (k != OS_OK)
        {
            return code(k);
        }
        return add(SOCK_UDP, os);
    case CONNECT:
        if (!addr_of(a, an, &ad))
        {
            return OTHER;
        }
        k = os_connect(&ad, cap, &os);
        if (k != OS_OK)
        {
            return code(k);
        }
        return add(SOCK_STREAM_, os);
    case CLOSE:
        plat_mutex_lock(&table_lock);
        s = h < table_n ? table[h] : NULL;
        if (s)
        {
            table[h] = NULL;
        }
        plat_mutex_unlock(&table_lock);
        if (!s)
        {
            return OTHER;
        }
        sock_put(s);
        return 0;
    case PARSE_IP:
        if (!utf8_ok(a, an) || !parse_ip(a, an, &ad))
        {
            return OTHER;
        }
        ad.port = 0;
        addr_bytes(&ad, out);
        return (int64_t)out->n;
    case ADDR_TEXT:
        if (!addr_of(a, an, &ad))
        {
            return OTHER;
        }
        ip_text(&ad, out);
        return (int64_t)out->n;
    case LOCAL_ADDR:
    case PEER_ADDR:
    case SET_ACCEPT_TIMEOUT:
    case ACCEPT:
    case SET_READ_TIMEOUT:
    case SET_WRITE_TIMEOUT:
    case SET_NODELAY:
    case READ:
    case LINE_LEN:
    case WRITE:
    case SHUTDOWN_WRITE:
    case SEND_TO:
    case RECV_FROM:
    case SET_KEEPALIVE:
    case TRY_CLONE:
    case UDP_CONNECT:
    case UDP_SEND:
    case UDP_RECV:
        break;
    default:
        return OTHER;
    }
    s = with(h);
    if (!s)
    {
        return OTHER;
    }
    int stream = s->kind == SOCK_STREAM_, udp = s->kind == SOCK_UDP;
    r = OTHER;
    switch (op)
    {
    case LOCAL_ADDR:
    case PEER_ADDR:
        if (op == LOCAL_ADDR)
        {
            k = os_sock_local(s->s, &ad);
        }
        else if (stream)
        {
            k = os_sock_peer(s->s, &ad);
        }
        else
        {
            break;
        }
        if (k != OS_OK)
        {
            r = code(k);
            break;
        }
        addr_bytes(&ad, out);
        r = (int64_t)out->n;
        break;
    case SET_ACCEPT_TIMEOUT:
        if (s->kind == SOCK_LISTENER)
        {
            s->accept_ms = cap;
            r = 0;
        }
        break;
    case ACCEPT:
        if (s->kind == SOCK_LISTENER)
        {
            r = accept_on(s, &os);
            if (r == 0)
            {
                done(s);
                return add(SOCK_STREAM_, os);
            }
        }
        break;
    case SET_READ_TIMEOUT:
        if (stream || udp)
        {
            k = os_set_timeout(s->s, 0, cap);
            r = k == OS_OK ? 0 : code(k);
        }
        break;
    case SET_WRITE_TIMEOUT:
        if (stream)
        {
            k = os_set_timeout(s->s, 1, cap);
            r = k == OS_OK ? 0 : code(k);
        }
        break;
    case SET_NODELAY:
        if (stream)
        {
            k = os_set_nodelay(s->s, cap != 0);
            r = k == OS_OK ? 0 : code(k);
        }
        break;
    case SET_KEEPALIVE:
        if (stream)
        {
            k = os_set_keepalive(s->s, cap != 0);
            r = k == OS_OK ? 0 : OTHER;
        }
        break;
    case READ:
        if (stream)
        {
            r = stream_read(s, (size_t)cap, out);
        }
        break;
    case LINE_LEN:
        if (stream)
        {
            r = stream_line_len(s);
        }
        break;
    case WRITE:
        if (stream)
        {
            r = stream_write_all(s->s, b, bn);
        }
        break;
    case SHUTDOWN_WRITE:
        if (stream)
        {
            k = os_shutdown_write(s->s);
            r = k == OS_OK ? 0 : code(k);
        }
        break;
    case SEND_TO:
        if (udp)
        {
            for (;;)
            {
                if (!addr_of(a, an, &ad))
                {
                    r = OTHER;
                    break;
                }
                size_t put = 0;
                k = os_sendto(s->s, b, bn, &ad, &put);
                if (k == OS_OK)
                {
                    r = (int64_t)put;
                    break;
                }
                if (!give_up(k))
                {
                    continue;
                }
                r = code(k);
                break;
            }
        }
        break;
    case UDP_SEND:
        if (udp)
        {
            for (;;)
            {
                size_t put = 0;
                k = os_send(s->s, b, bn, &put);
                if (k == OS_OK)
                {
                    r = (int64_t)put;
                    break;
                }
                if (!give_up(k))
                {
                    continue;
                }
                r = code(k);
                break;
            }
        }
        break;
    case RECV_FROM:
    case UDP_RECV:
        if (udp)
        {
            /* The answer is the sender's record and then the bytes, and must
               fit `cap`: the datagram's room is what is left after the
               record (a longer datagram's rest is lost, `[Udp]`). D-0255:
               received whole and then cut, because Windows refuses a
               datagram longer than the buffer (`WSAEMSGSIZE`, the sender's
               address lost with it) where Unix cuts it. */
            uint8_t *v = (uint8_t *)cb_xmalloc(DATAGRAM_MAX);
            uint64_t dl = 0;
            int has = deadline_of(s->s, 0, &dl);
            r = wait_ready(s->s, 1, has, dl);
            if (r != 0)
            {
                free(v);
                break;
            }
            size_t got = 0;
            os_addr from;
            for (;;)
            {
                k = op == RECV_FROM ? os_recvfrom(s->s, v, DATAGRAM_MAX, &got, &from) : os_recv(s->s, v, DATAGRAM_MAX, &got);
                if (k == OS_OK || give_up(k))
                {
                    break;
                }
            }
            if (k != OS_OK)
            {
                free(v);
                r = code(k);
                break;
            }
            if (op == RECV_FROM)
            {
                size_t room = cap > ADDR_LEN ? (size_t)cap - ADDR_LEN : 0;
                addr_bytes(&from, out);
                cb_buf_put(out, v, got < room ? got : room);
            }
            else
            {
                cb_buf_put(out, v, got < cap ? got : (size_t)cap);
            }
            free(v);
            r = (int64_t)out->n;
        }
        break;
    case TRY_CLONE:
        if (stream || udp)
        {
            /* D-0205: a UDP socket too, so one thread can wait for
               datagrams while others send on the same socket. */
            int kind = s->kind;
            k = os_sock_dup(s->s, &os);
            if (k != OS_OK)
            {
                r = code(k);
                break;
            }
            done(s);
            return add(kind, os);
        }
        break;
    case UDP_CONNECT:
        if (udp)
        {
            if (!addr_of(a, an, &ad))
            {
                r = OTHER;
                break;
            }
            k = os_udp_connect(s->s, &ad);
            r = k == OS_OK ? 0 : code(k);
        }
        break;
    default:
        break;
    }
    done(s);
    return r;
}

int64_t netio_call(uint64_t op, uint64_t h, const uint8_t *a, size_t an, const uint8_t *b, size_t bn, uint64_t cap, cb_buf *out)
{
    out->n = 0;
    int64_t r = netio_op(op, h, a, an, b, bn, cap, out);
    if (r < 0)
    {
        out->n = 0;
    }
    return r;
}
