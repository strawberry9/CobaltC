/* The runtime's platform layer on Windows 7 and later (MinGW-w64): plat.h's
   locks, threads, clocks and process, and plat_io.h's files, child
   processes, sockets and random bytes, each as Rust's standard library
   does it on `x86_64-pc-windows-gnu` (its release 1.77.2, the last that
   runs on Windows 7): paths made "verbatim" (`\\?\`) where `std::fs` makes
   them (`maybe_verbatim`), files opened sharing read, write and delete,
   handles not inherited but for a child's three streams, the program
   searched for as `Command::spawn` searches, its arguments quoted as
   `make_command_line` quotes them, errors mapped as `decode_error_kind`.
   Only APIs Windows 7 has. The whole file is empty elsewhere
   (plat_posix.c). */

#ifdef _WIN32

#include "plat.h"
#include "plat_io.h"

#include <winsock2.h>
#include <ws2tcpip.h>
#include <windows.h>
#include <bcrypt.h>
#include <fcntl.h>
#include <io.h>
#include <stdio.h>

const int os_windows = 1;

/* ---- locks ---- */

void plat_mutex_init(plat_mutex *m)
{
    InitializeSRWLock(&m->l);
}

void plat_mutex_lock(plat_mutex *m)
{
    AcquireSRWLockExclusive(&m->l);
}

int plat_mutex_trylock(plat_mutex *m)
{
    return TryAcquireSRWLockExclusive(&m->l) != 0;
}

void plat_mutex_unlock(plat_mutex *m)
{
    ReleaseSRWLockExclusive(&m->l);
}

void plat_cond_init(plat_cond *c)
{
    InitializeConditionVariable(&c->c);
}

void plat_cond_wait(plat_cond *c, plat_mutex *m)
{
    SleepConditionVariableSRW(&c->c, &m->l, INFINITE, 0);
}

int plat_cond_wait_ns(plat_cond *c, plat_mutex *m, uint64_t ns)
{
    /* Whole milliseconds, rounded up (Rust's `dur2timeout`). */
    uint64_t ms = ns / 1000000u + (ns % 1000000u ? 1 : 0);
    DWORD t = ms >= INFINITE ? INFINITE - 1 : (DWORD)ms;
    if (SleepConditionVariableSRW(&c->c, &m->l, t, 0))
    {
        return 1;
    }
    return GetLastError() != ERROR_TIMEOUT;
}

void plat_cond_signal(plat_cond *c)
{
    WakeConditionVariable(&c->c);
}

void plat_cond_broadcast(plat_cond *c)
{
    WakeAllConditionVariable(&c->c);
}

void plat_rw_read(plat_rwlock *l)
{
    AcquireSRWLockShared(&l->l);
}

int plat_rw_try_read(plat_rwlock *l)
{
    return TryAcquireSRWLockShared(&l->l) != 0;
}

void plat_rw_read_unlock(plat_rwlock *l)
{
    ReleaseSRWLockShared(&l->l);
}

void plat_rw_write(plat_rwlock *l)
{
    AcquireSRWLockExclusive(&l->l);
}

int plat_rw_try_write(plat_rwlock *l)
{
    return TryAcquireSRWLockExclusive(&l->l) != 0;
}

void plat_rw_write_unlock(plat_rwlock *l)
{
    ReleaseSRWLockExclusive(&l->l);
}

/* ---- threads ---- */

typedef struct
{
    void (*fn)(void *);
    void *arg;
} thread_start;

static DWORD WINAPI thread_main(LPVOID p)
{
    thread_start s = *(thread_start *)p;
    free(p);
    s.fn(s.arg);
    return 0;
}

/* As Rust's `Thread::new`: `CreateThread`, the stack size a reservation. */
int plat_thread_spawn(plat_thread *t, void (*fn)(void *), void *arg, size_t stack)
{
    thread_start *s = (thread_start *)malloc(sizeof *s);
    if (!s)
    {
        return -1;
    }
    s->fn = fn;
    s->arg = arg;
    HANDLE h = CreateThread(NULL, stack, thread_main, s, STACK_SIZE_PARAM_IS_A_RESERVATION, NULL);
    if (!h)
    {
        free(s);
        return -1;
    }
    *t = h;
    return 0;
}

void plat_thread_join(plat_thread t)
{
    WaitForSingleObject(t, INFINITE);
    CloseHandle(t);
}

void plat_thread_detach(plat_thread t)
{
    CloseHandle(t);
}

void plat_thread_yield(void)
{
    SwitchToThread();
}

uint64_t plat_thread_self(void)
{
    return (uint64_t)GetCurrentThreadId();
}

uint64_t plat_cpu_count(void)
{
    SYSTEM_INFO si;
    GetSystemInfo(&si);
    return si.dwNumberOfProcessors ? si.dwNumberOfProcessors : 1;
}

void plat_spin_hint(void)
{
    YieldProcessor();
}

/* Per-thread exit callbacks: a list in one fiber-local slot, whose callback
   (Vista and later) runs them, most recent first, when the thread ends. */
typedef struct at_exit_node
{
    void (*fn)(void *);
    void *arg;
    struct at_exit_node *next;
} at_exit_node;

static DWORD at_exit_slot = FLS_OUT_OF_INDEXES;
static INIT_ONCE at_exit_once = INIT_ONCE_STATIC_INIT;

static VOID WINAPI at_exit_run(PVOID p)
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

static BOOL CALLBACK at_exit_make(PINIT_ONCE once, PVOID param, PVOID *ctx)
{
    (void)once;
    (void)param;
    (void)ctx;
    at_exit_slot = FlsAlloc(at_exit_run);
    return at_exit_slot != FLS_OUT_OF_INDEXES;
}

void plat_thread_at_exit(void (*fn)(void *), void *arg)
{
    if (!InitOnceExecuteOnce(&at_exit_once, at_exit_make, NULL, NULL))
    {
        cbrt_internal("no fiber-local slot for a thread's end");
    }
    at_exit_node *n = (at_exit_node *)cb_xmalloc(sizeof *n);
    n->fn = fn;
    n->arg = arg;
    n->next = (at_exit_node *)FlsGetValue(at_exit_slot);
    FlsSetValue(at_exit_slot, n);
}

/* ---- the stack (D-0077) ---- */

/* Windows: the stack is one reservation, from its `AllocationBase` (the
   lowest address, below the guard page) up to the frame running now;
   `VirtualQuery` of a local's address gives it (Windows 7 has it, unlike
   `GetCurrentThreadStackLimits`). The frames above the local are a few
   hundred bytes, left to the margin. */
int plat_thread_stack(uintptr_t *lo, size_t *size)
{
    MEMORY_BASIC_INFORMATION info;
    volatile char here = 0;
    const volatile char *at = &here;
    SIZE_T n = VirtualQuery((LPCVOID)at, &info, sizeof info);
    uintptr_t base = (uintptr_t)info.AllocationBase;
    if (n == 0 || base == 0 || base >= (uintptr_t)at)
    {
        return 0;
    }
    *lo = base;
    *size = (size_t)((uintptr_t)at - base);
    return 1;
}

/* Bug 032: a frame larger than what is left of the stack. MinGW's GCC
   probes such a frame a page at a time (`___chkstk_ms`), so it reaches the
   guard page beneath the stack's committed part, and where the stack can
   grow no further Windows raises EXCEPTION_STACK_OVERFLOW; past the guard
   (after an overflow already handled) an access violation in the stack's
   reservation. A vectored handler turns either, on a watched thread, into
   a call of `on_overflow` (which raises `diag.stack-exhausted` and never
   returns) where the access was made: the probe's stack pointer is the
   caller's, above the frame it was making, so the committed stack below it
   is there for the fault's unwind; each watched thread also asks for
   `WATCH_GUARANTEE` bytes beyond the overflow (`SetThreadStackGuarantee`)
   for an overflow at the stack pointer. Any other exception, or one on a
   thread not watched, goes on to the handlers after it. The thread's own
   record is a thread-local slot (`TlsGetValue` allocates nothing, unlike
   MinGW's emulated `__thread`). x86-64 only; nothing elsewhere. */
#define WATCH_GUARANTEE ((ULONG)64 << 10)

typedef struct
{
    uintptr_t lo;
    uintptr_t floor;
    /* An overflow is being raised: one in its unwind is not caught again. */
    int busy;
} watch_rec;

static DWORD watch_slot = TLS_OUT_OF_INDEXES;
static void (*watch_overflow)(void);
static INIT_ONCE watch_once = INIT_ONCE_STATIC_INIT;

#if defined(__x86_64__) || defined(_M_X64)
static LONG CALLBACK on_stack_fault(EXCEPTION_POINTERS *e)
{
    DWORD code = e->ExceptionRecord->ExceptionCode;
    if (code != EXCEPTION_STACK_OVERFLOW && code != EXCEPTION_ACCESS_VIOLATION)
    {
        return EXCEPTION_CONTINUE_SEARCH;
    }
    DWORD keep = GetLastError();
    watch_rec *w = (watch_rec *)TlsGetValue(watch_slot);
    SetLastError(keep);
    if (w == NULL || w->floor <= 1 || w->busy || watch_overflow == NULL)
    {
        return EXCEPTION_CONTINUE_SEARCH;
    }
    if (code == EXCEPTION_ACCESS_VIOLATION)
    {
        if (e->ExceptionRecord->NumberParameters < 2)
        {
            return EXCEPTION_CONTINUE_SEARCH;
        }
        uintptr_t a = (uintptr_t)e->ExceptionRecord->ExceptionInformation[1];
        if (a < w->lo || a >= w->floor)
        {
            return EXCEPTION_CONTINUE_SEARCH;
        }
    }
    /* As if the access had called `on_overflow`: below the stack pointer,
       aligned as a call leaves it (Windows has no red zone; the 128 bytes
       spare the frame's own top). */
    w->busy = 1;
    CONTEXT *c = e->ContextRecord;
    DWORD64 sp = ((c->Rsp - 128) & ~(DWORD64)15) - 8;
    *(DWORD64 *)(uintptr_t)sp = c->Rip;
    c->Rsp = sp;
    c->Rip = (DWORD64)(uintptr_t)watch_overflow;
    return EXCEPTION_CONTINUE_EXECUTION;
}
#endif

static BOOL CALLBACK watch_start(PINIT_ONCE once, PVOID param, PVOID *ctx)
{
    (void)once;
    (void)ctx;
    watch_overflow = (void (*)(void))param;
    watch_slot = TlsAlloc();
#if defined(__x86_64__) || defined(_M_X64)
    if (watch_slot != TLS_OUT_OF_INDEXES)
    {
        AddVectoredExceptionHandler(1, on_stack_fault);
    }
#endif
    return TRUE;
}

void plat_watch_stack(uintptr_t lo, uintptr_t floor, void (*on_overflow)(void))
{
    InitOnceExecuteOnce(&watch_once, watch_start, (PVOID)on_overflow, NULL);
    if (watch_slot == TLS_OUT_OF_INDEXES)
    {
        return;
    }
    watch_rec *w = (watch_rec *)TlsGetValue(watch_slot);
    if (w == NULL)
    {
        /* Kept for the thread's life: a pooled worker watches its stack
           again for each job. */
        w = (watch_rec *)calloc(1, sizeof *w);
        if (w == NULL || !TlsSetValue(watch_slot, w))
        {
            free(w);
            return;
        }
        ULONG g = WATCH_GUARANTEE;
        SetThreadStackGuarantee(&g);
    }
    w->lo = lo;
    w->floor = floor;
    w->busy = 0;
}

/* The thread ends: its record is freed. */
void plat_unwatch_stack(void)
{
    if (watch_slot != TLS_OUT_OF_INDEXES)
    {
        watch_rec *w = (watch_rec *)TlsGetValue(watch_slot);
        if (w != NULL)
        {
            TlsSetValue(watch_slot, NULL);
            free(w);
        }
    }
}

/* After an overflow the thread survived (a contained fault, a pooled worker
   going on to its next job): the guard page under the stack's committed
   part made again, as the C library's `_resetstkoflw` makes it (which
   Windows 7's msvcrt may not have), so that a second overflow is raised as
   the first was. Below the stack pointer, with room for the guarantee
   beneath; nothing when a guard is there already. */
void plat_stack_recovered(void)
{
    if (watch_slot != TLS_OUT_OF_INDEXES)
    {
        watch_rec *w = (watch_rec *)TlsGetValue(watch_slot);
        if (w != NULL)
        {
            w->busy = 0;
        }
    }
    SYSTEM_INFO si;
    GetSystemInfo(&si);
    uintptr_t page = si.dwPageSize;
    volatile char here = 0;
    uintptr_t at = (uintptr_t)&here;
    MEMORY_BASIC_INFORMATION m;
    if (VirtualQuery((LPCVOID)at, &m, sizeof m) == 0)
    {
        return;
    }
    uintptr_t base = (uintptr_t)m.AllocationBase;
    ULONG g = 0;
    if (!SetThreadStackGuarantee(&g))
    {
        g = 0;
    }
    uintptr_t region = (((uintptr_t)g + page - 1) & ~(page - 1)) + page;
    /* The lowest committed page of the stack that may be used. */
    uintptr_t p = base;
    for (;;)
    {
        if (p >= at || VirtualQuery((LPCVOID)p, &m, sizeof m) == 0 || (uintptr_t)m.AllocationBase != base)
        {
            return;
        }
        if (m.State == MEM_COMMIT && (m.Protect & PAGE_GUARD))
        {
            return;
        }
        if (m.State == MEM_COMMIT && !(m.Protect & PAGE_NOACCESS))
        {
            break;
        }
        p = (uintptr_t)m.BaseAddress + m.RegionSize;
    }
    if (p < base + page)
    {
        p = base + page;
    }
    uintptr_t top = (at & ~(page - 1)) - page;
    if (p + region > top)
    {
        return;
    }
    DWORD old;
    VirtualProtect((LPVOID)p, region, PAGE_READWRITE | PAGE_GUARD, &old);
}

/* No own stack on Windows: the program runs on a thread with one. */
uintptr_t plat_reserve_stack(size_t size, size_t guard)
{
    (void)size;
    (void)guard;
    return 0;
}

/* ---- clocks ---- */

/* `Instant`: the performance counter, in nanoseconds. */
uint64_t plat_now_ns(void)
{
    static LONGLONG freq;
    LARGE_INTEGER c;
    if (!freq)
    {
        LARGE_INTEGER f;
        QueryPerformanceFrequency(&f);
        freq = f.QuadPart;
    }
    QueryPerformanceCounter(&c);
    uint64_t q = (uint64_t)c.QuadPart / (uint64_t)freq;
    uint64_t r = (uint64_t)c.QuadPart % (uint64_t)freq;
    return q * 1000000000u + r * 1000000000u / (uint64_t)freq;
}

/* `std::thread::sleep`: `Sleep` of the duration in whole milliseconds,
   rounded up. */
void plat_sleep_ns(uint64_t ns)
{
    uint64_t ms = ns / 1000000u + (ns % 1000000u ? 1 : 0);
    while (ms >= INFINITE)
    {
        Sleep(INFINITE - 1);
        ms -= INFINITE - 1;
    }
    Sleep((DWORD)ms);
}

typedef VOID(WINAPI *precise_fn)(LPFILETIME);

/* `SystemTime::now`: `GetSystemTimePreciseAsFileTime` where the system has
   it (Windows 8), else `GetSystemTimeAsFileTime`. */
void os_wall_time(int64_t *secs, uint32_t *nanos)
{
    static precise_fn precise;
    static int looked;
    if (!looked)
    {
        HMODULE k = GetModuleHandleW(L"kernel32.dll");
        if (k)
        {
            FARPROC f = GetProcAddress(k, "GetSystemTimePreciseAsFileTime");
            memcpy(&precise, &f, sizeof precise);
        }
        looked = 1;
    }
    FILETIME ft;
    if (precise)
    {
        precise(&ft);
    }
    else
    {
        GetSystemTimeAsFileTime(&ft);
    }
    int64_t t = (int64_t)((uint64_t)ft.dwHighDateTime << 32 | ft.dwLowDateTime);
    /* 100-nanosecond intervals since 1601. */
    int64_t u = t - 116444736000000000LL;
    int64_t s = u / 10000000;
    int64_t rem = u % 10000000;
    if (rem < 0)
    {
        rem += 10000000;
        s--;
    }
    *secs = s;
    *nanos = (uint32_t)(rem * 100);
}

static int64_t div_euclid(int64_t a, int64_t b)
{
    int64_t q = a / b;
    if (a % b < 0)
    {
        q = b > 0 ? q - 1 : q + 1;
    }
    return q;
}

/* The civil date of a day count (`civil_from_days`): (year, month, day). */
static void civil_from_days(int64_t z, int64_t *y, int64_t *m, int64_t *d)
{
    z += 719468;
    int64_t era = div_euclid(z, 146097);
    int64_t doe = z - era * 146097;
    int64_t yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    int64_t doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    int64_t mp = (5 * doy + 2) / 153;
    *d = doy - (153 * mp + 2) / 5 + 1;
    *m = mp < 10 ? mp + 3 : mp - 9;
    *y = yoe + era * 400 + (*m <= 2 ? 1 : 0);
}

int os_local_time(int64_t when, int64_t tm6[6])
{
    int64_t days = div_euclid(when, 86400);
    int64_t secs = when - days * 86400;
    int64_t y, m, d;
    civil_from_days(days, &y, &m, &d);
    /* A SYSTEMTIME's year is 1601 ..= 30827. */
    if (y < 1601 || y > 30827)
    {
        return 0;
    }
    SYSTEMTIME utc, local;
    memset(&utc, 0, sizeof utc);
    utc.wYear = (WORD)y;
    utc.wMonth = (WORD)m;
    utc.wDay = (WORD)d;
    utc.wHour = (WORD)(secs / 3600);
    utc.wMinute = (WORD)(secs % 3600 / 60);
    utc.wSecond = (WORD)(secs % 60);
    TIME_ZONE_INFORMATION tz;
    memset(&tz, 0, sizeof tz);
    memset(&local, 0, sizeof local);
    if (!GetTimeZoneInformationForYear((USHORT)y, NULL, &tz))
    {
        return 0;
    }
    if (!SystemTimeToTzSpecificLocalTime(&tz, &utc, &local))
    {
        return 0;
    }
    tm6[0] = local.wYear;
    tm6[1] = local.wMonth;
    tm6[2] = local.wDay;
    tm6[3] = local.wHour;
    tm6[4] = local.wMinute;
    tm6[5] = local.wSecond;
    return 1;
}

/* ---- UTF-16 ---- */

typedef struct
{
    WCHAR *p;
    size_t n, cap;
} wbuf;

static void w_put(wbuf *w, WCHAR c)
{
    if (w->n == w->cap)
    {
        w->cap = w->cap ? w->cap * 2 : 64;
        w->p = (WCHAR *)cb_xrealloc(w->p, w->cap * sizeof(WCHAR));
    }
    w->p[w->n++] = c;
}

static void w_puts(wbuf *w, const WCHAR *s, size_t n)
{
    for (size_t i = 0; i < n; i++)
    {
        w_put(w, s[i]);
    }
}

static void w_free(wbuf *w)
{
    free(w->p);
    w->p = NULL;
    w->n = w->cap = 0;
}

/* UTF-8 (or WTF-8) to UTF-16, appended; 0 when it holds a NUL (Rust's
   `to_u16s`: "strings passed to WinAPI cannot contain NULs"). */
static int to_wide(const uint8_t *s, size_t n, wbuf *w)
{
    size_t i = 0;
    while (i < n)
    {
        uint32_t c = s[i];
        size_t k = 1;
        if (c >= 0xF0 && i + 3 < n)
        {
            c = (c & 7) << 18 | (uint32_t)(s[i + 1] & 63) << 12 | (uint32_t)(s[i + 2] & 63) << 6 | (s[i + 3] & 63);
            k = 4;
        }
        else if (c >= 0xE0 && i + 2 < n)
        {
            c = (c & 15) << 12 | (uint32_t)(s[i + 1] & 63) << 6 | (s[i + 2] & 63);
            k = 3;
        }
        else if (c >= 0xC0 && i + 1 < n)
        {
            c = (c & 31) << 6 | (s[i + 1] & 63);
            k = 2;
        }
        if (c == 0)
        {
            return 0;
        }
        if (c >= 0x10000)
        {
            c -= 0x10000;
            w_put(w, (WCHAR)(0xD800 + (c >> 10)));
            w_put(w, (WCHAR)(0xDC00 + (c & 0x3FF)));
        }
        else
        {
            w_put(w, (WCHAR)c);
        }
        i += k;
    }
    return 1;
}

/* UTF-16 to WTF-8 (Rust's `OsString::from_wide`), appended: an unpaired
   surrogate kept as its three bytes. */
static void from_wide(const WCHAR *s, size_t n, cb_buf *out)
{
    for (size_t i = 0; i < n; i++)
    {
        uint32_t c = s[i];
        if (c >= 0xD800 && c <= 0xDBFF && i + 1 < n && s[i + 1] >= 0xDC00 && s[i + 1] <= 0xDFFF)
        {
            c = 0x10000 + ((c - 0xD800) << 10) + (s[i + 1] - 0xDC00u);
            i++;
        }
        if (c < 0x80)
        {
            cb_buf_byte(out, (uint8_t)c);
        }
        else if (c < 0x800)
        {
            cb_buf_byte(out, (uint8_t)(0xC0 | c >> 6));
            cb_buf_byte(out, (uint8_t)(0x80 | (c & 63)));
        }
        else if (c < 0x10000)
        {
            cb_buf_byte(out, (uint8_t)(0xE0 | c >> 12));
            cb_buf_byte(out, (uint8_t)(0x80 | (c >> 6 & 63)));
            cb_buf_byte(out, (uint8_t)(0x80 | (c & 63)));
        }
        else
        {
            cb_buf_byte(out, (uint8_t)(0xF0 | c >> 18));
            cb_buf_byte(out, (uint8_t)(0x80 | (c >> 12 & 63)));
            cb_buf_byte(out, (uint8_t)(0x80 | (c >> 6 & 63)));
            cb_buf_byte(out, (uint8_t)(0x80 | (c & 63)));
        }
    }
}

/* `String::from_utf16_lossy`: an unpaired surrogate as U+FFFD. */
static void from_wide_lossy(const WCHAR *s, size_t n, cb_buf *out)
{
    for (size_t i = 0; i < n; i++)
    {
        WCHAR c = s[i];
        if (c >= 0xD800 && c <= 0xDFFF)
        {
            int paired = c <= 0xDBFF && i + 1 < n && s[i + 1] >= 0xDC00 && s[i + 1] <= 0xDFFF;
            if (!paired)
            {
                cb_buf_put(out, "\xEF\xBF\xBD", 3);
                continue;
            }
            from_wide(s + i, 2, out);
            i++;
            continue;
        }
        from_wide(s + i, 1, out);
    }
}

/* Rust's `fill_utf16_buf`: an API answering into a buffer, asked again
   with more room until the answer fits. 0 with the error in
   GetLastError's place. */
typedef DWORD (*fill_fn)(void *ctx, WCHAR *buf, DWORD n);

static int fill_utf16(fill_fn f, void *ctx, wbuf *out)
{
    DWORD n = 512;
    for (;;)
    {
        WCHAR *buf = (WCHAR *)cb_xmalloc((size_t)n * sizeof(WCHAR));
        SetLastError(0);
        DWORD k = f(ctx, buf, n);
        if (k == 0 && GetLastError() != 0)
        {
            free(buf);
            return 0;
        }
        if (k == n)
        {
            if (GetLastError() != ERROR_INSUFFICIENT_BUFFER)
            {
                free(buf);
                SetLastError(ERROR_INSUFFICIENT_BUFFER);
                return 0;
            }
            n = n * 2 > n ? n * 2 : n;
            free(buf);
            continue;
        }
        if (k > n)
        {
            n = k;
            free(buf);
            continue;
        }
        out->n = 0;
        w_puts(out, buf, k);
        free(buf);
        return 1;
    }
}

/* ---- errors ---- */

/* `decode_error_kind` for system and socket errors, as far as the modules
   tell kinds apart; the three "unreachable" socket errors by number
   (netio's `unreachable`). */
static int kind_of(DWORD e)
{
    switch (e)
    {
    case 0:
        return OS_OK;
    case ERROR_FILE_NOT_FOUND:
    case ERROR_PATH_NOT_FOUND:
        return OS_NOT_FOUND;
    case ERROR_ACCESS_DENIED:
    case WSAEACCES:
        return OS_DENIED;
    case ERROR_ALREADY_EXISTS:
    case ERROR_FILE_EXISTS:
        return OS_EXISTS;
    case ERROR_BROKEN_PIPE:
    case ERROR_NO_DATA:
        return OS_BROKEN_PIPE;
    case ERROR_INVALID_PARAMETER:
    case WSAEINVAL:
        return OS_INVALID;
    case ERROR_SEM_TIMEOUT:
    case WAIT_TIMEOUT:
    case 594: /* ERROR_DRIVER_CANCEL_TIMEOUT */
    case ERROR_OPERATION_ABORTED:
    case ERROR_SERVICE_REQUEST_TIMEOUT:
    case ERROR_COUNTER_TIMEOUT:
    case ERROR_TIMEOUT:
    case 5910: /* ERROR_RESOURCE_CALL_TIMED_OUT */
    case ERROR_CTX_MODEM_RESPONSE_TIMEOUT:
    case ERROR_CTX_CLIENT_QUERY_TIMEOUT:
    case FRS_ERR_SYSVOL_POPULATE_TIMEOUT:
    case ERROR_DS_TIMELIMIT_EXCEEDED:
    case DNS_ERROR_RECORD_TIMED_OUT:
    case ERROR_IPSEC_IKE_TIMED_OUT:
    case ERROR_RUNLEVEL_SWITCH_TIMEOUT:
    case ERROR_RUNLEVEL_SWITCH_AGENT_TIMEOUT:
    case WSAETIMEDOUT:
        return OS_TIMED_OUT;
    case WSAEINTR:
        return OS_INTERRUPTED;
    case WSAEWOULDBLOCK:
        return OS_WOULD_BLOCK;
    case WSAECONNREFUSED:
        return OS_REFUSED;
    case WSAECONNRESET:
        return OS_RESET;
    case WSAECONNABORTED:
        return OS_ABORTED;
    case WSAEADDRINUSE:
        return OS_ADDR_IN_USE;
    case WSAEADDRNOTAVAIL:
        return OS_ADDR_NOT_AVAILABLE;
    case WSAENOTCONN:
        return OS_NOT_CONNECTED;
    case WSAENETDOWN:
    case WSAENETUNREACH:
    case WSAEHOSTUNREACH:
        return OS_UNREACHABLE;
    case ERROR_DIRECTORY:
        return OS_NOT_DIR;
    case ERROR_DISK_FULL:
    case ERROR_HANDLE_DISK_FULL:
        return OS_NO_SPACE;
    case ERROR_TOO_MANY_OPEN_FILES:
    case WSAEMFILE:
        return OS_TOO_MANY_OPEN;
    case WSAEMSGSIZE:
        return OS_MESSAGE_TOO_LONG;
    default:
        return OS_OTHER;
    }
}

static int last(void)
{
    return kind_of(GetLastError());
}

static int wsa_last(void)
{
    return kind_of((DWORD)WSAGetLastError());
}

/* ---- paths ---- */

#define LEGACY_MAX_PATH 248

static int is_sepw(WCHAR c)
{
    return c == L'\\' || c == L'/';
}

/* Rust's `get_long_path`: a path the legacy MAX_PATH limits do not apply
   to. Already verbatim (`\\?\`, `\??\`), empty, or short and absolute
   (`C:\...`, `\\...`): as it is. Otherwise made absolute by
   `GetFullPathNameW`, and given the verbatim prefix when `prefer_verbatim`
   or when it is long. `w` holds the path with its NUL. */
static int long_path(wbuf *w, int prefer_verbatim)
{
    const WCHAR *p = w->p;
    size_t n = w->n; /* with the NUL */
    if ((n >= 4 && p[0] == L'\\' && p[1] == L'\\' && p[2] == L'?' && p[3] == L'\\') || (n >= 4 && p[0] == L'\\' && p[1] == L'?' && p[2] == L'?' && p[3] == L'\\') || n == 1)
    {
        return 1;
    }
    if (n < LEGACY_MAX_PATH)
    {
        if (n >= 3 && p[1] == L':' && !is_sepw(p[0]) && (p[2] == 0 || is_sepw(p[2])))
        {
            return 1;
        }
        if (n >= 2 && is_sepw(p[0]) && is_sepw(p[1]))
        {
            return 1;
        }
    }
    DWORD need = GetFullPathNameW(w->p, 0, NULL, NULL);
    if (need == 0)
    {
        return 0;
    }
    WCHAR *abs = (WCHAR *)cb_xmalloc(((size_t)need + 1) * sizeof(WCHAR));
    DWORD k = GetFullPathNameW(w->p, need + 1, abs, NULL);
    if (k == 0 || k > need)
    {
        free(abs);
        return 0;
    }
    WCHAR *a = abs;
    size_t an = k;
    wbuf r = {0};
    if (prefer_verbatim || an + 1 >= LEGACY_MAX_PATH)
    {
        if (an >= 3 && a[1] == L':' && a[2] == L'\\')
        {
            w_puts(&r, L"\\\\?\\", 4);
        }
        else if (an >= 4 && a[0] == L'\\' && a[1] == L'\\' && a[2] == L'.' && a[3] == L'\\')
        {
            a += 4;
            an -= 4;
            w_puts(&r, L"\\\\?\\", 4);
        }
        else if ((an >= 4 && a[0] == L'\\' && a[1] == L'\\' && a[2] == L'?' && a[3] == L'\\') || (an >= 4 && a[0] == L'\\' && a[1] == L'?' && a[2] == L'?' && a[3] == L'\\'))
        {
        }
        else if (an >= 2 && a[0] == L'\\' && a[1] == L'\\')
        {
            a += 2;
            an -= 2;
            w_puts(&r, L"\\\\?\\UNC\\", 8);
        }
    }
    w_puts(&r, a, an);
    w_put(&r, 0);
    free(abs);
    w_free(w);
    *w = r;
    return 1;
}

/* `maybe_verbatim(path)`: what `std::fs` hands the system. 0 for a path
   with a NUL (OS_INVALID) or one `GetFullPathNameW` refuses. */
static int vpath(const uint8_t *p, size_t n, wbuf *w, int *kind)
{
    w->n = 0;
    if (!to_wide(p, n, w))
    {
        *kind = OS_INVALID;
        return 0;
    }
    w_put(w, 0);
    if (!long_path(w, 1))
    {
        *kind = last();
        return 0;
    }
    return 1;
}

/* `to_u16s`: the path as it is. */
static int upath(const uint8_t *p, size_t n, wbuf *w)
{
    w->n = 0;
    if (!to_wide(p, n, w))
    {
        return 0;
    }
    w_put(w, 0);
    return 1;
}

#define VPATH(var, p, n)          \
    wbuf var = {0};               \
    {                             \
        int k_;                   \
        if (!vpath(p, n, &var, &k_)) \
        {                         \
            w_free(&var);         \
            return k_;            \
        }                         \
    }

/* ---- the process ---- */

void *plat_aligned_alloc(size_t size, size_t align)
{
    return _aligned_malloc(size ? size : 1, align);
}

void plat_aligned_free(void *p)
{
    _aligned_free(p);
}

void plat_exit(int status)
{
    exit(status);
}

void plat_abort(void)
{
    abort();
}

typedef struct env_cache
{
    char *name;
    char *value;
    struct env_cache *next;
} env_cache;

static plat_mutex env_lock = PLAT_MUTEX_INIT;
static env_cache *env_seen;

static DWORD env_fill(void *ctx, WCHAR *buf, DWORD n)
{
    return GetEnvironmentVariableW((const WCHAR *)ctx, buf, n);
}

/* An environment variable as `env::var_os` reads it (UTF-16 as WTF-8), as
   C text; the answers are kept, for the runtime reads each name once. */
const char *plat_getenv(const char *name)
{
    cb_buf v = {0};
    if (!os_env_var((const uint8_t *)name, strlen(name), &v))
    {
        cb_buf_free(&v);
        return NULL;
    }
    cb_buf_byte(&v, 0);
    plat_mutex_lock(&env_lock);
    for (env_cache *e = env_seen; e; e = e->next)
    {
        if (strcmp(e->name, name) == 0 && strcmp(e->value, (const char *)v.p) == 0)
        {
            plat_mutex_unlock(&env_lock);
            cb_buf_free(&v);
            return e->value;
        }
    }
    env_cache *e = (env_cache *)cb_xmalloc(sizeof *e);
    e->name = _strdup(name);
    e->value = (char *)v.p;
    e->next = env_seen;
    env_seen = e;
    plat_mutex_unlock(&env_lock);
    return e->value;
}

int os_env_var(const uint8_t *name, size_t n, cb_buf *out)
{
    out->n = 0;
    wbuf k = {0};
    if (!upath(name, n, &k))
    {
        w_free(&k);
        return 0;
    }
    wbuf v = {0};
    int ok = fill_utf16(env_fill, k.p, &v);
    w_free(&k);
    if (!ok)
    {
        w_free(&v);
        return 0;
    }
    from_wide(v.p, v.n, out);
    w_free(&v);
    return 1;
}

/* D-0255: the command line itself split as Rust's `args_os` splits it
   (`parse_lp_cmd_line`, the rules of the 2008 C runtime), the program's
   name skipped. */
void plat_args(int argc, const char *const *argv, uint8_t ***args, size_t **lens, size_t *count)
{
    (void)argc;
    (void)argv;
    /* The C library's own standard streams in binary mode too, as the
       runtime's are, so C code a program links (`puts`) writes `\n`, not
       `\r\n`. */
    for (int fd = 0; fd < 3; fd++)
    {
        _setmode(fd, _O_BINARY);
    }
    const WCHAR *s = GetCommandLineW();
    size_t cap = 8, k = 0;
    uint8_t **a = (uint8_t **)cb_xmalloc(cap * sizeof(uint8_t *));
    size_t *l = (size_t *)cb_xmalloc(cap * sizeof(size_t));
    if (s && *s)
    {
        /* The program's name: quotes toggle, no escapes; ends at a space or
           tab outside quotes. */
        int inq = 0;
        while (*s)
        {
            WCHAR c = *s++;
            if (c == L'"')
            {
                inq = !inq;
            }
            else if ((c == L' ' || c == L'\t') && !inq)
            {
                break;
            }
        }
        while (*s == L' ' || *s == L'\t')
        {
            s++;
        }
        wbuf cur = {0};
        inq = 0;
        int any = 0;
#define PUSH_ARG()                                                        \
    do                                                                    \
    {                                                                     \
        cb_buf b_ = {0};                                                  \
        from_wide(cur.p, cur.n, &b_);                                     \
        cb_buf_byte(&b_, 0);                                              \
        if (k == cap)                                                     \
        {                                                                 \
            cap *= 2;                                                     \
            a = (uint8_t **)cb_xrealloc(a, cap * sizeof(uint8_t *));      \
            l = (size_t *)cb_xrealloc(l, cap * sizeof(size_t));           \
        }                                                                 \
        a[k] = b_.p;                                                      \
        l[k] = b_.n - 1;                                                  \
        k++;                                                              \
        cur.n = 0;                                                        \
    } while (0)
        while (*s)
        {
            WCHAR c = *s++;
            if ((c == L' ' || c == L'\t') && !inq)
            {
                PUSH_ARG();
                while (*s == L' ' || *s == L'\t')
                {
                    s++;
                }
            }
            else if (c == L'\\')
            {
                size_t bs = 1;
                while (*s == L'\\')
                {
                    s++;
                    bs++;
                }
                if (*s == L'"')
                {
                    for (size_t i = 0; i < bs / 2; i++)
                    {
                        w_put(&cur, L'\\');
                    }
                    if (bs % 2 == 1)
                    {
                        s++;
                        w_put(&cur, L'"');
                    }
                }
                else
                {
                    for (size_t i = 0; i < bs; i++)
                    {
                        w_put(&cur, L'\\');
                    }
                }
            }
            else if (c == L'"' && inq)
            {
                if (*s == L'"')
                {
                    w_put(&cur, L'"');
                    s++;
                }
                else if (*s)
                {
                    inq = 0;
                }
                else
                {
                    /* The end: push `cur` even if empty. */
                    break;
                }
            }
            else if (c == L'"')
            {
                inq = 1;
            }
            else
            {
                w_put(&cur, c);
            }
        }
        if (cur.n || inq)
        {
            PUSH_ARG();
        }
        (void)any;
#undef PUSH_ARG
        w_free(&cur);
    }
    *args = a;
    *lens = l;
    *count = k;
}

/* `is_terminal`: a console, or an MSYS or Cygwin terminal's pipe. */
static int handle_is_tty(HANDLE h)
{
    if (!h || h == INVALID_HANDLE_VALUE)
    {
        return 0;
    }
    DWORD mode;
    if (GetConsoleMode(h, &mode))
    {
        return 1;
    }
    if (GetFileType(h) != FILE_TYPE_PIPE)
    {
        return 0;
    }
    struct
    {
        DWORD len;
        WCHAR name[MAX_PATH];
    } info;
    memset(&info, 0, sizeof info);
    if (!GetFileInformationByHandleEx(h, FileNameInfo, &info, sizeof info))
    {
        return 0;
    }
    size_t n = info.len / 2;
    if (n > MAX_PATH)
    {
        return 0;
    }
    cb_buf name = {0};
    from_wide_lossy(info.name, n, &name);
    cb_buf_byte(&name, 0);
    const char *s = (const char *)name.p;
    const char *slash = strrchr(s, '\\');
    if (slash)
    {
        s = slash + 1;
    }
    int r = (strncmp(s, "msys-", 5) == 0 || strncmp(s, "cygwin-", 7) == 0) && strstr(s, "-pty") != NULL;
    cb_buf_free(&name);
    return r;
}

int plat_stderr_is_tty(void)
{
    return handle_is_tty(GetStdHandle(STD_ERROR_HANDLE));
}

int plat_stdout_is_tty(void)
{
    return handle_is_tty(GetStdHandle(STD_OUTPUT_HANDLE));
}

int os_stdout_is_tty(void)
{
    return plat_stdout_is_tty();
}

/* Older consoles print ANSI sequences raw unless asked to process them. */
int plat_enable_ansi(void)
{
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
    return SetConsoleMode(h, mode | 0x0004) != 0;
}

/* ---- standard streams ---- */

static int is_console(HANDLE h)
{
    DWORD mode;
    return GetConsoleMode(h, &mode) != 0;
}

static size_t utf8_width(uint8_t b)
{
    if (b < 0x80)
    {
        return 1;
    }
    if (b >= 0xC2 && b <= 0xDF)
    {
        return 2;
    }
    if (b >= 0xE0 && b <= 0xEF)
    {
        return 3;
    }
    if (b >= 0xF0 && b <= 0xF4)
    {
        return 4;
    }
    return 0;
}

/* The length of the valid UTF-8 prefix of `p[0..n)`. */
static size_t utf8_valid_up_to(const uint8_t *p, size_t n)
{
    size_t i = 0;
    while (i < n)
    {
        uint8_t c = p[i];
        size_t w = utf8_width(c);
        if (w == 0 || n - i < w)
        {
            return i;
        }
        uint8_t lo = 0x80, hi = 0xBF;
        if (c == 0xE0)
        {
            lo = 0xA0;
        }
        else if (c == 0xED)
        {
            hi = 0x9F;
        }
        else if (c == 0xF0)
        {
            lo = 0x90;
        }
        else if (c == 0xF4)
        {
            hi = 0x8F;
        }
        if (w > 1 && (p[i + 1] < lo || p[i + 1] > hi))
        {
            return i;
        }
        for (size_t k = 2; k < w; k++)
        {
            if (p[i + k] < 0x80 || p[i + k] > 0xBF)
            {
                return i;
            }
        }
        i += w;
    }
    return n;
}

static int write_console_utf8(HANDLE h, const uint8_t *p, size_t n)
{
    wbuf w = {0};
    to_wide(p, n, &w);
    size_t at = 0;
    while (at < w.n)
    {
        DWORD put = 0;
        DWORD chunk = (DWORD)(w.n - at > 4096 ? 4096 : w.n - at);
        if (!WriteConsoleW(h, w.p + at, chunk, &put, NULL))
        {
            int k = last();
            w_free(&w);
            return k;
        }
        if (put == 0)
        {
            w_free(&w);
            return OS_WRITE_ZERO;
        }
        at += put;
    }
    w_free(&w);
    return OS_OK;
}

/* A console stream's bytes of a character not yet complete (Rust's
   `IncompleteUtf8`). */
typedef struct
{
    uint8_t b[4];
    size_t n;
} incomplete;

static plat_mutex std_lock[3] = {PLAT_MUTEX_INIT, PLAT_MUTEX_INIT, PLAT_MUTEX_INIT};
static incomplete std_inc[3];

int os_std_write(int which, const uint8_t *p, size_t n)
{
    if (n == 0)
    {
        return OS_OK;
    }
    HANDLE h = GetStdHandle(which == 1 ? STD_OUTPUT_HANDLE : STD_ERROR_HANDLE);
    /* Rust's `handle_ebadf`: no stream (a null handle, ERROR_INVALID_HANDLE)
       swallows the bytes. */
    if (h == NULL)
    {
        return OS_OK;
    }
    if (h == INVALID_HANDLE_VALUE)
    {
        return GetLastError() == ERROR_INVALID_HANDLE ? OS_OK : last();
    }
    if (!is_console(h))
    {
        while (n > 0)
        {
            DWORD put = 0;
            DWORD chunk = n > 0x7FFFFFFF ? 0x7FFFFFFF : (DWORD)n;
            if (!WriteFile(h, p, chunk, &put, NULL))
            {
                DWORD e = GetLastError();
                if (e == ERROR_INVALID_HANDLE)
                {
                    return OS_OK;
                }
                return kind_of(e);
            }
            if (put == 0)
            {
                return OS_WRITE_ZERO;
            }
            p += put;
            n -= put;
        }
        return OS_OK;
    }
    /* A console: the text as UTF-16; bytes that are not UTF-8 are an error
       ("Windows stdio in console mode does not support writing non-UTF-8
       byte sequences"); a character cut at the end of a write is kept for
       the next. */
    plat_mutex_lock(&std_lock[which]);
    incomplete *inc = &std_inc[which];
    int k = OS_OK;
    while (n > 0 && k == OS_OK)
    {
        if (inc->n > 0)
        {
            if (p[0] >> 6 != 2)
            {
                inc->n = 0;
                k = OS_OTHER;
                break;
            }
            inc->b[inc->n++] = p[0];
            p++;
            n--;
            size_t w = utf8_width(inc->b[0]);
            if (inc->n < w)
            {
                continue;
            }
            size_t ok = utf8_valid_up_to(inc->b, inc->n);
            size_t m = inc->n;
            inc->n = 0;
            if (ok != m)
            {
                k = OS_OTHER;
                break;
            }
            k = write_console_utf8(h, inc->b, m);
            continue;
        }
        size_t len = n < 4096 ? n : 4096;
        size_t ok = utf8_valid_up_to(p, len);
        if (ok == 0)
        {
            size_t w = utf8_width(p[0]);
            if (w > 1 && n < w)
            {
                inc->b[0] = p[0];
                inc->n = 1;
                p++;
                n--;
                continue;
            }
            k = OS_OTHER;
            break;
        }
        k = write_console_utf8(h, p, ok);
        p += ok;
        n -= ok;
    }
    plat_mutex_unlock(&std_lock[which]);
    return k;
}

/* Console input as UTF-8: what one `ReadConsoleW` gave, waiting to be
   taken. */
static cb_buf con_in;
static size_t con_at;
static WCHAR con_surrogate;

int os_stdin_read(uint8_t *p, size_t n, size_t *got)
{
    *got = 0;
    HANDLE h = GetStdHandle(STD_INPUT_HANDLE);
    if (h == NULL)
    {
        return OS_OK;
    }
    if (h == INVALID_HANDLE_VALUE)
    {
        return GetLastError() == ERROR_INVALID_HANDLE ? OS_OK : last();
    }
    if (!is_console(h))
    {
        DWORD r = 0;
        DWORD chunk = n > 0x7FFFFFFF ? 0x7FFFFFFF : (DWORD)n;
        if (!ReadFile(h, p, chunk, &r, NULL))
        {
            DWORD e = GetLastError();
            if (e == ERROR_BROKEN_PIPE || e == ERROR_INVALID_HANDLE)
            {
                return OS_OK;
            }
            return kind_of(e);
        }
        *got = r;
        return OS_OK;
    }
    if (con_at >= con_in.n)
    {
        con_in.n = 0;
        con_at = 0;
        WCHAR buf[4096];
        DWORD want = (DWORD)(n / 3 < 4096 ? (n / 3 ? n / 3 : 1) : 4096);
        DWORD r = 0;
        CONSOLE_READCONSOLE_CONTROL ctl;
        memset(&ctl, 0, sizeof ctl);
        ctl.nLength = sizeof ctl;
        ctl.dwCtrlWakeupMask = 1u << 0x1A;
        for (;;)
        {
            if (!ReadConsoleW(h, buf, want, &r, &ctl))
            {
                return last();
            }
            /* Ctrl-C ends a read with nothing: read again. */
            if (r == 0 && GetLastError() == ERROR_OPERATION_ABORTED)
            {
                continue;
            }
            break;
        }
        /* Ctrl-Z ends the input. */
        if (r > 0 && buf[r - 1] == 0x1A)
        {
            r--;
        }
        wbuf w = {0};
        if (con_surrogate)
        {
            w_put(&w, con_surrogate);
            con_surrogate = 0;
        }
        w_puts(&w, buf, r);
        if (w.n > 0 && w.p[w.n - 1] >= 0xD800 && w.p[w.n - 1] <= 0xDBFF)
        {
            con_surrogate = w.p[--w.n];
        }
        for (size_t i = 0; i < w.n; i++)
        {
            WCHAR c = w.p[i];
            if (c >= 0xD800 && c <= 0xDFFF)
            {
                int paired = c <= 0xDBFF && i + 1 < w.n && w.p[i + 1] >= 0xDC00 && w.p[i + 1] <= 0xDFFF;
                if (!paired)
                {
                    /* "Windows stdin in console mode does not support
                       non-UTF-16 input; encountered unpaired surrogate" */
                    w_free(&w);
                    return OS_OTHER;
                }
                i++;
            }
        }
        from_wide(w.p, w.n, &con_in);
        w_free(&w);
    }
    size_t m = con_in.n - con_at;
    if (m > n)
    {
        m = n;
    }
    memcpy(p, con_in.p + con_at, m);
    con_at += m;
    *got = m;
    return OS_OK;
}

/* ---- files ---- */

static HANDLE fh(os_file f)
{
    return (HANDLE)f;
}

/* `OpenOptions::open` for fileio's MODE_*: the access and the creation
   disposition Rust gives each, sharing read, write and delete. */
static int open_mode(const uint8_t *path, size_t n, DWORD access, DWORD creation, DWORD flags, HANDLE *out)
{
    VPATH(w, path, n);
    HANDLE h = CreateFileW(w.p, access, FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE, NULL, creation, flags, NULL);
    DWORD e = GetLastError();
    w_free(&w);
    if (h == INVALID_HANDLE_VALUE)
    {
        return kind_of(e);
    }
    *out = h;
    return OS_OK;
}

int os_file_open(const uint8_t *path, size_t n, int mode, os_file *f)
{
    DWORD access, creation, flags = 0;
    switch (mode)
    {
    case 0:
        access = GENERIC_READ;
        creation = OPEN_EXISTING;
        break;
    case 1:
        access = GENERIC_WRITE;
        creation = CREATE_ALWAYS;
        break;
    case 2:
        access = FILE_GENERIC_WRITE & ~FILE_WRITE_DATA;
        creation = OPEN_ALWAYS;
        break;
    case 3:
        access = GENERIC_READ | GENERIC_WRITE;
        creation = OPEN_ALWAYS;
        break;
    default:
        access = GENERIC_WRITE;
        creation = CREATE_NEW;
        /* `create_new` does not follow a link at the name. */
        flags = FILE_FLAG_OPEN_REPARSE_POINT;
        break;
    }
    HANDLE h;
    int k = open_mode(path, n, access, creation, flags, &h);
    if (k == OS_OK)
    {
        *f = (os_file)h;
    }
    return k;
}

/* `Handle::read`: a broken pipe reads as the end. */
static int handle_read(HANDLE h, uint8_t *p, size_t n, size_t *got)
{
    DWORD r = 0;
    DWORD chunk = n > 0xFFFFFFFFu ? 0xFFFFFFFFu : (DWORD)n;
    if (!ReadFile(h, p, chunk, &r, NULL))
    {
        DWORD e = GetLastError();
        *got = 0;
        if (e == ERROR_BROKEN_PIPE)
        {
            return OS_OK;
        }
        return kind_of(e);
    }
    *got = r;
    return OS_OK;
}

static int handle_write(HANDLE h, const uint8_t *p, size_t n, size_t *put)
{
    DWORD w = 0;
    DWORD chunk = n > 0xFFFFFFFFu ? 0xFFFFFFFFu : (DWORD)n;
    if (!WriteFile(h, p, chunk, &w, NULL))
    {
        *put = 0;
        return last();
    }
    *put = w;
    return OS_OK;
}

int os_file_read(os_file f, uint8_t *p, size_t n, size_t *got)
{
    return handle_read(fh(f), p, n, got);
}

int os_file_write(os_file f, const uint8_t *p, size_t n, size_t *put)
{
    return handle_write(fh(f), p, n, put);
}

int os_file_seek(os_file f, int whence, int64_t off, uint64_t *pos)
{
    LARGE_INTEGER d, np;
    d.QuadPart = off;
    if (!SetFilePointerEx(fh(f), d, &np, whence == 0 ? FILE_BEGIN : FILE_CURRENT))
    {
        return last();
    }
    *pos = (uint64_t)np.QuadPart;
    return OS_OK;
}

/* The attributes Rust's `FileAttr` keeps, with the reparse tag (a name
   surrogate, a link, when `& 0x20000000`). */
typedef struct
{
    DWORD attrs;
    DWORD tag;
    uint64_t size;
    FILETIME mtime;
} attr;

static int handle_attr(HANDLE h, attr *a)
{
    BY_HANDLE_FILE_INFORMATION info;
    if (!GetFileInformationByHandle(h, &info))
    {
        return last();
    }
    a->attrs = info.dwFileAttributes;
    a->size = (uint64_t)info.nFileSizeHigh << 32 | info.nFileSizeLow;
    a->mtime = info.ftLastWriteTime;
    a->tag = 0;
    if (a->attrs & FILE_ATTRIBUTE_REPARSE_POINT)
    {
        FILE_ATTRIBUTE_TAG_INFO t;
        if (GetFileInformationByHandleEx(h, FileAttributeTagInfo, &t, sizeof t))
        {
            a->tag = t.ReparseTag;
        }
    }
    return OS_OK;
}

static int attr_is_symlink(const attr *a)
{
    return (a->attrs & FILE_ATTRIBUTE_REPARSE_POINT) && (a->tag & 0x20000000);
}

static int attr_is_dir(const attr *a)
{
    return !attr_is_symlink(a) && (a->attrs & FILE_ATTRIBUTE_DIRECTORY);
}

int os_file_stat(os_file f, uint64_t *len, int *is_dir)
{
    attr a;
    int k = handle_attr(fh(f), &a);
    if (k != OS_OK)
    {
        return k;
    }
    *len = a.size;
    *is_dir = attr_is_dir(&a);
    return OS_OK;
}

int os_file_sync(os_file f)
{
    return FlushFileBuffers(fh(f)) ? OS_OK : last();
}

void os_file_close(os_file f)
{
    CloseHandle(fh(f));
}

int os_read_file(const uint8_t *path, size_t n, cb_buf *out)
{
    HANDLE h;
    int k = open_mode(path, n, GENERIC_READ, OPEN_EXISTING, 0, &h);
    if (k != OS_OK)
    {
        return k;
    }
    out->n = 0;
    LARGE_INTEGER sz;
    if (GetFileSizeEx(h, &sz) && sz.QuadPart > 0)
    {
        cb_buf_reserve(out, (size_t)sz.QuadPart);
    }
    for (;;)
    {
        cb_buf_reserve(out, 8192);
        size_t got = 0;
        k = handle_read(h, out->p + out->n, out->cap - out->n, &got);
        if (k != OS_OK || got == 0)
        {
            break;
        }
        out->n += got;
    }
    CloseHandle(h);
    return k;
}

int os_write_file(const uint8_t *path, size_t n, const uint8_t *d, size_t dn)
{
    HANDLE h;
    int k = open_mode(path, n, GENERIC_WRITE, CREATE_ALWAYS, 0, &h);
    if (k != OS_OK)
    {
        return k;
    }
    while (dn > 0)
    {
        size_t put = 0;
        k = handle_write(h, d, dn, &put);
        if (k != OS_OK)
        {
            break;
        }
        if (put == 0)
        {
            k = OS_WRITE_ZERO;
            break;
        }
        d += put;
        dn -= put;
    }
    CloseHandle(h);
    return k;
}

/* `fs::metadata` (Rust's `stat`): the file opened for no access with
   FILE_FLAG_BACKUP_SEMANTICS (so a directory opens); on a sharing
   violation or a denied access, the directory's cached record
   (`FindFirstFileW`), unless that names a link. */
static int path_attr(const uint8_t *path, size_t n, attr *a)
{
    VPATH(w, path, n);
    HANDLE h = CreateFileW(w.p, 0, FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE, NULL, OPEN_EXISTING, FILE_FLAG_BACKUP_SEMANTICS, NULL);
    if (h != INVALID_HANDLE_VALUE)
    {
        w_free(&w);
        int k = handle_attr(h, a);
        CloseHandle(h);
        return k;
    }
    DWORD e = GetLastError();
    if (e == ERROR_SHARING_VIOLATION || e == ERROR_ACCESS_DENIED)
    {
        WIN32_FIND_DATAW fd;
        HANDLE f = FindFirstFileW(w.p, &fd);
        w_free(&w);
        if (f != INVALID_HANDLE_VALUE)
        {
            FindClose(f);
            a->attrs = fd.dwFileAttributes;
            a->size = (uint64_t)fd.nFileSizeHigh << 32 | fd.nFileSizeLow;
            a->mtime = fd.ftLastWriteTime;
            a->tag = (fd.dwFileAttributes & FILE_ATTRIBUTE_REPARSE_POINT) ? fd.dwReserved0 : 0;
            if (!attr_is_symlink(a))
            {
                return OS_OK;
            }
        }
        return kind_of(e);
    }
    w_free(&w);
    return kind_of(e);
}

int os_metadata(const uint8_t *path, size_t n, os_meta *m)
{
    attr a;
    int k = path_attr(path, n, &a);
    if (k != OS_OK)
    {
        return k;
    }
    int dir = attr_is_dir(&a);
    m->kind = dir ? 1 : !attr_is_symlink(&a) ? 0 : 2;
    m->len = a.size;
    m->has_modified = 1;
    int64_t t = (int64_t)((uint64_t)a.mtime.dwHighDateTime << 32 | a.mtime.dwLowDateTime) - 116444736000000000LL;
    int64_t s = t / 10000000;
    if (t % 10000000 < 0)
    {
        s--;
    }
    m->mod_secs = s;
    m->readonly = (a.attrs & FILE_ATTRIBUTE_READONLY) != 0;
    return OS_OK;
}

int os_is_dir(const uint8_t *path, size_t n)
{
    attr a;
    return path_attr(path, n, &a) == OS_OK && attr_is_dir(&a);
}

int os_mkdir(const uint8_t *path, size_t n)
{
    VPATH(w, path, n);
    BOOL ok = CreateDirectoryW(w.p, NULL);
    DWORD e = GetLastError();
    w_free(&w);
    return ok ? OS_OK : kind_of(e);
}

int os_remove_file(const uint8_t *path, size_t n)
{
    VPATH(w, path, n);
    BOOL ok = DeleteFileW(w.p);
    DWORD e = GetLastError();
    w_free(&w);
    return ok ? OS_OK : kind_of(e);
}

int os_remove_dir(const uint8_t *path, size_t n)
{
    VPATH(w, path, n);
    BOOL ok = RemoveDirectoryW(w.p);
    DWORD e = GetLastError();
    w_free(&w);
    return ok ? OS_OK : kind_of(e);
}

int os_rename(const uint8_t *a, size_t an, const uint8_t *b, size_t bn)
{
    VPATH(wa, a, an);
    wbuf wb = {0};
    int k;
    if (!vpath(b, bn, &wb, &k))
    {
        w_free(&wa);
        w_free(&wb);
        return k;
    }
    BOOL ok = MoveFileExW(wa.p, wb.p, MOVEFILE_REPLACE_EXISTING);
    DWORD e = GetLastError();
    w_free(&wa);
    w_free(&wb);
    return ok ? OS_OK : kind_of(e);
}

/* `p.join("*")` for `read_dir`: a separator unless the path ends in one or
   is a bare drive (`C:`). */
static void star_path(const uint8_t *p, size_t n, cb_buf *s)
{
    s->n = 0;
    cb_buf_put(s, p, n);
    int bare_drive = n == 2 && p[1] == ':' && ((p[0] >= 'a' && p[0] <= 'z') || (p[0] >= 'A' && p[0] <= 'Z'));
    if (n > 0 && p[n - 1] != '/' && p[n - 1] != '\\' && !bare_drive)
    {
        cb_buf_byte(s, '\\');
    }
    cb_buf_byte(s, '*');
}

int os_list_dir(const uint8_t *path, size_t n, cb_buf *names, size_t *count)
{
    *count = 0;
    names->n = 0;
    if (n == 0)
    {
        return OS_NOT_FOUND;
    }
    cb_buf star = {0};
    star_path(path, n, &star);
    wbuf w = {0};
    int k;
    if (!vpath(star.p, star.n, &w, &k))
    {
        cb_buf_free(&star);
        w_free(&w);
        return k;
    }
    cb_buf_free(&star);
    WIN32_FIND_DATAW fd;
    HANDLE f = FindFirstFileW(w.p, &fd);
    DWORD e = GetLastError();
    w_free(&w);
    if (f == INVALID_HANDLE_VALUE)
    {
        return kind_of(e);
    }
    for (;;)
    {
        const WCHAR *nm = fd.cFileName;
        if (!(nm[0] == L'.' && (nm[1] == 0 || (nm[1] == L'.' && nm[2] == 0))))
        {
            from_wide(nm, wcslen(nm), names);
            cb_buf_byte(names, 0);
            (*count)++;
        }
        if (!FindNextFileW(f, &fd))
        {
            e = GetLastError();
            FindClose(f);
            return e == ERROR_NO_MORE_FILES ? OS_OK : kind_of(e);
        }
    }
}

int os_canonicalize(const uint8_t *path, size_t n, cb_buf *out)
{
    VPATH(w, path, n);
    HANDLE h = CreateFileW(w.p, 0, FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE, NULL, OPEN_EXISTING, FILE_FLAG_BACKUP_SEMANTICS, NULL);
    DWORD e = GetLastError();
    w_free(&w);
    if (h == INVALID_HANDLE_VALUE)
    {
        return kind_of(e);
    }
    DWORD need = GetFinalPathNameByHandleW(h, NULL, 0, VOLUME_NAME_DOS);
    if (need == 0)
    {
        e = GetLastError();
        CloseHandle(h);
        return kind_of(e);
    }
    WCHAR *buf = (WCHAR *)cb_xmalloc(((size_t)need + 1) * sizeof(WCHAR));
    DWORD k = GetFinalPathNameByHandleW(h, buf, need + 1, VOLUME_NAME_DOS);
    e = GetLastError();
    CloseHandle(h);
    if (k == 0 || k > need)
    {
        free(buf);
        return kind_of(e);
    }
    out->n = 0;
    from_wide(buf, k, out);
    free(buf);
    return OS_OK;
}

int os_copy_file(const uint8_t *a, size_t an, const uint8_t *b, size_t bn, uint64_t *copied)
{
    VPATH(wa, a, an);
    wbuf wb = {0};
    int k;
    if (!vpath(b, bn, &wb, &k))
    {
        w_free(&wa);
        w_free(&wb);
        return k;
    }
    BOOL ok = CopyFileExW(wa.p, wb.p, NULL, NULL, NULL, 0);
    DWORD e = GetLastError();
    w_free(&wa);
    w_free(&wb);
    if (!ok)
    {
        return kind_of(e);
    }
    /* D-0255: the copy's size, as fileio reads it on Windows. */
    attr at;
    k = path_attr(b, bn, &at);
    if (k != OS_OK)
    {
        return k;
    }
    *copied = at.size;
    return OS_OK;
}

int os_copy_permissions(const uint8_t *a, size_t an, const uint8_t *b, size_t bn)
{
    attr at;
    int k = path_attr(a, an, &at);
    if (k != OS_OK)
    {
        return k;
    }
    VPATH(w, b, bn);
    BOOL ok = SetFileAttributesW(w.p, at.attrs);
    DWORD e = GetLastError();
    w_free(&w);
    return ok ? OS_OK : kind_of(e);
}

/* The directory `w` (with its NUL) emptied and removed: links not
   followed, a child already gone no failure. */
static int rm_tree(wbuf *w)
{
    wbuf pat = {0};
    w_puts(&pat, w->p, w->n - 1);
    if (pat.n && pat.p[pat.n - 1] != L'\\')
    {
        w_put(&pat, L'\\');
    }
    size_t base = pat.n;
    w_put(&pat, L'*');
    w_put(&pat, 0);
    WIN32_FIND_DATAW fd;
    HANDLE f = FindFirstFileW(pat.p, &fd);
    if (f == INVALID_HANDLE_VALUE)
    {
        DWORD e = GetLastError();
        w_free(&pat);
        return e == ERROR_FILE_NOT_FOUND ? OS_OK : kind_of(e);
    }
    int k = OS_OK;
    for (;;)
    {
        const WCHAR *nm = fd.cFileName;
        if (!(nm[0] == L'.' && (nm[1] == 0 || (nm[1] == L'.' && nm[2] == 0))))
        {
            wbuf child = {0};
            w_puts(&child, pat.p, base);
            w_puts(&child, nm, wcslen(nm));
            w_put(&child, 0);
            int r;
            int link = (fd.dwFileAttributes & FILE_ATTRIBUTE_REPARSE_POINT) && (fd.dwReserved0 & 0x20000000);
            if (fd.dwFileAttributes & FILE_ATTRIBUTE_DIRECTORY)
            {
                if (link)
                {
                    r = RemoveDirectoryW(child.p) ? OS_OK : last();
                }
                else
                {
                    r = rm_tree(&child);
                }
            }
            else
            {
                r = DeleteFileW(child.p) ? OS_OK : last();
            }
            w_free(&child);
            if (r != OS_OK && r != OS_NOT_FOUND)
            {
                k = r;
                break;
            }
        }
        if (!FindNextFileW(f, &fd))
        {
            DWORD e = GetLastError();
            if (e != ERROR_NO_MORE_FILES)
            {
                k = kind_of(e);
            }
            break;
        }
    }
    FindClose(f);
    w_free(&pat);
    if (k != OS_OK)
    {
        return k;
    }
    if (!RemoveDirectoryW(w->p))
    {
        DWORD e = GetLastError();
        if (e != ERROR_FILE_NOT_FOUND && e != ERROR_PATH_NOT_FOUND)
        {
            return kind_of(e);
        }
    }
    return OS_OK;
}

/* `fs::remove_dir_all`: the path opened without following a link; not a
   directory is ERROR_DIRECTORY; a link to one is removed itself. */
int os_remove_dir_all(const uint8_t *path, size_t n)
{
    VPATH(w, path, n);
    HANDLE h = CreateFileW(w.p, DELETE | FILE_LIST_DIRECTORY, FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE, NULL, OPEN_EXISTING, FILE_FLAG_BACKUP_SEMANTICS | FILE_FLAG_OPEN_REPARSE_POINT, NULL);
    if (h == INVALID_HANDLE_VALUE)
    {
        int k = last();
        w_free(&w);
        return k;
    }
    attr a;
    int k = handle_attr(h, &a);
    CloseHandle(h);
    if (k != OS_OK)
    {
        w_free(&w);
        return k;
    }
    if (!(a.attrs & FILE_ATTRIBUTE_DIRECTORY))
    {
        w_free(&w);
        return kind_of(ERROR_DIRECTORY);
    }
    if (attr_is_symlink(&a))
    {
        k = RemoveDirectoryW(w.p) ? OS_OK : last();
    }
    else
    {
        k = rm_tree(&w);
    }
    w_free(&w);
    return k;
}

int os_set_current_dir(const uint8_t *path, size_t n)
{
    wbuf w = {0};
    if (!upath(path, n, &w))
    {
        w_free(&w);
        return OS_INVALID;
    }
    BOOL ok = SetCurrentDirectoryW(w.p);
    DWORD e = GetLastError();
    w_free(&w);
    return ok ? OS_OK : kind_of(e);
}

/* ---- the environment ---- */

static DWORD cwd_fill(void *ctx, WCHAR *buf, DWORD n)
{
    (void)ctx;
    return GetCurrentDirectoryW(n, buf);
}

int os_current_dir(cb_buf *out)
{
    wbuf w = {0};
    if (!fill_utf16(cwd_fill, NULL, &w))
    {
        int k = last();
        w_free(&w);
        return k;
    }
    out->n = 0;
    from_wide(w.p, w.n, out);
    w_free(&w);
    return OS_OK;
}

static DWORD temp_fill(void *ctx, WCHAR *buf, DWORD n)
{
    (void)ctx;
    return GetTempPathW(n, buf);
}

void os_temp_dir(cb_buf *out)
{
    out->n = 0;
    wbuf w = {0};
    if (fill_utf16(temp_fill, NULL, &w))
    {
        from_wide_lossy(w.p, w.n, out);
    }
    w_free(&w);
}

static DWORD exe_fill(void *ctx, WCHAR *buf, DWORD n)
{
    (void)ctx;
    return GetModuleFileNameW(NULL, buf, n);
}

int os_current_exe(cb_buf *out)
{
    wbuf w = {0};
    if (!fill_utf16(exe_fill, NULL, &w))
    {
        int k = last();
        w_free(&w);
        return k;
    }
    out->n = 0;
    from_wide(w.p, w.n, out);
    w_free(&w);
    return OS_OK;
}

/* D-0181: `GetComputerNameExW` (the physical DNS host name). */
int os_hostname(cb_buf *out)
{
    WCHAR buf[256];
    DWORD size = 256;
    if (!GetComputerNameExW((COMPUTER_NAME_FORMAT)5, buf, &size))
    {
        return OS_OTHER;
    }
    out->n = 0;
    from_wide_lossy(buf, size, out);
    return OS_OK;
}

/* ---- child processes ---- */

struct os_child
{
    HANDLE h;
    DWORD pid;
    int has_status;
    int status;
};

/* Handles made inheritable for one child, and its creation, under one lock,
   so no other child started at the same time inherits them (Rust's
   `CREATE_PROCESS_LOCK`). */
static plat_mutex create_lock = PLAT_MUTEX_INIT;

/* `Ord for EnvKey`: names compared as `CompareStringOrdinal` compares
   them, ignoring case. */
static int env_key_cmp(const WCHAR *a, size_t an, const WCHAR *b, size_t bn)
{
    int r = CompareStringOrdinal(a, (int)an, b, (int)bn, TRUE);
    return r == CSTR_LESS_THAN ? -1 : r == CSTR_GREATER_THAN ? 1 : 0;
}

typedef struct
{
    wbuf k, v;
} wenv;

/* The child's environment map when the command changes it (`capture_env`):
   the parent's variables unless cleared, then the command's, each
   replacing the value of the same name (its first spelling kept). */
static wenv *capture_env(const os_cmd *c, size_t *count, int *bad)
{
    *bad = 0;
    *count = 0;
    if (!c->clear_env && c->nenv == 0)
    {
        return NULL;
    }
    size_t cap = 16, n = 0;
    wenv *v = (wenv *)cb_xcalloc(cap, sizeof(wenv));
#define ENV_SET(KP, KN, VP, VN)                                                   \
    do                                                                            \
    {                                                                             \
        size_t j_ = 0;                                                            \
        while (j_ < n && env_key_cmp(v[j_].k.p, v[j_].k.n, KP, KN) != 0)          \
        {                                                                         \
            j_++;                                                                 \
        }                                                                         \
        if (j_ == n)                                                              \
        {                                                                         \
            if (n == cap)                                                         \
            {                                                                     \
                cap *= 2;                                                         \
                v = (wenv *)cb_xrealloc(v, cap * sizeof(wenv));                   \
                memset(v + n, 0, (cap - n) * sizeof(wenv));                       \
            }                                                                     \
            w_puts(&v[n].k, KP, KN);                                              \
            n++;                                                                  \
        }                                                                         \
        v[j_].v.n = 0;                                                            \
        w_puts(&v[j_].v, VP, VN);                                                 \
    } while (0)
    if (!c->clear_env)
    {
        WCHAR *block = GetEnvironmentStringsW();
        if (block)
        {
            for (WCHAR *s = block; *s; s += wcslen(s) + 1)
            {
                size_t len = wcslen(s);
                if (len == 0)
                {
                    continue;
                }
                /* A name may begin with `=` (`=C:`). */
                const WCHAR *eq = wcschr(s + 1, L'=');
                if (!eq)
                {
                    continue;
                }
                size_t kn = (size_t)(eq - s);
                ENV_SET(s, kn, eq + 1, len - kn - 1);
            }
            FreeEnvironmentStringsW(block);
        }
    }
    for (size_t i = 0; i < c->nenv; i++)
    {
        wbuf k = {0}, val = {0};
        if (!to_wide(c->env_k[i], c->env_kn[i], &k) || !to_wide(c->env_v[i], c->env_vn[i], &val))
        {
            *bad = 1;
        }
        ENV_SET(k.p, k.n, val.p, val.n);
        w_free(&k);
        w_free(&val);
    }
#undef ENV_SET
    /* The map's order. */
    for (size_t i = 1; i < n; i++)
    {
        wenv x = v[i];
        size_t j = i;
        while (j > 0 && env_key_cmp(v[j - 1].k.p, v[j - 1].k.n, x.k.p, x.k.n) > 0)
        {
            v[j] = v[j - 1];
            j--;
        }
        v[j] = x;
    }
    *count = n;
    return v;
}

static void env_free(wenv *v, size_t n)
{
    for (size_t i = 0; i < n; i++)
    {
        w_free(&v[i].k);
        w_free(&v[i].v);
    }
    free(v);
}

/* `GetFileAttributesW` finds something there (`program_exists`, which
   does not follow a link); the path as `to_user_path` gives it. */
static int program_exists(const WCHAR *p, size_t n, wbuf *out)
{
    out->n = 0;
    w_puts(out, p, n);
    w_put(out, 0);
    if (!long_path(out, 0))
    {
        return 0;
    }
    return GetFileAttributesW(out->p) != INVALID_FILE_ATTRIBUTES;
}

/* `env::split_paths` on Windows: `;`-separated, a `"` toggling a quoted
   part (which may hold `;`), the quotes dropped; empty entries skipped. */
static int search_list(const WCHAR *paths, size_t n, const WCHAR *exe, size_t en, int has_ext, wbuf *found)
{
    size_t i = 0;
    while (i <= n)
    {
        wbuf dir = {0};
        int inq = 0;
        while (i < n && (paths[i] != L';' || inq))
        {
            if (paths[i] == L'"')
            {
                inq = !inq;
            }
            else
            {
                w_put(&dir, paths[i]);
            }
            i++;
        }
        i++;
        if (dir.n > 0)
        {
            /* `path.push(exe)`, then `set_extension("exe")` when the name
               has no `.` at all. */
            wbuf cand = {0};
            w_puts(&cand, dir.p, dir.n);
            int bare_drive = dir.n == 2 && dir.p[1] == L':';
            if (!is_sepw(dir.p[dir.n - 1]) && !bare_drive)
            {
                w_put(&cand, L'\\');
            }
            w_puts(&cand, exe, en);
            if (!has_ext)
            {
                w_puts(&cand, L".exe", 4);
            }
            int ok = program_exists(cand.p, cand.n, found);
            w_free(&cand);
            if (ok)
            {
                w_free(&dir);
                return 1;
            }
        }
        w_free(&dir);
    }
    return 0;
}

/* Rust's `resolve_exe`: a path is taken as it is (with `.exe` added when
   that exists); a bare name is searched for in the child's PATH, the
   program's own directory, the system directories, then the parent's PATH.
   The answer with its NUL. */
static int resolve_exe(const WCHAR *exe, size_t en, const WCHAR *child_path, size_t cpn, int has_child_path, wbuf *out)
{
    if (en == 0 || is_sepw(exe[en - 1]))
    {
        return OS_INVALID;
    }
    int has_exe_suffix = en >= 4 && exe[en - 4] == L'.' && (exe[en - 3] | 32) == L'e' && (exe[en - 2] | 32) == L'x' && (exe[en - 1] | 32) == L'e';
    int file_name = 1;
    for (size_t i = 0; i < en; i++)
    {
        if (is_sepw(exe[i]))
        {
            file_name = 0;
        }
    }
    if (!file_name)
    {
        if (has_exe_suffix)
        {
            out->n = 0;
            w_puts(out, exe, en);
            w_put(out, 0);
            return long_path(out, 0) ? OS_OK : last();
        }
        wbuf cand = {0};
        w_puts(&cand, exe, en);
        w_puts(&cand, L".exe", 4);
        int ok = program_exists(cand.p, cand.n, out);
        w_free(&cand);
        if (ok)
        {
            return OS_OK;
        }
        out->n = 0;
        w_puts(out, exe, en);
        w_put(out, 0);
        return long_path(out, 0) ? OS_OK : last();
    }
    int has_ext = 0;
    for (size_t i = 0; i < en; i++)
    {
        if (exe[i] == L'.')
        {
            has_ext = 1;
        }
    }
    if (has_child_path && search_list(child_path, cpn, exe, en, has_ext, out))
    {
        return OS_OK;
    }
    /* The program's own directory. */
    wbuf me = {0};
    if (fill_utf16(exe_fill, NULL, &me))
    {
        size_t k = me.n;
        while (k > 0 && !is_sepw(me.p[k - 1]))
        {
            k--;
        }
        if (k > 0)
        {
            /* `pop`: the parent, without its trailing separator (a root
               keeps it). */
            size_t d = k - 1;
            if (d == 2 && me.p[1] == L':')
            {
                d = 3;
            }
            if (search_list(me.p, d, exe, en, has_ext, out))
            {
                w_free(&me);
                return OS_OK;
            }
        }
    }
    w_free(&me);
    WCHAR sys[MAX_PATH + 1];
    UINT sn = GetSystemDirectoryW(sys, MAX_PATH);
    if (sn > 0 && sn < MAX_PATH && search_list(sys, sn, exe, en, has_ext, out))
    {
        return OS_OK;
    }
    sn = GetWindowsDirectoryW(sys, MAX_PATH);
    if (sn > 0 && sn < MAX_PATH && search_list(sys, sn, exe, en, has_ext, out))
    {
        return OS_OK;
    }
    wbuf pp = {0};
    if (fill_utf16(env_fill, (void *)L"PATH", &pp))
    {
        if (search_list(pp.p, pp.n, exe, en, has_ext, out))
        {
            w_free(&pp);
            return OS_OK;
        }
    }
    w_free(&pp);
    /* "program not found" */
    return OS_NOT_FOUND;
}

/* `make_command_line`'s quoting of one argument (`append_arg`, quoted
   when it holds a space or a tab or is empty). */
static void append_arg(wbuf *cmd, const WCHAR *a, size_t n)
{
    int quote = n == 0;
    for (size_t i = 0; i < n; i++)
    {
        if (a[i] == L' ' || a[i] == L'\t')
        {
            quote = 1;
        }
    }
    if (quote)
    {
        w_put(cmd, L'"');
    }
    size_t bs = 0;
    for (size_t i = 0; i < n; i++)
    {
        WCHAR x = a[i];
        if (x == L'\\')
        {
            bs++;
        }
        else
        {
            if (x == L'"')
            {
                /* n+1 backslashes, 2n+1 in all, before an inner quote. */
                for (size_t k = 0; k <= bs; k++)
                {
                    w_put(cmd, L'\\');
                }
            }
            bs = 0;
        }
        w_put(cmd, x);
    }
    if (quote)
    {
        for (size_t k = 0; k < bs; k++)
        {
            w_put(cmd, L'\\');
        }
        w_put(cmd, L'"');
    }
}

/* `make_bat_command_line` (Rust 1.77.2's): `cmd.exe /e:ON /v:OFF /d /c`
   and the script and arguments quoted as cmd.exe reads them; `%` escaped
   as `%%cd:~,%`; a CR or LF refused. 0 when refused. */
static int bat_command_line(const WCHAR *script, size_t sn, wbuf *args, size_t nargs, wbuf *cmd)
{
    w_puts(cmd, L"cmd.exe /e:ON /v:OFF /d /c \"", 28);
    w_put(cmd, L'"');
    for (size_t i = 0; i < sn; i++)
    {
        if (script[i] == L'"')
        {
            return 0;
        }
    }
    if (sn && script[sn - 1] == L'\\')
    {
        return 0;
    }
    w_puts(cmd, script, sn);
    w_put(cmd, L'"');
    static const char special[] = "\t &()[]{}^=;!'+,`~%|<>";
    for (size_t j = 0; j < nargs; j++)
    {
        const WCHAR *a = args[j].p;
        size_t n = args[j].n;
        w_put(cmd, L' ');
        int quote = n == 0;
        for (size_t i = 0; i < n; i++)
        {
            if (a[i] == L'\r' || a[i] == L'\n')
            {
                return 0;
            }
            if (a[i] < 128 && strchr(special, (char)a[i]))
            {
                quote = 1;
            }
        }
        if (quote)
        {
            w_put(cmd, L'"');
        }
        size_t bs = 0;
        for (size_t i = 0; i < n; i++)
        {
            WCHAR x = a[i];
            if (x == L'\\')
            {
                bs++;
            }
            else
            {
                if (x == L'"')
                {
                    for (size_t k = 0; k < bs; k++)
                    {
                        w_put(cmd, L'\\');
                    }
                }
                bs = 0;
            }
            if (x == L'%')
            {
                w_puts(cmd, L"%%cd:~,", 7);
            }
            else
            {
                w_put(cmd, x);
            }
        }
        if (quote)
        {
            for (size_t k = 0; k < bs; k++)
            {
                w_put(cmd, L'\\');
            }
            w_put(cmd, L'"');
        }
    }
    w_put(cmd, L'"');
    return 1;
}

/* The handle a child's stream gets: the parent's own (duplicated,
   inheritable; NULL when there is none), `NUL`, or a pipe's far end. */
static int child_stdio(int which, int mode, HANDLE *child_end, HANDLE *our_end)
{
    *child_end = NULL;
    *our_end = INVALID_HANDLE_VALUE;
    SECURITY_ATTRIBUTES sa = {sizeof sa, NULL, TRUE};
    if (mode == OS_STDIO_INHERIT)
    {
        DWORD id = which == 0 ? STD_INPUT_HANDLE : which == 1 ? STD_OUTPUT_HANDLE : STD_ERROR_HANDLE;
        HANDLE h = GetStdHandle(id);
        if (h == INVALID_HANDLE_VALUE)
        {
            return last();
        }
        if (h == NULL)
        {
            return OS_OK;
        }
        if (!DuplicateHandle(GetCurrentProcess(), h, GetCurrentProcess(), child_end, 0, TRUE, DUPLICATE_SAME_ACCESS))
        {
            return last();
        }
        return OS_OK;
    }
    if (mode == OS_STDIO_NULL)
    {
        HANDLE h = CreateFileW(L"NUL", which == 0 ? GENERIC_READ : GENERIC_WRITE, FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE, &sa, OPEN_EXISTING, 0, NULL);
        if (h == INVALID_HANDLE_VALUE)
        {
            return last();
        }
        *child_end = h;
        return OS_OK;
    }
    HANDLE r, w;
    if (!CreatePipe(&r, &w, &sa, 0))
    {
        return last();
    }
    HANDLE ours = which == 0 ? w : r;
    HANDLE theirs = which == 0 ? r : w;
    SetHandleInformation(ours, HANDLE_FLAG_INHERIT, 0);
    *child_end = theirs;
    *our_end = ours;
    return OS_OK;
}

int os_spawn(const os_cmd *c, int in_mode, int out_mode, int err_mode, os_child **child, os_pipe *in, os_pipe *out, os_pipe *err)
{
    *in = *out = *err = OS_PIPE_NONE;
    if (c->argc == 0)
    {
        return OS_INVALID;
    }
    int k = OS_OK;
    wbuf *argw = (wbuf *)cb_xcalloc(c->argc, sizeof(wbuf));
    for (size_t i = 0; i < c->argc; i++)
    {
        if (!to_wide(c->argv[i], c->argl[i], &argw[i]))
        {
            k = OS_INVALID;
        }
    }
    size_t nenv = 0;
    int bad = 0;
    wenv *env = capture_env(c, &nenv, &bad);
    if (bad)
    {
        k = OS_INVALID;
    }
    wbuf dir = {0};
    if (c->has_dir)
    {
        if (!to_wide(c->dir, c->dirn, &dir))
        {
            k = OS_INVALID;
        }
        w_put(&dir, 0);
    }
    wbuf prog = {0}, cmd = {0}, block = {0};
    if (k == OS_OK)
    {
        const WCHAR *cp = NULL;
        size_t cpn = 0;
        int has_cp = 0;
        for (size_t i = 0; env && i < nenv; i++)
        {
            if (env_key_cmp(env[i].k.p, env[i].k.n, L"PATH", 4) == 0)
            {
                cp = env[i].v.p;
                cpn = env[i].v.n;
                has_cp = 1;
            }
        }
        k = resolve_exe(argw[0].p, argw[0].n, cp, cpn, has_cp, &prog);
    }
    if (k == OS_OK)
    {
        size_t pn = prog.n - 1;
        int is_bat = pn >= 4 && prog.p[pn - 4] == L'.' && (((prog.p[pn - 3] | 32) == L'b' && (prog.p[pn - 2] | 32) == L'a' && (prog.p[pn - 1] | 32) == L't') || ((prog.p[pn - 3] | 32) == L'c' && (prog.p[pn - 2] | 32) == L'm' && (prog.p[pn - 1] | 32) == L'd'));
        if (is_bat)
        {
            if (!bat_command_line(prog.p, pn, argw + 1, c->argc - 1, &cmd))
            {
                k = OS_INVALID;
            }
            else
            {
                /* The program is the command prompt. */
                WCHAR sys[MAX_PATH + 1];
                UINT sn = GetSystemDirectoryW(sys, MAX_PATH);
                prog.n = 0;
                w_puts(&prog, sys, sn);
                w_puts(&prog, L"\\cmd.exe", 8);
                w_put(&prog, 0);
            }
        }
        else
        {
            /* The program's name always quoted (it cannot hold a quote). */
            w_put(&cmd, L'"');
            w_puts(&cmd, argw[0].p, argw[0].n);
            w_put(&cmd, L'"');
            for (size_t i = 1; i < c->argc; i++)
            {
                w_put(&cmd, L' ');
                append_arg(&cmd, argw[i].p, argw[i].n);
            }
        }
        w_put(&cmd, 0);
        if (env)
        {
            if (nenv == 0)
            {
                w_put(&block, 0);
            }
            for (size_t i = 0; i < nenv; i++)
            {
                w_puts(&block, env[i].k.p, env[i].k.n);
                w_put(&block, L'=');
                w_puts(&block, env[i].v.p, env[i].v.n);
                w_put(&block, 0);
            }
            w_put(&block, 0);
        }
    }
    HANDLE ce[3] = {NULL, NULL, NULL}, oe[3] = {INVALID_HANDLE_VALUE, INVALID_HANDLE_VALUE, INVALID_HANDLE_VALUE};
    int modes[3] = {in_mode, out_mode, err_mode};
    PROCESS_INFORMATION pi;
    memset(&pi, 0, sizeof pi);
    if (k == OS_OK)
    {
        plat_mutex_lock(&create_lock);
        for (int i = 0; i < 3 && k == OS_OK; i++)
        {
            k = child_stdio(i, modes[i], &ce[i], &oe[i]);
        }
        if (k == OS_OK)
        {
            STARTUPINFOW si;
            memset(&si, 0, sizeof si);
            si.cb = sizeof si;
            si.dwFlags = STARTF_USESTDHANDLES;
            si.hStdInput = ce[0];
            si.hStdOutput = ce[1];
            si.hStdError = ce[2];
            if (!CreateProcessW(prog.p, cmd.p, NULL, NULL, TRUE, CREATE_UNICODE_ENVIRONMENT, env ? block.p : NULL, c->has_dir ? dir.p : NULL, &si, &pi))
            {
                k = last();
            }
        }
        for (int i = 0; i < 3; i++)
        {
            if (ce[i] && ce[i] != INVALID_HANDLE_VALUE)
            {
                CloseHandle(ce[i]);
            }
        }
        plat_mutex_unlock(&create_lock);
    }
    for (size_t i = 0; i < c->argc; i++)
    {
        w_free(&argw[i]);
    }
    free(argw);
    if (env)
    {
        env_free(env, nenv);
    }
    w_free(&dir);
    w_free(&prog);
    w_free(&cmd);
    w_free(&block);
    if (k != OS_OK)
    {
        for (int i = 0; i < 3; i++)
        {
            if (oe[i] != INVALID_HANDLE_VALUE)
            {
                CloseHandle(oe[i]);
            }
        }
        return k;
    }
    CloseHandle(pi.hThread);
    os_pipe *ends[3] = {in, out, err};
    for (int i = 0; i < 3; i++)
    {
        if (oe[i] != INVALID_HANDLE_VALUE)
        {
            *ends[i] = (os_pipe)oe[i];
        }
    }
    os_child *ch = (os_child *)cb_xcalloc(1, sizeof(os_child));
    ch->h = pi.hProcess;
    ch->pid = pi.dwProcessId;
    *child = ch;
    return OS_OK;
}

int os_child_wait(os_child *c, int *status)
{
    if (c->has_status)
    {
        *status = c->status;
        return OS_OK;
    }
    if (WaitForSingleObject(c->h, INFINITE) != WAIT_OBJECT_0)
    {
        return last();
    }
    DWORD code;
    if (!GetExitCodeProcess(c->h, &code))
    {
        return last();
    }
    c->has_status = 1;
    c->status = (int)code;
    *status = c->status;
    return OS_OK;
}

int os_child_try_wait(os_child *c, int *done, int *status)
{
    if (c->has_status)
    {
        *done = 1;
        *status = c->status;
        return OS_OK;
    }
    DWORD r = WaitForSingleObject(c->h, 0);
    if (r == WAIT_TIMEOUT)
    {
        *done = 0;
        return OS_OK;
    }
    if (r != WAIT_OBJECT_0)
    {
        return last();
    }
    DWORD code;
    if (!GetExitCodeProcess(c->h, &code))
    {
        return last();
    }
    c->has_status = 1;
    c->status = (int)code;
    *done = 1;
    *status = c->status;
    return OS_OK;
}

/* `TerminateProcess` answers ERROR_ACCESS_DENIED for a process already
   ended: no failure then. */
int os_child_kill(os_child *c)
{
    if (!TerminateProcess(c->h, 1))
    {
        DWORD e = GetLastError();
        int done = 0, st;
        if (e != ERROR_ACCESS_DENIED || os_child_try_wait(c, &done, &st) != OS_OK)
        {
            return kind_of(e);
        }
    }
    return OS_OK;
}

int os_child_terminate(os_child *c)
{
    return os_child_kill(c);
}

/* D-0205: `taskkill /T /F /PID n`, which ends the tree; the child alone
   when that fails. */
int os_child_kill_tree(os_child *c)
{
    char pid[16];
    snprintf(pid, sizeof pid, "%lu", (unsigned long)c->pid);
    const uint8_t *argv[5] = {(const uint8_t *)"taskkill", (const uint8_t *)"/T", (const uint8_t *)"/F", (const uint8_t *)"/PID", (const uint8_t *)pid};
    size_t argl[5] = {8, 2, 2, 4, strlen(pid)};
    os_cmd cmd;
    memset(&cmd, 0, sizeof cmd);
    cmd.argc = 5;
    cmd.argv = argv;
    cmd.argl = argl;
    os_child *t;
    os_pipe a, b, d;
    if (os_spawn(&cmd, OS_STDIO_INHERIT, OS_STDIO_NULL, OS_STDIO_NULL, &t, &a, &b, &d) == OS_OK)
    {
        int st = -1;
        int k = os_child_wait(t, &st);
        os_child_free(t);
        if (k == OS_OK && st == 0)
        {
            return OS_OK;
        }
    }
    return os_child_kill(c);
}

uint32_t os_child_id(os_child *c)
{
    return (uint32_t)c->pid;
}

void os_child_free(os_child *c)
{
    CloseHandle(c->h);
    free(c);
}

int os_pipe_read(os_pipe p, uint8_t *buf, size_t n, size_t *got)
{
    return handle_read((HANDLE)p, buf, n, got);
}

int os_pipe_write(os_pipe p, const uint8_t *buf, size_t n, size_t *put)
{
    return handle_write((HANDLE)p, buf, n, put);
}

void os_pipe_close(os_pipe p)
{
    if (p != OS_PIPE_NONE)
    {
        CloseHandle((HANDLE)p);
    }
}

typedef struct
{
    HANDLE h;
    cb_buf *v;
    int k;
} drain_job;

static DWORD WINAPI drain_main(LPVOID p)
{
    drain_job *j = (drain_job *)p;
    for (;;)
    {
        cb_buf_reserve(j->v, 8192);
        size_t got = 0;
        j->k = handle_read(j->h, j->v->p + j->v->n, j->v->cap - j->v->n, &got);
        if (j->k != OS_OK || got == 0)
        {
            return 0;
        }
        j->v->n += got;
    }
}

/* `read2`: the second pipe drained by a thread while this one reads the
   first, each to its end (Rust reads both with overlapped I/O). */
int os_read2(os_pipe a, cb_buf *va, os_pipe b, cb_buf *vb)
{
    drain_job ja = {(HANDLE)a, va, OS_OK}, jb = {(HANDLE)b, vb, OS_OK};
    HANDLE t = NULL;
    if (b != OS_PIPE_NONE)
    {
        t = CreateThread(NULL, 0, drain_main, &jb, 0, NULL);
        if (!t)
        {
            return last();
        }
    }
    if (a != OS_PIPE_NONE)
    {
        drain_main(&ja);
    }
    if (t)
    {
        WaitForSingleObject(t, INFINITE);
        CloseHandle(t);
    }
    return ja.k != OS_OK ? ja.k : jb.k;
}

uint32_t os_process_id(void)
{
    return (uint32_t)GetCurrentProcessId();
}

static int *interrupt_flag;

static BOOL WINAPI on_ctrl(DWORD kind)
{
    (void)kind;
    if (interrupt_flag)
    {
        __atomic_store_n(interrupt_flag, 1, __ATOMIC_SEQ_CST);
    }
    return TRUE;
}

void os_watch_interrupts(int *flag)
{
    interrupt_flag = flag;
    SetConsoleCtrlHandler(on_ctrl, TRUE);
}

void os_ignore_sigpipe(void)
{
}

/* ---- sockets ---- */

static INIT_ONCE wsa_once = INIT_ONCE_STATIC_INIT;

static BOOL CALLBACK wsa_start(PINIT_ONCE once, PVOID param, PVOID *ctx)
{
    (void)once;
    (void)param;
    (void)ctx;
    WSADATA d;
    WSAStartup(0x202, &d);
    return TRUE;
}

/* `net::init`: Winsock started once, before the first socket. */
static void net_init(void)
{
    InitOnceExecuteOnce(&wsa_once, wsa_start, NULL, NULL);
}

static SOCKET sk(os_sock s)
{
    return (SOCKET)s;
}

static int to_sockaddr(const os_addr *a, struct sockaddr_storage *ss)
{
    memset(ss, 0, sizeof *ss);
    if (a->family == 4)
    {
        struct sockaddr_in *s = (struct sockaddr_in *)ss;
        s->sin_family = AF_INET;
        s->sin_port = htons(a->port);
        memcpy(&s->sin_addr, a->ip, 4);
        return (int)sizeof *s;
    }
    struct sockaddr_in6 *s = (struct sockaddr_in6 *)ss;
    s->sin6_family = AF_INET6;
    s->sin6_port = htons(a->port);
    memcpy(&s->sin6_addr, a->ip, 16);
    return (int)sizeof *s;
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

#ifndef WSA_FLAG_NO_HANDLE_INHERIT
#define WSA_FLAG_NO_HANDLE_INHERIT 0x80
#endif

/* Rust's `Socket::new`: overlapped, not inherited (older systems refuse
   the flag: then made without it, and its handle made not inherited). */
static int new_socket(int af, int type, int proto, WSAPROTOCOL_INFOW *info, os_sock *out)
{
    net_init();
    SOCKET s = WSASocketW(af, type, proto, info, 0, WSA_FLAG_OVERLAPPED | WSA_FLAG_NO_HANDLE_INHERIT);
    if (s == INVALID_SOCKET)
    {
        int e = WSAGetLastError();
        if (e != WSAEPROTOTYPE && e != WSAEINVAL)
        {
            return kind_of((DWORD)e);
        }
        s = WSASocketW(af, type, proto, info, 0, WSA_FLAG_OVERLAPPED);
        if (s == INVALID_SOCKET)
        {
            return wsa_last();
        }
        if (!SetHandleInformation((HANDLE)s, HANDLE_FLAG_INHERIT, 0))
        {
            int k = last();
            closesocket(s);
            return k;
        }
    }
    *out = (os_sock)s;
    return OS_OK;
}

int os_tcp_bind(const os_addr *a, os_sock *out)
{
    os_sock s;
    int k = new_socket(a->family == 4 ? AF_INET : AF_INET6, SOCK_STREAM, 0, NULL, &s);
    if (k != OS_OK)
    {
        return k;
    }
    struct sockaddr_storage ss;
    int len = to_sockaddr(a, &ss);
    if (bind(sk(s), (struct sockaddr *)&ss, len) != 0 || listen(sk(s), 128) != 0)
    {
        k = wsa_last();
        closesocket(sk(s));
        return k;
    }
    *out = s;
    return OS_OK;
}

int os_udp_bind(const os_addr *a, os_sock *out)
{
    os_sock s;
    int k = new_socket(a->family == 4 ? AF_INET : AF_INET6, SOCK_DGRAM, 0, NULL, &s);
    if (k != OS_OK)
    {
        return k;
    }
    struct sockaddr_storage ss;
    int len = to_sockaddr(a, &ss);
    if (bind(sk(s), (struct sockaddr *)&ss, len) != 0)
    {
        k = wsa_last();
        closesocket(sk(s));
        return k;
    }
    *out = s;
    return OS_OK;
}

int os_sock_local(os_sock s, os_addr *a)
{
    struct sockaddr_storage ss;
    int len = sizeof ss;
    memset(&ss, 0, sizeof ss);
    if (getsockname(sk(s), (struct sockaddr *)&ss, &len) != 0)
    {
        return wsa_last();
    }
    return from_sockaddr(&ss, a);
}

int os_sock_peer(os_sock s, os_addr *a)
{
    struct sockaddr_storage ss;
    int len = sizeof ss;
    memset(&ss, 0, sizeof ss);
    if (getpeername(sk(s), (struct sockaddr *)&ss, &len) != 0)
    {
        return wsa_last();
    }
    return from_sockaddr(&ss, a);
}

int os_sock_nonblocking(os_sock s, int on)
{
    u_long v = on ? 1 : 0;
    return ioctlsocket(sk(s), FIONBIO, &v) == 0 ? OS_OK : wsa_last();
}

int os_accept(os_sock l, os_sock *out)
{
    struct sockaddr_storage ss;
    int len = sizeof ss;
    SOCKET s = accept(sk(l), (struct sockaddr *)&ss, &len);
    if (s == INVALID_SOCKET)
    {
        return wsa_last();
    }
    *out = (os_sock)s;
    return OS_OK;
}

static int take_error(SOCKET s)
{
    int e = 0;
    int len = sizeof e;
    if (getsockopt(s, SOL_SOCKET, SO_ERROR, (char *)&e, &len) != 0)
    {
        return wsa_last();
    }
    return e ? kind_of((DWORD)e) : OS_OK;
}

int os_connect(const os_addr *a, uint64_t ms, os_sock *out)
{
    os_sock s;
    int k = new_socket(a->family == 4 ? AF_INET : AF_INET6, SOCK_STREAM, 0, NULL, &s);
    if (k != OS_OK)
    {
        return k;
    }
    struct sockaddr_storage ss;
    int len = to_sockaddr(a, &ss);
    if (ms == 0)
    {
        if (connect(sk(s), (struct sockaddr *)&ss, len) != 0)
        {
            k = wsa_last();
            closesocket(sk(s));
            return k;
        }
        *out = s;
        return OS_OK;
    }
    /* `connect_timeout`: a non-blocking connect, then `select` for it to be
       writable (connected) or in error. */
    u_long one = 1, zero = 0;
    if (ioctlsocket(sk(s), FIONBIO, &one) != 0)
    {
        k = wsa_last();
        closesocket(sk(s));
        return k;
    }
    int r = connect(sk(s), (struct sockaddr *)&ss, len);
    int e = r != 0 ? WSAGetLastError() : 0;
    if (ioctlsocket(sk(s), FIONBIO, &zero) != 0)
    {
        k = wsa_last();
        closesocket(sk(s));
        return k;
    }
    if (r == 0)
    {
        *out = s;
        return OS_OK;
    }
    if (e != WSAEWOULDBLOCK)
    {
        closesocket(sk(s));
        return kind_of((DWORD)e);
    }
    struct timeval tv;
    tv.tv_sec = (long)(ms / 1000 > 0x7FFFFFFF ? 0x7FFFFFFF : ms / 1000);
    tv.tv_usec = (long)(ms % 1000 * 1000);
    fd_set wr, er;
    FD_ZERO(&wr);
    FD_ZERO(&er);
    FD_SET(sk(s), &wr);
    FD_SET(sk(s), &er);
    int n = select(1, NULL, &wr, &er, &tv);
    if (n == SOCKET_ERROR)
    {
        k = wsa_last();
        closesocket(sk(s));
        return k;
    }
    if (n == 0)
    {
        closesocket(sk(s));
        return OS_TIMED_OUT;
    }
    if (wr.fd_count != 1)
    {
        k = take_error(sk(s));
        if (k != OS_OK)
        {
            closesocket(sk(s));
            return k;
        }
    }
    *out = s;
    return OS_OK;
}

int os_set_timeout(os_sock s, int write, uint64_t ms)
{
    DWORD t = ms > 0xFFFFFFFEu ? INFINITE : (DWORD)ms;
    if (setsockopt(sk(s), SOL_SOCKET, write ? SO_SNDTIMEO : SO_RCVTIMEO, (const char *)&t, sizeof t) != 0)
    {
        return wsa_last();
    }
    return OS_OK;
}

int os_get_timeout(os_sock s, int write, uint64_t *ns)
{
    DWORD t = 0;
    int len = sizeof t;
    if (getsockopt(sk(s), SOL_SOCKET, write ? SO_SNDTIMEO : SO_RCVTIMEO, (char *)&t, &len) != 0)
    {
        return wsa_last();
    }
    *ns = (uint64_t)t * 1000000u;
    return OS_OK;
}

int os_set_nodelay(os_sock s, int on)
{
    /* A one-byte BOOLEAN, as Rust passes it. */
    char v = on ? 1 : 0;
    return setsockopt(sk(s), IPPROTO_TCP, TCP_NODELAY, &v, 1) == 0 ? OS_OK : wsa_last();
}

int os_set_keepalive(os_sock s, int on)
{
    int v = on ? 1 : 0;
    return setsockopt(sk(s), SOL_SOCKET, SO_KEEPALIVE, (const char *)&v, 4) == 0 ? OS_OK : OS_OTHER;
}

/* `recv`: a socket shut down reads as the end (WSAESHUTDOWN). */
int os_recv(os_sock s, uint8_t *p, size_t n, size_t *got)
{
    int len = n > 0x7FFFFFFF ? 0x7FFFFFFF : (int)n;
    int k = recv(sk(s), (char *)p, len, 0);
    if (k == SOCKET_ERROR)
    {
        int e = WSAGetLastError();
        *got = 0;
        return e == WSAESHUTDOWN ? OS_OK : kind_of((DWORD)e);
    }
    *got = (size_t)k;
    return OS_OK;
}

int os_send(os_sock s, const uint8_t *p, size_t n, size_t *put)
{
    int len = n > 0x7FFFFFFF ? 0x7FFFFFFF : (int)n;
    int k = send(sk(s), (const char *)p, len, 0);
    if (k == SOCKET_ERROR)
    {
        *put = 0;
        return wsa_last();
    }
    *put = (size_t)k;
    return OS_OK;
}

int os_shutdown_write(os_sock s)
{
    return shutdown(sk(s), SD_SEND) == 0 ? OS_OK : wsa_last();
}

void os_sock_close(os_sock s)
{
    closesocket(sk(s));
}

int os_sendto(os_sock s, const uint8_t *p, size_t n, const os_addr *a, size_t *put)
{
    struct sockaddr_storage ss;
    int len = to_sockaddr(a, &ss);
    int pl = n > 0x7FFFFFFF ? 0x7FFFFFFF : (int)n;
    int k = sendto(sk(s), (const char *)p, pl, 0, (struct sockaddr *)&ss, len);
    if (k == SOCKET_ERROR)
    {
        *put = 0;
        return wsa_last();
    }
    *put = (size_t)k;
    return OS_OK;
}

int os_recvfrom(os_sock s, uint8_t *p, size_t n, size_t *got, os_addr *from)
{
    struct sockaddr_storage ss;
    int len = sizeof ss;
    memset(&ss, 0, sizeof ss);
    int pl = n > 0x7FFFFFFF ? 0x7FFFFFFF : (int)n;
    int k = recvfrom(sk(s), (char *)p, pl, 0, (struct sockaddr *)&ss, &len);
    if (k == SOCKET_ERROR)
    {
        int e = WSAGetLastError();
        *got = 0;
        if (e != WSAESHUTDOWN)
        {
            return kind_of((DWORD)e);
        }
        k = 0;
    }
    *got = (size_t)k;
    return from_sockaddr(&ss, from);
}

int os_udp_connect(os_sock s, const os_addr *a)
{
    struct sockaddr_storage ss;
    int len = to_sockaddr(a, &ss);
    return connect(sk(s), (struct sockaddr *)&ss, len) == 0 ? OS_OK : wsa_last();
}

int os_sock_dup(os_sock s, os_sock *out)
{
    WSAPROTOCOL_INFOW info;
    if (WSADuplicateSocketW(sk(s), GetCurrentProcessId(), &info) != 0)
    {
        return wsa_last();
    }
    return new_socket(info.iAddressFamily, info.iSocketType, info.iProtocol, &info, out);
}

int os_resolve(const char *host, uint16_t port, os_addr **out, size_t *count)
{
    net_init();
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

int os_ready(os_sock s, int read, int ms, int *ready)
{
    WSAPOLLFD p;
    p.fd = sk(s);
    p.events = read ? POLLRDNORM : POLLWRNORM;
    p.revents = 0;
    int n = WSAPoll(&p, 1, ms);
    if (n < 0)
    {
        return wsa_last();
    }
    *ready = n > 0;
    return OS_OK;
}

/* No wake on Windows: a socket wait asks between slices (netio.c). */
void os_wake_init(void)
{
}

int os_wake_enabled(void)
{
    return 0;
}

uint64_t os_wake_thread_self(void)
{
    return 0;
}

void os_wake_thread(uint64_t t)
{
    (void)t;
}

/* ---- random bytes ---- */

int os_random(uint8_t *p, size_t n)
{
    while (n > 0)
    {
        ULONG chunk = n > 0xFFFFFFFFu ? 0xFFFFFFFFu : (ULONG)n;
        if (BCryptGenRandom(NULL, p, chunk, BCRYPT_USE_SYSTEM_PREFERRED_RNG) != 0)
        {
            return 0;
        }
        p += chunk;
        n -= chunk;
    }
    return 1;
}

#endif
