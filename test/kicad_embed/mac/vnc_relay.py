"""The Mac mini gate's input and screen, for a harness running in h's GUI session (a LaunchAgent): one loopback Screen Sharing
connection (ard_vnc, Apple Remote Desktop auth), served on a Unix socket as JSON lines.

Started over ssh with the account password on stdin (Backplane's secret_run: never on disk on the Mac, never on a command line):
    ssh mac 'cd ~/bp-gate-m/src && ../venv/bin/python vnc_relay.py ~/bp-gate-m/vnc.sock --scale 2' < "$PW"
Requests are in screen points (the harness's and the window list's unit); the relay scales them to framebuffer pixels:
    {"op": "chord", "combo": "cmd+a"}            {"op": "text", "text": "abc"}
    {"op": "click", "x": 10, "y": 20}            {"op": "drag", "x0":.., "y0":.., "x1":.., "y1":.., "seconds": 0.6}
    {"op": "move", "x": 10, "y": 20}             {"op": "png", "path": "/abs/out.png"}
    {"op": "size"}                               {"op": "quit"}
Each answer is {"ok": true, ...} or {"ok": false, "error": "..."}.
"""
import argparse
import json
import os
import socket
import sys
import time

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import ard_vnc  # noqa: E402


def serve(path, client, scale):
    if os.path.exists(path):
        os.unlink(path)
    s = socket.socket(socket.AF_UNIX)
    s.bind(path)
    os.chmod(path, 0o600)
    s.listen(4)
    px = lambda v: int(round(float(v) * scale))  # noqa: E731
    while True:
        conn, _ = s.accept()
        f = conn.makefile('rw')
        for line in f:
            try:
                r = json.loads(line)
                op = r.get('op')
                out = {'ok': True}
                if op == 'chord':
                    client.chord(r['combo'])
                elif op == 'text':
                    client.type_text(r['text'])
                elif op == 'click':
                    client.click(px(r['x']), px(r['y']), r.get('button', 1))
                elif op == 'move':
                    client.move(px(r['x']), px(r['y']))
                elif op == 'drag':
                    client.drag(px(r['x0']), px(r['y0']), px(r['x1']), px(r['y1']), seconds=r.get('seconds', 0.6))
                elif op == 'png':
                    client.refresh()
                    client.png(r['path'])
                elif op == 'size':
                    out.update(width=client.width, height=client.height, scale=scale)
                elif op == 'quit':
                    f.write(json.dumps(out) + '\n')
                    f.flush()
                    client.close()
                    os.unlink(path)
                    return
                else:
                    out = {'ok': False, 'error': f'unknown op {op!r}'}
            except Exception as e:  # one bad request never ends the relay
                out = {'ok': False, 'error': f'{type(e).__name__}: {e}'}
            f.write(json.dumps(out) + '\n')
            f.flush()
        conn.close()


def main():
    a = argparse.ArgumentParser()
    a.add_argument('socket')
    a.add_argument('--host', default='127.0.0.1')
    a.add_argument('--user', default='h')
    a.add_argument('--scale', type=float, default=2.0, help='framebuffer pixels per screen point')
    o = a.parse_args()
    c = ard_vnc.Client(o.host).connect(o.user, '/dev/stdin')
    c.refresh()
    print(f'relay: {c.width}x{c.height}, scale {o.scale}, on {o.socket}', flush=True)
    serve(o.socket, c, o.scale)


if __name__ == '__main__':
    main()
