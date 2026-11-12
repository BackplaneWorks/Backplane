#!/usr/bin/env python3
"""A stand-in `claude` for the bare gate (no model): on its first turn it co-edits the open board through the hub's own MCP tools, as an agent
would, and writes what each call answered to $BARE_COEDIT_OUT (JSON). The calls: kicad_ipc_status; OpenDocument; GetItems (tracks) before;
CreateItems one F.Cu track beside the first footprint; GetItems after; SaveDocument; review_comment add (a comment on the board at that spot);
review_comments. $BARE_COEDIT_PCB names the board. Pattern from test/files/kipc_hub_e2e.mjs."""
import json, os, sys, urllib.request
a = sys.argv
def mcp_url():
    # --mcp-config takes JSON or a file holding it, depending on the hub's version
    if '--mcp-config' not in a: return ''
    v = a[a.index('--mcp-config') + 1]
    try: j = json.loads(v)
    except ValueError:
        try: j = json.load(open(v))
        except Exception: return ''
    return ((j.get('mcpServers') or {}).get('backplane') or {}).get('url', '')
url = mcp_url()
if '--version' in a:
    print('2.1.300 (Claude Code)'); sys.exit(0)
if len(a) > 2 and a[1] == 'auth' and a[2] == 'status':
    print(json.dumps({"loggedIn": True, "email": "standin@example.com", "authMethod": "claude.ai"})); sys.exit(0)
# where to write and which board: the hub gives an agent a filtered environment, so the gate leaves them in a file beside this program too
conf = {}
try: conf = json.load(open(os.path.join(os.path.dirname(os.path.realpath(__file__)), 'coedit.conf')))
except Exception: pass
out, pcb = os.environ.get('BARE_COEDIT_OUT') or conf.get('out', ''), os.environ.get('BARE_COEDIT_PCB') or conf.get('pcb', '')
open((out or '/tmp/bare-standin') + '.argv', 'a').write(json.dumps({'argv': a, 'url': url, 'pcb': pcb}) + '\n')
if '--output-format' in a and a[a.index('--output-format') + 1] == 'json':   # a turn is -p too, with stream-json
    # the hub's one-shot helpers (a thread's title, a summary): one plain sentence, nothing touched
    print(json.dumps({'type': 'result', 'subtype': 'success', 'is_error': False, 'result': 'A board edited in KiCad with an agent beside it.', 'session_id': 'bare-standin'}))
    sys.exit(0)
say = lambda o: (print(json.dumps(o)), sys.stdout.flush())
say({'type': 'system', 'subtype': 'init', 'session_id': 'bare-standin'})
sys.stdin.readline()
n = [0]
def tool(name, args):
    n[0] += 1
    r = urllib.request.Request(url, json.dumps({'jsonrpc': '2.0', 'id': n[0], 'method': 'tools/call', 'params': {'name': name, 'arguments': args}}).encode(),
                               {'content-type': 'application/json', 'accept': 'application/json'})
    try:
        j = json.loads(urllib.request.urlopen(r, timeout=120).read())
    except Exception as e:
        return {'error': True, 'body': f'transport: {e}'}
    x = j.get('result', {}); t = (x.get('content') or [{}])[0].get('text', '')
    try: b = json.loads(t)
    except Exception: b = t
    return {'error': bool(x.get('isError')), 'body': b}
C = 'kiapi.common.commands.'
ipc = lambda ty, m={}: tool('kicad_ipc', {'path': pcb, 'type': ty, 'message': m})
msg = lambda r: r['body'].get('message', {}) if isinstance(r['body'], dict) else {}
r = {}
if url and pcb:
    r['status'] = tool('kicad_ipc_status', {'path': pcb})
    r['open'] = ipc(C + 'OpenDocument', {'type': 'DOCTYPE_PCB', 'path': pcb})
    # one writer at a time: the hub names the board's lock in its refusal; an agent takes it and goes on (agents/design.rs)
    import re
    m = re.search(r'lock_acquire (\S+)', json.dumps(r['open'].get('body')))
    if r['open'].get('error') and m:
        r['lock'] = tool('lock_acquire', {'name': m.group(1).rstrip(',.')})
        r['open'] = ipc(C + 'OpenDocument', {'type': 'DOCTYPE_PCB', 'path': pcb})
    doc = msg(r['open']).get('document')
    if not doc:
        # KiCad open in the pane already holds the board: OpenDocument is a headless call, the GUI answers GetOpenDocuments
        r['open_docs'] = ipc(C + 'GetOpenDocuments', {'type': 'DOCTYPE_PCB'})
        doc = (msg(r['open_docs']).get('documents') or [None])[0]
    h = {'document': doc}
    r['before'] = ipc(C + 'GetItems', {'header': h, 'types': ['KOT_PCB_TRACE']})
    fps = ipc(C + 'GetItems', {'header': h, 'types': ['KOT_PCB_FOOTPRINT']})
    items = msg(fps).get('items') or [{}]
    p = items[0].get('position', {}); x, y = int(p.get('x_nm', 100000000)), int(p.get('y_nm', 100000000))
    r['at'] = [x, y]
    track = {'@type': 'type.googleapis.com/kiapi.board.types.Track', 'start': {'x_nm': x + 2000000, 'y_nm': y + 2000000},
             'end': {'x_nm': x + 9000000, 'y_nm': y + 2000000}, 'width': {'value_nm': 300000}, 'layer': 'BL_F_Cu'}
    r['create'] = ipc(C + 'CreateItems', {'header': h, 'items': [track]})
    r['after'] = ipc(C + 'GetItems', {'header': h, 'types': ['KOT_PCB_TRACE']})
    r['save'] = ipc(C + 'SaveDocument', {'document': doc})
    r['comment'] = tool('review_comment', {'op': 'add', 'text': 'bare gate: is this track wide enough?', 'file': os.path.basename(pcb), 'kind': 'board',
                                           'layer': 'F.Cu', 'x': (x + 5000000) / 1e6, 'y': (y + 2000000) / 1e6})
    r['comments'] = tool('review_comments', {})
if out and url:
    open(out + '.tmp', 'w').write(json.dumps(r)); os.rename(out + '.tmp', out)
say({'type': 'assistant', 'message': {'role': 'assistant', 'content': [{'type': 'text', 'text': 'Added a track beside the first part, saved, and left a comment.'}]}, 'session_id': 'bare-standin'})
say({'type': 'result', 'subtype': 'success', 'is_error': False, 'result': 'done', 'session_id': 'bare-standin'})
for _ in sys.stdin: pass
