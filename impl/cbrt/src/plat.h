/* What the runtime needs of the operating system, in one small interface
   (D-0326 phase B: the runtime in C): locks and condition variables,
   threads, the clocks, the stack's bounds, the program's arguments and the
   OS's random bytes. One file implements it for POSIX (plat_posix.c, glibc
   on Linux) and one for Windows 7 and later (plat_win.c, MinGW-w64); the
   rest of the runtime is portable C over this header and never names an OS
   header itself. The I/O modules (fileio, procio, netio) keep their own OS
   code apart, in io_posix.c and io_win.c, behind io_plat.h.

   Every lock here is a plain (non-reentrant) mutual exclusion; the
   runtime's reentrant hold is built over them (lib.c). Statically
   initialized locks and condition variables (PLAT_MUTEX_INIT,
   PLAT_COND_INIT) need no call before first use, as Rust's
   `Mutex::new` in a `static` needs none. */

#ifndef CBRT_PLAT_H
#define CBRT_PLAT_H

/* Windows 7's API (condition variables, SRW locks, fiber-local storage):
   set before any system header, which otherwise fixes an older default. */
#ifdef _WIN32
#undef _WIN32_WINNT
#define _WIN32_WINNT 0x0601
#endif

#include <stddef.h>
#include <stdint.h>

#ifdef _WIN32
#ifndef WIN32_LEAN_AND_MEAN
#define WIN32_LEAN_AND_MEAN
#endif
#include <windows.h>
typedef struct
{
    SRWLOCK l;
} plat_mutex;
typedef struct
{
    CONDITION_VARIABLE c;
} plat_cond;
typedef struct
{
    SRWLOCK l;
} plat_rwlock;
#define PLAT_MUTEX_INIT {SRWLOCK_INIT}
#define PLAT_COND_INIT {CONDITION_VARIABLE_INIT}
#define PLAT_RWLOCK_INIT {SRWLOCK_INIT}
typedef HANDLE plat_thread;
#else
#include <pthread.h>
typedef struct
{
    pthread_mutex_t m;
} plat_mutex;
typedef struct
{
    pthread_cond_t c;
} plat_cond;
typedef struct
{
    pthread_rwlock_t l;
} plat_rwlock;
#define PLAT_MUTEX_INIT {PTHREAD_MUTEX_INITIALIZER}
#define PLAT_COND_INIT {PTHREAD_COND_INITIALIZER}
#define PLAT_RWLOCK_INIT {PTHREAD_RWLOCK_INITIALIZER}
typedef pthread_t plat_thread;
#endif

/* ---- locks ---- */

void plat_mutex_init(plat_mutex *m);
void plat_mutex_lock(plat_mutex *m);
int plat_mutex_trylock(plat_mutex *m); /* 1 when taken */
void plat_mutex_unlock(plat_mutex *m);

void plat_cond_init(plat_cond *c);
void plat_cond_wait(plat_cond *c, plat_mutex *m);
/* Waits at most `ns` nanoseconds; 1 when woken (or spuriously), 0 on the
   timeout. */
int plat_cond_wait_ns(plat_cond *c, plat_mutex *m, uint64_t ns);
void plat_cond_signal(plat_cond *c);
void plat_cond_broadcast(plat_cond *c);

void plat_rw_read(plat_rwlock *l);
int plat_rw_try_read(plat_rwlock *l);
void plat_rw_read_unlock(plat_rwlock *l);
void plat_rw_write(plat_rwlock *l);
int plat_rw_try_write(plat_rwlock *l);
void plat_rw_write_unlock(plat_rwlock *l);

/* ---- threads ---- */

/* A new OS thread running `fn(arg)` with a stack of at least `stack` bytes
   (0: the platform's default); 0 on success. */
int plat_thread_spawn(plat_thread *t, void (*fn)(void *), void *arg, size_t stack);
void plat_thread_join(plat_thread t);
void plat_thread_detach(plat_thread t);
void plat_thread_yield(void);
/* A number naming the calling OS thread, unique among running threads. */
uint64_t plat_thread_self(void);
/* The machine's processors (`std::thread::available_parallelism`), 1 when
   unknown. */
uint64_t plat_cpu_count(void);
/* A CPU spin-wait hint (`std::hint::spin_loop`). */
void plat_spin_hint(void);

/* ---- clocks ---- */

/* A monotonic clock in nanoseconds (`std::time::Instant`). */
uint64_t plat_now_ns(void);
void plat_sleep_ns(uint64_t ns);

/* ---- the process ---- */

void plat_exit(int status) __attribute__((noreturn));
void plat_abort(void) __attribute__((noreturn));
/* An environment variable's value, or NULL when unset (`env::var_os`). The
   text stays valid until the variable is changed; the runtime only reads. */
const char *plat_getenv(const char *name);

/* ---- arguments and terminals ---- */

/* The program's arguments without its own name (`cb_set_args`,
   `rule.stdlib.args`): `*n` of them, argument i the `(*lens)[i]` bytes at
   `(*args)[i]`, made once and never freed. POSIX: argv's bytes. D-0255:
   on Windows the command line itself (UTF-16) split as Rust's `args_os`
   splits it, as WTF-8 (an unpaired surrogate kept, so `arg` gives `Err`
   for it), and the C library's standard streams put in binary mode, so C
   code a program links (`puts`) writes `\n`, not `\r\n`. */
void plat_args(int argc, const char *const *argv, uint8_t ***args, size_t **lens, size_t *n);
/* `std::io::stderr().is_terminal()` / stdout's. */
int plat_stderr_is_tty(void);
int plat_stdout_is_tty(void);
/* diagnostics' `enable_ansi`: on Windows the console asked to process ANSI
   sequences (1 when it does); 1 elsewhere. */
int plat_enable_ansi(void);

/* ---- the stack ---- */

/* The calling thread's stack (D-0077): its lowest address and its size, 1
   when known, 0 when not. POSIX: the thread's attributes
   (`pthread_getattr_np`). Windows: the reservation `VirtualQuery` finds
   under a local (`AllocationBase`, Windows 7 has it, unlike
   `GetCurrentThreadStackLimits`), its size counted up to that local. */
int plat_thread_stack(uintptr_t *lo, size_t *size);
/* A stack of `size` bytes reserved, not committed, with an inaccessible
   guard of `guard` bytes below it: the lowest usable address, or 0 when it
   could not be made (POSIX: `mmap` with MAP_NORESERVE | MAP_STACK,
   `mprotect` of the guard; Windows: always 0, the program runs on a
   thread instead). */
uintptr_t plat_reserve_stack(size_t size, size_t guard);
/* Bug 032: the calling thread's stack, from `lo` up, is checked down to
   `floor` (D-0077). An access below the floor, or in the guard beneath the
   stack, that the system refuses (a frame too large for what is left, which
   the C compiler's stack probes touch a page at a time) calls `on_overflow`
   on a stack of its own, rather than end the process with SIGSEGV. Windows:
   EXCEPTION_STACK_OVERFLOW, or an access violation below the floor in the
   stack's reservation, calls `on_overflow` where the access was made
   (x86-64). */
void plat_watch_stack(uintptr_t lo, uintptr_t floor, void (*on_overflow)(void));
/* After `on_overflow`'s fault was contained and the thread goes on: Windows
   makes the stack's guard page again (as `_resetstkoflw`), so that the next
   overflow is caught as the first was; nothing on POSIX. */
void plat_stack_recovered(void);
/* The calling thread ends: what plat_watch_stack gave it is freed. */
void plat_unwatch_stack(void);

/* ---- added by the init part ---- */

/* Runs `fn(arg)` when the calling thread ends (D-0257: a foreign thread's
   registration released, as Rust's thread-local destructor does). POSIX: a
   pthread key's destructor; Windows: a fiber-local slot's callback. */
void plat_thread_at_exit(void (*fn)(void *), void *arg);

/* ---- over-aligned blocks ---- */

/* An over-aligned block (Rust's `std::alloc::alloc` with a Layout whose
   alignment passes `malloc`'s): POSIX `posix_memalign`, Windows
   `_aligned_malloc`; NULL on failure. Freed by `plat_aligned_free`
   (`free` / `_aligned_free`). */
void *plat_aligned_alloc(size_t size, size_t align);
void plat_aligned_free(void *p);

#endif
