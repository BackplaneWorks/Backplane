#!/usr/bin/env python3
"""bp-guest.py: the agent inside a Nitro Sandbox. Listens on vsock port 5000 for the parent (bp-vsock.py); one forked child per connection.

The header line (base64 JSON) says the mode: ping, pipe (the command on the connection's stdin/stdout) or run (framed stdout/stderr/exit code).
Commands run as the unprivileged user `bp` in src/ of the work disk (/work), or a folder below it the header names.
"""
import base64, json, os, pwd, signal, socket, struct, subprocess, sys, selectors

PORT = 5000
W = os.environ.get("BP_GUEST_WORK", "/work")   # the work disk (a test points it at a folder)

def line(c):
    b = b""
    while not b.endswith(b"\n"):
        x = c.recv(1)
        if not x: raise EOFError
        b += x
    return json.loads(base64.b64decode(b))

def drop():
    if os.getuid() != 0: return
    try:
        u = pwd.getpwnam("bp"); os.setgid(u.pw_gid); os.setuid(u.pw_uid); os.environ["HOME"] = u.pw_dir
    except KeyError: pass

def env(extra):
    e = {"PATH": "/opt/cargo/bin:/usr/local/bin:/usr/bin:/bin", "HOME": W + "/home", "CARGO_HOME": W + "/cargo", "RUSTUP_HOME": "/opt/rustup",
         "CARGO_TARGET_DIR": W + "/target", "BP_NO_BOAT": "1", "BP_ON_NITRO": "1", "LANG": "C.UTF-8", "TMPDIR": W + "/tmp"}
    e.update(extra or {}); return e

def cwd_of(h):
    """the header's folder, relative to src/; never outside it"""
    src = os.path.realpath(W + "/src"); c = os.path.realpath(os.path.join(src, (h.get("cwd") or "").lstrip("/")))
    return c if c == src or c.startswith(src + "/") else src

def serve(c):
    h = line(c)
    m = h.get("mode")
    if m == "ping": c.sendall(b"pong"); return
    for d in (".bp", "tmp", "src", "home"):
        os.makedirs(W + "/" + d, exist_ok=True)
        if os.getuid() == 0:
            try: u = pwd.getpwnam("bp"); os.chown(W + "/" + d, u.pw_uid, u.pw_gid)
            except KeyError: pass
    if m == "pipe":
        err = open(W + "/.bp/pipe.err", "ab")
        p = subprocess.Popen(h["argv"], cwd=cwd_of(h), env=env({}), stdin=c.fileno(), stdout=c.fileno(), stderr=err, preexec_fn=drop)
        p.wait(); return
    p = subprocess.Popen(h["argv"], cwd=cwd_of(h), env=env(h.get("env")), stdin=subprocess.DEVNULL, stdout=subprocess.PIPE, stderr=subprocess.PIPE, preexec_fn=drop, start_new_session=True)
    sel = selectors.DefaultSelector(); sel.register(p.stdout, selectors.EVENT_READ, 1); sel.register(p.stderr, selectors.EVENT_READ, 2)
    c.setblocking(True)
    try:
        while sel.get_map():
            for k, _ in sel.select():
                b = os.read(k.fileobj.fileno(), 65536)
                if not b: sel.unregister(k.fileobj); continue
                c.sendall(struct.pack(">BI", k.data, len(b)) + b)
        rc = p.wait()
        c.sendall(struct.pack(">BIi", 0, 4, rc if rc >= 0 else 128 - rc))
    except (BrokenPipeError, ConnectionResetError):
        # the parent went away: the command and what it started go too
        try: os.killpg(p.pid, signal.SIGKILL)
        except ProcessLookupError: pass

def main():
    if len(sys.argv) > 2 and sys.argv[1] == "--fd":   # tests: serve one connection already made (a stand-in for the vsock)
        serve(socket.socket(fileno=int(sys.argv[2]))); return
    signal.signal(signal.SIGCHLD, signal.SIG_IGN)
    s = socket.socket(socket.AF_VSOCK, socket.SOCK_STREAM)
    s.setsockopt(socket.SOL_SOCKET, socket.SO_REUSEADDR, 1)
    s.bind((socket.VMADDR_CID_ANY, PORT)); s.listen(64)
    while True:
        c, _ = s.accept()
        if os.fork() == 0:
            s.close(); signal.signal(signal.SIGCHLD, signal.SIG_DFL)
            try: serve(c)
            except Exception as e: sys.stderr.write("bp-guest: %s\n" % e)
            os._exit(0)
        c.close()

if __name__ == "__main__":
    main()
