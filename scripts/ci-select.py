#!/usr/bin/env python3
"""Plan affected Bend tests conservatively. Does not execute or certify gates.

Unknown inputs select the full set. Drivers/custom effects and Base operations
that can reach undeclared inputs are opaque and always retained. The required
fast set is retained. Existing local/nightly selection is unchanged.
"""
import argparse
import hashlib
import json
import os
from pathlib import Path
import re
import shutil
import subprocess
import sys

NEGATIVE = 'test/cache_identity_before_test.bend'
BASE_SHA = '99ac43f2b2bb3e3f39acdcedcecbbd3cb44749ce13969c827d6973fa66f7facf'
BUNDLED_SHA = 'b49a465867250d3eb29caeed73d2dee70349428c7473e7b76811117385601f35'
# Official Bend2.0.36 executables (the same pins as inputs-key.sh), not a
# version-string guess: custom compilers get full selection.
BIN_SHA = {
    'b2b183bddc3c3fa4bc45d99f6a6bdbb192ebc616e6e697777f49e74c545b1643',
    '301a69a8e20937b37a431ecff5a80e85fdc5a2224ead972088e4ee9f3f89a300',
    '4d7280f366661d9b63b418eb70ca54ea1a867a23b196cc4b5f1f75e049cb80d1',
    'db96d899b65a1a1415291bfb9fb33d1ab43b0d076071d3489cc4a678670b2e00',
}
IMPORT = re.compile(r'^\s*import\s+(\S+)(?:\s+as\s+([A-Za-z_][A-Za-z0-9_]*))?\s*(?:#.*)?$')
TOKENS = re.compile(r'[A-Za-z_][A-Za-z_0-9]*(?:\.[A-Za-z_][A-Za-z_0-9]*)*')
LITERALS = re.compile(r'"(?:\\.|[^"\\])*"|\'(?:\\.|[^\'\\])*\'|#[^\n]*')


def tokens(text):
    return set(TOKENS.findall(LITERALS.sub(' ', text)))


def base_opaque(text):
    # This deliberately small model is bound to the exact supported Base.
    # New compiler/library bytes require full selection until reviewed.
    if hashlib.sha256(text.encode()).hexdigest() != BASE_SHA:
        raise ValueError('unsupported Base identity: select all tests')
    blocks = {}
    matches = list(re.finditer(r'^(def|law|type) ([A-Za-z_][A-Za-z_0-9.]*)', text, re.M))
    for i, m in enumerate(matches):
        body = text[m.start():matches[i+1].start() if i+1 < len(matches) else len(text)]
        blocks.setdefault(m[2], []).append(body)
    # These operations cannot load an undeclared source module/file or launch
    # another program. File/network/process/window/audio operations stay opaque.
    # Environment/clock/random values do not create new source dependencies;
    # a subsequent File/Process call remains opaque through the closure.
    harmless = {'./effs/'+name+ext for name in [
        'print', 'write', 'print_err', 'args', 'get_env', 'now', 'sleep',
        'random_u32', 'spawn', 'chan', 'thread_count'] for ext in ['.c', '.js']}
    opaque = set()
    refs = {}
    for name, bodies in blocks.items():
        body = '\n'.join(bodies)
        refs[name] = tokens(body) - {name}
        for line in body.splitlines():
            if re.match(r'^\s*import\b', line):
                m = IMPORT.fullmatch(line)
                if not m or m[1].strip('"') not in harmless:
                    opaque.add(name)
    while True:
        more = {n for n, used in refs.items() if used & opaque}
        if more <= opaque:
            return opaque
        opaque |= more


def file_digest(path):
    h = hashlib.sha256()
    with path.open('rb') as f:
        while chunk := f.read(1024 * 1024):
            h.update(chunk)
    return h.hexdigest()


def library_identity(folder):
    files = {}
    for p in sorted(folder.rglob('*')):
        if p.is_symlink():
            raise ValueError('unsupported linked compiler resource')
        if p.is_file():
            files[p.relative_to(folder).as_posix()] = file_digest(p)
    return hashlib.sha256(json.dumps(files, sort_keys=True, separators=(',', ':')).encode()).hexdigest()


def compiler_model(binary):
    if file_digest(binary) not in BIN_SHA:
        raise ValueError('unsupported compiler identity: select all tests')
    bundled = binary.parent.parent/'bend2'
    if library_identity(bundled) != BUNDLED_SHA:
        raise ValueError('unsupported compiler resources: select all tests')
    named = Path(os.environ.get('BEND_LIB', Path.home()/'.bend/lib'))
    if named.exists() and any(p.is_file() for p in named.rglob('*')):
        raise ValueError('unsupported named-library environment: select all tests')
    return base_opaque((bundled/'base.bend').read_text())


def normalized(path):
    name = os.path.normpath(path).replace(os.sep, '/')
    if name == '..' or name.startswith('../') or os.path.isabs(name):
        raise ValueError('dependency outside checkout: ' + name)
    return name


class Graph:
    def __init__(self, root, opaque_base):
        self.root = Path(root).resolve()
        self.opaque_base = opaque_base
        self.nodes = {}

    def node(self, name):
        name = normalized(name)
        if name in self.nodes:
            return self.nodes[name]
        path = self.root/name
        if any(p.is_symlink() for p in [path, *path.parents]) or not path.is_file():
            raise ValueError('missing or linked dependency: ' + name)
        body = path.read_text()
        deps, opaque = set(), bool(tokens(body) & self.opaque_base)
        for line in body.splitlines():
            if not re.match(r'^\s*import\b', line):
                continue
            m = IMPORT.fullmatch(line)
            if not m:
                raise ValueError('unrecognized import in ' + name)
            target, alias = m.groups()
            target = target.strip('"')
            if target.startswith(('./', '../')):
                dep = normalized(os.path.join(os.path.dirname(name), target))
                dep_path = self.root/dep
                if not dep_path.is_file() or any(p.is_symlink() for p in [dep_path, *dep_path.parents]):
                    raise ValueError('missing or linked dependency: ' + dep)
                if dep.endswith('.bend'):
                    deps.add(dep)
                else:
                    # Dynamic reads in effects cannot be inferred from imports.
                    opaque = True
            elif target != 'Base' or alias:
                # Named libraries and aliases of Base need an explicit model.
                opaque = True
        if '@unsafe' in LITERALS.sub(' ', body):
            opaque = True
        self.nodes[name] = deps, opaque
        return deps, opaque

    def closure(self, test):
        todo, seen, opaque = [test], set(), False
        while todo:
            name = todo.pop()
            if name in seen:
                continue
            seen.add(name)
            deps, dynamic = self.node(name)
            opaque |= dynamic
            todo.extend(deps - seen)
        stem = Path(test).stem
        drivers = [self.root/'test/tools'/(stem.removesuffix('_test')+'.mjs'),
                   self.root/'test/tools'/(stem+'.py')]
        return seen, opaque or any(p.exists() for p in drivers)


def all_tests(root):
    return sorted(p.relative_to(root).as_posix() for p in Path(root).glob('test/*_test.bend')
                  if p.is_file() and p.relative_to(root).as_posix() != NEGATIVE)


def plan(root, tests, required, changed, opaque_base):
    tests, required, changed = sorted(set(tests)), set(required), set(changed)
    if not tests or tests != all_tests(root) or NEGATIVE in tests or not required <= set(tests):
        raise ValueError('invalid test inventory or required set')
    full_reason = None
    selected = set(required)
    reasons = {t: ['required'] for t in required}
    if any(not p.endswith('.bend') and not p.startswith('.ci-packet/') for p in changed):
        full_reason = 'unmodelled non-Bend input changed'
    graph = Graph(root, opaque_base)
    try:
        footprints = {t: graph.closure(t) for t in tests}
    except (ValueError, OSError, UnicodeError) as e:
        footprints = {}
        full_reason = str(e)
    for test, (deps, opaque) in footprints.items():
        why = []
        if deps & changed:
            why.append('changed dependency')
        if changed and opaque:
            why.append('opaque driver/effect/Base operation')
        if why:
            selected.add(test)
            reasons.setdefault(test, []).extend(why)
    if full_reason:
        selected = set(tests)
    return {'schema': 1, 'mode': 'full' if full_reason else 'affected',
            'full_reason': full_reason, 'changed': sorted(changed),
            'tests': tests, 'selected': sorted(selected), 'skipped': sorted(set(tests)-selected),
            'reasons': reasons, 'required': sorted(required), 'source_nodes': len(graph.nodes),
            'scope': 'Bend tests only; check/proof/build/other-language gates are not skipped'}


def validate(plan_data, root, opaque_base):
    if plan_data['tests'] != all_tests(root):
        raise ValueError('test inventory changed; refusing to skip')
    required = plan_data['required']
    fast = Path(root)/'scripts/fast-tests.txt'
    if fast.exists():
        required = [l.split('#', 1)[0].strip() for l in fast.read_text().splitlines()]
        required = sorted({l for l in required if l})
    changed = (changed_files(plan_data['git_base'], root=root) if 'git_base' in plan_data
               else plan_data['changed'])
    expected = plan(root, plan_data['tests'], required, changed, opaque_base)
    if 'git_base' in plan_data:
        expected['git_base'] = plan_data['git_base']
    if plan_data != expected:
        raise ValueError('affected plan no longer matches inputs/selection; refusing to skip')


def changed_files(base, root=None):
    merge = subprocess.check_output(['git', 'merge-base', base, 'HEAD'], text=True, cwd=root).strip()
    tracked = subprocess.check_output(['git', 'diff', '--no-renames', '--name-only', '-z', merge], cwd=root)
    untracked = subprocess.check_output(['git', 'ls-files', '--others', '--exclude-standard', '-z'], cwd=root)
    return {p.decode() for p in (tracked+untracked).split(b'\0') if p}


def main():
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument('--base', required=True, help='PR base ref; merge-base includes dirty/untracked source')
    p.add_argument('--out', type=Path)
    a = p.parse_args()
    try:
        if a.out:
            a.out = a.out.resolve()
            a.out.unlink(missing_ok=True)  # A failed plan cannot leave an older skip list.
        root = Path(subprocess.check_output(['git', 'rev-parse', '--show-toplevel'], text=True).strip())
        os.chdir(root)
        tests = all_tests(root)
        required = [l.split('#', 1)[0].strip() for l in Path('scripts/fast-tests.txt').read_text().splitlines()]
        required = [l for l in required if l]
        if not tests or not required or not set(required) <= set(tests):
            raise ValueError('invalid positive-test inventory or required fast set')
        binary = Path(shutil.which('bend') or '').resolve()
        base = subprocess.check_output(['git', 'merge-base', a.base, 'HEAD'], text=True).strip()
        changed = changed_files(base)
        try:
            opaque = compiler_model(binary)
            result = plan(root, tests, required, changed, opaque)
            result['git_base'] = base
            validate(result, root, opaque)
        except (ValueError, OSError, UnicodeError) as error:
            tests = all_tests(root)
            result = {'schema': 1, 'mode': 'full', 'full_reason': str(error),
                      'tests': tests, 'selected': tests, 'skipped': [],
                      'changed': sorted(changed), 'scope': 'Bend tests only'}
        text = json.dumps(result, sort_keys=True, indent=2)+'\n'
        if a.out:
            a.out.parent.mkdir(parents=True, exist_ok=True)
            a.out.write_text(text)
        else:
            sys.stdout.write(text)
    except (ValueError, OSError, subprocess.CalledProcessError) as error:
        print('ci-select: '+str(error), file=sys.stderr)
        return 3  # no plan is preferable to an invented empty success
    return 0


if __name__ == '__main__':
    sys.exit(main())
