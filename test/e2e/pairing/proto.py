#!/usr/bin/env python3
"""Pairing protocol probe: drives a running temp-home hub over HTTP and its data/1 WebSocket.

This is a protocol result, not a client result: it says what the hub does for a caller on this
machine over this route. It never stands in for a GPUI, iOS or Android cell of the matrix.

  proto.py --base http://100.x.y.z:3801 --token-file HOME/token [--restart 'CMD'] [--expire 'CMD']

--restart runs a shell command that stops the hub and starts it again on the same home.
--expire  runs a shell command that stops the hub, moves every open invitation's expiresAt into the
          past in devices-v1.json, and starts it again. The hub has no test clock, so that is how an
          expired code is reached without a ten minute wait; the report says so.
Either left out: those checks are reported `not run`, never passed.

Enrollment allows ten attempts a minute per hub, so the checks wait out the bucket between groups.
Credentials are never printed. Exit 1 when a check failed. Python 3.9, standard library only.
"""
import argparse, base64, http.client, json, os, socket, ssl, struct, subprocess, sys, time, urllib.parse

RESULTS = []


def report(name, result, detail=""):
    RESULTS.append((name, result, detail))
    print(f"{result:8} {name}{'  ' + detail if detail else ''}", flush=True)


def check(name, ok, detail=""):
    report(name, "pass" if ok else "fail", detail)
    return ok


class Hub:
    def __init__(self, base, token):
        self.base, self.root = base.rstrip("/"), token
        u = urllib.parse.urlsplit(self.base)
        self.tls, self.host, self.port = u.scheme == "https", u.hostname, u.port or (443 if u.scheme == "https" else 80)

    def call(self, method, path, headers=None):
        make = http.client.HTTPSConnection if self.tls else http.client.HTTPConnection
        c = make(self.host, self.port, timeout=10)
        try:
            c.request(method, path, headers=headers or {})
            r = c.getresponse()
            raw = r.read()
        finally:
            c.close()
        try:
            return r.status, json.loads(raw)
        except ValueError:
            return r.status, {"raw": raw[:120].decode("utf-8", "replace")}

    def bearer(self, secret=None):
        return {"Authorization": f"Bearer {secret or self.root}"}

    def invite(self):
        return self.call("POST", "/auth/invitations", self.bearer())

    def enroll(self, secret, name="e2e probe"):
        return self.call("POST", "/auth/enroll", {**self.bearer(secret), "X-Backplane-Client": "native", "X-Backplane-Name": name})

    def devices(self):
        return self.call("GET", "/auth/devices", self.bearer())

    def revoke_device(self, ident):
        return self.call("POST", "/auth/revoke", {**self.bearer(), "X-Backplane-Device": ident})

    def revoke_invite(self, ident):
        return self.call("POST", "/auth/revoke", {**self.bearer(), "X-Backplane-Invitation": ident})

    def wait_up(self, seconds=30):
        end = time.time() + seconds
        while time.time() < end:
            try:
                if self.call("GET", "/hello")[0] == 200:
                    return True
            except OSError:
                pass
            time.sleep(0.5)
        return False


class Socket:
    """A data/1 socket: just enough WebSocket to read the hello frame and notice a close."""

    def __init__(self, hub, token, path=None):
        s = socket.create_connection((hub.host, hub.port), timeout=10)
        if hub.tls:
            s = ssl.create_default_context().wrap_socket(s, server_hostname=hub.host)
        self.s, self.buf = s, b""
        key = base64.b64encode(os.urandom(16)).decode()
        path = path or "/ws?data=1&enc=json&token=" + urllib.parse.quote(token)
        s.sendall((f"GET {path} HTTP/1.1\r\nHost: {hub.host}:{hub.port}\r\nConnection: Upgrade\r\nUpgrade: websocket\r\n"
                   f"Sec-WebSocket-Version: 13\r\nSec-WebSocket-Key: {key}\r\n\r\n").encode())
        while b"\r\n\r\n" not in self.buf:
            d = s.recv(4096)
            if not d:
                break
            self.buf += d
        head, _, self.buf = self.buf.partition(b"\r\n\r\n")
        parts = head.split(b" ", 2)
        self.status = int(parts[1]) if len(parts) > 1 and parts[1].isdigit() else 0

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

    def hello(self, seconds=10):
        self.s.settimeout(seconds)
        try:
            while True:
                op, body = self.frame()
                if op == 8:
                    return None
                if op == 1:
                    j = json.loads(body)
                    if j.get("t") == "hello":
                        return j
        except (OSError, EOFError, ValueError):
            return None

    def closed_within(self, seconds):
        """True when the hub ends this socket inside the time. Heartbeats are read past."""
        self.s.settimeout(seconds)
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


def live_socket(hub, token):
    s = Socket(hub, token)
    return s, s.status == 101 and s.hello() is not None


def secret_of(link):
    return dict(urllib.parse.parse_qsl(link.split("#", 1)[1]))["invite"]


def bucket(hub):
    print("-- waiting out the enrollment bucket", flush=True)
    time.sleep(61)


def run_shell(cmd):
    return subprocess.run(cmd, shell=True, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL).returncode == 0


def main():
    a = argparse.ArgumentParser()
    a.add_argument("--base", required=True)
    a.add_argument("--token-file", required=True)
    a.add_argument("--restart", default="")
    a.add_argument("--expire", default="")
    o = a.parse_args()
    hub = Hub(o.base, open(o.token_file).read().strip())
    print(f"route {hub.base} from {socket.gethostname()} (protocol probe, not a client result)", flush=True)

    # refused without a credential
    check("no credential: invitations refused", hub.call("POST", "/auth/invitations")[0] == 401)
    check("no credential: devices refused", hub.call("GET", "/auth/devices")[0] == 401)
    s = Socket(hub, "0" * 32)
    check("unknown device token: socket refused", s.status != 101, f"status {s.status}")
    s.close()

    # link
    st, inv = hub.invite()
    if not check("invitation issued", st == 200 and "link" in inv, f"status {st}"):
        return finish()
    check("invitation has a link, QR and short code", bool(inv.get("link")) and bool(inv.get("qr")) and len(inv.get("code", "")) == 8)
    check("the link's route is this hub's reachable address", not inv.get("localOnly", True) or "127." in hub.base, f"first route {inv['routes'][0]}")
    st, dev = hub.enroll(secret_of(inv["link"]), "probe link")
    if not check("link enrolls", st == 200 and "token" in dev, f"status {st}"):
        return finish()
    s, ok = live_socket(hub, dev["token"])
    check("device credential opens a data/1 socket", ok, f"status {s.status}")
    s.close()

    # the QR carries the same link: the hub encodes it, the camera side is the client's
    st, inv2 = hub.invite()
    st, e = hub.enroll(inv2["code"].lower()[:4] + "-" + inv2["code"].lower()[4:], "probe code")
    check("short code enrolls (lower case, with a dash)", st == 200 and "token" in e, f"status {st}")
    code_dev = e.get("token", "")

    # used, wrong
    st, e = hub.enroll(secret_of(inv["link"]))
    check("used link refused", st == 400 and e.get("error") == "used_code", f"{st} {e.get('error')}")
    st, e = hub.enroll(inv2["code"])
    check("used code refused", st == 400 and e.get("error") == "used_code", f"{st} {e.get('error')}")
    st, e = hub.enroll("ZZZZZZZZ")
    check("wrong code refused", st == 400 and e.get("error") == "wrong_code", f"{st} {e.get('error')}")

    # revoked, unused
    st, inv3 = hub.invite()
    hub.revoke_invite(inv3["id"])
    st, e = hub.enroll(inv3["code"])
    check("revoked code refused", st == 400 and e.get("error") == "revoked_code", f"{st} {e.get('error')}")

    # rate limit: the attempts above plus these pass ten inside the minute
    last = None
    for _ in range(11):
        last = hub.enroll("ZZZZZZZZ")
    check("guessing is rate limited", last[0] == 429 and last[1].get("error") == "rate_limited", f"{last[0]} {last[1].get('error')}")
    bucket(hub)

    # revoking a device ends its socket and its credential
    s, ok = live_socket(hub, dev["token"])
    check("device connected before revoke", ok)
    st, r = hub.revoke_device(dev["id"])
    check("device revoke accepted", st == 200 and r.get("ok") is True, f"status {st}")
    check("revoke closes the device's open socket", s.closed_within(8))
    s.close()
    s = Socket(hub, dev["token"])
    check("revoked credential: socket refused", s.status != 101, f"status {s.status}")
    s.close()
    check("revoked credential: HTTP refused", hub.call("GET", "/auth/devices", hub.bearer(dev["token"]))[0] == 401)

    # re-pair after revoke
    st, inv4 = hub.invite()
    st, again = hub.enroll(inv4["code"], "probe re-paired")
    check("re-pair after revoke enrolls", st == 200 and "token" in again, f"status {st}")
    s, ok = live_socket(hub, again.get("token", ""))
    check("re-paired device opens a socket", ok)
    check("re-pair is a new credential", again.get("token") != dev["token"])
    s.close()

    # revoking a used invitation takes its device with it
    s, ok = live_socket(hub, again["token"])
    hub.revoke_invite(inv4["id"])
    check("revoking a used link closes its device's socket", s.closed_within(8))
    s.close()
    st, listed = hub.devices()
    gone = next((d for d in listed.get("devices", []) if d.get("id") == again.get("id")), {})
    check("that device lists as revoked", bool(gone.get("revoked")))
    check("no hash reaches the devices list", all("hash" not in d for d in listed.get("devices", [])))

    # a hub restart
    if o.restart:
        st, keep = hub.invite()
        s, ok = live_socket(hub, code_dev)
        check("device connected before restart", ok)
        run_shell(o.restart)
        check("hub back after restart", hub.wait_up())
        check("the old socket ended with the old hub", s.closed_within(3))
        s.close()
        s, ok = live_socket(hub, code_dev)
        check("device reconnects after restart, same credential", ok)
        s.close()
        st, e = hub.enroll(keep["code"], "probe after restart")
        check("an invitation open across a restart still works", st == 200 and "token" in e, f"{st} {e.get('error')}")
    else:
        report("reconnect after hub restart", "not run", "no --restart given")

    # expired
    if o.expire:
        st, late = hub.invite()
        run_shell(o.expire)
        check("hub back after the registry edit", hub.wait_up())
        st, e = hub.enroll(late["code"])
        check("expired code refused (expiresAt moved back in devices-v1.json, hub restarted)", st == 400 and e.get("error") == "expired_code", f"{st} {e.get('error')}")
    else:
        report("expired code", "not run", "no --expire given")
    return finish()


def finish():
    counts = {k: sum(1 for r in RESULTS if r[1] == k) for k in ("pass", "fail", "not run")}
    print(f"summary pass={counts['pass']} fail={counts['fail']} not_run={counts['not run']}", flush=True)
    return 1 if counts["fail"] else 0


if __name__ == "__main__":
    sys.exit(main())
