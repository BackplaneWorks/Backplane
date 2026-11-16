"""One request to a hub over its WebSocket, stdlib only (the Mac has no node for test/kicad_embed/lib/hub_rpc.mjs): the hub takes a JSON text
frame {id, m, p}. The answer (CBOR) is not read: the callers watch the hub's own log and the stand-in's for what happened.
    send(port, token, 'turn.start', {'thread': 't1', 'text': 'mcpcall:{...}'})
"""
import base64
import json
import os
import socket
import struct
import urllib.parse


def send(port, token, method, params, host='127.0.0.1', wait=0.5):
    s = socket.create_connection((host, port), timeout=20)
    key = base64.b64encode(os.urandom(16)).decode()
    s.sendall((f'GET /ws?token={urllib.parse.quote(token)} HTTP/1.1\r\nHost: {host}:{port}\r\nUpgrade: websocket\r\nConnection: Upgrade\r\n'
               f'Sec-WebSocket-Key: {key}\r\nSec-WebSocket-Version: 13\r\n\r\n').encode())
    head = b''
    while b'\r\n\r\n' not in head:
        c = s.recv(4096)
        if not c:
            raise ConnectionError('the hub closed the WebSocket handshake')
        head += c
    if b' 101 ' not in head.split(b'\r\n', 1)[0]:
        raise ConnectionError(head.split(b'\r\n', 1)[0].decode(errors='replace'))
    data = json.dumps({'id': 1, 'm': method, 'p': params}).encode()
    mask = os.urandom(4)
    n = len(data)
    hdr = bytes([0x81]) + (bytes([0x80 | n]) if n < 126 else bytes([0x80 | 126]) + struct.pack('>H', n) if n < 65536 else bytes([0x80 | 127]) + struct.pack('>Q', n))
    s.sendall(hdr + mask + bytes(b ^ mask[i % 4] for i, b in enumerate(data)))
    if wait:
        s.settimeout(wait)
        try:
            while s.recv(65536):
                pass
        except (socket.timeout, OSError):
            pass
    s.sendall(bytes([0x88, 0x80]) + os.urandom(4))  # close
    s.close()
