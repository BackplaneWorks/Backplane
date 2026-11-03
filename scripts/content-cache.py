#!/usr/bin/env python3
"""Content-addressed results for Bend module checks and proof shards.

A unit (a module's type-check, a proof shard) is keyed by the SHA-256 of
everything it reads: the unit, its full dependency closure (every imported
.bend, effect file and local header, by content; a quoted include that is
missing makes the unit uncacheable), the compiler/library identity, the
environment bend reads, and the verdict logic.

Trust boundary. An entry is a pass only if it carries a MAC, made with a
secret key that lives beside the local cache (mode 0600, created on first
use), over its key and body. The only code that records an entry is `run-check`
below, which runs bend itself and applies scripts/bendok.sh's verdict; there
is no command that records a caller-declared pass. The boundary is therefore
the local user account that owns the cache folder: anyone who can read that
key can forge entries, and entries copied from another cache (another key) are
misses. There is NO shared or CI-seeded cache yet: a shared cache needs
signatures that readers can verify without being able to sign (asymmetric,
written only by trusted main builds), and nothing here claims to provide that.

  content-cache.py plan-check [--cache D] [--all] [--plan P]
      JSON: {"hit": [...], "miss": [...], "uncacheable": {...}}. --plan P
      saves the keys as taken BEFORE the check runs, and the misses.
  content-cache.py run-check --plan P [--cache D]
      needs the inherited heavy-guard lock and admission (it is run by
      scripts/cached-check.sh); re-reads the tool identity before and after
      bend runs; check the plan's misses with bend (one manifest; LAWS.bend alone) under
      the verdict policy, then record only those whose key is still the
      planned one. LAWS.bend counts only on a clean verdict (a TODO or any
      other line fails); scripts/check.sh stays the gate for a LAWS with TODO.
  content-cache.py plan-proof PARTITION_DIR [--cache D] [--all]
  content-cache.py record-proof PARTITION_DIR --receipt R...
      proof shards of a verified canonical partition; a receipt is recorded
      only if it is a passing receipt for that partition. Receipts are read
      from trusted same-user inputs; this code does not authenticate them
      (the worker that wrote one is not proven here)
  content-cache.py audit [--cache D]
      every entry must be whole and authentic; prints the others

--all ignores the cache for planning (what a nightly full run would use; no
nightly job runs these scripts yet).
Local cache: $BP_CONTENT_CACHE or ~/.cache/backplane-bend/content.
"""
import argparse
import hashlib
import hmac
import importlib.util
import json
import os
from pathlib import Path
import re
import shutil
import subprocess
import sys
import tempfile
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


class Uncacheable(CacheError):
    """A unit whose inputs cannot be fully named: always a miss, never recorded."""


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
                if not (root/p).exists():
                    # generated or system header, or one not made yet: either
                    # way its bytes are not an input we can name
                    raise Uncacheable('%s: quoted include not in the checkout: %s' % (f, p))
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
        try:
            key, body = unit_key('check', f, tree.closure(f), tool)
        except Uncacheable as error:
            units[f] = (None, str(error))
            continue
        units[f] = (key, body)
    return units


# ---- store ----

def local_dir():
    return Path(os.environ.get('BP_CONTENT_CACHE') or
                Path(os.environ.get('XDG_CACHE_HOME') or Path.home()/'.cache')/'backplane-bend'/'content')


def entry_path(folder, key):
    return Path(folder)/key[:2]/(key + '.json')


def secret(folder, create):
    path = Path(folder)/'.mac-key'
    try:
        data = path.read_bytes()
        if len(data) == 32:
            return data
    except OSError:
        pass
    if not create:
        return None
    path.parent.mkdir(parents=True, exist_ok=True)
    fd = os.open(path, os.O_WRONLY | os.O_CREAT | os.O_EXCL, 0o600)
    with os.fdopen(fd, 'wb') as f:
        f.write(os.urandom(32))
    return path.read_bytes()


def mac(secret_key, key, body):
    return hmac.new(secret_key, key.encode() + b'\0' + canonical(body), hashlib.sha256).hexdigest()


def valid_entry(entry, key, secret_key):
    """A pass only if whole, naming exactly this key, and made with this cache's secret."""
    try:
        return (secret_key is not None and isinstance(entry, dict) and entry.get('schema') == SCHEMA
                and entry.get('verdict') == 'passed' and entry.get('key') == key
                and isinstance(entry.get('body'), dict)
                and sha(canonical(entry['body'])) == key
                and isinstance(entry.get('mac'), str)
                and hmac.compare_digest(entry['mac'], mac(secret_key, key, entry['body'])))
    except (TypeError, ValueError):
        return False


def lookup(key, local=None):
    """True only for an authentic recorded pass."""
    local = local_dir() if local is None else Path(local)
    k = secret(local, False)
    try:
        entry = json.loads(entry_path(local, key).read_text())
    except (OSError, ValueError):
        return False
    return valid_entry(entry, key, k)


def _write(path, entry):
    path.parent.mkdir(parents=True, exist_ok=True)
    tmp = path.with_name(path.name + '.%d.tmp' % os.getpid())
    tmp.write_text(json.dumps(entry, sort_keys=True))
    os.replace(tmp, path)


def _record(key, body, local=None):
    """Internal: only run-check and record-proof call this, after a real pass."""
    if key is None or sha(canonical(body)) != key:
        raise CacheError('record: body does not hash to its key')
    local = local_dir() if local is None else Path(local)
    entry = {'schema': SCHEMA, 'key': key, 'verdict': 'passed', 'body': body,
             'mac': mac(secret(local, True), key, body), 'recorded': int(time.time())}
    _write(entry_path(local, key), entry)


def plan(units, use_cache=True, local=None):
    hit, miss = [], []
    for name, (key, _) in sorted(units.items()):
        (hit if use_cache and key is not None and lookup(key, local) else miss).append(name)
    return hit, miss


def verdict(code, text):
    """scripts/bendok.sh's policy, exactly: nothing else is a pass."""
    r = subprocess.run(['sh', '-c', '. "$1"; bend_verdict "$2" "$3"', 'verdict',
                        str(ROOT/'scripts/bendok.sh'), str(code), text], capture_output=True)
    return r.returncode == 0


def run_check(root, plan_file, local=None):
    """Check the planned misses with bend and record the ones that pass."""
    planned = json.loads(Path(plan_file).read_text())
    keys, miss = planned['keys'], planned['miss']
    todo = list(miss)  # an uncacheable module is still checked, only never recorded
    if not todo:
        return 0
    # bend starts only inside the shared heavy slot: the actual inherited
    # flock (taken by scripts/heavy-guard.py for scripts/cached-check.sh) and
    # the guard's own memory admission, here too, so a direct call cannot
    # skip either
    guard = _module('heavy-guard.py')
    if guard.inherited_lock(guard.lock_path()) is None:
        raise CacheError('run-check needs the inherited heavy-guard lock (run scripts/cached-check.sh)')
    if not guard.admit('check'):
        raise CacheError('run-check: heavy-guard admission refused')
    # the identity is read again, not remembered: a plan made under another
    # compiler, library or setting is stale, and so is one whose tools change
    # while bend runs (checked again below)
    reset_tool_identity()
    for m, (k, _) in check_units(root, todo).items():
        if k != keys[m]:
            raise CacheError('inputs or tools of %s changed since they were planned; nothing run' % m)
    env = {**os.environ, 'BEND_NO_TELEMETRY': '1'}
    work = Path(tempfile.mkdtemp(prefix='.bend-check.', dir=root))
    try:
        rest = [m for m in todo if m != 'LAWS.bend']
        results = {}
        if rest:
            (work/'m').mkdir()
            for step in ('generate', 'verify'):
                cmd = [sys.executable, str(ROOT/'scripts/check-manifest.py'), step, str(work/'m')]
                if step == 'generate':
                    cmd.append('1')
                subprocess.run(cmd + rest, cwd=root, check=True, capture_output=True)
            r = subprocess.run(['bend', str(work/'m/manifest-0.bend'), '--check-only'], cwd=root,
                               env=env, capture_output=True, text=True)
            out = r.stdout + r.stderr
            if not verdict(r.returncode, out):
                print(out, file=sys.stderr)
                return 1
            results.update({m: True for m in rest})
        if 'LAWS.bend' in todo:
            r = subprocess.run(['bend', 'LAWS.bend', '--check-only'], cwd=root, env=env,
                               capture_output=True, text=True)
            out = r.stdout + r.stderr
            if not verdict(r.returncode, out):  # a TODO is no pass here, and no error is hidden by one
                print(out, file=sys.stderr)
                return 1
            results['LAWS.bend'] = True
        reset_tool_identity()
        now = check_units(root, list(results))
        for m in results:
            if now[m][0] != keys[m]:  # None (uncacheable) on both sides is equal and is not recorded
                raise CacheError('inputs of %s changed since they were planned; nothing recorded' % m)
        for m in results:
            if now[m][0] is not None:
                _record(now[m][0], now[m][1], local)
        return 0
    finally:
        shutil.rmtree(work, ignore_errors=True)


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
    p.add_argument('action', choices=['plan-check', 'run-check', 'plan-proof', 'record-proof', 'audit'])
    p.add_argument('paths', nargs='*')
    p.add_argument('--root', type=Path, default=ROOT)
    p.add_argument('--cache', type=Path)
    p.add_argument('--all', action='store_true')
    p.add_argument('--plan', type=Path)
    p.add_argument('--receipt', type=Path, action='append', default=[])
    a = p.parse_args(argv)
    root = a.root.resolve()
    if a.action == 'plan-check':
        units = check_units(root)
        hit, miss = plan(units, not a.all, a.cache)
        if a.plan:
            a.plan.write_text(json.dumps({'keys': {n: k for n, (k, _) in units.items()}, 'miss': miss}, sort_keys=True))
        print(json.dumps({'hit': hit, 'miss': miss,
                          'uncacheable': {n: b for n, (k, b) in units.items() if k is None}}))
    elif a.action == 'run-check':
        if not a.plan:
            raise CacheError('run-check needs --plan')
        return run_check(root, a.plan, a.cache)
    elif a.action == 'plan-proof':
        if len(a.paths) != 1:
            raise CacheError('plan-proof needs the partition folder')
        units, _ = shard_units(root, Path(a.paths[0]).resolve())
        hit, miss = plan(units, not a.all, a.cache)
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
            _record(units[name][0], units[name][1], a.cache)
    else:
        bad = []
        folder = a.cache or local_dir()
        k = secret(folder, False)
        for f in sorted(folder.glob('*/*.json')):
            try:
                ok = valid_entry(json.loads(f.read_text()), f.stem, k)
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
