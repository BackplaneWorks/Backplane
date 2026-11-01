#!/usr/bin/env python3
"""Structural check of LAWS.bend and PROOF.bend, no Bend needed (< 1 s).

A merge or rebase of these two files can drop a law's `{... == ...}` line or
split a proof def from its body, and `scripts/check.sh` finds out only after
the emit. This catches that at once:

  - no conflict markers;
  - every `law NAME:` in LAWS.bend has a `{... == ...}` statement line;
  - every law has a `def Laws.NAME` in PROOF.bend with a body, and every
    `def Laws.NAME` there has its law;
  - no name is declared twice; no `{==}` line stands alone at the top level.

Usage: scripts/laws-shape.py [LAWS.bend PROOF.bend]
"""
import re
import sys

MARK = re.compile(r'^(<<<<<<<|=======|>>>>>>>|\|\|\|\|\|\|\|)( |$)')
START = re.compile(r'^\s+\{')


def has_stmt(body):
    """A statement `{ a == b : T }`, possibly over several lines: a line
    opening with `{`, a `==` from there on, and the block ending on `}`."""
    lines = [x for x in body if x.strip()]
    for i, x in enumerate(lines):
        if START.match(x):
            rest = '\n'.join(lines[i:])
            return '==' in rest and lines[-1].rstrip().endswith('}')
    return False

FIX = """To repair after a bad merge: take main's file (git checkout origin/main -- FILE)
and re-append your block (the law, or the proof def with its body). Do not
resolve LAWS.bend/PROOF.bend by hand-editing around lost lines.
scripts/setup-merge-driver.sh installs a merge driver that merges by block."""


def blocks(text):
    """(line, header, body lines) per top-level chunk; # comments at column 0
    and blank lines end the one before them."""
    out, cur = [], None
    for n, l in enumerate(text.split('\n'), 1):
        if l and l[0] not in ' #':
            cur = [n, l, []]
            out.append(cur)
        elif l.startswith('#'):
            cur = None
        elif cur is not None:
            cur[2].append(l)
    return out


def check(laws_text, proof_text, lname='LAWS.bend', pname='PROOF.bend'):
    errs = []
    for name, text in ((lname, laws_text), (pname, proof_text)):
        for n, l in enumerate(text.split('\n'), 1):
            if MARK.match(l):
                errs.append(f'{name}:{n}: conflict marker {l[:7]}')
    laws = {}
    for n, h, b in blocks(laws_text):
        m = re.match(r'law ([A-Za-z0-9_.]+):\s*$', h)
        if not m:
            if h.startswith('law '):
                errs.append(f'{lname}:{n}: malformed law header: {h}')
            continue
        k = m.group(1)
        if k in laws:
            errs.append(f'{lname}:{n}: law {k} declared twice (first at {laws[k]})')
        laws[k] = n
        if not has_stmt(b):
            errs.append(f'{lname}:{n}: law {k} has no {{... == ...}} line')
    proofs = {}
    for n, h, b in blocks(proof_text):
        if h.startswith('{'):
            errs.append(f'{pname}:{n}: stray top-level `{h[:20]}` (split from its def?)')
            continue
        m = re.match(r'def Laws\.([A-Za-z0-9_.]+)\(.*$', h)
        if not m:
            continue
        k = m.group(1)
        if k in proofs:
            errs.append(f'{pname}:{n}: proof {k} defined twice (first at {proofs[k]})')
        proofs[k] = n
        inline = h.split(':', 1)[1].strip() if h.rstrip().endswith('}') or not h.rstrip().endswith(':') else ''
        if not any(x.strip() for x in b) and not inline:
            errs.append(f'{pname}:{n}: proof {k} has no body')
    for k, n in laws.items():
        if k not in proofs:
            errs.append(f'{lname}:{n}: law {k} has no `def Laws.{k}` in {pname}')
    for k, n in proofs.items():
        if k not in laws:
            errs.append(f'{pname}:{n}: `def Laws.{k}` has no law in {lname}')
    return errs


def main(argv):
    lp, pp = (argv[1], argv[2]) if len(argv) == 3 else ('LAWS.bend', 'PROOF.bend')
    errs = check(open(lp).read(), open(pp).read(), lp, pp)
    for e in errs[:40]:
        print(e, file=sys.stderr)
    if len(errs) > 40:
        print(f'... and {len(errs) - 40} more', file=sys.stderr)
    if errs:
        print(FIX, file=sys.stderr)
        return 1
    return 0


if __name__ == '__main__':
    sys.exit(main(sys.argv))
