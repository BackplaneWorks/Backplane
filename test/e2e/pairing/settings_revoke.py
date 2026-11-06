#!/usr/bin/env python3
"""Revoke from Settings: the view.action a Settings button sends, against a running temp-home hub.

Settings -> Devices acts through the presenter, not POST /auth/revoke, so proto.py does not cover it.
Each action must end the credential AND the device's open socket.

  settings_revoke.py --base http://HOST:PORT --token-file HOME/token
"""
import argparse, json, os, re, struct, sys, time, urllib.parse
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from proto import Hub, RESULTS, Socket, check, finish, live_socket, secret_of


def dictionaries():
    """KEYS and WORDS from the hub's own source: append-only, and the wire depends on their order."""
    src = open(os.path.join(os.path.dirname(os.path.abspath(__file__)), "../../../native/host/src/wire_dict.rs")).read()
    def words(name):
        return re.findall(r'"((?:[^"\\\\]|\\\\.)*)"', re.search(r"pub const " + name + r": &\[&str\] = &\[(.*?)\];", src, re.S).group(1))
    return words("KEYS"), words("WORDS")


KEYS, WORDS = dictionaries()


def cbor(b, i=0):
    """The hub's CBOR: an integer map key is KEYS[n], tag 6 is WORDS[n]. Returns (value, next offset)."""
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


class View(Socket):
    """The view socket a client holds: CBOR frames from the hub; requests go as JSON text, which it also takes."""

    def __init__(self, hub):
        self.hub, self.rev, self.next, self.view_id = hub, 0, (1 << 47) + 1000, ""
        self.path_token = hub.root
        super().__init__(hub, hub.root, "/ws?view=1&mobile=1&token=" + urllib.parse.quote(hub.root))
        self.s.settimeout(10)
        self.greeting = self.until(lambda j: j.get("t") == "view")

    def until(self, want):
        while True:
            op, body = self.frame()
            if op == 2:
                j = cbor(body)[0]
                if isinstance(j, dict) and want(j):
                    return j

    def send(self, obj):
        data, mask = json.dumps(obj).encode(), os.urandom(4)
        n = len(data)
        head = bytes([0x81]) + (bytes([0x80 | n]) if n < 126 else bytes([0x80 | 126]) + struct.pack(">H", n) if n < 65536 else bytes([0x80 | 127]) + struct.pack(">Q", n))
        self.s.sendall(head + mask + bytes(b ^ mask[i % 4] for i, b in enumerate(data)))

    def request(self, method, extra):
        self.next += 1
        self.rev += 1
        p = {"version": 1, "hub": self.hub.base, "generation": self.greeting["generation"], "epoch": self.greeting["epoch"], "requestRevision": self.rev,
             "target": {"kind": "recent", "id": ""}, "viewId": self.view_id, "session": {"drafts": {}}, "cid": "e2e-settings-revoke", "nativeSidebar": 1, **extra}
        i = self.next
        self.send({"id": i, "m": method, "p": p})
        r = self.until(lambda j: str(j.get("id")) == str(i))  # the hub answers ids as strings
        self.view_id = (r.get("result") or r).get("viewId", self.view_id) if isinstance(r.get("result") or r, dict) else self.view_id
        return r


def paired(hub, name):
    st, inv = hub.invite()
    st, dev = hub.enroll(secret_of(inv["link"]), name)
    return inv, dev


def main():
    a = argparse.ArgumentParser()
    a.add_argument("--base", required=True)
    a.add_argument("--token-file", required=True)
    o = a.parse_args()
    hub = Hub(o.base, open(o.token_file).read().strip())
    v = View(hub)
    r = v.request("view.open", {})
    if not check("settings view opens", r.get("ok") is not False, json.dumps(r.get("error", ""), default=str)[:200]):
        return finish()

    for action, label in (("device-revoke", "Settings revoke of a device"), ("device-uninvite", "Settings revoke of a used link")):
        inv, dev = paired(hub, label)
        s, ok = live_socket(hub, dev["token"])
        check(f"{label}: device connected first", ok)
        r = v.request("view.action", {"action": action, "value": dev["id"] if action == "device-revoke" else inv["id"]})
        check(f"{label}: action accepted", r.get("ok") is not False, json.dumps(r.get("error", ""), default=str)[:200])
        check(f"{label}: credential refused after", hub.call("GET", "/auth/devices", hub.bearer(dev["token"]))[0] == 401)
        check(f"{label}: open socket closed", s.closed_within(8))
        s.close()
    v.close()
    return finish()


if __name__ == "__main__":
    sys.exit(main())
