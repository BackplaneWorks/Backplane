#!/usr/bin/env python3
"""Launch an app in one newly owned simulator, capture it, and delete only it."""
import argparse
from datetime import datetime, timezone
import hashlib
import json
import os
from pathlib import Path
import platform
import plistlib
import re
import signal
import subprocess
import sys
import time
import uuid


def version(value):
    parts = [int(n) for n in re.findall(r'\d+', value)]
    return tuple((parts+[0, 0, 0])[:3])


def target(runtimes, devices, sdk):
    # Hosted images can contain newer runtimes than their selected Xcode.
    # Use the active SDK's major/minor instead of an unrelated newest runtime.
    wanted = version(sdk)[:2]
    available = [r for r in runtimes if r.get('isAvailable') is True and '.iOS-' in r.get('identifier', '') and version(r.get('version', ''))[:2] == wanted]
    phones = [d for d in devices if d.get('productFamily') == 'iPhone' or d.get('name', '').startswith('iPhone')]
    for runtime in sorted(available, key=lambda r: version(r.get('version', '')), reverse=True):
        v = version(runtime.get('version', ''))
        for device in phones:
            lo, hi = device.get('minRuntimeVersionString'), device.get('maxRuntimeVersionString')
            if lo and v < version(lo): continue
            if hi and v > version(hi): continue
            return runtime, device
    raise ValueError(f'no available iOS runtime matching active SDK {sdk} and compatible enumerated iPhone device type')


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--app', type=Path, required=True)
    parser.add_argument('--out', type=Path, default=Path('build/ios-simulator-smoke'))
    parser.add_argument('--settle-seconds', type=float, default=3)
    parser.add_argument('--install-timeout', type=int, default=180)
    args = parser.parse_args()
    if not 0 <= args.settle_seconds <= 10: parser.error('settle seconds must be 0..10')
    if not 1 <= args.install_timeout <= 300: parser.error('install timeout must be 1..300 seconds')
    args.out.mkdir(parents=True, exist_ok=True)
    report = {'scope': 'isolated simulator launch smoke', 'host': platform.platform(),
              'started': datetime.now(timezone.utc).isoformat(), 'sourceCommit': os.environ.get('GITHUB_SHA'),
              'commands': [], 'result': 'running'}
    owned = None
    def interrupted(signum, _frame):
        raise KeyboardInterrupt(f'received signal {signum}')
    signal.signal(signal.SIGTERM, interrupted)
    signal.signal(signal.SIGINT, interrupted)

    def execute(command, timeout=60, required=True):
        try:
            result = subprocess.run(command, text=True, stdout=subprocess.PIPE, stderr=subprocess.STDOUT, timeout=timeout)
        except subprocess.TimeoutExpired as error:
            output = error.stdout or ''
            if isinstance(output, bytes): output = output.decode(errors='replace')
            report['commands'].append({'command': command, 'exitCode': None, 'timedOut': True,
                                       'timeoutSeconds': timeout, 'output': output.strip()})
            raise
        report['commands'].append({'command': command, 'exitCode': result.returncode, 'output': result.stdout.strip()})
        if required and result.returncode: raise RuntimeError(f'{command}: {result.stdout.strip()}')
        return result

    def run(*parts, timeout=60, required=True):
        return execute(['xcrun', 'simctl', *map(str, parts)], timeout, required)

    try:
        app = args.app.resolve(strict=True)
        info = plistlib.loads((app/'Info.plist').read_bytes())
        bundle = info['CFBundleIdentifier']
        report['bridgeSha256'] = hashlib.sha256((app/'bridge.js').read_bytes()).hexdigest()
        sdk = execute(['xcrun', '--sdk', 'iphonesimulator', '--show-sdk-version']).stdout.strip()
        report['sdkVersion'] = sdk
        runtime, device = target(json.loads(run('list', 'runtimes', '--json').stdout)['runtimes'],
                                 json.loads(run('list', 'devicetypes', '--json').stdout)['devicetypes'], sdk)
        report.update(bundleIdentifier=bundle, runtime=runtime, deviceType=device)
        created = run('create', 'Backplane acceptance '+str(uuid.uuid4()), device['identifier'], runtime['identifier']).stdout.strip()
        owned = str(uuid.UUID(created))
        report['ownedSimulator'] = owned
        run('boot', owned)
        run('bootstatus', owned, '-b', timeout=240)
        run('install', owned, app, timeout=args.install_timeout)
        launched = run('launch', owned, bundle).stdout.strip()
        pid = re.search(r':\s*(\d+)\s*$', launched)
        if not pid or int(pid.group(1)) <= 0: raise ValueError('simulator launch returned no process id')
        report['launchedPid'] = int(pid.group(1))
        time.sleep(args.settle_seconds)
        processes = run('spawn', owned, 'launchctl', 'list').stdout
        if not any(line.split() and line.split()[0] == pid.group(1) for line in processes.splitlines()):
            raise RuntimeError('launched app process is absent after the short launch wait')
        screenshot = args.out/'launch.png'
        screenshot.unlink(missing_ok=True)
        run('io', owned, 'screenshot', screenshot)
        pixels = screenshot.read_bytes()
        if not pixels.startswith(b'\x89PNG\r\n\x1a\n'): raise ValueError('simulator produced no PNG screenshot')
        report['screenshot'] = {'path': str(screenshot), 'sha256': hashlib.sha256(pixels).hexdigest(), 'bytes': len(pixels)}
        bridge = app/'bridge.js'
        report['bridgeSha256'] = hashlib.sha256(bridge.read_bytes()).hexdigest()
        report['result'] = 'pass'
    except (OSError, ValueError, KeyError, RuntimeError, subprocess.TimeoutExpired, KeyboardInterrupt) as error:
        report['result'] = 'fail'
        report['error'] = str(error)
    finally:
        if owned:
            try:
                run('shutdown', owned, required=False)
                removed = run('delete', owned, required=False)
                if removed.returncode:
                    report['result'] = 'fail'; report['cleanupError'] = 'could not delete owned simulator'
            except (OSError, subprocess.TimeoutExpired) as error:
                report['result'] = 'fail'; report['cleanupError'] = str(error)
        report['finished'] = datetime.now(timezone.utc).isoformat()
        (args.out/'result.json').write_text(json.dumps(report, indent=2)+'\n')
    print(f"{report['result']}: {args.out/'result.json'}")
    return 0 if report['result'] == 'pass' else 1


if __name__ == '__main__':
    sys.exit(main())
