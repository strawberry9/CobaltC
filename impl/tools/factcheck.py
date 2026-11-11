#!/usr/bin/env python3
"""A second reading of the static rules, checking the compiler's certificate (D-0299 step 2).

`cobc --certificate` (step 1) says, for every body the static pass checked, whether it is *decided*: no conflict
of its frame-held references is left to the run, so a conforming compiler keeps no record of them (D-0284,
D-0292). This tool reads `rule.control.flow-analysis` and `rule.control.frame-references` (`spec/14` §6) again,
from the text, over the program's syntax (`coby --dump-ast`, the canonical text the two parsers share), and
for each body the certificate calls decided derives the body's facts itself and looks at every access, borrow
and call the body makes. A body the certificate calls decided where this reading finds a conflict the run would
have to check (a fact `?`, a may-overlap, a point check at a call) or one the rules refute is the finding
`factcheck.decided-but-left`: one of the two readings, or the text, is wrong.

It shares no code with the static pass and keeps its own names for bindings (each declaration is its own
binding; a mention resolves to the one in scope where it is written). It computes the facts as the text says
(a forward dataflow over the syntactic CFG, joined at merges, loops iterated until the head stops moving: there
is no bound on the rounds, and a path a walk down a structure lengthens is cut at eight steps as a `?` on its
prefix, which overlaps all it stood for). It does not take the static pass's facts on trust.

It reads a core of the language. A body outside it is reported `unchecked` with the reason (the first
construct outside the core), and is no evidence either way. The core: locals of scalar, struct, array and plain
standard-library container types; references and slices to them; `let`, assignment, `if`, `while` (and the
`for`/`foreach` forms the parser writes as `while`), blocks, `return`, `break`, `continue`; borrows of places
(fields, literal and computed indices, dereferences); calls to the program's own functions and to `std`'s
(whose summaries are `{ε}`, the text's rule for `std`), with the call-site rule and its access summaries; the
block's-end rule; liveness by last use. Outside it for now: closures, threads, mutexes and channels, `match`,
values that keep references in data (a struct with a reference field, `Option` of a reference, a view), raw
pointers and `unsafe`, the evaluation-order rule's forms (an assignment through an element whose value takes
the container exclusively), moving out through a reference.

    python3 impl/tools/factcheck.py PATH... [--coby PATH] [--cobc PATH] [--out DIR] [--check] [-v]
    python3 impl/tools/factcheck.py --cert CERT FILE        # a certificate written elsewhere

PATH is a `.cb` file or a directory. For each program `cobc --certificate … --emit-c` writes the certificate
(nothing is compiled). `--check` exits 1 on a finding. Standard library only.
"""
import os
import re
import subprocess
import sys

ROOT = os.path.dirname(os.path.dirname(os.path.dirname(os.path.abspath(__file__))))
PATH_CAP = 8


class Outside(Exception):
    """The body uses a construct outside the core this reading covers."""


# ---------------------------------------------------------------- the canonical text

def parse_sexpr(text):
    toks = re.findall(r"\(|\)|[^\s()]+", text)
    out, stack = [], [[]]
    for t in toks:
        if t == "(":
            stack.append([])
        elif t == ")":
            done = stack.pop()
            stack[-1].append(done)
        else:
            stack[-1].append(t)
    return stack[0]


def pos(atom):
    m = re.match(r"^@(\d+):(\d+)$", atom) if isinstance(atom, str) else None
    return (int(m.group(1)), int(m.group(2))) if m else None


# ---------------------------------------------------------------- types

class Ty:
    """`kind`: 'ref' (a reference or a slice: `mode`, `inner`), 'plain' (no reference anywhere), 'data' (keeps
    references in data: outside the core), 'unknown'."""

    def __init__(self, kind, mode=None, inner=None, text=""):
        self.kind, self.mode, self.inner, self.text = kind, mode, inner, text


def type_of(t, structs):
    """A type of the canonical text."""
    if isinstance(t, str):
        if t in ("auto",):
            return Ty("unknown", text=t)
        if t in structs:
            return struct_ty(t, structs)
        return Ty("plain", text=t)
    head = t[0] if t else ""
    if head in ("ref", "slice"):
        inner = type_of(t[1], structs)
        return Ty("ref", mode=t[2], inner=inner, text="%s" % head)
    if head == "array":
        e = type_of(t[1], structs)
        return Ty("data" if e.kind in ("ref", "data") else ("unknown" if e.kind == "unknown" else "plain"), text="array")
    if head == "named":
        name = t[1]
        args = [type_of(a, structs) for a in (t[2] if len(t) > 2 and isinstance(t[2], list) else [])]
        if name in structs and not args:
            return struct_ty(name, structs)
        if name in ("StringView", "std::StringView", "Option", "Result", "std::Option", "std::Result") and (name.endswith("StringView") or any(a.kind in ("ref", "data") for a in args)):
            return Ty("data", text=name)
        if any(a.kind in ("ref", "data") for a in args):
            return Ty("data", text=name)
        if any(a.kind == "unknown" for a in args):
            return Ty("unknown", text=name)
        if name in structs:
            return struct_ty(name, structs)
        return Ty("plain", text=name)
    if head in ("fn", "handle", "mutex", "guard", "rawptr", "simd"):
        return Ty("data" if head in ("fn", "handle", "guard") else "plain", text=head)
    return Ty("unknown", text=str(head))


def struct_ty(name, structs, seen=()):
    if name in seen:
        return Ty("plain", text=name)
    for ft in structs[name]:
        k = type_of(ft, {n: v for n, v in structs.items() if n not in seen + (name,)}) if not isinstance(ft, str) or ft not in structs else struct_ty(ft, structs, seen + (name,))
        if k.kind in ("ref", "data"):
            return Ty("data", text=name)
        if k.kind == "unknown":
            return Ty("unknown", text=name)
    return Ty("plain", text=name)


# ---------------------------------------------------------------- facts

def overlap(p, q):
    """(certain, may): whether two projections overlap, step by step (spec/14 §6): fields equal; literal
    indices equal; a computed index meets every index (may)."""
    certain = True
    for a, b in zip(p, q):
        if "r" in (a[0], b[0]):
            # A slice's range [lo, hi) (D-0070): it meets a literal index inside it, or a literal range it
            # intersects; with a computed bound only the values say.
            ra, rb = (a, b) if a[0] == "r" else (b, a)
            lo, hi = ra[1], ra[2]
            if rb[0] == "r":
                if None in (lo, hi, rb[1], rb[2]):
                    certain = False
                elif max(lo, rb[1]) >= min(hi, rb[2]):
                    return (False, False)
                continue
            if None in (lo, hi, rb[1]):
                certain = False
            elif not (lo <= rb[1] < hi):
                return (False, False)
            continue
        if a[0] != b[0]:
            return (False, True) if "elem" in (a[0], b[0]) else (False, False)
        if a[0] == "f":
            if a[1] != b[1]:
                return (False, False)
        else:
            if a[1] is None or b[1] is None:
                certain = False
            elif a[1] != b[1]:
                return (False, False)
    return (certain, True)


def permitted(m1, m2):
    return m1 == "shared" and m2 == "shared"


def join_val(a, b):
    if a is None:
        return b
    if b is None:
        return a
    return a if a == b else "?"


def cut(key):
    root, path, mode = key
    if len(path) > PATH_CAP:
        return (root, path[:PATH_CAP], mode), True
    return key, False


class State:
    """The facts at a program point: `deriv[h]` maps (root, path, mode) to 'T' or '?' (an absent fact is
    `F`); `valid[b]` for every owner and reference declared; `unreachable` after `return`/`break`."""

    def __init__(self):
        self.deriv = {}
        self.valid = {}
        self.unreachable = False

    def copy(self):
        s = State()
        s.deriv = {h: dict(f) for h, f in self.deriv.items()}
        s.valid = dict(self.valid)
        s.unreachable = self.unreachable
        return s

    def join(self, o):
        if self.unreachable:
            return o.copy()
        if o.unreachable:
            return self.copy()
        s = State()
        for b in set(self.valid) & set(o.valid):
            s.valid[b] = join_val(self.valid[b], o.valid[b])
        for h in set(self.deriv) | set(o.deriv):
            if h not in s.valid:
                continue
            fa, fb = self.deriv.get(h, {}), o.deriv.get(h, {})
            m = {}
            for k in set(fa) | set(fb):
                va, vb = fa.get(k), fb.get(k)
                m[k] = va if va == vb else "?"
            if m:
                s.deriv[h] = m
        return s

    def widen(self):
        for h in list(self.deriv):
            m = {}
            for k, v in self.deriv[h].items():
                nk, was = cut(k)
                m[nk] = "?" if was or (nk in m and m[nk] != v) else v
            self.deriv[h] = m
        return self

    def same(self, o):
        return self.unreachable == o.unreachable and self.valid == o.valid and {h: f for h, f in self.deriv.items() if f} == {h: f for h, f in o.deriv.items() if f}


# ---------------------------------------------------------------- one body

STD_PLAIN_RESULT = re.compile(r"^(Vec::(len|is_empty|capacity|pop|push|insert|remove|clear|truncate|reserve|new|filled|with_capacity|sort|swap|contains)|"
                              r"String::(len|is_empty|new|copy_str|push_str|append|clear|clone|eq|eq_str)|HashMap::(new|len|insert|contains_key|remove)|"
                              r"HashSet::(new|len|insert|contains|remove)|narrow|widen|min_value|max_value|abs|min|max|print|printf|eprintf|assert|"
                              r"drop|clone|\$each_range|read_line|arg_count|flush_stdout|exit)$")


class Body:
    def __init__(self, prog, fn):
        self.prog, self.fn = prog, fn
        self.name = fn["name"]
        self.findings = []        # (kind, line, text): kind 'left' or 'refuted'
        self.bind_types = {}      # binding id -> Ty
        self.decls = {}           # binding id -> (decl position, end position)
        self.mentions = []        # (binding id, position)
        self.loops = []           # (first position, last position)
        self.params = []
        self.scope = [{}]

    # -- names
    def declare(self, name, p, ty):
        bid = "%s@%d:%d" % (name, p[0], p[1])
        self.scope[-1][name] = bid
        self.bind_types[bid] = ty
        return bid

    def lookup(self, name):
        for sc in reversed(self.scope):
            if name in sc:
                return sc[name]
        return None


def node_pos(n):
    if isinstance(n, list):
        for a in n[1:3]:
            p = pos(a)
            if p:
                return p
    return None


def max_pos(n, cur=(0, 0)):
    if isinstance(n, list):
        for a in n:
            cur = max_pos(a, cur)
        return cur
    p = pos(n)
    return max(cur, p) if p else cur


class Program:
    def __init__(self, items):
        self.structs, self.fns = {}, {}
        for it in items:
            if not isinstance(it, list) or not it:
                continue
            if it[0] == "struct":
                self.structs[it[3]] = [f[2] for f in it if isinstance(f, list) and f and f[0] == "field"]
            elif it[0] == "fn":
                params = it[6] if isinstance(it[6], list) else []
                body = next((x for x in it if isinstance(x, list) and x and x[0] == "block"), None)
                self.fns[it[3]] = {"name": it[3], "params": [(p[0], p[1]) for p in params], "ret": it[7], "body": body, "unsafe": "unsafe" in str(body)}
        self.summaries = {}


def walk_names(n, out):
    if isinstance(n, list):
        if n and n[0] == "path" and len(n) >= 3 and pos(n[1]):
            out.append((n[2], pos(n[1])))
        for a in n:
            walk_names(a, out)


class Checker:
    """One body, read as the text says. `run()` returns ('decided' | 'left' | 'refuted', [reasons]) or raises
    `Outside`."""

    def __init__(self, prog, fn):
        self.prog, self.fn = prog, fn
        self.types = {}
        self.decl_at = {}
        self.uses = []          # (bid, pos)
        self.loops = []
        self.scope = [{}]
        self.reasons = []
        self.refuted = []
        self.walked = False
        self.ref_formed = {}

    # ---------------- names and liveness (a pre-pass resolves every mention)
    def bid_of(self, name, at=None):
        for sc in reversed(self.scope):
            if name in sc:
                return sc[name]
        return None

    def ty(self, t):
        return type_of(t, self.prog.structs)

    def live(self, bid, at, depth=0):
        """`bid` is mentioned after `at`, or anywhere in a loop enclosing `at` that its declaration precedes,
        or another live binding holds a fact rooted at it or its referent."""
        if depth > 6:
            return True
        decl = self.decl_at.get(bid, (0, 0))
        loops = [(a, b) for (a, b) in self.loops if a <= at <= b and decl < a]
        for (u, p) in self.uses:
            if u == bid and (p > at or any(a <= p <= b for a, b in loops)):
                return True
        for h, facts in self.cur.deriv.items():
            if h != bid and any(k[0] in (bid, "*" + bid) for k in facts) and self.live(h, at, depth + 1):
                return True
        return False

    # ---------------- places
    def place(self, e):
        """(root, path) of a place expression: root a binding id (an owner) or '*' + a reference's id."""
        k = e[0]
        if k == "paren":
            return self.place(e[-1])
        if k == "path":
            b = self.bid_of(e[2])
            if b is None:
                return ("<static>", ())
            if self.types[b].kind == "ref":
                return (b, ())  # the reference binding itself, as a place (assigning it)
            return (b, ())
        if k == "field":
            r, p = self.place_through(e[3])
            return (r, p + (("f", e[2]),))
        if k == "index":
            r, p = self.place_through(e[2])
            idx = e[3]
            lit = int(idx[2]) if isinstance(idx, list) and idx[0] == "int" else None
            if self.root_is_vec(r):
                return (r, p + (("elem", lit),))
            return (r, p + (("i", lit),))
        if k == "deref":
            inner = e[2]
            if inner[0] == "path":
                b = self.bid_of(inner[2])
                if b is not None and self.types[b].kind == "ref":
                    return ("*" + b, ())
            raise Outside("a dereference of an expression that is no reference binding")
        raise Outside("a place of the form `%s`" % k)

    def place_through(self, e):
        """The base of a field or an index: a reference binding's field goes through it."""
        if e[0] == "path":
            b = self.bid_of(e[2])
            if b is not None and self.types[b].kind == "ref":
                return ("*" + b, ())
        return self.place(e)

    def root_is_vec(self, root):
        b = root[1:] if root.startswith("*") else root
        t = self.types.get(b)
        return t is not None and t.text in ("Vec", "String", "std::Vec")

    # ---------------- the discharge
    def access(self, root, path, mode, at, through=None, args=()):
        """An access at (root, path) in `mode`: every live holder other than the one it is made through (and
        a call's own arguments) with a fact meeting it in a mode not permitted is a conflict: refuted when the
        fact is `T` and the overlap certain, otherwise left to the run."""
        if not self.walked or self.cur.unreachable or root == "<static>":
            return
        excl = set(args)
        if through:
            excl.add(through)
        for h, facts in self.cur.deriv.items():
            if h in excl:
                continue
            for (kr, kp, km), v in facts.items():
                if kr != root or permitted(mode, km):
                    continue
                certain, may = overlap(path, kp)
                if not may:
                    continue
                if not self.live(h, at):
                    continue
                if v == "T" and certain:
                    self.refuted.append("line %d: an access to `%s` meets `%s`'s borrow" % (at[0], show(root, path), short(h)))
                else:
                    self.reasons.append("line %d: an access to `%s` that `%s` may reach" % (at[0], show(root, path), short(h)))

    # ---------------- the inherited facts of an expression (for a reference binding)
    def facts_of(self, e, mode=None):
        k = e[0]
        if k == "paren":
            return self.facts_of(e[-1], mode)
        if k == "borrow" or k == "sliceof":
            m = e[2]
            r, p = self.place(e[3])
            if k == "sliceof":
                p = p + (range_step(e),)
            return {(r, p, m): "T"}
        if k == "path":
            b = self.bid_of(e[2])
            if b is None:
                return {}
            t = self.types[b]
            if t.kind == "ref":
                own = self.cur.deriv.get(b)
                if own:
                    return dict(own)
                return {("*" + b, (), t.mode): "T"}
            return {}
        if k == "call":
            return self.call_result(e)
        if k == "if":
            then = self.block_value(e[3])
            other = self.block_value(e[4]) if len(e) > 4 else {}
            out = {}
            for kk in set(then) | set(other):
                out[kk] = "?"
            return out
        if k in ("int", "bool", "str", "float", "unit"):
            return {}
        raise Outside("a reference made from `%s`" % k)

    def block_value(self, b):
        raise Outside("a reference given by a branch's value")

    def call_target(self, e):
        callee = e[2]
        if callee[0] != "path":
            raise Outside("a call through a `fn` value")
        return callee[2]

    def param_kinds(self, name, nargs):
        fn = self.prog.fns.get(name)
        if fn is not None:
            return [self.ty(t) for (t, _) in fn["params"]]
        return None

    def arg_reach(self, a, pk):
        """W(a): what an argument reaches (spec/14 §6, the call-site rule)."""
        k = a[0]
        if k in ("borrow", "sliceof"):
            r, p = self.place(a[3])
            if k == "sliceof":
                p = p + (range_step(a),)
            return {(r, p, a[2]): "T"}
        if k == "path":
            b = self.bid_of(a[2])
            if b is not None and self.types[b].kind == "ref":
                out = dict(self.cur.deriv.get(b, {}))
                out[("*" + b, (), self.types[b].mode)] = "T"
                return out
        return {}

    def call_result(self, e):
        name = self.call_target(e)
        args = e[3:]
        fn = self.prog.fns.get(name)
        ref_args = []
        for i, a in enumerate(args):
            if fn is not None:
                if i < len(fn["params"]) and self.ty(fn["params"][i][0]).kind == "ref":
                    ref_args.append(a)
            elif a[0] in ("borrow", "sliceof") or (a[0] == "path" and self.bid_of(a[2]) and self.types[self.bid_of(a[2])].kind == "ref"):
                ref_args.append(a)
        if not ref_args:
            return {}
        reach = [self.arg_reach(a, None) for a in ref_args]
        if len(reach) == 1:
            return _strip_own(reach[0], ref_args[0], self)
        out = {}
        for r in reach:
            for kk in r:
                out[kk] = "?"
        return out

    # ---------------- the call-site rule
    def call_site(self, e, at):
        name = self.call_target(e)
        args = e[3:]
        fn = self.prog.fns.get(name)
        if fn is None and not STD_PLAIN_RESULT.match(name.split("::", 1)[-1] if name.startswith("std::") else name) and not name.startswith("$"):
            # A `std` function: its summaries are {ε} for every reference parameter.
            pass
        formed = set()
        keys = []
        same = []   # the reference binding each argument passes whole, or None
        for i, a in enumerate(args):
            if fn is not None and i < len(fn["params"]):
                pty = self.ty(fn["params"][i][0])
                if pty.kind != "ref":
                    if pty.kind in ("data", "unknown"):
                        raise Outside("a call passing a value that may keep references")
                    continue
                pm = pty.mode
                pname = fn["params"][i][1]
            else:
                if a[0] in ("borrow", "sliceof"):
                    pm = a[2]
                elif a[0] == "path" and self.bid_of(a[2]) and self.types[self.bid_of(a[2])].kind == "ref":
                    pm = self.types[self.bid_of(a[2])].mode
                else:
                    continue
                pname = None
            w = self.arg_reach(a, None)
            same.append(self.bid_of(a[2]) if a[0] == "path" else None)
            if a[0] == "path":
                formed.add(self.bid_of(a[2]))
            elif a[0] in ("borrow", "sliceof"):
                r, _ = self.place(a[3])
                formed.add(r[1:] if r.startswith("*") else r)
            summ = self.prog.summaries.get(name, {}).get(pname) if pname is not None and fn is not None else None
            if summ is None:
                reads, writes = {()}, ({()} if pm == "exclusive" else set())
            else:
                reads, writes = summ
            e_keys = {}
            for (r, p, m), v in w.items():
                for q in reads:
                    e_keys[(r, p + q, "shared")] = v
                for q in writes:
                    e_keys[(r, p + q, "exclusive" if pm == "exclusive" else "shared")] = v
                e_keys.setdefault((r, p, m if pm == "exclusive" else "shared"), v)
            keys.append((e_keys, w))
        # rule 1: against the live bindings other than the arguments' own
        for e_keys, _ in keys:
            for (r, p, m), v in e_keys.items():
                self.access_key(r, p, m, v, at, formed)
        # rule 2: one argument's effect against another's reach
        for i, (ei, _) in enumerate(keys):
            for j, (_, wj) in enumerate(keys):
                # One reference binding passed twice is one path; `swap`'s and `swap_places`'s two
                # arguments, of one type, are one place or disjoint (`[Swap-Places]`, spec/14 §6).
                if i == j or (same[i] is not None and same[i] == same[j]) or name.split("::")[-1] in ("swap", "swap_places"):
                    continue
                for (r1, p1, m1), v1 in ei.items():
                    for (r2, p2, m2), v2 in wj.items():
                        if r1 != r2 or permitted(m1, m2):
                            continue
                        certain, may = overlap(p1, p2)
                        if not may:
                            continue
                        if certain and v1 == "T" and v2 == "T":
                            self.refuted.append("line %d: two arguments of a call to `%s` meet at `%s`" % (at[0], name, show(r1, p1)))
                        else:
                            self.reasons.append("line %d: two arguments of a call to `%s` may meet at `%s` (a point check)" % (at[0], name, show(r1, p1)))

    def access_key(self, r, p, m, v, at, formed):
        if not self.walked or self.cur.unreachable:
            return
        for h, facts in self.cur.deriv.items():
            if h in formed:
                continue
            for (kr, kp, km), fv in facts.items():
                if kr != r or permitted(m, km):
                    continue
                certain, may = overlap(p, kp)
                if not may or not self.live(h, at):
                    continue
                if certain and fv == "T" and v == "T":
                    self.refuted.append("line %d: a call's argument meets `%s`'s borrow at `%s`" % (at[0], short(h), show(r, p)))
                else:
                    self.reasons.append("line %d: a call's argument may meet `%s`'s borrow at `%s`" % (at[0], short(h), show(r, p)))

    # ---------------- expressions: their accesses, in evaluation order
    def expr(self, e, ctx="value"):
        if not isinstance(e, list) or not e:
            return
        k = e[0]
        at = node_pos(e) or (0, 0)
        if k in ("int", "bool", "str", "float", "unit"):
            return
        if k == "paren":
            return self.expr(e[-1], ctx)
        if k == "path":
            b = self.bid_of(e[2])
            if b is None:
                return
            t = self.types[b]
            if t.kind == "ref":
                return  # reading a reference binding's value is no access to its referent
            self.access(b, (), "shared" if ctx != "move" else "exclusive", at)
            return
        if k in ("field", "index", "deref"):
            self.read_place(e, at)
            return
        if k in ("borrow", "sliceof"):
            self.borrow_place(e, at)
            return
        if k == "binary":
            self.expr(e[3])
            self.expr(e[4])
            return
        if k == "unary":
            self.expr(e[3])
            return
        if k == "assign":
            self.assign(e, at)
            return
        if k == "call":
            name = self.call_target(e)
            for a in e[3:]:
                if a[0] in ("borrow", "sliceof"):
                    self.indices(a[3])
                elif a[0] == "path" and self.bid_of(a[2]) and self.types[self.bid_of(a[2])].kind == "ref":
                    pass
                else:
                    self.expr(a, "move")
            if name == "drop" and e[3][0] == "path":
                b = self.bid_of(e[3][2])
                if b is not None:
                    self.end_binding(b, at)
                return
            self.call_site(e, at)
            return
        if k == "structlit":
            for s in e[4:]:
                if isinstance(s, list) and s and s[0] == "set":
                    self.expr(s[2], "move")
            return
        if k == "arraylit":
            for s in e[2:]:
                self.expr(s, "move")
            return
        if k == "if":
            self.if_(e, at)
            return
        if k == "blockexpr":
            self.block(e[2])
            return
        if k == "while":
            self.while_(e, at)
            return
        if k == "return":
            for a in e[2:]:
                self.expr(a, "move")
            self.cur.unreachable = True
            return
        if k in ("break", "continue"):
            if self.loop_ctx:
                (self.loop_ctx[-1]["breaks"] if k == "break" else self.loop_ctx[-1]["continues"]).append(self.cur.copy())
            self.cur.unreachable = True
            return
        raise Outside("an expression of the form `%s`" % k)

    def indices(self, place):
        """The index expressions inside a place, evaluated before the access."""
        if not isinstance(place, list):
            return
        if place[0] == "index":
            self.indices(place[2])
            self.expr(place[3])
        elif place[0] in ("field", "deref", "paren"):
            self.indices(place[-1] if place[0] != "field" else place[3])

    def read_place(self, e, at):
        self.indices(e)
        r, p = self.place(e)
        through = r[1:] if r.startswith("*") else None
        self.access(r, p, "shared", at, through=through)

    def borrow_place(self, e, at):
        self.indices(e[3])
        if e[0] == "sliceof":
            for b in e[4:6]:
                self.expr(b)
        r, p = self.place(e[3])
        if e[0] == "sliceof":
            p = p + (range_step(e),)
        through = r[1:] if r.startswith("*") else None
        self.access(r, p, e[2], at, through=through)

    def assign(self, e, at):
        lhs, rhs = e[2], e[3]
        if lhs[0] == "path":
            b = self.bid_of(lhs[2])
            if b is not None and self.types[b].kind == "ref":
                self.expr_for_ref(rhs)
                self.set_ref(b, self.facts_of_assigned(b, rhs), at)
                return
        self.expr(rhs, "move")
        if lhs[0] == "index" and self.has_call(rhs):
            raise Outside("an assignment through an element whose value makes a call (the evaluation-order rule)")
        self.indices(lhs)
        r, p = self.place(lhs)
        through = r[1:] if r.startswith("*") else None
        if lhs[0] == "path" and r in self.cur.valid:
            self.cur.valid[r] = "T"
        self.access(r, p, "exclusive", at, through=through)

    def has_call(self, e):
        return "call" in str(e)

    def expr_for_ref(self, e):
        """The accesses an expression makes while forming a reference (its borrow, a call's arguments)."""
        if e[0] == "path":
            # A reference read whole (a copy) while another live binding holds a fact through its referent
            # that does not permit its mode: the copy reaches `*r` under no fact (spec/14 §6, "Slices and
            # copies"), so the run decides.
            b = self.bid_of(e[2])
            if b is not None and self.types[b].kind == "ref" and self.walked and not self.cur.unreachable:
                at = node_pos(e)
                for h, facts in self.cur.deriv.items():
                    if h != b and any(k[0] == "*" + b and not permitted(self.types[b].mode, k[2]) for k in facts) and self.live(h, at):
                        self.reasons.append("line %d: `%s` copied while `%s` holds its referent" % (at[0], short(b), short(h)))
            return
        if e[0] in ("borrow", "sliceof"):
            self.borrow_place(e, node_pos(e))
        elif e[0] == "call":
            self.expr(e)
        elif e[0] == "paren":
            self.expr_for_ref(e[-1])
        elif e[0] == "if":
            raise Outside("a reference given by a branch's value")

    def facts_of_assigned(self, b, rhs):
        """`r = e`: a fact of `e` rooted at `*r` stands for what `r` held before (CHG-0384)."""
        f = self.facts_of(rhs)
        old = self.cur.deriv.get(b, {})
        out = {}
        for (r, p, m), v in f.items():
            if r == "*" + b:
                if old:
                    for (orr, op, om), ov in old.items():
                        out[(orr, op + p, "shared" if m == "shared" else om)] = "?" if (v != "T" or ov != "T") else "T"
                else:
                    out[(r, p, m)] = v
            else:
                out[(r, p, m)] = v
        return out

    def set_ref(self, b, facts, at):
        """A reference binding gets new facts: what other bindings held through its old referent is handed
        to what it held (D-0291 at its end; D-0300 at an assignment, each `?`), then its own are replaced."""
        self.hand_over(b, sure=False)
        self.cur.deriv[b] = {}
        for k, v in facts.items():
            nk, was = cut(k)
            self.cur.deriv[b][nk] = "?" if was else v
        if not self.cur.deriv[b]:
            del self.cur.deriv[b]

    def hand_over(self, b, sure=True):
        own = self.cur.deriv.get(b, {})
        rr = "*" + b
        for h in list(self.cur.deriv):
            if h == b:
                continue
            facts = self.cur.deriv[h]
            through = [(k, v) for k, v in facts.items() if k[0] == rr]
            for k, v in through:
                del facts[k]
                for (orr, op, om), ov in own.items():
                    nk = (orr, op + k[1], "shared" if k[2] == "shared" else om)
                    nk, was = cut(nk)
                    nv = "T" if (sure and v == "T" and ov == "T" and not was) else "?"
                    facts[nk] = nv if nk not in facts or facts[nk] == nv else "?"

    def end_binding(self, b, at):
        """A binding ends (a block's end, `drop`): held facts through it are handed over or end; a reference
        declared outside that holds a fact rooted at it and is live after is the block's-end rule's."""
        if self.types.get(b) and self.types[b].kind == "ref":
            self.hand_over(b)
        self.cur.deriv.pop(b, None)
        for h in list(self.cur.deriv):
            facts = self.cur.deriv[h]
            for k in [k for k in facts if k[0] in (b, "*" + b)]:
                if not k[0].startswith("*") and self.walked and self.live(h, at):
                    if facts[k] == "T":
                        self.refuted.append("line %d: `%s` outlives `%s`, which it borrows" % (at[0], short(h), short(b)))
                    else:
                        self.reasons.append("line %d: `%s` may outlive `%s`, which it may borrow" % (at[0], short(h), short(b)))
                del facts[k]
            if not facts:
                del self.cur.deriv[h]
        self.cur.valid.pop(b, None)

    # ---------------- statements
    def let(self, s, at):
        t = s[1]
        name = s[2]
        init = s[4] if len(s) > 4 else None
        ty = self.ty(t)
        if ty.kind == "unknown" and init is not None:
            ty = self.infer(init)
        if ty.kind == "data":
            raise Outside("a binding that keeps references in data (`%s`)" % ty.text)
        if ty.kind == "unknown":
            raise Outside("a binding of a type this reading does not follow (`%s`)" % (ty.text or t))
        if init is not None:
            if ty.kind == "ref":
                self.expr_for_ref(init)
                facts = self.facts_of(init)
            else:
                self.expr(init, "move")
        bid = "%s@%d:%d" % (name, at[0], at[1])
        self.scope[-1][name] = bid
        self.types[bid] = ty
        self.cur.valid[bid] = "T"
        if init is not None and ty.kind == "ref":
            self.cur.deriv.pop(bid, None)
            self.set_ref(bid, facts, at)

    def infer(self, e):
        k = e[0]
        if k == "paren":
            return self.infer(e[-1])
        if k in ("borrow", "sliceof"):
            inner = Ty("plain")
            return Ty("ref", mode=e[2], inner=inner, text="ref")
        if k in ("int", "bool", "float", "binary", "unary"):
            return Ty("plain")
        if k == "str":
            return Ty("plain", text="str")
        if k == "path":
            b = self.bid_of(e[2])
            if b is not None:
                return self.types[b]
            return Ty("unknown", text=e[2])
        if k == "call":
            name = self.call_target(e)
            fn = self.prog.fns.get(name)
            if fn is not None:
                return self.ty(fn["ret"])
            base = name[5:] if name.startswith("std::") else name
            if STD_PLAIN_RESULT.match(base) or base.startswith("$"):
                return Ty("plain", text=base.split("::")[0])
            return Ty("unknown", text=name)
        if k == "structlit":
            return self.ty(["named", e[2], []]) if e[2] in self.prog.structs else Ty("unknown", text=e[2])
        if k == "arraylit":
            return Ty("plain")
        return Ty("unknown", text=k)

    def stmt(self, s):
        k = s[0]
        if self.cur.unreachable:
            return
        if k == "let":
            self.let(s, pos(s[3]) or (0, 0))
        elif k == "expr":
            self.expr(s[1])
        elif k in ("blocklike", "tail"):
            self.expr(s[1])
        else:
            raise Outside("a statement of the form `%s`" % k)

    def block(self, b):
        self.scope.append({})
        for s in b[1:]:
            self.stmt(s)
        sc = self.scope.pop()
        at = max_pos(b)
        for name, bid in reversed(list(sc.items())):
            self.end_binding(bid, at)

    def if_(self, e, at):
        self.expr(e[2])
        before = self.cur.copy()
        self.block_or_expr(e[3])
        then = self.cur
        self.cur = before.copy()
        if len(e) > 4:
            self.block_or_expr(e[4])
        self.cur = then.join(self.cur)

    def block_or_expr(self, b):
        if b[0] == "block":
            self.block(b)
        else:
            self.expr(b)

    def while_(self, e, at):
        cond, body = e[2], e[3]
        step = e[4] if len(e) > 4 else None
        entry = self.cur.copy()
        head = entry.copy()
        saved_walk = self.walked
        rounds = 0
        while True:
            self.walked = False
            self.cur = head.copy()
            self.loop_ctx.append({"breaks": [], "continues": []})
            self.expr(cond)
            self.block(body)
            ctx = self.loop_ctx.pop()
            for c in ctx["continues"]:
                self.cur = self.cur.join(c)
            if step is not None:
                self.block_or_expr(step)
            new = entry.join(self.cur)
            rounds += 1
            if new.same(head):
                break
            head = head.join(new).widen() if rounds > 16 else new
            if rounds > 10000:
                raise Outside("a loop whose facts did not settle")
        self.walked = saved_walk
        self.cur = head.copy()
        self.loop_ctx.append({"breaks": [], "continues": []})
        self.expr(cond)
        after_cond = self.cur.copy()
        self.block(body)
        ctx = self.loop_ctx.pop()
        if step is not None:
            for c in ctx["continues"]:
                self.cur = self.cur.join(c)
            self.block_or_expr(step)
        exit_ = after_cond
        if cond[0] == "bool" and cond[2] == "true":
            exit_.unreachable = True
        for b in ctx["breaks"]:
            exit_ = exit_.join(b)
        self.cur = exit_

    # ---------------- the body
    def prepass(self, n, scope):
        """Resolve every mention to its binding; record loops' extents and declarations."""
        if not isinstance(n, list) or not n:
            return
        k = n[0]
        if k == "block":
            scope.append({})
            for s in n[1:]:
                self.prepass(s, scope)
            scope.pop()
            return
        if k == "let":
            if len(n) > 4:
                self.prepass(n[4], scope)
            p = pos(n[3])
            bid = "%s@%d:%d" % (n[2], p[0], p[1])
            scope[-1][n[2]] = bid
            self.decl_at[bid] = p
            return
        if k == "while":
            self.loops.append((node_pos(n), max_pos(n)))
        if k == "path" and len(n) >= 3 and pos(n[1]):
            for sc in reversed(scope):
                if n[2] in sc:
                    self.uses.append((sc[n[2]], pos(n[1])))
                    break
        if k == "closure":
            raise Outside("a closure")
        if k == "match":
            raise Outside("a `match`")
        if k == "unsafe":
            raise Outside("an `unsafe` block")
        if k == "destructure":
            raise Outside("a destructuring `let`")
        for a in n[1:]:
            self.prepass(a, scope)

    def run(self):
        fn = self.fn
        if fn["body"] is None:
            raise Outside("no body")
        if fn["unsafe"]:
            raise Outside("an `unsafe` block")
        params = {}
        for (t, name) in fn["params"]:
            ty = self.ty(t)
            if ty.kind in ("data", "unknown"):
                raise Outside("a parameter that may keep references in data (`%s`)" % name)
            bid = "%s@param" % name
            params[name] = bid
            self.types[bid] = ty
            self.decl_at[bid] = (0, 0)
        self.prepass(fn["body"], [dict(params)])
        self.scope = [dict(params)]
        self.cur = State()
        for bid in params.values():
            self.cur.valid[bid] = "T"
        self.loop_ctx = []
        self.walked = True
        self.block(fn["body"])
        if self.refuted:
            return "refuted", self.refuted
        if self.reasons:
            return "left", self.reasons
        return "decided", []


def _strip_own(reach, arg, ck):
    """A call's result holds the facts of its reference argument: for a reference binding passed whole,
    what it holds (its referent `*q` when it holds nothing)."""
    if arg[0] == "path":
        b = ck.bid_of(arg[2])
        own = ck.cur.deriv.get(b)
        if own:
            return dict(own)
        return {("*" + b, (), ck.types[b].mode): "T"}
    return reach


def range_step(e):
    """A slice's range step `[lo, hi)`, its bounds when they are literals."""
    lit = lambda x: int(x[2]) if isinstance(x, list) and x and x[0] == "int" else None
    return ("r", lit(e[4]) if len(e) > 4 else None, lit(e[5]) if len(e) > 5 else None)


def short(bid):
    return bid.split("@")[0]


def show(root, path):
    s = ("*" + short(root[1:])) if root.startswith("*") else short(root)
    for st in path:
        if st[0] == "f":
            s += "." + st[1]
        else:
            s += "[%s]" % ("?" if st[1] is None else st[1])
    return s


# ---------------------------------------------------------------- summaries

def summaries(prog):
    """`reads(F, p)` and `writes(F, p)` of every reference parameter, to a fixed point over the program
    (spec/14 §6): a read or a shared borrow rooted in `p` at `π`; a write or an exclusive borrow; passing on."""
    out = {name: {p: (set(), set()) for (t, p) in fn["params"] if type_of(t, prog.structs).kind == "ref"} for name, fn in prog.fns.items()}
    for name, fn in prog.fns.items():
        if fn["unsafe"]:
            for p in out[name]:
                out[name][p] = ({()}, {()})
    changed = True
    rounds = 0
    while changed and rounds < 100:
        changed = False
        rounds += 1
        for name, fn in prog.fns.items():
            if not out[name] or fn["body"] is None:
                continue
            new = {p: (set(r), set(w)) for p, (r, w) in out[name].items()}
            scan(prog, fn["body"], new, out, False)
            for p in new:
                if new[p] != out[name][p]:
                    out[name][p] = new[p]
                    changed = True
    return out


def proj_of(e, params):
    """A place rooted in a parameter: (param, projection cut at the first index)."""
    k = e[0]
    if k == "paren":
        return proj_of(e[-1], params)
    if k == "path":
        return (e[2], ()) if e[2] in params else None
    if k == "deref":
        return proj_of(e[2], params)
    if k == "field":
        r = proj_of(e[3], params)
        return (r[0], r[1] + (("f", e[2]),)) if r and not getattr(r, "cut", False) else r
    if k == "index":
        r = proj_of(e[2], params)
        return r
    return None


def scan(prog, n, acc, out, write):
    if not isinstance(n, list) or not n:
        return
    k = n[0]
    params = acc
    if k == "assign":
        r = proj_of(n[2], params)
        if r:
            acc[r[0]][1].add(r[1])
        scan(prog, n[3], acc, out, False)
        for a in n[2][1:]:
            if isinstance(a, list):
                scan(prog, a, acc, out, False)
        return
    if k == "borrow" or k == "sliceof":
        r = proj_of(n[3], params)
        if r:
            (acc[r[0]][1] if n[2] == "exclusive" else acc[r[0]][0]).add(r[1])
        return
    if k == "call" and n[2][0] == "path":
        callee = n[2][2]
        cfn = prog.fns.get(callee)
        for i, a in enumerate(n[3:]):
            r = proj_of(a[3], params) if a[0] in ("borrow", "sliceof") else proj_of(a, params)
            if r:
                if cfn is not None and i < len(cfn["params"]) and cfn["params"][i][1] in out.get(callee, {}):
                    rr, ww = out[callee][cfn["params"][i][1]]
                    acc[r[0]][0].update(r[1] + q for q in rr)
                    acc[r[0]][1].update(r[1] + q for q in ww)
                else:
                    acc[r[0]][0].add(r[1])
                    if a[0] == "borrow" and a[2] == "exclusive":
                        acc[r[0]][1].add(r[1])
            scan(prog, a, acc, out, False)
        return
    if k in ("path", "deref", "field", "index"):
        r = proj_of(n, params)
        if r:
            acc[r[0]][0].add(r[1])
            if k != "path":
                for a in n[1:]:
                    if isinstance(a, list) and a and a[0] == "index":
                        scan(prog, a, acc, out, False)
            return
    for a in n[1:]:
        scan(prog, a, acc, out, False)


# ---------------------------------------------------------------- a program

def read_cert(path):
    verdict, bodies = None, {}
    for line in open(path, encoding="utf-8"):
        line = line.rstrip("\n")
        if line.startswith("verdict: "):
            verdict = line[9:]
        m = re.match(r"^fn (.*): (decided|left|rejected)$", line)
        if m:
            bodies[m.group(1)] = m.group(2)
    return verdict, bodies


def check_program(cb, cert, coby):
    """[(body, cert status, our verdict or 'unchecked', reasons)]"""
    d = subprocess.run([coby, "--dump-ast", os.path.basename(cb)], cwd=os.path.dirname(os.path.abspath(cb)), capture_output=True, text=True, timeout=120)
    if d.returncode != 0:
        return None
    prog = Program(parse_sexpr(d.stdout))
    prog.summaries = summaries(prog)
    verdict, bodies = read_cert(cert)
    if verdict is None or verdict.startswith("rejected"):
        return []
    out = []
    for key, st in sorted(bodies.items()):
        if key.startswith("std::") or "<" in key or "::" in key:
            continue
        fn = prog.fns.get(key)
        if fn is None:
            continue
        try:
            got, why = Checker(prog, fn).run()
        except Outside as e:
            got, why = "unchecked", [str(e)]
        except (IndexError, KeyError, TypeError, AttributeError) as e:
            got, why = "unchecked", ["the reading could not follow this body (%s)" % type(e).__name__]
        out.append((key, st, got, why))
    return out


def programs(paths):
    for p in paths:
        if os.path.isdir(p):
            for d, _, fs in sorted(os.walk(p)):
                for f in sorted(fs):
                    if f.endswith(".cb"):
                        yield os.path.join(d, f)
        else:
            yield p


def main():
    args = sys.argv[1:]
    coby = os.path.join(ROOT, "impl", "target", "release", "coby")
    cobc = os.path.join(ROOT, "impl", "target", "release", "cobc")
    out = None
    check = verbose = False
    paths = []
    cert_only = None
    i = 0
    while i < len(args):
        a = args[i]
        if a == "--coby":
            coby = os.path.abspath(args[i + 1])
            i += 2
        elif a == "--cobc":
            cobc = os.path.abspath(args[i + 1])
            i += 2
        elif a == "--out":
            out = os.path.abspath(args[i + 1])
            i += 2
        elif a == "--cert":
            cert_only = os.path.abspath(args[i + 1])
            i += 2
        elif a == "--check":
            check = True
            i += 1
        elif a in ("-v", "--verbose"):
            verbose = True
            i += 1
        else:
            paths.append(a)
            i += 1
    if not paths:
        print(__doc__)
        return 2
    if out is None:
        out = os.path.join(ROOT, "stress", "verify", "factcheck")
    os.makedirs(out, exist_ok=True)
    tally = {}
    findings = 0
    reasons_out = {}
    for prog in programs(paths):
        if cert_only:
            cert = cert_only
        else:
            cert = os.path.join(out, re.sub(r"[^A-Za-z0-9_.-]", "_", os.path.relpath(os.path.abspath(prog), ROOT))[:-3] + ".cert")
            if os.path.exists(cert):
                os.remove(cert)
            subprocess.run([cobc, "--certificate", cert, "--emit-c", "-o", os.devnull, os.path.basename(prog)], cwd=os.path.dirname(os.path.abspath(prog)), capture_output=True, text=True, timeout=300)
            if not os.path.exists(cert):
                tally["no certificate"] = tally.get("no certificate", 0) + 1
                continue
        res = check_program(prog, cert, coby)
        if res is None:
            tally["no syntax"] = tally.get("no syntax", 0) + 1
            continue
        for key, st, got, why in res:
            tag = "%s/%s" % (st, got)
            tally[tag] = tally.get(tag, 0) + 1
            if st == "decided" and got in ("left", "refuted"):
                findings += 1
                print("factcheck: %s: `%s`: factcheck.decided-but-%s: %s" % (prog, key, got, "; ".join(why[:3])))
            elif got == "unchecked":
                r = why[0] if why else "?"
                reasons_out[r] = reasons_out.get(r, 0) + 1
                if verbose:
                    print("factcheck: %s: `%s`: unchecked: %s" % (prog, key, r))
            elif verbose and st != got:
                print("factcheck: %s: `%s`: the certificate says %s, this reading %s: %s" % (prog, key, st, got, "; ".join(why[:2])))
    agreed = sum(v for k, v in tally.items() if k in ("decided/decided", "left/left"))
    checked = sum(v for k, v in tally.items() if not k.endswith("/unchecked") and "/" in k)
    print("factcheck: %d bodies read: %d checked (%d agree; certificate decided and this reading decided %d; the certificate left a conflict and this reading found none %d; the certificate decided and this reading did not %d), %d unchecked"
          % (sum(v for k, v in tally.items() if "/" in k), checked, agreed, tally.get("decided/decided", 0), tally.get("left/decided", 0), findings, sum(v for k, v in tally.items() if k.endswith("/unchecked"))))
    if verbose:
        for r, n in sorted(reasons_out.items(), key=lambda kv: -kv[1])[:15]:
            print("factcheck:   unchecked %d: %s" % (n, r))
    return 1 if (check and findings) else 0


if __name__ == "__main__":
    sys.exit(main())
