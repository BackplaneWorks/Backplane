#!/usr/bin/env python3
"""Declaration-level slices of canonical modules (prototype, own file).

A shard is a set of root declarations. It keeps those and every declaration they
reach by name in LAWS.bend, PROOF.bend and the imported modules; each declaration
is copied byte for byte and nothing else is edited except dropping imports that
no kept declaration uses. A name that is dropped by mistake makes the compiler
fail (never a silent pass). Coverage is a separate, exact set computation:
`coverage()` lists every canonical declaration no shard keeps.
"""
import re
from pathlib import Path

HEAD = re.compile(r'^(def|type|law) ([A-Za-z_][A-Za-z_0-9.]*)(?=[\s(:<])')
IMPORT = re.compile(r'^import (Base|(?:\./|\.\./)[\w./-]+\.bend)(?: as ([A-Za-z_]\w*))?(?:\s*#.*)?$')
LEX = re.compile(r'"(?:\\.|[^"\\])*"|\'(?:\\.|[^\'\\])*\'|#[^\n]*|([A-Za-z_][A-Za-z_0-9]*(?:\.[A-Za-z_][A-Za-z_0-9]*)*)')
CTOR = re.compile(r'^\s+([A-Z][A-Za-z_0-9]*)\s*\{')


def scc(edges):
    """Strongly connected components of {node: set(nodes)}; returns {node: representative}."""
    index, low, onstack, stack, comp, count = {}, {}, set(), [], {}, [0]
    for start in sorted(edges):
        if start in index:
            continue
        work = [(start, iter(sorted(edges[start])))]
        index[start] = low[start] = count[0]; count[0] += 1
        stack.append(start); onstack.add(start)
        while work:
            node, it = work[-1]
            for w in it:
                if w not in index:
                    index[w] = low[w] = count[0]; count[0] += 1
                    stack.append(w); onstack.add(w)
                    work.append((w, iter(sorted(edges.get(w, ())))))
                    break
                if w in onstack:
                    low[node] = min(low[node], index[w])
            else:
                work.pop()
                if work:
                    low[work[-1][0]] = min(low[work[-1][0]], low[node])
                if low[node] == index[node]:
                    members = []
                    while True:
                        w = stack.pop(); onstack.discard(w); members.append(w)
                        if w == node:
                            break
                    for w in members:
                        comp[w] = min(members)
    return comp


def verify_output(out, expected):
    """Independent check of what was written: a plain line scanner (not Module) follows the
    entry file's imports on disk. Every emitted module must be reachable, every import must
    resolve to an emitted file, no top-level line may be unaccounted for, and the declarations
    found must equal `expected` exactly."""
    out = Path(out)
    head = re.compile(r'^(def|type|law) (\S+?)[\s(:<]')
    imp = re.compile(r'^import (\S+)(?: as (\w+))?\s*(?:#.*)?$')
    emitted = {p.relative_to(out/'t').as_posix() for p in (out/'t').rglob('*') if p.is_file()}
    if any(not n.endswith('.bend') for n in emitted):
        raise ValueError('non-module file in shard tree')
    seen, found, pending = set(), set(), [out/'PROOF.bend']
    while pending:
        path = pending.pop()
        rel = path.relative_to(out/'t').as_posix() if path != out/'PROOF.bend' else None
        if rel in seen:
            continue
        seen.add(rel)
        top = 0
        for line in path.read_text().splitlines():
            if not line.strip() or line[0] in ' \t#':
                continue
            m = imp.match(line)
            if m:
                top += 1
                if m[1] != 'Base':
                    target = (path.parent/m[1]).resolve()
                    if not target.is_file() or (out/'t').resolve() not in target.parents:
                        raise ValueError('import does not resolve inside the shard: ' + line)
                    pending.append(target)
                continue
            h = head.match(line)
            if not h or rel is None:
                raise ValueError('unaccounted top-level line in %s: %s' % (rel or 'PROOF.bend', line[:60]))
            top += 1
            found.add((rel, h[1], h[2]))
    if {r for r in seen if r} != emitted:
        raise ValueError('emitted modules unreachable from the entry: ' + ', '.join(sorted(emitted - seen)))
    if found != set(expected):
        raise ValueError('output declarations differ from the kept set: %d missing, %d extra' % (len(set(expected) - found), len(found - set(expected))))


class Module:
    """Strict reader: every top-level line must be an import (before any declaration), a
    comment, blank, or a def/type/law header. Anything else raises, so a construct this
    slicer does not understand can never be silently omitted."""

    def __init__(self, root, rel):
        self.rel = rel
        self.imports = {}   # alias -> (line, target rel path or 'Base')
        self.blocks = []
        cur = None
        for n, line in enumerate((root/rel).read_text().splitlines(keepends=True), 1):
            plain = line.rstrip('\r\n')
            if not plain.strip() or plain[0] in ' \t' or plain.startswith('#'):
                if plain.strip() and plain[0] in ' \t' and cur is None:
                    raise ValueError('%s:%d: indented text outside any declaration' % (rel, n))
                if cur is not None:
                    cur['text'] += line
                    c = CTOR.match(line)
                    if c and cur['kind'] == 'type':
                        cur['ctors'].add(c[1])
                continue
            m = IMPORT.match(plain)
            if m and cur is None:
                alias = m[2] or m[1]
                if alias in self.imports:
                    raise ValueError('%s:%d: duplicate import alias %s' % (rel, n, alias))
                target = m[1] if m[1] == 'Base' else (root/rel).parent.joinpath(m[1]).resolve().relative_to(root).as_posix()
                self.imports[alias] = (line, target)
                continue
            h = HEAD.match(line)
            if not h:
                raise ValueError('%s:%d: unsupported top-level syntax: %s' % (rel, n, plain[:60]))
            cur = {'kind': h[1], 'name': h[2], 'text': line, 'ctors': set(), 'mod': rel}
            self.blocks.append(cur)
        keys = [(b['kind'], b['name']) for b in self.blocks]
        if len(keys) != len(set(keys)):
            raise ValueError('%s: duplicate declaration' % rel)
        self.by_name, self.by_ctor = {}, {}
        for b in self.blocks:
            self.by_name.setdefault(b['name'], []).append(b)
            for c in b['ctors']:
                self.by_ctor.setdefault(c, []).append(b)

    def find(self, name):
        if name in self.by_name:
            return self.by_name[name]
        if name in self.by_ctor:
            return self.by_ctor[name]
        parts = name.split('.')
        for n in range(len(parts)-1, 0, -1):
            p = '.'.join(parts[:n])
            if p in self.by_name:
                return self.by_name[p]
            if p in self.by_ctor:
                return self.by_ctor[p]
        return []


class Slicer:
    def __init__(self, root, modules=()):
        self.root = Path(root).resolve()
        self.mods = {}
        for rel in modules:
            self.module(rel)

    def module(self, rel):
        if rel not in self.mods:
            self.mods[rel] = Module(self.root, rel)
        return self.mods[rel]

    def blocks(self, rel, name=None, kind=None):
        return [b for b in self.module(rel).blocks if (name is None or b['name'] == name) and (kind is None or b['kind'] == kind)]

    def reach(self, seeds):
        """seeds: declaration blocks. Returns (kept blocks by module rel as a set of ids, aliases used per module)."""
        keep, used, todo = {}, {}, []
        self.edges = {}
        current = [None]

        def push(b):
            if current[0] is not None and b['mod'] == current[0]['mod']:
                self.edges.setdefault(id(current[0]), []).append(b)
            k = keep.setdefault(b['mod'], set())
            if id(b) not in k:
                k.add(id(b))
                todo.append(b)

        for b in seeds:
            push(b)
        while todo:
            b = todo.pop()
            current[0] = b
            mod = self.module(b['mod'])
            imports = {a: t for a, (_, t) in mod.imports.items()}
            for m in LEX.finditer(b['text']):
                t = m[1]
                if not t:
                    continue
                first, _, rest = t.partition('.')
                if first in imports and imports[first] != 'Base':
                    used.setdefault(b['mod'], set()).add(first)
                    if rest:
                        for x in self.module(imports[first]).find(rest):
                            push(x)
                    # `Laws.x` is also a local declaration name in PROOF.bend
                    for x in mod.by_name.get(t, ()):
                        push(x)
                    continue
                for x in mod.find(t):
                    push(x)
        return keep, used

    def emit(self, keep, used, out):
        """Write kept declarations under out/<rel>. Returns the list of written module rels."""
        out = Path(out)
        for rel, ids in keep.items():
            mod = self.module(rel)
            lines = ['import Base\n']
            for alias, (line, target) in mod.imports.items():
                if target != 'Base' and alias in used.get(rel, ()) and keep.get(target):
                    lines.append(line)
            body = ''.join(b['text'] for b in self.ordered(mod, ids))
            path = out/rel
            path.parent.mkdir(parents=True, exist_ok=True)
            path.write_text(''.join(lines) + '\n' + body)
        return sorted(keep)

    def ordered(self, mod, ids):
        """Kept blocks with callees before callers, as the compiler wants a definition
        before its use. Mutually recursive groups (strongly connected components) keep
        their original relative order; unrelated blocks keep original order."""
        kept = [b for b in mod.blocks if id(b) in ids]
        pos = {id(b): i for i, b in enumerate(kept)}
        succ = {id(b): [d for d in self.edges.get(id(b), ()) if id(d) in pos and d is not b] for b in kept}
        index, low, onstack, stack, comp, count = {}, {}, set(), [], {}, [0]

        def strong(v):  # iterative Tarjan
            work = [(v, 0)]
            while work:
                node, i = work.pop()
                if i == 0:
                    index[node] = low[node] = count[0]; count[0] += 1
                    stack.append(node); onstack.add(node)
                recurse = False
                for j in range(i, len(succ[node])):
                    w = id(succ[node][j])
                    if w not in index:
                        work.append((node, j + 1)); work.append((w, 0)); recurse = True
                        break
                    if w in onstack:
                        low[node] = min(low[node], index[w])
                if recurse:
                    continue
                if low[node] == index[node]:
                    members = []
                    while True:
                        w = stack.pop(); onstack.discard(w); members.append(w)
                        if w == node:
                            break
                    for w in members:
                        comp[w] = node
                if work:
                    parent = work[-1][0]
                    low[parent] = min(low[parent], low[node])

        for b in kept:
            if id(b) not in index:
                strong(id(b))
        groups = {}
        for b in kept:
            groups.setdefault(comp[id(b)], []).append(b)
        out, done = [], set()

        def emit(c):
            work = [(c, False)]
            while work:
                node, seen = work.pop()
                if node in done:
                    continue
                if seen:
                    done.add(node); out.extend(sorted(groups[node], key=lambda b: pos[id(b)]))
                    continue
                work.append((node, True))
                for b in groups[node]:
                    for d in succ[id(b)]:
                        if comp[id(d)] != node and comp[id(d)] not in done:
                            work.append((comp[id(d)], False))

        for b in kept:
            emit(comp[id(b)])
        return out

    def entry(self, keep, used, tree='t'):
        """PROOF.bend for a shard. Imports one module from every source component of the kept
        import graph (a cycle is one component: any member reaches the others)."""
        edges = {rel: {t for a, (_, t) in self.module(rel).imports.items() if a in used.get(rel, ()) and keep.get(t)} for rel in keep}
        comp = scc(edges)
        incoming = {c for rel, ts in edges.items() for t in ts if comp[t] != comp[rel] for c in [comp[t]]}
        roots = sorted(min(r for r in keep if comp[r] == c) for c in set(comp.values()) - incoming)
        lines = ['import Base\n']
        for i, rel in enumerate(roots):
            lines.append('import ./%s/%s as E%d\n' % (tree, rel, i))
        return ''.join(lines)

    def write_shard(self, seeds, out):
        keep, used = self.reach(seeds)
        out = Path(out)
        out.mkdir(parents=True, exist_ok=True)
        self.emit(keep, used, out/'t')
        (out/'PROOF.bend').write_text(self.entry(keep, used))
        kept = {(rel, b['kind'], b['name']) for rel, ids in keep.items() for b in self.module(rel).blocks if id(b) in ids}
        verify_output(out, kept)
        return kept

    def all_declarations(self, rels):
        return {(rel, b['kind'], b['name']) for rel in rels for b in self.module(rel).blocks}

    def law_seeds(self, law):
        return self.blocks('LAWS.bend', law, 'law') + self.blocks('PROOF.bend', 'Laws.' + law, 'def')
