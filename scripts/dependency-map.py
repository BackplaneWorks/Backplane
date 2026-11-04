#!/usr/bin/env python3
"""Static declaration/import graph for restructuring; never a proof or coverage gate.

Uses #871's strict source reader and lexical name resolution. Base is excluded;
local binders can conservatively collide with declaration names. The numbers
describe source reachability, not the compiler's elaborated dependency graph.
"""
import argparse
import hashlib
import importlib.util
import json
from pathlib import Path
import subprocess


def load_slicer():
    spec = importlib.util.spec_from_file_location('proof_slice', Path(__file__).with_name('proof-slice.py'))
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


def closures(edges, scc):
    """All reflexive transitive closures as integer bitsets, including cycles."""
    comp = scc(edges)
    members, children = {}, {}
    for node in edges:
        c = comp[node]
        members[c] = members.get(c, 0) | (1 << node)
        children.setdefault(c, set()).update(comp[d] for d in edges[node] if comp[d] != c)
    parents = {c: set() for c in members}
    for c, ds in children.items():
        for d in ds:
            parents[d].add(c)
    pending = {c: len(ds) for c, ds in children.items()}
    ready = [c for c, n in pending.items() if not n]
    reached = dict(members)
    count = 0
    while ready:
        c = ready.pop()
        count += 1
        for p in parents[c]:
            reached[p] |= reached[c]
            pending[p] -= 1
            if not pending[p]:
                ready.append(p)
    if count != len(members):
        raise ValueError('component graph did not terminate')
    return {n: reached[comp[n]] for n in edges}, sorted((v.bit_count() for v in members.values()), reverse=True)


def analyze(root, roots, include_graph=False):
    sl = load_slicer()
    reader = sl.Slicer(root)
    todo = list(roots)
    while todo:
        rel = todo.pop()
        if rel in reader.mods:
            continue
        mod = reader.module(rel)
        todo.extend(t for _, t in mod.imports.values() if t != 'Base' and t not in reader.mods)
    blocks = [b for rel in sorted(reader.mods) for b in reader.mods[rel].blocks]
    index = {id(b): i for i, b in enumerate(blocks)}
    edges = {i: set() for i in range(len(blocks))}
    for i, b in enumerate(blocks):
        mod = reader.mods[b['mod']]
        for token in sl.LEX.finditer(b['text']):
            name = token[1]
            if not name:
                continue
            first, _, rest = name.partition('.')
            imported = mod.imports.get(first)
            if imported and imported[1] != 'Base':
                deps = reader.mods[imported[1]].find(rest) if rest else []
                deps = [*deps, *mod.by_name.get(name, ())]
            else:
                deps = mod.find(name)
            edges[i].update(index[id(d)] for d in deps if d is not b)
    reached, components = closures(edges, sl.scc)
    module_ids = {m: i for i, m in enumerate(sorted(reader.mods))}
    module_edges = {module_ids[m]: {module_ids[t] for _, t in mod.imports.values() if t != 'Base'} for m, mod in reader.mods.items()}
    module_reach, _ = closures(module_edges, sl.scc)
    module_masks = {m: 0 for m in reader.mods}
    for i, b in enumerate(blocks):
        module_masks[b['mod']] |= 1 << i

    def describe(bits):
        return {'declarations': bits.bit_count(),
                'modules': sum(bool(bits & mask) for mask in module_masks.values())}

    declarations = [{'module': b['mod'], 'kind': b['kind'], 'name': b['name'], **describe(reached[i])} for i, b in enumerate(blocks)]
    by_key = {(b['mod'], b['kind'], b['name']): i for i, b in enumerate(blocks)}
    laws = []
    for i, b in enumerate(blocks):
        if b['kind'] != 'law' or b['mod'] != 'LAWS.bend':
            continue
        p = by_key.get(('PROOF.bend', 'def', 'Laws.' + b['name']))
        if p is None:
            raise ValueError('law missing paired proof: ' + b['name'])
        bits = reached[i] | reached[p]
        law_modules = [m for m, mask in module_masks.items() if bits & mask]
        imported = 0
        for m in law_modules:
            if m not in ('LAWS.bend', 'PROOF.bend'):
                imported |= module_reach[module_ids[m]]
        laws.append({'name': b['name'], **describe(bits), 'import_modules': imported.bit_count()})
    modules = []
    for m, mod in reader.mods.items():
        mid = module_ids[m]
        modules.append({'module': m, 'lines': len((root/m).read_text().splitlines()),
                        'bytes': (root/m).stat().st_size, 'declarations': len(mod.blocks),
                        'direct_importers': sum(mid in ds for ds in module_edges.values()),
                        'transitive_importers': sum(bool(bits & (1 << mid)) for n, bits in module_reach.items() if n != mid),
                        'import_closure': module_reach[mid].bit_count()})
    source_hashes = {m: hashlib.sha256((root/m).read_bytes()).hexdigest() for m in sorted(reader.mods)}
    result = {'schema': 1, 'method': 'lexical declaration graph; Base excluded; conservative binder collisions; not a proof gate',
            'sources': source_hashes,
            'source_identity': hashlib.sha256(json.dumps(source_hashes, sort_keys=True).encode()).hexdigest(),
            'roots': roots, 'totals': {'modules': len(reader.mods), 'declarations': len(blocks),
                                     'edges': sum(map(len, edges.values())), 'laws': len(laws)},
            'scc_sizes': components[:20],
            'definitions': sorted((d for d in declarations if d['kind'] == 'def'), key=lambda d: (-d['declarations'], d['module'], d['name'])),
            'laws': sorted(laws, key=lambda d: (-d['declarations'], d['name'])),
            'modules': sorted(modules, key=lambda d: (-d['lines'], d['module']))}
    if include_graph:
        result['graph'] = {'nodes': [[b['mod'], b['kind'], b['name']] for b in blocks],
                           'edges': [sorted(edges[i]) for i in range(len(blocks))],
                           'imports': {m: sorted(t for _, t in mod.imports.values() if t != 'Base')
                                       for m, mod in sorted(reader.mods.items())}}
    return result


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--root', type=Path, default=Path(__file__).resolve().parents[1])
    parser.add_argument('--out', type=Path, required=True)
    parser.add_argument('--graph', action='store_true', help='include indexed declaration edges and module imports')
    parser.add_argument('roots', nargs='*', default=['PROOF.bend', 'src/server/server.bend'])
    args = parser.parse_args()
    result = analyze(args.root.resolve(), args.roots, args.graph)
    result['revision'] = subprocess.check_output(['git', 'rev-parse', 'HEAD'], cwd=args.root, text=True).strip()
    args.out.parent.mkdir(parents=True, exist_ok=True)
    args.out.write_text(json.dumps(result, indent=2) + '\n')
    print(json.dumps({**result['totals'], 'largest_definition': result['definitions'][0], 'largest_law': result['laws'][0]}))


if __name__ == '__main__':
    main()
