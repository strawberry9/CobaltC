/* C code linked by extern_code_c_file_ok.cb (spec/20 rule.trust.extern-code). */
#include <stdint.h>

int32_t area(int32_t w, int32_t h)
{
    return w * h;
}

int64_t squares(int64_t *out, int64_t n)
{
    for (int64_t i = 0; i < n; i++)
    {
        out[i] = i * i;
    }
    return n;
}
