#!/usr/bin/env python3
"""Buy it end to end in the GPUI window, the way a person walks it, against the order server on loopback (order/fabsrv through
test/order/local_server.py: its stand-in fab pack and a Stripe double) and a scripted stand-in agent (claude_standin.ts, mode buyflow).
Never the real order server, never a real payment, never a real model.

    buy_e2e.py --client BACKPLANE_GPUI --hub BACKPLANE_HUB --python PY [--project KICAD_DIR] [--out DIR] [--display :N] [--port N]

PY has order/requirements.txt installed. --project is copied (its KiCad files only; default: a fixture board of this repo). kicad-cli
exports the gerbers as a buy thread's agent does. Real X input through XTest (test/cef/xinput.py); screenshots in OUT.

The walk: the design thread's header has "Buy it · from $X" -> Buy it opens the buy thread -> the review gate asks; Review for
manufacturing runs the review as a task, which records it, and the gate passes -> "Ship to which country?" typed in its box -> the
US state, typed -> the export -> the files' approval shows "Ships to US (GA)" -> Send -> the Order card waits for the quote -> the
quote is posted with Manufacturing, Shipping, Duties & taxes and Sales tax (estimated) and the card has its total, customs line and
Buy now -> Buy now opens the order server's page, which pays the double -> Paid -> the operator marks it placed, then shipped,
and a restarted hub says so in the thread at once. With FAB_PAUSED, --paused checks Buy now opens the paused page instead.

Env: XV (an Xvfb tree with usr/bin/Xvfb), TWM (a twm binary; else i3 if on PATH, else no window manager), BUN, VK_ICD_FILENAMES,
BACKPLANE_GPUI_ASSETS. Run it with no GPU devices visible (software Vulkan): it stops if the window renders on a real GPU.
Prints PASS/FAIL lines; exit 1 on any FAIL."""
import argparse, glob, json, os, pathlib, re, shutil, signal, socket, subprocess, sys, time, urllib.parse, urllib.request

ROOT = pathlib.Path(__file__).resolve().parents[2]
a = argparse.ArgumentParser()
a.add_argument("--client", required=True); a.add_argument("--hub", required=True); a.add_argument("--python", required=True)
a.add_argument("--project", default=""); a.add_argument("--out", default=os.path.expanduser("~/.cache/bp-tmp/buy-e2e")); a.add_argument("--display", default=":183")
a.add_argument("--port", type=int, default=0); a.add_argument("--paused", action="store_true"); a.add_argument("--keep", action="store_true", help="leave everything up until OUT/stop exists")
o = a.parse_args()
OUT = o.out; DISP = o.display
XIN = [sys.executable, "-I", str(ROOT / "test/cef/xinput.py")]
procs, fails, passes = [], [], []


def check(ok, what, got=""):
    print(("PASS " if ok else "FAIL ") + what + ("" if ok else f" ({str(got)[:300]})"), flush=True)
    (passes if ok else fails).append(what)


def spawn(args, log, **kw):
    p = subprocess.Popen(args, stdout=open(f"{OUT}/{log}", "ab"), stderr=subprocess.STDOUT, **kw); procs.append(p); return p


def stop(p):
    if p.poll() is None:
        p.terminate()
        try: p.wait(8)
        except subprocess.TimeoutExpired: p.kill()


signal.signal(signal.SIGTERM, lambda *_: sys.exit(1))


def free_port():
    if o.port:
        free_port.n = getattr(free_port, "n", -1) + 1; return o.port + free_port.n
    while True:
        s = socket.socket(); s.bind(("127.0.0.1", 0)); p = s.getsockname()[1]; s.close()
        if p >= 3799: return p


def X(*args): subprocess.run(XIN + [str(x) for x in args], check=True, env={**os.environ, "DISPLAY": DISP})


def shot(name):
    subprocess.run(["import", "-display", DISP, "-window", "root", f"{OUT}/{name}.png"], stderr=subprocess.DEVNULL); print("shot", f"{OUT}/{name}.png", flush=True)


def metrics(name):
    out = []
    for line in open(f"{OUT}/client.log", errors="replace"):
        if line.startswith("GPUI_METRIC "):
            try: m = json.loads(line[12:])
            except ValueError: continue
            if m.get("event") == name: out.append(m)
    return out


def controls():
    c = metrics("controls")
    return {x["id"]: x for x in c[-1]["controls"]} if c else {}


def wait(what, f, secs=30):
    t = time.time()
    while time.time() - t < secs:
        v = f()
        if v: return v
        X("move", 3 + int(time.time() * 10) % 5, 3); time.sleep(0.3)  # a little motion: the window redraws and reports its controls
    shot("timeout-" + re.sub(r"\W+", "-", what)[:40]); raise SystemExit(f"FAIL timed out: {what}")


def rect_of(id):
    r = controls().get(id, {}).get("rect"); return r if r and r["w"] > 0 else None


OFF = [0, 0]  # the window's top left on the screen (a window manager may place it anywhere): its rects are the window's own


def window_at():
    """Where the biggest mapped window's top left is on the screen, through Xlib (no xwininfo here)."""
    import ctypes, ctypes.util
    x = ctypes.CDLL(ctypes.util.find_library("X11")); x.XOpenDisplay.restype = ctypes.c_void_p; x.XOpenDisplay.argtypes = [ctypes.c_char_p]
    d = x.XOpenDisplay(DISP.encode()); x.XDefaultRootWindow.restype = ctypes.c_ulong; x.XDefaultRootWindow.argtypes = [ctypes.c_void_p]
    x.XQueryTree.argtypes = [ctypes.c_void_p, ctypes.c_ulong, ctypes.POINTER(ctypes.c_ulong), ctypes.POINTER(ctypes.c_ulong), ctypes.POINTER(ctypes.POINTER(ctypes.c_ulong)), ctypes.POINTER(ctypes.c_uint)]
    x.XGetGeometry.argtypes = [ctypes.c_void_p, ctypes.c_ulong, ctypes.POINTER(ctypes.c_ulong)] + [ctypes.POINTER(ctypes.c_int)] * 2 + [ctypes.POINTER(ctypes.c_uint)] * 4
    x.XTranslateCoordinates.argtypes = [ctypes.c_void_p, ctypes.c_ulong, ctypes.c_ulong, ctypes.c_int, ctypes.c_int, ctypes.POINTER(ctypes.c_int), ctypes.POINTER(ctypes.c_int), ctypes.POINTER(ctypes.c_ulong)]
    root = x.XDefaultRootWindow(d); best = (0, 0, 0)
    def walk(w, depth):
        nonlocal best
        r, p, kids, n = ctypes.c_ulong(), ctypes.c_ulong(), ctypes.POINTER(ctypes.c_ulong)(), ctypes.c_uint()
        if not x.XQueryTree(d, w, ctypes.byref(r), ctypes.byref(p), ctypes.byref(kids), ctypes.byref(n)): return
        for i in range(n.value):
            k = kids[i]; rr, gx, gy, gw, gh, bw, dp = ctypes.c_ulong(), ctypes.c_int(), ctypes.c_int(), ctypes.c_uint(), ctypes.c_uint(), ctypes.c_uint(), ctypes.c_uint()
            x.XGetGeometry(d, k, ctypes.byref(rr), ctypes.byref(gx), ctypes.byref(gy), ctypes.byref(gw), ctypes.byref(gh), ctypes.byref(bw), ctypes.byref(dp))
            if gw.value * gh.value > best[0] and gw.value < 4000:
                tx, ty, ch = ctypes.c_int(), ctypes.c_int(), ctypes.c_ulong(); x.XTranslateCoordinates(d, k, root, 0, 0, ctypes.byref(tx), ctypes.byref(ty), ctypes.byref(ch))
                if depth > 0 or gw.value < 1600: best = (gw.value * gh.value, tx.value, ty.value)
            if depth < 3: walk(k, depth + 1)
    walk(root, 0); return [best[1], best[2]]


def press_rect(r): X("click", OFF[0] + int(r["x"] + r["w"] / 2), OFF[1] + int(r["y"] + r["h"] / 2)); time.sleep(0.5)


def labelled(prefix, label):
    """The rect of the first drawn control whose id starts with prefix and whose label contains label."""
    for k, c in controls().items():
        if k.startswith(prefix) and label in (c.get("label") or "") and c.get("rect", {}).get("w", 0) > 0: return c["rect"]
    return None


def events():
    out = []
    for l in open(OUT + "/home/events.jsonl", errors="replace"):
        try: out.append(json.loads(l))
        except ValueError: pass
    return out


def setting(key):
    v = None
    for e in events():
        if e.get("$") == "SettingSet" and e.get("key") == key: v = e.get("value")
    return v


def asks_open(thread):
    """The thread's open asks, from the log: opened and not yet answered or retired."""
    opened = {}
    for e in events():
        if e.get("$") == "AskOpened" and e.get("thread") == thread: opened[e["id"]] = e
        elif e.get("$") in ("AskAnswered", "AskClosed", "AskRetired", "AskResolved") and e.get("id") in opened: opened.pop(e["id"])
    return list(opened.values())


def posted(thread, pred):
    return next((e for e in events() if e.get("$") == "MessagePosted" and e.get("thread") == thread and pred(e)), None)


def get(url, headers=None):
    try:
        r = urllib.request.urlopen(urllib.request.Request(url, headers=headers or {}), timeout=5); return r.status, r.read().decode(errors="replace")
    except urllib.error.HTTPError as e: return e.code, e.read().decode(errors="replace")
    except Exception: return 0, ""


def type_into(box, text):
    press_rect(box); X("type", text); time.sleep(0.3)


def answer_typed(what, text, secs=40):
    """A "Backplane needs" card with a typed box: click the box, type, press Continue."""
    box = wait(f"the typed box for {what}", lambda: rect_of("ask:0:box"), secs); time.sleep(0.8)
    type_into(box, text); go = rect_of("ask:0:go"); press_rect(go)


def hub_env(fport):
    return {**env, "BACKPLANE_AGENTS": OUT + "/bin", "BACKPLANE_NO_UPDATE": "1", "BACKPLANE_KISTACK_SYNC": "0", "BACKPLANE_ORDER_SERVER": f"http://127.0.0.1:{fport}",
            "FAKE_LOG": OUT + "/standin.log", "BP_STANDIN_MODE": "buyflow", "BP_BUY_SCRIPT": json.dumps(SCRIPT), "BP_WORK_MS": "300"}


def start_hub(fport, port):
    if os.path.exists(OUT + "/home/token"): os.remove(OUT + "/home/token")
    h = spawn([o.hub, "--home", OUT + "/home", "--port", str(port), "--host", "127.0.0.1", "--no-tailscale", "--foreground"], "hub.log", cwd=proj, env=hub_env(fport))
    t = time.time()
    while not os.path.exists(OUT + "/home/token"):
        if time.time() - t > 30 or h.poll() is not None: raise SystemExit("FAIL the hub did not start: " + open(OUT + "/hub.log", errors="replace").read()[-800:])
        time.sleep(0.2)
    return h


env = {k: v for k, v in os.environ.items() if k not in ("WAYLAND_DISPLAY", "XDG_SESSION_TYPE", "DBUS_SESSION_BUS_ADDRESS", "CLAUDE_CONFIG_DIR", "I3SOCK", "ANTHROPIC_API_KEY")}
try:
    if os.path.exists(f"/tmp/.X11-unix/X{DISP[1:]}"): raise SystemExit(f"FAIL display {DISP} in use")
    shutil.rmtree(OUT, ignore_errors=True)
    for d in ("home", "bin", "fakehome/.cache", "fab"): os.makedirs(f"{OUT}/{d}")
    os.makedirs(OUT + "/run", mode=0o700)
    env.update(HOME=OUT + "/fakehome", XDG_RUNTIME_DIR=OUT + "/run", XDG_CACHE_HOME=OUT + "/fakehome/.cache", XDG_CONFIG_HOME=OUT + "/fakehome/.config", XDG_DATA_HOME=OUT + "/fakehome/.local/share", LC_ALL="C")
    # the project: a copy of the person's KiCad files (or the repo's fixture board), never the original
    proj = OUT + "/proj/lamp"; os.makedirs(proj)
    if o.project:
        for f in os.listdir(o.project):
            p = os.path.join(o.project, f)
            if os.path.isfile(p) and (f.startswith("kicad_") or ".kicad_" in f or f in ("fp-lib-table", "sym-lib-table")): shutil.copy2(p, proj)
    else:
        shutil.copy2(ROOT / "test/files/fixtures/plot/ecc83-pp.kicad_pcb", proj + "/lamp.kicad_pcb")
        open(proj + "/lamp.kicad_pro", "w").write("{}\n")
    pcbs = sorted(glob.glob(proj + "/*.kicad_pcb")); assert pcbs, "no board in the project"
    pcb = os.path.basename(pcbs[0])
    # the design thread already wrote the board's fab file (Order.prompt.fab): measured from a throwaway export
    subprocess.run([sys.executable, str(ROOT / "test/order/export_kicad.py"), pcbs[0], OUT + "/measure", "--fab-json", proj + "/" + pcb.replace(".kicad_pcb", ".fab.json")], check=True, cwd=proj)
    shutil.rmtree(OUT + "/measure")
    exporter = str(ROOT / "test/order/export_kicad.py")
    # the buy thread's agent, scripted by what each message says (Order.prompt.buy's steps)
    SCRIPT = [
        {"when": "^Review this project for manufacturing", "shell": "mkdir -p .backplane && printf '# Manufacturing review\\n\\nNothing a fab would refuse.\\n' > .backplane/review.md",
         "calls": [{"name": "order_review", "arguments": {"action": "record", "verdict": "ok", "summary": "Nothing a fab would refuse", "issues": []}}], "say": "Reviewed for manufacturing: nothing a fab would refuse."},
        {"when": "Delegated task .* finished", "calls": [{"name": "order_review", "arguments": {"action": "gate"}},
         {"name": "order_ask", "arguments": {"question": "Ship to which country?", "typed": True}}], "say": "The review passed."},
        {"when": "^Review for manufacturing", "calls": [{"name": "delegate_task", "arguments": {"prompt": "Review this project for manufacturing.", "provider": "claude"}}],
         "say": "The review runs as a task of this thread; the order goes on when it's done."},
        {"when": "^Order (without a review|anyway)", "calls": [{"name": "order_ask", "arguments": {"question": "Ship to which country?", "typed": True}}], "say": "Going on without a review."},
        {"when": "^(US|USA|United States)", "calls": [{"name": "order_ask", "arguments": {"question": "Ship to which US state?", "typed": True}}], "say": "To the US."},
        {"when": "^(GA|Georgia)", "shell": f"d=mfg/$(date +%Y%m%d-%H%M%S) && {sys.executable} {exporter} {pcb} $d --name 'Main board' --qty 5 --ship-to US --ship-region GA --title 'Lamp board' >&2 && echo $d",
         "calls": [{"name": "order_quote", "arguments": {"dir": "{{out}}"}}], "say": "Exported the board's gerbers and drill files; Backplane asks you before sending them."},
        {"when": "quote is posted", "say": "It covers the boards, shipping, US import duty and Georgia's sales tax."},
        {"when": "^Buy it", "calls": [{"name": "order_review", "arguments": {"action": "gate"}}], "say": "Ordering Main board ×5 (4-layer)."},
    ]
    bun = os.environ.get("BUN") or shutil.which("bun") or "bun"
    open(OUT + "/bin/claude", "w").write(f'#!/bin/sh\nexec {bun} {ROOT}/test/tools/claude_standin.ts "$@"\n'); os.chmod(OUT + "/bin/claude", 0o755)
    for n in ("codex", "grok", "muse", "opencode", "agy", "pi", "omp"): open(f"{OUT}/bin/{n}", "w").write("#!/bin/sh\nexit 1\n"); os.chmod(f"{OUT}/bin/{n}", 0o755)
    # Buy now's browser: the link is recorded, then opened as a browser would (through the order server's redirect to the Stripe double, which pays)
    open(OUT + "/bin/xdg-open", "w").write(f'#!/bin/sh\nprintf "%s\\n" "$1" > {OUT}/opened.txt\nexec curl -sS -L -o {OUT}/opened.html "$1"\n'); os.chmod(OUT + "/bin/xdg-open", 0o755)
    open(OUT + "/bin/notify", "w").write(f'#!/bin/sh\nprintf "%s\\n" "$*" >> {OUT}/notify.log\ncase "$1" in --close) ;; *) echo 7 ;; esac\n'); os.chmod(OUT + "/bin/notify", 0o755)

    # the order server on loopback (local_server.py refuses a Stripe that is not)
    fport, port, sport = free_port(), free_port(), free_port()
    fenv = {**env, "FAB_PAUSED": "1"} if o.paused else env
    spawn([o.python, str(ROOT / "test/order/local_server.py"), "--port", str(fport), "--stripe-port", str(sport), "--data", OUT + "/fab"], "fab.log", env=fenv)
    t = time.time()
    while get(f"http://127.0.0.1:{fport}/v1/health")[0] != 200:
        if time.time() - t > 40: raise SystemExit("FAIL the order server did not start: " + open(OUT + "/fab.log", errors="replace").read()[-600:])
        time.sleep(0.3)
    # a private X server, a window manager, the hub on a temp home, the window
    XV = os.environ.get("XV", "")
    xvfb = XV + "/usr/bin/Xvfb" if XV else "Xvfb"
    spawn([xvfb, DISP, "-screen", "0", "1600x1000x24", "-nolisten", "tcp", "-extension", "GLX"], "xvfb.log", env={**env, "LD_LIBRARY_PATH": f"{XV}/usr/lib:{XV}/usr/lib/x86_64-linux-gnu" if XV else ""})
    time.sleep(1.5)
    twm = os.environ.get("TWM", "")
    if twm:
        open(OUT + "/twmrc", "w").write("NoTitle\nRandomPlacement\n"); spawn([twm, "-f", OUT + "/twmrc"], "wm.log", env={**env, "DISPLAY": DISP})
    elif shutil.which("i3"):
        open(f"{OUT}/i3.conf", "w").write(f"new_window none\nfor_window [class=\".*\"] border none\ndefault_border none\nipc-socket {OUT}/i3.sock\n"); spawn(["i3", "-c", f"{OUT}/i3.conf"], "wm.log", env={**env, "DISPLAY": DISP})
    time.sleep(1)
    now = int(time.time())
    ev = [{"$": "SettingSet", "key": k, "value": v, "at": now} for k, v in (("restart.continue", "off"), ("onboard.done", "1"), ("onboard.uber", "1"), ("order.enabled", "on"))] + [
          {"$": "ProjectCreated", "id": "p1", "title": "lamp", "root": proj, "at": now},
          {"$": "ThreadCreated", "id": "t1", "project": "p1", "title": "Lamp board", "env": "local", "branch": "", "worktree": "", "provider": "claude", "model": "", "at": now},
          {"$": "MessagePosted", "thread": "t1", "msg": "m1", "role": "user", "text": "Lay out the lamp board and write its fab file.", "at": now},
          {"$": "MessagePosted", "thread": "t1", "msg": "m2", "role": "assistant", "text": "Done: the board is routed and its fab file is written.", "at": now}]
    open(f"{OUT}/home/events.jsonl", "w").write("".join(json.dumps(e, separators=(",", ":")) + "\n" for e in ev))
    hub = start_hub(fport, port)
    json.dump([{"select": 0}, {"wait": 900000}], open(OUT + "/script.json", "w"))
    cenv = {**env, "DISPLAY": DISP, "PATH": OUT + "/bin:" + env["PATH"], "DBUS_SESSION_BUS_ADDRESS": "unix:path=" + OUT + "/no-bus", "BACKPLANE_GPUI_NOTIFY_CMD": OUT + "/bin/notify",
            "BACKPLANE_GPUI_RECTS": "1", "LP_NUM_THREADS": "2", "LIBGL_ALWAYS_SOFTWARE": "1", "BACKPLANE_GPUI_LOCAL": OUT + "/local.json"}
    spawn([o.client, "--url", f"http://127.0.0.1:{port}", "--token-file", OUT + "/home/token", "--script", OUT + "/script.json", "--exit-after", "1500"], "client.log", env=cenv)
    wait("the window", lambda: metrics("usable") and "ui:Draw" in controls(), 90); time.sleep(2)
    OFF[:] = window_at(); print("window at", OFF, flush=True)
    gpu = (metrics("gpu") or [{}])[-1]
    if gpu.get("software") is False: raise SystemExit(f"FAIL the window renders on a real GPU ({gpu.get('device')}): run it through a no-GPU wrapper")

    # 1. the design thread's header: Buy it with the estimate
    wait("Buy it with a price in the header", lambda: "from $" in (controls().get("tool:fab-buy", {}).get("label") or ""), 60)
    r = rect_of("tool:fab-buy"); lab = controls()["tool:fab-buy"]["label"]; time.sleep(1); shot("01-buy-it")
    check(lab.startswith("Buy it · ") and "from $" in lab, "the header offers Buy it with the estimate", lab)
    press_rect(r)
    # 2. the buy thread: its title, its review gate
    buy = wait("a buy thread", lambda: next((e["id"] for e in events() if e.get("$") == "ThreadCreated" and e["id"] != "t1"), None), 20)
    title = next(e.get("title", "") for e in events() if e.get("$") == "ThreadCreated" and e["id"] == buy)
    check(title.startswith("Buy: lamp"), 'the buy thread is named "Buy: <project>"', title)
    check(setting(f"buy.{buy}") == "t1", "the buy thread is marked as the design thread's", setting(f"buy.{buy}"))
    r = wait("the review gate", lambda: labelled("ask:", "Review for manufacturing"), 40); time.sleep(1); shot("02-review-gate")
    check(labelled("ask:", "Order without a review") is not None, "the gate offers Review for manufacturing or Order without a review")
    check(not rect_of("tool:fab-buy"), "a buy thread has no Buy it of its own")
    press_rect(r)
    # 3. the review runs as a task of the buy thread, records its verdict, and the gate lets the order go on
    rv = wait("the review recorded", lambda: os.path.exists(proj + "/.backplane/review.json") and json.load(open(proj + "/.backplane/review.json")), 60)
    check(rv.get("verdict") == "ok" and len(rv.get("fingerprint", "")) == 32, "the review task records an ok verdict with the design's fingerprint", rv)
    child = next((e for e in events() if e.get("$") == "ThreadCreated" and e["id"] not in ("t1", buy)), None)
    check(child is not None and child.get("project") == "p1", "the review ran as a thread of the same project", child)
    # 4. where it ships: typed, country then state
    answer_typed("the country", "US", 60); shot("03-country")
    wait("the state question", lambda: any("US state" in x.get("detail", "") for x in asks_open(buy)), 40)
    check(posted(buy, lambda e: e.get("role") == "user" and e.get("text", "").strip() == "US") is not None, "the typed country is posted as the person's message")
    answer_typed("the state", "GA", 30)
    # 5. the export and the files' approval
    r = wait("the files' approval with Send", lambda: labelled("ask:", "Send"), 90); time.sleep(1); shot("04-files-approval")
    ap = next((x for x in asks_open(buy) if x.get("tool") == "order_quote"), {})
    check("Ships to US (GA)" in ap.get("detail", ""), "the approval says where it ships, with the state", ap.get("detail", "")[:200])
    m = sorted(glob.glob(proj + "/mfg/*/manifest.json"))
    man = json.load(open(m[-1])) if m else {}
    check(man.get("ship_to") == "US" and man.get("ship_region") == "GA" and any(k.endswith(".drl") for k in man.get("files", {})), "the manifest has the country, the state and the drill files", man.get("ship_region"))
    check(not any(f.startswith("q_") for f in os.listdir(OUT + "/fab")), "nothing reached the order server before Send", os.listdir(OUT + "/fab"))
    # the hub restarts (an update, say) while the person looks at the files: the card stays and its Send still sends what they saw
    stop(hub); hub = start_hub(fport, port)
    r = wait("the files' approval back after the restart", lambda: labelled("ask:", "Send"), 60); time.sleep(1.5); r = labelled("ask:", "Send") or r
    check(any(x.get("tool") == "order_quote" for x in asks_open(buy)), "the files' approval outlives a hub restart")
    press_rect(r)
    # 6. waiting for the quote: the card says so, the thread waits
    wait("the thread waiting on the quote", lambda: setting(f"order.wait.{buy}"), 20); shot("05-waiting")
    check(True, "after Send the thread waits on the quote (order.wait)")
    ans = next((e for e in events() if e.get("$") == "ActivityLogged" and e.get("thread") == buy and e.get("kind") == "answer"), {})
    check(ans.get("summary", "").startswith("Sent for a quote: ") and "\n" not in ans.get("summary", ""), "the thread keeps one line of what was sent", ans.get("summary"))
    # 7. the quote: four lines, the card's total and customs, Buy now
    r = wait("Buy now on the Order card", lambda: rect_of("ui:Buy now"), 150); time.sleep(2); shot("06-quote")
    q = posted(buy, lambda e: str(e.get("msg", "")).startswith("q:"))
    text = (q or {}).get("text", "")
    for line in ("Manufacturing", "Shipping", "Duties & taxes", "Sales tax (estimated)"):
        check(f"| {line} |" in text, f"the posted quote has its {line} line", text[:400])
    check("Georgia" in text and "Includes import VAT and duties" in text, "the quote names the state and says what it covers about customs", text[:400])
    check("?t=" not in text and "(order:q_" in text, "the log keeps the link without its token")
    card = json.loads(setting(f"order.quote.{buy}") or "{}")
    check(card.get("payable") is True and card.get("total", "").startswith("$"), "the card's quote can be paid and has a total", card)
    check(not setting(f"order.wait.{buy}"), "the thread no longer waits once the quote is posted")
    lab = controls().get("ui:Buy now", {}).get("label", "")
    check(lab == f"Buy now · {card.get('total')}", "Buy now names the total", lab)
    # 8. Buy now: the order server's page
    press_rect(r)
    opened = wait("Buy now opened a link", lambda: os.path.exists(OUT + "/opened.txt") and open(OUT + "/opened.txt").read().strip(), 15)
    check(opened.startswith(f"http://127.0.0.1:{fport}/q/"), "Buy now opens the order server's page for this quote", opened.split("?")[0])
    if o.paused:
        time.sleep(1); page = open(OUT + "/opened.html", errors="replace").read() if os.path.exists(OUT + "/opened.html") else ""
        check("Ordering is paused" in page, "with paying paused, Buy now shows the paused page", page[:200])
        check(not re.search(r"jlc", page, re.I), "the paused page names no fab")
    else:
        wait("the Paid step", lambda: posted(buy, lambda e: str(e.get("msg", "")).endswith(":paid")), 100); time.sleep(3); shot("07-paid")
        check(True, "the test payment comes back as Paid in the thread")
        qid = q["msg"][2:]; tok = json.load(open(f"{OUT}/home/orders/{qid}.json"))["token"]
        st, body = get(f"http://127.0.0.1:{fport}/v1/quotes/{qid}", {"Authorization": f"Bearer {tok}", "X-Backplane-Install": "e2e"})
        oid = json.loads(body)["order"]["id"]
        # 9. the operator places it, then ships it; a restarted hub says each at once
        for status, word, extra in (("placed", "**Ordered.**", {}), ("shipped", "**Shipped.** Tracking: 1Z999E2E", {"tracking": "1Z999E2E"})):
            data = urllib.parse.urlencode({"status": status, **extra}).encode()
            req = urllib.request.Request(f"http://127.0.0.1:{fport}/admin/order/{oid}", data=data, headers={"Cookie": "fab_admin=admin-token-for-local-tests-0123"})
            try: urllib.request.urlopen(req, timeout=5)
            except urllib.error.HTTPError as e: check(e.code in (303, 302), f"the operator marks it {status}", e.code)
            stop(hub); hub = start_hub(fport, port)
            m = wait(f"the {status} step in the thread", lambda: posted(buy, lambda e: str(e.get("msg", "")).endswith(f":{status}")), 60)
            check(m["text"].startswith(word), f"marked {status}, the thread says {word}", m["text"])
        wait("the window back on the hub", lambda: rect_of(f"thread:{buy}"), 60); time.sleep(3); shot("08-shipped")
        card = json.loads(setting(f"order.quote.{buy}") or "{}")
        check(card.get("state") == "shipped" and card.get("tracking") == "1Z999E2E", "the card follows the order to Shipped with its tracking", card)
    real = [l for l in open(OUT + "/hub.log", errors="replace") if "api.anthropic" in l or "backplane.works" in l]
    check(not real, "no real model or order server was called", real[:1])
    print(f"{len(passes)} passed, {len(fails)} failed", flush=True)
    if o.keep:
        print(f"up: hub {port}, fab {fport}; touch {OUT}/stop to end", flush=True)
        while not os.path.exists(OUT + "/stop"): time.sleep(1)
finally:
    for p in reversed(procs): stop(p)
sys.exit(1 if fails else 0)
