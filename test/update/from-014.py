#!/usr/bin/env python3
"""The way from 0.14, end to end, on this machine (docs/deploy.md, "From 0.14"): a real 0.14.1 install updates itself to a 0.15 release dry run.

    from-014.py V014_DIR RELEASE_DIR [--panics]

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
PASS, FAIL = [], []
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
    log014 = open(os.path.join(T, "serve-014.log"), "ab")
    p = subprocess.Popen([serve, "--home", h14, "--port", str(port), "--no-tailscale", "--foreground"], env=env, stdout=log014, stderr=log014, stdin=subprocess.DEVNULL, start_new_session=True)
    procs.append(p)
    base = f"http://127.0.0.1:{port}"
    tok = until(90, lambda: open(os.path.join(h14, "token")).read().strip() if Hub(base, "x").call("GET", "/hello")[0] == 200 else None)
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
    body = cbor({"id": 1, "m": "app.update", "p": {}}); mask = os.urandom(4)
    w.s.sendall(bytes([0x82, 0x80 | 126]) + struct.pack(">H", len(body)) + mask + bytes(b ^ mask[i % 4] for i, b in enumerate(body)) if len(body) >= 126 else
                bytes([0x82, 0x80 | len(body)]) + mask + bytes(b ^ mask[i % 4] for i, b in enumerate(body)))
    shimmed = until(600, lambda: "BACKPLANE-0.15-SHIM" in open(serve, errors="replace").read())
    if not shimmed: no("update-014", "0.14 never laid 0.15 out: " + open(os.path.join(T, "serve-014.log"), errors="replace").read()[-400:]); raise SystemExit
    ok("update-014", f"0.14's updater took {tag} from SHA256SUMS and ran its install-release.sh; 0.14 kept as backplane-serve.0.14={os.path.exists(serve + '.0.14')}")
    h14_before = {f: hashlib.sha256(open(os.path.join(h14, f), "rb").read()).hexdigest() for f in ("events.jsonl", "token", "devices-v1.json") if os.path.exists(os.path.join(h14, f))}

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
    else:
        on15 = until(400, lambda: gstat().get("build") == tag and gstat().get("stable") is True)
        if not on15: no("on-015", f"build={gstat().get('build')} state={gstat().get('state')} text={gstat().get('text')}"); raise SystemExit
        said = hello(); ver = manifest["version"]
        if ver.encode() in said and manifest["commit"][:7].encode() in said: ok("on-015", f"the guard runs {tag} on 0.14's port {port}, proven; /hello names {ver} and {manifest['commit'][:9]}")
        else: no("on-015", f"/hello: {said[:120]!r}")
        tok15 = open(os.path.join(h15, "token")).read().strip(); hub15 = Hub(base, tok15)
        st, summary = hub15.call("GET", "/import/0.14", hub15.bearer())
        st2, result = hub15.call("GET", "/import/0.14/run", hub15.bearer())
        if st == 200 and st2 == 200: ok("import", f"found {summary.get('from')}: {json.dumps(result)[:160]}")
        else: no("import", f"/import/0.14 {st}, /run {st2}: {str(result)[:160]} (the 0.14-import lane's route)")
        if device:
            w2 = Ws(hub15, device); hi = w2.hello() if w2.status == 101 else None
            ok("device", "the phone paired to 0.14 is accepted by 0.15 without pairing again") if hi else no("device", f"0.15 refused the 0.14 device token (ws {w2.status})")
    after = {f: hashlib.sha256(open(os.path.join(h14, f), "rb").read()).hexdigest() for f in h14_before}
    ok("old-home", "0.14's home unchanged by 0.15") if after == h14_before or panics else no("old-home", "0.14's home changed")
finally:
    stop()
    print(f"from-014{' --panics' if panics else ''}: {len(PASS)} passed, {len(FAIL)} failed{': ' + ' '.join(FAIL) if FAIL else ''}  (kept: {T})")
    sys.exit(1 if FAIL else 0)
