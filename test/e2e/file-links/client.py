#!/usr/bin/env python3
"""Box B of the file-links E2E (run.sh): every check of docs/file-links.md against box A's hub, over the network between the boxes.

  client.py PHASE BASE ARGS_JSON

PHASE is one of setup, download, checks, stale, deny, shares, revoke (run.sh runs them in that order, with box A's steps between). ARGS_JSON
carries what box A said: {token, proj, outside, sha, size}. State between phases is kept in ~/fl/state.json (on this box only).
Prints one line per check ("pass  name  detail" / "fail  name  detail") and ends with "summary pass=N fail=N". Exit 1 when a check failed.
Python 3, standard library only. The hub's credentials are ephemeral test ones and never printed.
"""
import base64, hashlib, http.client, json, os, re, socket, struct, sys, time, urllib.parse

PHASE, BASE, ARGS = sys.argv[1], sys.argv[2].rstrip("/"), json.loads(sys.argv[3])
U = urllib.parse.urlsplit(BASE)
HOST, PORT = U.hostname, U.port
STATE = os.path.expanduser("~/fl/state.json")
st = json.load(open(STATE)) if os.path.exists(STATE) else {}
RES = []


def check(name, ok, detail=""):
    RES.append(bool(ok))
    print(f"{'pass' if ok else 'FAIL'}  [{PHASE}] {name}{'  (' + detail + ')' if detail else ''}", flush=True)
    return ok


def save():
    json.dump(st, open(STATE, "w"))


def q(s):
    return urllib.parse.quote(s, safe="")


def bearer(t):
    return {"Authorization": f"Bearer {t}"} if t else {}


def get(path, headers=None, method="GET", stop_at=None, keep=None):
    """One request. Streams the body into a sha256 (never held whole); stop_at: close the connection after that many bytes (an interrupted
    download). keep: a hashlib object to feed instead of a new one. Returns (status, headers dict lowercased, sha hex, bytes read, body head)."""
    c = http.client.HTTPConnection(HOST, PORT, timeout=60)
    c.request(method, path, headers=headers or {})
    r = c.getresponse()
    h = {k.lower(): v for k, v in r.getheaders()}
    sha, n, head = keep or hashlib.sha256(), 0, b""
    if method != "HEAD":
        while True:
            want = 1 << 20 if stop_at is None else min(1 << 20, stop_at - n)
            if want <= 0:
                break
            b = r.read(want)
            if not b:
                break
            if len(head) < 200:
                head += b[:200 - len(head)]
            sha.update(b)
            n += len(b)
    c.close()
    return r.status, h, sha.hexdigest(), n, head


def dictionaries():
    """KEYS and WORDS of the hub's CBOR (native/host/src/wire_dict.rs, copied beside this script): the plain /ws answers in CBOR."""
    f = os.path.join(os.path.dirname(os.path.abspath(__file__)), "wire_dict.rs")
    if not os.path.exists(f):
        return [], []
    src = open(f).read()
    words = lambda name: re.findall(r'"((?:[^"\\\\]|\\\\.)*)"', re.search(r"pub const " + name + r": &\[&str\] = &\[(.*?)\];", src, re.S).group(1))  # noqa: E731
    return words("KEYS"), words("WORDS")


KEYS, WORDS = dictionaries()


def cbor(b, i=0):
    """The hub's CBOR (test/e2e/pairing/settings_revoke.py): an integer map key is KEYS[n], tag 6 is WORDS[n]. Returns (value, next offset)."""
    major, info = b[i] >> 5, b[i] & 31
    i += 1
    if major == 7:
        if info == 27:
            return struct.unpack(">d", b[i:i + 8])[0], i + 8
        return {20: False, 21: True}.get(info), i
    n = info
    if info >= 24:
        size = 1 << (info - 24)
        n, i = int.from_bytes(b[i:i + size], "big"), i + size
    if major == 0:
        return n, i
    if major == 1:
        return -1 - n, i
    if major == 2:
        return b[i:i + n], i + n
    if major == 3:
        return b[i:i + n].decode("utf-8", "replace"), i + n
    if major == 4:
        out = []
        for _ in range(n):
            v, i = cbor(b, i)
            out.append(v)
        return out, i
    if major == 5:
        out = {}
        for _ in range(n):
            k, i = cbor(b, i)
            v, i = cbor(b, i)
            out[KEYS[k] if isinstance(k, int) and k < len(KEYS) else k] = v
        return out, i
    v, i = cbor(b, i)
    return (WORDS[v] if n == 6 and isinstance(v, int) and v < len(WORDS) else v), i


class Data:
    """A data/1 socket (enc=json): requests as masked JSON text frames, replies matched by id."""

    def __init__(self, token, path=None):
        self.s = socket.create_connection((HOST, PORT), timeout=20)
        key = base64.b64encode(os.urandom(16)).decode()
        path = path or "/ws?data=1&enc=json&token=" + q(token)
        self.s.sendall((f"GET {path} HTTP/1.1\r\nHost: {HOST}:{PORT}\r\nConnection: Upgrade\r\nUpgrade: websocket\r\n"
                        f"Sec-WebSocket-Version: 13\r\nSec-WebSocket-Key: {key}\r\n\r\n").encode())
        self.buf = b""
        while b"\r\n\r\n" not in self.buf:
            d = self.s.recv(4096)
            if not d:
                break
            self.buf += d
        head, _, self.buf = self.buf.partition(b"\r\n\r\n")
        p = head.split(b" ", 2)
        self.status = int(p[1]) if len(p) > 1 and p[1].isdigit() else 0
        self.n = 100

    def _need(self, n):
        while len(self.buf) < n:
            d = self.s.recv(65536)
            if not d:
                raise EOFError
            self.buf += d
        out, self.buf = self.buf[:n], self.buf[n:]
        return out

    def frame(self):
        b0, b1 = self._need(2)
        n = b1 & 127
        if n == 126:
            n = struct.unpack(">H", self._need(2))[0]
        elif n == 127:
            n = struct.unpack(">Q", self._need(8))[0]
        return b0 & 15, self._need(n)

    def next_json(self):
        while True:
            op, body = self.frame()
            if op == 8:
                raise EOFError("closed")
            if op == 1:
                try:
                    return json.loads(body)
                except ValueError:
                    continue
            if op == 2:
                try:
                    v = cbor(body)[0]
                except (IndexError, struct.error, ValueError):
                    continue
                if isinstance(v, dict):
                    return v

    def request(self, m, p=None, wait=30):
        self.n += 1
        data, mask = json.dumps({"id": self.n, "m": m, "p": p or {}}).encode(), os.urandom(4)
        n = len(data)
        hd = bytes([0x81]) + (bytes([0x80 | n]) if n < 126 else bytes([0x80 | 126]) + struct.pack(">H", n) if n < 65536 else bytes([0x80 | 127]) + struct.pack(">Q", n))
        self.s.sendall(hd + mask + bytes(b ^ mask[i % 4] for i, b in enumerate(data)))
        end = time.time() + wait
        while time.time() < end:
            self.s.settimeout(max(0.1, end - time.time()))
            try:
                j = self.next_json()
            except socket.timeout:
                break
            if j.get("t") == "reply" and str(j.get("id")) == str(self.n):
                return j
        return {"ok": None, "error": f"no reply to {m} in {wait}s"}

    def closed_within(self, seconds):
        end = time.time() + seconds
        try:
            while time.time() < end:
                self.s.settimeout(max(0.1, end - time.time()))
                if self.frame()[0] == 8:
                    return True
        except socket.timeout:
            return False
        except (OSError, EOFError):
            return True
        return False

    def close(self):
        try:
            self.s.close()
        except OSError:
            pass


def entries(d, thread):
    r = d.request("thread.entries", {"thread": thread, "limit": 40})
    return (r.get("result") or {}).get("entries") or []


def wait_entry(d, thread, pred, seconds=60):
    end = time.time() + seconds
    while time.time() < end:
        for e in entries(d, thread):
            if e.get("kind") == "assistant" and not e.get("live") and pred(e):
                return e
        time.sleep(1)
    return None


def mcp_call(d, thread, args, tag):
    """The stand-in agent of `thread` makes one MCP file_share call (the hub's MCP endpoint with that thread's token) and says its answer."""
    before = {e.get("id") for e in entries(d, thread)}
    d.request("message.send", {"thread": thread, "text": "mcpcall:" + json.dumps({"name": "file_share", "arguments": args}), "clientId": f"fl-{tag}-{time.time()}"})
    e = wait_entry(d, thread, lambda e: e.get("id") not in before and e.get("text", "").startswith("MCPCALL file_share"), 60)
    return (e or {}).get("text", "")


def share_of(text):
    t = text.replace('\\"', '"').replace("\\\\", "\\")
    link = re.search(r"/s/[0-9a-f]{48}/[A-Za-z0-9._~%-]+", t)
    sid = re.search(r'"id"\s*:\s*"(sh[0-9a-f]+)"', t)
    return (link.group(0) if link else ""), (sid.group(1) if sid else "")


HUB = ARGS.get("token", "")
DEV = st.get("dev", "")
P = ARGS.get("proj", "")
BIG = f"{P}/mfg/big.bin"
FILE = lambda p: f"/file?path={q(p)}"  # noqa: E731


def setup():
    c = http.client.HTTPConnection(HOST, PORT, timeout=20)
    c.request("POST", "/auth/invitations", headers=bearer(HUB)); r = c.getresponse(); inv = json.loads(r.read() or b"{}"); c.close()
    check("pairing: hub token makes an invitation", r.status == 200 and "#" in inv.get("link", ""), f"HTTP {r.status}")
    secret = dict(urllib.parse.parse_qsl(inv.get("link", "#").split("#", 1)[1])).get("invite", "")
    c = http.client.HTTPConnection(HOST, PORT, timeout=20)
    c.request("POST", "/auth/enroll", headers={**bearer(secret), "X-Backplane-Client": "native", "X-Backplane-Name": "Linux fl-e2e box B"})
    r = c.getresponse(); dev = json.loads(r.read() or b"{}"); c.close()
    check("pairing: box B enrolls as a device", r.status == 200 and dev.get("token") and dev.get("id"), f"HTTP {r.status}")
    st.update(dev=dev.get("token", ""), dev_id=dev.get("id", "")); save()
    d = Data(st["dev"])
    check("data/1: device socket opens", d.status == 101 and d.next_json().get("t") == "hello")
    r = d.request("project.add", {"path": P})
    pid = ((r.get("result") or {}).get("project") or {}).get("id", "")
    check("data/1: project.add of the box A folder", bool(pid), json.dumps(r.get("error", ""))[:200])
    text = f"The output is in {BIG}, the part is `{P}/mech/bracket.step` and the keys are in {P}/.env and {P}/id_rsa."
    r = d.request("thread.create", {"project": pid, "title": "fl links", "env": "local", "provider": "claude", "text": "mode:plain echo:" + text + ":echo"})
    t1 = ((r.get("result") or {}).get("thread") or {}).get("id", "")
    check("data/1: thread.create with the stand-in", bool(t1), json.dumps(r.get("error", ""))[:200])
    e = wait_entry(d, t1, lambda e: "big.bin" in e.get("text", ""), 90)
    check("data/1: the stand-in's reply naming the paths arrives", e is not None)
    files = {f.get("name"): f for f in (e or {}).get("files") or []}
    b = files.get("big.bin") or {}
    check("data/1: entry.files has mfg/big.bin with open=download", b.get("open") == "download" and b.get("path") == BIG and b.get("size") == ARGS.get("size"), json.dumps(b))
    s = files.get("bracket.step") or {}
    check("data/1: entry.files has the STEP with open=view", s.get("open") == "view", json.dumps(s))
    check("data/1: entry.files leaves out .env and id_rsa", ".env" not in files and "id_rsa" not in files, ",".join(files))
    # a second thread in the stand-in's stream mode: its MCP calls (mcpcall) are the shares' source
    r = d.request("thread.create", {"project": pid, "title": "fl shares", "env": "local", "provider": "claude", "text": "mode:stream hello"})
    t2 = ((r.get("result") or {}).get("thread") or {}).get("id", "")
    wait_entry(d, t2, lambda e: True, 60)
    st.update(project=pid, t1=t1, t2=t2); save()
    d.close()


def download():
    t0 = time.time()
    s, h, sha, n, _ = get(FILE(BIG), bearer(DEV))
    dt = time.time() - t0
    check("/file big.bin: full download, sha256 matches", s == 200 and sha == ARGS["sha"] and n == ARGS["size"], f"HTTP {s}, {n} bytes in {dt:.1f}s, {n / dt / 1e6:.0f} MB/s")


def checks():
    s, h, _, n, _ = get(FILE(BIG), bearer(DEV), method="HEAD")
    st["etag"] = h.get("etag", ""); save()
    check("HEAD /file: 200, Content-Length, Accept-Ranges, strong ETag, attachment, no body",
          s == 200 and h.get("content-length") == str(ARGS["size"]) and h.get("accept-ranges") == "bytes" and h.get("etag", "").startswith('"')
          and "attachment" in h.get("content-disposition", "") and n == 0 and h.get("x-content-type-options") == "nosniff",
          f"HTTP {s} len={h.get('content-length')} etag={h.get('etag')}")
    cut = 60 << 20
    acc = hashlib.sha256()
    s1, h1, _, n1, _ = get(FILE(BIG), bearer(DEV), stop_at=cut, keep=acc)
    check("interrupted download: connection dropped part way", s1 == 200 and n1 == cut, f"{n1} of {ARGS['size']} bytes")
    s2, h2, sha2, n2, _ = get(FILE(BIG), {**bearer(DEV), "Range": f"bytes={n1}-", "If-Range": h1.get("etag", "")}, keep=acc)
    check("resume: Range + If-Range is a 206 of the rest, whole sha256 matches",
          s2 == 206 and h2.get("content-range") == f"bytes {n1}-{ARGS['size'] - 1}/{ARGS['size']}" and n1 + n2 == ARGS["size"] and sha2 == ARGS["sha"],
          f"HTTP {s2} {h2.get('content-range')}, {n2} bytes")
    s, h, _, n, _ = get(FILE(BIG), {**bearer(DEV), "Range": "bytes=100-199"})
    check("Range bytes=100-199: 206, 100 bytes", s == 206 and n == 100 and h.get("content-range") == f"bytes 100-199/{ARGS['size']}", f"HTTP {s}")
    s, h, _, n, _ = get(FILE(BIG), {**bearer(DEV), "Range": f"bytes={ARGS['size'] + 10}-"})
    check("Range past the end: 416", s == 416, f"HTTP {s}")
    t0 = time.time()
    s, h, sha, n, _ = get(f"/files/get?thread={q(st['t1'])}&path={q('mfg/big.bin')}", bearer(DEV))
    check("/files/get?thread=&path=mfg/big.bin streamed, sha256 matches", s == 200 and sha == ARGS["sha"], f"HTTP {s}, {n} bytes in {time.time() - t0:.1f}s")
    s, h, _, n, _ = get(f"/files/get?thread={q(st['t1'])}&path={q('mfg/big.bin')}", bearer(DEV), method="HEAD")
    check("HEAD /files/get: 200 with Content-Length", s == 200 and h.get("content-length") == str(ARGS["size"]) and n == 0, f"HTTP {s}")
    s, h, _, n, _ = get(f"/file?path={q(BIG)}", {"Authorization": f"Bearer {HUB}"}, method="HEAD")
    check("HEAD /file with the hub token: 200", s == 200, f"HTTP {s}")


def stale():
    s, h, sha, n, _ = get(FILE(BIG), {**bearer(DEV), "Range": "bytes=1000-", "If-Range": st.get("etag", "")})
    check("changed file + stale If-Range: 200 with the whole new file", s == 200 and sha == ARGS["sha"] and n == ARGS["size"] and h.get("etag") != st.get("etag"),
          f"HTTP {s}, {n} bytes, etag {st.get('etag')} -> {h.get('etag')}")
    s, h, sha, n, _ = get(FILE(BIG), {**bearer(DEV), "Range": "bytes=1000-", "If-Range": h.get("etag", "")})
    check("changed file + fresh If-Range: 206", s == 206 and n == ARGS["size"] - 1000, f"HTTP {s}")


def deny():
    s = get(FILE(BIG))[0]
    check("/file with no credential: 401", s == 401, f"HTTP {s}")
    s = get(FILE(BIG), bearer("0" * 64))[0]
    check("/file with a made-up token: 401", s == 401, f"HTTP {s}")
    s = get(f"/files/get?thread={q(st['t1'])}&path={q('mfg/big.bin')}")[0]
    check("/files/get with no credential: 401", s == 401, f"HTTP {s}")
    for label, p in ((".env", f"{P}/.env"), ("id_rsa", f"{P}/id_rsa"), (".git/config", f"{P}/.git/config"), ("symlink to outside", f"{P}/link-out.txt"),
                     ("a file outside every project", ARGS["outside"]), ("the hub's own token", f"{ARGS['home']}/token"), ("/etc/passwd", "/etc/passwd")):
        s, _, _, n, body = get(FILE(p), bearer(DEV))
        check(f"/file {label}: 403", s == 403 and n < 100, f"HTTP {s} {body[:60]!r}")
    s, _, _, _, body = get(FILE(f"{P}/mfg/none.bin"), bearer(DEV))
    check("/file missing file: 404", s == 404, f"HTTP {s} {body[:40]!r}")
    s, _, _, _, body = get(FILE(f"{P}/mfg/../../outside/notes.txt"), bearer(DEV))
    check("/file .. climb: refused", s in (400, 403), f"HTTP {s} {body[:40]!r}")
    s, _, _, _, body = get(FILE(f"{P}/mfg/..%2F..%2Foutside/notes.txt"), bearer(DEV))
    check("/file encoded .. climb: refused", s in (400, 403, 404), f"HTTP {s} {body[:40]!r}")
    s, _, _, _, body = get(FILE("mfg/big.bin"), bearer(DEV))
    check("/file relative path: refused", s in (400, 403), f"HTTP {s} {body[:40]!r}")
    fg = lambda rel: get(f"/files/get?thread={q(st['t1'])}&path={q(rel)}", bearer(DEV))  # noqa: E731
    for label, rel in (("..", "../outside/notes.txt"), ("absolute", ARGS["outside"]), ("symlink to outside", "link-out.txt")):
        s, _, _, n, body = fg(rel)
        check(f"/files/get {label}: refused", s in (400, 403, 404) and n < 100, f"HTTP {s} {body[:40]!r}")
    for label, rel in ((".env", ".env"), ("id_rsa", "id_rsa"), (".git/config", ".git/config")):
        s, _, _, n, body = fg(rel)
        check(f"/files/get {label}: refused (the access rule's secrets)", s in (403, 404) and n < 100, f"HTTP {s} {n} bytes {body[:30]!r}")
    s = get(f"/files/get?thread={q('t000-nope')}&path={q('mfg/big.bin')}", bearer(DEV))[0]
    check("/files/get unknown thread: 404", s == 404, f"HTTP {s}")


def shares():
    d = Data(DEV); d.next_json()
    t2 = st["t2"]
    ans = mcp_call(d, t2, {"path": BIG, "expires": "1h"}, "big")
    link, sid = share_of(ans)
    check("MCP file_share (stand-in agent, thread token) answers a link", bool(link) and bool(sid), ans[:160])
    if link:
        t0 = time.time()
        s, h, sha, n, _ = get(link)
        check("/s/<token>/<name> with no credential from box B: 200, sha256 matches", s == 200 and sha == ARGS["sha"], f"HTTP {s}, {n} bytes in {time.time() - t0:.1f}s")
        s, h, _, n, _ = get(link, {"Range": "bytes=0-9"})
        check("/s/ share: ranged 206", s == 206 and n == 10, f"HTTP {s}")
        s = get(link[:-3] + "zzz")[0]
        print(f"info  [shares] /s/<token>/<another name>: HTTP {s} (the token alone picks the share)", flush=True)
        s = get("/s/" + "0" * 48 + "/big.bin")[0]
        check("/s/ unknown token: 404", s == 404, f"HTTP {s}")
    ans2 = mcp_call(d, t2, {"path": f"{P}/.env"}, "env")
    check("MCP file_share of .env: refused", "isError" in ans2 and "/s/" not in ans2, ans2[:160])
    r = d.request("shares.list")
    listed = [x.get("id") for x in ((r.get("result") or {}).get("shares") or [])]
    check("data/1 shares.list lists the share", sid in listed, json.dumps(r.get("error", ""))[:160] if not r.get("ok") else ",".join(listed))
    r = d.request("shares.revoke", {"id": sid})
    via = "data/1"
    if not check("data/1 shares.revoke answers ok", r.get("ok") is True, json.dumps(r.get("error", ""))[:200]):
        # the hub RPC on the plain /ws socket with the hub token
        w = Data(HUB, "/ws?token=" + q(HUB))
        try:
            r = w.request("shares.revoke", {"id": sid}); via = "/ws hub RPC"
        except Exception as e:  # noqa: BLE001
            r = {"error": str(e)}
        w.close()
        check("shares.revoke over the hub's /ws RPC (fallback)", r.get("ok") is True, json.dumps(r.get("error", ""))[:200])
    if link:
        s = get(link)[0]
        check(f"/s/ after shares.revoke ({via}): 404", s == 404, f"HTTP {s}")
    ans3 = mcp_call(d, t2, {"path": f"{P}/mech/bracket.step", "expires": "2s"}, "exp")
    link3, _ = share_of(ans3)
    s = get(link3)[0] if link3 else 0
    check("short share (expires 2s): 200 at once", s == 200, f"HTTP {s} {ans3[:100] if not link3 else ''}")
    time.sleep(3.5)
    s = get(link3)[0] if link3 else 0
    check("short share after expiry: 404", s == 404, f"HTTP {s}")
    d.close()


def revoke():
    d = Data(DEV); d.next_json()
    c = http.client.HTTPConnection(HOST, PORT, timeout=20)
    c.request("POST", "/auth/revoke", headers={**bearer(HUB), "X-Backplane-Device": st["dev_id"]}); r = c.getresponse(); r.read(); c.close()
    check("/auth/revoke of box B's device", r.status == 200, f"HTTP {r.status}")
    s = get(FILE(BIG), bearer(DEV))[0]
    check("/file with the revoked device: 401", s == 401, f"HTTP {s}")
    s = get(f"/files/get?thread={q(st['t1'])}&path={q('mfg/big.bin')}", bearer(DEV))[0]
    check("/files/get with the revoked device: 401", s == 401, f"HTTP {s}")
    check("the revoked device's data/1 socket is closed", d.closed_within(8))
    d.close()


try:
    {"setup": setup, "download": download, "checks": checks, "stale": stale, "deny": deny, "shares": shares, "revoke": revoke}[PHASE]()
except Exception as e:  # noqa: BLE001
    check(f"phase ran to the end", False, f"{type(e).__name__}: {e}")
print(f"summary [{PHASE}] pass={sum(RES)} fail={len(RES) - sum(RES)}", flush=True)
sys.exit(0 if all(RES) else 1)
