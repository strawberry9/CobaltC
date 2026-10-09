"""Guide §21's listing of every `std` module and every item it declares,
read from the reference implementation's source when the page is built:
`impl/std/**/*.cb` for the declarations (an item with `export` is a
program's to use, any other is the module's own), and `impl/src/modres.rs`
and `impl/src/typecheck.rs` for the items both tools build in, which have
no CobaltC body. Nothing here is written by hand, so the listing cannot
fall behind `std`."""
import html
import os
import re

R = os.path.dirname(os.path.dirname(os.path.dirname(os.path.abspath(__file__))))
STD = os.path.join(R, "impl", "std")


def inventory_html():
    FN = re.compile(r'^(export\s+)?(unsafe\s+)?(extern\s+)?fn\s+([A-Za-z_]\w*(?:::[A-Za-z_]\w*)?)')
    TY = re.compile(r'^(export\s+)?(resource\s+|quiet\s+)?(struct|enum|bitstruct)\s+([A-Za-z_]\w*)')
    CO = re.compile(r'^(export\s+)?const\s+.*?([A-Za-z_]\w*)\s*=')
    MOD = re.compile(r'^export\s+module\s+(\w+)\s+"([^"]+)"')
    IMP = re.compile(r'^export\s+import\s+([\w:]+)\s*;')

    def parse(path):
        items, seen, mods, reex = [], set(), [], []
        for line in open(path, encoding='utf-8'):
            m = FN.match(line)
            if m:
                it = ('extern' if m.group(3) else 'fn', m.group(4), bool(m.group(1)), False)
            elif TY.match(line):
                m = TY.match(line)
                it = ('type', m.group(4), bool(m.group(1)), False)
            elif CO.match(line):
                m = CO.match(line)
                it = ('const', m.group(2), bool(m.group(1)), False)
            elif MOD.match(line):
                mods.append(MOD.match(line).groups())
                continue
            elif IMP.match(line):
                reex.append(IMP.match(line).group(1))
                continue
            else:
                continue
            if it[:2] not in seen:
                seen.add(it[:2])
                items.append(it)
        return items, mods, reex

    # Modules in declaration order, each child after its parent.
    order = []
    def walk(path, file_rel):
        items, mods, reex = parse(os.path.join(STD, *file_rel.split('/')))
        order.append({'path': path, 'file': file_rel, 'items': items, 'mods': [m for m, _ in mods], 'reex': reex})
        base = file_rel.rpartition('/')[0]
        for name, f in mods:
            walk(path + '::' + name, base + '/' + f if base else f)
    walk('std', 'std.cb')
    bypath = {m['path']: m for m in order}

    # The natives: std items both tools realize without a CobaltC body.
    modres = open(os.path.join(R, 'impl', 'src', 'modres.rs'), encoding='utf-8').read()
    tc = open(os.path.join(R, 'impl', 'src', 'typecheck.rs'), encoding='utf-8').read()
    for mod, name, exp in re.findall(r'native\("([\w:]+)", "([\w:]+)", (true|false)\)', modres):
        bypath[mod]['items'].append(('fn', name, exp == 'true', True))
    floats = []
    for arr in ('FLOAT_FNS', 'FLOAT2_FNS'):
        body = re.search(r'pub const %s: &\[&str\] = &\[(.*?)\];' % arr, tc, re.S).group(1)
        floats += re.findall(r'"std::(\w+)"', body)
    for f in floats:
        bypath['std::math']['items'].append(('fn', f, True, True))

    def key(n):
        return n.lower()

    def code(name, exported, native):
        cls = '' if exported else ' class="priv"'
        mark = '<sup class="nat">n</sup>' if native else ''
        return '<code%s>%s</code>%s' % (cls, html.escape(name), mark)

    def row(label, cells):
        return '<tr><td>%s</td><td>%s</td></tr>' % (label, ', '.join(cells))

    out = []
    nav = ', '.join('<a href="#stdmod-%s"><code>%s</code></a>' % (m['path'].replace('::', '-'), m['path']) for m in order)
    out.append('<p class="std-nav">%s</p>' % nav)
    total = {'exp': 0, 'priv': 0}
    for m in order:
        items = m['items']
        types = [i for i in items if i[0] == 'type']
        typenames = [t[1] for t in types]
        free = [i for i in items if i[0] == 'fn' and '::' not in i[1]]
        assoc = {}
        for i in items:
            if i[0] == 'fn' and '::' in i[1]:
                t, f = i[1].split('::')
                assoc.setdefault(t, []).append(i)
                if t not in typenames:
                    typenames.append(t)
        consts = [i for i in items if i[0] == 'const']
        externs = [i for i in items if i[0] == 'extern']
        for i in items:
            total['exp' if i[2] else 'priv'] += 1
        anchor = m['path'].replace('::', '-')
        out.append('<h4 id="stdmod-%s"><code>%s</code> <span class="std-file">impl/std/%s</span></h4>' % (anchor, m['path'], m['file']))
        rows = []
        if m['mods']:
            cells = []
            for s in m['mods']:
                child = m['path'] + '::' + s
                exp = child in m['reex']
                cells.append('<a href="#stdmod-%s">%s</a>' % (child.replace('::', '-'), code(s, exp, False)))
            rows.append(row('Submodules', cells))
        if types:
            rows.append(row('Types', [code(t[1], t[2], False) for t in sorted(types, key=lambda t: key(t[1]))]))
        if free:
            rows.append(row('Functions', [code(f[1], f[2], f[3]) for f in sorted(free, key=lambda f: key(f[1]))]))
        for t in typenames:
            if t in assoc:
                fs = sorted(assoc[t], key=lambda f: key(f[1]))
                rows.append(row('<code>%s::</code>' % html.escape(t), [code(f[1].split('::')[1], f[2], f[3]) for f in fs]))
        if consts:
            rows.append(row('Constants', [code(c[1], c[2], False) for c in sorted(consts, key=lambda c: key(c[1]))]))
        if externs:
            rows.append(row('Externs (<code>unsafe</code>)', [code(e[1], e[2], False) for e in sorted(externs, key=lambda e: key(e[1]))]))
        if not rows:
            out.append('<p>No items: the reference implementation declares nothing here.</p>')
            continue
        out.append('<table class="std-inv">')
        out.append('<thead><tr><th>Kind</th><th>Items</th></tr></thead>')
        out.append('<tbody>')
        out.extend(rows)
        out.append('</tbody>')
        out.append('</table>')
    return '\n'.join(out) + '\n'
