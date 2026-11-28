/* `read_file`'s and `write_file`'s file-system access (`spec/21` §2e
   `rule.stdlib.file`). A path is the bytes of a `String`, so it is UTF-8;
   it is resolved as the operating system resolves it (relative to the
   working directory).

   Failures are codes: -1 the file does not exist, -2 access is denied,
   -3 anything else (a directory, a device error, ...).

   `File` (`rule.stdlib.file-handle`, D-0054) is a table of open files,
   shared by every thread: a `File`'s handle is its index. Each entry keeps
   a read-ahead buffer, so `read_line` does not read a byte at a time; a
   write or a seek first gives back what was read ahead but not taken, so
   the file's position is always the one the program sees.

   D-0201 (`[File-Buffer]`): writes are buffered too, `OUT_CAP` bytes per
   entry. The bytes reach the operating system before any other operation
   on the same file, before any other file-system operation of the program
   (`fileio_flush_all`, at every path-taking entry point here and in
   procio before a child starts), and at the program's end. A failure found
   while writing them out is kept (`failed`) and reported by the file's next
   operation.

   Rust's `std::path` is ported here (`Components`, `push`, `parent`,
   `file_name`, ...), for the path grammar of the platform the program runs
   on (`os_windows`): `C:\` and `\\server\share` on Windows. */

#include "io.h"
#include "plat.h"
#include "plat_io.h"

#define NOT_FOUND FILEIO_NOT_FOUND
#define DENIED FILEIO_DENIED
#define OTHER FILEIO_OTHER
#define EXISTS FILEIO_EXISTS

static int64_t code(int kind)
{
    switch (kind)
    {
    case OS_NOT_FOUND:
        return NOT_FOUND;
    case OS_DENIED:
        return DENIED;
    case OS_IS_DIR:
        return FILEIO_IS_DIR;
    case OS_NOT_DIR:
        return FILEIO_NOT_DIR;
    case OS_NO_SPACE:
        return FILEIO_NO_SPACE;
    case OS_TOO_MANY_OPEN:
        return FILEIO_TOO_MANY_OPEN;
    case OS_NOT_EMPTY:
        return FILEIO_NOT_EMPTY;
    case OS_LINK_LOOP:
        return FILEIO_LINK_LOOP;
    case OS_SAME_FILE:
        return FILEIO_SAME_FILE;
    case OS_CROSS_DEVICE:
        return FILEIO_CROSS_DEVICE;
    case OS_NAME_TOO_LONG:
        return FILEIO_NAME_TOO_LONG;
    case OS_INVALID:
        return FILEIO_INVALID;
    case OS_BAD_HANDLE:
        return FILEIO_BAD_HANDLE;
    case OS_ARGS_TOO_LONG:
        return FILEIO_ARGS_TOO_LONG;
    default:
        return OTHER;
    }
}

/* ---- UTF-8 (`std::str::from_utf8`, `String::from_utf8_lossy`) ---- */

/* The length of the valid UTF-8 sequence at `p[i]`, or 0 when it is not
   one; `*bad` the length of the maximal invalid subpart then (Rust's
   `Utf8Chunks`: at least 1). */
static size_t utf8_seq(const uint8_t *p, size_t n, size_t i, size_t *bad)
{
    uint8_t c = p[i];
    *bad = 1;
    if (c < 0x80)
    {
        return 1;
    }
    size_t w;
    uint8_t lo = 0x80, hi = 0xBF;
    if (c >= 0xC2 && c <= 0xDF)
    {
        w = 2;
    }
    else if (c >= 0xE0 && c <= 0xEF)
    {
        w = 3;
        if (c == 0xE0)
        {
            lo = 0xA0;
        }
        else if (c == 0xED)
        {
            hi = 0x9F;
        }
    }
    else if (c >= 0xF0 && c <= 0xF4)
    {
        w = 4;
        if (c == 0xF0)
        {
            lo = 0x90;
        }
        else if (c == 0xF4)
        {
            hi = 0x8F;
        }
    }
    else
    {
        return 0;
    }
    for (size_t k = 1; k < w; k++)
    {
        if (i + k >= n)
        {
            *bad = k;
            return 0;
        }
        uint8_t d = p[i + k];
        uint8_t l = k == 1 ? lo : 0x80, h = k == 1 ? hi : 0xBF;
        if (d < l || d > h)
        {
            *bad = k;
            return 0;
        }
    }
    return w;
}

static int utf8_valid(const uint8_t *p, size_t n)
{
    size_t i = 0;
    while (i < n)
    {
        if (p[i] < 0x80)
        {
            i++;
            continue;
        }
        size_t bad;
        size_t w = utf8_seq(p, n, i, &bad);
        if (!w)
        {
            return 0;
        }
        i += w;
    }
    return 1;
}

/* `OsStr::to_string_lossy` in place: on POSIX `String::from_utf8_lossy`
   (one U+FFFD per maximal invalid subpart); on Windows the bytes are WTF-8,
   and each surrogate (three bytes) becomes one U+FFFD. */
static void lossy(cb_buf *b)
{
    if (utf8_valid(b->p, b->n))
    {
        return;
    }
    cb_buf o = {0};
    size_t i = 0;
    while (i < b->n)
    {
        uint8_t c = b->p[i];
        if (os_windows && c == 0xED && i + 2 < b->n && b->p[i + 1] >= 0xA0 && b->p[i + 1] <= 0xBF)
        {
            cb_buf_put(&o, "\xEF\xBF\xBD", 3);
            i += 3;
            continue;
        }
        size_t bad;
        size_t w = utf8_seq(b->p, b->n, i, &bad);
        if (w)
        {
            cb_buf_put(&o, b->p + i, w);
            i += w;
        }
        else
        {
            cb_buf_put(&o, "\xEF\xBF\xBD", 3);
            i += bad;
        }
    }
    cb_buf_free(b);
    *b = o;
}

/* `path`: the bytes as a `&str`, or OTHER. */
static int path_ok(const uint8_t *p, size_t n)
{
    return utf8_valid(p, n);
}

/* ---- std::path ---- */

typedef struct
{
    const uint8_t *p;
    size_t n;
} bstr;

enum
{
    PFX_NONE,
    PFX_VERBATIM,
    PFX_VERBATIM_UNC,
    PFX_VERBATIM_DISK,
    PFX_DEVICE_NS,
    PFX_UNC,
    PFX_DISK
};

typedef struct
{
    int kind;
    size_t len;
} prefix;

/* Whether `\\?\` written with a `/` is refused as verbatim ("the meaning of
   verbatim paths can change when they use a different separator"): 1, as
   Rust's `std::path` does in its release 1.77.2 -- tests/io found
   `//?/C:/x` a UNC path on Windows (server `?`, share `C:`). */
#ifndef CBRT_VERBATIM_SLASH_CHECK
#define CBRT_VERBATIM_SLASH_CHECK 1
#endif

static int is_sep(uint8_t b)
{
    return b == '/' || (os_windows && b == '\\');
}

static int is_verbatim_sep(uint8_t b)
{
    return b == '\\';
}

static int pfx_verbatim(int k)
{
    return k == PFX_VERBATIM || k == PFX_VERBATIM_UNC || k == PFX_VERBATIM_DISK;
}

/* `Prefix::has_implicit_root`: every prefix but a disk's. */
static int pfx_implicit_root(int k)
{
    return k != PFX_NONE && k != PFX_DISK;
}

/* `parse_next_component`: the bytes up to the first separator, and the
   rest after it. */
static void next_comp(bstr path, int verbatim, bstr *comp, bstr *rest)
{
    for (size_t i = 0; i < path.n; i++)
    {
        if (verbatim ? is_verbatim_sep(path.p[i]) : is_sep(path.p[i]))
        {
            comp->p = path.p;
            comp->n = i;
            rest->p = path.p + i + 1;
            rest->n = path.n - i - 1;
            return;
        }
    }
    *comp = path;
    rest->p = path.p + path.n;
    rest->n = 0;
}

static int parse_drive(bstr p)
{
    if (p.n >= 2 && p.p[1] == ':' && ((p.p[0] >= 'a' && p.p[0] <= 'z') || (p.p[0] >= 'A' && p.p[0] <= 'Z')))
    {
        return 1;
    }
    return 0;
}

/* Windows' `parse_prefix`, through `PrefixParser<8>`: the first eight bytes
   read with `/` as `\`. */
static prefix parse_prefix(const uint8_t *s, size_t n)
{
    prefix r = {PFX_NONE, 0};
    if (!os_windows)
    {
        return r;
    }
    uint8_t pre[8];
    size_t pn = n < 8 ? n : 8;
    for (size_t i = 0; i < pn; i++)
    {
        pre[i] = s[i] == '/' ? '\\' : s[i];
    }
#define STARTS(at, lit) ((at) + sizeof(lit) - 1 <= pn && memcmp(pre + (at), lit, sizeof(lit) - 1) == 0)
    if (STARTS(0, "\\\\"))
    {
        size_t at = 2;
        int verb = STARTS(at, "?\\");
        /* The meaning of verbatim paths can change when they use a
           different separator. */
        if (verb && CBRT_VERBATIM_SLASH_CHECK)
        {
            for (size_t i = 0; i < at + 2; i++)
            {
                if (s[i] == '/')
                {
                    verb = 0;
                }
            }
        }
        if (verb)
        {
            at += 2;
            if (STARTS(at, "UNC\\"))
            {
                at += 4;
                bstr path = {s + at, n - at}, server, share, rest;
                next_comp(path, 1, &server, &rest);
                next_comp(rest, 1, &share, &rest);
                r.kind = PFX_VERBATIM_UNC;
                r.len = 8 + server.n + (share.n > 0 ? 1 + share.n : 0);
                return r;
            }
            bstr path = {s + at, n - at};
            if ((path.n <= 2 || is_verbatim_sep(path.p[2])) && parse_drive(path))
            {
                r.kind = PFX_VERBATIM_DISK;
                r.len = 6;
                return r;
            }
            bstr comp, rest;
            next_comp(path, 1, &comp, &rest);
            r.kind = PFX_VERBATIM;
            r.len = 4 + comp.n;
            return r;
        }
        if (STARTS(at, ".\\"))
        {
            at += 2;
            bstr path = {s + at, n - at}, comp, rest;
            next_comp(path, 0, &comp, &rest);
            r.kind = PFX_DEVICE_NS;
            r.len = 4 + comp.n;
            return r;
        }
        bstr path = {s + at, n - at}, server, share, rest;
        next_comp(path, 0, &server, &rest);
        next_comp(rest, 0, &share, &rest);
        if (server.n && share.n)
        {
            r.kind = PFX_UNC;
            r.len = 2 + server.n + 1 + share.n;
        }
        return r;
    }
#undef STARTS
    bstr all = {s, n};
    if (parse_drive(all))
    {
        r.kind = PFX_DISK;
        r.len = 2;
    }
    return r;
}

enum
{
    ST_PREFIX = 0,
    ST_STARTDIR = 1,
    ST_BODY = 2,
    ST_DONE = 3
};

enum
{
    C_PREFIX,
    C_ROOT,
    C_CUR,
    C_PARENT,
    C_NORMAL
};

typedef struct
{
    int kind;
    bstr s;
} comp;

/* `std::path::Components`. */
typedef struct
{
    const uint8_t *path;
    size_t n;
    prefix pfx;
    int phys_root;
    int front, back;
} comps;

static int c_sep(const comps *c, uint8_t b)
{
    return pfx_verbatim(c->pfx.kind) ? is_verbatim_sep(b) : is_sep(b);
}

static comps components(const uint8_t *p, size_t n)
{
    comps c;
    c.path = p;
    c.n = n;
    c.pfx = parse_prefix(p, n);
    size_t at = c.pfx.kind != PFX_NONE ? c.pfx.len : 0;
    c.phys_root = n > at && is_sep(p[at]);
    c.front = ST_PREFIX;
    c.back = ST_BODY;
    return c;
}

static size_t prefix_remaining(const comps *c)
{
    return c->front == ST_PREFIX && c->pfx.kind != PFX_NONE ? c->pfx.len : 0;
}

static int c_has_root(const comps *c)
{
    return c->phys_root || pfx_implicit_root(c->pfx.kind);
}

static int include_cur_dir(const comps *c)
{
    if (c_has_root(c))
    {
        return 0;
    }
    size_t at = prefix_remaining(c);
    if (at < c->n && c->path[at] == '.')
    {
        if (at + 1 == c->n)
        {
            return 1;
        }
        return c_sep(c, c->path[at + 1]);
    }
    return 0;
}

static size_t len_before_body(const comps *c)
{
    size_t root = c->front <= ST_STARTDIR && c->phys_root ? 1 : 0;
    size_t cur = c->front <= ST_STARTDIR && include_cur_dir(c) ? 1 : 0;
    return prefix_remaining(c) + root + cur;
}

/* `parse_single_component`: 0 for none. */
static int single(const comps *c, bstr s, comp *out)
{
    out->s = s;
    if (s.n == 1 && s.p[0] == '.')
    {
        if (pfx_verbatim(c->pfx.kind))
        {
            out->kind = C_CUR;
            return 1;
        }
        return 0;
    }
    if (s.n == 2 && s.p[0] == '.' && s.p[1] == '.')
    {
        out->kind = C_PARENT;
        return 1;
    }
    if (s.n == 0)
    {
        return 0;
    }
    out->kind = C_NORMAL;
    return 1;
}

static int finished(const comps *c)
{
    return c->front == ST_DONE || c->back == ST_DONE || c->front > c->back;
}

static int comp_next(comps *c, comp *out)
{
    while (!finished(c))
    {
        switch (c->front)
        {
        case ST_PREFIX:
            if (c->pfx.kind != PFX_NONE && c->pfx.len > 0)
            {
                c->front = ST_STARTDIR;
                out->kind = C_PREFIX;
                out->s.p = c->path;
                out->s.n = c->pfx.len;
                c->path += c->pfx.len;
                c->n -= c->pfx.len;
                return 1;
            }
            c->front = ST_STARTDIR;
            break;
        case ST_STARTDIR:
            c->front = ST_BODY;
            if (c->phys_root)
            {
                out->kind = C_ROOT;
                out->s.p = c->path;
                out->s.n = 1;
                c->path++;
                c->n--;
                return 1;
            }
            else if (c->pfx.kind != PFX_NONE)
            {
                if (pfx_implicit_root(c->pfx.kind) && !pfx_verbatim(c->pfx.kind))
                {
                    out->kind = C_ROOT;
                    out->s.p = c->path;
                    out->s.n = 0;
                    return 1;
                }
            }
            else if (include_cur_dir(c))
            {
                {
                    out->kind = C_CUR;
                    out->s.p = c->path;
                    out->s.n = 1;
                    c->path++;
                    c->n--;
                    return 1;
                }
            }
            break;
        case ST_BODY:
            if (c->n > 0)
            {
                bstr all = {c->path, c->n};
                size_t i = 0, extra = 0;
                while (i < all.n && !c_sep(c, all.p[i]))
                {
                    i++;
                }
                if (i < all.n)
                {
                    extra = 1;
                }
                bstr s = {all.p, i};
                c->path += i + extra;
                c->n -= i + extra;
                if (single(c, s, out))
                {
                    return 1;
                }
            }
            else
            {
                c->front = ST_DONE;
            }
            break;
        default:
            cbrt_internal("path components past their end");
        }
    }
    return 0;
}

static void back_part(const comps *c, size_t *size, bstr *s)
{
    size_t start = len_before_body(c);
    size_t i = c->n;
    while (i > start && !c_sep(c, c->path[i - 1]))
    {
        i--;
    }
    if (i > start)
    {
        s->p = c->path + i;
        s->n = c->n - i;
        *size = s->n + 1;
    }
    else
    {
        s->p = c->path + start;
        s->n = c->n - start;
        *size = s->n;
    }
}

static int comp_next_back(comps *c, comp *out)
{
    while (!finished(c))
    {
        switch (c->back)
        {
        case ST_BODY:
            if (c->n > len_before_body(c))
            {
                size_t size;
                bstr s;
                back_part(c, &size, &s);
                c->n -= size;
                if (single(c, s, out))
                {
                    return 1;
                }
            }
            else
            {
                c->back = ST_STARTDIR;
            }
            break;
        case ST_STARTDIR:
            c->back = ST_PREFIX;
            if (c->phys_root)
            {
                c->n--;
                out->kind = C_ROOT;
                out->s.p = c->path + c->n;
                out->s.n = 1;
                return 1;
            }
            else if (c->pfx.kind != PFX_NONE)
            {
                if (pfx_implicit_root(c->pfx.kind) && !pfx_verbatim(c->pfx.kind))
                {
                    out->kind = C_ROOT;
                    out->s.p = c->path;
                    out->s.n = 0;
                    return 1;
                }
            }
            else if (include_cur_dir(c))
            {
                c->n--;
                out->kind = C_CUR;
                out->s.p = c->path + c->n;
                out->s.n = 1;
                return 1;
            }
            break;
        case ST_PREFIX:
            c->back = ST_DONE;
            if (c->pfx.kind != PFX_NONE && c->pfx.len > 0)
            {
                out->kind = C_PREFIX;
                out->s.p = c->path;
                out->s.n = c->n;
                return 1;
            }
            return 0;
        default:
            cbrt_internal("path components past their end");
        }
    }
    return 0;
}

/* `Components::as_path`. */
static bstr comps_as_path(const comps *c0)
{
    comps c = *c0;
    if (c.front == ST_BODY)
    {
        while (c.n > 0)
        {
            size_t i = 0, extra = 0;
            while (i < c.n && !c_sep(&c, c.path[i]))
            {
                i++;
            }
            if (i < c.n)
            {
                extra = 1;
            }
            bstr s = {c.path, i};
            comp tmp;
            if (single(&c, s, &tmp))
            {
                break;
            }
            c.path += i + extra;
            c.n -= i + extra;
        }
    }
    if (c.back == ST_BODY)
    {
        while (c.n > len_before_body(&c))
        {
            size_t size;
            bstr s;
            back_part(&c, &size, &s);
            comp tmp;
            if (single(&c, s, &tmp))
            {
                break;
            }
            c.n -= size;
        }
    }
    bstr r = {c.path, c.n};
    return r;
}

/* A component's text (`Component::as_os_str`). */
static bstr comp_text(const comp *c)
{
    static const uint8_t fwd[] = "/", bwd[] = "\\", dot[] = ".", dots[] = "..";
    bstr r;
    switch (c->kind)
    {
    case C_ROOT:
        r.p = os_windows ? bwd : fwd;
        r.n = 1;
        return r;
    case C_CUR:
        r.p = dot;
        r.n = 1;
        return r;
    case C_PARENT:
        r.p = dots;
        r.n = 2;
        return r;
    default:
        return c->s;
    }
}

static int path_is_absolute(const uint8_t *p, size_t n)
{
    comps c = components(p, n);
    if (os_windows)
    {
        return c_has_root(&c) && c.pfx.kind != PFX_NONE;
    }
    return c_has_root(&c);
}

/* `PathBuf::push`. */
static void path_push(cb_buf *buf, const uint8_t *p, size_t n)
{
    int need_sep = buf->n > 0 && !is_sep(buf->p[buf->n - 1]);
    comps c = components(buf->p, buf->n);
    if (c.pfx.kind != PFX_NONE && c.pfx.len > 0 && c.pfx.len == buf->n && c.pfx.kind == PFX_DISK)
    {
        need_sep = 0;
    }
    comps pc = components(p, n);
    const uint8_t *sep = (const uint8_t *)(os_windows ? "\\" : "/");
    if (path_is_absolute(p, n) || pc.pfx.kind != PFX_NONE)
    {
        buf->n = 0;
    }
    else if (pfx_verbatim(c.pfx.kind) && n > 0)
    {
        /* Verbatim paths need `.` and `..` removed. */
        size_t cap = 4 + buf->n + n;
        comp *list = (comp *)cb_xmalloc(cap * sizeof(comp));
        size_t k = 0;
        comp x;
        while (comp_next(&c, &x))
        {
            list[k++] = x;
        }
        while (comp_next(&pc, &x))
        {
            switch (x.kind)
            {
            case C_ROOT:
                if (k > 1)
                {
                    k = 1;
                }
                list[k++] = x;
                break;
            case C_CUR:
                break;
            case C_PARENT:
                if (k > 0 && list[k - 1].kind == C_NORMAL)
                {
                    k--;
                }
                break;
            default:
                list[k++] = x;
            }
        }
        cb_buf res = {0};
        int ns = 0;
        for (size_t i = 0; i < k; i++)
        {
            if (ns && list[i].kind != C_ROOT)
            {
                cb_buf_put(&res, sep, 1);
            }
            bstr t = comp_text(&list[i]);
            cb_buf_put(&res, t.p, t.n);
            if (list[i].kind == C_ROOT)
            {
                ns = 0;
            }
            else if (list[i].kind == C_PREFIX)
            {
                prefix pf = parse_prefix(list[i].s.p, list[i].s.n);
                ns = pf.kind != PFX_DISK && list[i].s.n > 0;
            }
            else
            {
                ns = 1;
            }
        }
        free(list);
        cb_buf_free(buf);
        *buf = res;
        return;
    }
    else if (c_has_root(&pc))
    {
        buf->n = prefix_remaining(&c);
    }
    else if (need_sep)
    {
        cb_buf_put(buf, sep, 1);
    }
    cb_buf_put(buf, p, n);
}

/* `Path::file_name`: 1 and the name. */
static int path_file_name(const uint8_t *p, size_t n, bstr *out)
{
    comps c = components(p, n);
    comp x;
    if (comp_next_back(&c, &x) && x.kind == C_NORMAL)
    {
        *out = x.s;
        return 1;
    }
    return 0;
}

static int path_parent(const uint8_t *p, size_t n, bstr *out)
{
    comps c = components(p, n);
    comp x;
    if (comp_next_back(&c, &x) && (x.kind == C_NORMAL || x.kind == C_CUR || x.kind == C_PARENT))
    {
        *out = comps_as_path(&c);
        return 1;
    }
    return 0;
}

/* `rsplit_file_at_dot`: (before, after), each maybe absent. */
static void split_at_dot(bstr f, int *has_b, bstr *before, int *has_a, bstr *after)
{
    if (f.n == 2 && f.p[0] == '.' && f.p[1] == '.')
    {
        *has_b = 1;
        *before = f;
        *has_a = 0;
        return;
    }
    size_t i = f.n;
    while (i > 0 && f.p[i - 1] != '.')
    {
        i--;
    }
    if (i == 0)
    {
        /* No dot: `after` the whole name, `before` none. */
        *has_a = 1;
        *after = f;
        *has_b = 0;
        return;
    }
    before->p = f.p;
    before->n = i - 1;
    after->p = f.p + i;
    after->n = f.n - i;
    if (before->n == 0)
    {
        *has_b = 1;
        *before = f;
        *has_a = 0;
        return;
    }
    *has_b = 1;
    *has_a = 1;
}

static int path_stem(const uint8_t *p, size_t n, bstr *out)
{
    bstr f, b, a;
    int hb, ha;
    if (!path_file_name(p, n, &f))
    {
        return 0;
    }
    split_at_dot(f, &hb, &b, &ha, &a);
    if (hb)
    {
        *out = b;
        return 1;
    }
    if (ha)
    {
        *out = a;
        return 1;
    }
    return 0;
}

static int path_extension(const uint8_t *p, size_t n, bstr *out)
{
    bstr f, b, a;
    int hb, ha;
    if (!path_file_name(p, n, &f))
    {
        return 0;
    }
    split_at_dot(f, &hb, &b, &ha, &a);
    if (hb && ha)
    {
        *out = a;
        return 1;
    }
    return 0;
}

/* A lexical normalization: `.` parts removed, `x/..` removed where `x` is a
   name, `..` at the start kept, `/..` is `/`; nothing becomes `.`. */
static void normalize(const uint8_t *p, size_t n, cb_buf *out)
{
    comps c = components(p, n);
    comp *parts = (comp *)cb_xmalloc((n + 2) * sizeof(comp));
    size_t k = 0;
    comp x;
    while (comp_next(&c, &x))
    {
        switch (x.kind)
        {
        case C_CUR:
            break;
        case C_PARENT:
            if (k > 0 && parts[k - 1].kind == C_NORMAL)
            {
                k--;
            }
            else if (k > 0 && (parts[k - 1].kind == C_ROOT || parts[k - 1].kind == C_PREFIX))
            {
            }
            else
            {
                parts[k++] = x;
            }
            break;
        default:
            parts[k++] = x;
        }
    }
    out->n = 0;
    for (size_t i = 0; i < k; i++)
    {
        bstr t = comp_text(&parts[i]);
        /* `collect` pushes each part's text; a copy, since `push` may
           rebuild the buffer. */
        uint8_t small[8];
        uint8_t *tmp = t.n <= sizeof small ? small : (uint8_t *)cb_xmalloc(t.n);
        if (t.n)
        {
            memcpy(tmp, t.p, t.n);
        }
        path_push(out, tmp, t.n);
        if (tmp != small)
        {
            free(tmp);
        }
    }
    free(parts);
    if (out->n == 0)
    {
        cb_buf_put(out, ".", 1);
    }
}

/* `Path::new("") == path`: no components at all. */
static int path_is_empty(const uint8_t *p, size_t n)
{
    comps c = components(p, n);
    comp x;
    return !comp_next(&c, &x);
}

/* ---- the open-file table ---- */

/* What is read from the operating system at a time. */
#define CHUNK 8192
/* D-0201: the write buffer's size; a write as large is not buffered. */
#define OUT_CAP 8192

typedef struct
{
    os_file file;
    int readable;
    int writable;
    cb_buf ahead;
    size_t at;
    /* D-0201: bytes written but not yet handed to the operating system, and
       a failure found handing them over, for the next operation. */
    cb_buf out;
    int has_failed;
    int64_t failed;
} entry;

static plat_mutex table_lock = PLAT_MUTEX_INIT;
static entry **table;
static size_t table_n, table_cap;
/* D-0201: how many entries hold buffered bytes; `fileio_flush_all` with
   none costs one load. */
static size_t dirty;

/* `write_all` of a file: every byte, retrying an interrupted call; a write
   of none is an error (`WriteZero`). */
static int file_write_all(os_file f, const uint8_t *p, size_t n)
{
    while (n > 0)
    {
        size_t put = 0;
        int k = os_file_write(f, p, n, &put);
        if (k == OS_INTERRUPTED)
        {
            continue;
        }
        if (k != OS_OK)
        {
            return k;
        }
        if (put == 0)
        {
            return OS_WRITE_ZERO;
        }
        p += put;
        n -= put;
    }
    return OS_OK;
}

/* Hands the entry's buffered bytes to the operating system: 0 or a code. */
static int64_t drain_out(entry *e)
{
    if (e->out.n == 0)
    {
        return 0;
    }
    int k = file_write_all(e->file, e->out.p, e->out.n);
    e->out.n = 0;
    __atomic_fetch_sub(&dirty, 1, __ATOMIC_ACQ_REL);
    return k == OS_OK ? 0 : code(k);
}

/* The entry's buffered bytes written out, and any failure so far taken:
   what every operation on a file does first. */
static int64_t settle(entry *e)
{
    int64_t r = drain_out(e);
    if (e->has_failed)
    {
        e->has_failed = 0;
        return e->failed;
    }
    return r;
}

void fileio_flush_all(void)
{
    if (__atomic_load_n(&dirty, __ATOMIC_ACQUIRE) == 0)
    {
        return;
    }
    plat_mutex_lock(&table_lock);
    for (size_t i = 0; i < table_n; i++)
    {
        entry *e = table[i];
        if (e)
        {
            int64_t c = drain_out(e);
            if (c != 0 && !e->has_failed)
            {
                e->has_failed = 1;
                e->failed = c;
            }
        }
    }
    plat_mutex_unlock(&table_lock);
}

/* The entry under `h`, with the table locked (the caller unlocks), or NULL
   with it unlocked. */
static entry *find(uint64_t h)
{
    plat_mutex_lock(&table_lock);
    if (h < table_n && table[h])
    {
        return table[h];
    }
    plat_mutex_unlock(&table_lock);
    return NULL;
}

static int64_t fill(entry *e, size_t *got)
{
    if (e->at > 0)
    {
        memmove(e->ahead.p, e->ahead.p + e->at, e->ahead.n - e->at);
        e->ahead.n -= e->at;
        e->at = 0;
    }
    size_t old = e->ahead.n;
    cb_buf_reserve(&e->ahead, CHUNK);
    int k;
    size_t n = 0;
    do
    {
        k = os_file_read(e->file, e->ahead.p + old, CHUNK, &n);
    } while (k == OS_INTERRUPTED);
    if (k != OS_OK)
    {
        n = 0;
    }
    e->ahead.n = old + n;
    *got = n;
    return k == OS_OK ? 0 : code(k);
}

/* Gives back the bytes read ahead but not taken: the operating system's
   position becomes the program's. */
static int64_t unread(entry *e)
{
    int64_t rest = (int64_t)(e->ahead.n - e->at);
    e->ahead.n = 0;
    e->at = 0;
    if (rest > 0)
    {
        uint64_t pos;
        int k = os_file_seek(e->file, 1, -rest, &pos);
        if (k != OS_OK)
        {
            return code(k);
        }
    }
    return 0;
}

#define MODE_READ 0
#define MODE_CREATE 1
#define MODE_APPEND 2
#define MODE_READ_WRITE 3
#define MODE_CREATE_NEW 4
#define MODE_CREATE_PRIVATE 5 /* report 224: CREATE_NEW, mode 0600 */

static int64_t file_open(const uint8_t *p, size_t n, uint64_t mode)
{
    fileio_flush_all();
    if (mode > MODE_CREATE_PRIVATE)
    {
        return OTHER;
    }
    if (!path_ok(p, n))
    {
        return OTHER;
    }
    os_file f;
    int k = os_file_open(p, n, (int)mode, &f);
    if (k != OS_OK)
    {
        /* D-0255: Windows refuses to create a file over a directory as
           "access denied", not "already exists"; for `create_new` the name
           is taken either way. */
        int taken = k == OS_EXISTS || (os_windows && os_is_dir(p, n));
        if ((mode == MODE_CREATE_NEW || mode == MODE_CREATE_PRIVATE) && taken)
        {
            return EXISTS;
        }
        /* Issue 041: Windows refuses to open a directory as "access
           denied"; it is `IsDirectory` there too. */
        if (k == OS_DENIED && os_windows && os_is_dir(p, n))
        {
            return FILEIO_IS_DIR;
        }
        return code(k);
    }
    /* A directory opens on some systems; it is not a file to read. */
    uint64_t len;
    int is_dir = 1;
    int sk = os_file_stat(f, &len, &is_dir);
    if (sk != OS_OK || is_dir)
    {
        os_file_close(f);
        return sk == OS_OK ? FILEIO_IS_DIR : OTHER;
    }
    entry *e = (entry *)cb_xcalloc(1, sizeof(entry));
    e->file = f;
    e->readable = mode == MODE_READ || mode == MODE_READ_WRITE;
    e->writable = mode != MODE_READ;
    plat_mutex_lock(&table_lock);
    size_t i = 0;
    while (i < table_n && table[i])
    {
        i++;
    }
    if (i == table_n)
    {
        if (table_n == table_cap)
        {
            table_cap = table_cap ? table_cap * 2 : 8;
            table = (entry **)cb_xrealloc(table, table_cap * sizeof(entry *));
        }
        table_n++;
    }
    table[i] = e;
    plat_mutex_unlock(&table_lock);
    return (int64_t)i;
}

static void entry_free(entry *e)
{
    os_file_close(e->file);
    cb_buf_free(&e->ahead);
    cb_buf_free(&e->out);
    free(e);
}

/* Closes the file. With `sync`, a file open for writing is first made
   durable, so an error the operating system reports late (a full disk, a
   lost network file) is reported here; without it (a destructor), nothing
   is reported. */
static int64_t file_close(uint64_t h, int sync)
{
    entry *e;
    plat_mutex_lock(&table_lock);
    if (h < table_n && table[h])
    {
        e = table[h];
        table[h] = NULL;
    }
    else
    {
        plat_mutex_unlock(&table_lock);
        return OTHER;
    }
    plat_mutex_unlock(&table_lock);
    /* D-0201: the buffered bytes first; their failure is reported by
       `close`, not by a destructor. */
    int64_t settled = settle(e);
    int64_t r = 0;
    if (sync)
    {
        if (settled != 0)
        {
            r = settled;
        }
        else if (e->writable)
        {
            int k = os_file_sync(e->file);
            if (k != OS_OK)
            {
                r = code(k);
            }
        }
    }
    entry_free(e);
    return r;
}

/* Up to `cap` bytes, from the read-ahead first and then from the file
   directly, fewer only at the end of the file; none at the end. */
static int64_t read_some(uint64_t h, uint64_t cap64, cb_buf *out)
{
    entry *e = find(h);
    if (!e)
    {
        return OTHER;
    }
    int64_t r = settle(e);
    if (r != 0)
    {
        goto done;
    }
    /* A file not opened for reading: `NotOpenForThat` on every platform
       (report 326), rather than whatever the operating system would say
       (EBADF on Linux, access denied on Windows). */
    if (!e->readable)
    {
        r = FILEIO_BAD_HANDLE;
        goto done;
    }
    size_t cap = (size_t)cap64;
    if (cap == 0)
    {
        goto done;
    }
    if (e->at == e->ahead.n && cap < CHUNK)
    {
        size_t got;
        r = fill(e, &got);
        if (r != 0)
        {
            goto done;
        }
    }
    size_t k = e->ahead.n - e->at;
    if (k > cap)
    {
        k = cap;
    }
    cb_buf_put(out, e->ahead.p + e->at, k);
    e->at += k;
    /* The rest straight from the file (the read-ahead is empty now, or
       `out` is already full). */
    while (out->n < cap)
    {
        size_t old = out->n;
        cb_buf_reserve(out, cap - old);
        size_t got = 0;
        int kd;
        do
        {
            kd = os_file_read(e->file, out->p + old, cap - old, &got);
        } while (kd == OS_INTERRUPTED);
        if (kd == OS_OK && got == 0)
        {
            break;
        }
        if (kd == OS_OK)
        {
            out->n = old + got;
            continue;
        }
        if (old == 0)
        {
            r = code(kd);
            goto done;
        }
        break;
    }
    r = (int64_t)out->n;
done:
    plat_mutex_unlock(&table_lock);
    if (r < 0)
    {
        out->n = 0;
    }
    return r;
}

/* The length of the next line, its '\n' included (the rest of the file
   when there is none); 0 at the end. Its bytes are then in the read-ahead,
   and `read_some` takes them. */
static int64_t line_len(uint64_t h)
{
    entry *e = find(h);
    if (!e)
    {
        return OTHER;
    }
    int64_t r = settle(e);
    if (r != 0)
    {
        goto done;
    }
    if (!e->readable)
    {
        r = FILEIO_BAD_HANDLE;
        goto done;
    }
    size_t from = e->at;
    for (;;)
    {
        const uint8_t *nl = from < e->ahead.n ? (const uint8_t *)memchr(e->ahead.p + from, '\n', e->ahead.n - from) : NULL;
        if (nl)
        {
            r = (int64_t)((size_t)(nl - e->ahead.p) + 1 - e->at);
            goto done;
        }
        size_t seen = e->ahead.n - e->at;
        size_t got;
        r = fill(e, &got);
        if (r != 0)
        {
            goto done;
        }
        if (got == 0)
        {
            r = (int64_t)(e->ahead.n - e->at);
            goto done;
        }
        from = e->at + seen;
    }
done:
    plat_mutex_unlock(&table_lock);
    return r;
}

static int64_t file_write_buffered(uint64_t h, const uint8_t *d, size_t n)
{
    entry *e = find(h);
    if (!e)
    {
        return OTHER;
    }
    int64_t r = 0;
    if (e->has_failed)
    {
        e->has_failed = 0;
        r = e->failed;
        goto done;
    }
    /* Not opened for writing: `NotOpenForThat`, the same everywhere (as reading). */
    if (!e->writable)
    {
        r = FILEIO_BAD_HANDLE;
        goto done;
    }
    r = unread(e);
    if (r != 0)
    {
        goto done;
    }
    /* D-0201: into the buffer; one as large as the buffer, after what it
       holds, straight through. */
    if (e->out.n + n > OUT_CAP)
    {
        r = drain_out(e);
        if (r != 0)
        {
            goto done;
        }
    }
    if (n >= OUT_CAP)
    {
        int k = file_write_all(e->file, d, n);
        r = k == OS_OK ? 0 : code(k);
        goto done;
    }
    if (e->out.n == 0 && n > 0)
    {
        __atomic_fetch_add(&dirty, 1, __ATOMIC_ACQ_REL);
    }
    cb_buf_put(&e->out, d, n);
done:
    plat_mutex_unlock(&table_lock);
    return r;
}

static int64_t file_seek(uint64_t h, uint64_t pos)
{
    entry *e = find(h);
    if (!e)
    {
        return OTHER;
    }
    int64_t r = settle(e);
    if (r == 0)
    {
        e->ahead.n = 0;
        e->at = 0;
        uint64_t np;
        int k = os_file_seek(e->file, 0, (int64_t)pos, &np);
        r = k == OS_OK ? 0 : code(k);
    }
    plat_mutex_unlock(&table_lock);
    return r;
}

static int64_t file_len(uint64_t h)
{
    entry *e = find(h);
    if (!e)
    {
        return OTHER;
    }
    int64_t r = settle(e);
    if (r == 0)
    {
        uint64_t len;
        int is_dir;
        int k = os_file_stat(e->file, &len, &is_dir);
        r = k == OS_OK ? (int64_t)len : code(k);
    }
    plat_mutex_unlock(&table_lock);
    return r;
}

/* D-0181: the program's position: the system's, less what was read ahead
   and not yet taken. */
static int64_t file_position(uint64_t h)
{
    entry *e = find(h);
    if (!e)
    {
        return OTHER;
    }
    int64_t r = settle(e);
    if (r == 0)
    {
        uint64_t sys;
        int k = os_file_seek(e->file, 1, 0, &sys);
        r = k == OS_OK ? (int64_t)(sys - (uint64_t)(e->ahead.n - e->at)) : code(k);
    }
    plat_mutex_unlock(&table_lock);
    return r;
}

/* D-0181: everything written made durable (`fsync`). */
static int64_t file_flush(uint64_t h)
{
    entry *e = find(h);
    if (!e)
    {
        return OTHER;
    }
    int64_t r = settle(e);
    if (r == 0)
    {
        if (!e->writable)
        {
            r = FILEIO_BAD_HANDLE;
        }
        else
        {
            int k = os_file_sync(e->file);
            r = k == OS_OK ? 0 : code(k);
        }
    }
    plat_mutex_unlock(&table_lock);
    return r;
}

int64_t fileio_read(const uint8_t *path, size_t pn, cb_buf *out)
{
    out->n = 0;
    fileio_flush_all();
    if (!path_ok(path, pn))
    {
        return OTHER;
    }
    int k = os_read_file(path, pn, out);
    if (k != OS_OK)
    {
        out->n = 0;
        return code(k);
    }
    return 0;
}

int64_t fileio_write(const uint8_t *path, size_t pn, const uint8_t *d, size_t dn)
{
    fileio_flush_all();
    if (!path_ok(path, pn))
    {
        return OTHER;
    }
    int k = os_write_file(path, pn, d, dn);
    return k == OS_OK ? 0 : code(k);
}

/* `std::file_op(op, h, buf, n)`, the one primitive `File` is written over.
   `OPEN` and `WRITE` take the `n` bytes at `buf` as `input`
   (`fileio_takes_input`); `READ` puts its bytes at `buf`. The result is the
   operation's number (a handle, a count, a length, 0) or a failure code. */
#define OP_OPEN 0
#define OP_CLOSE 1
#define OP_DROP 2
#define OP_READ 3
#define OP_LINE_LEN 4
#define OP_WRITE 5
#define OP_SEEK 6
#define OP_LEN 7
#define OP_FLUSH 8    /* D-0181, through `file_op` */
#define OP_POSITION 9 /* D-0181, through `file_at` */

int fileio_takes_input(uint64_t op)
{
    return op == OP_OPEN || op == OP_WRITE;
}

int64_t fileio_call(uint64_t op, uint64_t h, const uint8_t *in, size_t inn, uint64_t n, cb_buf *out)
{
    out->n = 0;
    switch (op)
    {
    case OP_OPEN:
        return file_open(in, inn, h);
    case OP_CLOSE:
    case OP_DROP:
        return file_close(h, op == OP_CLOSE);
    case OP_READ:
        return read_some(h, n, out);
    case OP_LINE_LEN:
        return line_len(h);
    case OP_WRITE:
        return file_write_buffered(h, in, inn);
    case OP_SEEK:
        return file_seek(h, n);
    case OP_LEN:
        return file_len(h);
    case OP_FLUSH:
        return file_flush(h);
    case OP_POSITION:
        return file_position(h);
    default:
        return OTHER;
    }
}

/* ---- the filesystem, the environment and the clocks (D-0061) ----

   `std::fs_op(op, a, b)`: 0 `make_dir(a)` (1 created, 0 a directory was
   already there), 1 `remove_file(a)`, 2 `remove_dir(a)` (empty only), 3
   `rename(a, b)` (replacing a file at b), 4 a private directory (report
   224: mode 0700; 1 made, 0 anything already at the name). 0 or a failure
   code. */
int64_t fileio_fs_op(uint64_t op, const uint8_t *a, size_t an, const uint8_t *b, size_t bn)
{
    fileio_flush_all();
    if (!path_ok(a, an))
    {
        return OTHER;
    }
    int k;
    switch (op)
    {
    case 0:
        k = os_mkdir(a, an);
        if (k == OS_OK)
        {
            return 1;
        }
        if (k == OS_EXISTS)
        {
            return os_is_dir(a, an) ? 0 : OTHER;
        }
        return code(k);
    case 1:
        k = os_remove_file(a, an);
        return k == OS_OK ? 0 : code(k);
    case 2:
        k = os_remove_dir(a, an);
        return k == OS_OK ? 0 : code(k);
    case 3:
        if (!path_ok(b, bn))
        {
            return OTHER;
        }
        k = os_rename(a, an, b, bn);
        return k == OS_OK ? 0 : code(k);
    case 4:
        k = os_mkdir_private(a, an);
        if (k == OS_OK)
        {
            return 1;
        }
        return k == OS_EXISTS ? 0 : code(k);
    default:
        return OTHER;
    }
}

/* Byte order, a prefix first (`Vec<Vec<u8>>::sort`). */
static int name_cmp(const void *x, const void *y)
{
    const bstr *a = (const bstr *)x, *b = (const bstr *)y;
    size_t m = a->n < b->n ? a->n : b->n;
    int c = m ? memcmp(a->p, b->p, m) : 0;
    if (c)
    {
        return c;
    }
    return a->n < b->n ? -1 : a->n > b->n ? 1 : 0;
}

/* `std::fs_query(op, a)`: the bytes of an answer, or a failure code: 0
   `list_dir(a)`, its entries' names sorted by their bytes, each followed by
   a 0 byte; 1 `path_kind(a)`, a kind byte (0 file, 1 directory, 2 other)
   and the length as 8 little-endian bytes; 2 `env_var(a)`, the value's
   bytes (NOT_FOUND when unset); 3 `current_dir()`. Names and values are the
   platform's bytes (`into_encoded_bytes`), which `std` checks are UTF-8. */
int64_t fileio_fs_query(uint64_t op, const uint8_t *a, size_t an, cb_buf *out)
{
    out->n = 0;
    fileio_flush_all();
    int k;
    switch (op)
    {
    case 0:
    {
        if (!path_ok(a, an))
        {
            return OTHER;
        }
        cb_buf names = {0};
        size_t count = 0;
        k = os_list_dir(a, an, &names, &count);
        if (k != OS_OK)
        {
            cb_buf_free(&names);
            return code(k);
        }
        bstr *list = (bstr *)cb_xmalloc((count ? count : 1) * sizeof(bstr));
        size_t i = 0, at = 0;
        while (at < names.n && i < count)
        {
            const uint8_t *z = (const uint8_t *)memchr(names.p + at, 0, names.n - at);
            size_t len = (size_t)(z - (names.p + at));
            list[i].p = names.p + at;
            list[i].n = len;
            i++;
            at += len + 1;
        }
        qsort(list, i, sizeof(bstr), name_cmp);
        for (size_t j = 0; j < i; j++)
        {
            cb_buf_put(out, list[j].p, list[j].n);
            cb_buf_byte(out, 0);
        }
        free(list);
        cb_buf_free(&names);
        return 0;
    }
    case 1:
    {
        if (!path_ok(a, an))
        {
            return OTHER;
        }
        os_meta m;
        k = os_metadata(a, an, &m);
        if (k != OS_OK)
        {
            return code(k);
        }
        cb_buf_byte(out, (uint8_t)m.kind);
        for (int s = 0; s < 64; s += 8)
        {
            cb_buf_byte(out, (uint8_t)(m.len >> s));
        }
        return 0;
    }
    case 2:
        if (!path_ok(a, an))
        {
            return OTHER;
        }
        if (os_env_var(a, an, out))
        {
            return 0;
        }
        out->n = 0;
        return NOT_FOUND;
    case 3:
        k = os_current_dir(out);
        if (k != OS_OK)
        {
            out->n = 0;
            return code(k);
        }
        return 0;
    default:
        return OTHER;
    }
}

/* ---- paths and file metadata (D-0141) ----

   `std::path_op(op, h, a, b, out, cap)`: one or two paths in (`a`, `b`),
   the bytes of an answer out, or a number; `h` is unused. A path's grammar
   is the platform's (`std::path`): `C:\` and `\\server\share` on Windows.
   The `PATH_*` part functions and `PATH_NORMALIZE` never touch the file
   system. An absent answer (no parent, no extension, no home) is
   NOT_FOUND. */
#define PATH_JOIN 0
#define PATH_PARENT 1
#define PATH_FILE_NAME 2
#define PATH_STEM 3
#define PATH_EXTENSION 4
#define PATH_IS_ABSOLUTE 5
#define PATH_NORMALIZE 6
#define PATH_CANONICAL 7
#define FILE_INFO 8
#define COPY_FILE 9
#define MAKE_DIR_ALL 10
#define REMOVE_DIR_ALL 11
#define SET_CURRENT_DIR 12
#define TEMP_DIR 13
#define HOME_DIR 14
#define CURRENT_EXE 15      /* D-0181 */
#define HOSTNAME 16         /* D-0181 */
#define COPY_PERMISSIONS 17 /* D-0272 */
#define READ_LINK 18        /* report 096: a 1 byte and the target, or a 0 byte */

/* The 18-byte record `file_info` decodes: a kind byte (0 file, 1
   directory, 2 other), the length (8 bytes), the last modification in
   seconds since 1970 (8 bytes, rounded down), and a read-only byte; the
   numbers little-endian. */
static void info_record(const os_meta *m, cb_buf *out)
{
    cb_buf_byte(out, (uint8_t)m->kind);
    for (int s = 0; s < 64; s += 8)
    {
        cb_buf_byte(out, (uint8_t)(m->len >> s));
    }
    uint64_t mod = m->has_modified ? (uint64_t)m->mod_secs : 0;
    for (int s = 0; s < 64; s += 8)
    {
        cb_buf_byte(out, (uint8_t)(mod >> s));
    }
    cb_buf_byte(out, (uint8_t)(m->readonly ? 1 : 0));
}

/* `DirBuilder::create_dir_all`. */
static int create_dir_all(const uint8_t *p, size_t n)
{
    if (path_is_empty(p, n))
    {
        return OS_OK;
    }
    int k = os_mkdir(p, n);
    if (k == OS_OK)
    {
        return OS_OK;
    }
    if (k != OS_NOT_FOUND)
    {
        return os_is_dir(p, n) ? OS_OK : k;
    }
    bstr par;
    if (!path_parent(p, n, &par))
    {
        /* "failed to create whole tree" */
        return OS_OTHER;
    }
    k = create_dir_all(par.p, par.n);
    if (k != OS_OK)
    {
        return k;
    }
    k = os_mkdir(p, n);
    if (k == OS_OK || os_is_dir(p, n))
    {
        return OS_OK;
    }
    return k;
}

static int64_t some(int found, bstr s, cb_buf *out)
{
    if (!found)
    {
        return NOT_FOUND;
    }
    cb_buf_put(out, s.p, s.n);
    return (int64_t)s.n;
}

static int64_t path_op(uint64_t op, const uint8_t *a, size_t an, const uint8_t *b, size_t bn, cb_buf *out)
{
    fileio_flush_all();
    if (!path_ok(a, an))
    {
        return OTHER;
    }
    bstr s;
    int k;
    switch (op)
    {
    case PATH_JOIN:
        if (!path_ok(b, bn))
        {
            return OTHER;
        }
        cb_buf_put(out, a, an);
        path_push(out, b, bn);
        return (int64_t)out->n;
    case PATH_PARENT:
        k = path_parent(a, an, &s);
        return some(k, s, out);
    case PATH_FILE_NAME:
        k = path_file_name(a, an, &s);
        return some(k, s, out);
    case PATH_STEM:
        k = path_stem(a, an, &s);
        return some(k, s, out);
    case PATH_EXTENSION:
        k = path_extension(a, an, &s);
        return some(k, s, out);
    case PATH_IS_ABSOLUTE:
        return path_is_absolute(a, an);
    case PATH_NORMALIZE:
        normalize(a, an, out);
        return (int64_t)out->n;
    case PATH_CANONICAL:
        k = os_canonicalize(a, an, out);
        if (k != OS_OK)
        {
            out->n = 0;
            return code(k);
        }
        /* Windows answers in the verbatim form (`\\?\...`); the ordinary
           form names the same file and is what people write. */
        if (os_windows && out->n >= 4 && memcmp(out->p, "\\\\?\\", 4) == 0 && !(out->n >= 8 && memcmp(out->p + 4, "UNC\\", 4) == 0))
        {
            memmove(out->p, out->p + 4, out->n - 4);
            out->n -= 4;
            lossy(out);
        }
        return (int64_t)out->n;
    case FILE_INFO:
    {
        os_meta m;
        k = os_metadata(a, an, &m);
        if (k != OS_OK)
        {
            return code(k);
        }
        info_record(&m, out);
        return (int64_t)out->n;
    }
    case COPY_FILE:
    {
        /* D-0255: on Windows the count comes from the copy's size, not from
           `CopyFileEx`'s progress calls, which Wine does not make. */
        if (!path_ok(b, bn))
        {
            return OTHER;
        }
        uint64_t copied;
        k = os_copy_file(a, an, b, bn, &copied);
        return k == OS_OK ? (int64_t)copied : code(k);
    }
    case COPY_PERMISSIONS:
    {
        /* D-0272: the mode bits on Unix, the read-only flag on Windows. */
        os_meta m;
        k = os_metadata(a, an, &m);
        if (k != OS_OK)
        {
            return code(k);
        }
        if (!path_ok(b, bn))
        {
            return OTHER;
        }
        k = os_copy_permissions(a, an, b, bn);
        return k == OS_OK ? 0 : code(k);
    }
    case READ_LINK:
    {
        int is_link = 0;
        cb_buf t = {0};
        k = os_read_link(a, an, &is_link, &t);
        if (k != OS_OK)
        {
            cb_buf_free(&t);
            return code(k);
        }
        cb_buf_byte(out, (uint8_t)is_link);
        if (is_link)
        {
            cb_buf_put(out, t.p, t.n);
        }
        cb_buf_free(&t);
        lossy(out);
        return (int64_t)out->n;
    }
    case MAKE_DIR_ALL:
        k = create_dir_all(a, an);
        return k == OS_OK ? 0 : code(k);
    case REMOVE_DIR_ALL:
        k = os_remove_dir_all(a, an);
        return k == OS_OK ? 0 : code(k);
    case SET_CURRENT_DIR:
        k = os_set_current_dir(a, an);
        return k == OS_OK ? 0 : code(k);
    case TEMP_DIR:
        os_temp_dir(out);
        return (int64_t)out->n;
    case CURRENT_EXE:
        k = os_current_exe(out);
        if (k != OS_OK)
        {
            out->n = 0;
            return code(k);
        }
        return (int64_t)out->n;
    case HOSTNAME:
        k = os_hostname(out);
        if (k != OS_OK)
        {
            out->n = 0;
            return OTHER;
        }
        return (int64_t)out->n;
    case HOME_DIR:
    {
        int found = os_env_var((const uint8_t *)"HOME", 4, out) && out->n > 0;
        if (!found && os_windows)
        {
            found = os_env_var((const uint8_t *)"USERPROFILE", 11, out) && out->n > 0;
        }
        if (!found)
        {
            out->n = 0;
            return NOT_FOUND;
        }
        lossy(out);
        return (int64_t)out->n;
    }
    default:
        return OTHER;
    }
}

/* The shape every byte-answering primitive shares (`path_op`, and
   `proc_op`, `net_op` in their modules): a number, or a failure code, and
   the bytes of an answer, of which the caller copies up to `cap`. */
int64_t fileio_path_call(uint64_t op, uint64_t h, const uint8_t *a, size_t an, const uint8_t *b, size_t bn, uint64_t cap, cb_buf *out)
{
    (void)h;
    (void)cap;
    out->n = 0;
    int64_t r = path_op(op, a, an, b, bn, out);
    if (r < 0)
    {
        out->n = 0;
    }
    return r;
}

/* `std::sleep_ns(ns)` (D-0124): the calling thread sleeps at least `ns`
   nanoseconds. */
void fileio_sleep_ns(uint64_t ns)
{
    plat_sleep_ns(ns);
}

/* `std::clock_read(which)`: 0 a monotonic clock's nanoseconds (from an
   arbitrary start, the same for the whole program); 1 the wall clock's
   seconds since 1970-01-01 UTC (negative before it); 2 the wall clock's
   milliseconds since then (D-0140), rounded down. */
int64_t fileio_clock_read(uint64_t which)
{
    static plat_mutex start_lock = PLAT_MUTEX_INIT;
    static int started;
    static uint64_t start;
    if (which == 0)
    {
        if (!__atomic_load_n(&started, __ATOMIC_ACQUIRE))
        {
            plat_mutex_lock(&start_lock);
            if (!started)
            {
                start = plat_now_ns();
                __atomic_store_n(&started, 1, __ATOMIC_RELEASE);
            }
            plat_mutex_unlock(&start_lock);
        }
        uint64_t now = plat_now_ns();
        return (int64_t)(now >= start ? now - start : 0);
    }
    int64_t secs;
    uint32_t nanos;
    os_wall_time(&secs, &nanos);
    if (secs >= 0)
    {
        if (which == 2)
        {
            return secs * 1000 + (int64_t)(nanos / 1000000);
        }
        return secs;
    }
    /* Before 1970: the duration back to it, `e.duration()`. */
    cb_u128 d = (cb_u128)(0 - (uint64_t)secs) * 1000000000u - nanos;
    if (which == 2)
    {
        return -(int64_t)((d + 999999) / 1000000);
    }
    return -(int64_t)(d / 1000000000u);
}

/* ---- the machine's local time (D-0147) ----

   `std::tz_offset(unix)`: the seconds the machine's local time is ahead of
   UTC at that moment (negative when behind), daylight saving included, by
   the operating system's rule for the zone the program runs in. For a
   moment the platform has no rule for (before its records, beyond its
   range), the offset at the current moment, so a program never gets 0
   masquerading as UTC. */

static int64_t div_euclid(int64_t a, int64_t b)
{
    int64_t q = a / b;
    if (a % b < 0)
    {
        q = b > 0 ? q - 1 : q + 1;
    }
    return q;
}

/* Days since 1970-01-01 of a civil date (H. Hinnant's `days_from_civil`). */
static int64_t days_from_civil(int64_t y, int64_t m, int64_t d)
{
    y = m <= 2 ? y - 1 : y;
    int64_t era = div_euclid(y, 400);
    int64_t yoe = y - era * 400;
    int64_t mp = (m + 9) % 12;
    int64_t doy = (153 * mp + 2) / 5 + d - 1;
    int64_t doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    return era * 146097 + doe - 719468;
}

/* The seconds since 1970 of a broken-down local time, read as if it were
   UTC; its difference from the moment is the offset. */
static int64_t local_seconds(const int64_t tm[6])
{
    return days_from_civil(tm[0], tm[1], tm[2]) * 86400 + tm[3] * 3600 + tm[4] * 60 + tm[5];
}

static int platform_offset(int64_t t, int64_t *off)
{
    int64_t tm[6];
    if (!os_local_time(t, tm))
    {
        return 0;
    }
    return !__builtin_sub_overflow(local_seconds(tm), t, off);
}

int64_t fileio_tz_offset(int64_t t)
{
    int64_t o;
    if (!platform_offset(t, &o) && !platform_offset(fileio_clock_read(1), &o))
    {
        o = 0;
    }
    if (o < -64800)
    {
        o = -64800;
    }
    if (o > 64800)
    {
        o = 64800;
    }
    return o;
}

/* ---- standard input ----

   Rust's `std::io::stdin()`: one `BufReader` over the handle, shared by
   every reader of it (`io_stdin_read`, `fileio_read_line_chunk`); 8 KiB on
   POSIX, 12 KiB on Windows (`STDIN_BUF_SIZE`, room for UTF-16 read from a
   console as UTF-8). */

static plat_mutex stdin_lock = PLAT_MUTEX_INIT;
static uint8_t *stdin_buf;
static size_t stdin_pos, stdin_filled;

static size_t stdin_cap(void)
{
    return os_windows ? 12288 : 8192;
}

/* `fill_buf`: what is buffered, reading once when nothing is. */
static int stdin_fill(void)
{
    if (!stdin_buf)
    {
        stdin_buf = (uint8_t *)cb_xmalloc(stdin_cap());
    }
    if (stdin_pos >= stdin_filled)
    {
        if (stdin_wait_cancelled())
        {
            return STDIN_CANCELLED;
        }
        size_t got = 0;
        int k = os_stdin_read(stdin_buf, stdin_cap(), &got);
        if (k != OS_OK)
        {
            return k;
        }
        stdin_pos = 0;
        stdin_filled = got;
    }
    return OS_OK;
}

/* Reports 323, 324 (`[Print-Buffer]`): standard output's buffer is written
   out before a read of standard input that waits for the operating system
   (nothing buffered), and before every read when standard output is a
   terminal -- not before each line already buffered, which made a line
   filter one `write` a line. Called with `stdin_lock` held; when it writes,
   it gives the lock up for the write (which may end the program, whose
   destructors may read standard input) and returns 1, and the caller looks
   at the buffer again. */
static int stdin_flush_first(int *flushed)
{
    if (*flushed || (stdin_pos < stdin_filled && !outbuf_tty()))
    {
        return 0;
    }
    *flushed = 1;
    plat_mutex_unlock(&stdin_lock);
    stdout_before_wait();
    plat_mutex_lock(&stdin_lock);
    return 1;
}

/* `std::stdin_read`: `Read::read` of the locked `BufReader`, retried
   when interrupted; a read as large as the buffer, with nothing buffered,
   bypasses it. */
int64_t io_stdin_read(uint8_t *buf, size_t n)
{
    int flushed = 0;
    plat_mutex_lock(&stdin_lock);
    for (;;)
    {
        if (stdin_flush_first(&flushed))
        {
            continue;
        }
        int k;
        if (stdin_pos >= stdin_filled && n >= stdin_cap())
        {
            stdin_pos = 0;
            stdin_filled = 0;
            if (stdin_wait_cancelled())
            {
                plat_mutex_unlock(&stdin_lock);
                return STDIN_CANCELLED;
            }
            size_t got = 0;
            k = os_stdin_read(buf, n, &got);
            if (k == OS_OK)
            {
                plat_mutex_unlock(&stdin_lock);
                return (int64_t)got;
            }
        }
        else
        {
            k = stdin_fill();
            if (k == OS_OK)
            {
                size_t avail = stdin_filled - stdin_pos;
                size_t m = avail < n ? avail : n;
                if (m)
                {
                    memcpy(buf, stdin_buf + stdin_pos, m);
                }
                stdin_pos += m;
                plat_mutex_unlock(&stdin_lock);
                return (int64_t)m;
            }
        }
        if (k == OS_INTERRUPTED)
        {
            continue;
        }
        plat_mutex_unlock(&stdin_lock);
        return k == STDIN_CANCELLED ? STDIN_CANCELLED : -1;
    }
}

/* `std::stdin_read_line` (`read_line`'s primitive): standard input's next
   bytes up to and including the next `\n`, at most `n` of them, from the
   same buffered `stdin` every other read of it uses; 0 at its end, -1 on an
   error. Standard output's buffer is written out before the wait
   (D-0178, `stdin_flush_first`), so a prompt is on the screen. */
int64_t fileio_read_line_chunk(uint8_t *buf, size_t n)
{
    if (n == 0)
    {
        return 0;
    }
    int flushed = 0;
    plat_mutex_lock(&stdin_lock);
    for (;;)
    {
        if (stdin_flush_first(&flushed))
        {
            continue;
        }
        int k = stdin_fill();
        if (k == OS_INTERRUPTED)
        {
            continue;
        }
        if (k != OS_OK)
        {
            plat_mutex_unlock(&stdin_lock);
            return k == STDIN_CANCELLED ? STDIN_CANCELLED : -1;
        }
        size_t avail = stdin_filled - stdin_pos;
        if (avail == 0)
        {
            plat_mutex_unlock(&stdin_lock);
            return 0;
        }
        const uint8_t *at = (const uint8_t *)memchr(stdin_buf + stdin_pos, '\n', avail);
        size_t m = at ? (size_t)(at - (stdin_buf + stdin_pos)) + 1 : avail;
        if (m > n)
        {
            m = n;
        }
        memcpy(buf, stdin_buf + stdin_pos, m);
        stdin_pos += m;
        plat_mutex_unlock(&stdin_lock);
        return (int64_t)m;
    }
}
