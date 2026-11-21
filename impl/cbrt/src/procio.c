/* Child processes and interrupt requests (`spec/21` §2k
   `rule.stdlib.process`, D-0143).

   `std::proc_op(op, h, a, b, out, cap)` is the one primitive. A command
   arrives in `a` as `std` encodes it (`decode`); `b` is bytes for the
   child's standard input. No shell is involved: the program is found as
   the system finds programs (a name without a separator on `PATH`) and its
   arguments reach it exactly as given.

   A running child is an entry of a process-wide table, shared by every
   thread; its handle is its index. Each entry is locked on its own, and the
   table only long enough to find it, so one thread waiting for its child's
   output does not stop another starting a child or waiting for its own
   (Rust's `Arc<Mutex<Entry>>`: an entry dropped from the table while a
   call uses it lives until that call ends). Finished runs (`OUTPUT`) wait
   in a second table until `TAKE` copies them out.

   Failures are codes: -1 the program was not found, -2 permission was
   denied, -3 anything else. */

#include "io.h"
#include "plat.h"
#include "plat_io.h"

#define NOT_FOUND (-1)
#define DENIED (-2)
#define OTHER (-3)

#define OUTPUT 0
#define TAKE 1
#define STATUS 2
#define SPAWN 3
#define CHILD_ID 4
#define WRITE_INPUT 5
#define CLOSE_INPUT 6
#define READ_OUTPUT 7
#define OUTPUT_LINE_LEN 8
#define READ_ERROR 9
#define WAIT 10
#define TRY_WAIT 11
#define KILL 12
#define DROP 13
#define WATCH_INTERRUPTS 14
#define INTERRUPT_REQUESTED 15
#define PROCESS_ID 16     /* D-0181 */
#define TERMINATE 17      /* D-0181 */
#define ERROR_LINE_LEN 18 /* D-0181 */
#define CPU_COUNT 19      /* D-0185 */
/* D-0195: a child's standard input or output taken out as a pipe of its own
   (`h` the child for the TAKE_*s, the pipe for the PIPE_*s). */
#define TAKE_INPUT 20
#define TAKE_OUTPUT 21
#define PIPE_WRITE 22
#define PIPE_READ 23
#define PIPE_LINE_LEN 24
#define PIPE_DROP 25
#define KILL_TREE 26 /* D-0205 */

static int64_t code(int kind)
{
    switch (kind)
    {
    case OS_NOT_FOUND:
        return NOT_FOUND;
    case OS_DENIED:
        return DENIED;
    case OS_IS_DIR:
        return FILEIO_IS_DIR;
    case OS_NOT_DIR:
        return FILEIO_NOT_DIR;
    case OS_NO_SPACE:
        return FILEIO_NO_SPACE;
    case OS_TOO_MANY_OPEN:
        return FILEIO_TOO_MANY_OPEN;
    default:
        return OTHER;
    }
}

/* ---- the command (`Spec`) ----

   The command `std` sends: strings as an 8-byte little-endian length and
   the bytes. First the program and its arguments, as a count and the
   strings; then a byte, 1 to clear the environment; a byte, 1 when a
   directory follows, and the directory; then a count of variables, and
   each name and value. */

typedef struct
{
    const uint8_t *p;
    size_t n;
} bstr;

typedef struct
{
    size_t argc, argcap;
    bstr *argv;
    int clear_env;
    int has_dir;
    bstr dir;
    size_t nenv, envcap;
    bstr *env; /* name, value, name, value, ... */
} spec;

typedef struct
{
    const uint8_t *b;
    size_t n, at;
} reader;

static int rd_n(reader *r, uint64_t *v)
{
    if (r->n - r->at < 8)
    {
        return 0;
    }
    uint64_t x = 0;
    for (int i = 7; i >= 0; i--)
    {
        x = x << 8 | r->b[r->at + (size_t)i];
    }
    r->at += 8;
    *v = x;
    return 1;
}

static int rd_byte(reader *r, uint8_t *v)
{
    if (r->at >= r->n)
    {
        return 0;
    }
    *v = r->b[r->at++];
    return 1;
}

/* `String::from_utf8`: strict UTF-8. */
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
        if (n - i < w)
        {
            return 0;
        }
        if (p[i + 1] < lo || p[i + 1] > hi)
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

static int rd_text(reader *r, bstr *s)
{
    uint64_t n;
    if (!rd_n(r, &n))
    {
        return 0;
    }
    if (n > r->n - r->at)
    {
        return 0;
    }
    s->p = r->b + r->at;
    s->n = (size_t)n;
    r->at += (size_t)n;
    return utf8_ok(s->p, s->n);
}

static void spec_free(spec *s)
{
    free(s->argv);
    free(s->env);
}

static int64_t decode(const uint8_t *a, size_t an, spec *s)
{
    memset(s, 0, sizeof *s);
    reader r = {a, an, 0};
    uint64_t n;
    if (!rd_n(&r, &n))
    {
        return OTHER;
    }
    for (uint64_t i = 0; i < n; i++)
    {
        bstr t;
        if (!rd_text(&r, &t))
        {
            spec_free(s);
            return OTHER;
        }
        if (s->argc == s->argcap)
        {
            s->argcap = s->argcap ? s->argcap * 2 : 8;
            s->argv = (bstr *)cb_xrealloc(s->argv, s->argcap * sizeof(bstr));
        }
        s->argv[s->argc++] = t;
    }
    uint8_t b;
    if (!rd_byte(&r, &b))
    {
        spec_free(s);
        return OTHER;
    }
    s->clear_env = b == 1;
    if (!rd_byte(&r, &b))
    {
        spec_free(s);
        return OTHER;
    }
    if (b == 1)
    {
        if (!rd_text(&r, &s->dir))
        {
            spec_free(s);
            return OTHER;
        }
        s->has_dir = 1;
    }
    uint64_t k;
    if (!rd_n(&r, &k))
    {
        spec_free(s);
        return OTHER;
    }
    for (uint64_t i = 0; i < k; i++)
    {
        bstr name, value;
        if (!rd_text(&r, &name) || !rd_text(&r, &value))
        {
            spec_free(s);
            return OTHER;
        }
        if (s->nenv == s->envcap)
        {
            s->envcap = s->envcap ? s->envcap * 2 : 8;
            s->env = (bstr *)cb_xrealloc(s->env, s->envcap * 2 * sizeof(bstr));
        }
        s->env[2 * s->nenv] = name;
        s->env[2 * s->nenv + 1] = value;
        s->nenv++;
    }
    if (s->argc == 0)
    {
        spec_free(s);
        return OTHER;
    }
    return 0;
}

/* `Command::new(argv[0]).args(..)` and so on, as the platform's `os_cmd`;
   the arrays live in `store`. D-0201: a child sees every byte this program
   wrote to its files. */
typedef struct
{
    os_cmd c;
    const uint8_t **argv;
    size_t *argl;
    const uint8_t **ek, **ev;
    size_t *ekn, *evn;
} command_t;

static void command(const spec *s, command_t *cm)
{
    fileio_flush_all();
    size_t na = s->argc ? s->argc : 1, ne = s->nenv ? s->nenv : 1;
    cm->argv = (const uint8_t **)cb_xmalloc(na * sizeof(uint8_t *));
    cm->argl = (size_t *)cb_xmalloc(na * sizeof(size_t));
    cm->ek = (const uint8_t **)cb_xmalloc(ne * sizeof(uint8_t *));
    cm->ev = (const uint8_t **)cb_xmalloc(ne * sizeof(uint8_t *));
    cm->ekn = (size_t *)cb_xmalloc(ne * sizeof(size_t));
    cm->evn = (size_t *)cb_xmalloc(ne * sizeof(size_t));
    for (size_t i = 0; i < s->argc; i++)
    {
        cm->argv[i] = s->argv[i].p;
        cm->argl[i] = s->argv[i].n;
    }
    for (size_t i = 0; i < s->nenv; i++)
    {
        cm->ek[i] = s->env[2 * i].p;
        cm->ekn[i] = s->env[2 * i].n;
        cm->ev[i] = s->env[2 * i + 1].p;
        cm->evn[i] = s->env[2 * i + 1].n;
    }
    cm->c.argc = s->argc;
    cm->c.argv = cm->argv;
    cm->c.argl = cm->argl;
    cm->c.clear_env = s->clear_env;
    cm->c.has_dir = s->has_dir;
    cm->c.dir = s->dir.p;
    cm->c.dirn = s->dir.n;
    cm->c.nenv = s->nenv;
    cm->c.env_k = cm->ek;
    cm->c.env_kn = cm->ekn;
    cm->c.env_v = cm->ev;
    cm->c.env_vn = cm->evn;
}

static void command_free(command_t *cm)
{
    free(cm->argv);
    free(cm->argl);
    free(cm->ek);
    free(cm->ev);
    free(cm->ekn);
    free(cm->evn);
}

/* A spawn's failure: a program that is not there is NotFound, whether or
   not a working directory given for it exists (the system says NotFound for
   both; a missing directory is Io here, checked first). */
static int64_t spawn_error(const spec *s, int kind)
{
    if (s->has_dir && !os_is_dir(s->dir.p, s->dir.n))
    {
        return OTHER;
    }
    return code(kind);
}

/* ---- runs to the end (`Command::output`, `Command::status`) ---- */

static plat_mutex done_lock = PLAT_MUTEX_INIT;
static cb_buf **done;
static size_t done_n, done_cap;

static int64_t keep(cb_buf *v)
{
    cb_buf *b = (cb_buf *)cb_xmalloc(sizeof(cb_buf));
    *b = *v;
    plat_mutex_lock(&done_lock);
    size_t i = 0;
    while (i < done_n && done[i])
    {
        i++;
    }
    if (i == done_n)
    {
        if (done_n == done_cap)
        {
            done_cap = done_cap ? done_cap * 2 : 8;
            done = (cb_buf **)cb_xrealloc(done, done_cap * sizeof(cb_buf *));
        }
        done_n++;
    }
    done[i] = b;
    plat_mutex_unlock(&done_lock);
    return (int64_t)i;
}

typedef struct
{
    os_pipe p;
    const uint8_t *d;
    size_t n;
} writer_job;

/* Written from its own thread, while the caller drains both output pipes,
   so a child that answers before reading all of its input cannot deadlock
   against us. A child that stops reading early breaks the pipe; that is
   not this run's failure. The pipe is closed at the end (`stdin`
   dropped). */
static void writer(void *arg)
{
    writer_job *j = (writer_job *)arg;
    const uint8_t *p = j->d;
    size_t n = j->n;
    while (n > 0)
    {
        size_t put = 0;
        int k = os_pipe_write(j->p, p, n, &put);
        if (k == OS_INTERRUPTED)
        {
            continue;
        }
        if (k != OS_OK || put == 0)
        {
            break;
        }
        p += put;
        n -= put;
    }
    os_pipe_close(j->p);
}

static void put_le(cb_buf *v, uint64_t x, int bytes)
{
    for (int i = 0; i < bytes; i++)
    {
        cb_buf_byte(v, (uint8_t)(x >> (8 * i)));
    }
}

/* The record `TAKE` hands over: the status (4 bytes), the standard
   output's length (8 bytes), its bytes, then the standard error's bytes. */
static int64_t run_output(const spec *s, const uint8_t *input, size_t inn, cb_buf *rec)
{
    command_t cm;
    command(s, &cm);
    os_child *child;
    os_pipe in, out, err;
    int k = os_spawn(&cm.c, inn == 0 ? OS_STDIO_NULL : OS_STDIO_PIPED, OS_STDIO_PIPED, OS_STDIO_PIPED, &child, &in, &out, &err);
    command_free(&cm);
    if (k != OS_OK)
    {
        return spawn_error(s, k);
    }
    plat_thread t;
    int has_writer = 0;
    uint8_t *copy = NULL;
    writer_job job;
    if (inn > 0)
    {
        copy = (uint8_t *)cb_xmalloc(inn);
        memcpy(copy, input, inn);
        job.p = in;
        job.d = copy;
        job.n = inn;
        if (plat_thread_spawn(&t, writer, &job, 0) == 0)
        {
            has_writer = 1;
        }
        else
        {
            cbrt_internal("failed to spawn thread");
        }
    }
    cb_buf vo = {0}, ve = {0};
    k = os_read2(out, &vo, err, &ve);
    os_pipe_close(out);
    os_pipe_close(err);
    int status = 0;
    int64_t r = 0;
    if (k != OS_OK)
    {
        /* Rust unwraps `read2`'s result: a read error there is a panic. */
        cbrt_internal("reading a child's output failed");
    }
    k = os_child_wait(child, &status);
    if (k != OS_OK)
    {
        r = inn == 0 ? spawn_error(s, k) : code(k);
    }
    if (has_writer)
    {
        plat_thread_join(t);
    }
    free(copy);
    os_child_free(child);
    if (r == 0)
    {
        rec->n = 0;
        cb_buf_reserve(rec, 12 + vo.n + ve.n);
        put_le(rec, (uint32_t)status, 4);
        put_le(rec, vo.n, 8);
        cb_buf_put(rec, vo.p, vo.n);
        cb_buf_put(rec, ve.p, ve.n);
    }
    cb_buf_free(&vo);
    cb_buf_free(&ve);
    return r;
}

/* ---- children with pipes (`Command::spawn`, `Child`) ---- */

/* What is read from a pipe at a time. */
#define CHUNK 8192

typedef struct
{
    os_child *child;
    os_pipe in, out, err;
    /* Standard output read ahead for `read_output_line`, and how much of
       it is taken; the same for standard error (`read_error_line`). */
    cb_buf ahead;
    size_t at;
    cb_buf ahead_err;
    size_t at_err;
    int has_status;
    int status;
} entry;

typedef struct
{
    int refs;
    plat_mutex lock;
    entry e;
} slot;

static plat_mutex table_lock = PLAT_MUTEX_INIT;
static slot **table;
static size_t table_n, table_cap;

static void entry_drop(entry *e)
{
    os_pipe_close(e->in);
    os_pipe_close(e->out);
    os_pipe_close(e->err);
    os_child_free(e->child);
    cb_buf_free(&e->ahead);
    cb_buf_free(&e->ahead_err);
}

static void slot_put(slot *s)
{
    if (__atomic_sub_fetch(&s->refs, 1, __ATOMIC_ACQ_REL) == 0)
    {
        entry_drop(&s->e);
        free(s);
    }
}

static slot *slot_get(uint64_t h)
{
    plat_mutex_lock(&table_lock);
    slot *s = h < table_n ? table[h] : NULL;
    if (s)
    {
        __atomic_add_fetch(&s->refs, 1, __ATOMIC_ACQ_REL);
    }
    plat_mutex_unlock(&table_lock);
    return s;
}

static int64_t add_slot(slot ***t, size_t *tn, size_t *tc, void *s)
{
    size_t i = 0;
    while (i < *tn && (*t)[i])
    {
        i++;
    }
    if (i == *tn)
    {
        if (*tn == *tc)
        {
            *tc = *tc ? *tc * 2 : 8;
            *t = (slot **)cb_xrealloc(*t, *tc * sizeof(slot *));
        }
        (*tn)++;
    }
    (*t)[i] = (slot *)s;
    return (int64_t)i;
}

static int64_t spawn(const spec *s)
{
    command_t cm;
    command(s, &cm);
    os_child *child;
    os_pipe in, out, err;
    int k = os_spawn(&cm.c, OS_STDIO_PIPED, OS_STDIO_PIPED, OS_STDIO_PIPED, &child, &in, &out, &err);
    command_free(&cm);
    if (k != OS_OK)
    {
        return spawn_error(s, k);
    }
    slot *sl = (slot *)cb_xcalloc(1, sizeof(slot));
    sl->refs = 1;
    plat_mutex_init(&sl->lock);
    sl->e.child = child;
    sl->e.in = in;
    sl->e.out = out;
    sl->e.err = err;
    plat_mutex_lock(&table_lock);
    int64_t i = add_slot(&table, &table_n, &table_cap, sl);
    plat_mutex_unlock(&table_lock);
    return i;
}

/* ---- pipes taken out of a child (D-0195) ----

   One end, owned by a `ChildInput` or `ChildOutput`: each has its own lock,
   so a thread writing one child's input and another reading a second
   child's output never wait for each other, nor for the `Child`s. */
typedef struct
{
    int refs;
    plat_mutex lock;
    int is_in;
    os_pipe p;
    cb_buf ahead;
    size_t at;
} pipe_slot;

static plat_mutex pipes_lock = PLAT_MUTEX_INIT;
static slot **pipes;
static size_t pipes_n, pipes_cap;

static void pipe_put(pipe_slot *s)
{
    if (__atomic_sub_fetch(&s->refs, 1, __ATOMIC_ACQ_REL) == 0)
    {
        os_pipe_close(s->p);
        cb_buf_free(&s->ahead);
        free(s);
    }
}

static int64_t add_pipe(int is_in, os_pipe p, cb_buf ahead, size_t at)
{
    pipe_slot *s = (pipe_slot *)cb_xcalloc(1, sizeof(pipe_slot));
    s->refs = 1;
    plat_mutex_init(&s->lock);
    s->is_in = is_in;
    s->p = p;
    s->ahead = ahead;
    s->at = at;
    plat_mutex_lock(&pipes_lock);
    int64_t i = add_slot(&pipes, &pipes_n, &pipes_cap, s);
    plat_mutex_unlock(&pipes_lock);
    return i;
}

static pipe_slot *pipe_get(uint64_t h)
{
    plat_mutex_lock(&pipes_lock);
    pipe_slot *s = h < pipes_n ? (pipe_slot *)pipes[h] : NULL;
    if (s)
    {
        __atomic_add_fetch(&s->refs, 1, __ATOMIC_ACQ_REL);
    }
    plat_mutex_unlock(&pipes_lock);
    return s;
}

static int64_t read_pipe(os_pipe p, uint8_t *buf, size_t n, size_t *got)
{
    for (;;)
    {
        int k = os_pipe_read(p, buf, n, got);
        if (k == OS_INTERRUPTED)
        {
            continue;
        }
        return k == OS_OK ? 0 : code(k);
    }
}

/* Up to `cap` bytes of a pipe: what was read ahead (`ahead`, taken to
   `at`) first, then one read of the pipe; none at its end. */
static int64_t read_ahead_pipe(os_pipe pipe, cb_buf *ahead, size_t *at, size_t cap, cb_buf *out)
{
    out->n = 0;
    if (cap == 0)
    {
        return 0;
    }
    if (*at < ahead->n)
    {
        size_t k = ahead->n - *at;
        if (k > cap)
        {
            k = cap;
        }
        cb_buf_put(out, ahead->p + *at, k);
        *at += k;
        return (int64_t)k;
    }
    if (pipe == OS_PIPE_NONE)
    {
        return 0;
    }
    cb_buf_reserve(out, cap);
    size_t got = 0;
    int64_t r = read_pipe(pipe, out->p, cap, &got);
    if (r != 0)
    {
        return r;
    }
    out->n = got;
    return (int64_t)got;
}

/* The length of a pipe's next line, its '\n' included (the rest when there
   is none); 0 at the end. Its bytes are then read ahead, and
   `read_ahead_pipe` takes them. */
static int64_t pipe_line_len(os_pipe pipe, cb_buf *ahead, size_t *at)
{
    size_t from = *at;
    for (;;)
    {
        if (from < ahead->n)
        {
            const uint8_t *nl = (const uint8_t *)memchr(ahead->p + from, '\n', ahead->n - from);
            if (nl)
            {
                return (int64_t)((size_t)(nl - ahead->p) + 1 - *at);
            }
        }
        if (*at > 0)
        {
            memmove(ahead->p, ahead->p + *at, ahead->n - *at);
            ahead->n -= *at;
            *at = 0;
        }
        size_t seen = ahead->n;
        if (pipe == OS_PIPE_NONE)
        {
            return (int64_t)seen;
        }
        size_t old = ahead->n;
        cb_buf_reserve(ahead, CHUNK);
        size_t got = 0;
        int64_t r = read_pipe(pipe, ahead->p + old, CHUNK, &got);
        ahead->n = old + (r == 0 ? got : 0);
        if (r != 0)
        {
            return r;
        }
        if (got == 0)
        {
            return (int64_t)ahead->n;
        }
        from = seen;
    }
}

static int64_t pipe_write_all(os_pipe p, const uint8_t *d, size_t n)
{
    while (n > 0)
    {
        size_t put = 0;
        int k = os_pipe_write(p, d, n, &put);
        if (k == OS_INTERRUPTED)
        {
            continue;
        }
        if (k != OS_OK)
        {
            return code(k);
        }
        if (put == 0)
        {
            return OTHER;
        }
        d += put;
        n -= put;
    }
    return 0;
}

static int64_t wait_entry(entry *e, int *st)
{
    if (e->has_status)
    {
        *st = e->status;
        return 0;
    }
    /* Standard input closed first, so a child reading to its end ends. */
    os_pipe_close(e->in);
    e->in = OS_PIPE_NONE;
    int k = os_child_wait(e->child, st);
    if (k != OS_OK)
    {
        return code(k);
    }
    e->has_status = 1;
    e->status = *st;
    return 0;
}

/* 1 when finished (status in `*st`), 0 when running, or a code. */
static int64_t try_wait_entry(entry *e, int *st)
{
    if (e->has_status)
    {
        *st = e->status;
        return 1;
    }
    int done_ = 0;
    int k = os_child_try_wait(e->child, &done_, st);
    if (k != OS_OK)
    {
        return code(k);
    }
    if (done_)
    {
        e->has_status = 1;
        e->status = *st;
        return 1;
    }
    return 0;
}

/* The child's pipes closed and its entry gone; a finished child is reaped.
   A running one keeps running, as Rust's `Child` does when dropped. */
static int64_t drop_child(uint64_t h)
{
    slot *s;
    plat_mutex_lock(&table_lock);
    if (h < table_n && table[h])
    {
        s = table[h];
        table[h] = NULL;
    }
    else
    {
        plat_mutex_unlock(&table_lock);
        return OTHER;
    }
    plat_mutex_unlock(&table_lock);
    plat_mutex_lock(&s->lock);
    os_pipe_close(s->e.in);
    os_pipe_close(s->e.out);
    os_pipe_close(s->e.err);
    s->e.in = s->e.out = s->e.err = OS_PIPE_NONE;
    int done_, st;
    (void)os_child_try_wait(s->e.child, &done_, &st);
    plat_mutex_unlock(&s->lock);
    slot_put(s);
    return 0;
}

/* ---- interrupt requests ---- */

static int interrupted;

void procio_ignore_sigpipe(void)
{
    os_ignore_sigpipe();
}

int procio_interrupt_requested(void)
{
    return __atomic_load_n(&interrupted, __ATOMIC_SEQ_CST) != 0;
}

/* ---- the primitive ---- */

/* A status as an answer: its 4 bytes, little-endian, and their count. */
static int64_t status_bytes(int s, cb_buf *out)
{
    out->n = 0;
    put_le(out, (uint32_t)s, 4);
    return 4;
}

int64_t procio_call(uint64_t op, uint64_t h, const uint8_t *a, size_t an, const uint8_t *b, size_t bn, uint64_t cap, cb_buf *out)
{
    out->n = 0;
    int64_t r;
    spec s;
    switch (op)
    {
    case OUTPUT:
    {
        r = decode(a, an, &s);
        if (r != 0)
        {
            return r;
        }
        cb_buf rec = {0};
        r = run_output(&s, b, bn, &rec);
        spec_free(&s);
        if (r != 0)
        {
            cb_buf_free(&rec);
            return r;
        }
        return keep(&rec);
    }
    case TAKE:
    {
        /* The kept record's length; its bytes when they fit in `cap`, and
           then it is gone. */
        plat_mutex_lock(&done_lock);
        if (h >= done_n || !done[h])
        {
            plat_mutex_unlock(&done_lock);
            return OTHER;
        }
        cb_buf *v = done[h];
        int64_t n = (int64_t)v->n;
        if ((uint64_t)n <= cap)
        {
            done[h] = NULL;
            plat_mutex_unlock(&done_lock);
            cb_buf_put(out, v->p, v->n);
            cb_buf_free(v);
            free(v);
            return n;
        }
        plat_mutex_unlock(&done_lock);
        return n;
    }
    case STATUS:
    {
        r = decode(a, an, &s);
        if (r != 0)
        {
            return r;
        }
        /* The child writes to this program's standard output, after what
           the program has written so far (D-0178). */
        (void)outbuf_flush();
        command_t cm;
        command(&s, &cm);
        os_child *child;
        os_pipe in, o, e;
        int k = os_spawn(&cm.c, OS_STDIO_INHERIT, OS_STDIO_INHERIT, OS_STDIO_INHERIT, &child, &in, &o, &e);
        command_free(&cm);
        if (k != OS_OK)
        {
            r = spawn_error(&s, k);
            spec_free(&s);
            return r;
        }
        int st;
        k = os_child_wait(child, &st);
        os_child_free(child);
        if (k != OS_OK)
        {
            r = spawn_error(&s, k);
            spec_free(&s);
            return r;
        }
        spec_free(&s);
        return status_bytes(st, out);
    }
    case SPAWN:
        r = decode(a, an, &s);
        if (r != 0)
        {
            return r;
        }
        r = spawn(&s);
        spec_free(&s);
        return r;
    case WATCH_INTERRUPTS:
        os_watch_interrupts(&interrupted);
        netio_watching();
        return 0;
    case INTERRUPT_REQUESTED:
        return procio_interrupt_requested();
    case PROCESS_ID:
        return (int64_t)os_process_id();
    case CPU_COUNT:
        return (int64_t)plat_cpu_count();
    case PIPE_WRITE:
    case PIPE_READ:
    case PIPE_LINE_LEN:
    {
        pipe_slot *p = pipe_get(h);
        if (!p)
        {
            return OTHER;
        }
        plat_mutex_lock(&p->lock);
        if (op == PIPE_WRITE)
        {
            r = p->is_in ? pipe_write_all(p->p, b, bn) : OTHER;
        }
        else if (op == PIPE_READ)
        {
            r = !p->is_in ? read_ahead_pipe(p->p, &p->ahead, &p->at, (size_t)cap, out) : OTHER;
        }
        else
        {
            r = !p->is_in ? pipe_line_len(p->p, &p->ahead, &p->at) : OTHER;
        }
        plat_mutex_unlock(&p->lock);
        pipe_put(p);
        if (r < 0)
        {
            out->n = 0;
        }
        return r;
    }
    case PIPE_DROP:
    {
        pipe_slot *p = NULL;
        plat_mutex_lock(&pipes_lock);
        if (h < pipes_n)
        {
            p = (pipe_slot *)pipes[h];
            pipes[h] = NULL;
        }
        plat_mutex_unlock(&pipes_lock);
        if (p)
        {
            pipe_put(p);
        }
        return 0;
    }
    case DROP:
        return drop_child(h);
    default:
        break;
    }
    /* The operations on one child, under its entry's lock. */
    switch (op)
    {
    case CHILD_ID:
    case WRITE_INPUT:
    case CLOSE_INPUT:
    case READ_OUTPUT:
    case OUTPUT_LINE_LEN:
    case READ_ERROR:
    case WAIT:
    case TRY_WAIT:
    case KILL:
    case KILL_TREE:
    case TERMINATE:
    case ERROR_LINE_LEN:
    case TAKE_INPUT:
    case TAKE_OUTPUT:
        break;
    default:
        return OTHER;
    }
    slot *sl = slot_get(h);
    if (!sl)
    {
        return OTHER;
    }
    plat_mutex_lock(&sl->lock);
    entry *e = &sl->e;
    int st;
    os_pipe taken = OS_PIPE_NONE;
    cb_buf taken_ahead = {0};
    size_t taken_at = 0;
    switch (op)
    {
    case CHILD_ID:
        r = (int64_t)os_child_id(e->child);
        break;
    case WRITE_INPUT:
        r = e->in != OS_PIPE_NONE ? pipe_write_all(e->in, b, bn) : OTHER;
        break;
    case CLOSE_INPUT:
        os_pipe_close(e->in);
        e->in = OS_PIPE_NONE;
        r = 0;
        break;
    case READ_OUTPUT:
        r = read_ahead_pipe(e->out, &e->ahead, &e->at, (size_t)cap, out);
        break;
    case OUTPUT_LINE_LEN:
        r = pipe_line_len(e->out, &e->ahead, &e->at);
        break;
    case READ_ERROR:
        r = read_ahead_pipe(e->err, &e->ahead_err, &e->at_err, (size_t)cap, out);
        break;
    case ERROR_LINE_LEN:
        r = pipe_line_len(e->err, &e->ahead_err, &e->at_err);
        break;
    case WAIT:
        r = wait_entry(e, &st);
        if (r == 0)
        {
            r = status_bytes(st, out);
        }
        break;
    case TRY_WAIT:
        r = try_wait_entry(e, &st);
        if (r == 1)
        {
            status_bytes(st, out);
        }
        break;
    case KILL:
    case KILL_TREE:
        if (e->has_status)
        {
            r = 0;
            break;
        }
        {
            int k = op == KILL ? os_child_kill(e->child) : os_child_kill_tree(e->child);
            r = k == OS_OK ? 0 : code(k);
        }
        break;
    case TERMINATE:
        if (e->has_status)
        {
            r = 0;
            break;
        }
        {
            int k = os_child_terminate(e->child);
            r = k == OS_OK ? 0 : code(k);
        }
        break;
    case TAKE_INPUT:
        taken = e->in;
        e->in = OS_PIPE_NONE;
        r = 0;
        break;
    case TAKE_OUTPUT:
        taken = e->out;
        e->out = OS_PIPE_NONE;
        if (taken != OS_PIPE_NONE)
        {
            taken_ahead = e->ahead;
            taken_at = e->at;
            memset(&e->ahead, 0, sizeof e->ahead);
            e->at = 0;
        }
        r = 0;
        break;
    default:
        r = OTHER;
    }
    plat_mutex_unlock(&sl->lock);
    slot_put(sl);
    if (op == TAKE_INPUT || op == TAKE_OUTPUT)
    {
        /* The pipe's number, or NOT_FOUND when it was taken or closed. */
        if (taken == OS_PIPE_NONE)
        {
            return NOT_FOUND;
        }
        return add_pipe(op == TAKE_INPUT, taken, taken_ahead, taken_at);
    }
    if (r < 0)
    {
        out->n = 0;
    }
    return r;
}
