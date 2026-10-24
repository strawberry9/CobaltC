#include <stdint.h>

struct Point { int32_t x; int32_t y; };
struct Mixed { uint8_t tag; double weight; int16_t small; float f; };
struct WithArray { uint16_t n; int32_t vals[3]; };
struct Big { int64_t a, b, c, d, e; };
struct __attribute__((aligned(16))) Aligned { int32_t v; };
struct Nested { struct Point p; struct Point q; };

struct Point point_add(struct Point a, struct Point b) { struct Point r = { a.x + b.x, a.y + b.y }; return r; }
struct Mixed mixed_twice(struct Mixed m) { m.tag += 1; m.weight *= 2; m.small *= 2; m.f *= 2; return m; }
int32_t array_sum(struct WithArray w) { int32_t s = 0; for (int i = 0; i < 3; i++) s += w.vals[i]; return s + w.n; }
struct Big big_make(int64_t base) { struct Big b = { base, base + 1, base + 2, base + 3, base + 4 }; return b; }
int64_t big_sum(struct Big b) { return b.a + b.b + b.c + b.d + b.e; }
struct Aligned aligned_inc(struct Aligned a) { a.v += 1; return a; }
struct Nested nested_swap(struct Nested n) { struct Nested r = { n.q, n.p }; return r; }
