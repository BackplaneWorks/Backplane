#!/usr/bin/env python3
"""fake-firecracker.py UDS GUEST: a stand-in for Firecracker's host side of a vsock: it listens on UDS, takes the "CONNECT <port>" handshake,
answers "OK", and hands the connection to the guest agent (GUEST --fd N) as the guest's port 5000 would. No VM, no AWS: the protocol only."""
import os, socket, subprocess, sys
uds, guest = sys.argv[1], sys.argv[2]
s = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM); s.bind(uds); s.listen(16)
while True:
    c, _ = s.accept(); line = b""
    while not line.endswith(b"\n"):
        x = c.recv(1)
        if not x: break
        line += x
    if line.strip() != b"CONNECT 5000": c.sendall(b"ERR no listener\n"); c.close(); continue
    c.sendall(b"OK 1073741824\n")
    subprocess.Popen([sys.executable, "-I", guest, "--fd", str(c.fileno())], pass_fds=[c.fileno()]); c.close()
