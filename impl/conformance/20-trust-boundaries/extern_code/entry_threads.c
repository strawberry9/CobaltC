#include <stdint.h>
#ifdef _WIN32
#include <windows.h>
#else
#include <pthread.h>
#endif

/* C threads the program did not start, calling an entry point defined in
   CobaltC (extern_entry_from_foreign_thread_ok.cb). */
uint64_t tally(uint64_t k);

#define MAX_THREADS 8
#define CALLS 2000

static uint64_t sums[MAX_THREADS];
static uint32_t started;

#ifdef _WIN32
static HANDLE threads[MAX_THREADS];
static DWORD WINAPI work(LPVOID arg)
#else
static pthread_t threads[MAX_THREADS];
static void *work(void *arg)
#endif
{
    uintptr_t i = (uintptr_t)arg;
    uint64_t s = 0;
    for (uint64_t k = 0; k < CALLS; k++)
    {
        s += tally(k);
    }
    sums[i] = s;
    return 0;
}

/* Starts `n` threads, each calling `tally` CALLS times; returns at once. */
void start_threads(uint32_t n)
{
    started = n > MAX_THREADS ? MAX_THREADS : n;
    for (uint32_t i = 0; i < started; i++)
    {
#ifdef _WIN32
        threads[i] = CreateThread(NULL, 0, work, (LPVOID)(uintptr_t)i, 0, NULL);
#else
        pthread_create(&threads[i], NULL, work, (void *)(uintptr_t)i);
#endif
    }
}

/* Waits for the threads `start_threads` started; the sum of their results. */
uint64_t join_threads(void)
{
    uint64_t total = 0;
    for (uint32_t i = 0; i < started; i++)
    {
#ifdef _WIN32
        WaitForSingleObject(threads[i], INFINITE);
        CloseHandle(threads[i]);
#else
        pthread_join(threads[i], NULL);
#endif
        total += sums[i];
    }
    return total;
}
