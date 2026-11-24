"""The Mac mini gate's input and screen, for a harness running in h's GUI session (a LaunchAgent): one loopback Screen Sharing
connection (ard_vnc, Apple Remote Desktop auth), served on a Unix socket as JSON lines.

Started over ssh with the account password on stdin (Backplane's secret_run: never on disk on the Mac, never on a command line):
    ssh mac 'cd ~/bp-gate-m/src && ../venv/bin/python vnc_relay.py ~/bp-gate-m/vnc.sock --scale 2' < "$PW"
Requests are in screen points (the harness's and the window list's unit); the relay scales them to framebuffer pixels:
    {"op": "chord", "combo": "cmd+a"}            {"op": "text", "text": "abc"}
    {"op": "click", "x": 10, "y": 20}            {"op": "drag", "x0":.., "y0":.., "x1":.., "y1":.., "seconds": 0.6}
    {"op": "move", "x": 10, "y": 20}             {"op": "png", "path": "/abs/out.png"}
    {"op": "press", "x": 10, "y": 20, "button": 3, "seconds": 0.3}   (a held click; button 1 left, 3 right)
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
                elif op == 'press':  # a held click: move, button down, hold `seconds`, up (button 1 left, 3 right)
                    b = r.get('button', 1)
                    client.move(px(r['x']), px(r['y'])); time.sleep(0.05); client.button(True, b); time.sleep(float(r.get('seconds', 0.3))); client.button(False, b)
                elif op == 'drag':
                    client.drag(px(r['x0']), px(r['y0']), px(r['x1']), px(r['y1']), seconds=r.get('seconds', 0.6))
                elif op == 'png':
                    # never the whole screen: the person's own windows are never captured. A picture is the given rect only
                    # (the test windows' bounds, in points), cut from the frame in memory before anything is written
                    if not r.get('rect'):
                        raise ValueError('png needs a rect (the test windows only): the whole screen is never saved')
                    client.refresh()
                    x0, y0, w, h = (px(v) for v in r['rect'])
                    x0, y0 = max(0, x0), max(0, y0); w, h = max(1, min(w, client.width - x0)), max(1, min(h, client.height - y0))
                    sub = ard_vnc.Client.__new__(ard_vnc.Client)
                    sub.width, sub.height, sub._masked = w, h, client._masked
                    sub.fb = bytearray(b''.join(bytes(client.fb[((y0 + row) * client.width + x0) * 4:((y0 + row) * client.width + x0 + w) * 4]) for row in range(h)))
                    sub.png(r['path'])
                elif op == 'crop':
                    # raw RGB of a rect in points (the harness's canvas finder): "<w> <h>\n" then w*h*3 bytes
                    client.refresh()
                    x0, y0, w, h = px(r['x']), px(r['y']), px(r['w']), px(r['h'])
                    x0, y0 = max(0, x0), max(0, y0)
                    w, h = max(0, min(w, client.width - x0)), max(0, min(h, client.height - y0))
                    rows = []
                    for y in range(y0, y0 + h):
                        line = client.fb[(y * client.width + x0) * 4:(y * client.width + x0 + w) * 4]
                        rgb = bytearray(w * 3)
                        rgb[0::3], rgb[1::3], rgb[2::3] = line[2::4], line[1::4], line[0::4]
                        rows.append(bytes(rgb))
                    with open(r['path'], 'wb') as out_f:
                        out_f.write(f'{w} {h}\n'.encode() + b''.join(rows))
                    out.update(width=w, height=h)
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
            try:
                f.write(json.dumps(out) + '\n')
                f.flush()
            except OSError:  # the asker left before its answer
                break
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
