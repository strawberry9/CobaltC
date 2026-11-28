/* The containers of ds.h that are not inline: vector growth, the sort of
   (address, token) lists, and the ordered map (Rust's `BTreeMap<usize,
   u64>`), a B+ tree whose leaves are linked in key order so that a range
   is visited without climbing back up. */

#include "ds.h"

void u64vec_grow(u64vec *v, size_t need)
{
    size_t c = v->cap ? (size_t)v->cap * 2 : 4;
    if (c < need)
    {
        c = need;
    }
    if (c > 0xffffffffu)
    {
        cbrt_internal("a list of more than 2^32 ids");
    }
    v->p = (uint64_t *)cb_xrealloc(v->p, c * sizeof(uint64_t));
    v->cap = (uint32_t)c;
}

static int at_cmp(const void *x, const void *y)
{
    const addr_tok *a = (const addr_tok *)x;
    const addr_tok *b = (const addr_tok *)y;
    if (a->a != b->a)
    {
        return a->a < b->a ? -1 : 1;
    }
    if (a->t != b->t)
    {
        return a->t < b->t ? -1 : 1;
    }
    return 0;
}

void atvec_sort_from(atvec *v, size_t from)
{
    size_t n = v->n - from;
    if (n < 2)
    {
        return;
    }
    addr_tok *p = v->p + from;
    if (n <= 16)
    {
        for (size_t i = 1; i < n; i++)
        {
            addr_tok x = p[i];
            size_t j = i;
            while (j > 0 && at_cmp(&p[j - 1], &x) > 0)
            {
                p[j] = p[j - 1];
                j--;
            }
            p[j] = x;
        }
        return;
    }
    qsort(p, n, sizeof(addr_tok), at_cmp);
}

/* ---- the ordered map ---- */

/* Up to OB keys in a leaf, OB children in an inner node; every node but
   the root holds at least OB / 2. An inner node's key i is the least key
   of its child i + 1. */
#define OB 16
#define OMIN (OB / 2)

struct omap_node
{
    uint16_t n;
    uint8_t leaf;
    omap_node *next; /* a leaf's successor */
    uint64_t k[OB];
    union
    {
        uint64_t v[OB];
        omap_node *c[OB];
    } u;
};

static omap_node *node_new(int leaf)
{
    omap_node *x = (omap_node *)cb_xmalloc(sizeof(omap_node));
    x->n = 0;
    x->leaf = (uint8_t)leaf;
    x->next = NULL;
    return x;
}

/* The first position in a leaf whose key is >= k. */
static inline int leaf_lb(const omap_node *x, uint64_t k)
{
    int lo = 0, hi = x->n;
    while (lo < hi)
    {
        int m = (lo + hi) >> 1;
        if (x->k[m] < k)
        {
            lo = m + 1;
        }
        else
        {
            hi = m;
        }
    }
    return lo;
}

/* The child of an inner node that holds k: the separators <= k. */
static inline int inner_child(const omap_node *x, uint64_t k)
{
    int lo = 0, hi = x->n - 1;
    while (lo < hi)
    {
        int m = (lo + hi) >> 1;
        if (x->k[m] <= k)
        {
            lo = m + 1;
        }
        else
        {
            hi = m;
        }
    }
    return lo;
}

bool omap_get(const omap *m, uint64_t k, uint64_t *v)
{
    const omap_node *x = m->root;
    if (!x)
    {
        return false;
    }
    while (!x->leaf)
    {
        x = x->u.c[inner_child(x, k)];
    }
    int i = leaf_lb(x, k);
    if (i < x->n && x->k[i] == k)
    {
        if (v)
        {
            *v = x->u.v[i];
        }
        return true;
    }
    return false;
}

/* Inserts below `x`; when `x` splits, its new right sibling is returned
   with the sibling's least key in *sep. */
static omap_node *ins(omap_node *x, uint64_t k, uint64_t v, bool *had, uint64_t *old, uint64_t *sep)
{
    if (x->leaf)
    {
        int i = leaf_lb(x, k);
        if (i < x->n && x->k[i] == k)
        {
            *had = true;
            if (old)
            {
                *old = x->u.v[i];
            }
            x->u.v[i] = v;
            return NULL;
        }
        *had = false;
        if (x->n < OB)
        {
            memmove(&x->k[i + 1], &x->k[i], (size_t)(x->n - i) * 8);
            memmove(&x->u.v[i + 1], &x->u.v[i], (size_t)(x->n - i) * 8);
            x->k[i] = k;
            x->u.v[i] = v;
            x->n++;
            return NULL;
        }
        /* Full: split in two halves, then insert into the half it belongs to. */
        omap_node *r = node_new(1);
        int h = OB / 2;
        memcpy(r->k, &x->k[h], (size_t)(OB - h) * 8);
        memcpy(r->u.v, &x->u.v[h], (size_t)(OB - h) * 8);
        r->n = (uint16_t)(OB - h);
        x->n = (uint16_t)h;
        r->next = x->next;
        x->next = r;
        omap_node *t = i <= h ? x : r;
        int j = i <= h ? i : i - h;
        memmove(&t->k[j + 1], &t->k[j], (size_t)(t->n - j) * 8);
        memmove(&t->u.v[j + 1], &t->u.v[j], (size_t)(t->n - j) * 8);
        t->k[j] = k;
        t->u.v[j] = v;
        t->n++;
        *sep = r->k[0];
        return r;
    }
    int ci = inner_child(x, k);
    uint64_t csep;
    omap_node *nc = ins(x->u.c[ci], k, v, had, old, &csep);
    if (!nc)
    {
        return NULL;
    }
    /* A new child after ci, separated by csep. */
    if (x->n < OB)
    {
        memmove(&x->u.c[ci + 2], &x->u.c[ci + 1], (size_t)(x->n - ci - 1) * sizeof(omap_node *));
        memmove(&x->k[ci + 1], &x->k[ci], (size_t)(x->n - 1 - ci) * 8);
        x->u.c[ci + 1] = nc;
        x->k[ci] = csep;
        x->n++;
        return NULL;
    }
    /* Full: gather the OB + 1 children and OB separators, split them. */
    omap_node *cs[OB + 1];
    uint64_t ks[OB];
    /* Children: nc after ci. Separators: csep at ci. */
    for (int i = 0, j = 0; i < OB; i++)
    {
        cs[j++] = x->u.c[i];
        if (i == ci)
        {
            cs[j++] = nc;
        }
    }
    for (int i = 0, j = 0; i <= OB - 1; i++)
    {
        if (i == ci)
        {
            ks[j++] = csep;
        }
        if (i < OB - 1)
        {
            ks[j++] = x->k[i];
        }
    }
    int lc = (OB + 1) / 2;       /* children kept on the left */
    int rc = OB + 1 - lc;        /* moved right */
    omap_node *r = node_new(0);
    for (int i = 0; i < lc; i++)
    {
        x->u.c[i] = cs[i];
    }
    for (int i = 0; i < lc - 1; i++)
    {
        x->k[i] = ks[i];
    }
    x->n = (uint16_t)lc;
    *sep = ks[lc - 1];
    for (int i = 0; i < rc; i++)
    {
        r->u.c[i] = cs[lc + i];
    }
    for (int i = 0; i < rc - 1; i++)
    {
        r->k[i] = ks[lc + i];
    }
    r->n = (uint16_t)rc;
    return r;
}

bool omap_insert(omap *m, uint64_t k, uint64_t v, uint64_t *old)
{
    if (!m->root)
    {
        m->root = node_new(1);
        m->depth = 1;
    }
    bool had = false;
    uint64_t sep;
    omap_node *r = ins(m->root, k, v, &had, old, &sep);
    if (r)
    {
        omap_node *top = node_new(0);
        top->u.c[0] = m->root;
        top->u.c[1] = r;
        top->k[0] = sep;
        top->n = 2;
        m->root = top;
        m->depth++;
    }
    if (!had)
    {
        m->n++;
    }
    return had;
}

/* The least key under `x`. */
static uint64_t least(const omap_node *x)
{
    while (!x->leaf)
    {
        x = x->u.c[0];
    }
    return x->k[0];
}

/* Child ci of inner node x has fewer than OMIN entries: borrow from a
   sibling or merge with one. */
static void fix_child(omap_node *x, int ci)
{
    omap_node *c = x->u.c[ci];
    int li = ci > 0 ? ci - 1 : ci;      /* the left one of the pair */
    omap_node *a = x->u.c[li];
    omap_node *b = x->u.c[li + 1];
    if (c->leaf)
    {
        if (a->n + b->n <= OB)
        {
            /* Merge b into a. */
            memcpy(&a->k[a->n], b->k, (size_t)b->n * 8);
            memcpy(&a->u.v[a->n], b->u.v, (size_t)b->n * 8);
            a->n = (uint16_t)(a->n + b->n);
            a->next = b->next;
            free(b);
            memmove(&x->u.c[li + 1], &x->u.c[li + 2], (size_t)(x->n - li - 2) * sizeof(omap_node *));
            memmove(&x->k[li], &x->k[li + 1], (size_t)(x->n - li - 2) * 8);
            x->n--;
            return;
        }
        if (c == a)
        {
            /* Borrow b's first. */
            a->k[a->n] = b->k[0];
            a->u.v[a->n] = b->u.v[0];
            a->n++;
            memmove(&b->k[0], &b->k[1], (size_t)(b->n - 1) * 8);
            memmove(&b->u.v[0], &b->u.v[1], (size_t)(b->n - 1) * 8);
            b->n--;
        }
        else
        {
            /* Borrow a's last. */
            memmove(&b->k[1], &b->k[0], (size_t)b->n * 8);
            memmove(&b->u.v[1], &b->u.v[0], (size_t)b->n * 8);
            b->k[0] = a->k[a->n - 1];
            b->u.v[0] = a->u.v[a->n - 1];
            b->n++;
            a->n--;
        }
        x->k[li] = b->k[0];
        return;
    }
    if (a->n + b->n <= OB)
    {
        /* Merge b into a, the separator between them coming down. */
        a->k[a->n - 1] = x->k[li];
        memcpy(&a->k[a->n], b->k, (size_t)(b->n - 1) * 8);
        memcpy(&a->u.c[a->n], b->u.c, (size_t)b->n * sizeof(omap_node *));
        a->n = (uint16_t)(a->n + b->n);
        free(b);
        memmove(&x->u.c[li + 1], &x->u.c[li + 2], (size_t)(x->n - li - 2) * sizeof(omap_node *));
        memmove(&x->k[li], &x->k[li + 1], (size_t)(x->n - li - 2) * 8);
        x->n--;
        return;
    }
    if (c == a)
    {
        /* b's first child moves to a's end. */
        a->k[a->n - 1] = x->k[li];
        a->u.c[a->n] = b->u.c[0];
        a->n++;
        x->k[li] = b->k[0];
        memmove(&b->u.c[0], &b->u.c[1], (size_t)(b->n - 1) * sizeof(omap_node *));
        memmove(&b->k[0], &b->k[1], (size_t)(b->n - 2) * 8);
        b->n--;
    }
    else
    {
        /* a's last child moves to b's front. */
        memmove(&b->u.c[1], &b->u.c[0], (size_t)b->n * sizeof(omap_node *));
        memmove(&b->k[1], &b->k[0], (size_t)(b->n - 1) * 8);
        b->u.c[0] = a->u.c[a->n - 1];
        b->k[0] = x->k[li];
        b->n++;
        x->k[li] = a->k[a->n - 2];
        a->n--;
    }
}

static bool del(omap_node *x, uint64_t k, uint64_t *old)
{
    if (x->leaf)
    {
        int i = leaf_lb(x, k);
        if (i >= x->n || x->k[i] != k)
        {
            return false;
        }
        if (old)
        {
            *old = x->u.v[i];
        }
        memmove(&x->k[i], &x->k[i + 1], (size_t)(x->n - i - 1) * 8);
        memmove(&x->u.v[i], &x->u.v[i + 1], (size_t)(x->n - i - 1) * 8);
        x->n--;
        return true;
    }
    int ci = inner_child(x, k);
    if (!del(x->u.c[ci], k, old))
    {
        return false;
    }
    omap_node *c = x->u.c[ci];
    /* A separator equal to the removed key names the child's new least. */
    if (ci > 0 && x->k[ci - 1] == k && c->n > 0)
    {
        x->k[ci - 1] = least(c);
    }
    if (c->n < OMIN)
    {
        fix_child(x, ci);
    }
    return true;
}

bool omap_remove(omap *m, uint64_t k, uint64_t *old)
{
    if (!m->root)
    {
        return false;
    }
    if (!del(m->root, k, old))
    {
        return false;
    }
    m->n--;
    omap_node *r = m->root;
    if (!r->leaf && r->n == 1)
    {
        m->root = r->u.c[0];
        m->depth--;
        free(r);
    }
    /* The last leaf, emptied, is kept: a map that is filled and emptied
       over and over (an element's object at every access) allocates once. */
    return true;
}

static void free_tree(omap_node *x)
{
    if (!x->leaf)
    {
        for (int i = 0; i < x->n; i++)
        {
            free_tree(x->u.c[i]);
        }
    }
    free(x);
}

void omap_clear(omap *m)
{
    if (m->root)
    {
        free_tree(m->root);
    }
    m->root = NULL;
    m->n = 0;
    m->depth = 0;
}

omap_it omap_lower_bound(const omap *m, uint64_t lo)
{
    omap_it it = {NULL, 0};
    const omap_node *x = m->root;
    if (!x)
    {
        return it;
    }
    while (!x->leaf)
    {
        x = x->u.c[inner_child(x, lo)];
    }
    int i = leaf_lb(x, lo);
    while (x && i >= x->n)
    {
        x = x->next;
        i = 0;
    }
    it.leaf = x;
    it.i = i;
    return it;
}

bool omap_it_next(omap_it *it, uint64_t *k, uint64_t *v)
{
    const omap_node *x = it->leaf;
    if (!x)
    {
        return false;
    }
    if (k)
    {
        *k = x->k[it->i];
    }
    if (v)
    {
        *v = x->u.v[it->i];
    }
    it->i++;
    while (x && it->i >= x->n)
    {
        x = x->next;
        it->i = 0;
    }
    it->leaf = x;
    return true;
}
