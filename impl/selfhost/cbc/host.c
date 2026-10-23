/* What cbc's driver needs of the platform that `std` does not offer
   (impl/selfhost/cbc/driver.cb): whether a stream is a terminal, standard
   input gathered as it arrives by a thread nothing waits for, and a guard
   on a started program, and on Unix a program started with arguments that
   need not be UTF-8. cobc does these in Rust (`std::io::IsTerminal`, a
   detached `std::thread`, `child_guard`/`job_guard` in cobc/src/main.rs).
   Plain C for Linux (glibc) and Windows (MinGW-w64), no other library. */

#ifdef _WIN32
#undef _WIN32_WINNT
#define _WIN32_WINNT 0x0601   /* Windows 7: condition variables */
#else
#define _GNU_SOURCE   /* prlimit */
#endif
#include <stdint.h>
#include <stdlib.h>
#include <string.h>

#ifdef _WIN32
#include <windows.h>
#include <io.h>
#else
#include <pthread.h>
#include <unistd.h>
#include <sys/resource.h>
#include <sys/types.h>
#include <time.h>
#include <errno.h>
#include <poll.h>
#include <sys/wait.h>
#include <signal.h>
#include <stdio.h>
#ifdef __linux__
#include <sys/prctl.h>
#endif
#endif

/* ---- terminals ---- */

int64_t cbc_host_isatty(int64_t fd)
{
#ifdef _WIN32
    return _isatty((int)fd) ? 1 : 0;
#else
    return isatty((int)fd) ? 1 : 0;
#endif
}

/* Older Windows consoles print ANSI sequences raw unless asked to process
   them (cobc's `diagnostics::enable_ansi`); elsewhere nothing to do. */
int64_t cbc_host_enable_ansi(void)
{
#ifdef _WIN32
    HANDLE h = GetStdHandle(STD_ERROR_HANDLE);
    DWORD mode = 0;
    if (!GetConsoleMode(h, &mode))
    {
        return 0;
    }
    if (mode & 0x0004)
    {
        return 1;
    }
    return SetConsoleMode(h, mode | 0x0004) ? 1 : 0;
#else
    return 1;
#endif
}

/* Whether standard output is a pipe nobody reads any more: where cobc's
   `print!` fails (EPIPE) and it ends quietly with status 1, a CobaltC
   program's output is dropped without a word. Linux: the pipe's write end
   polls as an error; elsewhere never known. */
int64_t cbc_host_stdout_closed(void)
{
#ifdef _WIN32
    return 0;
#else
    struct pollfd p;
    p.fd = 1;
    p.events = POLLOUT;
    p.revents = 0;
    if (poll(&p, 1, 0) == 1 && (p.revents & POLLERR))
    {
        return 1;
    }
    return 0;
#endif
}

/* ---- standard input, gathered as it arrives ---- */

static uint8_t *in_buf;
static uint64_t in_len, in_cap;
static int in_ended;

#ifdef _WIN32
static CRITICAL_SECTION in_lock;
static CONDITION_VARIABLE in_cond;
#define IN_LOCK() EnterCriticalSection(&in_lock)
#define IN_UNLOCK() LeaveCriticalSection(&in_lock)
#define IN_WAKE() WakeAllConditionVariable(&in_cond)
#else
static pthread_mutex_t in_lock = PTHREAD_MUTEX_INITIALIZER;
static pthread_cond_t in_cond = PTHREAD_COND_INITIALIZER;
#define IN_LOCK() pthread_mutex_lock(&in_lock)
#define IN_UNLOCK() pthread_mutex_unlock(&in_lock)
#define IN_WAKE() pthread_cond_broadcast(&in_cond)
#endif

static void in_add(const uint8_t *p, uint64_t n)
{
    IN_LOCK();
    if (n == 0)
    {
        in_ended = 1;
    }
    else
    {
        if (in_len + n > in_cap)
        {
            uint64_t cap = in_cap ? in_cap : 65536;
            while (cap < in_len + n)
            {
                cap *= 2;
            }
            uint8_t *b = (uint8_t *)realloc(in_buf, cap);
            if (!b)
            {
                in_ended = 1;
                IN_UNLOCK();
                IN_WAKE();
                return;
            }
            in_buf = b;
            in_cap = cap;
        }
        memcpy(in_buf + in_len, p, n);
        in_len += n;
    }
    IN_UNLOCK();
    IN_WAKE();
}

#ifdef _WIN32
static DWORD WINAPI in_gather(LPVOID arg)
{
    (void)arg;
    static uint8_t chunk[65536];
    HANDLE h = GetStdHandle(STD_INPUT_HANDLE);
    for (;;)
    {
        DWORD n = 0;
        if (!ReadFile(h, chunk, sizeof chunk, &n, NULL))
        {
            n = 0;
        }
        in_add(chunk, n);
        if (n == 0)
        {
            return 0;
        }
    }
}
#else
static void *in_gather(void *arg)
{
    (void)arg;
    static uint8_t chunk[65536];
    for (;;)
    {
        ssize_t n = read(0, chunk, sizeof chunk);
        if (n < 0 && errno == EINTR)
        {
            continue;
        }
        if (n < 0)
        {
            n = 0;
        }
        in_add(chunk, (uint64_t)n);
        if (n == 0)
        {
            return NULL;
        }
    }
}
#endif

/* Starts gathering standard input (once), or, for a terminal, takes it as
   ended at once. */
void cbc_host_input_start(void)
{
    static int started;
    if (started)
    {
        return;
    }
    started = 1;
#ifdef _WIN32
    InitializeCriticalSection(&in_lock);
    InitializeConditionVariable(&in_cond);
#endif
    if (cbc_host_isatty(0))
    {
        in_ended = 1;
        return;
    }
#ifdef _WIN32
    HANDLE t = CreateThread(NULL, 0, in_gather, NULL, 0, NULL);
    if (t == NULL)
    {
        in_ended = 1;
    }
    else
    {
        CloseHandle(t);
    }
#else
    pthread_t t;
    if (pthread_create(&t, NULL, in_gather, NULL) != 0)
    {
        in_ended = 1;
    }
    else
    {
        pthread_detach(t);
    }
#endif
}

/* Waits up to `ms` milliseconds for more than `have` bytes, or the end;
   the number of bytes gathered so far, with 2^63 added once the input has
   ended. */
uint64_t cbc_host_input_wait(uint64_t have, uint64_t ms)
{
    IN_LOCK();
    if (in_len == have && !in_ended)
    {
#ifdef _WIN32
        SleepConditionVariableCS(&in_cond, &in_lock, (DWORD)ms);
#else
        struct timespec at;
        clock_gettime(CLOCK_REALTIME, &at);
        at.tv_sec += (time_t)(ms / 1000);
        at.tv_nsec += (long)((ms % 1000) * 1000000);
        if (at.tv_nsec >= 1000000000L)
        {
            at.tv_sec += 1;
            at.tv_nsec -= 1000000000L;
        }
        pthread_cond_timedwait(&in_cond, &in_lock, &at);
#endif
    }
    uint64_t r = in_len | (in_ended ? (UINT64_C(1) << 63) : 0);
    IN_UNLOCK();
    return r;
}

/* Byte `i` of what has been gathered (below the length `cbc_host_input_wait`
   gave). */
int64_t cbc_host_input_byte(uint64_t i)
{
    IN_LOCK();
    int64_t b = i < in_len ? (int64_t)in_buf[i] : -1;
    IN_UNLOCK();
    return b;
}

/* ---- a started program's guard ---- */

/* cobc's `child_guard` and `job_guard` for a program already started (std
   has no hook between fork and exec): on Linux its CPU time limited to
   `secs` seconds (RLIMIT_CPU, soft `secs`, hard `secs + 1`) when `secs`
   >= 0; on Windows the program put in a job object that ends it when cbc
   ends, with that much processor time if set. A handle to release with
   `cbc_host_unguard`, or 0. Best effort, as cobc's: where it cannot be
   done the program runs without. */
uint64_t cbc_host_guard(uint64_t pid, int64_t secs)
{
#ifdef _WIN32
    HANDLE job = CreateJobObjectW(NULL, NULL);
    if (job == NULL)
    {
        return 0;
    }
    JOBOBJECT_EXTENDED_LIMIT_INFORMATION info;
    memset(&info, 0, sizeof info);
    info.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
    if (secs >= 0)
    {
        info.BasicLimitInformation.LimitFlags |= JOB_OBJECT_LIMIT_PROCESS_TIME;
        info.BasicLimitInformation.PerProcessUserTimeLimit.QuadPart = secs * 10000000LL;
    }
    HANDLE proc = OpenProcess(PROCESS_SET_QUOTA | PROCESS_TERMINATE, FALSE, (DWORD)pid);
    if (proc == NULL || !SetInformationJobObject(job, JobObjectExtendedLimitInformation, &info, sizeof info)
        || !AssignProcessToJobObject(job, proc))
    {
        if (proc != NULL)
        {
            CloseHandle(proc);
        }
        CloseHandle(job);
        return 0;
    }
    CloseHandle(proc);
    return (uint64_t)(uintptr_t)job;
#else
    if (secs >= 0)
    {
        struct rlimit r;
        r.rlim_cur = (rlim_t)secs;
        r.rlim_max = (rlim_t)secs + 1;
        prlimit((pid_t)pid, RLIMIT_CPU, &r, NULL);
    }
    return 0;
#endif
}

void cbc_host_unguard(uint64_t h)
{
#ifdef _WIN32
    if (h != 0)
    {
        CloseHandle((HANDLE)(uintptr_t)h);
    }
#else
    (void)h;
#endif
}

/* ---- running a built program (Unix) ---- */

#ifndef _WIN32
/* The runtime's arguments (cbrt `cb_arg_bytes`, std's `arg_bytes`): the
   bytes of argument `i` (after the program's name) into [p, p + n), its
   length; < 0 when there is no argument `i`. */
extern int64_t cb_arg_bytes(uint64_t i, uint8_t *p, uint64_t n);

static uint8_t *run_path;
static uint64_t run_path_len, run_path_cap;

/* The path of the program `cbc_host_run` starts, a byte at a time. */
void cbc_host_run_path_byte(int64_t b)
{
    if (b < 0)
    {
        run_path_len = 0;
        return;
    }
    if (run_path_len + 2 > run_path_cap)
    {
        run_path_cap = run_path_cap * 2 + 64;
        run_path = realloc(run_path, run_path_cap);
        if (run_path == NULL)
        {
            abort();
        }
    }
    run_path[run_path_len++] = (uint8_t)b;
    run_path[run_path_len] = 0;
}

/* `--run`: the program at the path given, with cbc's own arguments from
   `first` on as its arguments, byte for byte (an argument need not be
   UTF-8), started as cobc starts it (`child_guard`: ended when cbc ends,
   `secs` seconds of CPU time when >= 0, `COBALTC_SCHEDULE_SEED` set to `seed`
   when >= 0), its streams cbc's. Its exit status, minus its signal when a
   signal ended it, or -1000 when it could not be started. */
int64_t cbc_host_run(uint64_t first, int64_t secs, int64_t seed)
{
    int64_t n = 0;
    while (cb_arg_bytes(first + (uint64_t)n, NULL, 0) >= 0)
    {
        n += 1;
    }
    char **argv = calloc((size_t)n + 2, sizeof(char *));
    if (argv == NULL || run_path == NULL)
    {
        return -1000;
    }
    argv[0] = (char *)run_path;
    for (int64_t i = 0; i < n; i++)
    {
        int64_t len = cb_arg_bytes(first + (uint64_t)i, NULL, 0);
        char *a = malloc((size_t)len + 1);
        if (a == NULL)
        {
            return -1000;
        }
        cb_arg_bytes(first + (uint64_t)i, (uint8_t *)a, (uint64_t)len);
        a[len] = 0;
        argv[i + 1] = a;
    }
    char seed_text[32];
    snprintf(seed_text, sizeof seed_text, "%lld", (long long)seed);
    pid_t parent = getpid();
    pid_t pid = fork();
    if (pid < 0)
    {
        return -1000;
    }
    if (pid == 0)
    {
#ifdef __linux__
        if (prctl(PR_SET_PDEATHSIG, SIGKILL, 0, 0, 0) != 0 || getppid() != parent)
        {
            _exit(127);
        }
#else
        (void)parent;
#endif
        if (secs >= 0)
        {
            struct rlimit r;
            r.rlim_cur = (rlim_t)secs;
            r.rlim_max = (rlim_t)secs + 1;
            if (setrlimit(RLIMIT_CPU, &r) != 0)
            {
                _exit(127);
            }
        }
        if (seed >= 0)
        {
            setenv("COBALTC_SCHEDULE_SEED", seed_text, 1);
        }
        execvp(argv[0], argv);
        _exit(127);
    }
    for (int64_t i = 0; i < n; i++)
    {
        free(argv[i + 1]);
    }
    free(argv);
    int status = 0;
    while (waitpid(pid, &status, 0) < 0)
    {
        if (errno != EINTR)
        {
            return -1000;
        }
    }
    if (WIFSIGNALED(status))
    {
        return -(int64_t)WTERMSIG(status);
    }
    return (int64_t)WEXITSTATUS(status);
}
#else
/* Windows: a command line is UTF-16, so every argument is text and std's
   commands pass it on; never called there. */
void cbc_host_run_path_byte(int64_t b)
{
    (void)b;
}

int64_t cbc_host_run(uint64_t first, int64_t secs, int64_t seed)
{
    (void)first;
    (void)secs;
    (void)seed;
    return -1000;
}
#endif
