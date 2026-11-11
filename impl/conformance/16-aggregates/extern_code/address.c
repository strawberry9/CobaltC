/* D-0251: an address modulo m, for struct_align_addresses_ok.cb. */
#include <stdint.h>

uint64_t addr_mod(void *p, uint64_t m)
{
    return (uint64_t)(uintptr_t)p % m;
}
