"""Rate limits: sliding windows per client IP (an IPv6 /64 counts as one
address, since one subscriber is handed the whole block) and per install.

nginx limits bursts in front of this; these are the hourly and daily budgets.
The counters live in memory: the server runs one process, and a restart only
forgets a few hours of counts.
"""
import collections
import ipaddress
import threading
import time

_lock = threading.Lock()
_hits = collections.defaultdict(collections.deque)

# (window seconds, max hits) per scope; every scope is checked per IP and per install
RULES = {
    # the "from $X" number: cheap and cached, asked each time a thread is opened or its turn
    # ends, so a busy person asks often; a limit they reach would hide their Buy button
    "estimate": [(3600, 1500), (86400, 12000)],
    # a full quote: files are unpacked, measured and every part is priced
    "quote": [(3600, 20), (86400, 60)],
    # polling a quote or an order
    "read": [(3600, 1200)],
    # opening a checkout
    "pay": [(3600, 30)],
}
# across everyone: what keeps one abuser from spending the distributor APIs' daily quota
GLOBAL = {"quote": (86400, 400)}


def client_key(ip):
    try:
        a = ipaddress.ip_address(ip)
    except ValueError:
        return "ip:" + ip
    if a.version == 6:
        if a.ipv4_mapped is not None:
            return "ip:" + str(a.ipv4_mapped)
        return "ip6:" + str(ipaddress.ip_network(f"{a}/64", strict=False).network_address)
    return "ip:" + str(a)


def _take(key, window, limit, now):
    d = _hits[key]
    while d and d[0] <= now - window:
        d.popleft()
    if len(d) >= limit:
        return d[0] + window - now
    return 0


def check(scope, ip, install=""):
    """0 when allowed (and counted), else seconds until the next hit is allowed."""
    now = time.time()
    keys = [client_key(ip)] + (["in:" + install] if install else [])
    with _lock:
        wait = 0
        for k in keys:
            for window, limit in RULES[scope]:
                wait = max(wait, _take(f"{scope}|{window}|{k}", window, limit, now))
        if scope in GLOBAL:
            window, limit = GLOBAL[scope]
            wait = max(wait, _take(f"{scope}|global", window, limit, now))
        if wait:
            return int(wait) + 1
        for k in keys:
            for window, _ in RULES[scope]:
                _hits[f"{scope}|{window}|{k}"].append(now)
        if scope in GLOBAL:
            _hits[f"{scope}|global"].append(now)
        return 0


def sweep():
    """Drop windows that are empty, so idle keys do not pile up."""
    now = time.time()
    with _lock:
        for k in list(_hits):
            window = int(k.split("|")[1]) if k.count("|") >= 2 else GLOBAL[k.split("|")[0]][0]
            d = _hits[k]
            while d and d[0] <= now - window:
                d.popleft()
            if not d:
                del _hits[k]
