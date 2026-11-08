#!/usr/bin/env python3
# Writes test/fixtures/v014-home: a small Backplane 0.14 home for the 0.14 import's tests (native/host/src/import014). Each event has the shape
# v0.14.1 wrote (field order included); the names, ids, text and keys are made up. Run from the repo root: python3 test/fixtures/make-v014-home.py
# A large home for the progress and probation tests (not committed): make-v014-home.py OUT --large PROJECTS THREADS LINES writes OUT with that many projects
# and threads, and history lines spread over the threads (messages and tool activity, long texts), plus the small home's bots, rooms and devices.
import hashlib, json, os, shutil, sys
args = sys.argv[1:]
D = args[0] if args and not args[0].startswith('--') else 'test/fixtures/v014-home'
LARGE = tuple(int(x) for x in args[args.index('--large') + 1:args.index('--large') + 4]) if '--large' in args else None
shutil.rmtree(D, ignore_errors=True); os.makedirs(D)
E = []
def e(**k):
    t = k.pop('t'); E.append(json.dumps({"$": t, **k}, separators=(',', ':')))
T1, T2, TB = "t1790175755-10", "t1790175900-20", "t1790292962-8017"; P1, P2 = "p1790175753-8", "p1790175760-9"; B = "b1790292962-8017"
e(t="SettingSet", key="machine.id", value="m0123456789abcdef", at=1790175000)
e(t="SettingSet", key="agent.provider", value="codex", at=1790175657)
e(t="SettingSet", key="agent.provider", value="claude", at=1790175658)
e(t="SettingSet", key="ui.theme", value="dark", at=1790175659)
e(t="SettingSet", key="allowance.codex", value="{}", at=1790175660)
e(t="ProjectCreated", id=P1, title="Nordic", root="~/Documents/Nordic", at=1790175753)
e(t="ProjectCreated", id=P2, title="Old idea", root="~/Documents/old-idea", at=1790175760)
e(t="ThreadCreated", id=T1, project=P1, title="New thread", env="local", branch="", worktree="", provider="claude", model="opus", at=1790175755)
e(t="ModesSet", id=T1, runtime="full-access", interaction="default", model="opus", at=1790175766)
e(t="ThreadRenamed", id=T1, title="Pick a regulator", at=1790175780)
e(t="MessagePosted", thread=T1, msg="m1790175780-13", role="user", text="Which LDO fits 3.3 V at 300 mA?", at=1790175780)
e(t="TurnChanged", thread=T1, state="running", at=1790175780)
e(t="ActivityLogged", thread=T1, act="e1790175781-16", tone="tool", kind="claude", summary="jlc_search LDO 3.3V", at=1790175781)
e(t="UsageSet", thread=T1, used=12000, at=1790175782)
E.append('{"$":"MessagePosted","thread":"%s","msg":"m1790175790-18","role":"assistant","text":"An AP2112K-3.3 fits \\ud83d: 600 mA, SOT-23-5.","at":1790175790}' % T1)  # a lone surrogate, as 0.14 kept a pasted half emoji
e(t="TurnUsage", thread=T1, cost=1, at=1790175790)
e(t="TurnChanged", thread=T1, state="completed", at=1790175791)
e(t="SessionSet", id=T1, session="47119e0e-5017-4813-8353-70a01f309d22", at=1790175791)
e(t="ThreadSettled", id=T1, manual=True, at=1790175800)
e(t="SettingSet", key="summary.at." + T1, value="1790175800", at=1790175800)
e(t="ThreadCreated", id=T2, project=P1, title="Layout", env="worktree", branch="backplane/a1", worktree="", provider="claude", model="opus", at=1790175900)
e(t="TaskDelegated", task="k1790175901-21", parent=T1, child=T2, depth=1, provider="claude", model="opus", at=1790175901)
e(t="TaskStateSet", task="k1790175901-21", state="running", summary="placing parts", at=1790175901)
e(t="TurnChanged", thread=T2, state="running", at=1790175902)
e(t="AskOpened", id="a1790175903-22", thread=T2, at=1790175903)
e(t="TurnQueued", thread=T2, text="and the silkscreen", at=1790175904)
e(t="WakeSet", id="w1790175905-23", thread=T1, at=1790175905)
e(t="LockChanged", name="build", lock="held", at=1790175906)
e(t="ThreadCreated", id="t1790175910-24", project=P2, title="Gone with its project", env="local", branch="", worktree="", provider="claude", model="", at=1790175910)
e(t="MessagePosted", thread="t1790175910-24", msg="m1790175911-25", role="user", text="hello", at=1790175911)
e(t="ProjectRemoved", id=P2, at=1790175920)
e(t="ThreadCreated", id="t1790175930-26", project=P1, title="Deleted", env="local", branch="", worktree="", provider="claude", model="", at=1790175930)
e(t="ThreadDeleted", id="t1790175930-26", at=1790175931)
e(t="BotSet", id=B, name="Ada", thread=TB, look=306440, persona="Answers the person's mail.", at=1790292962)
e(t="ThreadCreated", id=TB, project="", title="Ada", env="local", branch="", worktree="", provider="claude", model="opus", at=1790292962)
e(t="MessagePosted", thread=TB, msg="m1790292970-8018", role="user", text="Remember that I like 0402 parts.", at=1790292970)
e(t="MemorySaved", bot=B, key="", kind="fact", text="The person likes 0402 parts.", tags="", at=1790292971)
e(t="SpaceSet", bot=B, spec="{\"title\":\"Ada\",\"blocks\":[]}", at=1790292972)
e(t="RoutineSet", id="o1790305944-9249", bot=B, name="Morning check", cron="0 8 * * *", prompt="Read the overnight mail.", on=True, at=1790305944)
e(t="HookSet", id="h1790305792-9226", bot=B, name="CI", revoked=False, at=1790305792)
e(t="BotWoke", bot=B, hop=1, at=1790305950)
e(t="RoutineFired", id="o1790305944-9249", at=1790306000)
e(t="RunSet", thread="run1792383741-196955", bot=B, conv="wake:w1", mail="t:t1:lr-r1", at=1792383741)
e(t="ThreadCreated", id="run1792383741-196955", project=P1, title="Mail run", env="local", branch="", worktree="", provider="claude", model="", at=1792383741)
e(t="MessagePosted", thread="run1792383741-196955", msg="m1792383742-1", role="user", text="mail in", at=1792383742)
e(t="MailPosted", id="mail1792383698-196877", conv="pair:x", **{"from": "thread:a", "to": "thread:b"}, kind="thread", ref="", want="fyi", head="hi", size=2, at=1792383698)
e(t="SettingSet", key="liveness.run1792383741-196955", value="1", at=1792383743)
e(t="SettingSet", key="summary.at.run1792383741-196955", value="1", at=1792383743)
e(t="RoomSet", id="r1790349564-10743", name="Everybody", members=B, at=1790349564)
e(t="RoomPosted", id="p1790373060-12782", room="r1790349564-10743", **{"from": "Ada"}, text="Good morning.", hop=1, at=1790373060)
e(t="UberSet", bot=B, uid="u1791867695-0011223344556677", at=1791867695)
e(t="DeviceRegistered", token="a" * 64, platform="ios", kind="alert", thread="", env="production", bundle="dev.backplane.works", at=1790213430)
e(t="DeviceRegistered", token="b" * 64, platform="android", kind="alert", thread="", env="production", bundle="dev.backplane.works", at=1790213431)
e(t="DeviceDropped", token="b" * 64, at=1790213432)
e(t="PeerSet", id="p97a9070fbff5a66b", name="laptop", url="http://laptop.example.ts.net:3787", revoked=False, at=1790349575)
e(t="DeployBuilt", id="d1", at=1790349576)
e(t="CtxNoted", thread=T1, at=1790349577)
e(t="SettingSet", key="told.k1792001002-105187-1", value="1", at=1792001002)
if LARGE:
    P, T, L = LARGE
    words = "board schematic regulator footprint layer via trace net clearance pad stencil silkscreen copper ground plane decoupling capacitor".split()
    pids = [f"p1791000000-{i}" for i in range(P)]
    for i, pid in enumerate(pids): e(t="ProjectCreated", id=pid, title=f"Project {i:03d}", root=f"~/Documents/large/p{i:03d}", at=1791000000 + i)
    per = max(1, (L - P - T) // max(T, 1))
    for j in range(T):
        tid = f"t1791100000-{j}"; at = 1791100000 + j * 10
        e(t="ThreadCreated", id=tid, project=pids[j % P], title=f"Thread {j:05d} " + " ".join(words[(j + k) % len(words)] for k in range(3)), env="local", branch="", worktree="", provider="claude", model="opus", at=at)
        for k in range(per):
            text = " ".join(words[(j * 7 + k * 3 + n) % len(words)] for n in range(40))
            if k % 3 == 2: e(t="ActivityLogged", thread=tid, act=f"e{j}-{k}", tone="tool", kind="claude", summary=text[:120], at=at + k)
            else: e(t="MessagePosted", thread=tid, msg=f"m{j}-{k}", role="user" if k % 3 == 0 else "assistant", text=text, at=at + k)
# a torn tail: 0.14 was writing when the import read it
open(f'{D}/events.jsonl', 'w').write('\n'.join(E) + '\n{"$":"MessagePosted","thread":"%s","msg":"torn' % T1)
# a home 0.14 can boot: its pairing.js wants devices-v1.json's owner to be the sha256 of the home's token (a made-up one)
TOKEN = "0" * 32
open(f'{D}/token', 'w').write(TOKEN)
json.dump({"version": 1, "owner": hashlib.sha256(TOKEN.encode()).hexdigest(), "devices": [
    {"id": "1" * 32, "hash": "2" * 64, "name": "Phone", "createdAt": 1790213400000, "lastSeen": 1790213400000, "revoked": False},
    {"id": "3" * 32, "hash": "4" * 64, "name": "Old tablet", "createdAt": 1790213400000, "lastSeen": 1790213400000, "revoked": True}]}, open(f'{D}/devices-v1.json', 'w'))
pid, sid = "5e70d94b-6252-4f29-8eed-d3bd8c4b961a", "02b33ae0-d452-432d-8838-bc364d86404d"
st = f'{D}/providers/accounts-v1/{pid}/{sid}'
os.makedirs(f'{st}/projects/-home-a-Documents-Nordic')
json.dump({"version": 1, "profiles": [{"id": pid, "provider": "claude", "label": "Work", "epoch": 2, "fingerprint": "f" * 64, "store": sid}]}, open(f'{D}/providers/accounts-v1/profiles.json', 'w'))
open(f'{st}/.credentials.json', 'w').write('{"fixture":"not a real login"}\n')
open(f'{st}/projects/-home-a-Documents-Nordic/47119e0e-5017-4813-8353-70a01f309d22.jsonl', 'w').write('{"type":"user","fixture":true}\n')
for d in ['secrets/hooks', 'bots/' + B, 'attachments/desktop_3787_' + T1, 'message-images', 'mail', 'browser-profile']: os.makedirs(f'{D}/{d}', exist_ok=True)
files = {'secrets/openai.key': 'sk-fixture-not-real\n', 'secrets/google.json': '{"fixture":true}\n', 'secrets/hooks/h1790305792-9226': 'fixture-hook-secret\n',
    'secrets/hooks/h1790305792-9226.token': 'fixture-hook-token\n', f'bots/{B}/notes.md': '# Ada\n\nfixture notes\n', f'attachments/desktop_3787_{T1}/datasheet.txt': 'fixture attachment\n',
    'message-images/00a0.json': '{"image":"x.png"}\n', 'mail/mail1792383698-196877.json': '{}\n', 'browser-profile/Cookies': 'fixture\n',
    'README.md': 'A small Backplane 0.14 home for the 0.14 import tests (native/host/src/import014), written by test/fixtures/make-v014-home.py. Each line has the shape v0.14.1 wrote; names, ids, text and keys are made up. The last line of events.jsonl is torn on purpose. Tests copy this folder to a temp dir and import the copy.\n'}
for p, t in files.items(): open(f'{D}/{p}', 'w').write(t)
