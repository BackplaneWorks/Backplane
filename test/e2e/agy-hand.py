#!/usr/bin/env python3
"""agy-hand.py: the Antigravity stand-in driven by hand, as the hub drives it: initialize, authenticate, session/new, a prompt that asks for a permission,
the permission answer, and then: does the turn end? Prints every line the stand-in says and the time it took. usage: agy-hand.py STANDIN.py"""
import json, subprocess, sys, time, os, select
p = subprocess.Popen([sys.executable, sys.argv[1]], stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True, bufsize=1,
                     env=dict(os.environ, AGY_LOG='/tmp/agy-hand.log'))
print('python', sys.version.split()[0], 'stand-in', sys.argv[1], flush=True)
def send(o): p.stdin.write(json.dumps(o) + '\n'); p.stdin.flush()
buf = b''
def read(secs):   # os.read on the descriptor: select() cannot see lines a text reader already buffered
    global buf
    out = []; end = time.time() + secs
    while time.time() < end:
        r, _, _ = select.select([p.stdout.fileno()], [], [], 0.2)
        if r:
            d = os.read(p.stdout.fileno(), 65536)
            if not d: break
            buf += d
            while b'\n' in buf:
                l, buf = buf.split(b'\n', 1)
                if l.strip(): out.append(json.loads(l)); print('<-', l.decode()[:200], flush=True)
        if p.poll() is not None: break
    return out
send({'jsonrpc': '2.0', 'id': 1, 'method': 'initialize', 'params': {}}); read(2)
send({'jsonrpc': '2.0', 'id': 2, 'method': 'authenticate', 'params': {}}); read(1)
send({'jsonrpc': '2.0', 'id': 3, 'method': 'session/new', 'params': {'cwd': '/tmp'}}); read(1)
send({'jsonrpc': '2.0', 'id': 4, 'method': 'session/prompt', 'params': {'sessionId': 'AS1', 'prompt': [{'type': 'text', 'text': 'zq perm please'}]}})
got = read(3); ask = [o for o in got if o.get('method') == 'session/request_permission']
print('permission asked:', bool(ask), flush=True)
if ask:
    send({'jsonrpc': '2.0', 'id': ask[0]['id'], 'result': {'outcome': {'outcome': 'selected', 'optionId': 'allow'}}}); t = time.time()
    got = read(5); print('turn ended after the answer:', any(o.get('id') == 4 for o in got), '(%.1f s)' % (time.time() - t), flush=True)
print('stderr:', p.stderr.read() if p.poll() is not None else '(still running)', flush=True)
p.kill()
