#!/usr/bin/env python3
"""Git merge driver for LAWS.bend and PROOF.bend: merge by block, not by line.

`merge=union` aligns the many identical lines (`  {==}`, blank lines) of
different laws and proofs, and drops a law's `{==}` line or splits a proof def
from its body when two branches both append after the same last block. Here a
block is a top-level line (law, def, import...) with the comments and blank
lines before it and the indented lines after it, keyed by its header
(`law NAME`, `def NAME`). Per key: a side that changed it wins over one that
did not; blocks a side added are kept (the other side's go after them, at the
same place); both sides changing one block differently is a conflict.

Usage (git): merge-laws.py BASE OURS THEIRS   -> merged result in OURS,
exit 0 clean, 1 conflict (OURS then holds the markers).
Install with scripts/setup-merge-driver.sh.
"""
import re
import sys


def parse(text):
    """-> list of [key, text] in file order; the tail after the last block
    (comments, blanks) is the block keyed <eof>."""
    out, prefix, seen = [], [], {}
    cur = None
    lines = text.split('\n')
    if lines and lines[-1] == '':
        lines.pop()
    for l in lines:
        top = l and l[0] not in ' #\t'
        if top:
            m = re.match(r'(law|def)\s+([^\s(:]+)', l)
            k = f'{m.group(1)} {m.group(2)}' if m else l.strip()
            seen[k] = seen.get(k, 0) + 1
            if seen[k] > 1:
                k += f'#{seen[k]}'
            cur = [k, prefix + [l]]
            prefix = []
            out.append(cur)
        elif cur is not None and l.startswith((' ', '\t')):
            cur[1].append(l)
        else:
            prefix.append(l)
            cur = None
    out.append(['<eof>', prefix])
    return [[k, '\n'.join(v) + '\n' if v else ''] for k, v in out]


def merge(base, ours, theirs):
    b = dict(parse(base))
    o, t = parse(ours), parse(theirs)
    od, td = dict(o), dict(t)
    conflict = False
    res = []  # [key, text]
    for k, ot in o:
        if k == '<eof>':
            continue
        tt = td.get(k)
        if k in b:
            if tt is None:  # theirs deleted it
                if ot == b[k]:
                    continue
                res.append([k, ot]); continue
            if ot == tt or tt == b[k]:
                res.append([k, ot])
            elif ot == b[k]:
                res.append([k, tt])
            else:
                conflict = True
                res.append([k, f'<<<<<<< ours\n{ot}=======\n{tt}>>>>>>> theirs\n'])
        else:
            res.append([k, ot])
    # blocks only theirs added: after their predecessor, behind any run of
    # blocks only ours added there
    keys = lambda: [r[0] for r in res]
    prev = None
    for k, tt in t:
        if k == '<eof>':
            continue
        if k not in od and k not in b:
            ks = keys()
            i = ks.index(prev) + 1 if prev in ks else 0
            while i < len(ks) and ks[i] not in b and ks[i] not in td:
                i += 1
            res.insert(i, [k, tt])
        prev = k
    # theirs changed a block ours deleted
    for k, tt in t:
        if k in b and k not in od and tt != b[k] and k != '<eof>':
            conflict = True
            res.append([k, f'<<<<<<< ours\n=======\n{tt}>>>>>>> theirs\n'])
    eo, et, eb = od['<eof>'], td['<eof>'], b['<eof>']
    eof = eo if et == eb or eo == et else et if eo == eb else None
    if eof is None:
        conflict = True
        eof = f'<<<<<<< ours\n{eo}=======\n{et}>>>>>>> theirs\n'
    parts = []
    for _, tx in res:
        # a block joined behind one with no blank line gets one
        if parts and not parts[-1].endswith('\n\n') and not tx.startswith(('\n', '#')):
            parts[-1] += '\n'
        parts.append(tx)
    if parts and eof and not parts[-1].endswith('\n\n') and not eof.startswith(('\n', '#')):
        parts[-1] += '\n'
    parts.append(eof)
    return ''.join(parts), conflict


def main(argv):
    if len(argv) < 4:
        print(__doc__, file=sys.stderr)
        return 2
    base, ours, theirs = (open(p).read() for p in argv[1:4])
    out, conflict = merge(base, ours, theirs)
    open(argv[2], 'w').write(out)
    return 1 if conflict else 0


if __name__ == '__main__':
    sys.exit(main(sys.argv))
