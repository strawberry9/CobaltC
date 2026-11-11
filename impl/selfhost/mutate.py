#!/usr/bin/env python3
# Mutated copies of a CobaltC file for compare_parser.sh: a line deleted, a
# character deleted, a stray character inserted, each at a few positions
# chosen from the file's own hash, so a run is repeatable.
#
#   mutate.py FILE OUTDIR    writes OUTDIR/<n>.cb and prints each path
import hashlib, os, sys

path, out = sys.argv[1], sys.argv[2]
data = open(path, 'rb').read()
seed = int(hashlib.sha256(data).hexdigest()[:16], 16)
def pick(k, n):
    return (seed >> (k * 7)) % n if n else 0
lines = data.split(b'\n')
variants = []
for k in range(2):
    if len(lines) > 2:
        i = pick(k, len(lines) - 1)
        variants.append(b'\n'.join(lines[:i] + lines[i + 1:]))
for k in range(2, 4):
    if data:
        i = pick(k, len(data))
        variants.append(data[:i] + data[i + 1:])
for k, ch in zip(range(4, 7), [b'(', b'}', b';']):
    if data:
        i = pick(k, len(data))
        variants.append(data[:i] + ch + data[i:])
os.makedirs(out, exist_ok=True)
for n, v in enumerate(variants):
    p = os.path.join(out, '%d.cb' % n)
    open(p, 'wb').write(v)
    print(p)
