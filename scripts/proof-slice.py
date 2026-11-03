#!/usr/bin/env python3
"""Declaration-level slices of canonical modules (prototype, own file).

A shard is a set of root declarations. It keeps those and every declaration they
reach by name in LAWS.bend, PROOF.bend and the imported modules; each declaration
is copied byte for byte and nothing else is edited except dropping imports that
no kept declaration uses. A name that is dropped by mistake makes the compiler
fail (never a silent pass). Coverage is a separate, exact set computation:
`coverage()` lists every canonical declaration no shard keeps.
"""
import hashlib
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


EFFECT = re.compile(r'^\s+import\s+"([^"]+)"')
INCLUDE = re.compile(r'^\s*#\s*include\s+"([^"]+)"')
HEAD2 = re.compile(r'^(def|type|law) (\S+?)[\s(:<]')
IMP2 = re.compile(r'^import (\S+)(?: as (\w+))?\s*(?:#.*)?$')


def scan_module(text, origin):
    """Plain, separate line scanner (not Module). Returns (import lines, {(kind, name): sha256 of the
    declaration with trailing blank/comment lines removed}, {(kind, name): [effect paths]}).
    Unsupported top-level text and duplicate declarations raise."""
    imports, digests, effects, cur = [], {}, {}, None

    def close():
        if cur is not None:
            lines = cur[2]
            while lines and (not lines[-1].strip() or lines[-1].startswith('#')):
                lines = lines[:-1]
            digests[cur[:2]] = hashlib.sha256(''.join(lines).encode()).hexdigest()

    for n, line in enumerate(text.splitlines(keepends=True), 1):
        plain = line.rstrip('\r\n')
        if plain.strip() and plain[0] not in ' \t#':
            if IMP2.match(plain) and cur is None:
                imports.append(plain)
                continue
            h = HEAD2.match(plain)
            if not h:
                raise ValueError('%s:%d: unaccounted top-level line: %s' % (origin, n, plain[:60]))
            close()
            if (h[1], h[2]) in digests:
                raise ValueError('%s: duplicate declaration %s %s' % (origin, h[1], h[2]))
            cur = (h[1], h[2], [])
            effects[(h[1], h[2])] = []
        if cur is not None:
            cur[2].append(line)
            e = EFFECT.match(plain)
            if e:
                effects[cur[:2]].append(e[1])
    close()
    return imports, digests, effects


def effect_closure(root, module, path):
    """Repo-relative effect files a module's `import "x"` needs: the file and its local #include "y.h" headers."""
    root = Path(root)
    found, pending = set(), [(root/module).parent.joinpath(path).resolve()]
    while pending:
        f = pending.pop()
        rel = f.relative_to(root.resolve()).as_posix()
        if rel in found:
            continue
        if not f.is_file():
            raise ValueError('effect companion missing: %s (from %s)' % (rel, module))
        found.add(rel)
        if f.suffix in ('.c', '.h'):
            for line in f.read_text().splitlines():
                m = INCLUDE.match(line)
                if m:
                    pending.append((f.parent/m[1]).resolve())
    return found


def verify_output(out, kept, root):
    """Independent check against the ORIGINAL files. A separate scanner re-reads every emitted
    module from disk, follows the entry file's imports, and requires: every emitted module reachable;
    every import line copied verbatim from the original and resolving inside the shard; no
    unaccounted top-level line; no duplicate declaration; each declaration's exact text digest equal
    to the original's (law statements and proof bodies included); every alias used by a kept
    declaration still imported; and every effect companion a kept declaration imports present and
    byte-identical, with no extra companion files."""
    out, root = Path(out), Path(root).resolve()
    tree = (out/'t').resolve()
    original = {}
    expected = {}
    for rel, kind, name in kept:
        if rel not in original:
            original[rel] = scan_module((root/rel).read_text(), rel)
        if (kind, name) not in original[rel][1]:
            raise ValueError('kept declaration missing from the original: %s %s %s' % (rel, kind, name))
        expected[(rel, kind, name)] = original[rel][1][(kind, name)]
    disk = {p.relative_to(out/'t').as_posix() for p in (out/'t').rglob('*') if p.is_file()}
    emitted = {n for n in disk if n.endswith('.bend')}
    seen, found, needed, pending = set(), {}, set(), [out/'PROOF.bend']
    while pending:
        path = pending.pop()
        rel = None if path == out/'PROOF.bend' else path.relative_to(tree).as_posix()
        if rel in seen:
            continue
        seen.add(rel)
        text = path.read_text()
        imports, digests, effects = scan_module(text, rel or 'PROOF.bend')
        if rel is None:
            if digests:
                raise ValueError('entry file must not declare anything')
        else:
            orig_imports = set(original[rel][0]) if rel in original else None
            if orig_imports is None:
                raise ValueError('emitted module has no kept declaration: ' + rel)
            aliases = {}
            for line in original[rel][0]:
                m = IMP2.match(line)
                aliases[m[2] or m[1]] = line
            kept_aliases = set()
            for line in imports:
                if line not in orig_imports:
                    raise ValueError('import not present in the original %s: %s' % (rel, line))
                m = IMP2.match(line)
                kept_aliases.add(m[2] or m[1])
            body = ''.join(l for l in text.splitlines(keepends=True) if not (l and l[0] not in ' \t#\n' and IMP2.match(l.rstrip('\r\n'))))
            for m in LEX.finditer(body):
                first = m[1].split('.')[0] if m[1] else None
                if first in aliases and first != 'Base' and first not in kept_aliases:
                    raise ValueError('%s uses alias %s but no longer imports it' % (rel, first))
            for key, digest in digests.items():
                if (rel, *key) in found:
                    raise ValueError('duplicate declaration in output: %s %s %s' % (rel, *key))
                found[(rel, *key)] = digest
            for key, paths in effects.items():
                for e in paths:
                    for f in effect_closure(root, rel, e):
                        needed.add(f)
                        copy = tree/f
                        if not copy.is_file() or copy.read_bytes() != (root/f).read_bytes():
                            raise ValueError('effect companion missing or changed in the shard: ' + f)
        for line in imports:
            m = IMP2.match(line)
            if m[1] != 'Base':
                target = (path.parent/m[1]).resolve()
                if not target.is_file() or tree not in target.parents:
                    raise ValueError('import does not resolve inside the shard: ' + line)
                pending.append(target)
    if {r for r in seen if r} != emitted:
        raise ValueError('emitted modules unreachable from the entry: ' + ', '.join(sorted(emitted - seen)))
    if set(found) != set(expected):
        raise ValueError('output declarations differ from the kept set: %d missing, %d extra' % (len(set(expected) - set(found)), len(set(found) - set(expected))))
    changed = sorted(k for k in expected if found[k] != expected[k])
    if changed:
        raise ValueError('declaration text differs from the original: %s %s %s' % changed[0])
    extra = disk - emitted - {str(f) for f in needed}
    if extra or needed - (disk - emitted):
        raise ValueError('effect companions do not match the declarations that import them')


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
        for rel, ids in keep.items():
            for b in self.module(rel).blocks:
                if id(b) in ids:
                    for line in b['text'].splitlines():
                        e = EFFECT.match(line)
                        if e:
                            for f in effect_closure(self.root, rel, e[1]):
                                dest = out/'t'/f
                                dest.parent.mkdir(parents=True, exist_ok=True)
                                dest.write_bytes((self.root/f).read_bytes())
        (out/'PROOF.bend').write_text(self.entry(keep, used))
        kept = {(rel, b['kind'], b['name']) for rel, ids in keep.items() for b in self.module(rel).blocks if id(b) in ids}
        verify_output(out, kept, self.root)
        return kept

    def all_declarations(self, rels):
        return {(rel, b['kind'], b['name']) for rel in rels for b in self.module(rel).blocks}

    def law_seeds(self, law):
        return self.blocks('LAWS.bend', law, 'law') + self.blocks('PROOF.bend', 'Laws.' + law, 'def')
