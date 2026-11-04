#!/usr/bin/env python3
"""One-time, user-requested six-lane repair through the installed MCP API.

Run after the subthread-launch fix is installed. No event/store files are read
or written. Preflight requires the new parent schema before any mutation.
"""
import argparse
import json
from pathlib import Path
import sys
import urllib.parse
import urllib.request

LANES = {
    't1793774562-1546455': ('t1793774597-1546684', 't1793774605-1546729', 't1793774612-1546783'),
    't1793774582-1546582': ('t1793774614-1546807', 't1793774616-1546840', 't1793774618-1546896'),
}


def content(reply):
    if reply.get('isError'):
        raise ValueError('API refused the operation; no further lane will be changed')
    rows = reply.get('content', [])
    return json.loads(next(r['text'] for r in rows if r.get('type') == 'text'))


def repair(request):
    plan = []
    # Preflight every target before writing any parent. A different saved
    # parent is a subsequent manual move and is deliberately left alone.
    for parent, children in LANES.items():
        tools = request(parent, 'tools/list', {})['tools']
        update = next(t for t in tools if t['name'] == 'thread_update')
        if 'parent' not in update['inputSchema']['properties']:
            raise ValueError('Installed harness has no parent-move API; install the fix first')
        owner = content(request(parent, 'tools/call', {'name': 'thread_read', 'arguments': {'threadId': parent, 'limit': 0}}))['status']
        for child in children:
            status = content(request(parent, 'tools/call', {'name': 'thread_read', 'arguments': {'threadId': child, 'limit': 0}}))['status']
            if 'parent' not in status or status['project'] != owner['project']:
                raise ValueError('Parent status unavailable or lane belongs to another project')
            if status['parent'] not in ('', parent):
                plan.append((parent, child, 'kept-manual-move'))
            else:
                plan.append((parent, child, 'already-linked' if status['parent'] == parent else 'attach'))
    receipt = []
    for parent, child, action in plan:
        if action == 'attach':
            saved = content(request(parent, 'tools/call', {'name': 'thread_update', 'arguments': {'threadId': child, 'parent': parent}}))['status']
            if saved.get('parent') != parent:
                raise ValueError('Parent write did not return the requested saved relationship')
            readback = content(request(parent, 'tools/call', {'name': 'thread_read', 'arguments': {'threadId': child, 'limit': 0}}))['status']
            if readback.get('parent') != parent:
                raise ValueError('Parent read-back differs; inspect before retrying')
        receipt.append({'thread': child, 'parent': parent, 'result': action})
    return receipt


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--home', type=Path, required=True, help='Installed hub home (used only for its lock/credential)')
    args = parser.parse_args()
    lock = (args.home/'hub.lock').read_text().split()
    port = int(lock[1]); credential = (args.home/'token').read_text().strip()
    def request(thread, method, params):
        payload = json.dumps({'jsonrpc': '2.0', 'id': 1, 'method': method, 'params': params}).encode()
        url = f'http://127.0.0.1:{port}/mcp/{thread}?' + urllib.parse.urlencode({'token': credential})
        req = urllib.request.Request(url, data=payload, headers={'Content-Type': 'application/json', 'Accept': 'application/json'})
        with urllib.request.urlopen(req, timeout=30) as response:
            reply = json.load(response)
        if 'error' in reply:
            raise ValueError('RPC refused the request')
        return reply['result']
    try:
        print(json.dumps(repair(request), indent=2))
    except Exception as error:
        # Never print an exception containing the authenticated URL.
        print(str(error) if isinstance(error, ValueError) else 'Repair stopped; transport failed', file=sys.stderr)
        return 1
    return 0


if __name__ == '__main__':
    raise SystemExit(main())
