#!/usr/bin/env python3
"""The rules of the phones' HubList (mobile/ios/Backplane/HubList.swift, mobile/android/.../HubList.kt) in Python, run
against test/mobile/hubs_cases.json: it keeps the table honest where no Swift or Kotlin toolchain is at hand."""
import json, os, sys, urllib.parse


def origin(link):
    s = link.strip()
    if "://" not in s:
        return None
    scheme, rest = s.split("://", 1)
    scheme = scheme.lower()
    auth = rest
    for c in "/?#":
        auth = auth.split(c, 1)[0]
    if scheme not in ("http", "https") or not auth or "@" in auth:
        return None
    return f"{scheme}://{auth}"


def authority(link):
    o = origin(link.replace("ws://", "http://", 1).replace("wss://", "https://", 1))
    return o.split("://", 1)[1] if o else None


def adopt(links, link, key):
    k = key(link)
    return [link] + [x for x in links if key(x) != k]


def use(links, k, key):
    hit = [x for x in links if key(x) == k]
    return hit[:1] + [x for x in links if key(x) != k] if hit else links


def forget(links, k, key):
    return [x for x in links if key(x) != k]


def routes(text):
    first = origin(text)
    out = [first] if first else []
    frag = text.split("#", 1)[1] if "#" in text else ""
    for kv in frag.split("&"):
        if not kv.startswith("routes="):
            continue
        try:
            xs = json.loads(urllib.parse.unquote_plus(kv[len("routes="):]))
        except ValueError:
            xs = []
        for x in xs if isinstance(xs, list) else []:
            o = origin(x) if isinstance(x, str) else None
            if o and o not in out:
                out.append(o)
    return out


def loopback(o):
    r = o.split("://", 1)[1] if "://" in o else ""
    return r.startswith("localhost") or r.startswith("127.") or r.startswith("[::1]")


def local(rs):
    return bool(rs) and all(loopback(x) for x in rs)


def main():
    t = json.load(open(os.path.join(os.path.dirname(os.path.abspath(__file__)), "hubs_cases.json")))
    bad = 0
    def check(ok, name, got):
        nonlocal bad
        if not ok:
            bad += 1; print("FAIL", name, got)
    for c in t["adopt"]:
        g = adopt(c["links"], c["link"], authority); check(g == c["want"], c["name"], g)
    for c in t["use"]:
        g = use(c["links"], c["key"], authority); check(g == c["want"], c["name"], g)
    for c in t["forget"]:
        g = forget(c["links"], c["key"], authority); check(g == c["want"], c["name"], g)
    for c in t["routes"]:
        g = routes(c["text"]); check(g == c["want"] and local(g) == c["local"], c["name"], (g, local(g)))
    n = sum(len(t[k]) for k in ("adopt", "use", "forget", "routes"))
    print(f"{n - bad}/{n} cases")
    return 1 if bad else 0


if __name__ == "__main__":
    sys.exit(main())
