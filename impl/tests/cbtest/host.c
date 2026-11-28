/* What cbtest (D-0326, the test runner written in CobaltC) needs of the
   platform that `std`'s `Command` does not give: a program run with
   arguments that need not be UTF-8, its standard input given, its standard
   output and error gathered, a CPU limit, a wall-clock limit that ends it
   and every process it started, and its exit status. Plain C for
   Linux (glibc) and Windows 7 or later (MinGW-w64), no other library.

   The interface is a few calls on one run at a time per thread: the
   arguments are built byte by byte, the run made, then its outputs read
   back byte by byte. */

#ifdef _WIN32
#undef _WIN32_WINNT
#define _WIN32_WINNT 0x0601
#include <windows.h>
#else
#define _GNU_SOURCE
#include <errno.h>
#include <fcntl.h>
#include <poll.h>
#include <signal.h>
#include <sys/resource.h>
#include <sys/types.h>
#include <sys/wait.h>
#include <time.h>
#include <unistd.h>
#ifdef __linux__
#include <sys/prctl.h>
#endif
#endif
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

/* A growable byte buffer. */
struct buf
{
    unsigned char *p;
    size_t n;
    size_t cap;
};

static void buf_push(struct buf *b, const unsigned char *data, size_t n)
{
    if (b->n + n + 1 > b->cap)
    {
        size_t cap = b->cap ? b->cap : 256;
        while (cap < b->n + n + 1)
        {
            cap *= 2;
        }
        unsigned char *p = realloc(b->p, cap);
        if (!p)
        {
            fprintf(stderr, "cbtest: out of memory\n");
            exit(4);
        }
        b->p = p;
        b->cap = cap;
    }
    memcpy(b->p + b->n, data, n);
    b->n += n;
    b->p[b->n] = 0;
}

static void buf_clear(struct buf *b)
{
    b->n = 0;
}

/* The run being built and the last run's results (one at a time, per
   thread). The arguments and the environment are NUL-separated. */
static _Thread_local struct buf args;
static _Thread_local struct buf envs;
static _Thread_local struct buf cwd;
static _Thread_local struct buf input;
static _Thread_local struct buf out;
static _Thread_local struct buf err;
static _Thread_local int64_t timed_out;

void cbtest_host_reset(void)
{
    buf_clear(&args);
    buf_clear(&envs);
    buf_clear(&cwd);
    buf_clear(&input);
    buf_clear(&out);
    buf_clear(&err);
    timed_out = 0;
}

/* Byte `b` of the current argument; -1 ends the argument. */
void cbtest_host_arg_byte(int64_t b)
{
    unsigned char c = (unsigned char)(b < 0 ? 0 : b);
    buf_push(&args, &c, 1);
}

/* Byte `b` of an environment entry `NAME=VALUE`; -1 ends it. */
void cbtest_host_env_byte(int64_t b)
{
    unsigned char c = (unsigned char)(b < 0 ? 0 : b);
    buf_push(&envs, &c, 1);
}

void cbtest_host_cwd_byte(int64_t b)
{
    unsigned char c = (unsigned char)b;
    buf_push(&cwd, &c, 1);
}

void cbtest_host_input_byte(int64_t b)
{
    unsigned char c = (unsigned char)b;
    buf_push(&input, &c, 1);
}

uint64_t cbtest_host_out_len(void)
{
    return out.n;
}

int64_t cbtest_host_out_byte(uint64_t i)
{
    return i < out.n ? out.p[i] : -1;
}

uint64_t cbtest_host_err_len(void)
{
    return err.n;
}

int64_t cbtest_host_err_byte(uint64_t i)
{
    return i < err.n ? err.p[i] : -1;
}

/* Whether the last run was ended by its wall-clock limit. */
int64_t cbtest_host_timed_out(void)
{
    return timed_out;
}

#ifndef _WIN32

static double now_secs(void)
{
    struct timespec t;
    clock_gettime(CLOCK_MONOTONIC, &t);
    return (double)t.tv_sec + (double)t.tv_nsec / 1e9;
}

/* Runs the program the arguments name (the first, looked up in PATH when
   it has no `/`), in `cwd` when one was given, with the environment
   entries added to this process's, its standard input `input`; its CPU
   limited to `cpu_secs` and its wall clock to `wall_secs` (0: none), after
   which it and its process group are killed. The status: its exit code,
   128 + the signal that ended it, or -1 when it could not be started (the
   reason on `err`). */
int64_t cbtest_host_run(int64_t cpu_secs, int64_t wall_secs)
{
    buf_clear(&out);
    buf_clear(&err);
    timed_out = 0;
    size_t argc = 0;
    for (size_t i = 0; i < args.n; i++)
    {
        argc += args.p[i] == 0;
    }
    if (argc == 0)
    {
        const char *m = "cbtest: no program to run\n";
        buf_push(&err, (const unsigned char *)m, strlen(m));
        return -1;
    }
    char **argv = calloc(argc + 1, sizeof *argv);
    size_t k = 0;
    for (size_t i = 0; i < args.n; i += strlen((char *)args.p + i) + 1)
    {
        argv[k++] = (char *)args.p + i;
    }
    int pin[2], pout[2], perr[2];
    if (pipe(pin) != 0 || pipe(pout) != 0 || pipe(perr) != 0)
    {
        free(argv);
        return -1;
    }
    pid_t pid = fork();
    if (pid < 0)
    {
        free(argv);
        return -1;
    }
    if (pid == 0)
    {
        setpgid(0, 0);
#ifdef __linux__
        prctl(PR_SET_PDEATHSIG, SIGKILL);
#endif
        if (cpu_secs > 0)
        {
            struct rlimit r = {(rlim_t)cpu_secs, (rlim_t)cpu_secs + 1};
            setrlimit(RLIMIT_CPU, &r);
        }
        dup2(pin[0], 0);
        dup2(pout[1], 1);
        dup2(perr[1], 2);
        close(pin[0]);
        close(pin[1]);
        close(pout[0]);
        close(pout[1]);
        close(perr[0]);
        close(perr[1]);
        for (size_t i = 0; i < envs.n; i += strlen((char *)envs.p + i) + 1)
        {
            putenv((char *)envs.p + i);
        }
        if (cwd.n > 0 && chdir((char *)cwd.p) != 0)
        {
            fprintf(stderr, "cbtest: cannot enter %s\n", (char *)cwd.p);
            _exit(127);
        }
        execvp(argv[0], argv);
        fprintf(stderr, "cbtest: cannot run %s: %s\n", argv[0], strerror(errno));
        _exit(127);
    }
    setpgid(pid, pid);
    free(argv);
    close(pin[0]);
    close(pout[1]);
    close(perr[1]);
    fcntl(pin[1], F_SETFL, O_NONBLOCK);
    size_t written = 0;
    if (input.n == 0)
    {
        close(pin[1]);
        pin[1] = -1;
    }
    double start = now_secs();
    int open_out = 1, open_err = 1;
    unsigned char chunk[65536];
    while (open_out || open_err || pin[1] >= 0)
    {
        struct pollfd fds[3];
        int nf = 0;
        int io = -1, ie = -1, ii = -1;
        if (open_out)
        {
            fds[nf].fd = pout[0];
            fds[nf].events = POLLIN;
            io = nf++;
        }
        if (open_err)
        {
            fds[nf].fd = perr[0];
            fds[nf].events = POLLIN;
            ie = nf++;
        }
        if (pin[1] >= 0)
        {
            fds[nf].fd = pin[1];
            fds[nf].events = POLLOUT;
            ii = nf++;
        }
        int r = poll(fds, (nfds_t)nf, 100);
        if (r < 0 && errno != EINTR)
        {
            break;
        }
        if (io >= 0 && (fds[io].revents & (POLLIN | POLLHUP | POLLERR)))
        {
            ssize_t g = read(pout[0], chunk, sizeof chunk);
            if (g > 0)
            {
                buf_push(&out, chunk, (size_t)g);
            }
            else
            {
                open_out = 0;
            }
        }
        if (ie >= 0 && (fds[ie].revents & (POLLIN | POLLHUP | POLLERR)))
        {
            ssize_t g = read(perr[0], chunk, sizeof chunk);
            if (g > 0)
            {
                buf_push(&err, chunk, (size_t)g);
            }
            else
            {
                open_err = 0;
            }
        }
        if (ii >= 0 && (fds[ii].revents & (POLLOUT | POLLERR | POLLHUP)))
        {
            ssize_t w = write(pin[1], input.p + written, input.n - written);
            if (w > 0)
            {
                written += (size_t)w;
            }
            if (w < 0 || written == input.n)
            {
                close(pin[1]);
                pin[1] = -1;
            }
        }
        if (wall_secs > 0 && now_secs() - start > (double)wall_secs)
        {
            timed_out = 1;
            kill(-pid, SIGKILL);
            break;
        }
    }
    if (pin[1] >= 0)
    {
        close(pin[1]);
    }
    close(pout[0]);
    close(perr[0]);
    int status = 0;
    while (waitpid(pid, &status, 0) < 0 && errno == EINTR)
    {
    }
    /* Whatever the program left running in its group goes with it. */
    kill(-pid, SIGKILL);
    if (WIFEXITED(status))
    {
        return WEXITSTATUS(status);
    }
    if (WIFSIGNALED(status))
    {
        return 128 + WTERMSIG(status);
    }
    return -1;
}

#else

/* Windows: CreateProcessW in a job object (closing the job ends every
   process in it), the arguments quoted as the C runtime parses them, the
   pipes read by threads. */

struct reader
{
    HANDLE h;
    struct buf *b;
};

static DWORD WINAPI read_all(LPVOID p)
{
    struct reader *r = p;
    unsigned char chunk[65536];
    DWORD got;
    while (ReadFile(r->h, chunk, sizeof chunk, &got, NULL) && got > 0)
    {
        buf_push(r->b, chunk, got);
    }
    return 0;
}

struct writer
{
    HANDLE h;
};

static DWORD WINAPI write_all(LPVOID p)
{
    struct writer *w = p;
    size_t done = 0;
    while (done < input.n)
    {
        DWORD put;
        if (!WriteFile(w->h, input.p + done, (DWORD)(input.n - done), &put, NULL))
        {
            break;
        }
        done += put;
    }
    CloseHandle(w->h);
    return 0;
}

/* `s` (UTF-8) appended to `cmd` as one argument, quoted for
   CommandLineToArgvW and the MinGW C runtime. */
static void quote_arg(struct buf *cmd, const char *s)
{
    int needs = *s == 0 || strpbrk(s, " \t\n\v\"") != NULL;
    if (!needs)
    {
        buf_push(cmd, (const unsigned char *)s, strlen(s));
        return;
    }
    buf_push(cmd, (const unsigned char *)"\"", 1);
    for (const char *p = s;; p++)
    {
        size_t slashes = 0;
        while (*p == '\\')
        {
            p++;
            slashes++;
        }
        if (*p == 0)
        {
            for (size_t i = 0; i < slashes * 2; i++)
            {
                buf_push(cmd, (const unsigned char *)"\\", 1);
            }
            break;
        }
        if (*p == '"')
        {
            for (size_t i = 0; i < slashes * 2 + 1; i++)
            {
                buf_push(cmd, (const unsigned char *)"\\", 1);
            }
        }
        else
        {
            for (size_t i = 0; i < slashes; i++)
            {
                buf_push(cmd, (const unsigned char *)"\\", 1);
            }
        }
        buf_push(cmd, (const unsigned char *)p, 1);
    }
    buf_push(cmd, (const unsigned char *)"\"", 1);
}

static wchar_t *wide(const char *s)
{
    int n = MultiByteToWideChar(CP_UTF8, 0, s, -1, NULL, 0);
    wchar_t *w = calloc((size_t)n + 1, sizeof *w);
    MultiByteToWideChar(CP_UTF8, 0, s, -1, w, n);
    return w;
}

int64_t cbtest_host_run(int64_t cpu_secs, int64_t wall_secs)
{
    buf_clear(&out);
    buf_clear(&err);
    timed_out = 0;
    struct buf cmd = {0};
    for (size_t i = 0; i < args.n; i += strlen((char *)args.p + i) + 1)
    {
        if (cmd.n)
        {
            buf_push(&cmd, (const unsigned char *)" ", 1);
        }
        quote_arg(&cmd, (char *)args.p + i);
    }
    if (cmd.n == 0)
    {
        return -1;
    }
    /* The environment: this process's, then the entries given (a later
       entry for the same name wins in the child's lookup order). */
    for (size_t i = 0; i < envs.n; i += strlen((char *)envs.p + i) + 1)
    {
        char *e = (char *)envs.p + i;
        char *eq = strchr(e, '=');
        if (eq)
        {
            *eq = 0;
            wchar_t *wn = wide(e);
            wchar_t *wv = wide(eq + 1);
            SetEnvironmentVariableW(wn, wv);
            free(wn);
            free(wv);
            *eq = '=';
        }
    }
    SECURITY_ATTRIBUTES sa = {sizeof sa, NULL, TRUE};
    HANDLE in_r, in_w, out_r, out_w, err_r, err_w;
    CreatePipe(&in_r, &in_w, &sa, 0);
    CreatePipe(&out_r, &out_w, &sa, 0);
    CreatePipe(&err_r, &err_w, &sa, 0);
    SetHandleInformation(in_w, HANDLE_FLAG_INHERIT, 0);
    SetHandleInformation(out_r, HANDLE_FLAG_INHERIT, 0);
    SetHandleInformation(err_r, HANDLE_FLAG_INHERIT, 0);
    STARTUPINFOW si;
    memset(&si, 0, sizeof si);
    si.cb = sizeof si;
    si.dwFlags = STARTF_USESTDHANDLES;
    si.hStdInput = in_r;
    si.hStdOutput = out_w;
    si.hStdError = err_w;
    PROCESS_INFORMATION pi;
    HANDLE job = CreateJobObjectW(NULL, NULL);
    JOBOBJECT_EXTENDED_LIMIT_INFORMATION lim;
    memset(&lim, 0, sizeof lim);
    lim.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
    if (cpu_secs > 0)
    {
        lim.BasicLimitInformation.LimitFlags |= JOB_OBJECT_LIMIT_PROCESS_TIME;
        lim.BasicLimitInformation.PerProcessUserTimeLimit.QuadPart = cpu_secs * 10000000LL;
    }
    SetInformationJobObject(job, JobObjectExtendedLimitInformation, &lim, sizeof lim);
    buf_push(&cmd, (const unsigned char *)"", 0);
    wchar_t *wcmd = wide((char *)cmd.p);
    wchar_t *wdir = cwd.n ? wide((char *)cwd.p) : NULL;
    BOOL ok = CreateProcessW(NULL, wcmd, NULL, NULL, TRUE, CREATE_SUSPENDED, NULL, wdir, &si, &pi);
    free(wcmd);
    free(wdir);
    free(cmd.p);
    CloseHandle(in_r);
    CloseHandle(out_w);
    CloseHandle(err_w);
    if (!ok)
    {
        const char *m = "cbtest: cannot start the program\n";
        buf_push(&err, (const unsigned char *)m, strlen(m));
        CloseHandle(in_w);
        CloseHandle(out_r);
        CloseHandle(err_r);
        CloseHandle(job);
        return -1;
    }
    AssignProcessToJobObject(job, pi.hProcess);
    ResumeThread(pi.hThread);
    CloseHandle(pi.hThread);
    struct writer w = {in_w};
    struct reader ro = {out_r, &out};
    struct reader re = {err_r, &err};
    HANDLE tw = CreateThread(NULL, 0, write_all, &w, 0, NULL);
    HANDLE to = CreateThread(NULL, 0, read_all, &ro, 0, NULL);
    HANDLE te = CreateThread(NULL, 0, read_all, &re, 0, NULL);
    DWORD waited = WaitForSingleObject(pi.hProcess, wall_secs > 0 ? (DWORD)(wall_secs * 1000) : INFINITE);
    if (waited == WAIT_TIMEOUT)
    {
        timed_out = 1;
        TerminateJobObject(job, 137);
        WaitForSingleObject(pi.hProcess, INFINITE);
    }
    DWORD code = 0;
    GetExitCodeProcess(pi.hProcess, &code);
    /* Whatever the program left running goes with the job. */
    TerminateJobObject(job, 137);
    WaitForSingleObject(to, INFINITE);
    WaitForSingleObject(te, INFINITE);
    WaitForSingleObject(tw, INFINITE);
    CloseHandle(to);
    CloseHandle(te);
    CloseHandle(tw);
    CloseHandle(out_r);
    CloseHandle(err_r);
    CloseHandle(pi.hProcess);
    CloseHandle(job);
    return (int64_t)code;
}

#endif
