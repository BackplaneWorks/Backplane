#!/usr/bin/env python3
"""Conservative source inventory, not a proof that a reachable handler works.

Resolve local Bend call edges and inventory literal D.On / Dom.button actions.
Computed action expressions are reported as unresolved, never counted covered.
--revision reads immutable git objects without switching or editing the checkout.
"""
import argparse
import json
from pathlib import Path
import re
import subprocess
import time

ROOT = Path(__file__).resolve().parents[2]


class Source:
    def __init__(self, revision=None, root=ROOT):
        self.revision, self.root = revision, Path(root)

    def read(self, path):
        if self.revision:
            p = subprocess.run(['git', 'show', f'{self.revision}:{path}'], cwd=self.root,
                               capture_output=True, text=True)
            return p.stdout if p.returncode == 0 else ''
        p = self.root / path
        return p.read_text() if p.exists() else ''

    def views(self):
        if self.revision:
            return subprocess.check_output(['git', 'ls-tree', '-r', '--name-only', self.revision,
                                            'src/web'], cwd=self.root, text=True).splitlines()
        return [str(p.relative_to(self.root)) for p in (self.root / 'src/web').glob('*.bend')]


def code(text):
    # Strip comments without treating a # inside a string as a comment.
    return re.sub(r'"(?:\\.|[^"\\])*"|#[^\n]*',
                  lambda m: '' if m[0].startswith('#') else m[0], text)


def definitions(text):
    starts = list(re.finditer(r'^(?:@unsafe )?def ([\w.]+)[^\n]*', code(text), re.M))
    text = code(text)
    return {m[1]: text[m.start(): starts[i+1].start() if i+1 < len(starts) else len(text)]
            for i, m in enumerate(starts)}


def reachable(defs, roots):
    todo, seen = list(roots), set()
    while todo:
        name = todo.pop()
        if name in seen or name not in defs:
            continue
        seen.add(name)
        # Ignore literals when finding function calls.
        body = re.sub(r'"(?:\\.|[^"\\])*"', '""', defs[name])
        todo.extend(n for n in re.findall(r'([\w.]+)\s*\(', body) if n in defs)
    return '\n'.join(defs[n] for n in sorted(seen))


def arguments(text, start):
    depth, quoted, escape, begin, out = 0, False, False, start, []
    for i in range(start, len(text)):
        c = text[i]
        if quoted:
            if escape: escape = False
            elif c == '\\': escape = True
            elif c == '"': quoted = False
        elif c == '"': quoted = True
        elif c in '([{': depth += 1
        elif c in ')]}':
            if c == ')' and depth == 0:
                return out + [text[begin:i].strip()]
            depth -= 1
        elif c == ',' and depth == 0:
            out.append(text[begin:i].strip()); begin = i+1
    return []


def inventory(src):
    start = time.perf_counter()
    client = definitions(src.read('src/core/client.bend'))
    dispatch = reachable(client, ['Ui.act'])
    # The index-dispatch table is called, and its case body is reachable too.
    names = set(re.findall(r'"([a-z][a-z0-9-]+)"', dispatch))
    host = code(src.read('src/web/host.js'))
    # Do not count arbitrary host comparisons as handlers: e.g. bookkeeping
    # before forwarding "select" does not implement selection. Only these
    # local effect owners are admitted, and still need behavioral checks.
    for action in ['copy', 'hold-bottom', 'load-earlier', 'sealed', 'secret-save', 'devices-open', 'desktop-folder']:
        if re.search(r'action\s*===\s*[\'"]'+action+r'[\'"]', host):
            names.add(action)
    app = code(src.read('src/web/app.bend'))
    names.update(re.findall(r'String.eq\(action, "([\w-]+)"\)', app))
    workspace = src.read('src/core/workspace.bend')
    names.update(re.findall(r'"(ws-[\w-]+)"', workspace))
    # Additional web-only dispatcher (mail/browser/workspace) tables.
    for path in ['src/web/workspace.bend', 'src/web/mail-actions.bend']:
        names.update(re.findall(r'"((?:ws-|web-mail-|mail-)[a-z0-9-]+)"', reachable(definitions(src.read(path)),
                          ['WS.handles', 'WS.act', 'MA.action'])))
    table = re.findall(r'"([^"]+)"', client.get('Ui.actions', ''))
    indexed = re.split(r'^    case (\d+)n:\s*\n', client.get('Ui.act.k', ''), flags=re.M)
    inert = set()
    for i in range(1,len(indexed)-1,2):
        n = int(indexed[i]); body = re.split(r'^    case ', indexed[i+1], flags=re.M)[0]
        if n < len(table) and re.fullmatch(r'Act\{u,Nil\{\}\}', re.sub(r'\s+','',body)):
            inert.add(table[n])
    names -= inert
    actions, unresolved, urls = [], [], []
    for path in sorted(src.views()):
        if not path.endswith('.bend'):
            continue
        text = code(src.read(path))
        for m in re.finditer(r'\b(?:D\.)?Dom\.button\(', text):
            if text[text.rfind('\n',0,m.start())+1:m.start()].lstrip().startswith('def '):
                continue
            args = arguments(text, m.end())
            if len(args) != 5:
                continue
            literal = re.fullmatch(r'"([\w-]+)"', args[1])
            where = {'file':path,'line':text.count('\n',0,m.start())+1}
            if literal:
                actions.append({**where,'event':'click','action':literal[1],'covered':literal[1] in names})
            else:
                unresolved.append({**where,'expression':args[1]})
        for m in re.finditer(r'(?:D\.)?On\{\s*"([^"]+)"\s*,\s*([^}\n]+)\}', text):
            if text[text.rfind('\n',0,m.start())+1:m.start()].lstrip().startswith('case '):
                continue
            where = {'file': path, 'line': text.count('\n', 0, m.start()) + 1}
            literal = re.fullmatch(r'"([\w-]+)"', m[2].strip())
            if literal:
                actions.append({**where, 'event': m[1], 'action': literal[1],
                                'covered': literal[1] in names})
            else:
                unresolved.append({**where, 'expression': m[2].strip()})
        for m in re.finditer(r'(?:D\.)?At\{\s*"(src|href)"\s*,\s*([^}\n]+)\}', text):
            urls.append({'file': path, 'line': text.count('\n', 0, m.start()) + 1,
                         'attribute': m[1], 'expression': m[2].strip()})
    server = src.read('src/server/server.bend')
    image = src.read('src/core/images.bend')
    # A route-presence check deliberately reports limited assurance: authentication,
    # far-hub ownership, and streamed bodies need dynamic contract checks.
    img_route = bool(re.search(r'"/img"', server) and re.search(r'Img\.serve\(allowed,', server))
    img_relative = '"/img?path="' in image
    # Clipboard user activation expires across a hub round trip. A generic
    # Copy command is not a substitute for this local event interception.
    listener = host[host.find('for (const ev of EVENTS)'):]
    gesture = bool(re.search(r'if\s*\(ev\s*===\s*"click"\s*&&\s*action\s*===\s*"copy"\)\s*\{[^}]*copyButton\(el,\s*el\.getAttribute\("data-value"\)', listener))
    return {'revision': src.revision or 'workspace', 'seconds': time.perf_counter()-start,
            'actions': actions, 'missing': [a for a in actions if not a['covered']],
            'computed_actions': unresolved, 'urls': urls,
            'unresolved_urls':[u for u in urls if not re.fullmatch(r'"(?:#|https?://[^"\s]+)"',u['expression'])],
            'copy_local_gesture': gesture,
            'syntactically_inert': sorted(inert),
            'remote_image_route_present': img_route, 'relative_image_url': img_relative,
            'limits': ['literal action existence only; effects may still fail',
                       'computed actions and URLs unresolved',
                       'route presence does not prove remote ownership/auth/body delivery']}


def main():
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument('--revision')
    p.add_argument('--json', action='store_true')
    p.add_argument('--strict', action='store_true', help='fail on missing literal actions')
    a = p.parse_args()
    out = inventory(Source(a.revision))
    if a.json:
        print(json.dumps(out, indent=2))
    else:
        print(f"{len(out['actions'])} literal bindings, {len(out['missing'])} unmatched, "
              f"{len(out['computed_actions'])} computed; {out['seconds']:.3f}s")
        for row in out['missing']:
            print(f"{row['file']}:{row['line']}: {row['action']}")
        print('remote /img route present:', out['remote_image_route_present'],
              '; relative URL:', out['relative_image_url'])
        print('local copy gesture contract:', out['copy_local_gesture'])
    return int(a.strict and bool(out['missing']))


if __name__ == '__main__':
    raise SystemExit(main())
