#!/usr/bin/env python3
"""The way from 0.14, end to end, on this machine (docs/deploy.md, "From 0.14"): a real 0.14.1 install updates itself to a 0.15 release dry run.

    from-014.py V014_DIR RELEASE_DIR [--panics | --crash-import GOOD_DIR] [--large P,T,L] [--throttle MS]

V014_DIR: 0.14.1's published install.sh and this platform's archive (gh release download v0.14.1 -p install.sh -p 'backplane-v0.14.1-<plat>.tar.gz').
RELEASE_DIR: one release.yml dry run's artifact (release-dry-run): the 0.14-format archives, SHA256SUMS, the -native archives, the signed manifest.
--panics: RELEASE_DIR's 0.15 dies at its start; then 0.14 must be back on its port and the 0.15 build marked bad.

Everything is private: HOME, 0.14's prefix and home, 0.15's home, a free port. 0.14's GitHub calls go through a curl in front of the real one that
answers the release list from a loopback feed (0.14 has no feed setting). The Update press is 0.14's own `app.update` request on its socket.
Checks: 0.14 laid 0.15 out and re-exec'd into the shim; the guard runs 0.15 on 0.14's port, proven with its identity; 0.14 is set aside to go
back to; 0.14's home is unchanged; a device paired to 0.14 is accepted by 0.15 after the import (GET /import/0.14/run, the 0.14-import lane's).
"""
import hashlib, http.client, json, os, shutil, signal, socket, struct, subprocess, sys, tempfile, time, base64, urllib.parse

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, os.path.join(HERE, "..", "e2e", "pairing"))
from proto import Hub, Socket as Ws  # noqa: E402

v014, rel = os.path.abspath(sys.argv[1]), os.path.abspath(sys.argv[2])
panics = "--panics" in sys.argv
def opt(name, default=""):
    return sys.argv[sys.argv.index(name) + 1] if name in sys.argv else default
# --large P,T,L: a 0.14 home shaped like a person's, written by the import lane's test/fixtures/make-v014-home.py (P projects, T threads, about L lines);
# --throttle MS: the import slowed on purpose (BACKPLANE_IMPORT_THROTTLE_MS, test-only), so it runs past the guard's probation window
large, throttle = opt("--large"), opt("--throttle")
# --crash-import GOOD_DIR: RELEASE_DIR's 0.15 dies once partway through the import (a DRY RUN ONLY build); the guard must fail it although it is migrating and
# go back to 0.14, whose home 0.15 never touched; then GOOD_DIR's release (another dry run's artifact) is laid out and moved to, and finishes the import:
# every 0.14 thread and message it keeps is in 0.15's log exactly once
crash = opt("--crash-import")
# --expect-from FILE: a clean import of the same home (a plain run's kept import-result.json): the resumed import must reach its threads and messages
expect_once = json.load(open(opt("--expect-from"))).get("once") if opt("--expect-from") else None
PASS, FAIL = [], []
def ids(path, kind, key):   # how often each id of a kind of line is in a log
    n = {}
    for l in open(path, errors="replace"):
        if f'"{kind}"' not in l: continue
        try: v = json.loads(l)
        except Exception: continue
        if v.get("$") == kind: n[v.get(key)] = n.get(v.get(key), 0) + 1
    return n
def once(h14, h15):   # per kind: 0.14's ids that are in 0.15's log more than once, and exactly once
    out = {}
    for kind, key in (("ThreadCreated", "id"), ("MessagePosted", "msg")):
        old, new = ids(os.path.join(h14, "events.jsonl"), kind, key), ids(os.path.join(h15, "events.jsonl"), kind, key)
        out[kind] = {"twice": sum(1 for k in old if new.get(k, 0) > 1), "once": sum(1 for k in old if new.get(k, 0) == 1)}
    return out
def ok(name, detail=""): PASS.append(name); print(f"PASS {name:12} {detail}", flush=True)
def no(name, detail=""): FAIL.append(name); print(f"FAIL {name:12} {detail}", flush=True)
def free_port():
    s = socket.socket(); s.bind(("127.0.0.1", 0)); p = s.getsockname()[1]; s.close(); return p
def until(sec, f):
    t = time.time() + sec
    while time.time() < t:
        try:
            v = f()
            if v: return v
        except Exception: pass
        time.sleep(1)
    return None

plat = {"Linux": "linux", "Darwin": "darwin"}[os.uname().sysname] + "-" + ("arm64" if os.uname().machine in ("arm64", "aarch64") else "x64")
manifest = json.load(open(os.path.join(rel, "manifest.json"))); tag = manifest["tag"]
T = tempfile.mkdtemp(prefix=".from-014.", dir=os.environ.get("UPD_DIR", os.path.join(HERE, "..", "..", "build")))
home_user, prefix, binp, h14, h15, feed = (os.path.join(T, x) for x in ("userhome", "prefix", "bin", "home-014", "home-015", "feed"))
for d in (home_user, prefix, binp, h14, h15, feed, os.path.join(T, "shim")): os.makedirs(d, exist_ok=True)
port, fport = free_port(), free_port()
real_curl = shutil.which("curl")

# the feed: GitHub's answer for /releases/latest, the assets on loopback
for f in os.listdir(rel): os.link(os.path.join(rel, f), os.path.join(feed, f)) if os.stat(rel).st_dev == os.stat(feed).st_dev else shutil.copy(os.path.join(rel, f), feed)
latest = {"tag_name": tag, "prerelease": False, "draft": False, "assets": [{"name": n, "browser_download_url": f"http://127.0.0.1:{fport}/{n}"} for n in sorted(os.listdir(feed))]}
open(os.path.join(feed, "latest.json"), "w").write(json.dumps(latest)); open(os.path.join(feed, "list.json"), "w").write(json.dumps([latest]))
fsrv = subprocess.Popen([sys.executable, "-I", "-m", "http.server", str(fport), "--bind", "127.0.0.1"], cwd=feed, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
# curl in front of the real one: 0.14.1's own download from GitHub comes from V014_DIR, its release list from the feed
shim = os.path.join(T, "shim", "curl")
open(shim, "w").write(f"""#!/bin/sh
for a in "$@"; do case $a in
  https://api.github.com/repos/*/releases/latest) set -- "$@" "http://127.0.0.1:{fport}/latest.json" ;;
  https://api.github.com/repos/*/releases\\?*) set -- "$@" "http://127.0.0.1:{fport}/list.json" ;;
  https://github.com/*/releases/download/v0.14.1/*) set -- "$@" "file://{v014}/${{a##*/}}" ;;
  --proto|--proto-redir) set -- "$@" "$a"; skip=1 ;;
  *) if [ -n "${{skip:-}}" ]; then set -- "$@" "=http,https,file"; skip=; else set -- "$@" "$a"; fi ;;
esac; shift; done
exec {real_curl} "$@"
""")
os.chmod(shim, 0o755)
env = {k: v for k, v in os.environ.items() if not k.startswith("BACKPLANE_")}
env.update(HOME=home_user, PATH=os.path.join(T, "shim") + ":" + env.get("PATH", "/usr/bin:/bin"), BACKPLANE_PREFIX=prefix, BACKPLANE_BIN=binp,
           BACKPLANE_NO_DESKTOP_APP="1", BACKPLANE_NO_DESKTOP="1", BACKPLANE_NEW_HOME=h15, BACKPLANE_GUARD_STABLE="10")
procs = []
def stop():
    g = os.path.join(prefix, "backplane")
    if os.path.exists(os.path.join(h15, "guard", "lock")): subprocess.run([g, "hub", "stop", "--home", h15], env=env, capture_output=True, timeout=60)
    for p in procs:
        try: os.killpg(p.pid, signal.SIGTERM)
        except Exception: pass
    fsrv.terminate()

try:
    # 1. 0.14.1, installed by its own published installer
    r = subprocess.run(["sh", os.path.join(v014, "install.sh")], env=env, capture_output=True, text=True, timeout=900)
    serve = os.path.join(prefix, "backplane-serve")
    if r.returncode != 0 or not os.access(serve, os.X_OK): no("install-014", (r.stdout + r.stderr)[-300:]); raise SystemExit
    ok("install-014", f"0.14.1's install.sh laid out {prefix}")
    if large:
        pp, tt, ll = large.split(",")
        r = subprocess.run([sys.executable, "-I", os.path.join(HERE, "..", "fixtures", "make-v014-home.py"), h14, "--large", pp, tt, ll], capture_output=True, text=True, timeout=1800)
        if r.returncode != 0: no("large-home", (r.stdout + r.stderr)[-300:]); raise SystemExit
        lines = sum(1 for _ in open(os.path.join(h14, "events.jsonl"), "rb"))
        ok("large-home", f"a 0.14 home of {pp} projects, {tt} threads, {lines} lines")
    if throttle: env["BACKPLANE_IMPORT_THROTTLE_MS"] = throttle
    log014 = open(os.path.join(T, "serve-014.log"), "ab")
    p = subprocess.Popen([serve, "--home", h14, "--port", str(port), "--no-tailscale", "--foreground"], env=env, stdout=log014, stderr=log014, stdin=subprocess.DEVNULL, start_new_session=True)
    procs.append(p)
    base = f"http://127.0.0.1:{port}"
    tok = until(900 if large else 90, lambda: open(os.path.join(h14, "token")).read().strip() if Hub(base, "x").call("GET", "/hello")[0] == 200 else None)
    if not tok: no("start-014", "0.14 never answered"); raise SystemExit
    ok("start-014", f"0.14.1 serves {base}")
    # a phone paired to 0.14: its token must reach 0.15 through the import
    hub14 = Hub(base, tok); st, inv = hub14.invite(); secret = urllib.parse.parse_qs(urllib.parse.urlsplit(inv["link"]).fragment).get("invite", [""])[0]
    st, en = hub14.enroll(secret, "phone paired to 0.14"); device = en.get("token") or en.get("deviceToken") or ""
    if not device: no("pair-014", f"enroll {st} {str(en)[:120]}")
    else: ok("pair-014", "a device paired to 0.14")

    # 2. Update, as the person presses it: 0.14's own request, a CBOR map on its socket
    def cbor(v):
        def head(major, n):
            if n < 24: return bytes([major << 5 | n])
            for k, f in ((0xff, ">B"), (0xffff, ">H"), (0xffffffff, ">I")):
                if n <= k: return bytes([major << 5 | {">B": 24, ">H": 25, ">I": 26}[f]]) + struct.pack(f, n)
        if isinstance(v, int): return head(0, v)
        if isinstance(v, str): b = v.encode(); return head(3, len(b)) + b
        if isinstance(v, dict): return head(5, len(v)) + b"".join(cbor(k) + cbor(x) for k, x in v.items())
    time.sleep(20)   # 0.14 looks 15 s after its start; the press works either way
    w = Ws(hub14, tok, path="/ws?token=" + urllib.parse.quote(tok))
    def send(i, m):
        body = cbor({"id": i, "m": m, "p": {}}); mask = os.urandom(4)
        w.s.sendall(bytes([0x82, 0x80 | 126]) + struct.pack(">H", len(body)) + mask + bytes(b ^ mask[i % 4] for i, b in enumerate(body)) if len(body) >= 126 else
                    bytes([0x82, 0x80 | len(body)]) + mask + bytes(b ^ mask[i % 4] for i, b in enumerate(body)))
    send(1, "app.update")
    # with threads at work 0.14 waits for them (update.wait, up to 30 minutes) and the person presses Update now: update.now (a large home has turns open)
    time.sleep(2); send(2, "update.now")
    # read what 0.14 sends on the socket, so a large home's frames never fill it
    import threading
    def drain():
        try:
            w.s.settimeout(None)
            while w.s.recv(1 << 20): pass
        except Exception: pass
    threading.Thread(target=drain, daemon=True).start()
    shimmed = until(600, lambda: "BACKPLANE-0.15-SHIM" in open(serve, errors="replace").read())
    if not shimmed: no("update-014", "0.14 never laid 0.15 out: " + open(os.path.join(T, "serve-014.log"), errors="replace").read()[-400:]); raise SystemExit
    ok("update-014", f"0.14's updater took {tag} from SHA256SUMS and ran its install-release.sh; 0.14 kept as backplane-serve.0.14={os.path.exists(serve + '.0.14')}")
    # 0.14's home: every file's sha256. 0.14 itself writes there while it runs (and again as v0.14-legacy before the move); 0.15 never may
    def tree(d):
        out = {}
        for r, _, fs in os.walk(d):
            for f in fs:
                p = os.path.join(r, f)
                try: out[os.path.relpath(p, d)] = hashlib.sha256(open(p, "rb").read()).hexdigest()
                except OSError: pass
        return out
    def marks():   # what only a 0.15 hub or its guard would leave in a home
        return [m for m in ("guard", "update", "ckpt", "hub.owner", "quarantine", "update-from-0.14.log") if os.path.exists(os.path.join(h14, m))]

    def gstat():
        try: return json.load(open(os.path.join(h15, "guard", "status.json")))
        except Exception: return {}
    def hello():
        c = http.client.HTTPConnection("127.0.0.1", port, timeout=3); c.request("GET", "/hello"); return c.getresponse().read()
    if panics:
        bad = until(400, lambda: tag in open(os.path.join(prefix, "BAD")).read())
        back = until(240, lambda: gstat().get("build") == "v0.14-legacy" and gstat().get("stable") is True)
        if bad and back: ok("back-to-014", f"{tag} died, marked bad, the guard runs 0.14 again on {port}: {gstat().get('text')}")
        else: no("back-to-014", f"bad={bool(bad)} build={gstat().get('build')} state={gstat().get('state')}")
    elif crash:
        def health():
            c = http.client.HTTPConnection("127.0.0.1", port, timeout=5); c.request("GET", "/healthz"); r = c.getresponse(); return r.status, json.loads(r.read() or b"{}")
        def bad(): return os.path.exists(os.path.join(prefix, "BAD")) and tag in open(os.path.join(prefix, "BAD")).read()
        # 1. 0.15 imports and dies partway: its progress shows, then the guard fails it although it was migrating
        seen, before, at_bad, t0 = [], None, None, time.time()
        while time.time() < t0 + 1800 and not bad():
            g = gstat()
            if g.get("build") == tag and before is None: before = tree(h14)   # 0.14 stopped, 0.15 started: 0.14's home as 0.15 found it
            try:
                st, h = health(); m = h.get("migrating")
                if isinstance(m, dict) and m.get("state") == "running": seen.append((round(time.time() - t0), m.get("done", 0), m.get("total", 0), g.get("stable")))
            except Exception: pass
            time.sleep(0.25)
        if bad(): at_bad = tree(h14)   # at once, before the guard's 0.14 runs again (0.14 writes its own home as it runs)
        died = open(os.path.join(h15, "dry-run-crashed")).read().strip() if os.path.exists(os.path.join(h15, "dry-run-crashed")) else ""
        if seen and died and bad() and not any(s for *_, s in seen):
            ok("crash-import", f"{tag} imported ({len(seen)} progress samples, last {seen[-1][1]} of {seen[-1][2]}), died at {died}; the guard failed it on probation and marked it bad")
        else: no("crash-import", f"seen={seen[-2:]} died={died!r} bad={bad()}")
        back = until(240, lambda: gstat().get("build") == "v0.14-legacy" and gstat().get("stable") is True)
        said = b""
        try: said = hello()
        except Exception: pass
        if back and b"0.14.1" in said: ok("back-to-014", f"0.14 runs again on {port} under the guard: {gstat().get('text')}")
        else: no("back-to-014", f"build={gstat().get('build')} state={gstat().get('state')} hello={said[:80]!r}")
        if before is None or at_bad is None: no("old-home", "0.14's home was not seen at 0.15's start and at its failure")
        elif before != at_bad:
            # the guard starts 0.14 again within moments of the crash, and 0.14 writes its own home as it starts (hub.lock, its store): a change made after the
            # crash (the build's marker) is that; one made before it, while 0.15 imported, is 0.15 writing into 0.14's home
            changed = sorted(k for k in set(before) | set(at_bad) if before.get(k) != at_bad.get(k))
            died_ns = os.stat(os.path.join(h15, "dry-run-crashed")).st_mtime_ns if died else 0
            def mt(k):
                try: return os.stat(os.path.join(h14, k)).st_mtime_ns
                except OSError: return None
            early = [k for k in changed if mt(k) is None or mt(k) <= died_ns]
            if early: no("old-home", f"0.14's home changed while 0.15 imported: {early[:5]} (all changed: {changed[:5]})")
            elif marks(): no("old-home", f"0.15 wrote into 0.14's home: {marks()}")
            else: ok("old-home", f"0.14's home is byte-identical ({len(before)} files) from 0.15's start to its crash; after it, 0.14 starting again wrote its own {changed[:4]}")
        elif marks(): no("old-home", f"0.15 wrote into 0.14's home: {marks()}")
        else: ok("old-home", f"0.14's home is byte-identical ({len(before)} files) from 0.15's start to its failure, and has nothing of 0.15's")
        # 2. the next 0.15 (GOOD_DIR's, laid out by its own install-release.sh as 0.14's updater would) picks the import up and finishes it, nothing twice
        good = os.path.abspath(crash); gm = json.load(open(os.path.join(good, "manifest.json"))); gtag = gm["tag"]
        ga = os.path.join(good, f"backplane-{gtag}-{plat}{'-cef' if plat == 'linux-x64' else ''}.tar.gz"); gx = os.path.join(T, "good"); os.makedirs(gx, exist_ok=True)
        subprocess.run(["tar", "-xzf", ga, "-C", gx], check=True)
        r = subprocess.run(["sh", os.path.join(gx, f"backplane-{plat}", "install-release.sh"), os.path.join(gx, f"backplane-{plat}"), prefix], env=env, capture_output=True, text=True, timeout=600)
        r2 = subprocess.run([os.path.join(prefix, "backplane"), "hub", "update", gtag, "--home", h15, "--now"], env=env, capture_output=True, text=True, timeout=120) if r.returncode == 0 else r
        on = until(1800, lambda: gstat().get("build") == gtag and gstat().get("stable") is True)
        tok15 = open(os.path.join(h15, "token")).read().strip(); hub15 = Hub(base, tok15)
        # the guard proves a build only once its import has ended: by then the route (which only reads) says done, from the import this start resumed by itself (else it answers a scan of what is left, nothing written)
        st2, result = hub15.call("GET", "/import/0.14", hub15.bearer()) if on else (0, {})
        if not on or result.get("state") != "done": no("resume", f"layout {r.returncode} update {r2.returncode} {(r2.stdout + r2.stderr)[-160:]} build={gstat().get('build')} import={str(result)[:160]}")
        else:
            got = once(h14, h15); clean = expect_once or {}
            out = [(k, v["twice"], v["once"], clean.get(k, {}).get("once", v["once"])) for k, v in got.items()]   # without --expect-from: only that nothing came twice
            if all(t == 0 and there == want for _, t, there, want in out):
                ok("resume", f"{gtag} finished the import ({', '.join(f'{there} {k}' for k, _, there, _ in out)}, each once{", as a clean import of this home brings" if expect_once else ""}); elapsed {result.get('elapsed_s')} s")
            else: no("resume", f"(kind, twice, there once, a clean import) {out}")
            if device:
                w2 = Ws(hub15, device); hi = w2.hello() if w2.status == 101 else None
                ok("device", "the phone paired to 0.14 is accepted by 0.15 without pairing again") if hi else no("device", f"0.15 refused the 0.14 device token (ws {w2.status})")
    else:
        # the move to 0.15 imports 0.14's home by itself (the shim sets BACKPLANE_IMPORT_AUTO): watch it from outside, as the new app and phones would.
        # While it runs the hub answers /hello and /healthz with the progress, the guard keeps it on probation without failing it, and only then proves it.
        def health():
            c = http.client.HTTPConnection("127.0.0.1", port, timeout=5); c.request("GET", "/healthz"); r = c.getresponse(); return r.status, json.loads(r.read() or b"{}")
        seen, hello_ok, rolled, t0 = [], 0, False, time.time()
        deadline = time.time() + (3600 if large else 400)
        while time.time() < deadline:
            g = gstat()
            if os.path.exists(os.path.join(prefix, "BAD")) and tag in open(os.path.join(prefix, "BAD")).read(): rolled = True; break
            try:
                st, h = health(); m = h.get("migrating")
                if isinstance(m, dict) and m.get("state") == "running":
                    seen.append((round(time.time() - t0), m.get("done", 0), m.get("total", 0), g.get("build"), g.get("stable"), m.get("phase", "")))
                    try:
                        if b"migrating" in hello(): hello_ok += 1
                    except Exception: pass
            except Exception: pass
            if g.get("build") == tag and g.get("stable") is True and (not large or seen): break
            time.sleep(0.25 if seen else 1)   # an unthrottled import of 400k lines takes seconds: sample it often
        if large or throttle:
            if rolled: no("migrating", f"the guard rolled {tag} back while it imported: {open(os.path.join(prefix, 'BAD')).read().strip()[:160]}")
            elif not seen: no("migrating", "no import progress was ever reported on /healthz")
            else:
                # the import goes through steps, each with its own count (reading: bytes; writing: events; finishing: none): progress rises within each step,
                # and the writing step is the one the card's bar follows
                unproven = all(not x[4] for x in seen); span = seen[-1][0] - seen[0][0]
                steps = {}
                for x in seen: steps.setdefault(x[5], []).append(x[1])
                rising = all(v == sorted(v) for v in steps.values()) and (len(steps.get("writing", [])) <= 1 or steps["writing"][-1] > steps["writing"][0])
                w = [x for x in seen if x[5] == "writing"] or seen; dones = [x[1] for x in w]
                if rising and unproven and hello_ok: ok("migrating", (f"{len(seen)} progress samples over {span} s (steps {', '.join(steps)}; writing {dones[0]} -> {dones[-1]} of {w[-1][2]})" if len(seen) > 1 else f"the import ran within one sample ({dones[0]} of {seen[0][2]})") + f", /hello carried it {hello_ok} times, the build stayed on probation and was never failed")
                else: no("migrating", f"rising={rising} unproven-while-importing={unproven} hello={hello_ok} steps={ {k: (v[0], v[-1], len(v)) for k, v in steps.items()} }")
        on15 = until(400, lambda: gstat().get("build") == tag and gstat().get("stable") is True)
        if not on15: no("on-015", f"build={gstat().get('build')} state={gstat().get('state')} text={gstat().get('text')}"); raise SystemExit
        h14_at_proof = tree(h14)
        said = hello(); ver = manifest["version"]
        if ver.encode() in said and manifest["commit"][:7].encode() in said: ok("on-015", f"the guard runs {tag} on 0.14's port {port}, proven; /hello names {ver} and {manifest['commit'][:9]}")
        else: no("on-015", f"/hello: {said[:120]!r}")
        tok15 = open(os.path.join(h15, "token")).read().strip(); hub15 = Hub(base, tok15)
        st, summary = hub15.call("GET", "/import/0.14", hub15.bearer())
        st2, result = hub15.call("GET", "/import/0.14/run", hub15.bearer())
        # the automatic import's result (GET /import/0.14 answers it once done): a clean import's counts, what --crash-import --expect-from checks a resumed one against.
        # /run here is a second import of the same home: it must write nothing (every line already here)
        if st == 200:
            o = once(h14, h15); summary["once"] = o; json.dump(summary, open(os.path.join(T, "import-result.json"), "w"))
            if any(v["twice"] for v in o.values()): no("import-once", f"0.14 ids in 0.15's log more than once: {o}")
            else: ok("import-once", "every 0.14 thread and message in 0.15's log once: " + ", ".join(f"{v['once']} {k}" for k, v in o.items()))
        if st2 == 200 and summary.get("state") == "done":
            rc = result.get("counts", {}); again = {k: rc.get(k) for k in ("projects", "threads", "messages", "lines")}
            ok("import-again", f"a second import of the same home wrote nothing ({rc.get('already')} lines already here)") if not any(again.values()) else no("import-again", f"a second import wrote {again}")
        if st == 200 and st2 == 200: ok("import", f"found {summary.get('from')}: {json.dumps(result)[:160]}")
        else: no("import", f"/import/0.14 {st}, /run {st2}: {str(result)[:160]} (the 0.14-import lane's route)")
        if device:
            w2 = Ws(hub15, device); hi = w2.hello() if w2.status == 101 else None
            ok("device", "the phone paired to 0.14 is accepted by 0.15 without pairing again") if hi else no("device", f"0.15 refused the 0.14 device token (ws {w2.status})")
    m = [] if crash else marks()
    if crash: pass
    elif m: no("old-home", f"0.15 wrote into 0.14's home: {m}")
    elif not panics and tree(h14) != h14_at_proof:
        changed = sorted(k for k in set(tree(h14)) | set(h14_at_proof) if tree(h14).get(k) != h14_at_proof.get(k))
        no("old-home", f"0.14's home changed while 0.15 ran (import, device): {changed[:5]}")
    else: ok("old-home", "0.14's home has nothing of 0.15's" + ("" if panics else f", and is byte-identical ({len(h14_at_proof)} files) from 0.15's start through the import"))
finally:
    stop()
    print(f"from-014{' --panics' if panics else ' --crash-import' if crash else ''}: {len(PASS)} passed, {len(FAIL)} failed{': ' + ' '.join(FAIL) if FAIL else ''}  (kept: {T})")
    sys.exit(1 if FAIL else 0)
