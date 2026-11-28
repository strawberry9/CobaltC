/* `std::os_random` (`spec/21` `[Os-Random]`, D-0142): bytes from the
   operating system's cryptographically secure source. Never seeded from a
   clock: when the system has no source, the answer is -1 and `std` faults
   (`diag.entropy-unavailable`).

   Linux: the `getrandom(2)` system call (by number, so an old C library
   does not matter), falling back to `/dev/urandom` on a kernel without
   it; Windows: `BCryptGenRandom` with the system's preferred generator
   (plat_posix.c, plat_win.c: `os_random`). */

#include "io.h"
#include "plat.h"
#include "plat_io.h"

/* Fills `p`: 0, or -1 when the system gives no secure bytes. */
int64_t osrand_fill(uint8_t *p, size_t n)
{
    if (n == 0 || os_random(p, n))
    {
        return 0;
    }
    return -1;
}

/* The seed of the hash tables' hash (D-0330; `cb_hash_start` in cbrt.h),
   drawn at the first hash a program makes: `COBALTC_HASH_SEED` when it
   is set (a decimal number: the same seed for every run, for measuring),
   else eight bytes of the system's randomness, else, where the system
   has none, the clock and an address. Never 0, which says "not drawn".
   Threads that race here agree on the first seed stored. */
uint64_t cb_hash_seed_cell = 0;

static uint64_t seed_spread(uint64_t z)
{
    z += 0x9E3779B97F4A7C15ull;
    z = (z ^ (z >> 30)) * 0xBF58476D1CE4E5B9ull;
    z = (z ^ (z >> 27)) * 0x94D049BB133111EBull;
    return z ^ (z >> 31);
}

uint64_t cb_hash_seed_make(void)
{
    uint64_t s = 0;
    const char *v = plat_getenv("COBALTC_HASH_SEED");
    if (v != NULL && *v != '\0')
    {
        uint64_t x = 0;
        for (; *v >= '0' && *v <= '9'; v++)
        {
            x = x * 10u + (uint64_t)(*v - '0');
        }
        s = seed_spread(x);
    }
    else if (!os_random((uint8_t *)&s, sizeof(s)))
    {
        s = seed_spread(plat_now_ns() ^ (uint64_t)(uintptr_t)&s);
    }
    if (s == 0)
    {
        s = 0x9E3779B97F4A7C15ull;
    }
    uint64_t none = 0;
    if (!__atomic_compare_exchange_n(&cb_hash_seed_cell, &none, s, 0, __ATOMIC_SEQ_CST, __ATOMIC_SEQ_CST))
    {
        s = none;
    }
    return s;
}
