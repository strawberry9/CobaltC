#include <stdint.h>

/* Entry points defined in CobaltC (extern_entry_called_from_c_ok.cb). */
struct Pair { int32_t a; int32_t b; };
int64_t weight(int64_t i);
struct Pair swap_pair(struct Pair p);
void bump(void);

int64_t twice_weight(int64_t i)
{
    return weight(i) + weight(i);
}

int32_t pair_diff(int32_t a, int32_t b)
{
    struct Pair p = { a, b };
    struct Pair q = swap_pair(p);
    bump();
    return q.a - q.b;
}
