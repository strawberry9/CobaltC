/* What cobc's driver needs of the platform that `std` does not offer
   (impl/cobc/driver.cb): whether a stream is a terminal, standard input
   gathered as it arrives by a thread nothing waits for, and a guard on a
   started program, and on Unix a program started with arguments that need not
   be UTF-8; and a stack large enough for the nesting of the program the
   compiler reads. Plain C for Linux (glibc) and Windows (MinGW-w64), no other
   library. */

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

/* ---- the stack ---- */

/* The parser, the checker and the lowering recurse on the nesting of the
   program they read (a 100 000-deep parenthesised expression, a
   100 000-term sum): the compiler asks the runtime, before `main` starts,
   for a larger stack than a program's 256 MiB (impl/cbrt/src/rt_threads.c;
   reserved, so only what a program's nesting uses is committed). On ELF
   the reference is weak: impl/build.sh's first stage after a change of the
   runtime links this file with the runtime before it, which may not have
   the function, and that compiler then runs on the usual stack. */
#ifdef _WIN32
void cbrt_request_stack(size_t bytes);
#else
void cbrt_request_stack(size_t bytes) __attribute__((weak));
#endif

#if UINTPTR_MAX > 0xffffffffu
__attribute__((constructor)) static void cobc_host_stack(void)
{
#ifndef _WIN32
    if (!cbrt_request_stack)
    {
        return;
    }
#endif
    cbrt_request_stack((size_t)4 << 30);
}
#endif

/* ---- terminals ---- */

int64_t cobc_host_isatty(int64_t fd)
{
#ifdef _WIN32
    return _isatty((int)fd) ? 1 : 0;
#else
    return isatty((int)fd) ? 1 : 0;
#endif
}

/* Older Windows consoles print ANSI sequences raw unless asked to process
   them; elsewhere nothing to do. */
int64_t cobc_host_enable_ansi(void)
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

/* Whether standard output is a pipe nobody reads any more: a CobaltC
   program's output to one is dropped without a word, so the driver asks, and
   ends quietly with status 1. Linux: the pipe's write end polls as an error;
   elsewhere never known. */
int64_t cobc_host_stdout_closed(void)
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
void cobc_host_input_start(void)
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
    if (cobc_host_isatty(0))
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
uint64_t cobc_host_input_wait(uint64_t have, uint64_t ms)
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

/* Byte `i` of what has been gathered (below the length `cobc_host_input_wait`
   gave). */
int64_t cobc_host_input_byte(uint64_t i)
{
    IN_LOCK();
    int64_t b = i < in_len ? (int64_t)in_buf[i] : -1;
    IN_UNLOCK();
    return b;
}

/* ---- a started program's guard ---- */

/* The guard on a program already started (std has no hook between fork and
   exec): on Linux its CPU time limited to `secs` seconds (RLIMIT_CPU, soft
   `secs`, hard `secs + 1`) when `secs` >= 0; on Windows the program put in a
   job object that ends it when cobc ends, with that much processor time if
   set. A handle to release with `cobc_host_unguard`, or 0. Best effort: where
   it cannot be done the program runs without. */
uint64_t cobc_host_guard(uint64_t pid, int64_t secs)
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

/* Report 314: ends the started program `pid` at once (`--explore`'s run
   that did not finish in time). */
void cobc_host_kill(uint64_t pid)
{
#ifdef _WIN32
    HANDLE proc = OpenProcess(PROCESS_TERMINATE, FALSE, (DWORD)pid);
    if (proc != NULL)
    {
        TerminateProcess(proc, 1);
        CloseHandle(proc);
    }
#else
    if (pid != 0)
    {
        kill((pid_t)pid, SIGKILL);
    }
#endif
}

void cobc_host_unguard(uint64_t h)
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

/* ---- the temporary directories (bug 037) ----

   From the moment cobc makes a directory of its own (`--run`'s, the object
   directory), an interrupt does not leave it behind. Unix: SIGINT, SIGTERM
   and SIGHUP (those not ignored) remove every directory watched, then end
   cobc by the same signal; a program started under `--explore`
   (`cobc_host_watch_child`) gets the signal first, unless it came from the
   terminal, which sent it to the program too. `--run`'s own run
   (`cobc_host_run`) holds the signals itself and the driver removes the
   directory when the program has ended. Windows: a console handler, for
   Ctrl-C, Ctrl-Break and the console's closing, which every process on the
   console gets: it waits for the programs cobc started to end (each gets the
   event too; on a close at most three seconds, then they are ended), removes
   the directories, and ends cobc with the status the default handler gives
   (STATUS_CONTROL_C_EXIT). A directory already removed is passed over. */

#define WATCH_MAX 4

static uint8_t *watch_path;
static uint64_t watch_path_len, watch_path_cap;

/* The path of the directory `cobc_host_watch_commit` watches, a byte at a
   time (-1: begin again). */
void cobc_host_watch_byte(int64_t b)
{
    if (b < 0)
    {
        watch_path_len = 0;
        return;
    }
    if (watch_path_len + 2 > watch_path_cap)
    {
        watch_path_cap = watch_path_cap * 2 + 64;
        watch_path = realloc(watch_path, watch_path_cap);
        if (watch_path == NULL)
        {
            abort();
        }
    }
    watch_path[watch_path_len++] = (uint8_t)b;
    watch_path[watch_path_len] = 0;
}

#ifdef _WIN32
#include <tlhelp32.h>

static wchar_t *watch_dirs[WATCH_MAX];
static volatile LONG watch_count;
static volatile LONG console_stop;
static HANDLE console_done;
static int console_installed;

/* `dir` and all it holds, removed; 0 when something is left. */
static int remove_tree_w(const wchar_t *dir)
{
    size_t n = wcslen(dir);
    wchar_t *pat = malloc((n + 3) * sizeof(wchar_t));
    if (pat == NULL)
    {
        return 0;
    }
    memcpy(pat, dir, n * sizeof(wchar_t));
    pat[n] = L'\\';
    pat[n + 1] = L'*';
    pat[n + 2] = 0;
    WIN32_FIND_DATAW fd;
    HANDLE f = FindFirstFileW(pat, &fd);
    free(pat);
    if (f != INVALID_HANDLE_VALUE)
    {
        do
        {
            if (wcscmp(fd.cFileName, L".") == 0 || wcscmp(fd.cFileName, L"..") == 0)
            {
                continue;
            }
            size_t m = wcslen(fd.cFileName);
            wchar_t *p = malloc((n + m + 2) * sizeof(wchar_t));
            if (p == NULL)
            {
                continue;
            }
            memcpy(p, dir, n * sizeof(wchar_t));
            p[n] = L'\\';
            memcpy(p + n + 1, fd.cFileName, (m + 1) * sizeof(wchar_t));
            if ((fd.dwFileAttributes & FILE_ATTRIBUTE_DIRECTORY) && !(fd.dwFileAttributes & FILE_ATTRIBUTE_REPARSE_POINT))
            {
                remove_tree_w(p);
            }
            else
            {
                if (fd.dwFileAttributes & FILE_ATTRIBUTE_READONLY)
                {
                    SetFileAttributesW(p, fd.dwFileAttributes & ~(DWORD)FILE_ATTRIBUTE_READONLY);
                }
                DeleteFileW(p);
            }
            free(p);
        } while (FindNextFileW(f, &fd));
        FindClose(f);
    }
    if (RemoveDirectoryW(dir))
    {
        return 1;
    }
    return GetFileAttributesW(dir) == INVALID_FILE_ATTRIBUTES;
}

/* The programs cobc started (their parent is cobc, and they began after
   it: a number the system gave again to another process is not one),
   waited for up to `ms` milliseconds, then ended. */
static void wait_children(DWORD ms)
{
    FILETIME mine_c, x1, x2, x3;
    if (!GetProcessTimes(GetCurrentProcess(), &mine_c, &x1, &x2, &x3))
    {
        return;
    }
    HANDLE snap = CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0);
    if (snap == INVALID_HANDLE_VALUE)
    {
        return;
    }
    HANDLE kids[MAXIMUM_WAIT_OBJECTS];
    DWORD nk = 0;
    DWORD self = GetCurrentProcessId();
    PROCESSENTRY32W pe;
    pe.dwSize = sizeof pe;
    if (Process32FirstW(snap, &pe))
    {
        do
        {
            if (pe.th32ParentProcessID != self || pe.th32ProcessID == self || nk == MAXIMUM_WAIT_OBJECTS)
            {
                continue;
            }
            HANDLE h = OpenProcess(SYNCHRONIZE | PROCESS_TERMINATE | PROCESS_QUERY_LIMITED_INFORMATION, FALSE, pe.th32ProcessID);
            if (h == NULL)
            {
                continue;
            }
            FILETIME c;
            if (!GetProcessTimes(h, &c, &x1, &x2, &x3) || CompareFileTime(&c, &mine_c) < 0)
            {
                CloseHandle(h);
                continue;
            }
            kids[nk++] = h;
        } while (Process32NextW(snap, &pe));
    }
    CloseHandle(snap);
    if (nk == 0)
    {
        return;
    }
    if (WaitForMultipleObjects(nk, kids, TRUE, ms) != WAIT_OBJECT_0)
    {
        for (DWORD i = 0; i < nk; i++)
        {
            if (WaitForSingleObject(kids[i], 0) != WAIT_OBJECT_0)
            {
                TerminateProcess(kids[i], (UINT)STATUS_CONTROL_C_EXIT);
            }
        }
        WaitForMultipleObjects(nk, kids, TRUE, 2000);
    }
    for (DWORD i = 0; i < nk; i++)
    {
        CloseHandle(kids[i]);
    }
}

static BOOL WINAPI on_console_stop(DWORD ev)
{
    if (ev != CTRL_C_EVENT && ev != CTRL_BREAK_EVENT && ev != CTRL_CLOSE_EVENT)
    {
        return FALSE;
    }
    if (InterlockedExchange(&console_stop, (LONG)ev + 1) != 0)
    {
        /* A second event while the first is handled: the first ends cobc
           (on a close the system would end it when this returns). */
        WaitForSingleObject(console_done, 10000);
        return TRUE;
    }
    wait_children(ev == CTRL_CLOSE_EVENT ? 3000 : INFINITE);
    /* An executable that has just ended may stay locked a moment. */
    LONG n = watch_count;
    for (int tries = 0; tries < 40; tries++)
    {
        int left = 0;
        for (LONG i = 0; i < n; i++)
        {
            if (watch_dirs[i] != NULL && !remove_tree_w(watch_dirs[i]))
            {
                left = 1;
            }
        }
        if (!left)
        {
            break;
        }
        Sleep(50);
    }
    SetEvent(console_done);
    ExitProcess((UINT)STATUS_CONTROL_C_EXIT);
    return TRUE;
}

/* cobc ending of itself while the handler works (a build the interrupt
   stopped): it waits for the directories to go. */
static void console_exit_hook(void)
{
    if (console_stop != 0)
    {
        WaitForSingleObject(console_done, 30000);
        ExitProcess((UINT)STATUS_CONTROL_C_EXIT);
    }
}

void cobc_host_watch_commit(void)
{
    if (watch_count >= WATCH_MAX || watch_path_len == 0)
    {
        return;
    }
    int n = MultiByteToWideChar(CP_UTF8, 0, (const char *)watch_path, -1, NULL, 0);
    if (n <= 0)
    {
        return;
    }
    wchar_t *w = malloc((size_t)n * sizeof(wchar_t));
    if (w == NULL)
    {
        return;
    }
    MultiByteToWideChar(CP_UTF8, 0, (const char *)watch_path, -1, w, n);
    for (int i = 0; i < n; i++)
    {
        if (w[i] == L'/')
        {
            w[i] = L'\\';
        }
    }
    watch_dirs[watch_count] = w;
    InterlockedIncrement(&watch_count);
    if (!console_installed)
    {
        console_installed = 1;
        console_done = CreateEventW(NULL, TRUE, FALSE, NULL);
        atexit(console_exit_hook);
        SetConsoleCtrlHandler(on_console_stop, TRUE);
    }
}

void cobc_host_watch_child(uint64_t pid)
{
    /* The handler finds cobc's programs itself (`wait_children`). */
    (void)pid;
}

/* `--run` with a processor time limit (COBALTC_CPU_LIMIT) on Windows: the
   program std's `Command::status` starts has no number to give, so a thread
   looks for cobc's children (as `wait_children` does) while it runs and puts
   each in a job that ends it when its processor time is up, and when cobc
   ends. A program is in the job a few milliseconds after it starts. */
static HANDLE time_job;
static HANDLE time_thread;
static volatile LONG time_stop;
static volatile LONG time_hit;
static int64_t time_secs;
#define TIME_QUOTA 1816 /* ERROR_NOT_ENOUGH_QUOTA: the status a job's limit ends a process with */

static DWORD WINAPI time_watcher(LPVOID arg)
{
    (void)arg;
    FILETIME mine_c, x1, x2, x3;
    if (!GetProcessTimes(GetCurrentProcess(), &mine_c, &x1, &x2, &x3))
    {
        return 0;
    }
    DWORD self = GetCurrentProcessId();
    DWORD done[64];
    HANDLE held[64];
    int nd = 0;
    while (!time_stop)
    {
        /* The job's limit ends a process that has used its time; where the
           system does not enforce it (Wine), this does, to the millisecond
           this thread looks. */
        for (int i = 0; i < nd; i++)
        {
            FILETIME ct, et, kt, ut;
            if (WaitForSingleObject(held[i], 0) == WAIT_TIMEOUT && GetProcessTimes(held[i], &ct, &et, &kt, &ut))
            {
                ULONGLONG used = ((ULONGLONG)ut.dwHighDateTime << 32) | ut.dwLowDateTime;
                if (used >= (ULONGLONG)time_secs * 10000000ULL)
                {
                    InterlockedExchange(&time_hit, 1);
                    TerminateProcess(held[i], TIME_QUOTA);
                }
            }
        }
        HANDLE snap = CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0);
        if (snap != INVALID_HANDLE_VALUE)
        {
            PROCESSENTRY32W pe;
            pe.dwSize = sizeof pe;
            if (Process32FirstW(snap, &pe))
            {
                do
                {
                    if (pe.th32ParentProcessID != self || pe.th32ProcessID == self)
                    {
                        continue;
                    }
                    int seen = 0;
                    for (int i = 0; i < nd; i++)
                    {
                        seen |= done[i] == pe.th32ProcessID;
                    }
                    if (seen || nd == 64)
                    {
                        continue;
                    }
                    HANDLE h = OpenProcess(SYNCHRONIZE | PROCESS_SET_QUOTA | PROCESS_TERMINATE | PROCESS_QUERY_LIMITED_INFORMATION, FALSE, pe.th32ProcessID);
                    if (h == NULL)
                    {
                        continue;
                    }
                    FILETIME c;
                    if (GetProcessTimes(h, &c, &x1, &x2, &x3) && CompareFileTime(&c, &mine_c) >= 0)
                    {
                        AssignProcessToJobObject(time_job, h);
                        done[nd] = pe.th32ProcessID;
                        held[nd++] = h;
                    }
                    else
                    {
                        CloseHandle(h);
                    }
                } while (Process32NextW(snap, &pe));
            }
            CloseHandle(snap);
        }
        Sleep(5);
    }
    for (int i = 0; i < nd; i++)
    {
        DWORD code = 0;
        if (GetExitCodeProcess(held[i], &code) && code == TIME_QUOTA)
        {
            InterlockedExchange(&time_hit, 1);
        }
        CloseHandle(held[i]);
    }
    return 0;
}

/* `--run` on Windows: cobc put in a job that ends what it holds when cobc
   ends, so that the program it starts next (std's `Command::status`, which
   gives no number for it), and what that starts, end with cobc. Where cobc
   cannot be put in a job (Windows 7, when cobc is already in one) nothing
   changes. */
void cobc_host_join_job(int64_t secs)
{
    HANDLE job = CreateJobObjectW(NULL, NULL);
    if (job == NULL)
    {
        return;
    }
    JOBOBJECT_EXTENDED_LIMIT_INFORMATION info;
    memset(&info, 0, sizeof info);
    info.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
    if (secs >= 0)
    {
        /* A limit on the processor time of each process in the job applies
           to cobc too, if cobc were in it: the program's processes are put
           in it instead (`time_watcher`). */
        info.BasicLimitInformation.LimitFlags |= JOB_OBJECT_LIMIT_PROCESS_TIME;
        info.BasicLimitInformation.PerProcessUserTimeLimit.QuadPart = secs * 10000000LL;
        if (!SetInformationJobObject(job, JobObjectExtendedLimitInformation, &info, sizeof info))
        {
            CloseHandle(job);
            return;
        }
        time_job = job;
        time_stop = 0;
        time_hit = 0;
        time_secs = secs;
        time_thread = CreateThread(NULL, 0, time_watcher, NULL, 0, NULL);
        if (time_thread == NULL)
        {
            time_job = NULL;
            CloseHandle(job);
        }
        return;
    }
    if (!SetInformationJobObject(job, JobObjectExtendedLimitInformation, &info, sizeof info)
        || !AssignProcessToJobObject(job, GetCurrentProcess()))
    {
        CloseHandle(job);
    }
    /* Otherwise kept open: it closes when cobc ends. */
}

/* The run has ended: the watcher stops. 1 when the processor time limit
   ended the program. */
int64_t cobc_host_leave_job(void)
{
    int64_t hit = 0;
    if (time_thread != NULL)
    {
        InterlockedExchange(&time_stop, 1);
        WaitForSingleObject(time_thread, 2000);
        CloseHandle(time_thread);
        time_thread = NULL;
        /* Closing the job ends what is left in it. */
        CloseHandle(time_job);
        time_job = NULL;
        hit = time_hit != 0;
    }
    return hit;
}
#else
#include <dirent.h>
#include <fcntl.h>
#include <sys/stat.h>
#ifdef __linux__
#include <sys/syscall.h>
#endif

static char *watch_dirs[WATCH_MAX];
static volatile sig_atomic_t watch_count;
static volatile sig_atomic_t watch_kid;
static pid_t watch_owner;
static const int watch_stops[3] = { SIGINT, SIGTERM, SIGHUP };

/* The entry `name` of the directory `at`, and all it holds, removed. A
   signal handler's: on Linux system calls only (`getdents64`), no memory
   allocated; elsewhere `fdopendir`. Three levels at most (cobc's
   directories hold files). */
static void remove_at(int at, const char *name, int depth)
{
    if (unlinkat(at, name, 0) == 0 || (errno != EISDIR && errno != EPERM))
    {
        return;
    }
    int fd = openat(at, name, O_RDONLY | O_DIRECTORY | O_NOFOLLOW | O_CLOEXEC);
    if (fd < 0)
    {
        return;
    }
    for (int pass = 0; pass < 3; pass++)
    {
        int any = 0;
#ifdef __linux__
        char buf[4096] __attribute__((aligned(8)));
        lseek(fd, 0, SEEK_SET);
        for (;;)
        {
            long n = syscall(SYS_getdents64, fd, buf, sizeof buf);
            if (n <= 0)
            {
                break;
            }
            for (long off = 0; off < n;)
            {
                /* struct linux_dirent64: d_ino, d_off, d_reclen, d_type, d_name */
                unsigned short reclen;
                memcpy(&reclen, buf + off + 16, sizeof reclen);
                const char *nm = buf + off + 19;
                off += reclen;
                if (strcmp(nm, ".") == 0 || strcmp(nm, "..") == 0)
                {
                    continue;
                }
                any = 1;
                if (depth < 3)
                {
                    remove_at(fd, nm, depth + 1);
                }
                else
                {
                    unlinkat(fd, nm, 0);
                }
            }
        }
#else
        int fd2 = dup(fd);
        DIR *d = fd2 < 0 ? NULL : fdopendir(fd2);
        if (d != NULL)
        {
            rewinddir(d);
            struct dirent *e;
            while ((e = readdir(d)) != NULL)
            {
                if (strcmp(e->d_name, ".") == 0 || strcmp(e->d_name, "..") == 0)
                {
                    continue;
                }
                any = 1;
                if (depth < 3)
                {
                    remove_at(fd, e->d_name, depth + 1);
                }
                else
                {
                    unlinkat(fd, e->d_name, 0);
                }
            }
            closedir(d);
        }
        else if (fd2 >= 0)
        {
            close(fd2);
        }
#endif
        if (!any)
        {
            break;
        }
    }
    close(fd);
    unlinkat(at, name, AT_REMOVEDIR);
}

/* Bug 037 (c): every process below cobc (the C compiler and what it
   started), sent `sig`, so that none is left running when cobc ends. Linux:
   /proc read with system calls only (a signal handler's); elsewhere only
   the program `--explore` runs gets the signal. Returns how many were sent
   it. */
#define KIN_MAX 128

#ifdef __linux__
static int kin_parent(const char *name, pid_t *ppid)
{
    char path[64];
    size_t n = 6;
    memcpy(path, "/proc/", 6);
    for (const char *c = name; *c != 0 && n < 40; c++)
    {
        path[n++] = *c;
    }
    memcpy(path + n, "/stat", 6);
    int fd = open(path, O_RDONLY | O_CLOEXEC);
    if (fd < 0)
    {
        return 0;
    }
    char buf[512];
    long r = read(fd, buf, sizeof buf - 1);
    close(fd);
    if (r <= 0)
    {
        return 0;
    }
    buf[r] = 0;
    /* "pid (comm) S ppid ...": comm may hold anything, the last ")" ends it. */
    char *e = NULL;
    for (char *c = buf; *c != 0; c++)
    {
        if (*c == ')')
        {
            e = c;
        }
    }
    if (e == NULL || e[1] != ' ' || e[2] == 0 || e[3] != ' ')
    {
        return 0;
    }
    long v = 0;
    const char *d = e + 4;
    if (*d < '0' || *d > '9')
    {
        return 0;
    }
    while (*d >= '0' && *d <= '9')
    {
        v = v * 10 + (*d - '0');
        d++;
    }
    *ppid = (pid_t)v;
    return 1;
}

static int kill_kin(int sig)
{
    pid_t kin[KIN_MAX];
    int nk = 0;
    pid_t self = getpid();
    for (int pass = 0; pass < 6; pass++)
    {
        int fd = open("/proc", O_RDONLY | O_DIRECTORY | O_CLOEXEC);
        if (fd < 0)
        {
            break;
        }
        int added = 0;
        char buf[4096] __attribute__((aligned(8)));
        for (;;)
        {
            long n = syscall(SYS_getdents64, fd, buf, sizeof buf);
            if (n <= 0)
            {
                break;
            }
            for (long off = 0; off < n;)
            {
                unsigned short reclen;
                memcpy(&reclen, buf + off + 16, sizeof reclen);
                const char *nm = buf + off + 19;
                off += reclen;
                if (nm[0] < '1' || nm[0] > '9')
                {
                    continue;
                }
                long pid = 0;
                const char *c = nm;
                while (*c >= '0' && *c <= '9')
                {
                    pid = pid * 10 + (*c - '0');
                    c++;
                }
                if (*c != 0 || (pid_t)pid == self)
                {
                    continue;
                }
                int known = 0;
                for (int i = 0; i < nk; i++)
                {
                    if (kin[i] == (pid_t)pid)
                    {
                        known = 1;
                    }
                }
                pid_t pp;
                if (known || nk == KIN_MAX || !kin_parent(nm, &pp))
                {
                    continue;
                }
                int below = pp == self;
                for (int i = 0; i < nk && !below; i++)
                {
                    below = kin[i] == pp;
                }
                if (below)
                {
                    kin[nk++] = (pid_t)pid;
                    added = 1;
                }
            }
        }
        close(fd);
        if (!added)
        {
            break;
        }
    }
    for (int i = 0; i < nk; i++)
    {
        kill(kin[i], sig);
    }
    return nk;
}
#else
static int kill_kin(int sig)
{
    (void)sig;
    return 0;
}
#endif

static void on_stop(int sig, siginfo_t *si, void *uc)
{
    (void)uc;
    /* A process cobc forked, before it runs its program, ends as the
       signal would end it, and leaves cobc's directories alone. */
    if (getpid() == watch_owner)
    {
        int from_terminal = 0;
#ifdef SI_KERNEL
        from_terminal = si != NULL && si->si_code == SI_KERNEL;
#else
        (void)si;
#endif
        if (watch_kid > 0 && !from_terminal)
        {
            kill((pid_t)watch_kid, sig);
        }
        int saved = errno;
        /* The C compiler (and what it started) has the signal too, and a
           moment to end before the directory it writes in is removed. */
        int kin = from_terminal ? 0 : kill_kin(sig);
        for (int round = 0; round < (kin > 0 ? 3 : 1); round++)
        {
            if (round > 0)
            {
                struct timespec ts = { 0, 100 * 1000 * 1000 };
                nanosleep(&ts, NULL);
            }
            sig_atomic_t n = watch_count;
            for (sig_atomic_t i = 0; i < n; i++)
            {
                if (watch_dirs[i] != NULL)
                {
                    remove_at(AT_FDCWD, watch_dirs[i], 0);
                }
            }
        }
        errno = saved;
    }
    signal(sig, SIG_DFL);
    sigset_t one;
    sigemptyset(&one);
    sigaddset(&one, sig);
    sigprocmask(SIG_UNBLOCK, &one, NULL);
    raise(sig);
    _exit(128 + sig);
}

void cobc_host_watch_commit(void)
{
    if (watch_count >= WATCH_MAX || watch_path_len == 0)
    {
        return;
    }
    char *p = malloc((size_t)watch_path_len + 1);
    if (p == NULL)
    {
        return;
    }
    memcpy(p, watch_path, (size_t)watch_path_len + 1);
    sigset_t stops, mask;
    sigemptyset(&stops);
    for (int k = 0; k < 3; k++)
    {
        sigaddset(&stops, watch_stops[k]);
    }
    sigprocmask(SIG_BLOCK, &stops, &mask);
    watch_dirs[watch_count] = p;
    watch_count += 1;
    if (watch_owner == 0)
    {
        watch_owner = getpid();
        for (int k = 0; k < 3; k++)
        {
            struct sigaction before;
            if (sigaction(watch_stops[k], NULL, &before) == 0 && before.sa_handler != SIG_IGN)
            {
                struct sigaction sa;
                memset(&sa, 0, sizeof sa);
                sa.sa_sigaction = on_stop;
                sa.sa_flags = SA_SIGINFO;
                sa.sa_mask = stops;
                sigaction(watch_stops[k], &sa, NULL);
            }
        }
    }
    sigprocmask(SIG_SETMASK, &mask, NULL);
}

/* The program `--explore` runs now (0: none), which a signal to cobc
   reaches first. */
void cobc_host_watch_child(uint64_t pid)
{
    watch_kid = (sig_atomic_t)pid;
}

void cobc_host_join_job(int64_t secs)
{
    (void)secs;
}

int64_t cobc_host_leave_job(void)
{
    return 0;
}
#endif

/* ---- running a built program (Unix) ---- */

#ifndef _WIN32
/* The runtime's arguments (cbrt `cb_arg_bytes`, std's `arg_bytes`): the
   bytes of argument `i` (after the program's name) into [p, p + n), its
   length; < 0 when there is no argument `i`. */
extern int64_t cb_arg_bytes(uint64_t i, uint8_t *p, uint64_t n);

static uint8_t *run_path;
static uint64_t run_path_len, run_path_cap;

/* SIGINT, SIGTERM and SIGHUP to cobc while the program runs: passed on to
   the program, and remembered, so that cobc deletes `--run`'s directory
   before it ends by the same signal (`cobc_host_stop_signal`,
   `cobc_host_end_by`). A terminal's own interrupt reaches the whole
   process group, the program too, and is not sent to it again. */
static const int run_stops[3] = { SIGINT, SIGTERM, SIGHUP };
static volatile sig_atomic_t run_child;
static volatile sig_atomic_t run_stop;

static void on_run_stop(int sig, siginfo_t *si, void *uc)
{
    (void)uc;
    run_stop = sig;
#ifdef SI_KERNEL
    if (si != NULL && si->si_code == SI_KERNEL)
    {
        return;
    }
#else
    (void)si;
#endif
    if (run_child > 0)
    {
        kill((pid_t)run_child, sig);
    }
}

/* The signal that stopped cobc during the last `cobc_host_run`, or 0. */
int64_t cobc_host_stop_signal(void)
{
    return (int64_t)run_stop;
}

/* Ends cobc by `sig`, as the signal would have ended it. */
void cobc_host_end_by(int64_t sig)
{
    signal((int)sig, SIG_DFL);
    sigset_t one;
    sigemptyset(&one);
    sigaddset(&one, (int)sig);
    sigprocmask(SIG_UNBLOCK, &one, NULL);
    raise((int)sig);
    _exit(128 + (int)sig);
}

/* The path of the program `cobc_host_run` starts, a byte at a time. */
void cobc_host_run_path_byte(int64_t b)
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

/* The arguments that go between the program's path and cobc's own arguments
   (a runner's: `wine64 -x` in `COBALTC_RUNNER`, then the built program), a
   byte at a time: -1 begins again, -2 ends one argument. */
static uint8_t *run_pre;
static uint64_t run_pre_len, run_pre_cap;
static int64_t run_pre_n;

void cobc_host_run_pre_byte(int64_t b)
{
    if (b == -1)
    {
        run_pre_len = 0;
        run_pre_n = 0;
        return;
    }
    if (run_pre_len + 2 > run_pre_cap)
    {
        run_pre_cap = run_pre_cap * 2 + 64;
        run_pre = realloc(run_pre, run_pre_cap);
        if (run_pre == NULL)
        {
            abort();
        }
    }
    if (b == -2)
    {
        run_pre[run_pre_len++] = 0;
        run_pre_n += 1;
        return;
    }
    run_pre[run_pre_len++] = (uint8_t)b;
}

/* `--run`: the program at the path given, with cobc's own arguments from
   `first` on as its arguments, byte for byte (an argument need not be UTF-8),
   started under its guard (ended when cobc ends, `secs` seconds of CPU time
   when >= 0, `COBALTC_SCHEDULE_SEED` set to `seed` when >= 0, the variables
   the options set, a bit of `flags` each, set to `1`), its streams
   cobc's. Its exit status, minus its signal when a signal ended it, or -1000
   when it could not be started. */
int64_t cobc_host_run(uint64_t first, int64_t secs, int64_t seed, int64_t flags)
{
    int64_t n = 0;
    while (cb_arg_bytes(first + (uint64_t)n, NULL, 0) >= 0)
    {
        n += 1;
    }
    int64_t np = run_pre_n;
    char **argv = calloc((size_t)(n + np) + 2, sizeof(char *));
    if (argv == NULL || run_path == NULL)
    {
        return -1000;
    }
    argv[0] = (char *)run_path;
    {
        uint64_t at = 0;
        for (int64_t i = 0; i < np; i++)
        {
            argv[i + 1] = strdup((const char *)run_pre + at);
            at += strlen((const char *)run_pre + at) + 1;
        }
    }
    for (int64_t i = np; i < n + np; i++)
    {
        int64_t len = cb_arg_bytes(first + (uint64_t)(i - np), NULL, 0);
        char *a = malloc((size_t)len + 1);
        if (a == NULL)
        {
            return -1000;
        }
        cb_arg_bytes(first + (uint64_t)(i - np), (uint8_t *)a, (uint64_t)len);
        a[len] = 0;
        argv[i + 1] = a;
    }
    char seed_text[32];
    snprintf(seed_text, sizeof seed_text, "%lld", (long long)seed);
    /* The stop signals are caught (those not ignored: a job started in the
       background keeps ignoring SIGINT) and held until the child's number
       is known. */
    struct sigaction before[3];
    int caught[3] = { 0, 0, 0 };
    sigset_t stops, mask;
    sigemptyset(&stops);
    for (int k = 0; k < 3; k++)
    {
        sigaddset(&stops, run_stops[k]);
    }
    sigprocmask(SIG_BLOCK, &stops, &mask);
    run_stop = 0;
    run_child = 0;
    for (int k = 0; k < 3; k++)
    {
        if (sigaction(run_stops[k], NULL, &before[k]) == 0 && before[k].sa_handler != SIG_IGN)
        {
            struct sigaction sa;
            memset(&sa, 0, sizeof sa);
            sa.sa_sigaction = on_run_stop;
            sa.sa_flags = SA_SIGINFO;
            sigemptyset(&sa.sa_mask);
            caught[k] = sigaction(run_stops[k], &sa, NULL) == 0;
        }
    }
    pid_t parent = getpid();
    pid_t pid = fork();
    if (pid == 0)
    {
        /* The program gets cobc's dispositions and mask as they were. */
        for (int k = 0; k < 3; k++)
        {
            if (caught[k])
            {
                sigaction(run_stops[k], &before[k], NULL);
            }
        }
        sigprocmask(SIG_SETMASK, &mask, NULL);
    }
    else
    {
        run_child = pid > 0 ? (sig_atomic_t)pid : 0;
        sigprocmask(SIG_SETMASK, &mask, NULL);
    }
    if (pid < 0)
    {
        for (int k = 0; k < 3; k++)
        {
            if (caught[k])
            {
                sigaction(run_stops[k], &before[k], NULL);
            }
        }
        return -1000;
    }
    if (pid == 0)
    {
#ifdef __linux__
        if (prctl(PR_SET_PDEATHSIG, SIGKILL, 0, 0, 0) != 0 || getppid() != parent)
        {
            static const char msg[] = "cobc: could not tie the program to cobc's life (prctl); the program was not started\n";
            (void)!write(2, msg, sizeof msg - 1);
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
                static const char msg[] = "cobc: could not set the program's CPU limit (setrlimit); the program was not started\n";
                (void)!write(2, msg, sizeof msg - 1);
                _exit(127);
            }
        }
        if (seed >= 0)
        {
            setenv("COBALTC_SCHEDULE_SEED", seed_text, 1);
        }
        /* driverbase.cb's `env_flags`. */
        static const char *const set_by_option[4] = { "COBALTC_EXPLAIN", "COBALTC_WARN_SHADOW", "COBALTC_ALL_CHECKS", "COBALTC_BOOTSTRAP_FORM" };
        for (int k = 0; k < 4; k++)
        {
            if ((flags >> k) & 1)
            {
                setenv(set_by_option[k], "1", 1);
            }
        }
        execvp(argv[0], argv);
        {
            static const char msg[] = "cobc: could not start the program (exec failed)\n";
            (void)!write(2, msg, sizeof msg - 1);
        }
        _exit(127);
    }
    for (int64_t i = 0; i < n + np; i++)
    {
        free(argv[i + 1]);
    }
    free(argv);
    int status = 0;
    int waited = 1;
    while (waitpid(pid, &status, 0) < 0)
    {
        if (errno != EINTR)
        {
            waited = 0;
            break;
        }
    }
    run_child = 0;
    for (int k = 0; k < 3; k++)
    {
        if (caught[k])
        {
            sigaction(run_stops[k], &before[k], NULL);
        }
    }
    if (!waited)
    {
        return -1000;
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
void cobc_host_run_path_byte(int64_t b)
{
    (void)b;
}

void cobc_host_run_pre_byte(int64_t b)
{
    (void)b;
}

int64_t cobc_host_run(uint64_t first, int64_t secs, int64_t seed, int64_t flags)
{
    (void)first;
    (void)secs;
    (void)seed;
    (void)flags;
    return -1000;
}

/* A console event (Ctrl-C, Ctrl-Break, the console closed) that came during
   the run, as SIGINT's number, or 0: the console handler is removing the
   directories (`on_console_stop`). */
int64_t cobc_host_stop_signal(void)
{
    return console_stop != 0 ? 2 : 0;
}

/* Ends cobc once the console handler has removed the directories, with the
   status it ends with. */
void cobc_host_end_by(int64_t sig)
{
    (void)sig;
    if (console_stop != 0)
    {
        WaitForSingleObject(console_done, 30000);
        ExitProcess((UINT)STATUS_CONTROL_C_EXIT);
    }
}
#endif
