#!/usr/bin/env python3
"""Content-addressed results for Bend module checks and proof shards.

A unit (a module's type-check, a proof shard) is keyed by the SHA-256 of
everything it reads: the unit, its full dependency closure (every imported
.bend, effect file and local header, by content; a quoted include that is
missing makes the unit uncacheable), the compiler/library identity, the
environment bend reads, and the verdict logic.

Trust boundary. An entry is a pass only if it carries a MAC, made with a
secret key that lives beside the local cache (mode 0600, created on first
use), over its key and body. Only the admitted `run-check` and the owned
`proof-cache.py run` execution record passes, after Bend and the existing
verdict policy succeed; no command imports a caller-declared pass. The boundary is therefore
the local user account that owns the cache folder: anyone who can read that
key can forge entries, and entries copied from another cache (another key) are
misses. There is NO shared or CI-seeded cache yet: a shared cache needs
signatures that readers can verify without being able to sign (asymmetric,
written only by trusted main builds), and nothing here claims to provide that.

  content-cache.py plan-check [--cache D] [--all] [--plan P]
      JSON: {"hit": [...], "miss": [...], "uncacheable": {...}}. --plan P
      saves the keys as taken BEFORE the check runs, and the misses.
  content-cache.py run-check --plan P [--cache D]
      Standalone PRE-CHECK ONLY: exit3 even when every entry passes. Does not
      establish check.sh's aggregate verdict; cross-module fills/templates can
      make that verdict different. Needs inherited heavy-guard admission (via
      scripts/cached-check.sh); re-reads the tool identity before and after
      bend runs; check each miss as its exact root in a fresh process under
      the verdict policy, then record only those whose key is still the
      planned one. LAWS.bend counts only on a clean verdict (a TODO or any
      other line fails); scripts/check.sh stays the gate for a LAWS with TODO.
  content-cache.py plan-proof PARTITION_DIR [--cache D] [--all]
      proof shards of a verified canonical partition (planning only).
  scripts/proof-cache.py plan|run|join
      one original missing shard per admitted execution, then a complete
      cached/fresh join. Only the owned worker execution records proof passes.
      The old record-proof receipt import is refused: JSON is not execution.
  content-cache.py audit [--cache D]
      every entry must be whole and authentic; prints the others

--all ignores cached standalone entries. Nightly gates must still run the
original aggregate check and full proof suite; this pre-check is not a gate.
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
import resource
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
    execution = {'schema': 2, 'process': 'fresh-bend-entry', 'cwd': 'repository-root',
                 'python_sha256': sha(Path(sys.executable).resolve().read_bytes()),
                 'stack_limit': list(resource.getrlimit(resource.RLIMIT_STACK)),
                 'jsc': {name: os.environ.get(name, '') for name in
                         ('BUN_JSC_forceRAMSize', 'BUN_JSC_maxPerThreadStackUsage')}}
    units = {}
    for f in files if files is not None else tracked_modules(root):
        try:
            # A successful aggregate manifest is NOT an independent module
            # pass: earlier fills/template instances can affect later checks.
            # Each key names the exact entry used in a fresh checker process.
            entry_tool = {**tool, 'check_execution': {**execution, 'argv': ['bend', f, '--check-only']}}
            key, body = unit_key('check-entry-v2', f, tree.closure(f), entry_tool)
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
                # Legacy check passes came from a different aggregate context.
                # They are invalid even with an intact same-user MAC.
                and entry['body'].get('gate') != 'check'
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
    """Internal: run-check and the owned proof-cache worker call this after a pass."""
    if key is None or sha(canonical(body)) != key:
        raise CacheError('record: body does not hash to its key')
    if body.get('gate') == 'check':
        raise CacheError('retired aggregate-derived check entries cannot be recorded')
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


def check_plan_fields(planned):
    """Old, partial or malformed plans cannot authorize a skip."""
    if (not isinstance(planned, dict) or type(planned.get('schema')) is not int
            or planned['schema'] != 1 or not isinstance(planned.get('keys'), dict)):
        raise CacheError('check plan needs schema1, full modules, keys and miss list; re-plan')
    for field in ('modules', 'miss'):
        values = planned.get(field)
        if (not isinstance(values, list) or not all(isinstance(n, str) for n in values)
                or len(values) != len(set(values))):
            raise CacheError('check plan has invalid/duplicate ' + field)
    if (set(planned['modules']) != set(planned['keys'])
            or not set(planned['miss']) <= set(planned['modules'])):
        raise CacheError('check plan module/key/miss inventories differ')


def validate_check_plan(root, planned, local=None):
    """Re-read the complete inventory, keys and MACs, including claimed hits.

    A plan describes a snapshot, not permission to trust old lookup results.
    Forced misses (--all) are allowed; every omitted unit needs a current pass.
    """
    check_plan_fields(planned)
    reset_tool_identity()
    units = check_units(root)
    if (set(units) != set(planned['modules'])
            or {n: k for n, (k, _) in units.items()} != planned['keys']):
        raise CacheError('check plan module inventory, inputs or tools changed; re-plan')
    misses = set(planned['miss'])
    for name, (key, _) in units.items():
        if name not in misses and (key is None or not lookup(key, local)):
            raise CacheError('check plan hit is missing or unauthentic: ' + name)
    return units


def run_check(root, plan_file, local=None):
    """Check misses as separate exact entries, never in an aggregate context."""
    planned = json.loads(Path(plan_file).read_text())
    check_plan_fields(planned)
    todo = list(planned['miss'])  # an uncacheable module is checked, never recorded
    if not todo:
        validate_check_plan(root, planned, local)
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
    validate_check_plan(root, planned, local)
    env = {**os.environ, 'BEND_NO_TELEMETRY': '1'}
    passed, failed = [], False
    for module in todo:
        # Every process starts with exactly this root's import/fill/template
        # context. No preceding miss or cached module can supply extra state.
        r = subprocess.run(['bend', module, '--check-only'], cwd=root, env=env,
                           capture_output=True, text=True)
        out = r.stdout + r.stderr
        if not verdict(r.returncode, out):  # TODO remains no pass, including LAWS
            print(out, file=sys.stderr)
            failed = True
            break
        passed.append(module)
    # Revalidate ALL units and hit MACs before recording any new entry.
    now = validate_check_plan(root, planned, local)
    for module in passed:
        if now[module][0] is not None:
            _record(now[module][0], now[module][1], local)
    return 1 if failed else 0


# ---- proof shards ----

def shard_units(root, folder):
    root, folder = Path(root).resolve(), Path(folder).resolve()
    ps = _module('proof-shards.py')
    # Re-derive the entire canonical partition before trusting ANY cached pass.
    # The originals remain coverage/provenance inputs, but are execution inputs
    # only when a generated shard actually imports them.
    manifest = ps.verify(root, folder)
    tool = tool_identity()
    if manifest.get('toolchain') != tool['toolchain']:
        raise CacheError('partition was planned with another compiler')
    proof_tool = {**tool, 'partition_logic': {
        name: manifest[name] for name in
        ('planner_sha256', 'verdict_sha256', 'guard_sha256')},
        # proof-shards.py uses this helper for its independent root inventory.
        'closure_sha256': sha((ROOT/'scripts/bend-closure.py').read_bytes()),
        'canonical_modes': {n: manifest['source_modes'][n]
                            for n in ('LAWS.bend', 'PROOF.bend')},
        # Invalidate legacy entries made by importing caller-declared JSON.
        'proof_cache_execution': {'schema': 'owned-original-worker-v1',
                                 'sha256': sha((ROOT/'scripts/proof-cache.py').read_bytes())}}
    tree = Tree(root)
    units = {}
    for s in manifest['shards']:
        at = folder/s['id']
        entry = (at/'PROOF.bend').relative_to(root).as_posix()
        inputs = {}
        for name, digest in tree.closure(entry).items():
            path = root/name
            ps.regular(root, path)
            if at in path.parents:
                generated = path.relative_to(folder).as_posix()
                if s['files'].get(generated) != digest:
                    raise CacheError('unexpected generated proof input: ' + name)
                label = 'gen/' + path.relative_to(at).as_posix()
            else:
                if manifest['sources'].get(name) != digest:
                    raise CacheError('proof input absent from verified inventory: ' + name)
                label = 'src/' + name
            inputs[label] = {'sha256': digest, 'mode': ps.portable_mode(path)}
        # Numeric slots can move when a different law changes. Bind logical
        # membership and exact executable closure instead of the numeric slot.
        membership = {'primary_laws': s['primary_laws'], 'declarations': s['declarations']}
        key, body = unit_key('proof', sha(canonical(membership)), inputs, proof_tool)
        units[s['id']] = (key, body)
    return units, manifest


def receipt_passes(receipt, manifest_sha, ps):
    # Legacy format check only. This never authenticates execution or records a
    # pass; the former caller-supplied receipt import command is now refused.
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
            a.plan.write_text(json.dumps({'schema': 1, 'modules': sorted(units),
                                         'keys': {n: k for n, (k, _) in units.items()},
                                         'miss': miss}, sort_keys=True))
        print(json.dumps({'kind': 'standalone-module-plan', 'gate_qualified': False, 'hit': hit, 'miss': miss,
                          'uncacheable': {n: b for n, (k, b) in units.items() if k is None}}))
    elif a.action == 'run-check':
        if not a.plan:
            raise CacheError('run-check needs --plan')
        status = run_check(root, a.plan, a.cache)
        print(json.dumps({'kind': 'standalone-module-precheck',
                          'status': 'pass-standalone' if status == 0 else 'inconclusive-standalone',
                          'gate_qualified': False, 'next': 'scripts/check.sh aggregate gate remains required'}))
        return 3 if status == 0 else 4  # Scoped pass/failure, never aggregate green.
    elif a.action == 'plan-proof':
        if len(a.paths) != 1:
            raise CacheError('plan-proof needs the partition folder')
        units, _ = shard_units(root, Path(a.paths[0]).resolve())
        hit, miss = plan(units, not a.all, a.cache)
        print(json.dumps({'hit': hit, 'miss': miss}))
    elif a.action == 'record-proof':
        raise CacheError('record-proof cannot import caller-declared passes; use proof-cache.py run')
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
