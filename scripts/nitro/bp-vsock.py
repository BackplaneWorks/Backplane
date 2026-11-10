#!/usr/bin/env python3
"""bp-vsock.py UDS PORT MODE ...: talk to the agent in a Nitro Sandbox (bp-guest.py) through Firecracker's vsock socket on the parent.

Firecracker's host-initiated vsock: connect to the Unix socket, send "CONNECT <port>\\n", read "OK <n>\\n", then the stream is the guest's port.
  ping                         the agent answers (exit 0)
  pipe CMD...                  CMD's stdin/stdout are ours (rsync -e); its stderr stays in the guest's /work/.bp/pipe.err
  run CWD_B64 ARGV_B64 ENV_B64 framed: stdout and stderr apart, then the exit code (ours); NUL-separated argv and NAME=VALUE env, base64
Frames from the guest in run mode: 1 byte stream (1 stdout, 2 stderr, 0 exit), 4 bytes big-endian length, the bytes (exit: 4-byte signed code).
A connection that cannot be made is exit 97 (the transport, never the command).
"""
import base64, json, os, select, socket, struct, sys

def connect(uds, port):
    s = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
    s.connect(uds)
    s.sendall(b"CONNECT %d\n" % port)
    line = b""
    while not line.endswith(b"\n"):
        c = s.recv(1)
        if not c: raise OSError("closed during the handshake")
        line += c
    if not line.startswith(b"OK"): raise OSError(line.decode(errors="replace").strip())
    return s

def header(s, h):
    s.sendall(base64.b64encode(json.dumps(h).encode()) + b"\n")

def readn(s, n):
    b = b""
    while len(b) < n:
        c = s.recv(n - len(b))
        if not c: raise EOFError
        b += c
    return b

def main():
    uds, port, mode = sys.argv[1], int(sys.argv[2]), sys.argv[3]
    try: s = connect(uds, port)
    except OSError as e:
        sys.stderr.write("bp-vsock: %s\n" % e); sys.exit(97)
    d = lambda x: base64.b64decode(x).decode() if x and x != "-" else ""
    if mode == "ping":
        header(s, {"mode": "ping"}); sys.exit(0 if readn(s, 4) == b"pong" else 97)
    if mode == "pipe":
        header(s, {"mode": "pipe", "argv": sys.argv[4:], "cwd": ""})
        fin, fout = sys.stdin.buffer.raw, sys.stdout.buffer.raw
        ins = [fin, s]
        while ins:
            r, _, _ = select.select(ins, [], [])
            if fin in r:
                b = os.read(fin.fileno(), 65536)
                if b: s.sendall(b)
                else: s.shutdown(socket.SHUT_WR); ins.remove(fin)
            if s in r:
                b = s.recv(65536)
                if not b: break
                fout.write(b); fout.flush()
        sys.exit(0)
    cwd, argv, env = d(sys.argv[4]), d(sys.argv[5]).split("\0"), d(sys.argv[6] if len(sys.argv) > 6 else "")
    header(s, {"mode": "run", "cwd": cwd, "argv": argv, "env": dict(e.split("=", 1) for e in env.split("\0") if "=" in e)})
    out = {1: sys.stdout.buffer, 2: sys.stderr.buffer}
    try:
        while True:
            t, n = struct.unpack(">BI", readn(s, 5)); b = readn(s, n)
            if t == 0: sys.exit(struct.unpack(">i", b)[0] & 255)
            out[t].write(b); out[t].flush()
    except EOFError:
        sys.stderr.write("bp-vsock: the sandbox closed the connection\n"); sys.exit(97)

if __name__ == "__main__":
    main()
