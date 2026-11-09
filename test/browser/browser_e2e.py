#!/usr/bin/env python3
"""The Browser tab end to end on one machine (Linux, macOS, Windows): a hub on a temp home, the GPUI client on the desktop, a local http page.

  python3 test/browser/browser_e2e.py --client BIN --hub BIN --helper BIN [--mode frames|cef|any] [--out DIR] [--port N]

The client runs a script of `{"browser":{"op":...}}` steps (native/gpui/src/viewers/browser.rs BrowserPage::script: the toolbar's, the page's mouse,
wheel and keys' own calls), each followed by a report. The checks read those reports from the client's log:
a local http page loads, typing reaches its input, scrolling reaches it, a click on its link navigates, back and forward, a file:// page, an https
page, a second tab opened and closed, the Browser view closed and reopened, the client exits 0 by itself, and after the client and the hub are gone
no process of this run (anything whose command line names the run's folder, the helper or the client) is left.
--mode says which page must have been shown: frames (the hub's JPEG frames, CEF turned off), cef (embedded Chromium; a fallback to frames fails),
fallback (a cef client whose runtime is missing: it must start, say why in the tab and show the hub's frames), any.
Exit 0 when every check passes. Artifacts (logs, screenshots where the OS can take one) in --out.
"""
import argparse, http.server, json, os, shutil, subprocess, sys, tempfile, threading, time, urllib.request
from pathlib import Path

HERE = Path(__file__).resolve().parent
WIN = sys.platform == "win32"

def free_port():
    import socket; s = socket.socket(); s.bind(("127.0.0.1", 0)); p = s.getsockname()[1]; s.close(); return p

def procs():
    """(pid, command line) of every process on the machine."""
    if WIN:
        out = subprocess.run(["powershell", "-NoProfile", "-Command", "Get-CimInstance Win32_Process | ForEach-Object { \"$($_.ProcessId)`t$($_.ExecutablePath) $($_.CommandLine)\" }"], capture_output=True, text=True).stdout
        rows = [l.split("\t", 1) for l in out.splitlines() if "\t" in l]
    else:
        out = subprocess.run(["ps", "-axww", "-o", "pid=,command="], capture_output=True, text=True).stdout
        rows = [l.strip().split(None, 1) for l in out.splitlines() if l.strip()]
    return [(int(r[0]), r[1] if len(r) > 1 else "") for r in rows if r and r[0].strip().isdigit()]

def mine(needles, skip):
    me = os.getpid()
    norm = lambda s: s.replace("\\", "/").lower()
    ns = [norm(n) for n in needles if n]
    return [(p, c) for p, c in procs() if p != me and p not in skip and any(n in norm(c) for n in ns) and "browser_e2e.py" not in c]

def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--client", required=True); ap.add_argument("--hub", required=True); ap.add_argument("--helper", required=True)
    ap.add_argument("--mode", default="any", choices=["frames", "cef", "fallback", "any"]); ap.add_argument("--out", default="")
    ap.add_argument("--port", type=int, default=0); ap.add_argument("--https", default="https://example.com/")
    ap.add_argument("--exit-after", type=int, default=150)
    a = ap.parse_args()
    a.client, a.hub, a.helper = (os.path.abspath(x) for x in (a.client, a.hub, a.helper))
    out = Path(a.out or tempfile.mkdtemp(prefix="bp-browser-e2e-")).resolve(); shutil.rmtree(out, ignore_errors=True); out.mkdir(parents=True)
    home, proj, agents, cfg = out / "home", out / "proj", out / "agents", out / "config"
    for d in (home, proj, agents, cfg): d.mkdir()
    now = int(time.time())
    ev = [{"$": "ProjectCreated", "id": "p1", "title": "browser", "root": str(proj), "at": now},
          {"$": "ThreadCreated", "id": "t1", "project": "p1", "title": "browser", "env": "local", "branch": "", "worktree": "", "provider": "claude", "model": "", "at": now}]
    (home / "events.jsonl").write_text("".join(json.dumps(e, separators=(",", ":")) + "\n" for e in ev))
    # the page, served from this folder on loopback
    class Quiet(http.server.SimpleHTTPRequestHandler):
        def __init__(s, *x, **k): super().__init__(*x, directory=str(HERE / "page"), **k)
        def log_message(s, *x): pass
    srv = http.server.ThreadingHTTPServer(("127.0.0.1", 0), Quiet); pport = srv.server_address[1]
    threading.Thread(target=srv.serve_forever, daemon=True).start()
    page = f"http://127.0.0.1:{pport}/index.html"
    fileurl = (HERE / "page" / "page2.html").as_uri()
    hport = a.port or free_port()
    before = {p for p, _ in procs()}
    env = dict(os.environ, BACKPLANE_BROWSER=os.path.abspath(a.helper), BACKPLANE_AGENTS=str(agents), BACKPLANE_NO_UPDATE="1", BACKPLANE_BROWSER_DEBUG="1",
               BACKPLANE_CEF_PROFILE=str(out / "cef-profile"), XDG_CONFIG_HOME=str(cfg))
    if a.mode == "frames": env["BACKPLANE_CEF_OFF"] = "1"
    hlog = open(out / "hub.log", "wb")
    hub = subprocess.Popen([a.hub, "--home", str(home), "--port", str(hport), "--host", "127.0.0.1", "--no-tailscale", "--foreground"], cwd=proj, env=env, stdout=hlog, stderr=subprocess.STDOUT)
    for _ in range(240):
        try: urllib.request.urlopen(f"http://127.0.0.1:{hport}/hello", timeout=1).read(); break
        except Exception: pass
        if hub.poll() is not None: print("FAIL the hub exited", hub.returncode); print((out / "hub.log").read_text(errors="replace")[-3000:]); return 1
        time.sleep(0.5)
    else: print("FAIL the hub did not answer /hello"); return 1
    B = lambda op, **k: {"browser": dict(op=op, **k)}
    R = lambda: B("report")
    def act(name, step, wait=4000, reports=3):
        s = [{"mark": name}, step]
        for _ in range(reports): s += [{"wait": wait // reports}, R()]
        return s
    steps = [{"select": 0}, {"wait": 2500}, {"mark": "open"}, {"click": "rail:Browser"}]
    for _ in range(10): steps += [{"wait": 1500}, R()]
    steps += act("goto_http", B("goto", url=page), 6000, 4)
    steps += act("click_input", B("click", x=500, y=150), 1500, 1)
    steps += act("type", B("type", text="hi"), 3000)
    steps += act("scroll", B("scroll", dy=600), 3000)
    steps += act("click_link", B("click", x=500, y=500), 5000)
    steps += act("back", B("back"), 5000)
    steps += act("forward", B("forward"), 5000)
    steps += act("goto_file", B("goto", url=fileurl), 5000)
    steps += act("goto_https", B("goto", url=a.https), 12000, 6)
    steps += act("new_tab", B("new_tab"), 4000)
    steps += act("close_tab", B("close_tab", index=1), 4000)
    steps += [{"mark": "close_view"}, {"click": "rail:Browser"}, {"wait": 2000}, R()]
    steps += act("reopen_view", {"click": "rail:Browser"}, 5000)
    steps += act("goto_again", B("goto", url=page), 5000)
    steps += [{"mark": "end"}]
    (out / "script.json").write_text(json.dumps(steps, indent=0))
    token = home / "token"
    t0 = time.time()
    with open(out / "client.log", "wb") as clog:
        try:
            cenv = dict(env); cenv.update(json.loads(os.environ.get("BP_CLIENT_ENV", "{}")))
            rc = subprocess.run([a.client, "--url", f"ws://127.0.0.1:{hport}", "--token-file", str(token), "--script", str(out / "script.json"), "--exit-after", str(a.exit_after)],
                                env=cenv, cwd=out, stdout=clog, stderr=subprocess.STDOUT, timeout=a.exit_after + 120).returncode
        except subprocess.TimeoutExpired: rc = "timeout"
    print(f"client exited {rc} after {time.time() - t0:.0f} s")
    # the hub stops (as the guard stops it: its process is ended), then nothing of this run may be left
    if WIN: hub.terminate()
    else: hub.send_signal(15)
    try: hub.wait(30)
    except subprocess.TimeoutExpired: hub.kill(); hub.wait()
    srv.shutdown()
    needles = [str(out), os.path.abspath(a.helper), os.path.abspath(a.client)]
    left = []
    for _ in range(20):
        left = mine(needles, before)
        if not left: break
        time.sleep(1)
    # the checks
    log = (out / "client.log").read_text(errors="replace")
    ev = []; mark = ""
    for line in log.splitlines():
        if "GPUI_METRIC " not in line: continue
        try: m = json.loads(line.split("GPUI_METRIC ", 1)[1])
        except Exception: continue
        if m.get("event") == "mark": mark = m.get("name", "")
        elif m.get("event") == "browser": m["_mark"] = mark; ev.append(m)
    (out / "reports.jsonl").write_text("".join(json.dumps(e) + "\n" for e in ev))
    def at(name): return [e for e in ev if e["_mark"] == name]
    def any_(name, f): return any(f(e) for e in at(name))
    last = lambda name: (at(name) or [{}])[-1]
    modes = sorted({e.get("mode", "") for e in ev})
    checks = [
        ("the client exited 0 by itself (it survived the run)", rc == 0, f"rc={rc}"),
        ("the Browser tab opened a page", any_("open", lambda e: e.get("ok") and (e.get("tabs", 0) >= 1)), json.dumps(last("open"))[:300]),
        (f"the page is the {a.mode} page" if a.mode != "any" else "a page mode was reported", (a.mode == "any" and bool(modes)) or modes == [{"fallback": "frames"}.get(a.mode, a.mode)], f"modes={modes} banner={last('open').get('banner', '')!r}"),
        *([("the tab says why the embedded browser is not used", any_("open", lambda e: "embedded browser unavailable" in str(e.get("banner", ""))), f"banner={last('open').get('banner', '')!r}")] if a.mode == "fallback" else []),
        ("a local http page loaded (title bp-index)", any_("goto_http", lambda e: e.get("title") == "bp-index" and e.get("frame", [0])[0] > 0), json.dumps(last("goto_http"))[:300]),
        ("the page's frame has content", any_("goto_http", lambda e: e.get("non_white_samples", 0) > 0), json.dumps(last("goto_http"))[:200]),
        ("typing reached the page's input (title typed:hi)", any_("type", lambda e: e.get("title") == "typed:hi"), json.dumps(last("type"))[:300]),
        ("scrolling reached the page (title scrolled:)", any_("scroll", lambda e: str(e.get("title", "")).startswith("scrolled:")), json.dumps(last("scroll"))[:300]),
        ("a click on the page's link navigated (bp-page2)", any_("click_link", lambda e: e.get("title") == "bp-page2"), json.dumps(last("click_link"))[:300]),
        ("back returned to the first page", any_("back", lambda e: str(e.get("url", "")).endswith("/index.html")), json.dumps(last("back"))[:300]),
        ("forward went to the second page again", any_("forward", lambda e: str(e.get("url", "")).endswith("/page2.html")), json.dumps(last("forward"))[:300]),
        # embedded Chromium is the person's own browser: file:// opens. The hub's page is on the hub's machine, and its toolbar opens http(s) only
        # (native/host/src/term/driver.rs cef_url): a file:// address there is refused and the page stays where it was
        (("a file:// page loaded", any_("goto_file", lambda e: e.get("title") == "bp-page2" and str(e.get("url", "")).startswith("file:")), json.dumps(last("goto_file"))[:300]) if a.mode == "cef" else
         ("a file:// address is refused by the hub's page (it stays on page two)", all(not str(e.get("url", "")).startswith("file:") for e in at("goto_file")) and any_("goto_file", lambda e: e.get("title") == "bp-page2"), json.dumps(last("goto_file"))[:300])),
        (f"an https page loaded ({a.https})", any_("goto_https", lambda e: str(e.get("url", "")).startswith("https://") and e.get("title") not in ("", None, "bp-page2") and not e.get("load_error")), json.dumps(last("goto_https"))[:300]),
        ("a second tab opened", any_("new_tab", lambda e: e.get("tabs", 0) >= 2), json.dumps(last("new_tab"))[:200]),
        ("closing it left one tab", any_("close_tab", lambda e: e.get("tabs", 0) == 1), json.dumps(last("close_tab"))[:200]),
        ("after the Browser view was closed and reopened the page is still there", any_("reopen_view", lambda e: e.get("ok") and str(e.get("url", "")).startswith("https://")), json.dumps(last("reopen_view"))[:300]),
        ("and it still navigates", any_("goto_again", lambda e: e.get("title") == "bp-index"), json.dumps(last("goto_again"))[:300]),
        ("no process of this run is left after quit", not left, "; ".join(f"{p}: {c[:160]}" for p, c in left[:8])),
    ]
    fails = 0
    for name, ok, why in checks:
        print(("PASS " if ok else "FAIL ") + name + ("" if ok else f"  [{why}]"))
        fails += 0 if ok else 1
    print(f"BROWSER_E2E mode={','.join(modes)} passed={len(checks) - fails} failed={fails} out={out}")
    if fails:
        for l in log.splitlines():
            if '"cef' in l or "browser" in l.lower() and "GPUI_LOG" in l: print("  log:", l[:300])
        print("--- hub log tail"); print((out / "hub.log").read_text(errors="replace")[-2500:])
    return 1 if fails else 0

if __name__ == "__main__":
    sys.exit(main())
