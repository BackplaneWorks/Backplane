#!/usr/bin/env python3
"""Serialize heavy work with Backplane's bend flock and require RAM headroom."""
import fcntl
import os
import re
import shutil
from pathlib import Path
import sys
import subprocess

GIB = 1024 ** 3
# Expected peak plus an execution margin, not process limits. Local jobs also
# leave host headroom; Boat jobs are isolated and use their own capacity.
FLOORS = {'check': 14 * GIB, 'proof': 19 * GIB, 'test': 14 * GIB,
          'build': 16 * GIB, 'web': 10 * GIB, 'browser': 14 * GIB,
          # Stamping and Bun linking of exact verified pre-emission only.
          # No Bend fallback is permitted by build-js.sh on this path.
          'compile-js': 3 * GIB,
          # One bounded shard worker only (see worker_contract). Admission is
          # not a cap: the worker must also sit in a verified, no-swap cgroup.
          'proof-shard': 3 * GIB, 'compile-shard': 3 * GIB,
          # sepcomp-fixtures.py run only: same floor as the shard profiles, nothing lowered.
          'fixture-check': 3 * GIB,
          # a closed table of exact `bend FILE --check-only` commands (FILE_CHECK_COMMANDS), nothing else
          'file-check': 3 * GIB}
# Compiler cap: 3.7 GB (decimal). The lane-6 controller (512 MiB) is accounted
# separately; 3.7e9 + 536870912 stays under the 4 GiB combined ceiling.
SHARD_CAP = 3_700_000_000
CONTROLLER_CAP = 536_870_912
SHARD_PROFILES = ('proof-shard', 'compile-shard', 'fixture-check', 'file-check')
# compile-shard runs only these exact compiler invocations ({out} is substituted
# by the worker); nothing else can use the lower floor.
COMPILE_COMMANDS = (
    ('bend', 'src/server/main.bend', '-o', '{out}/hub.js'),
    ('bend', 'src/web/index.html', '-o', '{out}/web'),
    ('bend', 'src/mobile/index.html', '-o', '{out}/mobile-web'),
)
WORKER_OPTIONS = {'--id', '--out', '--cap-bytes', '--timeout', '--protect', '--copy'}
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


def own_cgroup():
    groups = Path('/proc/self/cgroup').read_text().splitlines()
    group = next((line.split(':', 2)[2] for line in groups if line.startswith('0::')), None)
    root = Path('/sys/fs/cgroup')
    if group is None or '..' in Path(group).parts or not (root / 'cgroup.controllers').exists():
        raise ValueError('cannot locate cgroup v2')
    return root / group.lstrip('/')


def verify_shard_cgroup(directory=None):
    """The worker's own cgroup must hold a finite <=3.7 GiB limit and no swap."""
    try:
        directory = directory or own_cgroup()
        limit = (directory / 'memory.max').read_text().strip()
        swap = (directory / 'memory.swap.max').read_text().strip()
        return limit.isdigit() and 0 < int(limit) <= SHARD_CAP and swap == '0'
    except (OSError, ValueError):
        return False


def trusted_python(name):
    """The interpreter must be the one running this guard, never a substitute."""
    found = shutil.which(name) if '/' not in name else name
    try:
        return found is not None and Path(found).resolve() == Path(sys.executable).resolve() and found
    except OSError:
        return False


def shard_run_ok(command):
    """python3 scripts/proof-shards.py run with exactly one --out/--shard/--expect/--result."""
    script = Path(__file__).resolve().parent / 'proof-shards.py'
    if len(command) < 3 or not trusted_python(command[0]) or command[2] != 'run':
        return False
    try:
        if Path(command[1]).resolve() != script:
            return False
    except OSError:
        return False
    return all(command.count(o) == 1 for o in ('--out', '--shard', '--expect', '--result'))


def cached_proof_run_ok(command):
    """One fixed original-shard cache executor, never arbitrary checker argv."""
    if len(command) < 3 or not trusted_python(command[0]) or command[2] != 'run':
        return False
    try:
        if Path(command[1]).resolve() != Path(__file__).resolve().parent / 'proof-cache.py':
            return False
    except (OSError, ValueError):
        return False
    options = command[3:]
    if len(options) % 2:
        return False
    seen = {}
    for name, value in zip(options[::2], options[1::2]):
        if name not in {'--plan', '--shard', '--result', '--root', '--cache'} or name in seen or not value:
            return False
        seen[name] = value
    return {'--plan', '--shard', '--result'} <= set(seen) and re.fullmatch(r'[0-9]{3}', seen['--shard']) is not None


# file-check admits exactly these argv lists, run from the repository root. Adding a command is a reviewed change to
# this table; PROOF.bend, LAWS.bend, a directory or any option such as --verdict is never admitted.
FILE_CHECK_COMMANDS = (
    ('bend', 'test/stream_step_proof.bend', '--check-only'),
)


def file_check_ok(command):
    """Exactly one of FILE_CHECK_COMMANDS, the file a regular non-symlink inside the repo, cwd = the repo root."""
    root = Path(__file__).resolve().parent.parent
    if tuple(command) not in FILE_CHECK_COMMANDS:
        return False
    try:
        target = root/command[1]
        return (Path.cwd().resolve() == root and target.is_file() and not target.is_symlink()
                and target.resolve().relative_to(root).as_posix() == command[1])
    except (OSError, ValueError):
        return False


def fixture_run_ok(command):
    """Exactly `python3 scripts/sepcomp-fixtures.py run`: no other action, option or compiler override."""
    script = Path(__file__).resolve().parent / 'sepcomp-fixtures.py'
    try:
        return len(command) == 3 and bool(trusted_python(command[0])) and Path(command[1]).resolve() == script and command[2] == 'run'
    except OSError:
        return False


def worker_contract(profile, command):
    """Return the argv to exec (interpreter pinned to an absolute path) or None.

    Direct (proof-shard): python3 scripts/proof-shards.py run ...
    Wrapped (both): python3 scripts/lowmem-build.py worker <known options> -- CMD
    with --cap-bytes <= SHARD_CAP, no scope bypass, and CMD limited to the
    profile's one allowed command shape. Unknown options or shapes are refused.
    """
    if profile not in SHARD_PROFILES or not command:
        return None
    if profile == 'file-check':
        return list(command) if file_check_ok(command) else None     # run as `bend` from PATH, argv unchanged
    python = trusted_python(command[0])
    if not python:
        return None
    pinned = [str(Path(sys.executable).resolve()), *command[1:]]
    if profile == 'fixture-check':
        return pinned if fixture_run_ok(command) else None
    if profile == 'proof-shard' and (shard_run_ok(command) or cached_proof_run_ok(command)):
        return pinned
    wrapper = Path(__file__).resolve().parent / 'lowmem-build.py'
    try:
        if len(command) < 4 or Path(command[1]).resolve() != wrapper or command[2] != 'worker' or '--' not in command[3:]:
            return None
    except OSError:
        return None
    split = command.index('--', 3)
    options, inner = command[3:split], command[split + 1:]
    seen = {}
    i = 0
    while i < len(options):
        name = options[i]
        if name not in WORKER_OPTIONS or i + 1 >= len(options):
            return None
        if name != '--protect':
            if name in seen:
                return None
            seen[name] = options[i + 1]
        i += 2
    try:
        if not {'--id', '--out', '--cap-bytes', '--timeout'} <= set(seen) or not 0 < int(seen['--cap-bytes']) <= SHARD_CAP:
            return None
    except ValueError:
        return None
    if profile == 'compile-shard':
        ok = tuple(inner) in COMPILE_COMMANDS
    else:
        ok = shard_run_ok([python, *inner[1:]]) if inner and trusted_python(inner[0]) else False
    return pinned if ok else None


def retained_compile_contract(command):
    """Only build-js.sh may use the retained-emission admission floor.

    The entry point checks the actual bytes/import key before admission and
    again while copying under the native lock. Requiring all retained values
    here ensures that entry point cannot choose its Bend fallback. This is
    command routing, not producer attestation or a replacement for those checks.
    """
    if len(command) != 3 or not command[1] or command[1].startswith('-') or not command[2]:
        return False
    try:
        if Path(command[0]).resolve() != Path(__file__).resolve().parent / 'build-js.sh':
            return False
    except (OSError, ValueError):
        return False
    return bool(os.environ.get('BACKPLANE_PREEMITTED_JS')
                and re.fullmatch(r'[0-9a-f]{64}', os.environ.get('BACKPLANE_PREEMITTED_SHA256', ''))
                and re.fullmatch(r'[0-9a-f]{40}(?:[0-9a-f]{24})?', os.environ.get('BACKPLANE_PREEMITTED_KEY', '')))


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


def local_cap_ok():
    """True when this process's own cgroup is a verified <=3.7e9 B, swap-0 cap (the admission of last resort)."""
    return verify_shard_cgroup()


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
    if not in_boat and profile not in SHARD_PROFILES and not local_cap_ok():
        # Admission is not a cap. A whole-workload profile on this machine would run the compiler uncapped, and the
        # installed bend/bun shims (scripts/compiler-guard.py) refuse that anyway: send it to Boat instead.
        print(f'heavy-guard: {profile} would run uncapped here (own cgroup memory.max must be finite and <= 3700000000 with swap 0); use Boat', file=sys.stderr)
        return False
    return True


def offload(command, env):
    # Only source-controlled entry points with reproducible, declared outputs
    # are eligible. Never upload a dirty/untracked workspace or arbitrary argv.
    if env.get('BACKPLANE_HEAVY_OFFLOADED'):
        print('heavy-guard: remote memory/lock admission failed; no recursive offload', file=sys.stderr)
        return 75
    if any(env.get(name) for name in ('BACKPLANE_PREEMITTED_JS', 'BACKPLANE_PREEMITTED_SHA256', 'BACKPLANE_PREEMITTED_KEY')):
        print('heavy-guard: retained emission needs an explicit verified transport; no automatic source-only offload', file=sys.stderr)
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
        print('usage: heavy-guard.py [--held] {' + '|'.join(FLOORS) + '} [-- command ...]', file=sys.stderr)
        return 2
    profile, rest = args[0], args[1:]
    if profile == 'compile-js':
        command = rest[1:] if rest and rest[0] == '--' else []
        if not retained_compile_contract(command):
            print('heavy-guard: compile-js requires the exact build-js.sh SRC OUT retained-emission command', file=sys.stderr)
            return 2
    if profile in SHARD_PROFILES:
        # Bound to one concrete worker command and a verified cgroup; no --held
        # form (nothing to bind), no Boat/source offload fallback.
        pinned = None if held_only or len(rest) < 2 or rest[0] != '--' else worker_contract(profile, rest[1:])
        if pinned is None:
            print('heavy-guard: ' + profile + ' runs only its exact bounded worker command', file=sys.stderr)
            return 2
        if not verify_shard_cgroup():
            print('heavy-guard: ' + profile + ' needs its own cgroup memory.max <= 3.7 GB (3700000000) and memory.swap.max = 0', file=sys.stderr)
            return 75
        rest = ['--', *pinned]
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
            if profile in SHARD_PROFILES:
                return 75
            return offload(rest[1:], os.environ.copy())
    if not admit(profile):
        if not owned:
            os.close(fd)
            if profile in SHARD_PROFILES:
                return 75
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
