#!/usr/bin/env python3
"""The package without a window binary: backplane-serve and the `backplane`
launcher, through the launcher itself, install-release.sh
(the installer's and the updater's swap), package.sh and dev.sh stage. Every
binary is a stub that records how it was called; nothing is built."""
import os, shutil, subprocess, sys, tempfile, time
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
LAUNCH = ROOT / 'deploy/backplane-launch.sh'
fails = []


def check(name, ok):
    print(('PASS ' if ok else 'FAIL ') + name)
    if not ok:
        fails.append(name)


def stub(path, tag, log):
    path.write_text(f'#!/bin/sh\necho "{tag} $*" >> "{log}"\n')
    path.chmod(0o755)


STUBS = Path(tempfile.mkdtemp(prefix='bp-launch-stubs-'))
OPENED = STUBS / 'opened.log'
# never a real browser: the launcher's opener and curl are stand-ins
for name, body in (('xdg-open', f'echo "$1" >> "{OPENED}"'), ('open', f'echo "$1" >> "{OPENED}"'), ('curl', 'exit 0')):
    (STUBS / name).write_text('#!/bin/sh\n' + body + '\n')
    (STUBS / name).chmod(0o755)


def run(cmd, env=None, cwd=None):
    e = {k: v for k, v in os.environ.items() if k not in ('DISPLAY', 'WAYLAND_DISPLAY', 'BACKPLANE_DESKTOP')}
    e['PATH'] = str(STUBS) + os.pathsep + e['PATH']
    e.update(env or {})
    return subprocess.run(cmd, env=e, cwd=cwd, capture_output=True, text=True, timeout=60)


def launcher(t):
    d = t / 'launch'
    d.mkdir()
    log = d / 'calls.log'
    shutil.copy(LAUNCH, d / 'backplane')
    stub(d / 'backplane-serve', 'serve', log)
    bin_ = t / 'bin'
    bin_.mkdir()
    (bin_ / 'backplane').symlink_to(d / 'backplane')
    home = t / 'home'

    def calls():
        return log.read_text().splitlines() if log.exists() else []

    r = run([str(bin_ / 'backplane'), '--render', 'a.step'])
    check('launcher: --render says renders are not in this build (through a link)',
          r.returncode == 2 and 'not available in this build' in r.stderr and calls() == [])
    run([str(d / 'backplane'), '--home', str(home), '--port', '9'], {'DISPLAY': ':9'})
    check('launcher: no desktop app runs the hub in the foreground', calls() == [f'serve --home {home} --port 9'])
    for _ in range(50):
        if OPENED.exists():
            break
        time.sleep(0.05)
    check('launcher: with a display and no desktop app, the hub page opens', OPENED.exists() and OPENED.read_text().split() == ['http://127.0.0.1:9/'])
    log.unlink()
    r = run([str(d / 'backplane'), '--headless', '--home', str(home)])
    for _ in range(50):
        if log.exists():
            break
        time.sleep(0.05)
    check('launcher: --headless detaches the hub with --foreground', calls() == [f'serve --headless --home {home} --foreground'])
    check('launcher: --headless says where the log is', 'headless.log' in r.stdout and (home / 'headless.log').exists())
    log.unlink()
    run([str(d / 'backplane'), '--headless', '--foreground'])
    check('launcher: --headless --foreground stays in front', calls() == ['serve --headless --foreground'])
    log.unlink()
    (d / 'desktop').mkdir()
    desk = d / 'desktop/backplane-open'
    desk.write_text(f'#!/bin/sh\necho "desktop home=$BACKPLANE_HOME $*" >> "{log}"\n')
    desk.chmod(0o755)
    run([str(d / 'backplane'), '--home', str(home)], {'DISPLAY': ':9'})
    check('launcher: desktop app with a display, --home as BACKPLANE_HOME', calls() == [f'desktop home={home} '])
    log.unlink()
    OPENED.unlink(missing_ok=True)
    run([str(d / 'backplane'), '--home', str(home)])
    check('launcher: desktop app but no display runs the hub', calls() == [f'serve --home {home}'])
    time.sleep(0.3)
    check('launcher: no display opens no browser', not OPENED.exists())
    log.unlink()
    run([str(d / 'backplane'), '--headless', '--foreground'], {'DISPLAY': ':9'})
    check('launcher: --headless never opens the desktop app', calls() == ['serve --headless --foreground'])


def pkg(t, name, with_launcher):
    src = t / name
    (src / 'web').mkdir(parents=True)
    (src / 'web/index.html').write_text('new')
    for f in ('backplane-serve',):
        (src / f).write_text('#!/bin/sh\necho new\n')
        (src / f).chmod(0o755)
    if with_launcher:
        shutil.copy(LAUNCH, src / 'backplane')
    return src


def old_install(t, name):
    prefix = t / name
    (prefix / 'web').mkdir(parents=True)
    (prefix / 'web/index.html').write_text('old')
    for f in ('backplane', 'backplane-serve'):
        (prefix / f).write_text('OLD WINDOW BINARY')
    bin_ = t / (name + '-bin')
    bin_.mkdir()
    (bin_ / 'backplane').symlink_to(prefix / 'backplane')
    return prefix, bin_


def install(t):
    sh = ROOT / 'scripts/install-release.sh'
    prefix, bin_ = old_install(t, 'p1')
    r = run(['sh', str(sh), str(pkg(t, 'r1', True)), str(prefix)])
    check('install: package with launcher installs', r.returncode == 0)
    check('install: backplane is the launcher', (prefix / 'backplane').read_text() == LAUNCH.read_text())
    check('install: web swapped, old kept', (prefix / 'web/index.html').read_text() == 'new' and (prefix / 'web.old').is_dir())
    check('install: ~/.local/bin/backplane resolves', (bin_ / 'backplane').resolve().exists())

    prefix, bin_ = old_install(t, 'p2')
    r = run(['sh', str(sh), str(pkg(t, 'r2', False)), str(prefix)])
    check('install: package without backplane installs', r.returncode == 0)
    b = prefix / 'backplane'
    check('install: old window binary replaced by a link to backplane-serve',
          b.is_symlink() and os.readlink(b) == 'backplane-serve')
    check('install: no dangling ~/.local/bin/backplane', (bin_ / 'backplane').resolve().exists())

    prefix, _ = old_install(t, 'p3')
    src = pkg(t, 'r3', True)
    (src / 'backplane-serve').unlink()
    r = run(['sh', str(sh), str(src), str(prefix)])
    check('install: package without backplane-serve refused, nothing replaced',
          r.returncode != 0 and 'backplane-serve' in r.stderr and (prefix / 'web/index.html').read_text() == 'old')


def package(t):
    tree = t / 'tree'
    (tree / 'scripts').mkdir(parents=True)
    for f in ('scripts/package.sh', 'scripts/install-release.sh', 'deploy/backplane.desktop', 'assets/icon/backplane.svg',
              'assets/icon/backplane-256.png', 'assets/icon/backplane-desktop.svg', 'assets/icon/backplane-desktop-256.png', 'LICENSE'):
        (tree / f).parent.mkdir(parents=True, exist_ok=True)
        shutil.copy(ROOT / f, tree / f)
    dist = tree / 'dist'
    (dist / 'web').mkdir(parents=True)
    (dist / 'kistack').mkdir()
    (dist / 'web/index.html').write_text('w')
    (dist / 'backplane-serve').write_text('bin')
    r = run(['sh', 'scripts/package.sh', 'v0.12.0', 'linux-arm64'], cwd=tree)
    check('package: builds without a window binary', r.returncode == 0)
    names = subprocess.run(['tar', '-tzf', str(dist / 'backplane-v0.12.0-linux-arm64.tar.gz')], capture_output=True, text=True).stdout.split()
    check('package: serve, no backplane', 'backplane-linux-arm64/backplane-serve' in names and 'backplane-linux-arm64/backplane' not in names)
    shutil.copy(LAUNCH, dist / 'backplane')
    r = run(['sh', 'scripts/package.sh', 'v0.12.0', 'linux-arm64'], cwd=tree)
    names = subprocess.run(['tar', '-tzf', str(dist / 'backplane-v0.12.0-linux-arm64.tar.gz')], capture_output=True, text=True).stdout.split()
    check('package: carries the launcher when built', r.returncode == 0 and 'backplane-linux-arm64/backplane' in names)


def dev_stage(t):
    tree = t / 'dev'
    log = tree / 'calls.log'
    for f in ('scripts/dev.sh', 'scripts/bendok.sh', 'deploy/backplane-launch.sh', 'LICENSE', 'src/web/sw.js'):
        (tree / f).parent.mkdir(parents=True, exist_ok=True)
        shutil.copy(ROOT / f, tree / f)
    (tree / 'tools/voice').mkdir(parents=True)
    (tree / 'tools/kipc').mkdir(parents=True)
    home = tree / 'home'
    (home / '.bend/bin').mkdir(parents=True)
    (home / '.bun/bin').mkdir(parents=True)
    (tree / 'scripts/verified.sh').write_text('#!/bin/sh\nexit 1\n')
    for f in ('js-unmarshal.py', 'js-native.py', 'cef-runtime.py'):
        (tree / 'scripts' / f).write_text(f'import sys\nopen({str(log)!r},"a").write({f!r}+" "+" ".join(sys.argv[1:])+"\\n")\n')
    (tree / 'scripts/build-app.sh').write_text(f'#!/bin/sh\necho "build-app $1 $2 stamp=${{BACKPLANE_VERSION_STAMP:-}}" >> "{log}"\necho bin > "$2"\n')
    (tree / 'scripts/build-kistack.sh').write_text(f'#!/bin/sh\necho "kistack" >> "{log}"\nmkdir -p "$1"\n')
    (tree / 'scripts/build-browser.sh').write_text('#!/bin/sh\nexit 1\n')
    (home / '.bend/bin/bend').write_text(
        f'#!/bin/sh\necho "bend $*" >> "{log}"\ncase "$*" in *--check-only*) echo "ALL PROOFS CHECK"; exit 0;; esac\n'
        'if [ "${2:-}" = -o ]; then mkdir -p "$3"; echo x > "$3/chunk-1.js"; echo y > "$3/index.html"; echo z > "$3/a.css"; fi\n')
    (home / '.bun/bin/bun').write_text(f'#!/bin/sh\necho "bun $*" >> "{log}"\n[ "${{1:-}}" = build.ts ] && echo b > "$2"\nexit 0\n')
    for p in list((tree / 'scripts').glob('*.sh')) + [home / '.bend/bin/bend', home / '.bun/bin/bun']:
        p.chmod(0o755)
    subprocess.run('git init -q && git add -A && git -c user.email=m@m -c user.name=m commit -qm m && git tag v0.11.1',
                   shell=True, cwd=tree, check=True)

    def stage(env):
        log.unlink(missing_ok=True)
        out = tree / 'out'
        r = run(['sh', 'scripts/dev.sh', 'stage', str(out)], dict(env, HOME=str(home), BACKPLANE_PREFIX=str(tree / 'prefix')), cwd=tree)
        return r, (log.read_text() if log.exists() else ''), sorted(p.name for p in out.iterdir()) if out.is_dir() else []

    r, calls, staged = stage({'BACKPLANE_VERSION_STAMP': '0.12.1-main.1'})
    check('dev stage: default builds only the server root, stamped',
          r.returncode == 0 and 'build-app src/server/main.bend dist/backplane-serve stamp=0.12.1-main.1' in calls
          and calls.count('build-app ') == 1)
    check('dev stage: never checks or builds src/app/main.bend or a render root', 'src/app/main.bend' not in calls and 'src/render' not in calls)
    check('dev stage: default still builds KiStack and helpers, looks for CEF', 'kistack' in calls and 'bun build.ts' in calls and 'cef-runtime.py discover' in calls)
    check('dev stage: default stages launcher and serve, no renderer',
          {'backplane', 'backplane-serve', 'kistack', 'web'} <= set(staged) and 'backplane-render' not in staged)
    r, calls, staged = stage({'BACKPLANE_DEV_SERVICE_ONLY': '1', 'BACKPLANE_VERSION_STAMP': '0.12.1-main.2'})
    check('dev stage: service-only stages launcher, serve, web, LICENSE',
          r.returncode == 0 and staged == ['LICENSE', 'backplane', 'backplane-serve', 'web'])
    check('dev stage: service-only skips KiStack, helpers and CEF', not any(k in calls for k in ('kistack', 'bun ', 'cef-runtime')))
    r, calls, _ = stage({'BACKPLANE_DEV_SERVICE_ONLY': '1'})
    check('dev stage: no caller stamp keeps the generated dev stamp', 'stamp=0.11.2-dev.' in calls)
    r, calls, _ = stage({'BACKPLANE_DEV_SERVICE_ONLY': 'yes'})
    check('dev stage: a bad service-only value is refused before building', r.returncode != 0 and calls == '')


def classic(t):
    """release-classic.sh: classic packages + install.sh from built payloads,
    installed by install.sh, swapped in the way every 0.11 updater does."""
    rel = t / 'rel'
    rel.mkdir()
    args = []
    for plat in ('linux-x64', 'darwin-arm64'):
        payload = t / ('payload-' + plat)
        (payload / 'web').mkdir(parents=True)
        (payload / 'web/index.html').write_text('web ' + plat)
        (payload / 'backplane-serve').write_text('#!/bin/sh\necho serve-' + plat + '\n')
        (payload / 'backplane-serve').chmod(0o755)
        (payload / 'LICENSE.Backplane').write_text('license')
        (payload / 'BUILD.json').write_text('{}')
        app = t / ('app-' + plat) / ('backplane-' + plat)
        app.mkdir(parents=True)
        launch = app / ('backplane-open' if plat.startswith('linux') else 'Backplane.app/Contents/MacOS/Backplane')
        launch.parent.mkdir(parents=True, exist_ok=True)
        launch.write_text('#!/bin/sh\necho desktop\n')
        launch.chmod(0o755)
        if plat.startswith('linux'):
            (app / 'runtime').mkdir()
            (app / 'runtime/chrome-sandbox').write_text('sandbox')
            (app / 'runtime/backplane-desktop').write_text('electron')
        desk = rel / f'backplane-v0.12.0-{plat}.tar.gz'
        subprocess.run(['tar', '-czf', str(desk), '-C', str(app.parent), app.name], check=True)
        args += [plat, str(payload), str(desk)]
    r = run(['sh', str(ROOT / 'scripts/release-classic.sh'), 'v0.12.0', str(rel), *args], cwd=ROOT)
    check('classic: builds packages and install.sh', r.returncode == 0)
    again = t / 'rel2'
    run(['sh', str(ROOT / 'scripts/release-classic.sh'), 'v0.12.0', str(again), *args], cwd=ROOT)
    same = all((rel / n).read_bytes() == (again / n).read_bytes() for n in ('backplane-v0.12.0-classic-linux-x64.tar.gz', 'install.sh'))
    check('classic: the same inputs give the same bytes', same)
    names = subprocess.run(['tar', '-tzf', str(rel / 'backplane-v0.12.0-classic-linux-x64.tar.gz')], capture_output=True, text=True).stdout.split()
    check('classic: launcher, serve, web, install-release, menu entry under backplane-linux-x64/',
          {'backplane-linux-x64/backplane', 'backplane-linux-x64/backplane-serve', 'backplane-linux-x64/web/index.html',
           'backplane-linux-x64/install-release.sh', 'backplane-linux-x64/backplane.desktop', 'backplane-linux-x64/BUILD.json'} <= set(names))
    script = (rel / 'install.sh').read_text()
    check('classic: install.sh has every sum filled or empty, no placeholder left', '@' not in script.split('REPO=')[1].split('fail()')[0])
    check('classic: linux-arm64 has no sum (no build given)', 'SHA_LINUX_ARM64=""' in script)
    # 0.11 updaters: GitHub lists assets by name (case-insensitively); the
    # first ending in <plat>.tar.gz is what they install
    assets = sorted([p.name for p in rel.iterdir()] + ['SHA256SUMS', 'backplane-v0.12.0-web.tar.gz', 'backplane-v0.12.0-windows-x64.zip'], key=str.lower)
    for plat in ('linux-x64', 'darwin-arm64'):
        first = next(n for n in assets if n.endswith(plat + '.tar.gz'))
        check(f'classic: a 0.11 updater on {plat} takes the classic package', first == f'backplane-v0.12.0-classic-{plat}.tar.gz')
    # install.sh, end to end, against the built archives
    tools = t / 'tools'
    tools.mkdir()
    (tools / 'curl').write_text(f'#!/bin/sh\nwhile [ $# -gt 0 ]; do if [ "$1" = -o ]; then o=$2; fi; u=$1; shift; done\ncp "{rel}/$(basename "$u")" "$o"\n')
    (tools / 'uname').write_text('#!/bin/sh\ncase "$1" in -s) echo Linux;; -m) echo x86_64;; esac\n')
    (tools / 'unshare').write_text('#!/bin/sh\nexit 1\n')
    for f in tools.iterdir():
        f.chmod(0o755)
    prefix, bin_ = t / 'inst', t / 'instbin'
    env = {'PATH': str(tools) + os.pathsep + os.environ['PATH'], 'HOME': str(t / 'h'), 'BACKPLANE_PREFIX': str(prefix),
           'BACKPLANE_BIN': str(bin_), 'BACKPLANE_NO_DESKTOP': '1'}
    r = run(['sh', str(rel / 'install.sh')], env)
    check('classic: install.sh installs hub, launcher and desktop app',
          r.returncode == 0 and (prefix / 'backplane-serve').exists() and (prefix / 'desktop/backplane-open').exists()
          and (prefix / 'backplane').read_text() == LAUNCH.read_text() and (bin_ / 'backplane').resolve() == (prefix / 'backplane').resolve())
    helper = prefix / 'desktop/runtime/chrome-sandbox'
    check('classic: no user namespaces and no root chrome-sandbox: install.sh prints the one command',
          f"sudo chown root:root '{helper}' && sudo chmod 4755 '{helper}'" in r.stdout)
    (tools / 'unshare').write_text('#!/bin/sh\nexit 0\n')
    (tools / 'unshare').chmod(0o755)
    r2 = run(['sh', str(rel / 'install.sh')], env)
    check('classic: with user namespaces it prints nothing about the sandbox', r2.returncode == 0 and 'chrome-sandbox' not in r2.stdout)
    r = run(['sh', str(rel / 'install.sh')], dict(env, BACKPLANE_PREFIX=str(t / 'inst2'), BACKPLANE_NO_DESKTOP_APP='1'))
    check('classic: BACKPLANE_NO_DESKTOP_APP=1 installs the hub alone', r.returncode == 0 and not (t / 'inst2/desktop').exists() and (t / 'inst2/backplane-serve').exists())
    bad = rel / 'backplane-v0.12.0-linux-x64.tar.gz'
    keep = bad.read_bytes()
    bad.write_bytes(keep + b'x')
    r = run(['sh', str(rel / 'install.sh')], dict(env, BACKPLANE_PREFIX=str(t / 'inst3')))
    check('classic: a desktop archive that fails its sum installs nothing', r.returncode != 0 and not (t / 'inst3/backplane-serve').exists())
    bad.write_bytes(keep)
    # the .deb, from the same two archives
    debs = t / 'debs'
    r = run(['sh', str(ROOT / 'scripts/release-deb.sh'), 'v0.12.0', str(debs), 'linux-x64',
             str(rel / 'backplane-v0.12.0-classic-linux-x64.tar.gz'), str(rel / 'backplane-v0.12.0-linux-x64.tar.gz')], cwd=ROOT)
    deb = debs / 'backplane_0.12.0_amd64.deb'
    check('deb: builds', r.returncode == 0 and deb.exists())
    x = t / 'debx'
    x.mkdir()
    subprocess.run(['ar', 'x', str(deb)], cwd=x, check=True)
    data = subprocess.run(['tar', '-tzf', str(x / 'data.tar.gz')], capture_output=True, text=True).stdout.split()
    check('deb: /opt/backplane has launcher, hub, web and the desktop app',
          {'./opt/backplane/backplane', './opt/backplane/backplane-serve', './opt/backplane/web/index.html',
           './opt/backplane/desktop/runtime/chrome-sandbox', './usr/bin/backplane', './etc/apparmor.d/backplane'} <= set(data))
    listing = subprocess.run(['tar', '-tvzf', str(x / 'data.tar.gz')], capture_output=True, text=True).stdout.splitlines()
    check('deb: nothing group- or world-writable', all(l[5] != 'w' and l[8] != 'w' for l in listing if not l.startswith('l')))
    (x / 'c').mkdir()
    subprocess.run(['tar', '-xzf', str(x / 'control.tar.gz'), '-C', str(x / 'c')], check=True)
    post = (x / 'c/postinst').read_text()
    check('deb: postinst sets chrome-sandbox root 4755 and loads the AppArmor profile',
          'chown root:root /opt/backplane/desktop/runtime/chrome-sandbox' in post and 'chmod 4755' in post and 'apparmor_parser -r' in post)
    control = (x / 'c/control').read_text()
    check('deb: version 0.12.0, amd64, managed (no self-update)', 'Version: 0.12.0\n' in control and 'Architecture: amd64' in control
          and 'BACKPLANE_NO_UPDATE=1' in subprocess.run(['tar', '-xzOf', str(x / 'data.tar.gz'), './usr/bin/backplane'], capture_output=True, text=True).stdout)
    # what a 0.11 updater does: rename backplane, backplane-serve, web into place
    old = t / 'old'
    (old / 'web').mkdir(parents=True)
    (old / 'backplane').write_text('OLD WINDOW BINARY')
    (old / 'backplane-serve').write_text('OLD')
    up = t / 'up'
    up.mkdir()
    subprocess.run(['tar', '-xzf', str(rel / 'backplane-v0.12.0-classic-linux-x64.tar.gz'), '-C', str(up)], check=True)
    src = up / 'backplane-linux-x64'
    for f in ('backplane', 'backplane-serve'):
        os.replace(src / f, old / f)
    os.replace(old / 'web', old / 'web.old-v0.12.0')
    os.replace(src / 'web', old / 'web')
    check('classic: a 0.11 swap leaves the launcher, the new hub and its web', (old / 'backplane').read_text() == LAUNCH.read_text()
          and (old / 'backplane-serve').read_text().startswith('#!/bin/sh\necho serve-linux-x64') and (old / 'web/index.html').read_text() == 'web linux-x64')


with tempfile.TemporaryDirectory() as d:
    t = Path(d)
    launcher(t)
    install(t)
    package(t)
    dev_stage(t)
    classic(t)
shutil.rmtree(STUBS, ignore_errors=True)
print(f'{len(fails)} failed' if fails else 'ok')
sys.exit(1 if fails else 0)
