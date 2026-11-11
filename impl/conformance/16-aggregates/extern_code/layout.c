/* D-0250, D-0251: C's view of the structs in struct_c_layout_ok.cb. */
#include <stddef.h>
#include <stdint.h>

struct rec
{
    uint8_t a;
    uint64_t b;
    uint16_t c;
    int32_t d;
    uint8_t e;
    double f;
    uint8_t flag;
};

struct outer
{
    uint8_t x;
    struct rec r;
    uint16_t y[3];
};

struct __attribute__((aligned(64))) slot
{
    uint32_t n;
};

struct lines
{
    uint8_t tag;
    struct slot s[2];
    uint8_t last;
};

/* Each struct's size, alignment and one field's offset, as C lays it out. */
uint64_t c_layout(uint32_t which)
{
    switch (which)
    {
    case 0: return sizeof(struct rec) * 10000 + _Alignof(struct rec) * 100 + offsetof(struct rec, f);
    case 1: return sizeof(struct outer) * 10000 + _Alignof(struct outer) * 100 + offsetof(struct outer, y);
    case 2: return sizeof(struct slot) * 10000 + _Alignof(struct slot) * 100 + 0;
    default: return sizeof(struct lines) * 10000 + _Alignof(struct lines) * 100 + offsetof(struct lines, last);
    }
}

/* Reads every field of a CobaltC `Outer` through a pointer, where C puts them. */
int64_t c_outer_sum(const struct outer *o)
{
    const struct rec *r = &o->r;
    return (int64_t)o->x + (int64_t)r->a + (int64_t)r->b + r->c + r->d + r->e + (int64_t)r->f + r->flag
        + o->y[0] + o->y[1] + o->y[2];
}

/* Reads the second slot's count and the last byte of a CobaltC `Lines`. */
int64_t c_lines_sum(const struct lines *l)
{
    return (int64_t)l->tag * 1000000 + (int64_t)l->s[1].n * 10 + l->last;
}
