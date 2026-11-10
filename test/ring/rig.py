#!/usr/bin/env python3
"""A temp hub for driving the Ring page on a phone (docs/ring.md): stand-in agents, the real voice helper and local model, ring not set up.

    python3 -I test/ring/rig.py up HUB_BIN VOICE_BIN LOCAL_DIR DIR     starts it in the background; DIR/env has PORT, HOST, TOKEN, HOME, LINK
    python3 -I test/ring/rig.py test DIR                               the Pebble app's Send test event, to the hook the phone set up
    python3 -I test/ring/rig.py record DIR SAMPLE.m4a [DELIVERY]       a recording as the Pebble app sends it (Hold & talk, Both)
    python3 -I test/ring/rig.py down DIR

HOST (default 127.0.0.1) is where the hub listens: the emulator reaches the host's loopback as 10.0.2.2; a simulator on another Mac needs the tailnet address.
The hook and its token are read from the hub's own files (what Set up made), as the person would paste them from the phone."""
import glob, json, os, signal, socket, subprocess, sys, time, urllib.error, urllib.request

def env_of(d): return dict(l.split("=", 1) for l in open(f"{d}/env").read().split("\n") if "=" in l)
def hook_of(d):
    e = env_of(d); toks = glob.glob(f"{e['HOME']}/secrets/hooks/*.token")
    if not toks: sys.exit("no webhook yet: press Set up on the phone first")
    p = max(toks, key=os.path.getmtime); return e, os.path.basename(p)[:-6], open(p).read().strip()
def post(url, headers, body):
    r = urllib.request.Request(url, data=body, headers=headers, method="POST")
    try:
        with urllib.request.urlopen(r, timeout=30) as a: return a.status, a.read().decode()
    except urllib.error.HTTPError as x: return x.code, x.read().decode()

def up(hub, voice, local, d):
    os.makedirs(d, exist_ok=True); home = f"{d}/home"; agents = f"{d}/bin"
    os.makedirs(f"{home}/voice", exist_ok=True); os.makedirs(agents, exist_ok=True); os.makedirs(f"{d}/fakehome", exist_ok=True)
    if not os.path.lexists(f"{home}/voice/local"): os.symlink(os.path.abspath(local), f"{home}/voice/local")
    for n in ["claude", "codex", "grok", "muse", "agy", "gemini"]:
        p = f"{agents}/{n}"; open(p, "w").write("#!/bin/sh\nexit 1\n"); os.chmod(p, 0o755)
    now = int(time.time())
    ev = [{"$": "SettingSet", "key": k, "value": v, "at": now} for k, v in [("restart.continue", "off"), ("onboard.done", "1"), ("onboard.uber", "1"), ("voice.engine", "local"), ("voice.model", "parakeet-v2"), ("person.tz", "UTC")]]
    ev += [{"$": "ThreadCreated", "id": "tbub", "project": "", "title": "Bub", "env": "local", "branch": "", "worktree": "", "provider": "claude", "model": "", "at": now},
           {"$": "BotSet", "id": "bbub", "name": "Bub", "thread": "tbub", "look": 7, "persona": "", "at": now}]
    open(f"{home}/events.jsonl", "w").write("".join(json.dumps(e) + "\n" for e in ev))
    host = os.environ.get("HOST", "127.0.0.1"); port = int(os.environ.get("PORT", "0"))
    if not port: s = socket.socket(); s.bind(("127.0.0.1", 0)); port = s.getsockname()[1]; s.close()
    env = dict(os.environ, BACKPLANE_AGENTS=agents, BACKPLANE_NO_UPDATE="1", BACKPLANE_VOICE_BIN=os.path.abspath(voice), HOME=f"{d}/fakehome", LC_ALL="C")
    p = subprocess.Popen([hub, "--home", home, "--port", str(port), "--host", host, "--no-tailscale", "--foreground"], stdout=open(f"{d}/hub.log", "wb"), stderr=subprocess.STDOUT, env=env, start_new_session=True)
    for _ in range(150):
        try: urllib.request.urlopen(f"http://{host}:{port}/healthz", timeout=2); break
        except Exception: time.sleep(0.2)
    tok = open(f"{home}/token").read().strip()
    open(f"{d}/env", "w").write(f"PORT={port}\nHOST={host}\nTOKEN={tok}\nHOME={home}\nPID={p.pid}\nLINK=http://{os.environ.get('LINKHOST', host)}:{port}/#token={tok}\n")
    print(open(f"{d}/env").read().replace(tok, "<token>"), end="")

def main():
    cmd = sys.argv[1]
    if cmd == "up": up(*sys.argv[2:6])
    elif cmd == "down":
        e = env_of(sys.argv[2])
        try: os.killpg(int(e["PID"]), signal.SIGTERM)
        except ProcessLookupError: pass
        print("down")
    elif cmd == "test":
        e, hid, tok = hook_of(sys.argv[2]); hs = {"Content-Type": "multipart/form-data; boundary=bnd", "Authorization": f"Bearer {tok}", "X-Index-Webhook-Version": "1", "X-Index-Delivery": f"test-{time.time()}", "X-Index-Trigger": "test-event", "X-Index-Test": "true"}
        print(*post(f"http://{e['HOST']}:{e['PORT']}/hook/{hid}", hs, b"--bnd\r\nContent-Disposition: form-data; name=\"transcription\"\r\n\r\nThis is a test\r\n--bnd\r\nContent-Disposition: form-data; name=\"client\"\r\n\r\nring\r\n--bnd--\r\n"))
    elif cmd == "record":
        e, hid, tok = hook_of(sys.argv[2]); audio = open(sys.argv[3], "rb").read(); dl = sys.argv[4] if len(sys.argv) > 4 else f"rec-{int(time.time())}"
        body = b"--bnd\r\nContent-Disposition: form-data; name=\"audio\"; filename=\"" + dl.encode() + b".m4a\"\r\nContent-Type: audio/mp4\r\n\r\n" + audio + b"\r\n"
        for k, v in [("transcription", "the apps own words"), ("recordedAt", str(int(time.time() * 1000) - 60000)), ("client", "ring")]: body += f"--bnd\r\nContent-Disposition: form-data; name=\"{k}\"\r\n\r\n{v}\r\n".encode()
        body += b"--bnd--\r\n"
        hs = {"Content-Type": "multipart/form-data; boundary=bnd", "Authorization": f"Bearer {tok}", "X-Index-Webhook-Version": "1", "X-Index-Delivery": dl, "X-Index-Trigger": "single-click-hold", "X-Audio-Size": str(len(audio))}
        print(*post(f"http://{e['HOST']}:{e['PORT']}/hook/{hid}", hs, body))
    else: sys.exit(__doc__)

main()
