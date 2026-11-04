#!/usr/bin/env python3
"""Refuse to start the Bend/Bun toolchain on this machine outside an approved capped context.

The installed `bend` and `bun` (and `bunx`) paths are replaced by a small shim; the real binary is renamed to
`<name>.real` in the SAME directory (the Bun-compiled bend finds its Base library and guide relative to its own
location, so it cannot move elsewhere). Because the shim sits at the ORIGINAL path, a direct absolute-path call such
as /home/USER/.bend/bin/bend or /home/USER/.bun/bin/bun is covered, not only PATH lookups. The shim runs the real
binary only when one of these holds, and otherwise exits 75 with a message:

  * light, non-compiling subcommands (bend version/help/guide/base/update, bun --version/--revision/--help);
  * a Boat sandbox: BACKPLANE_HEAVY_BOAT_SANDBOX=1 and BACKPLANE_HEAVY_OFFLOADED=1 on a machine whose MemTotal is
    at most 8 GiB (the isolated-VM rule scripts/proof-shards.py uses) AND this process's own cgroup has a finite
    memory.max <= 3,700,000,000 B and swap.max 0 (read from the kernel; Boat needs no local lock, so runs stay
    parallel);
  * a capped local context: the actual native bend flock, genuinely held through the inherited descriptor
    (BACKPLANE_HEAVY_LOCK_FD on /tmp/bp-bend.lock), AND the same kernel-read cgroup cap.

Commands:  install [--apply] [--tool bend|bun ...] [--home DIR]   (dry run unless --apply)
           uninstall [--apply] ...    check [...]    shim NAME REAL INVOKED -- ARGS...   (called by the shim)

What this CANNOT enforce without operating-system support (root, an LSM such as AppArmor/SELinux, seccomp from a
parent, or a container) is listed in docs/lowmem/compiler-guard.md: direct execution of the relocated real binary or
of any other copy, interpreters that exec a binary by path, other users and services, and reinstalls that bypass
`bend update`. This is a guard against accidental heavy runs, not a sandbox.
"""
import argparse
import hashlib
import json
import os
import shutil
import stat
import sys
import tempfile
from pathlib import Path

GIB = 1024 ** 3
CAP = 3_700_000_000
LOCK = Path('/tmp/bp-bend.lock')
MARK = '# backplane-compiler-guard shim v1'
TOOLS = {'bend': ('.bend/bin/bend',), 'bun': ('.bun/bin/bun',)}
LIGHT = {'bend': {'', 'version', 'guide', 'base', 'help', '--help', '-h', 'update'},
         'bun': {'--version', '-v', '--revision', '--help', '-h'}}
SELF = Path(__file__).resolve()


def guard_home(env=None):
    env = os.environ if env is None else env
    return Path(env.get('BACKPLANE_GUARD_HOME') or Path.home()/'.local/share/backplane/guard')


# ---- the gate -------------------------------------------------------------------------------------------------
def mem_total():
    for line in Path('/proc/meminfo').read_text().splitlines():
        if line.startswith('MemTotal:'):
            return int(line.split()[1]) * 1024
    raise ValueError('MemTotal unavailable')


def boat(env):
    if env.get('BACKPLANE_HEAVY_BOAT_SANDBOX') != '1' or env.get('BACKPLANE_HEAVY_OFFLOADED') != '1':
        return False
    try:
        return mem_total() <= 8 * GIB
    except (OSError, ValueError):
        return False


def own_cgroup():
    groups = Path('/proc/self/cgroup').read_text().splitlines()
    group = next((line.split(':', 2)[2] for line in groups if line.startswith('0::')), None)
    root = Path('/sys/fs/cgroup')
    if group is None or '..' in Path(group).parts or not (root/'cgroup.controllers').exists():
        raise ValueError('cannot locate cgroup v2')
    return root/group.lstrip('/')


def capped():
    try:
        directory = own_cgroup()
        limit = (directory/'memory.max').read_text().strip()
        swap = (directory/'memory.swap.max').read_text().strip()
        return limit.isdigit() and 0 < int(limit) <= CAP and swap == '0'
    except (OSError, ValueError):
        return False


def native_lock(env):
    """The descriptor names the lock file AND its open file description really holds the flock.

    Opening the lock file is not holding it: a fresh open file description must be refused a non-blocking
    exclusive lock (someone holds it), while the inherited descriptor must still be able to take it (it is the
    holder). An opened but unlocked descriptor fails the first test (the new description gets the lock)."""
    import fcntl
    probe = None
    try:
        fd = int(env['BACKPLANE_HEAVY_LOCK_FD'])
        info, path = os.fstat(fd), LOCK.stat()
        if (info.st_ino, info.st_dev) != (path.st_ino, path.st_dev):
            return False
        probe = os.open(str(LOCK), os.O_RDWR | getattr(os, 'O_NOFOLLOW', 0))
        try:
            fcntl.flock(probe, fcntl.LOCK_EX | fcntl.LOCK_NB)
            return False                      # nobody held it: the inherited descriptor is not a lock holder
        except BlockingIOError:
            pass
        fcntl.flock(fd, fcntl.LOCK_EX | fcntl.LOCK_NB)      # held by this description: re-locking succeeds
        return True
    except (KeyError, OSError, ValueError):
        return False
    finally:
        if probe is not None:
            os.close(probe)


def light(name, args):
    first = args[0] if args else ''
    return first in LIGHT.get(name, set())


def decide(name, args, env):
    """('allow'|'refuse', reason)."""
    if light(name, args):
        return 'allow', 'light subcommand'
    if boat(env) and capped():
        return 'allow', 'Boat sandbox with a kernel-read cap'
    if native_lock(env) and capped():
        return 'allow', 'capped local context with the native flock'
    return 'refuse', ('%s refused: not in an approved capped context. Run it on Boat inside a scope with memory.max <= %d '
                      'and swap 0 (scripts/heavy-boat.sh does this), or through scripts/heavy-guard.py inside a verified cgroup.' % (name, CAP))


def shim(name, real, invoked, args, env=None):
    env = dict(os.environ if env is None else env)
    verdict, why = decide(name, args, env)
    if verdict == 'refuse':
        print('backplane-compiler-guard: ' + why, file=sys.stderr)
        return 75
    real = str(real)
    if not os.access(real, os.X_OK):
        print('backplane-compiler-guard: real binary missing or not executable: ' + real, file=sys.stderr)
        return 127
    if name == 'bend' and args[:1] == ['update']:
        code = os.spawnve(os.P_WAIT, real, [invoked, *args], env)   # `bend update` may replace the shimmed path
        reapply(name, env)
        return code
    os.execve(real, [invoked, *args], env)


# ---- install / uninstall / check ------------------------------------------------------------------------------------
def sha(path):
    h = hashlib.sha256()
    with open(path, 'rb') as f:
        for chunk in iter(lambda: f.read(1 << 20), b''):
            h.update(chunk)
    return h.hexdigest()


def is_shim(path):
    try:
        with open(path, 'rb') as f:
            return f.read(256).decode('utf8', 'replace').find(MARK) >= 0 and not os.path.islink(path)
    except OSError:
        return False


def shim_text(name, real, guard):
    return '#!/bin/sh\n%s\nexec python3 %s shim %s %s "$0" -- "$@"\n' % (MARK, shlex_quote(guard), name, shlex_quote(real))


def shlex_quote(value):
    import shlex
    return shlex.quote(str(value))


def manifest_path(home_guard):
    return home_guard/'manifest.json'


def load(home_guard):
    try:
        return json.loads(manifest_path(home_guard).read_text())
    except (OSError, ValueError):
        return {}


def plan(home, tools, env=None, paths=None):
    """Where each tool lives. `paths` ({name: path}) overrides the default; the manifest remembers earlier choices.
    The real binary is renamed beside the shim, never moved: bend finds its Base library relative to itself."""
    home, state = Path(home), load(guard_home(env))
    items = []
    for name in tools:
        path = Path((paths or {}).get(name) or state.get(name, {}).get('path') or home/TOOLS[name][0])
        items.append({'name': name, 'path': path, 'real': path.with_name(path.name + '.real')})
    return items


def move(src, dst):
    dst.parent.mkdir(parents=True, exist_ok=True)
    try:
        os.replace(src, dst)
    except OSError:
        shutil.copy2(src, dst)
        if sha(src) != sha(dst):
            dst.unlink()
            raise
        os.unlink(src)


def install(home, tools, apply, env=None, paths=None):
    hg = guard_home(env)
    state = load(hg)
    actions = []
    for item in plan(home, tools, env, paths):
        name, path, real = item['name'], item['path'], item['real']
        if not path.exists() and not path.is_symlink():
            actions.append((name, 'absent: nothing to guard at %s' % path))
            continue
        if path.is_symlink():
            actions.append((name, 'symlink %s -> %s left alone (resolves through the guarded path or is out of scope)' % (path, os.readlink(path))))
            continue
        if is_shim(path):
            actions.append((name, 'already guarded'))
            continue
        actions.append((name, 'move %s -> %s and write shim' % (path, real)))
        if apply:
            mode = stat.S_IMODE(path.stat().st_mode)
            digest = sha(path)
            hg.mkdir(parents=True, exist_ok=True)
            move(path, real)
            os.chmod(real, mode | 0o111)
            guard = hg/'compiler-guard.py'
            if not guard.exists() or sha(guard) != sha(SELF):
                shutil.copyfile(SELF, guard)        # a stable copy: worktrees come and go
                os.chmod(guard, 0o644)
            text = shim_text(name, real, guard)
            fd, tmp = tempfile.mkstemp(dir=str(path.parent), prefix='.guard-')
            with os.fdopen(fd, 'w') as f:
                f.write(text)
            os.chmod(tmp, 0o755)
            os.replace(tmp, path)
            state[name] = {'path': str(path), 'real': str(real), 'real_sha256': digest, 'mode': mode}
    if apply:
        hg.mkdir(parents=True, exist_ok=True)
        manifest_path(hg).write_text(json.dumps(state, indent=1, sort_keys=True) + '\n')
    return actions


def uninstall(home, tools, apply, env=None, paths=None):
    hg = guard_home(env)
    state = load(hg)
    actions = []
    for item in plan(home, tools, env, paths):
        name, path, real = item['name'], item['path'], item['real']
        if not is_shim(path):
            actions.append((name, 'not guarded'))
            continue
        want = state.get(name, {})
        if not real.is_file() or want.get('real_sha256') != sha(real):
            actions.append((name, 'REFUSED: real binary missing or changed; not restoring'))
            continue
        actions.append((name, 'restore %s -> %s' % (real, path)))
        if apply:
            os.replace(real, path) if real.parent.stat().st_dev == path.parent.stat().st_dev else move(real, path)
            os.chmod(path, want.get('mode', 0o755))
            state.pop(name, None)
    if apply:
        manifest_path(hg).write_text(json.dumps(state, indent=1, sort_keys=True) + '\n')
    return actions


def check(home, tools, env=None, paths=None):
    hg = guard_home(env)
    state = load(hg)
    problems = []
    for item in plan(home, tools, env, paths):
        name, path, real = item['name'], item['path'], item['real']
        if not path.exists() and not path.is_symlink():
            continue
        if path.is_symlink():
            continue
        if not is_shim(path):
            problems.append('%s: %s is a real binary, not the guard shim' % (name, path))
        elif not real.is_file() or state.get(name, {}).get('real_sha256') != sha(real):
            problems.append('%s: relocated real binary missing or changed (%s)' % (name, real))
    return problems


def reapply(name, env=None):
    home = Path((env or os.environ).get('HOME') or Path.home())
    install(home, [name], True, env)


def main(argv):
    if argv[:1] == ['shim']:                    # handled by hand: the tool's own arguments must never be parsed here
        if len(argv) < 4 or argv[1] not in TOOLS:
            print('compiler-guard: bad shim call', file=sys.stderr)
            return 2
        rest = argv[4:]
        return shim(argv[1], argv[2], argv[3], rest[1:] if rest[:1] == ['--'] else rest)
    p = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    sub = p.add_subparsers(dest='cmd', required=True)
    for cmd in ('install', 'uninstall', 'check'):
        s = sub.add_parser(cmd)
        s.add_argument('--home', type=Path, default=Path.home())
        s.add_argument('--tool', action='append', choices=sorted(TOOLS))
        s.add_argument('--path', action='append', default=[], metavar='TOOL=PATH', help='guard this installed path instead of the default (e.g. bun=/usr/local/bin/bun)')
        if cmd != 'check':
            s.add_argument('--apply', action='store_true', help='change files (dry run otherwise)')
    a = p.parse_args(argv)
    tools = a.tool or sorted(TOOLS)
    paths = dict(item.split('=', 1) for item in a.path)
    if not set(paths) <= set(TOOLS):
        print('compiler-guard: --path needs TOOL=PATH with TOOL in %s' % sorted(TOOLS), file=sys.stderr)
        return 2
    tools = sorted(set(tools) | set(paths))
    if a.cmd == 'check':
        problems = check(a.home, tools, None, paths)
        for line in problems:
            print(line, file=sys.stderr)
        return 1 if problems else 0
    actions = (install if a.cmd == 'install' else uninstall)(a.home, tools, a.apply, None, paths)
    for name, text in actions:
        print('%s%s: %s' % ('' if a.apply else '[dry run] ', name, text))
    return 0


if __name__ == '__main__':
    sys.exit(main(sys.argv[1:]))
