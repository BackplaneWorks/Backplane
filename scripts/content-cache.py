#!/usr/bin/env python3
"""Content-addressed results for Bend module checks and proof shards.

A unit (a module's type-check, a proof shard) is keyed by the SHA-256 of
everything it reads: the unit, its full dependency closure (every imported
.bend, effect file and local header, by content), the compiler/library
identity, the environment bend reads, and the verdict logic. An entry exists
only for a pass that was recorded after the inputs were re-hashed unchanged.
Anything else - an unknown import, an unreadable file, an unidentifiable
compiler, a damaged or mislabelled entry - is a miss (or an error), never a
pass.

  content-cache.py plan-check [--cache D] [--shared D] [--all] [--plan P]
      JSON: {"hit": [...], "miss": [...]} over the tracked modules
      (LAWS.bend is a module; PROOF.bend is proved by shards, not checked here).
      --plan P saves each module's key as taken BEFORE the check runs.
  content-cache.py record-check FILE... --plan P [--shared-write]
      record the named modules as passed after a real passing check; a module
      whose key now differs from the planned one (edited while checking, or
      not planned) is refused, so a pass is never filed under other inputs
  content-cache.py plan-proof PARTITION_DIR [--cache D] [--shared D] [--all]
      JSON: shards of a verified proof-shards partition, hit or miss
  content-cache.py record-proof PARTITION_DIR --receipt R... [--shared-write]
  content-cache.py audit [--cache D]
      every entry must still be self-consistent; prints the stale ones
      (a stale entry is harmless: its key is never asked for again)

--all ignores the cache for planning (the nightly full run): every unit is a
miss. Local cache: $BP_CONTENT_CACHE or ~/.cache/backplane-bend/content.
Shared caches (read-only unless --shared-write, which only trusted main
builds pass): $BP_CONTENT_CACHE_SHARED, a path list separated by ':'.
"""
import argparse
import hashlib
import importlib.util
import json
import os
from pathlib import Path
import re
import subprocess
import sys
import time

ROOT = Path(__file__).resolve().parent.parent
SCHEMA = 1
BEND_IMPORT = re.compile(r'^\s*import\s+(\S+)(?:\s+as\s+[A-Za-z_][A-Za-z0-9_]*)?\s*(?:#.*)?$')
INCLUDE = re.compile(r'^\s*#\s*include\s+"([^"]+)"')
# files whose bytes decide what a pass means, beside the compiler itself
LOGIC = ['scripts/bendok.sh', 'scripts/check-manifest.py', 'scripts/content-cache.py']
ENV = ['BEND_LIB', 'BENDTT', 'CC']


class CacheError(ValueError):
    """Anything that makes a key or an entry untrustworthy."""


def sha(data):
    return hashlib.sha256(data).hexdigest()


def canonical(value):
    return json.dumps(value, sort_keys=True, separators=(',', ':')).encode()


def _module(name):
    spec = importlib.util.spec_from_file_location(name.replace('-', '_').replace('.py', ''), ROOT/'scripts'/name)
    mod = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(mod)
    return mod


_tool = None


def tool_identity():
    """Compiler + libraries + the settings bend reads + the verdict logic."""
    global _tool
    if _tool is None:
        try:
            tc = _module('ci-packet.py').toolchain()
        except Exception as error:  # no bend, unreadable library, link...
            raise CacheError('compiler identity unavailable: %s' % error)
        if not isinstance(tc, dict) or not tc.get('binary_sha256') or 'bundled' not in tc:
            raise CacheError('incomplete compiler identity')
        logic = {}
        for name in LOGIC:
            try:
                logic[name] = sha((ROOT/name).read_bytes())
            except OSError as error:
                raise CacheError('verdict logic unreadable: %s' % error)
        _tool = {'toolchain': tc, 'env': {k: os.environ.get(k, '') for k in ENV}, 'logic': logic}
    return _tool


def reset_tool_identity():
    global _tool
    _tool = None


# ---- dependency closure (same grammar as scripts/bend-closure.py, strict) ----

def rel(root, path):
    p = os.path.normpath(path)
    if os.path.isabs(p) or p == '..' or p.startswith('..' + os.sep):
        raise CacheError('outside the checkout: %s' % path)
    return p


def imports_of(root, f, text):
    """Direct dependencies of file f (relative paths), failing closed."""
    here = os.path.dirname(f)
    out = []
    if f.endswith('.bend'):
        for line in text.split('\n'):
            if not re.match(r'^\s*import\b', line):
                continue
            m = BEND_IMPORT.match(line)
            if not m:
                raise CacheError('%s: an import it does not understand: %r' % (f, line))
            name = m.group(1)
            if len(name) > 1 and name[0] == '"' and name[-1] == '"':
                name = name[1:-1]
            if name.startswith(('./', '../')):
                out.append(rel(root, os.path.join(here, name)))
            elif not re.match(r'^[A-Za-z_][A-Za-z0-9_]*$', name):
                raise CacheError('%s: an import it does not understand: %r' % (f, line))
            # a bare name is bend's own: covered by the tool identity
    elif f.endswith(('.c', '.h')):
        for line in text.split('\n'):
            m = INCLUDE.match(line)
            if m:
                p = rel(root, os.path.join(here, m.group(1)))
                if (root/p).exists():
                    out.append(p)
    return out


class Tree:
    """File hashes and closures over one checkout, each file read once."""

    def __init__(self, root):
        self.root = Path(root)
        self.digest, self.text, self.edges, self.memo = {}, {}, {}, {}

    def read(self, f):
        if f not in self.digest:
            p = self.root/f
            if p.is_symlink() or not p.is_file():
                raise CacheError('cannot read a regular file: %s' % f)
            try:
                data = p.read_bytes()
            except OSError as error:
                raise CacheError('cannot read %s: %s' % (f, error))
            self.digest[f] = sha(data)
            self.text[f] = data.decode('utf-8', 'replace') if f.endswith(('.bend', '.c', '.h')) else ''
            self.edges[f] = imports_of(self.root, f, self.text[f])
        return self.digest[f]

    def closure(self, f):
        f = rel(self.root, f)
        if f in self.memo:
            return self.memo[f]
        seen, todo = set(), [f]
        while todo:
            n = todo.pop()
            if n in seen:
                continue
            seen.add(n)
            self.read(n)
            todo.extend(self.edges[n])
        self.memo[f] = {n: self.digest[n] for n in sorted(seen)}
        return self.memo[f]


def unit_key(gate, unit, inputs, tool):
    if not inputs:
        raise CacheError('a unit with no inputs has no key')
    body = {'schema': SCHEMA, 'gate': gate, 'unit': unit, 'inputs': inputs, 'tool': tool}
    return sha(canonical(body)), body


def tracked_modules(root):
    out = subprocess.run(['git', 'ls-files', '-co', '--exclude-standard', '-z', '--', '*.bend'],
                         cwd=root, capture_output=True)
    if out.returncode != 0:
        raise CacheError('no git checkout to list modules from')
    names = [n for n in out.stdout.decode().split('\0') if n]
    if not names:
        raise CacheError('no modules found')
    return sorted(n for n in names if n != 'PROOF.bend'
                  and not n.startswith(('.bend-check.', '.bend-test.')))


def check_units(root, files=None):
    tree = Tree(root)
    tool = tool_identity()
    units = {}
    for f in files if files is not None else tracked_modules(root):
        key, body = unit_key('check', f, tree.closure(f), tool)
        units[f] = (key, body)
    return units


# ---- store ----

def local_dir():
    return Path(os.environ.get('BP_CONTENT_CACHE') or
                Path(os.environ.get('XDG_CACHE_HOME') or Path.home()/'.cache')/'backplane-bend'/'content')


def shared_dirs():
    return [Path(p) for p in os.environ.get('BP_CONTENT_CACHE_SHARED', '').split(':') if p]


def entry_path(folder, key):
    return Path(folder)/key[:2]/(key + '.json')


def valid_entry(entry, key):
    """An entry is a pass only if it is whole and names exactly this key."""
    try:
        return (isinstance(entry, dict) and entry.get('schema') == SCHEMA
                and entry.get('verdict') == 'passed' and entry.get('key') == key
                and isinstance(entry.get('body'), dict)
                and sha(canonical(entry['body'])) == key)
    except (TypeError, ValueError):
        return False


def lookup(key, local=None, shared=None):
    """'local' | 'shared' | None. A shared hit is copied to the local cache."""
    local = local_dir() if local is None else Path(local)
    for where, folders in (('local', [local]), ('shared', shared_dirs() if shared is None else shared)):
        for folder in folders:
            try:
                entry = json.loads(entry_path(folder, key).read_text())
            except (OSError, ValueError):
                continue
            if valid_entry(entry, key):
                if where == 'shared':
                    _write(entry_path(local, key), entry)
                return where
    return None


def _write(path, entry):
    path.parent.mkdir(parents=True, exist_ok=True)
    tmp = path.with_name(path.name + '.%d.tmp' % os.getpid())
    tmp.write_text(json.dumps(entry, sort_keys=True))
    os.replace(tmp, path)


def record(key, body, local=None, shared_write=False, shared=None):
    """Store a pass. body must hash to key; callers re-key just before."""
    if sha(canonical(body)) != key:
        raise CacheError('record: body does not hash to its key')
    entry = {'schema': SCHEMA, 'key': key, 'verdict': 'passed', 'body': body, 'recorded': int(time.time())}
    _write(entry_path(local_dir() if local is None else local, key), entry)
    if shared_write:
        folders = shared_dirs() if shared is None else shared
        if not folders:
            raise CacheError('--shared-write without a shared cache')
        _write(entry_path(folders[0], key), entry)


def plan(units, use_cache=True, local=None, shared=None):
    hit, miss = [], []
    for name, (key, _) in sorted(units.items()):
        (hit if use_cache and lookup(key, local, shared) else miss).append(name)
    return hit, miss


# ---- proof shards ----

def shard_units(root, folder):
    ps = _module('proof-shards.py')
    manifest = ps.verify(Path(root), Path(folder))  # fails unless it is the canonical partition
    tool = tool_identity()
    if manifest.get('toolchain') != tool['toolchain']:
        raise CacheError('partition was planned with another compiler')
    units = {}
    for s in manifest['shards']:
        inputs = {'gen/' + k: v for k, v in s['files'].items()}
        for n in s['source_files']:
            inputs['src/' + n] = manifest['sources'][n]
        key, body = unit_key('proof', s['id'], inputs, tool)
        units[s['id']] = (key, body)
    return units, manifest


def receipt_passes(receipt, manifest_sha, ps):
    return (receipt.get('state') == 'passed' and receipt.get('manifest_sha256') == manifest_sha
            and receipt.get('inputs_unchanged') is True
            and ps.verdict(receipt.get('exit'), receipt.get('output', '')))


# ---- CLI ----

def main(argv=None):
    p = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    p.add_argument('action', choices=['plan-check', 'record-check', 'plan-proof', 'record-proof', 'audit'])
    p.add_argument('paths', nargs='*')
    p.add_argument('--root', type=Path, default=ROOT)
    p.add_argument('--cache', type=Path)
    p.add_argument('--shared', type=Path, action='append')
    p.add_argument('--all', action='store_true')
    p.add_argument('--plan', type=Path)
    p.add_argument('--shared-write', action='store_true')
    p.add_argument('--receipt', type=Path, action='append', default=[])
    a = p.parse_args(argv)
    root = a.root.resolve()
    if a.action == 'plan-check':
        units = check_units(root)
        hit, miss = plan(units, not a.all, a.cache, a.shared)
        if a.plan:
            a.plan.write_text(json.dumps({n: k for n, (k, _) in units.items()}, sort_keys=True))
        print(json.dumps({'hit': hit, 'miss': miss}))
    elif a.action == 'record-check':
        if not a.paths or not a.plan:
            raise CacheError('record-check needs the modules that passed and --plan')
        planned = json.loads(a.plan.read_text())
        now = check_units(root, a.paths)
        for name, (key, body) in now.items():
            if planned.get(name) != key:
                raise CacheError('inputs of %s changed since they were planned; nothing recorded' % name)
        for name, (key, body) in now.items():
            record(key, body, a.cache, a.shared_write, a.shared)
    elif a.action == 'plan-proof':
        if len(a.paths) != 1:
            raise CacheError('plan-proof needs the partition folder')
        units, _ = shard_units(root, Path(a.paths[0]).resolve())
        hit, miss = plan(units, not a.all, a.cache, a.shared)
        print(json.dumps({'hit': hit, 'miss': miss}))
    elif a.action == 'record-proof':
        if len(a.paths) != 1 or not a.receipt:
            raise CacheError('record-proof needs the partition folder and receipts')
        folder = Path(a.paths[0]).resolve()
        units, manifest = shard_units(root, folder)
        ps = _module('proof-shards.py')
        msha = ps.sha(ps.encoded(manifest))
        for r in a.receipt:
            receipt = json.loads(r.read_text())
            name = receipt.get('shard')
            if name not in units or not receipt_passes(receipt, msha, ps):
                raise CacheError('receipt is not a pass for this partition: %s' % r)
            record(units[name][0], units[name][1], a.cache, a.shared_write, a.shared)
    else:
        bad = []
        for f in sorted((a.cache or local_dir()).glob('*/*.json')):
            try:
                ok = valid_entry(json.loads(f.read_text()), f.stem)
            except (OSError, ValueError):
                ok = False
            if not ok:
                bad.append(str(f))
        print(json.dumps({'invalid': bad}))
        return 1 if bad else 0
    return 0


if __name__ == '__main__':
    try:
        raise SystemExit(main())
    except (CacheError, ValueError, OSError, subprocess.CalledProcessError) as error:
        print('content-cache: %s' % error, file=sys.stderr)
        raise SystemExit(2)
