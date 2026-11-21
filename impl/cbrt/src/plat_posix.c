/* The runtime's platform layer on POSIX (glibc on Linux): plat.h's locks,
   threads, clocks and process, and plat_io.h's files, child processes,
   sockets and random bytes, each as Rust's standard library does it on
   Linux (its release 1.98): every descriptor opened close-on-exec, EINTR
   retried where `cvt_r` retries it and reported where `cvt` does, errors
   mapped onto the kinds of `decode_error_kind`. The whole file is empty on
   Windows (plat_win.c). */

#ifndef _WIN32

#define _GNU_SOURCE
#include "plat.h"
#include "plat_io.h"

#include <arpa/inet.h>
#include <dirent.h>
#include <errno.h>
#include <fcntl.h>
#include <limits.h>
#include <netdb.h>
#include <netinet/in.h>
#include <netinet/tcp.h>
#include <poll.h>
#include <sched.h>
#include <signal.h>
#include <stdio.h>
#include <sys/ioctl.h>
#include <sys/socket.h>
#include <sys/stat.h>
#include <sys/syscall.h>
#include <sys/types.h>
#include <sys/wait.h>
#include <time.h>
#include <unistd.h>

extern char **environ;

const int os_windows = 0;

/* ---- locks ---- */

void plat_mutex_init(plat_mutex *m)
{
    pthread_mutex_init(&m->m, NULL);
}

void plat_mutex_lock(plat_mutex *m)
{
    pthread_mutex_lock(&m->m);
}

int plat_mutex_trylock(plat_mutex *m)
{
    return pthread_mutex_trylock(&m->m) == 0;
}

void plat_mutex_unlock(plat_mutex *m)
{
    pthread_mutex_unlock(&m->m);
}

void plat_cond_init(plat_cond *c)
{
    pthread_cond_init(&c->c, NULL);
}

void plat_cond_wait(plat_cond *c, plat_mutex *m)
{
    pthread_cond_wait(&c->c, &m->m);
}

int plat_cond_wait_ns(plat_cond *c, plat_mutex *m, uint64_t ns)
{
    /* A statically initialized condition variable waits on the real-time
       clock. */
    struct timespec ts;
    clock_gettime(CLOCK_REALTIME, &ts);
    uint64_t s = ns / 1000000000u;
    uint64_t rest = ns % 1000000000u;
    if (s > (uint64_t)INT64_MAX / 2)
    {
        s = (uint64_t)INT64_MAX / 2;
    }
    ts.tv_sec += (time_t)s;
    ts.tv_nsec += (long)rest;
    if (ts.tv_nsec >= 1000000000L)
    {
        ts.tv_nsec -= 1000000000L;
        ts.tv_sec++;
    }
    return pthread_cond_timedwait(&c->c, &m->m, &ts) != ETIMEDOUT;
}

void plat_cond_signal(plat_cond *c)
{
    pthread_cond_signal(&c->c);
}

void plat_cond_broadcast(plat_cond *c)
{
    pthread_cond_broadcast(&c->c);
}

void plat_rw_read(plat_rwlock *l)
{
    pthread_rwlock_rdlock(&l->l);
}

int plat_rw_try_read(plat_rwlock *l)
{
    return pthread_rwlock_tryrdlock(&l->l) == 0;
}

void plat_rw_read_unlock(plat_rwlock *l)
{
    pthread_rwlock_unlock(&l->l);
}

void plat_rw_write(plat_rwlock *l)
{
    pthread_rwlock_wrlock(&l->l);
}

int plat_rw_try_write(plat_rwlock *l)
{
    return pthread_rwlock_trywrlock(&l->l) == 0;
}

void plat_rw_write_unlock(plat_rwlock *l)
{
    pthread_rwlock_unlock(&l->l);
}

/* ---- threads ---- */

typedef struct
{
    void (*fn)(void *);
    void *arg;
} thread_start;

static void *thread_main(void *p)
{
    thread_start s = *(thread_start *)p;
    free(p);
    s.fn(s.arg);
    return NULL;
}

int plat_thread_spawn(plat_thread *t, void (*fn)(void *), void *arg, size_t stack)
{
    pthread_attr_t attr;
    if (pthread_attr_init(&attr) != 0)
    {
        return -1;
    }
    if (stack)
    {
        /* As Rust's `Builder::stack_size`: at least the minimum, a whole
           number of pages. */
        size_t page = (size_t)sysconf(_SC_PAGESIZE);
        if (stack < (size_t)PTHREAD_STACK_MIN)
        {
            stack = (size_t)PTHREAD_STACK_MIN;
        }
        stack = (stack + page - 1) & ~(page - 1);
        pthread_attr_setstacksize(&attr, stack);
    }
    thread_start *s = (thread_start *)malloc(sizeof *s);
    if (!s)
    {
        pthread_attr_destroy(&attr);
        return -1;
    }
    s->fn = fn;
    s->arg = arg;
    int r = pthread_create(t, &attr, thread_main, s);
    pthread_attr_destroy(&attr);
    if (r != 0)
    {
        free(s);
        return -1;
    }
    return 0;
}

void plat_thread_join(plat_thread t)
{
    pthread_join(t, NULL);
}

void plat_thread_detach(plat_thread t)
{
    pthread_detach(t);
}

void plat_thread_yield(void)
{
    sched_yield();
}

uint64_t plat_thread_self(void)
{
    return (uint64_t)pthread_self();
}

/* Per-thread exit callbacks (`plat_thread_at_exit`): a list behind one
   pthread key, whose destructor runs them, most recent first, as Rust runs
   a thread's thread-local destructors. */
typedef struct at_exit_node
{
    void (*fn)(void *);
    void *arg;
    struct at_exit_node *next;
} at_exit_node;

static pthread_key_t at_exit_key;
static pthread_once_t at_exit_once = PTHREAD_ONCE_INIT;

static void at_exit_run(void *p)
{
    at_exit_node *n = (at_exit_node *)p;
    while (n)
    {
        at_exit_node *next = n->next;
        n->fn(n->arg);
        free(n);
        n = next;
    }
}

static void at_exit_make(void)
{
    if (pthread_key_create(&at_exit_key, at_exit_run) != 0)
    {
        cbrt_internal("no thread-local key for a thread's end");
    }
}

void plat_thread_at_exit(void (*fn)(void *), void *arg)
{
    pthread_once(&at_exit_once, at_exit_make);
    at_exit_node *n = (at_exit_node *)cb_xmalloc(sizeof *n);
    n->fn = fn;
    n->arg = arg;
    n->next = (at_exit_node *)pthread_getspecific(at_exit_key);
    pthread_setspecific(at_exit_key, n);
}

/* Rust's `cgroups::quota` for cgroup v2: the smallest `cpu.max` limit over
   the program's cgroup and its ancestors, in whole processors. */
static size_t cgroup_quota(void)
{
    size_t quota = SIZE_MAX;
    FILE *f = fopen("/proc/self/cgroup", "re");
    if (!f)
    {
        return quota;
    }
    char line[4096];
    char path[4096];
    int found = 0, v1 = 0;
    while (fgets(line, sizeof line, f))
    {
        line[strcspn(line, "\n")] = 0;
        char *a = strchr(line, ':');
        if (!a)
        {
            continue;
        }
        char *b = strchr(a + 1, ':');
        if (!b)
        {
            continue;
        }
        *b = 0;
        const char *ctl = a + 1;
        int is_v2 = ctl[0] == 0;
        int is_v1 = 0;
        if (!is_v2)
        {
            char tmp[1024];
            snprintf(tmp, sizeof tmp, "%s", ctl);
            for (char *tok = strtok(tmp, ","); tok; tok = strtok(NULL, ","))
            {
                if (strcmp(tok, "cpu") == 0)
                {
                    is_v1 = 1;
                }
            }
            if (!is_v1)
            {
                continue;
            }
        }
        /* Already-found v1 trumps v2, since it names its controllers. */
        if (found && is_v2)
        {
            continue;
        }
        const char *p = b + 1;
        snprintf(path, sizeof path, "%s", p[0] ? p + 1 : p);
        found = 1;
        v1 = is_v1;
    }
    fclose(f);
    if (!found || v1)
    {
        /* cgroup v1 (Rust reads the mount from /proc/self/mountinfo): not
           followed here; v2 is what current systems mount. */
        return quota;
    }
    char p[8300];
    struct stat st;
    snprintf(p, sizeof p, "/sys/fs/cgroup/%s/cgroup.controllers", path);
    if (stat(p, &st) != 0 && errno == ENOENT)
    {
        return quota;
    }
    snprintf(p, sizeof p, "/sys/fs/cgroup/%s", path);
    size_t len = strlen(p);
    while (len > 0 && p[len - 1] == '/')
    {
        p[--len] = 0;
    }
    for (;;)
    {
        char file[8400];
        snprintf(file, sizeof file, "%s/cpu.max", p);
        FILE *q = fopen(file, "re");
        if (q)
        {
            char buf[128];
            if (fgets(buf, sizeof buf, q))
            {
                unsigned long long lim, per;
                char extra;
                buf[strcspn(buf, "\n")] = 0;
                if (sscanf(buf, "%llu %llu%c", &lim, &per, &extra) == 2 && per > 0 && buf[0] >= '0' && buf[0] <= '9')
                {
                    size_t qq = (size_t)(lim / per);
                    if (qq < quota)
                    {
                        quota = qq;
                    }
                }
            }
            fclose(q);
        }
        if (strcmp(p, "/sys/fs/cgroup") == 0)
        {
            break;
        }
        char *slash = strrchr(p, '/');
        if (!slash || slash == p)
        {
            break;
        }
        *slash = 0;
        if (strncmp(p, "/sys/fs/cgroup", 14) != 0)
        {
            break;
        }
    }
    return quota;
}

/* `std::thread::available_parallelism` on Linux: the processors this thread
   may run on, at most the cgroup's quota. */
uint64_t plat_cpu_count(void)
{
    size_t quota = cgroup_quota();
    if (quota < 1)
    {
        quota = 1;
    }
    cpu_set_t set;
    CPU_ZERO(&set);
    if (sched_getaffinity(0, sizeof set, &set) == 0)
    {
        size_t n = (size_t)CPU_COUNT(&set);
        if (n > 0)
        {
            return n < quota ? n : quota;
        }
    }
    long n = sysconf(_SC_NPROCESSORS_ONLN);
    if (n < 1)
    {
        return 1;
    }
    return (size_t)n < quota ? (uint64_t)n : quota;
}

void plat_spin_hint(void)
{
#if defined(__x86_64__) || defined(__i386__)
    __builtin_ia32_pause();
#elif defined(__aarch64__)
    __asm__ __volatile__("yield");
#endif
}

/* ---- clocks ---- */

uint64_t plat_now_ns(void)
{
    struct timespec ts;
    clock_gettime(CLOCK_MONOTONIC, &ts);
    return (uint64_t)ts.tv_sec * 1000000000u + (uint64_t)ts.tv_nsec;
}

/* `std::thread::sleep`: `nanosleep`, resumed with what is left when a
   signal interrupts it. */
void plat_sleep_ns(uint64_t ns)
{
    uint64_t secs = ns / 1000000000u;
    long nsecs = (long)(ns % 1000000000u);
    while (secs > 0 || nsecs > 0)
    {
        struct timespec ts;
        ts.tv_sec = secs > (uint64_t)INT64_MAX ? (time_t)INT64_MAX : (time_t)secs;
        ts.tv_nsec = nsecs;
        secs -= (uint64_t)ts.tv_sec;
        if (nanosleep(&ts, &ts) == -1)
        {
            secs += (uint64_t)ts.tv_sec;
            nsecs = ts.tv_nsec;
        }
        else
        {
            nsecs = 0;
        }
    }
}

void os_wall_time(int64_t *secs, uint32_t *nanos)
{
    struct timespec ts;
    clock_gettime(CLOCK_REALTIME, &ts);
    *secs = (int64_t)ts.tv_sec;
    *nanos = (uint32_t)ts.tv_nsec;
}

int os_local_time(int64_t when, int64_t tm6[6])
{
    time_t t = (time_t)when;
    struct tm tm;
    if (!localtime_r(&t, &tm))
    {
        return 0;
    }
    tm6[0] = (int64_t)tm.tm_year + 1900;
    tm6[1] = (int64_t)tm.tm_mon + 1;
    tm6[2] = tm.tm_mday;
    tm6[3] = tm.tm_hour;
    tm6[4] = tm.tm_min;
    tm6[5] = tm.tm_sec;
    return 1;
}

/* ---- the process ---- */

void *plat_aligned_alloc(size_t size, size_t align)
{
    void *p = NULL;
    if (align < sizeof(void *))
    {
        align = sizeof(void *);
    }
    if (posix_memalign(&p, align, size ? size : 1) != 0)
    {
        return NULL;
    }
    return p;
}

void plat_aligned_free(void *p)
{
    free(p);
}

void plat_exit(int status)
{
    exit(status);
}

void plat_abort(void)
{
    abort();
}

const char *plat_getenv(const char *name)
{
    return getenv(name);
}

void plat_args(int argc, const char *const *argv, uint8_t ***args, size_t **lens, size_t *n)
{
    size_t k = argc > 1 ? (size_t)argc - 1 : 0;
    uint8_t **a = (uint8_t **)cb_xmalloc((k ? k : 1) * sizeof(uint8_t *));
    size_t *l = (size_t *)cb_xmalloc((k ? k : 1) * sizeof(size_t));
    for (size_t i = 0; i < k; i++)
    {
        size_t m = strlen(argv[i + 1]);
        a[i] = (uint8_t *)cb_xmalloc(m + 1);
        memcpy(a[i], argv[i + 1], m + 1);
        l[i] = m;
    }
    *args = a;
    *lens = l;
    *n = k;
}

int plat_stderr_is_tty(void)
{
    return isatty(2) != 0;
}

int plat_stdout_is_tty(void)
{
    return isatty(1) != 0;
}

int os_stdout_is_tty(void)
{
    return isatty(1) != 0;
}

int plat_enable_ansi(void)
{
    return 1;
}

/* ---- errors ---- */

/* `decode_error_kind`, as far as the modules tell kinds apart. */
static int kind_of(int e)
{
    switch (e)
    {
    case 0:
        return OS_OK;
    case ENOENT:
        return OS_NOT_FOUND;
    case EACCES:
    case EPERM:
        return OS_DENIED;
    case EEXIST:
        return OS_EXISTS;
    case EINTR:
        return OS_INTERRUPTED;
    case EAGAIN:
        return OS_WOULD_BLOCK;
    case ETIMEDOUT:
        return OS_TIMED_OUT;
    case ECONNREFUSED:
        return OS_REFUSED;
    case ECONNRESET:
        return OS_RESET;
    case ECONNABORTED:
        return OS_ABORTED;
    case EADDRINUSE:
        return OS_ADDR_IN_USE;
    case EADDRNOTAVAIL:
        return OS_ADDR_NOT_AVAILABLE;
    case EPIPE:
        return OS_BROKEN_PIPE;
    case ENOTCONN:
        return OS_NOT_CONNECTED;
    case ENETDOWN:
    case ENETUNREACH:
    case EHOSTUNREACH:
        return OS_UNREACHABLE;
    case EINVAL:
        return OS_INVALID;
    case EISDIR:
        return OS_IS_DIR;
    case ENOTDIR:
        return OS_NOT_DIR;
    case ENOSPC:
#ifdef EDQUOT
    case EDQUOT:
#endif
        return OS_NO_SPACE;
    case EMFILE:
    case ENFILE:
        return OS_TOO_MANY_OPEN;
    case EMSGSIZE:
        return OS_MESSAGE_TOO_LONG;
    default:
        return OS_OTHER;
    }
}

static int last(void)
{
    return kind_of(errno);
}

/* A path's bytes as a C string (Rust's `run_path_with_cstr`): NULL for one
   holding a NUL. Short paths use `small`. */
static char *cpath(const uint8_t *p, size_t n, char *small, size_t cap)
{
    if (n && memchr(p, 0, n))
    {
        return NULL;
    }
    char *s = n + 1 <= cap ? small : (char *)cb_xmalloc(n + 1);
    if (n)
    {
        memcpy(s, p, n);
    }
    s[n] = 0;
    return s;
}

#define CPATH(var, p, n)                    \
    char var##_small[512];                  \
    char *var = cpath(p, n, var##_small, sizeof var##_small); \
    if (!var)                               \
    {                                       \
        return OS_INVALID;                  \
    }
#define CFREE(var)               \
    if (var != var##_small)      \
    {                            \
        free(var);               \
    }

/* ---- standard streams ---- */

int os_std_write(int which, const uint8_t *p, size_t n)
{
    while (n > 0)
    {
        ssize_t k = write(which, p, n > (size_t)SSIZE_MAX ? (size_t)SSIZE_MAX : n);
        if (k < 0)
        {
            if (errno == EINTR)
            {
                continue;
            }
            /* Rust's `handle_ebadf`: a stream that is not open swallows. */
            if (errno == EBADF)
            {
                return OS_OK;
            }
            return last();
        }
        if (k == 0)
        {
            return OS_WRITE_ZERO;
        }
        p += k;
        n -= (size_t)k;
    }
    return OS_OK;
}

int os_stdin_read(uint8_t *p, size_t n, size_t *got)
{
    ssize_t k = read(0, p, n > (size_t)SSIZE_MAX ? (size_t)SSIZE_MAX : n);
    if (k < 0)
    {
        if (errno == EBADF)
        {
            *got = 0;
            return OS_OK;
        }
        return last();
    }
    *got = (size_t)k;
    return OS_OK;
}

/* ---- files ---- */

static int open_flags(int mode)
{
    switch (mode)
    {
    case 0:
        return O_RDONLY;
    case 1:
        return O_WRONLY | O_CREAT | O_TRUNC;
    case 2:
        return O_WRONLY | O_APPEND | O_CREAT;
    case 3:
        return O_RDWR | O_CREAT;
    default:
        return O_WRONLY | O_CREAT | O_EXCL;
    }
}

static int open_retry(const char *path, int flags, mode_t m, int *fd)
{
    for (;;)
    {
        int f = open(path, flags | O_CLOEXEC, m);
        if (f >= 0)
        {
            *fd = f;
            return OS_OK;
        }
        if (errno != EINTR)
        {
            return last();
        }
    }
}

int os_file_open(const uint8_t *path, size_t n, int mode, os_file *f)
{
    CPATH(s, path, n);
    int fd = -1;
    int k = open_retry(s, open_flags(mode), 0666, &fd);
    CFREE(s);
    if (k == OS_OK)
    {
        *f = fd;
    }
    return k;
}

int os_file_read(os_file f, uint8_t *p, size_t n, size_t *got)
{
    ssize_t k = read((int)f, p, n > (size_t)SSIZE_MAX ? (size_t)SSIZE_MAX : n);
    if (k < 0)
    {
        *got = 0;
        return last();
    }
    *got = (size_t)k;
    return OS_OK;
}

int os_file_write(os_file f, const uint8_t *p, size_t n, size_t *put)
{
    ssize_t k = write((int)f, p, n > (size_t)SSIZE_MAX ? (size_t)SSIZE_MAX : n);
    if (k < 0)
    {
        *put = 0;
        return last();
    }
    *put = (size_t)k;
    return OS_OK;
}

int os_file_seek(os_file f, int whence, int64_t off, uint64_t *pos)
{
    off_t r = lseek((int)f, (off_t)off, whence == 0 ? SEEK_SET : SEEK_CUR);
    if (r < 0)
    {
        return last();
    }
    *pos = (uint64_t)r;
    return OS_OK;
}

int os_file_stat(os_file f, uint64_t *len, int *is_dir)
{
    struct stat st;
    if (fstat((int)f, &st) != 0)
    {
        return last();
    }
    *len = (uint64_t)st.st_size;
    *is_dir = S_ISDIR(st.st_mode);
    return OS_OK;
}

int os_file_sync(os_file f)
{
    for (;;)
    {
        if (fsync((int)f) == 0)
        {
            return OS_OK;
        }
        if (errno != EINTR)
        {
            return last();
        }
    }
}

void os_file_close(os_file f)
{
    close((int)f);
}

/* `read_to_end`: every byte to the end, interrupted reads retried. */
static int read_to_end(int fd, cb_buf *out)
{
    for (;;)
    {
        cb_buf_reserve(out, 8192);
        size_t room = out->cap - out->n;
        ssize_t k = read(fd, out->p + out->n, room);
        if (k < 0)
        {
            if (errno == EINTR)
            {
                continue;
            }
            return last();
        }
        if (k == 0)
        {
            return OS_OK;
        }
        out->n += (size_t)k;
    }
}

static int write_all_fd(int fd, const uint8_t *p, size_t n)
{
    while (n > 0)
    {
        ssize_t k = write(fd, p, n > (size_t)SSIZE_MAX ? (size_t)SSIZE_MAX : n);
        if (k < 0)
        {
            if (errno == EINTR)
            {
                continue;
            }
            return last();
        }
        if (k == 0)
        {
            return OS_WRITE_ZERO;
        }
        p += k;
        n -= (size_t)k;
    }
    return OS_OK;
}

int os_read_file(const uint8_t *path, size_t n, cb_buf *out)
{
    CPATH(s, path, n);
    int fd = -1;
    int k = open_retry(s, O_RDONLY, 0666, &fd);
    CFREE(s);
    if (k != OS_OK)
    {
        return k;
    }
    out->n = 0;
    struct stat st;
    if (fstat(fd, &st) == 0 && st.st_size > 0)
    {
        cb_buf_reserve(out, (size_t)st.st_size);
    }
    k = read_to_end(fd, out);
    close(fd);
    return k;
}

int os_write_file(const uint8_t *path, size_t n, const uint8_t *d, size_t dn)
{
    CPATH(s, path, n);
    int fd = -1;
    int k = open_retry(s, O_WRONLY | O_CREAT | O_TRUNC, 0666, &fd);
    CFREE(s);
    if (k != OS_OK)
    {
        return k;
    }
    k = write_all_fd(fd, d, dn);
    close(fd);
    return k;
}

static void meta_of(const struct stat *st, os_meta *m)
{
    m->kind = S_ISREG(st->st_mode) ? 0 : S_ISDIR(st->st_mode) ? 1 : 2;
    m->len = (uint64_t)st->st_size;
    m->has_modified = 1;
    m->mod_secs = (int64_t)st->st_mtim.tv_sec;
    m->readonly = (st->st_mode & 0222) == 0;
}

int os_metadata(const uint8_t *path, size_t n, os_meta *m)
{
    CPATH(s, path, n);
    struct stat st;
    int r = stat(s, &st);
    int e = errno;
    CFREE(s);
    if (r != 0)
    {
        return kind_of(e);
    }
    meta_of(&st, m);
    return OS_OK;
}

int os_is_dir(const uint8_t *path, size_t n)
{
    os_meta m;
    return os_metadata(path, n, &m) == OS_OK && m.kind == 1;
}

int os_mkdir(const uint8_t *path, size_t n)
{
    CPATH(s, path, n);
    int r = mkdir(s, 0777);
    int e = errno;
    CFREE(s);
    return r == 0 ? OS_OK : kind_of(e);
}

int os_remove_file(const uint8_t *path, size_t n)
{
    CPATH(s, path, n);
    int r = unlink(s);
    int e = errno;
    CFREE(s);
    return r == 0 ? OS_OK : kind_of(e);
}

int os_remove_dir(const uint8_t *path, size_t n)
{
    CPATH(s, path, n);
    int r = rmdir(s);
    int e = errno;
    CFREE(s);
    return r == 0 ? OS_OK : kind_of(e);
}

int os_rename(const uint8_t *a, size_t an, const uint8_t *b, size_t bn)
{
    CPATH(s, a, an);
    char t_small[512];
    char *t = cpath(b, bn, t_small, sizeof t_small);
    if (!t)
    {
        CFREE(s);
        return OS_INVALID;
    }
    int r = rename(s, t);
    int e = errno;
    CFREE(s);
    CFREE(t);
    return r == 0 ? OS_OK : kind_of(e);
}

int os_list_dir(const uint8_t *path, size_t n, cb_buf *names, size_t *count)
{
    CPATH(s, path, n);
    DIR *d = opendir(s);
    int e = errno;
    CFREE(s);
    if (!d)
    {
        return kind_of(e);
    }
    *count = 0;
    names->n = 0;
    for (;;)
    {
        errno = 0;
        struct dirent *ent = readdir(d);
        if (!ent)
        {
            if (errno != 0)
            {
                e = errno;
                closedir(d);
                return kind_of(e);
            }
            break;
        }
        const char *nm = ent->d_name;
        if (strcmp(nm, ".") == 0 || strcmp(nm, "..") == 0)
        {
            continue;
        }
        cb_buf_put(names, nm, strlen(nm));
        cb_buf_byte(names, 0);
        (*count)++;
    }
    closedir(d);
    return OS_OK;
}

int os_canonicalize(const uint8_t *path, size_t n, cb_buf *out)
{
    CPATH(s, path, n);
    char *r = realpath(s, NULL);
    int e = errno;
    CFREE(s);
    if (!r)
    {
        return kind_of(e);
    }
    out->n = 0;
    cb_buf_str(out, r);
    free(r);
    return OS_OK;
}

/* `std::fs::copy` on Linux: the source must be a regular file; the copy is
   made with the source's mode, which is set again in case it existed. */
int os_copy_file(const uint8_t *a, size_t an, const uint8_t *b, size_t bn, uint64_t *copied)
{
    CPATH(s, a, an);
    int in = -1;
    int k = open_retry(s, O_RDONLY, 0666, &in);
    CFREE(s);
    if (k != OS_OK)
    {
        return k;
    }
    struct stat st;
    if (fstat(in, &st) != 0)
    {
        k = last();
        close(in);
        return k;
    }
    if (!S_ISREG(st.st_mode))
    {
        close(in);
        return OS_INVALID;
    }
    char t_small[512];
    char *t = cpath(b, bn, t_small, sizeof t_small);
    if (!t)
    {
        close(in);
        return OS_INVALID;
    }
    int out = -1;
    k = open_retry(t, O_WRONLY | O_CREAT | O_TRUNC, st.st_mode & 07777, &out);
    CFREE(t);
    if (k != OS_OK)
    {
        close(in);
        return k;
    }
    struct stat ost;
    if (fstat(out, &ost) != 0)
    {
        k = last();
        goto done;
    }
    if (S_ISREG(ost.st_mode))
    {
        for (;;)
        {
            if (fchmod(out, st.st_mode) == 0)
            {
                break;
            }
            if (errno != EINTR)
            {
                k = last();
                goto done;
            }
        }
    }
    uint64_t total = 0;
    uint8_t buf[65536];
    for (;;)
    {
        ssize_t r = read(in, buf, sizeof buf);
        if (r < 0)
        {
            if (errno == EINTR)
            {
                continue;
            }
            k = last();
            goto done;
        }
        if (r == 0)
        {
            break;
        }
        k = write_all_fd(out, buf, (size_t)r);
        if (k != OS_OK)
        {
            goto done;
        }
        total += (uint64_t)r;
    }
    *copied = total;
    k = OS_OK;
done:
    close(in);
    close(out);
    return k;
}

int os_copy_permissions(const uint8_t *a, size_t an, const uint8_t *b, size_t bn)
{
    CPATH(s, a, an);
    struct stat st;
    int r = stat(s, &st);
    int e = errno;
    CFREE(s);
    if (r != 0)
    {
        return kind_of(e);
    }
    CPATH(t, b, bn);
    int k = OS_OK;
    for (;;)
    {
        if (chmod(t, st.st_mode) == 0)
        {
            break;
        }
        if (errno != EINTR)
        {
            k = last();
            break;
        }
    }
    CFREE(t);
    return k;
}

/* Rust's `remove_dir_all_recursive`: by descriptors, never following a
   link; a child already gone is no failure. */
static int rm_recursive(int parent, const char *name)
{
    int fd = openat(parent, name, O_CLOEXEC | O_RDONLY | O_NOFOLLOW | O_DIRECTORY);
    if (fd < 0)
    {
        int e = errno;
        if (e == ENOTDIR || e == ELOOP)
        {
            if (parent != AT_FDCWD)
            {
                return unlinkat(parent, name, 0) == 0 ? OS_OK : last();
            }
        }
        return kind_of(e);
    }
    DIR *d = fdopendir(fd);
    if (!d)
    {
        int e = errno;
        close(fd);
        return kind_of(e);
    }
    for (;;)
    {
        errno = 0;
        struct dirent *ent = readdir(d);
        if (!ent)
        {
            if (errno != 0)
            {
                int e = errno;
                closedir(d);
                return kind_of(e);
            }
            break;
        }
        const char *nm = ent->d_name;
        if (strcmp(nm, ".") == 0 || strcmp(nm, "..") == 0)
        {
            continue;
        }
        int r;
        if (ent->d_type == DT_DIR || ent->d_type == DT_UNKNOWN)
        {
            r = rm_recursive(dirfd(d), nm);
        }
        else
        {
            r = unlinkat(dirfd(d), nm, 0) == 0 ? OS_OK : last();
        }
        if (r != OS_OK && r != OS_NOT_FOUND)
        {
            closedir(d);
            return r;
        }
    }
    closedir(d);
    if (unlinkat(parent, name, AT_REMOVEDIR) != 0 && errno != ENOENT)
    {
        return last();
    }
    return OS_OK;
}

int os_remove_dir_all(const uint8_t *path, size_t n)
{
    CPATH(s, path, n);
    struct stat st;
    int k;
    if (lstat(s, &st) != 0)
    {
        k = last();
    }
    else if (S_ISLNK(st.st_mode))
    {
        k = unlink(s) == 0 ? OS_OK : last();
    }
    else
    {
        k = rm_recursive(AT_FDCWD, s);
    }
    CFREE(s);
    return k;
}

int os_set_current_dir(const uint8_t *path, size_t n)
{
    CPATH(s, path, n);
    int r = chdir(s);
    int e = errno;
    CFREE(s);
    return r == 0 ? OS_OK : kind_of(e);
}

/* ---- the environment ---- */

int os_env_var(const uint8_t *name, size_t n, cb_buf *out)
{
    out->n = 0;
    char small[256];
    char *s = cpath(name, n, small, sizeof small);
    if (!s)
    {
        return 0;
    }
    const char *v = getenv(s);
    if (s != small)
    {
        free(s);
    }
    if (!v)
    {
        return 0;
    }
    cb_buf_str(out, v);
    return 1;
}

int os_current_dir(cb_buf *out)
{
    size_t cap = 512;
    for (;;)
    {
        out->n = 0;
        cb_buf_reserve(out, cap);
        if (getcwd((char *)out->p, out->cap))
        {
            out->n = strlen((char *)out->p);
            return OS_OK;
        }
        if (errno != ERANGE)
        {
            return last();
        }
        cap = out->cap * 2;
    }
}

void os_temp_dir(cb_buf *out)
{
    out->n = 0;
    const char *t = getenv("TMPDIR");
    cb_buf_str(out, t ? t : "/tmp");
}

int os_current_exe(cb_buf *out)
{
    size_t cap = 256;
    for (;;)
    {
        out->n = 0;
        cb_buf_reserve(out, cap);
        ssize_t k = readlink("/proc/self/exe", (char *)out->p, out->cap);
        if (k < 0)
        {
            /* "no /proc/self/exe available. Is /proc mounted?" */
            return errno == ENOENT ? OS_OTHER : last();
        }
        if ((size_t)k < out->cap)
        {
            out->n = (size_t)k;
            return OS_OK;
        }
        cap = out->cap * 2;
    }
}

int os_hostname(cb_buf *out)
{
    char buf[256];
    memset(buf, 0, sizeof buf);
    if (gethostname(buf, sizeof buf) != 0)
    {
        return OS_OTHER;
    }
    out->n = 0;
    cb_buf_put(out, buf, strnlen(buf, sizeof buf));
    return OS_OK;
}

/* ---- child processes ---- */

struct os_child
{
    pid_t pid;
    int has_status;
    int status;
};

static int status_of(int st)
{
    if (WIFEXITED(st))
    {
        return WEXITSTATUS(st);
    }
    if (WIFSIGNALED(st))
    {
        return -WTERMSIG(st);
    }
    return -1;
}

/* A program's search as glibc's `__execvpex` (`posix_spawnp`): the
   directories of `path` in order, an empty one the working directory;
   EACCES remembered, ENOENT and its like passed over, anything else the
   answer. Returns only on failure, with errno set. */
static void exec_search(const char *file, char *const *argv, char *const *envp, const char *path)
{
    size_t flen = strlen(file);
    int got_eacces = 0;
    const char *p = path;
    char buf[8192];
    for (;;)
    {
        const char *sub = strchrnul(p, ':');
        size_t plen = (size_t)(sub - p);
        if (plen + flen + 2 <= sizeof buf)
        {
            memcpy(buf, p, plen);
            size_t at = plen;
            if (plen)
            {
                buf[at++] = '/';
            }
            memcpy(buf + at, file, flen + 1);
            execve(buf, argv, envp);
            switch (errno)
            {
            case EACCES:
                got_eacces = 1;
                break;
            case ENOENT:
            case ESTALE:
            case ENOTDIR:
            case ENODEV:
            case ETIMEDOUT:
                break;
            default:
                return;
            }
        }
        if (*sub == 0)
        {
            break;
        }
        p = sub + 1;
    }
    errno = got_eacces ? EACCES : ENOENT;
}

static int cmd_has_nul(const os_cmd *c)
{
    for (size_t i = 0; i < c->argc; i++)
    {
        if (c->argl[i] && memchr(c->argv[i], 0, c->argl[i]))
        {
            return 1;
        }
    }
    for (size_t i = 0; i < c->nenv; i++)
    {
        if ((c->env_kn[i] && memchr(c->env_k[i], 0, c->env_kn[i])) || (c->env_vn[i] && memchr(c->env_v[i], 0, c->env_vn[i])))
        {
            return 1;
        }
    }
    if (c->has_dir && c->dirn && memchr(c->dir, 0, c->dirn))
    {
        return 1;
    }
    return 0;
}

static char *dupn(const uint8_t *p, size_t n)
{
    char *s = (char *)cb_xmalloc(n + 1);
    if (n)
    {
        memcpy(s, p, n);
    }
    s[n] = 0;
    return s;
}

typedef struct
{
    char *k;
    size_t kn;
    char *kv; /* "k=v" */
} env_ent;

static int env_cmp(const void *x, const void *y)
{
    const env_ent *a = (const env_ent *)x, *b = (const env_ent *)y;
    size_t m = a->kn < b->kn ? a->kn : b->kn;
    int c = memcmp(a->k, b->k, m);
    if (c)
    {
        return c;
    }
    return a->kn < b->kn ? -1 : a->kn > b->kn ? 1 : 0;
}

/* The child's environment when the command changes it (Rust's
   `capture_env`): the parent's variables unless cleared, then each one the
   command sets, in a map ordered by name. NULL when unchanged. */
static char **capture_env(const os_cmd *c)
{
    if (!c->clear_env && c->nenv == 0)
    {
        return NULL;
    }
    size_t cap = c->nenv + 1;
    if (!c->clear_env)
    {
        for (char **e = environ; e && *e; e++)
        {
            cap++;
        }
    }
    env_ent *v = (env_ent *)cb_xmalloc(cap * sizeof(env_ent));
    size_t n = 0;
    if (!c->clear_env)
    {
        for (char **e = environ; e && *e; e++)
        {
            /* `vars_os`: the name is up to the first `=` after the first
               byte; an entry without one is skipped. */
            const char *s = *e;
            if (!s[0])
            {
                continue;
            }
            const char *eq = strchr(s + 1, '=');
            if (!eq)
            {
                continue;
            }
            size_t kn = (size_t)(eq - s);
            /* A later duplicate replaces an earlier one (a map). */
            size_t j;
            for (j = 0; j < n; j++)
            {
                if (v[j].kn == kn && memcmp(v[j].k, s, kn) == 0)
                {
                    break;
                }
            }
            char *kv = strdup(s);
            if (j < n)
            {
                free(v[j].kv);
                v[j].kv = kv;
                v[j].k = kv;
            }
            else
            {
                v[n].k = kv;
                v[n].kn = kn;
                v[n].kv = kv;
                n++;
            }
        }
    }
    for (size_t i = 0; i < c->nenv; i++)
    {
        size_t kn = c->env_kn[i], vn = c->env_vn[i];
        char *kv = (char *)cb_xmalloc(kn + vn + 2);
        memcpy(kv, c->env_k[i], kn);
        kv[kn] = '=';
        memcpy(kv + kn + 1, c->env_v[i], vn);
        kv[kn + vn + 1] = 0;
        size_t j;
        for (j = 0; j < n; j++)
        {
            if (v[j].kn == kn && memcmp(v[j].k, kv, kn) == 0)
            {
                break;
            }
        }
        if (j < n)
        {
            free(v[j].kv);
            v[j].kv = kv;
            v[j].k = kv;
        }
        else
        {
            v[n].k = kv;
            v[n].kn = kn;
            v[n].kv = kv;
            n++;
        }
    }
    qsort(v, n, sizeof(env_ent), env_cmp);
    char **envp = (char **)cb_xmalloc((n + 1) * sizeof(char *));
    for (size_t i = 0; i < n; i++)
    {
        envp[i] = v[i].kv;
    }
    envp[n] = NULL;
    free(v);
    return envp;
}

static int cmd_saw_path(const os_cmd *c)
{
    if (c->clear_env)
    {
        return 1;
    }
    for (size_t i = 0; i < c->nenv; i++)
    {
        if (c->env_kn[i] == 4 && memcmp(c->env_k[i], "PATH", 4) == 0)
        {
            return 1;
        }
    }
    return 0;
}

/* A pipe, both ends close-on-exec; `fds[0]` reads. */
static int make_pipe(int fds[2])
{
    if (pipe2(fds, O_CLOEXEC) != 0)
    {
        return last();
    }
    return OS_OK;
}

/* `Command::spawn` (Rust's fork and exec path; its `posix_spawn` path
   behaves the same for what procio asks): the child's streams set up, its
   signal mask emptied and SIGPIPE back to its default, its working
   directory changed, then the program run; a failure before or in `exec`
   comes back through a close-on-exec pipe as the child's errno. */
int os_spawn(const os_cmd *c, int in_mode, int out_mode, int err_mode, os_child **child, os_pipe *in, os_pipe *out, os_pipe *err)
{
    *in = *out = *err = OS_PIPE_NONE;
    if (c->argc == 0 || cmd_has_nul(c))
    {
        return OS_INVALID;
    }
    char **argv = (char **)cb_xmalloc((c->argc + 1) * sizeof(char *));
    for (size_t i = 0; i < c->argc; i++)
    {
        argv[i] = dupn(c->argv[i], c->argl[i]);
    }
    argv[c->argc] = NULL;
    char *dir = c->has_dir ? dupn(c->dir, c->dirn) : NULL;
    char **envp = capture_env(c);
    int saw_path = cmd_saw_path(c);
    int has_slash = strchr(argv[0], '/') != NULL;
    const char *ppath = getenv("PATH");
    if (!ppath)
    {
        ppath = "/bin:/usr/bin";
    }
    char *search = strdup(ppath);
    int modes[3] = {in_mode, out_mode, err_mode};
    int ends[3][2] = {{-1, -1}, {-1, -1}, {-1, -1}};
    int k = OS_OK;
    for (int i = 0; i < 3 && k == OS_OK; i++)
    {
        if (modes[i] == OS_STDIO_PIPED)
        {
            k = make_pipe(ends[i]);
        }
        else if (modes[i] == OS_STDIO_NULL)
        {
            int fd = -1;
            k = open_retry("/dev/null", i == 0 ? O_RDONLY : O_WRONLY, 0666, &fd);
            if (k == OS_OK)
            {
                /* The child's end in both places. */
                ends[i][0] = ends[i][1] = fd;
            }
        }
    }
    int report[2] = {-1, -1};
    if (k == OS_OK)
    {
        k = make_pipe(report);
    }
    pid_t pid = -1;
    if (k == OS_OK)
    {
        pid = fork();
        if (pid < 0)
        {
            k = last();
        }
    }
    if (pid == 0)
    {
        /* The child: async-signal-safe calls only, until `exec`. */
        int e = 0;
        for (int i = 0; i < 3; i++)
        {
            int fd = -1;
            if (modes[i] == OS_STDIO_PIPED)
            {
                fd = i == 0 ? ends[i][0] : ends[i][1];
            }
            else if (modes[i] == OS_STDIO_NULL)
            {
                fd = ends[i][0];
            }
            if (fd < 0)
            {
                continue;
            }
            if (fd == i)
            {
                int fl = fcntl(fd, F_GETFD);
                if (fl < 0 || fcntl(fd, F_SETFD, fl & ~FD_CLOEXEC) < 0)
                {
                    e = errno;
                    goto fail;
                }
            }
            else
            {
                while (dup2(fd, i) < 0)
                {
                    if (errno != EINTR)
                    {
                        e = errno;
                        goto fail;
                    }
                }
            }
        }
        if (dir && chdir(dir) != 0)
        {
            e = errno;
            goto fail;
        }
        {
            sigset_t set;
            sigemptyset(&set);
            if (pthread_sigmask(SIG_SETMASK, &set, NULL) != 0)
            {
                e = errno;
                goto fail;
            }
            signal(SIGPIPE, SIG_DFL);
        }
        if (has_slash)
        {
            execve(argv[0], argv, envp ? envp : environ);
        }
        else if (saw_path)
        {
            /* The child's own PATH, as `execvp` reads it. */
            environ = envp;
            execvp(argv[0], argv);
        }
        else if (argv[0][0] == 0)
        {
            errno = ENOENT;
        }
        else
        {
            exec_search(argv[0], argv, envp ? envp : environ, search);
        }
        e = errno;
    fail:
    {
        uint8_t b[4] = {(uint8_t)(e >> 24), (uint8_t)(e >> 16), (uint8_t)(e >> 8), (uint8_t)e};
        ssize_t w = write(report[1], b, 4);
        (void)w;
        _exit(1);
    }
    }
    /* The parent. */
    if (report[1] >= 0)
    {
        close(report[1]);
    }
    if (pid > 0)
    {
        uint8_t b[4];
        size_t got = 0;
        while (got < 4)
        {
            ssize_t r = read(report[0], b + got, 4 - got);
            if (r < 0 && errno == EINTR)
            {
                continue;
            }
            if (r <= 0)
            {
                break;
            }
            got += (size_t)r;
        }
        if (got == 4)
        {
            int e = (int)((uint32_t)b[0] << 24 | (uint32_t)b[1] << 16 | (uint32_t)b[2] << 8 | b[3]);
            int st;
            while (waitpid(pid, &st, 0) < 0 && errno == EINTR)
            {
            }
            k = kind_of(e);
            pid = -1;
        }
    }
    if (report[0] >= 0)
    {
        close(report[0]);
    }
    for (int i = 0; i < 3; i++)
    {
        if (modes[i] == OS_STDIO_PIPED)
        {
            int child_end = i == 0 ? ends[i][0] : ends[i][1];
            int our_end = i == 0 ? ends[i][1] : ends[i][0];
            if (child_end >= 0)
            {
                close(child_end);
            }
            if (our_end >= 0)
            {
                if (k == OS_OK)
                {
                    if (i == 0)
                    {
                        *in = our_end;
                    }
                    else if (i == 1)
                    {
                        *out = our_end;
                    }
                    else
                    {
                        *err = our_end;
                    }
                }
                else
                {
                    close(our_end);
                }
            }
        }
        else if (modes[i] == OS_STDIO_NULL && ends[i][0] >= 0)
        {
            close(ends[i][0]);
        }
    }
    for (size_t i = 0; i < c->argc; i++)
    {
        free(argv[i]);
    }
    free(argv);
    free(dir);
    free(search);
    if (envp)
    {
        for (char **e = envp; *e; e++)
        {
            free(*e);
        }
        free(envp);
    }
    if (k != OS_OK)
    {
        return k;
    }
    os_child *ch = (os_child *)cb_xcalloc(1, sizeof(os_child));
    ch->pid = pid;
    *child = ch;
    return OS_OK;
}

int os_child_wait(os_child *c, int *status)
{
    if (c->has_status)
    {
        *status = status_of(c->status);
        return OS_OK;
    }
    int st;
    for (;;)
    {
        if (waitpid(c->pid, &st, 0) >= 0)
        {
            break;
        }
        if (errno != EINTR)
        {
            return last();
        }
    }
    c->has_status = 1;
    c->status = st;
    *status = status_of(st);
    return OS_OK;
}

int os_child_try_wait(os_child *c, int *done, int *status)
{
    if (c->has_status)
    {
        *done = 1;
        *status = status_of(c->status);
        return OS_OK;
    }
    int st;
    pid_t r = waitpid(c->pid, &st, WNOHANG);
    if (r < 0)
    {
        return last();
    }
    if (r == 0)
    {
        *done = 0;
        return OS_OK;
    }
    c->has_status = 1;
    c->status = st;
    *done = 1;
    *status = status_of(st);
    return OS_OK;
}

int os_child_kill(os_child *c)
{
    /* A reaped child's id may name another process by now. */
    if (c->has_status)
    {
        return OS_OK;
    }
    return kill(c->pid, SIGKILL) == 0 ? OS_OK : last();
}

int os_child_terminate(os_child *c)
{
    return kill(c->pid, SIGTERM) == 0 ? OS_OK : OS_OTHER;
}

/* D-0205: `Child::kill_tree` on Linux: the child's descendants, found from
   /proc (each process's parent), are killed after the child -- which is
   killed first so it starts no more -- one level at a time. */
int os_child_kill_tree(os_child *c)
{
    size_t n = 0, cap = 256;
    uint32_t(*pairs)[2] = (uint32_t(*)[2])cb_xmalloc(cap * sizeof *pairs);
    DIR *d = opendir("/proc");
    if (d)
    {
        struct dirent *ent;
        while ((ent = readdir(d)))
        {
            const char *nm = ent->d_name;
            if (!nm[0])
            {
                continue;
            }
            /* `str::parse::<u32>`: digits, an optional leading `+`. */
            const char *q = nm[0] == '+' ? nm + 1 : nm;
            if (!*q)
            {
                continue;
            }
            uint64_t pid = 0;
            int ok = 1;
            for (const char *x = q; *x; x++)
            {
                if (*x < '0' || *x > '9')
                {
                    ok = 0;
                    break;
                }
                pid = pid * 10 + (uint64_t)(*x - '0');
                if (pid > UINT32_MAX)
                {
                    ok = 0;
                    break;
                }
            }
            if (!ok)
            {
                continue;
            }
            char file[64];
            snprintf(file, sizeof file, "/proc/%u/stat", (unsigned)pid);
            int fd = open(file, O_RDONLY | O_CLOEXEC);
            if (fd < 0)
            {
                continue;
            }
            cb_buf st = {0};
            int k = read_to_end(fd, &st);
            close(fd);
            if (k != OS_OK)
            {
                cb_buf_free(&st);
                continue;
            }
            /* "pid (comm) state ppid ...": comm may hold spaces and
               parentheses, so read after the last ')'. */
            size_t i = st.n;
            while (i > 0 && st.p[i - 1] != ')')
            {
                i--;
            }
            if (i == 0)
            {
                cb_buf_free(&st);
                continue;
            }
            /* The second whitespace-separated field after it. */
            int field = 0;
            uint64_t ppid = 0;
            int got = 0;
            size_t j = i;
            while (j < st.n)
            {
                while (j < st.n && (st.p[j] == ' ' || st.p[j] == '\n' || st.p[j] == '\t' || st.p[j] == '\r' || st.p[j] == '\f' || st.p[j] == '\v'))
                {
                    j++;
                }
                if (j >= st.n)
                {
                    break;
                }
                size_t s0 = j;
                while (j < st.n && !(st.p[j] == ' ' || st.p[j] == '\n' || st.p[j] == '\t' || st.p[j] == '\r' || st.p[j] == '\f' || st.p[j] == '\v'))
                {
                    j++;
                }
                if (field == 1)
                {
                    int good = 1;
                    size_t a = s0;
                    if (st.p[a] == '+')
                    {
                        a++;
                    }
                    if (a == j)
                    {
                        good = 0;
                    }
                    for (size_t x = a; x < j && good; x++)
                    {
                        if (st.p[x] < '0' || st.p[x] > '9')
                        {
                            good = 0;
                        }
                        else
                        {
                            ppid = ppid * 10 + (uint64_t)(st.p[x] - '0');
                            if (ppid > UINT32_MAX)
                            {
                                good = 0;
                            }
                        }
                    }
                    got = good;
                    break;
                }
                field++;
            }
            cb_buf_free(&st);
            if (got)
            {
                if (n == cap)
                {
                    cap *= 2;
                    pairs = (uint32_t(*)[2])cb_xrealloc(pairs, cap * sizeof *pairs);
                }
                pairs[n][0] = (uint32_t)pid;
                pairs[n][1] = (uint32_t)ppid;
                n++;
            }
        }
        closedir(d);
    }
    size_t lc = 1, ac = 0;
    uint32_t *level = (uint32_t *)cb_xmalloc(sizeof(uint32_t) * (n + 1));
    uint32_t *next = (uint32_t *)cb_xmalloc(sizeof(uint32_t) * (n + 1));
    uint32_t *all = NULL;
    size_t all_cap = 0;
    level[0] = (uint32_t)c->pid;
    while (lc > 0)
    {
        size_t nc = 0;
        for (size_t i = 0; i < n; i++)
        {
            for (size_t j = 0; j < lc; j++)
            {
                if (pairs[i][1] == level[j])
                {
                    next[nc++] = pairs[i][0];
                    break;
                }
            }
        }
        if (ac + nc > all_cap)
        {
            all_cap = (ac + nc) * 2 + 8;
            all = (uint32_t *)cb_xrealloc(all, all_cap * sizeof(uint32_t));
        }
        memcpy(all + ac, next, nc * sizeof(uint32_t));
        ac += nc;
        uint32_t *t = level;
        level = next;
        next = t;
        lc = nc;
        if (lc > n)
        {
            break;
        }
    }
    free(level);
    free(next);
    free(pairs);
    int k = os_child_kill(c);
    if (k == OS_OK)
    {
        for (size_t i = 0; i < ac; i++)
        {
            kill((pid_t)all[i], 9);
        }
    }
    free(all);
    return k;
}

uint32_t os_child_id(os_child *c)
{
    return (uint32_t)c->pid;
}

void os_child_free(os_child *c)
{
    free(c);
}

int os_pipe_read(os_pipe p, uint8_t *buf, size_t n, size_t *got)
{
    ssize_t k = read((int)p, buf, n > (size_t)SSIZE_MAX ? (size_t)SSIZE_MAX : n);
    if (k < 0)
    {
        *got = 0;
        return last();
    }
    *got = (size_t)k;
    return OS_OK;
}

int os_pipe_write(os_pipe p, const uint8_t *buf, size_t n, size_t *put)
{
    ssize_t k = write((int)p, buf, n > (size_t)SSIZE_MAX ? (size_t)SSIZE_MAX : n);
    if (k < 0)
    {
        *put = 0;
        return last();
    }
    *put = (size_t)k;
    return OS_OK;
}

void os_pipe_close(os_pipe p)
{
    if (p != OS_PIPE_NONE)
    {
        close((int)p);
    }
}

/* Rust's `read2`: both pipes read to their ends at once (by `poll`), so a
   child filling one while we wait on the other cannot stall. */
int os_read2(os_pipe a, cb_buf *va, os_pipe b, cb_buf *vb)
{
    int fa = (int)a, fb = (int)b;
    int done_a = fa < 0, done_b = fb < 0;
    if (!done_a)
    {
        fcntl(fa, F_SETFL, fcntl(fa, F_GETFL) | O_NONBLOCK);
    }
    if (!done_b)
    {
        fcntl(fb, F_SETFL, fcntl(fb, F_GETFL) | O_NONBLOCK);
    }
    while (!done_a || !done_b)
    {
        struct pollfd p[2];
        int np = 0;
        if (!done_a)
        {
            p[np].fd = fa;
            p[np].events = POLLIN;
            p[np].revents = 0;
            np++;
        }
        if (!done_b)
        {
            p[np].fd = fb;
            p[np].events = POLLIN;
            p[np].revents = 0;
            np++;
        }
        if (poll(p, (nfds_t)np, -1) < 0)
        {
            if (errno == EINTR)
            {
                continue;
            }
            return last();
        }
        for (int i = 0; i < np; i++)
        {
            if (!p[i].revents)
            {
                continue;
            }
            int fd = p[i].fd;
            cb_buf *v = fd == fa ? va : vb;
            for (;;)
            {
                cb_buf_reserve(v, 8192);
                ssize_t r = read(fd, v->p + v->n, v->cap - v->n);
                if (r > 0)
                {
                    v->n += (size_t)r;
                    continue;
                }
                if (r == 0)
                {
                    if (fd == fa)
                    {
                        done_a = 1;
                    }
                    else
                    {
                        done_b = 1;
                    }
                    break;
                }
                if (errno == EINTR)
                {
                    continue;
                }
                if (errno == EAGAIN)
                {
                    break;
                }
                return last();
            }
        }
    }
    return OS_OK;
}

uint32_t os_process_id(void)
{
    return (uint32_t)getpid();
}

static int *interrupt_flag;

static void on_interrupt(int sig)
{
    (void)sig;
    if (interrupt_flag)
    {
        __atomic_store_n(interrupt_flag, 1, __ATOMIC_SEQ_CST);
    }
}

/* procio's `watch`: `signal` and then `siginterrupt(sig, 1)`, which leaves
   the handler without SA_RESTART, so a blocking call the signal arrives in
   returns EINTR rather than resume. */
void os_watch_interrupts(int *flag)
{
    interrupt_flag = flag;
    struct sigaction sa;
    memset(&sa, 0, sizeof sa);
    sa.sa_handler = on_interrupt;
    sa.sa_flags = 0;
    sigemptyset(&sa.sa_mask);
    sigaddset(&sa.sa_mask, SIGINT);
    sigaction(SIGINT, &sa, NULL);
    sigemptyset(&sa.sa_mask);
    sigaddset(&sa.sa_mask, SIGTERM);
    sigaction(SIGTERM, &sa, NULL);
}

void os_ignore_sigpipe(void)
{
    signal(SIGPIPE, SIG_IGN);
}

/* ---- sockets ---- */

static socklen_t to_sockaddr(const os_addr *a, struct sockaddr_storage *ss)
{
    memset(ss, 0, sizeof *ss);
    if (a->family == 4)
    {
        struct sockaddr_in *s = (struct sockaddr_in *)ss;
        s->sin_family = AF_INET;
        s->sin_port = htons(a->port);
        memcpy(&s->sin_addr, a->ip, 4);
        return sizeof *s;
    }
    struct sockaddr_in6 *s = (struct sockaddr_in6 *)ss;
    s->sin6_family = AF_INET6;
    s->sin6_port = htons(a->port);
    memcpy(&s->sin6_addr, a->ip, 16);
    return sizeof *s;
}

static int from_sockaddr(const struct sockaddr_storage *ss, os_addr *a)
{
    memset(a, 0, sizeof *a);
    if (ss->ss_family == AF_INET)
    {
        const struct sockaddr_in *s = (const struct sockaddr_in *)ss;
        a->family = 4;
        memcpy(a->ip, &s->sin_addr, 4);
        a->port = ntohs(s->sin_port);
        return OS_OK;
    }
    if (ss->ss_family == AF_INET6)
    {
        const struct sockaddr_in6 *s = (const struct sockaddr_in6 *)ss;
        a->family = 6;
        memcpy(a->ip, &s->sin6_addr, 16);
        a->port = ntohs(s->sin6_port);
        return OS_OK;
    }
    return OS_INVALID;
}

static int new_socket(int family, int type, int *fd)
{
    int s = socket(family == 4 ? AF_INET : AF_INET6, type | SOCK_CLOEXEC, 0);
    if (s < 0)
    {
        return last();
    }
    *fd = s;
    return OS_OK;
}

int os_tcp_bind(const os_addr *a, os_sock *out)
{
    int s = -1;
    int k = new_socket(a->family, SOCK_STREAM, &s);
    if (k != OS_OK)
    {
        return k;
    }
    int one = 1;
    if (setsockopt(s, SOL_SOCKET, SO_REUSEADDR, &one, sizeof one) != 0)
    {
        k = last();
        close(s);
        return k;
    }
    struct sockaddr_storage ss;
    socklen_t len = to_sockaddr(a, &ss);
    if (bind(s, (struct sockaddr *)&ss, len) != 0 || listen(s, 128) != 0)
    {
        k = last();
        close(s);
        return k;
    }
    /* D-0204: room for 4096 connections waiting to be accepted (the
       standard library asks for 128): a burst of clients larger than that
       had connections dropped, and retried a second later. */
    listen(s, 4096);
    *out = s;
    return OS_OK;
}

int os_udp_bind(const os_addr *a, os_sock *out)
{
    int s = -1;
    int k = new_socket(a->family, SOCK_DGRAM, &s);
    if (k != OS_OK)
    {
        return k;
    }
    struct sockaddr_storage ss;
    socklen_t len = to_sockaddr(a, &ss);
    if (bind(s, (struct sockaddr *)&ss, len) != 0)
    {
        k = last();
        close(s);
        return k;
    }
    *out = s;
    return OS_OK;
}

int os_sock_local(os_sock s, os_addr *a)
{
    struct sockaddr_storage ss;
    socklen_t len = sizeof ss;
    memset(&ss, 0, sizeof ss);
    if (getsockname((int)s, (struct sockaddr *)&ss, &len) != 0)
    {
        return last();
    }
    return from_sockaddr(&ss, a);
}

int os_sock_peer(os_sock s, os_addr *a)
{
    struct sockaddr_storage ss;
    socklen_t len = sizeof ss;
    memset(&ss, 0, sizeof ss);
    if (getpeername((int)s, (struct sockaddr *)&ss, &len) != 0)
    {
        return last();
    }
    return from_sockaddr(&ss, a);
}

int os_sock_nonblocking(os_sock s, int on)
{
    int v = on;
    return ioctl((int)s, FIONBIO, &v) == 0 ? OS_OK : last();
}

int os_accept(os_sock l, os_sock *out)
{
    for (;;)
    {
        struct sockaddr_storage ss;
        socklen_t len = sizeof ss;
        int s = accept4((int)l, (struct sockaddr *)&ss, &len, SOCK_CLOEXEC);
        if (s >= 0)
        {
            *out = s;
            return OS_OK;
        }
        if (errno != EINTR)
        {
            return last();
        }
    }
}

static int take_error(int s)
{
    int e = 0;
    socklen_t len = sizeof e;
    if (getsockopt(s, SOL_SOCKET, SO_ERROR, &e, &len) != 0)
    {
        return last();
    }
    /* "no error set after POLLHUP" is uncategorized. */
    return e ? kind_of(e) : OS_OTHER;
}

int os_connect(const os_addr *a, uint64_t ms, os_sock *out)
{
    int s = -1;
    int k = new_socket(a->family, SOCK_STREAM, &s);
    if (k != OS_OK)
    {
        return k;
    }
    struct sockaddr_storage ss;
    socklen_t len = to_sockaddr(a, &ss);
    if (ms == 0)
    {
        for (;;)
        {
            if (connect(s, (struct sockaddr *)&ss, len) == 0)
            {
                *out = s;
                return OS_OK;
            }
            if (errno != EINTR)
            {
                k = last();
                close(s);
                return k;
            }
        }
    }
    /* `connect_timeout`. */
    int one = 1, zero = 0;
    if (ioctl(s, FIONBIO, &one) != 0)
    {
        k = last();
        close(s);
        return k;
    }
    int r = connect(s, (struct sockaddr *)&ss, len);
    int e = errno;
    if (ioctl(s, FIONBIO, &zero) != 0)
    {
        k = last();
        close(s);
        return k;
    }
    if (r == 0)
    {
        *out = s;
        return OS_OK;
    }
    if (e != EINPROGRESS)
    {
        close(s);
        return kind_of(e);
    }
    uint64_t start = plat_now_ns();
    uint64_t total = ms * 1000000u;
    for (;;)
    {
        uint64_t el = plat_now_ns() - start;
        if (el >= total)
        {
            close(s);
            return OS_TIMED_OUT;
        }
        uint64_t left = total - el;
        uint64_t t = left / 1000000u;
        if (t == 0)
        {
            t = 1;
        }
        if (t > INT_MAX)
        {
            t = INT_MAX;
        }
        struct pollfd p = {s, POLLOUT, 0};
        int n = poll(&p, 1, (int)t);
        if (n < 0)
        {
            if (errno != EINTR)
            {
                k = last();
                close(s);
                return k;
            }
            continue;
        }
        if (n == 0)
        {
            continue;
        }
        /* Linux answers POLLOUT|POLLERR|POLLHUP for a refused connection. */
        if (p.revents & (POLLHUP | POLLERR))
        {
            k = take_error(s);
            close(s);
            return k;
        }
        *out = s;
        return OS_OK;
    }
}

int os_set_timeout(os_sock s, int write, uint64_t ms)
{
    struct timeval tv = {0, 0};
    if (ms)
    {
        uint64_t secs = ms / 1000;
        tv.tv_sec = secs > (uint64_t)INT64_MAX ? (time_t)INT64_MAX : (time_t)secs;
        tv.tv_usec = (suseconds_t)((ms % 1000) * 1000);
        if (tv.tv_sec == 0 && tv.tv_usec == 0)
        {
            tv.tv_usec = 1;
        }
    }
    if (setsockopt((int)s, SOL_SOCKET, write ? SO_SNDTIMEO : SO_RCVTIMEO, &tv, sizeof tv) != 0)
    {
        return last();
    }
    return OS_OK;
}

int os_get_timeout(os_sock s, int write, uint64_t *ns)
{
    struct timeval tv;
    socklen_t len = sizeof tv;
    if (getsockopt((int)s, SOL_SOCKET, write ? SO_SNDTIMEO : SO_RCVTIMEO, &tv, &len) != 0)
    {
        return last();
    }
    if (tv.tv_sec == 0 && tv.tv_usec == 0)
    {
        *ns = 0;
    }
    else
    {
        *ns = (uint64_t)tv.tv_sec * 1000000000u + (uint64_t)(uint32_t)tv.tv_usec * 1000u;
    }
    return OS_OK;
}

int os_set_nodelay(os_sock s, int on)
{
    int v = on ? 1 : 0;
    return setsockopt((int)s, IPPROTO_TCP, TCP_NODELAY, &v, sizeof v) == 0 ? OS_OK : last();
}

int os_set_keepalive(os_sock s, int on)
{
    int v = on ? 1 : 0;
    return setsockopt((int)s, SOL_SOCKET, SO_KEEPALIVE, &v, 4) == 0 ? OS_OK : OS_OTHER;
}

int os_recv(os_sock s, uint8_t *p, size_t n, size_t *got)
{
    ssize_t k = recv((int)s, p, n, 0);
    if (k < 0)
    {
        *got = 0;
        return last();
    }
    *got = (size_t)k;
    return OS_OK;
}

int os_send(os_sock s, const uint8_t *p, size_t n, size_t *put)
{
    ssize_t k = send((int)s, p, n, MSG_NOSIGNAL);
    if (k < 0)
    {
        *put = 0;
        return last();
    }
    *put = (size_t)k;
    return OS_OK;
}

int os_shutdown_write(os_sock s)
{
    return shutdown((int)s, SHUT_WR) == 0 ? OS_OK : last();
}

void os_sock_close(os_sock s)
{
    close((int)s);
}

int os_sendto(os_sock s, const uint8_t *p, size_t n, const os_addr *a, size_t *put)
{
    struct sockaddr_storage ss;
    socklen_t len = to_sockaddr(a, &ss);
    ssize_t k = sendto((int)s, p, n, MSG_NOSIGNAL, (struct sockaddr *)&ss, len);
    if (k < 0)
    {
        *put = 0;
        return last();
    }
    *put = (size_t)k;
    return OS_OK;
}

int os_recvfrom(os_sock s, uint8_t *p, size_t n, size_t *got, os_addr *from)
{
    struct sockaddr_storage ss;
    socklen_t len = sizeof ss;
    memset(&ss, 0, sizeof ss);
    ssize_t k = recvfrom((int)s, p, n, 0, (struct sockaddr *)&ss, &len);
    if (k < 0)
    {
        *got = 0;
        return last();
    }
    *got = (size_t)k;
    return from_sockaddr(&ss, from);
}

int os_udp_connect(os_sock s, const os_addr *a)
{
    struct sockaddr_storage ss;
    socklen_t len = to_sockaddr(a, &ss);
    for (;;)
    {
        if (connect((int)s, (struct sockaddr *)&ss, len) == 0)
        {
            return OS_OK;
        }
        if (errno != EINTR)
        {
            return last();
        }
    }
}

int os_sock_dup(os_sock s, os_sock *out)
{
    int d = fcntl((int)s, F_DUPFD_CLOEXEC, 3);
    if (d < 0)
    {
        return last();
    }
    *out = d;
    return OS_OK;
}

int os_resolve(const char *host, uint16_t port, os_addr **out, size_t *count)
{
    struct addrinfo hints, *res = NULL;
    memset(&hints, 0, sizeof hints);
    hints.ai_socktype = SOCK_STREAM;
    *out = NULL;
    *count = 0;
    if (getaddrinfo(host, NULL, &hints, &res) != 0)
    {
        return OS_NOT_FOUND;
    }
    size_t n = 0;
    for (struct addrinfo *p = res; p; p = p->ai_next)
    {
        n++;
    }
    os_addr *v = (os_addr *)cb_xcalloc(n ? n : 1, sizeof(os_addr));
    size_t k = 0;
    for (struct addrinfo *p = res; p; p = p->ai_next)
    {
        struct sockaddr_storage ss;
        memset(&ss, 0, sizeof ss);
        if (!p->ai_addr || p->ai_addrlen > sizeof ss)
        {
            continue;
        }
        memcpy(&ss, p->ai_addr, p->ai_addrlen);
        if (from_sockaddr(&ss, &v[k]) == OS_OK)
        {
            v[k].port = port;
            k++;
        }
    }
    freeaddrinfo(res);
    *out = v;
    *count = k;
    return OS_OK;
}

/* D-0204: `SIGURG` (which nothing else here uses) is blocked in every
   thread from the start -- threads inherit it -- and let through only
   inside `ppoll`: one sent after the waiter last looked for a cancellation
   is pending, and interrupts the `ppoll` as it begins. */
static int wake_on;

static void woken(int sig)
{
    (void)sig;
}

void os_wake_init(void)
{
    sigset_t set;
    sigemptyset(&set);
    sigaddset(&set, SIGURG);
    if (pthread_sigmask(SIG_BLOCK, &set, NULL) != 0)
    {
        return;
    }
    /* As `signal(2)` sets it: SA_RESTART (no call it reaches restarts:
       it is let through only inside `ppoll`). */
    struct sigaction sa;
    memset(&sa, 0, sizeof sa);
    sa.sa_handler = woken;
    sa.sa_flags = SA_RESTART;
    sigemptyset(&sa.sa_mask);
    sigaddset(&sa.sa_mask, SIGURG);
    sigaction(SIGURG, &sa, NULL);
    __atomic_store_n(&wake_on, 1, __ATOMIC_RELAXED);
}

int os_wake_enabled(void)
{
    return __atomic_load_n(&wake_on, __ATOMIC_RELAXED);
}

uint64_t os_wake_thread_self(void)
{
    return (uint64_t)pthread_self();
}

void os_wake_thread(uint64_t t)
{
    if (os_wake_enabled() && t != 0)
    {
        pthread_kill((pthread_t)t, SIGURG);
    }
}

int os_ready(os_sock s, int read, int ms, int *ready)
{
    struct pollfd p = {(int)s, (short)(read ? POLLIN : POLLOUT), 0};
    int n;
    if (os_wake_enabled())
    {
        struct timespec ts;
        int m = ms < 0 ? 0 : ms;
        ts.tv_sec = m / 1000;
        ts.tv_nsec = (long)(m % 1000) * 1000000L;
        sigset_t mask;
        pthread_sigmask(SIG_BLOCK, NULL, &mask);
        sigdelset(&mask, SIGURG);
        n = ppoll(&p, 1, ms < 0 ? NULL : &ts, &mask);
    }
    else
    {
        n = poll(&p, 1, ms);
    }
    if (n < 0)
    {
        return last();
    }
    *ready = n > 0;
    return OS_OK;
}

/* ---- random bytes ---- */

int os_random(uint8_t *p, size_t n)
{
    size_t at = 0;
    while (at < n)
    {
        long k = syscall(SYS_getrandom, p + at, n - at, 0u);
        if (k < 0)
        {
            if (errno == EINTR)
            {
                continue;
            }
            if (at != 0)
            {
                return 0;
            }
            /* ENOSYS: a kernel before 3.17; the device will do. */
            int fd = -1;
            if (open_retry("/dev/urandom", O_RDONLY, 0, &fd) != OS_OK)
            {
                return 0;
            }
            size_t got = 0;
            while (got < n)
            {
                ssize_t r = read(fd, p + got, n - got);
                if (r < 0 && errno == EINTR)
                {
                    continue;
                }
                if (r <= 0)
                {
                    close(fd);
                    return 0;
                }
                got += (size_t)r;
            }
            close(fd);
            return 1;
        }
        at += (size_t)k;
    }
    return 1;
}

/* ---- the stack's bounds and the program's own stack ---- */

#include <sys/mman.h>

/* ---- the stack (D-0077) ---- */

/* The calling thread's stack from its attributes (glibc's
   `pthread_getattr_np`; the main thread's is the stack's mapping up to its
   rlimit). */
int plat_thread_stack(uintptr_t *lo, size_t *size)
{
#ifdef __linux__
    pthread_attr_t attr;
    void *addr = NULL;
    size_t n = 0;
    if (pthread_getattr_np(pthread_self(), &attr) != 0)
    {
        return 0;
    }
    int ok = pthread_attr_getstack(&attr, &addr, &n) == 0;
    pthread_attr_destroy(&attr);
    if (!ok || addr == NULL)
    {
        return 0;
    }
    *lo = (uintptr_t)addr;
    *size = n;
    return 1;
#else
    (void)lo;
    (void)size;
    return 0;
#endif
}

/* Bug 032: SIGSEGV for an access below the calling thread's stack floor.
   Each watched thread has a signal stack of its own (reserved, not
   committed) and its bounds; the handler runs there. An access it does not
   recognise goes to the handler that was there before (a sanitizer's), or
   ends the process as SIGSEGV did. */
#define WATCH_ALT_STACK ((size_t)1 << 20)
/* How far below the stack's lowest address an access still counts as the
   stack's: its guard, and a probe just past it. */
#define WATCH_BELOW ((uintptr_t)1 << 20)

static __thread uintptr_t watch_lo;
static __thread uintptr_t watch_floor;
static __thread int watch_alt;
static void (*watch_overflow)(void);
static struct sigaction watch_before;
static int watch_installed;
static plat_mutex watch_lock = PLAT_MUTEX_INIT;

static void on_segv(int sig, siginfo_t *si, void *uc)
{
    (void)uc;
    uintptr_t a = (uintptr_t)si->si_addr;
    uintptr_t lo = watch_lo;
    uintptr_t fl = watch_floor;
    if (fl > 1 && a < fl && a + WATCH_BELOW >= lo && watch_overflow)
    {
        /* The fault is raised here, on the signal stack (SA_NODEFER: a
           second SIGSEGV is not blocked meanwhile). */
        watch_overflow();
    }
    /* Not an overflow: as before this handler, the access faults again
       when the handler returns. */
    sigaction(sig, &watch_before, NULL);
}

void plat_watch_stack(uintptr_t lo, uintptr_t floor, void (*on_overflow)(void))
{
    if (!watch_alt)
    {
        void *p = mmap(NULL, WATCH_ALT_STACK, PROT_READ | PROT_WRITE, MAP_PRIVATE | MAP_ANONYMOUS | MAP_NORESERVE, -1, 0);
        if (p == MAP_FAILED || p == NULL)
        {
            return;
        }
        stack_t ss;
        memset(&ss, 0, sizeof ss);
        ss.ss_sp = p;
        ss.ss_size = WATCH_ALT_STACK;
        if (sigaltstack(&ss, NULL) != 0)
        {
            munmap(p, WATCH_ALT_STACK);
            return;
        }
        watch_alt = 1;
    }
    watch_lo = lo;
    watch_floor = floor;
    plat_mutex_lock(&watch_lock);
    if (!watch_installed)
    {
        watch_overflow = on_overflow;
        struct sigaction sa;
        memset(&sa, 0, sizeof sa);
        sa.sa_sigaction = on_segv;
        sa.sa_flags = SA_SIGINFO | SA_ONSTACK | SA_NODEFER;
        sigemptyset(&sa.sa_mask);
        if (sigaction(SIGSEGV, &sa, &watch_before) == 0)
        {
            watch_installed = 1;
        }
    }
    plat_mutex_unlock(&watch_lock);
}

/* The thread ends: its signal stack is taken from it and unmapped (a
   thread that was watched and ends would otherwise leave a mapping
   behind). */
void plat_unwatch_stack(void)
{
    if (!watch_alt)
    {
        return;
    }
    stack_t cur;
    if (sigaltstack(NULL, &cur) == 0 && cur.ss_sp != NULL)
    {
        stack_t off;
        memset(&off, 0, sizeof off);
        off.ss_flags = SS_DISABLE;
        if (sigaltstack(&off, NULL) == 0)
        {
            munmap(cur.ss_sp, WATCH_ALT_STACK);
        }
    }
    watch_alt = 0;
    watch_floor = 0;
}

/* The signal stack serves again: nothing to make again. */
void plat_stack_recovered(void)
{
}

/* The program's own stack (`run_on_own_stack`): reserved, not committed,
   a guard below it. */
uintptr_t plat_reserve_stack(size_t size, size_t guard)
{
#if defined(__linux__)
    void *p = mmap(NULL, size + guard, PROT_READ | PROT_WRITE, MAP_PRIVATE | MAP_ANONYMOUS | MAP_NORESERVE | MAP_STACK, -1, 0);
    if (p == MAP_FAILED || p == NULL)
    {
        return 0;
    }
    mprotect(p, guard, PROT_NONE);
    return (uintptr_t)p + guard;
#else
    (void)size;
    (void)guard;
    return 0;
#endif
}

#endif
