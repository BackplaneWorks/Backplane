"""bp.13 release gate on Windows: the published-style KiCad zip installed through the hub's own driver (kicad.install), into a
scratch LOCALAPPDATA, as an upgrade from bp.10.

  python win_install_check.py HUB_OLD HUB_NEW SERVE_DIR OUT [PORT]

HUB_OLD: a hub whose Windows pin is bp.10 (main): it installs bp.10 from the real GitHub release first. HUB_NEW: a TEST-ONLY hub whose pin
is the candidate (version, asset, SHA256) and whose download base is BACKPLANE_TEST_ONLY_KICAD_BASE (a loopback http server on SERVE_DIR,
which holds the zip and its SHA256SUMS as the release would). Everything else is the driver as shipped: the size probe, the download,
both checksums, the unzip, the manifest check and the move into place. No KiCad under C:\\bp is used (BACKPLANE_KICAD_NO_SYSTEM=1).
"""
import json, os, subprocess, sys, time, threading, http.server, functools, shutil, urllib.request

HERE = os.path.dirname(os.path.abspath(__file__))
RPC = os.path.join(HERE, "lib", "hub_rpc.mjs")
BUN = shutil.which("bun") or os.path.expandvars(r"%USERPROFILE%\.bun\bin\bun.exe")
FAILS = []
PASSES = []


def check(name, ok, ev=None):
    print(("PASS " if ok else "FAIL ") + name + ("" if ok else f" ({ev})"), flush=True)
    (PASSES if ok else FAILS).append(name)


def serve(d, port):
    h = functools.partial(http.server.SimpleHTTPRequestHandler, directory=d)
    srv = http.server.ThreadingHTTPServer(("127.0.0.1", port), h)
    threading.Thread(target=srv.serve_forever, daemon=True).start()
    return srv


def env_for(out, extra=None):
    e = {k: v for k, v in os.environ.items() if not k.startswith("BACKPLANE_")}
    user = os.path.join(out, "user")
    e.update({"USERPROFILE": user, "HOME": user, "LOCALAPPDATA": os.path.join(user, "AppData", "Local"), "BACKPLANE_KICAD_NO_SYSTEM": "1"})
    os.makedirs(e["LOCALAPPDATA"], exist_ok=True)
    e.update(extra or {})
    return e


def start_hub(exe, out, port, env, log):
    lf = open(os.path.join(out, log), "ab")
    p = subprocess.Popen([exe, "--home", os.path.join(out, "home"), "--port", str(port), "--host", "127.0.0.1", "--no-tailscale", "--foreground"],
                         env=env, cwd=out, stdout=lf, stderr=subprocess.STDOUT, creationflags=subprocess.CREATE_NO_WINDOW)
    for _ in range(150):
        try:
            with urllib.request.urlopen(f"http://127.0.0.1:{port}/hello", timeout=1.2) as r:
                if r.status == 200:
                    return p
        except Exception:
            pass
        time.sleep(0.2)
    raise RuntimeError("hub did not start")


def rpc(out, port, steps):
    env = dict(os.environ, HUB_RPC_INFO_PREFIXES="kicad.")
    cp = subprocess.run([BUN, RPC, str(port), os.path.join(out, "home", "token"), json.dumps(steps)], capture_output=True, text=True, timeout=120, env=env)
    replies, infos = [], {}
    for l in cp.stdout.splitlines():
        if l.startswith("INFO "):
            j = json.loads(l[5:]); infos[j["key"]] = j["raw"]
        elif l.startswith("{"):
            replies.append(json.loads(l))
    return replies, infos


def install_and_wait(out, port, version, minutes=25):
    rep, _ = rpc(out, port, [{"m": "kicad.install", "p": {"consent": True}, "timeout_ms": 20000}])
    print("DIAG install_reply " + json.dumps(rep)[:600], flush=True)
    parent = os.path.join(out, "user", "AppData", "Local", "Backplane", "kicad")
    end = time.time() + minutes * 60
    state, note = "", ""
    while time.time() < end:
        _, infos = rpc(out, port, [{"m": "kicad.info", "p": {}}])
        try:
            v = json.loads(infos.get("kicad.install") or "{}")
        except ValueError:
            v = {}
        state, note = v.get("state", ""), v.get("note", "")
        if state in ("done", "failed"):
            break
        time.sleep(10)
    _, infos = rpc(out, port, [{"m": "kicad.info", "p": {}}])
    return rep, state, note, infos, parent


def main():
    hub_old, hub_new, serve_dir, out = sys.argv[1:5]
    port = int(sys.argv[5]) if len(sys.argv) > 5 else 3871
    os.makedirs(out, exist_ok=True)
    sport = port + 7
    srv = serve(serve_dir, sport)
    try:
        # 1. bp.10 as a person has it today: main's hub installs its pin from the GitHub release
        h = start_hub(hub_old, out, port, env_for(out), "hub-old.log")
        rep, state, note, infos, parent = install_and_wait(out, port, "10.99.0-bp.10")
        check("bp.10 installed by main's hub into the scratch LOCALAPPDATA", state == "done" and os.path.isdir(os.path.join(parent, "10.99.0-bp.10")),
              {"state": state, "note": note, "status": infos.get("kicad.status"), "version": infos.get("kicad.version")})
        check("main's hub then says KiCad ok, bp.10", infos.get("kicad.status") == "ok" and "bp.10" in (infos.get("kicad.version") or ""),
              {"status": infos.get("kicad.status"), "version": infos.get("kicad.version")})
        h.terminate(); h.wait(20)
        # 2. the candidate's hub on the same home: bp.10 is older than its pin, so Install upgrades
        h = start_hub(hub_new, out, port, env_for(out, {"BACKPLANE_TEST_ONLY_KICAD_BASE": f"http://127.0.0.1:{sport}/"}), "hub-new.log")
        _, infos0 = rpc(out, port, [{"m": "kicad.info", "p": {}}])
        print("DIAG before_upgrade " + json.dumps({k: infos0.get(k) for k in ("kicad.status", "kicad.version", "kicad.install")}), flush=True)
        rep, state, note, infos, parent = install_and_wait(out, port, "10.99.0-bp.13")
        refused = any(r.get("ok") is False for r in rep)
        check("the candidate's hub accepts Install over bp.10 (an upgrade, not 'already installed')", not refused, rep)
        check("the candidate zip installs through the driver (download, SHA256SUMS and pinned digest, unzip, manifest, move)",
              state == "done" and os.path.isfile(os.path.join(parent, "10.99.0-bp.13", "bin", "kicad-cli.exe")), {"state": state, "note": note})
        check("after the upgrade the hub runs the candidate: status ok, version bp.13", infos.get("kicad.status") == "ok" and "bp.13" in (infos.get("kicad.version") or ""),
              {"status": infos.get("kicad.status"), "version": infos.get("kicad.version"), "cli": infos.get("kicad.cli")})
        check("bp.10's folder is left as it was (the install never deletes)", os.path.isdir(os.path.join(parent, "10.99.0-bp.10")), parent)
        try:
            src = open(os.path.join(parent, "10.99.0-bp.13", "SOURCE_COMMIT")).read().strip()
        except OSError as e:
            src = str(e)
        print("DIAG installed_source_commit " + src, flush=True)
        h.terminate(); h.wait(20)
    finally:
        srv.shutdown()
    print(f"win_install_check: passed={len(PASSES)} failed={len(FAILS)}", flush=True)
    return 1 if FAILS else 0


if __name__ == "__main__":
    sys.exit(main())
