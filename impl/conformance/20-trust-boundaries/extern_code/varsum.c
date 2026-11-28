/* Variadic functions of the program's own C code (report 228). */
#include <stdarg.h>
#include <stdint.h>

/* `count` pairs of an int weight and a double value. */
double weighted_sum(int count, ...)
{
    va_list ap;
    double total = 0.0;
    va_start(ap, count);
    for (int i = 0; i < count; i++)
    {
        int weight = va_arg(ap, int);
        double value = va_arg(ap, double);
        total += weight * value;
    }
    va_end(ap);
    return total;
}

/* The CobaltC struct `Pair { i32 weight; f64 value; }`, laid out as C lays it out. */
struct pair
{
    int32_t weight;
    double value;
};

/* `count` structs passed by value. */
double pair_sum(int count, ...)
{
    va_list ap;
    double total = 0.0;
    va_start(ap, count);
    for (int i = 0; i < count; i++)
    {
        struct pair p = va_arg(ap, struct pair);
        total += p.weight * p.value;
    }
    va_end(ap);
    return total;
}
