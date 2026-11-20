#!/usr/bin/env python3
"""A log for the infinite-context progress screenshots (scripts/desktop-memory-shot.sh): the uberbot Bub with infinite context on and a long
conversation whose memory tree is part written (RecallNoted lines for the older messages), so its Settings tab and context chip show the bars.
Usage: memory_seed.py OUT.jsonl"""
import json, sys, time

out = sys.argv[1]; now = int(time.time()); ev = []
def e(_k, **f): f.setdefault("at", now); ev.append({"$": _k, **f})
def setting(k, v): e("SettingSet", key=k, value=v)

e("ThreadCreated", id="tb", project="", title="Bub", env="local", branch="", worktree="", provider="claude", model="claude-fable-5-2[1m]", at=now - 20 * 86400)
e("BotSet", id="bub", name="Bub", thread="tb", look=3, persona="", at=now - 20 * 86400)
e("UberSet", bot="bub", uid="u1", at=now - 20 * 86400)
setting("recall.bots", "on")
for k, v in [("onboard.done", "1"), ("onboard.uber", "1"), ("restart.continue", "off"), ("telemetry.usage.asked", "1")]: setting(k, v)
# the compactor stays away while the shots are taken (it would ask the stand-in claude)
setting("recall.jobs", "1")
N = 600
def long(i): return i % 6 == 3
for i in range(N):
    text = (f"Board rev {i}: checked the regulator, the USB-C CC resistors and the LED string. " * 12) if long(i) else f"message {i} about the lamp board, item {i}"
    e("MessagePosted", thread="tb", msg=f"m{i}", role="user" if i % 2 == 0 else "assistant", text=text, at=now - (N - i) * 60)
# the lines written so far: the long messages of the first 420, and every merge over the first 384 that its halves allow
built = {(0, i) for i in range(N) if not long(i) or i < 420}
nodes = [[0, i, f"{'user' if i % 2 == 0 else 'you'}: board rev {i} checked, regulator and CC resistors fine"] for i in range(420) if long(i)]
for l in range(1, 10):
    for i in range(N >> l):
        if ((i + 1) << l) <= 384 and (l - 1, 2 * i) in built and (l - 1, 2 * i + 1) in built:
            built.add((l, i)); nodes.append([l, i, f"messages {i << l} to {((i + 1) << l) - 1}: lamp board bring-up, rev checks, all fine"])
for k in range(0, len(nodes), 16): e("RecallNoted", thread="tb", nodes=nodes[k:k + 16])
with open(out, "w") as f:
    for x in ev: f.write(json.dumps(x) + "\n")
