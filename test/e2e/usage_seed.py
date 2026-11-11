#!/usr/bin/env python3
"""A log for the Context panel's screenshots (scripts/desktop-usage-shot.sh): threads at 10%, 64% and 93% of their context with a history of turns,
and a bot's chat (the uberbot) with memories, a past account handover and mailroom lanes today. Usage: usage_seed.py OUT.jsonl PROJECT_ROOT"""
import json, random, sys, time

out, root = sys.argv[1], sys.argv[2]
now = int(time.time()); random.seed(7)
ev = []
def e(_k, **f): f.setdefault("at", now); ev.append({"$": _k, **f})
def setting(k, v, at=None): e("SettingSet", key=k, value=v, at=at or now)

e("ProjectCreated", id="p1", title="lamp", root=root, at=now - 9 * 86400)
# the CLI's own login (a saved account's inventory is the hub's to keep): its reading says the email and what is left
setting("allowance.claude", json.dumps({"provider": "claude", "email": "kellyhlord@example.com", "windows": [{"scope": "five_hour", "used_permille": 30, "resets_at": now + 7200}, {"scope": "seven_day", "used_permille": 410, "resets_at": now + 3 * 86400}]}), now - 30 * 86400)

def turns(th, model, n, ctx0, ctx1, window, cost_scale, t0, step=600):
    """n turns, the context growing from ctx0 to ctx1"""
    for i in range(n):
        at = t0 + i * step
        used = int(ctx0 + (ctx1 - ctx0) * (i + 1) / n)
        e("UsageSet", thread=th, used=used, window=window, at=at)
        cr = int(used * random.uniform(0.9, 0.97)); cw = random.randint(800, 9000); inp = random.randint(2, 40); out = random.randint(200, 4000)
        cost = int((cr * 0.3 + cw * 3.75 + inp * 3 + out * 15) * cost_scale)
        e("TurnUsage", thread=th, provider="claude", model=model, key=f"{th}-{i}", input=inp, cread=cr, cwrite=cw, output=out, reasoning=0, cost=cost, known="iorwco", basis="b", at=at)
    setting(f"turn.speed.{th}", f"0/0/{random.randint(4000, 9000)}/{random.randint(300, 800)}/{random.randint(9000, 30000)}/5200/420/3", t0 + n * step)

for th, title, model, n, c0, c1, win in [("t1", "Bring-up notes", "claude-fable-5-2[1m]", 14, 20000, 100157, 1_000_000),
                                         ("t2", "Firmware refactor", "claude-opus-5-5", 22, 40000, 128400, 200_000),
                                         ("t3", "Enclosure fit check", "claude-sonnet-5-5", 30, 60000, 186900, 200_000)]:
    e("ThreadCreated", id=th, project="p1", title=title, env="local", branch="", worktree="", provider="claude", model=model, at=now - 3 * 86400)
    setting(f"account.thread.{th}.claude", json.dumps({"id": "", "epoch": 0}), now - 3 * 86400)
    turns(th, model, n, c0, c1, win, 1000, now - n * 600 - 60)
    e("SessionSet", id=th, session=f"S-{th}", at=now - n * 600 - 120)
e("MessagePosted", thread="t1", msg="m2", role="assistant", text="The driver is up: 350 mA into the string, ripple under 5%.", at=now - 200)

# the uberbot and its chat
e("ThreadCreated", id="tb", project="", title="Bub", env="local", branch="", worktree="", provider="claude", model="claude-fable-5-2[1m]", at=now - 20 * 86400)
e("BotSet", id="bub", name="Bub", thread="tb", look=3, persona="", at=now - 20 * 86400)
e("UberSet", bot="bub", uid="u1", at=now - 20 * 86400); setting("mail.on", "on", now - 20 * 86400)
setting("account.thread.tb.claude", json.dumps({"id": "", "epoch": 0}), now - 20 * 86400)
e("TurnQueued", thread="tb", msg=f"handoff:switch:account{now - 30 * 3600}-1", text="[Backplane] x\n\none\n\ntwo\n\n[End]", at=now - 30 * 3600)
e("QueueRemoved", thread="tb", msg=f"handoff:switch:account{now - 30 * 3600}-1", at=now - 30 * 3600 + 5)
e("SessionSet", id="tb", session="S-bub-2", at=now - 30 * 3600 + 10)
for k, kind, text in [("name", "user", "Kelly; builds hardware with Backplane"), ("tz", "user", "Europe/London"), ("pref.report-completions", "pref", "Tell me when work lands on main, in one line, with what to try"),
                      ("pref.quiet", "pref", "No progress chatter; only what needs me or what finished"), ("pref.links", "pref", "Give source links I can click"),
                      ("lamp.driver", "fact", "LED driver: 350 mA constant current, AL8860"), ("lamp.board", "fact", "boards/lamp: 2 layers, 48x30 mm"), ("note.fab", "note", "fab quote came back at $14 for 5")]:
    e("MemorySaved", bot="bub", key=k, kind=kind, text=text, tags="", at=now - 86400)
turns("tb", "claude-fable-5-2[1m]", 18, 80000, 214300, 1_000_000, 1000, now - 18 * 900, 900)
e("MessagePosted", thread="tb", msg="b2", role="assistant", text="The enclosure fit check, then the order.", at=now - 300)
for i in range(5):
    th = f"run-bub-{i}"
    e("ThreadCreated", id=th, project="p1", title=f"Bub: mail {i}", env="local", branch="", worktree="", provider="claude", model="claude-haiku-5-5", at=now - 5000 + i)
    e("RunSet", thread=th, bot="bub", conv="c", mail=f"m{i}", at=now - 5000 + i)
    turns(th, "claude-haiku-5-5", 2, 9000, 30000, 200_000, 300, now - 4000 + i * 300, 60)
th = "mailcoord-bub-1"
e("ThreadCreated", id=th, project="", title="Bub: private coordination", env="local", branch="", worktree="", provider="claude", model="claude-fable-5-2[1m]", at=now - 3000)
setting(f"mail.private.owner.{th}", "bub", now - 3000); setting(f"account.thread.{th}.claude", json.dumps({"id": "", "epoch": 0}), now - 3000)
turns(th, "claude-fable-5-2[1m]", 4, 30000, 60000, 1_000_000, 1000, now - 2500, 300)

ev.sort(key=lambda x: x["at"])
with open(out, "w") as f:
    for x in ev: f.write(json.dumps(x) + "\n")
print(len(ev), "events")
