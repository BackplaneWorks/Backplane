#!/usr/bin/env python3
"""gen_big.py OUT [LINES=1000000] [THREADS=6666]: a synthetic log (no real data) shaped like a busy home: user/assistant messages, tool acts,
turns, token usage, and the per-message housekeeping settings that grow with the log (stopped.<msg>, failed.<msg>, told.*)."""
import sys, json, random
out = sys.argv[1]; N = int(sys.argv[2]) if len(sys.argv) > 2 else 1_000_000; T = int(sys.argv[3]) if len(sys.argv) > 3 else 6666
POLICY = len(sys.argv) > 4 and sys.argv[4] == "policy"   # every thread carries the settings the policies read
random.seed(7)
at = 1_790_000_000
def w(f, o):
    f.write(json.dumps(o, separators=(',', ':')) + '\n')
with open(out, 'w') as f:
    n = 0
    for p in range(60):
        w(f, {"$": "ProjectCreated", "id": f"p{p}", "title": f"proj{p}", "root": f"/r/{p}", "at": at}); n += 1
    for t in range(T):
        w(f, {"$": "ThreadCreated", "id": f"t{t}", "project": f"p{t % 60}", "title": f"thread {t}", "env": "local", "branch": "", "worktree": "", "provider": "claude", "model": "", "at": at}); n += 1
    if POLICY:
        for t in range(T):
            th = f"t{t}"
            for k, v in [(f"autosettle.quiet.{th}", str(at)), (f"autosettle.output.{th}", f"a{t}"), (f"autosettle.audience.{th}", "person"), (f"autosettle.read.{th}", f"a{t}"),
                         (f"liveness.{th}", json.dumps({"last": at, "nudged": 0, "stalled": False, "reported": False, "reason": "agent output"})), (f"alert.{th}", f"done\tturn-{th}\tc1\t{at}.1\t")]:
                w(f, {"$": "SettingSet", "key": k, "value": v, "at": at}); n += 1
            if t % 5 == 0:
                w(f, {"$": "SettingSet", "key": f"bg.{th}", "value": "job1\tsleep 1\tx", "at": at}); w(f, {"$": "SettingSet", "key": f"launch.{th}", "value": f"1t{(t + 1) % T}", "at": at}); n += 2
            if t % 10 == 0:
                w(f, {"$": "SettingSet", "key": f"watch.{th}", "value": "w1\tpr", "at": at}); w(f, {"$": "SettingSet", "key": f"thread.parent.{th}", "value": f"t{(t + 7) % T}", "at": at}); n += 2
            if t % 20 == 0:
                w(f, {"$": "WakeSet", "id": f"w{t}", "by": th, "kind": "thread", "target": f"t{(t + 3) % T}", "events": ["done"], "once": True, "note": "", "at": at}); n += 1
        # a realistic task table (parent/child chains up to 4 deep, half finished) and open asks
        for i in range(2500):
            child = random.randrange(T); parent = (child + 1 + i % 5) % T
            w(f, {"$": "TaskDelegated", "task": f"k{i}", "parent": f"t{parent}", "child": f"t{child}", "depth": 1, "provider": "claude", "model": "", "at": at}); n += 1
            if i % 2 == 0:
                w(f, {"$": "TaskStateSet", "task": f"k{i}", "state": "done", "summary": "", "at": at}); n += 1
        for i in range(400):
            w(f, {"$": "AskOpened", "id": f"q{i}", "thread": f"t{random.randrange(T)}", "kind": "approval", "tool": "Bash", "detail": "rm x", "options": "", "at": at}); n += 1
        w(f, {"$": "SettingSet", "key": "settle.idle.minutes", "value": "15", "at": at}); n += 1
    m = 0
    while n < N:
        t = random.randrange(T); th = f"t{t}"; at += 1; m += 1
        evs = [{"$": "MessagePosted", "thread": th, "msg": f"m{m}", "role": "user", "text": "please do the thing " * 3, "at": at},
               {"$": "TurnChanged", "thread": th, "state": "running", "at": at},
               {"$": "SettingSet", "key": f"turn.from.{th}", "value": f"@m{m}", "at": at}]
        for k in range(random.randrange(1, 4)):
            evs.append({"$": "ActivityLogged", "thread": th, "act": f"g{m}-{k}", "tone": "tool", "kind": "Bash", "summary": "ls -la", "at": at})
            evs.append({"$": "MessagePosted", "thread": th, "msg": f"a{m}-{k}", "role": "assistant", "text": "working on it " * 5, "at": at})
            evs.append({"$": "UsageSet", "thread": th, "used": 1000 + k, "window": 200000, "at": at})
        evs += [{"$": "SettingSet", "key": f"stopped.m{m}", "value": "1", "at": at}, {"$": "SettingSet", "key": f"failed.m{m}", "value": "", "at": at},
                {"$": "TurnUsage", "thread": th, "provider": "claude", "model": "opus", "key": f"k{m}", "input": 10, "cread": 20, "cwrite": 30, "output": 40, "reasoning": 0, "cost": 5, "known": "icwotc", "basis": "b", "at": at},
                {"$": "TurnChanged", "thread": th, "state": "completed", "at": at},
                {"$": "SettingSet", "key": f"told.k{m}", "value": json.dumps({"key": f"turn-{th}", "text": "x" * 40}), "at": at}]
        for e in evs:
            if n >= N: break
            w(f, e); n += 1
print(n)
