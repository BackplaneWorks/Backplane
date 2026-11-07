#!/usr/bin/env python3
"""A throwaway home shaped like a real one, for scripts/smoke-start.sh and test/guard/faults.sh.

    make-home.py HOME [--small]

Every event name the hub's source writes appears, with fields of the right shape and of the wrong one: a log that has lived through many builds holds both, and no fold may fall over on it.
Bulk conversation fills it past the size at which the hub calls a log a copy (--small leaves that out). Lines that are not JSON sit in the middle, as a crash leaves them. Nothing here is real data.
"""
import hashlib, json, pathlib, re, secrets, sys

home = pathlib.Path(sys.argv[1]); small = "--small" in sys.argv
home.mkdir(parents=True, exist_ok=True)
src = pathlib.Path(__file__).resolve().parents[2] / "native" / "host" / "src"
names = sorted({m for f in src.rglob("*.rs") for m in re.findall(r'"\$":"([A-Z][A-Za-z]+)"', f.read_text(errors="replace"))})
names = names or ["ProjectCreated", "ThreadCreated", "MessagePosted", "ActivityLogged", "TurnChanged", "SettingSet"]

token = secrets.token_hex(24)
(home / "token").write_text(token)
(home / "hub.owner").write_text("smoke\n")
device = "0123456789abcdef0123456789abcdef"
(home / "devices-v1.json").write_text(json.dumps({"version": 1, "owner": hashlib.sha256(token.encode()).hexdigest(), "invitations": [],
    "devices": [{"id": "a" * 32, "hash": hashlib.sha256(device.encode()).hexdigest(), "name": "Test phone", "lastSeen": 0}]}))
root = home / "project"; root.mkdir(exist_ok=True)

lines = []
def ev(**k): lines.append(json.dumps(k, ensure_ascii=False))
ev(**{"$": "SettingSet", "key": "restart.continue", "value": "off", "at": 1})
ev(**{"$": "ProjectCreated", "id": "p1", "title": "A board", "root": str(root), "at": 1})
threads = [f"t{i}" for i in range(40)]
for t in threads:
    ev(**{"$": "ThreadCreated", "id": t, "project": "p1", "title": f"Thread {t}", "env": "local", "branch": "", "worktree": "", "provider": "claude", "model": "", "at": 2})

good = {"id": "x1", "thread": "t1", "project": "p1", "bot": "b1", "room": "r1", "msg": "m1", "act": "a1", "key": "k", "value": "v", "text": "words", "role": "user", "state": "completed", "kind": "note",
        "tone": "info", "tool": "Bash", "summary": "s", "name": "n", "title": "T", "root": str(root), "token": "t0k", "platform": "ios", "bundle": "b", "url": "http://x", "uid": "u1", "origin": "m1", "n": 1,
        "stamp": "0000000001.000000.m1", "op": "{}", "count": 1, "at": 3}
for n in names:
    ev(**{"$": n, **good})                                                       # the usual fields
    ev(**{"$": n, "at": 3})                                                      # none of them
    ev(**{"$": n, **{k: 7 for k in good}})                                       # numbers where text goes
    ev(**{"$": n, **{k: None for k in good}})
    ev(**{"$": n, **{k: {"a": [1]} for k in good}})
    ev(**{"$": n, "thread": "t1", "id": "é́🙂", "text": "😀 \\ \" \n", "at": -1})

for key in ["watch.t1", "bg.t1", "alert.t1", "carry.wait.t1", "merged.t1", "summary.at.t1", "peer.own.p1", "thread.parent.t1", "launch.input.t1", "ride.t1", "tz.rule.Nowhere", "uber.bot", "accounts.inventory", "allowance.claude"]:
    for v in ["", "1", "\t\t\t", "not json {", '{"a":1}', "x" * 3000]:
        ev(**{"$": "SettingSet", "key": key, "value": v, "at": 4})

lines.insert(len(lines) // 2, '{"$":"MessagePosted","thread":"t1","msg":"torn')                        # a write cut short
lines.insert(len(lines) // 2, "\0" * 64)                                                               # blocks a power cut left
lines.insert(len(lines) // 2, 'not json at all')
lines.insert(len(lines) // 2, '{"x":"\\é \\u0041"}')                                                   # a backslash before a two-byte character
lines.insert(len(lines) // 2, '{"$":"MessagePosted","thread":"t1","msg":"sur","role":"user","text":"\\ud83d half","at":5}')

if not small:
    for i in range(9000):
        t = threads[i % len(threads)]
        ev(**{"$": "MessagePosted", "thread": t, "msg": f"m{i}", "role": "assistant" if i % 2 else "user", "text": "A reply of a usual length, with a few words in it. " * 3, "at": 100 + i})
        if i % 3 == 0: ev(**{"$": "ActivityLogged", "thread": t, "act": f"a{i}", "tone": "tool", "kind": "tool", "tool": "Bash", "summary": "ran something", "at": 100 + i})
        if i % 50 == 0: ev(**{"$": "TurnChanged", "thread": t, "state": "completed", "at": 100 + i})

with open(home / "events.jsonl", "w", encoding="utf-8", errors="surrogatepass") as f:
    for l in lines: f.write(l + "\n")
print(f"{home}: {len(lines)} lines, {len(names)} event names")
