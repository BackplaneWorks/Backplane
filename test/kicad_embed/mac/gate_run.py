#!/usr/bin/python3
"""Run one command in h's GUI session (a one-shot LaunchAgent in gui/UID) and wait for it: the Mac mini gate's harnesses start GUI
apps (the client, KiCad through the hub), which a process of an ssh session cannot show. Run over ssh, as h:
    gate_run.py LOG [--env K=V ...] -- PROGRAM ARGS...
Its output goes to LOG; the agent is booted out and its plist removed when it ends (or on Ctrl-C / SIGTERM). Exit code: the command's.
"""
import os
import plistlib
import signal
import subprocess
import sys
import time

LABEL = 'dev.backplane.gate-run'
PLIST = os.path.expanduser(f'~/Library/LaunchAgents/{LABEL}.plist')
DOMAIN = f'gui/{os.getuid()}'


def down():
    subprocess.run(['launchctl', 'bootout', f'{DOMAIN}/{LABEL}'], capture_output=True)
    if os.path.exists(PLIST):
        os.unlink(PLIST)


def state():
    r = subprocess.run(['launchctl', 'print', f'{DOMAIN}/{LABEL}'], capture_output=True, text=True)
    if r.returncode:
        return None, None
    st = code = None
    for raw in r.stdout.splitlines():
        # the job's own lines are indented once; nested sections (its endpoints, spawn) have their own "state = active"
        if not raw.startswith('\t') or raw.startswith('\t\t'):
            continue
        line = raw.strip()
        if line.startswith('state = ') and st is None:
            st = line.split('= ', 1)[1]
        elif line.startswith('last exit code = ') and code is None:
            code = line.split('= ', 1)[1]
    return st, code


# the person's own KiCad folders: a gate run must leave them exactly as they were (KiCad on macOS reads the account's real home, not
# $HOME: a run without KICAD_CONFIG_HOME seeded the person's 10.99 tables once, Nov 16)
GUARDED = [os.path.expanduser('~/Library/Preferences/kicad'), os.path.expanduser('~/Documents/KiCad')]


def snapshot():
    out = {}
    for root in GUARDED:
        for dp, dns, fns in os.walk(root):
            for n in dns + fns:
                p = os.path.join(dp, n)
                try:
                    st = os.lstat(p)
                    out[p] = (st.st_size if n in fns else -1, int(st.st_mtime))
                except OSError:
                    pass
    return out


def main():
    a = sys.argv[1:]
    log, a = a[0], a[1:]
    env = {'PATH': '/usr/bin:/bin:/usr/sbin:/sbin:/opt/homebrew/bin'}
    while a and a[0] == '--env':
        k, v = a[1].split('=', 1)
        env[k] = v
        a = a[2:]
    if a and a[0] == '--':
        a = a[1:]
    before = snapshot()
    down()  # a run left behind by an interrupted gate
    os.makedirs(os.path.dirname(PLIST), exist_ok=True)
    with open(PLIST, 'wb') as f:
        plistlib.dump({'Label': LABEL, 'ProgramArguments': a, 'EnvironmentVariables': env, 'RunAtLoad': True,
                       'ProcessType': 'Interactive', 'StandardOutPath': log, 'StandardErrorPath': log,
                       'WorkingDirectory': os.path.expanduser('~')}, f)
    signal.signal(signal.SIGTERM, lambda *_: (down(), sys.exit(143)))
    try:
        r = subprocess.run(['launchctl', 'bootstrap', DOMAIN, PLIST], capture_output=True, text=True)
        if r.returncode:
            print(f'bootstrap failed: {r.stderr.strip()}', file=sys.stderr)
            return 2
        time.sleep(2)
        # launchd says "active" or "running" while it runs (and "spawn scheduled" before); done once it has an exit code
        while True:
            st, code = state()
            if st is None or (st not in ('running', 'active', 'spawn scheduled') and code not in (None, '(never exited)')):
                break
            time.sleep(3)
        print(f'{LABEL}: {st}, last exit code {code}', flush=True)
        after = snapshot()
        changed = sorted(k for k in set(before) | set(after) if before.get(k) != after.get(k))
        if changed:
            print('GUARD FAIL: the person\'s own KiCad folders changed during the run: ' + '; '.join(
                f'{k} {before.get(k)} -> {after.get(k)}' for k in changed[:20]), flush=True)
            return 3
        print(f'guard: the person\'s KiCad folders unchanged ({len(after)} entries)', flush=True)
        try:
            return int(str(code).split()[0])
        except (TypeError, ValueError):
            return 1
    finally:
        down()


if __name__ == '__main__':
    sys.exit(main())
