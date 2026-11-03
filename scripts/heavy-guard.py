#!/usr/bin/env python3
"""Serialize heavy work with Backplane's bend flock and require RAM headroom."""
import fcntl
import os
from pathlib import Path
import sys
import subprocess

GIB = 1024 ** 3
# Expected peak plus an execution margin, not process limits. Local jobs also
# leave host headroom; Boat jobs are isolated and use their own capacity.
FLOORS = {'check': 14 * GIB, 'proof': 19 * GIB, 'test': 14 * GIB,
          'build': 16 * GIB, 'web': 10 * GIB, 'browser': 14 * GIB}
HOST_RESERVE = 8 * GIB
FD_VAR = 'BACKPLANE_HEAVY_LOCK_FD'
BOAT_VAR = 'BACKPLANE_HEAVY_BOAT_SANDBOX'


def host_available():
    fields = {}
    for line in Path('/proc/meminfo').read_text().splitlines():
        parts = line.split()
        if len(parts) >= 2 and parts[0] == 'MemAvailable:':
            fields['available'] = int(parts[1]) * 1024
    if 'available' not in fields:
        raise ValueError('MemAvailable is unavailable')
    return fields['available']


def available():
    fields = {'available': host_available()}
    cgroup_root = Path('/sys/fs/cgroup')
    if (cgroup_root / 'cgroup.controllers').exists():
        groups = Path('/proc/self/cgroup').read_text().splitlines()
        group = next((line.split(':', 2)[2] for line in groups if line.startswith('0::')), None)
        if group is None or '..' in Path(group).parts:
            raise ValueError('cannot locate cgroup v2')
        directory = cgroup_root / group.lstrip('/')
        if not directory.is_dir():
            raise ValueError('cgroup directory is unavailable')
        while directory == cgroup_root or cgroup_root in directory.parents:
            limit, current = directory / 'memory.max', directory / 'memory.current'
            if limit.exists() or current.exists():
                max_text, current_text = limit.read_text().strip(), current.read_text().strip()
                if max_text != 'max':
                    fields['available'] = min(fields['available'], max(0, int(max_text) - int(current_text)))
            if directory == cgroup_root:
                break
            directory = directory.parent
    else:
        v1_limit = cgroup_root / 'memory/memory.limit_in_bytes'
        v1_current = cgroup_root / 'memory/memory.usage_in_bytes'
        if v1_limit.exists() or v1_current.exists():
            maximum = int(v1_limit.read_text().strip())
            used = int(v1_current.read_text().strip())
            fields['available'] = min(fields['available'], max(0, maximum - used))
    return fields['available']


def lock_path():
    # The hub's bend guard uses this same host-wide inode. Named-lock and
    # fixture-directory overrides must not create separate heavy-work slots.
    return Path('/tmp/bp-bend.lock')


def inherited_lock(path):
    try:
        fd = int(os.environ[FD_VAR])
        if os.fstat(fd).st_ino != path.stat().st_ino or os.fstat(fd).st_dev != path.stat().st_dev:
            return None
        fcntl.flock(fd, fcntl.LOCK_EX | fcntl.LOCK_NB)
        return fd
    except (KeyError, OSError, ValueError):
        return None


def admit(profile):
    try:
        host = host_available()
        free = available()
    except (OSError, ValueError) as error:
        print('heavy-guard: cannot establish available memory: ' + str(error), file=sys.stderr)
        return False
    if free < FLOORS[profile]:
        print(f'heavy-guard: {profile} requires {FLOORS[profile] / GIB:.0f} GiB available; {free / GIB:.2f} GiB available', file=sys.stderr)
        return False
    in_boat = os.environ.get(BOAT_VAR) == '1' and os.environ.get('BACKPLANE_HEAVY_OFFLOADED') == '1'
    if not in_boat and host < FLOORS[profile] + HOST_RESERVE:
        print(f'heavy-guard: {profile} needs {FLOORS[profile] / GIB:.0f} GiB expected peak plus {HOST_RESERVE / GIB:.0f} GiB host reserve; {host / GIB:.2f} GiB host available', file=sys.stderr)
        return False
    return True


def offload(command, env):
    # Only source-controlled entry points with reproducible, declared outputs
    # are eligible. Never upload a dirty/untracked workspace or arbitrary argv.
    if env.get('BACKPLANE_HEAVY_OFFLOADED'):
        print('heavy-guard: remote memory/lock admission failed; no recursive offload', file=sys.stderr)
        return 75
    root = Path(__file__).resolve().parent.parent
    allowed = {'scripts/check.sh', 'scripts/test.sh', 'scripts/build-js.sh', 'scripts/build-mobile.sh', 'scripts/dev.sh'}
    try:
        relative = Path(command[0]).resolve().relative_to(root).as_posix()
    except ValueError:
        relative = command[0]
    if relative not in allowed:
        print('heavy-guard: command cannot be safely transferred; use a reviewed Boat packet', file=sys.stderr)
        return 75
    try:
        state = subprocess.run(['git', 'status', '--porcelain', '--untracked-files=all'], cwd=root, capture_output=True, text=True, check=True)
    except (OSError, subprocess.CalledProcessError) as error:
        print('heavy-guard: cannot verify source before Boat: ' + str(error), file=sys.stderr)
        return 75
    if state.stdout:
        print('heavy-guard: source is not clean; refuse automatic Boat transfer', file=sys.stderr)
        return 75
    if any(env.get(name) not in (None, '', '0', 'off') for name in ('BACKPLANE_TELEMETRY_BAKE', 'BACKPLANE_GOOGLE_BAKE')):
        print('heavy-guard: refuse automatic transfer with baked credentials', file=sys.stderr)
        return 75
    artifacts = []
    if relative == 'scripts/dev.sh':
        if command[1:] != ['web']:
            print('heavy-guard: only dev.sh web has a declared portable output', file=sys.stderr)
            return 75
        artifacts = ['--artifact', 'build/dev/web']
    elif relative == 'scripts/build-js.sh':
        if len(command) != 3:
            print('heavy-guard: build output is not explicit', file=sys.stderr)
            return 75
        output = Path(command[2])
        if output.is_absolute() or '..' in output.parts:
            print('heavy-guard: build output must be inside the checkout', file=sys.stderr)
            return 75
        artifacts = ['--artifact', str(output)]
    elif relative == 'scripts/build-mobile.sh':
        if command[1:] != ['--js']:
            print('heavy-guard: automatic Boat transfer supports only build-mobile.sh --js', file=sys.stderr)
            return 75
        artifacts = ['--artifact', 'mobile/build/assets/bridge.js']
    settings = []
    for name in ('TEST_FILES', 'TEST_FULL', 'TEST_SHARD', 'TEST_SHARD_COUNT'):
        if name in env:
            settings.append(name + '=' + env[name])
    runner = str(root / 'scripts/boat-job.sh')
    argv = [runner, '--size', 'large', '--timeout', '1200', '--wait', '0', '--idle', '0', '--no-reuse', *artifacts,
            '--', 'env', *settings, 'scripts/heavy-boat.sh', relative, *command[1:]]
    print('heavy-guard: local admission unavailable; starting a disposable Boat job', file=sys.stderr, flush=True)
    os.chdir(root)
    try:
        os.execvpe(runner, argv, env)
    except OSError as error:
        print('heavy-guard: cannot start Boat: ' + str(error), file=sys.stderr)
        return 75


def main(args):
    held_only = bool(args and args[0] == '--held')
    if held_only:
        args = args[1:]
    if not args or args[0] not in FLOORS:
        print('usage: heavy-guard.py [--held] {check|proof|test|build|web|browser} [-- command ...]', file=sys.stderr)
        return 2
    profile, rest = args[0], args[1:]
    path = lock_path()
    fd = inherited_lock(path)
    owned = fd is not None
    if held_only:
        if fd is None:
            return 1
        return 0 if admit(profile) else 75
    if len(rest) < 2 or rest[0] != '--':
        return 2
    if fd is None:
        try:
            fd = os.open(path, os.O_CREAT | os.O_RDWR | getattr(os, 'O_NOFOLLOW', 0), 0o666)
            fcntl.flock(fd, fcntl.LOCK_EX | fcntl.LOCK_NB)
        except OSError as error:
            print('heavy-guard: bend lock unavailable: ' + str(error), file=sys.stderr)
            return offload(rest[1:], os.environ.copy())
    if not admit(profile):
        if not owned:
            os.close(fd)
            return offload(rest[1:], os.environ.copy())
        return 75
    os.set_inheritable(fd, True)
    env = os.environ.copy()
    env[FD_VAR] = str(fd)
    try:
        os.execvpe(rest[1], rest[1:], env)
    except OSError as error:
        print('heavy-guard: cannot start command: ' + str(error), file=sys.stderr)
        return 126


if __name__ == '__main__':
    sys.exit(main(sys.argv[1:]))
