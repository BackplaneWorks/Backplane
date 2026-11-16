#!/usr/bin/python3
"""The libraries row's L8 on a Mac: a person's existing global fp-lib-table without the Backplane entries (and with one of their own)
gets exactly the six Backplane_* entries from the runtime's default table when a hub (lane K's libtables.rs) selects the runtime at start;
nothing else in it changes, and sym-lib-table (the Backplane library has no symbols) is left byte for byte.
    libtables_l8.py HUB RUNTIME OUT
"""
import json, os, re, shutil, subprocess, sys, time, urllib.request

HUB, RT, OUT = sys.argv[1:4]
SS = os.path.join(RT, 'KiCad.app/Contents/SharedSupport')
shutil.rmtree(OUT, ignore_errors=True)
cfg = os.path.join(OUT, 'fh/Library/Preferences/kicad/10.99'); os.makedirs(cfg)
fails = []
def check(ok, what):
    print(('ok   ' if ok else 'FAIL ') + what, flush=True)
    if not ok: fails.append(what)
default_fp = open(os.path.join(SS, 'fp-lib-table') if os.path.isfile(os.path.join(SS, 'fp-lib-table')) else os.path.join(SS, 'template/fp-lib-table')).read()
default_sym = open(os.path.join(SS, 'sym-lib-table') if os.path.isfile(os.path.join(SS, 'sym-lib-table')) else os.path.join(SS, 'template/sym-lib-table')).read()
LIB = re.compile(r'^\s*\(lib\s+\(name\s+"([^"]+)"\).*$', re.M)
bp = [m.group(0) for m in LIB.finditer(default_fp) if 'BACKPLANE_KICAD_LIBRARY' in m.group(0)]
check(len(bp) == 6, f'the runtime\'s default fp-lib-table names 6 Backplane libraries ({len(bp)})')
# the person's table: the default without the Backplane entries, plus a library of their own
mine = '  (lib (name "My_Parts")(type "KiCad")(uri "/Users/someone/kicad/My_Parts.pretty")(options "")(descr "the person\'s own"))\n'
person = ''.join(l for l in default_fp.splitlines(True) if 'BACKPLANE_KICAD_LIBRARY' not in l)
person = person.rstrip().rstrip(')') + '\n' + mine + ')\n'
open(os.path.join(cfg, 'fp-lib-table'), 'w').write(person)
open(os.path.join(cfg, 'sym-lib-table'), 'w').write(default_sym)
before_sym = default_sym
env = dict(os.environ, HOME=os.path.join(OUT, 'fh'), BACKPLANE_KICAD_CLI=os.path.join(RT, 'bin/kicad-cli'), BACKPLANE_NO_UPDATE='1',
           KICAD_DOCUMENTS_HOME=os.path.join(OUT, 'fh/Documents'), KICAD_CONFIG_HOME=os.path.join(OUT, 'fh/Library/Preferences/kicad'))
hub = subprocess.Popen([HUB, '--home', os.path.join(OUT, 'home'), '--port', '3897', '--host', '127.0.0.1', '--no-tailscale', '--foreground'],
                       env=env, stdout=open(os.path.join(OUT, 'hub.log'), 'w'), stderr=subprocess.STDOUT)
try:
    after = person
    for _ in range(60):
        time.sleep(1)
        after = open(os.path.join(cfg, 'fp-lib-table')).read()
        if after != person: time.sleep(2); after = open(os.path.join(cfg, 'fp-lib-table')).read(); break
finally:
    hub.terminate(); hub.wait(10)
added = [l for l in after.splitlines() if l not in person.splitlines()]
gone = [l for l in person.splitlines() if l not in after.splitlines()]
names = sorted(LIB.match(l).group(1) for l in added if LIB.match(l))
check(len(added) == 6 and all('BACKPLANE_KICAD_LIBRARY' in l for l in added), f'the hub adds exactly the 6 Backplane entries ({names})')
check(not gone and mine.strip() in after, f'nothing of the person\'s table goes or changes, their own library included ({len(gone)} lines gone)')
check(open(os.path.join(cfg, 'sym-lib-table')).read() == before_sym, 'sym-lib-table is left byte for byte')
# the same judgement as Linux and Windows: lane J's libraries_row.py phase b on these tables
REPO = os.path.abspath(os.path.join(os.path.dirname(os.path.abspath(__file__)), '..', '..', '..'))
tb, sb = os.path.join(OUT, 'table_before'), os.path.join(OUT, 'sym_before')
open(tb, 'w').write(person); open(sb, 'w').write(before_sym)
cfgp = os.path.join(OUT, 'row_b.json')
json.dump({'out': OUT, 'config_root': os.path.join(OUT, 'fh/Library/Preferences/kicad'), 'settings_version': '10.99', 'table_before': tb, 'sym_before': sb,
           'runtime_root': RT, 'platform': 'macos'}, open(cfgp, 'w'))
r = subprocess.run([sys.executable, os.path.join(REPO, 'test/kicad_embed/libraries_row.py'), 'b', cfgp], capture_output=True, text=True)
for line in (r.stdout + r.stderr).splitlines():
    print('  ' + line, flush=True)
    if line.startswith(('PASS ', 'FAIL ')): check(line.startswith('PASS '), 'libraries_row ' + line.split(' ', 1)[1][:300])
log = open(os.path.join(OUT, 'hub.log'), errors='replace').read()
print('hub said: ' + ' | '.join(l for l in log.splitlines() if 'libtables' in l or 'lib-table' in l)[:600], flush=True)
print(json.dumps({'failures': fails}))
sys.exit(1 if fails else 0)
