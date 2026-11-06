#!/usr/bin/env python3
"""Synthetic events.jsonl shaped like the real store by COUNTS only (no content): python3 gen_big.py OUT [SEED [SCALE]]   (SCALE 1.0 = the real counts; 0.05 for a quick one)
Counts from scripts/gpui/count_events.py on the store copy, as written in docs/gpui/PRESENTER-PORT.md:
 1,691,619 events; SettingSet 620,596 over 186,511 distinct keys (prefix sizes below, mean key 39.7 B, keys born steadily);
 ActivityLogged 301,120, MessagePosted 156,608 (27.1% entries), UsageSet 103,704, TurnChanged 97,769, the rest small kinds;
 12,446 ThreadCreated created steadily (1,898 by 25% of the events, 5,714 by 50%, 9,317 by 75%), 65 projects at the start.
Text sizes are tuned so the file lands near the real 1.48 GB. Text is random words from a pool, never anything real."""
import json, random, sys, bisect
out = sys.argv[1]; rnd = random.Random(int(sys.argv[2]) if len(sys.argv) > 2 else 7)
SC = float(sys.argv[3]) if len(sys.argv) > 3 else 1.0
sc = lambda n: max(1, int(n * SC))
N = sc(1_691_619); NTH = sc(12_446); NPROJ = min(65, max(3, sc(65))); T0 = 1_785_000_000; SPAN = 90 * 86400
C = {k: sc(v) for k, v in dict(setting=620_596, activity=301_120, message=156_608, usage=103_704, turn=97_769).items()}
NKEYS = sc(186_511)
PREFIX = [("fallback", 23_500), ("run", 20_800), ("summary", 18_900), ("turn", 16_700), ("account", 16_300), ("restart", 11_500), ("provider", 11_500), ("tok", 11_300)]
REST = ["liveness", "autosettle", "stopped", "launch", "told", "alert", "desk", "fab", "probe", "emb", "fold", "rule", "board", "split", "sch"]
OTHER = ["ToolOpen", "ToolShut", "AskOpened", "AskAnswered", "NoticePosted", "TaskSet", "LockChanged", "BotWoke", "RoutineFired", "ProjectSet", "WorktreeSet",
         "ThreadRenamed", "ThreadPinned", "QueueSet", "ReviewOp", "StepLogged", "TerminalOpened", "AlertSet", "SessionSet", "ModelSet"]
n_other = N - NPROJ - NTH - sum(C.values())
words = "board layout net rail footprint pad via trace schematic symbol resistor capacitor regulator decoupling clearance drill copper pour thermal connector header ground power signal clock reset sensor antenna stackup impedance routing placement silkscreen solder mask panel fiducial bracket enclosure mount tolerance datasheet package voltage current".split()
pool = [" ".join(rnd.choice(words) for _ in range(rnd.randint(25, 160))).capitalize() + "." for _ in range(3000)]
def text(k): return " ".join(rnd.choice(pool) for _ in range(k))
# thread creation positions: piecewise-linear inverse of the real cumulative curve
pts = [(0, 0), (.25, NTH * 1898 // 12446), (.5, NTH * 5714 // 12446), (.75, NTH * 9317 // 12446), (1, NTH)]
def pos_of(j):  # j-th thread (0-based) -> position in 0..1
    for (x0, y0), (x1, y1) in zip(pts, pts[1:]):
        if j < y1: return x0 + (x1 - x0) * (j - y0) / (y1 - y0)
    return 1.0
th_at = sorted(int(pos_of(j) * (N - NPROJ - NTH)) for j in range(NTH))
# the non-thread schedule, exact counts, shuffled
kinds = ["setting"] * C["setting"] + ["activity"] * C["activity"] + ["message"] * C["message"] + ["usage"] * C["usage"] + ["turn"] * C["turn"] + ["other"] * n_other
rnd.shuffle(kinds)
# setting keys: exact distinct count, born at random setting slots
keys = []
for pfx, n in PREFIX: keys += [f"{pfx}.{rnd.getrandbits(128):032x}" for _ in range(sc(n))]
rest = NKEYS - len(keys)
for i in range(rest): keys.append(f"{REST[i % len(REST)]}.{rnd.getrandbits(128):032x}")
rnd.shuffle(keys)
birth = set(rnd.sample(range(1, C["setting"]), NKEYS - 1)) | {0}
w = open(out, "w", buffering=1 << 20)
def put(o): w.write(json.dumps(o, separators=(",", ":")) + "\n")
projs = [f"p{T0}-{i}" for i in range(NPROJ)]
for i, p in enumerate(projs): put({"$": "ProjectCreated", "id": p, "title": f"synthetic {i}", "root": f"/tmp/synthetic/proj{i}", "at": T0})
ths = []; born = []; ki = 0; si = 0; mid = 0; ti = 0
def pick_thread():
    # heavy-tailed over the threads that exist (older threads busier), thread 0 holds ~3%
    if rnd.random() < 0.03: return ths[0]
    return ths[min(len(ths) - 1, int(len(ths) * (rnd.random() ** 1.7)))]
for i, kd in enumerate(kinds):
    while ti < NTH and th_at[ti] <= i:
        t = f"t{T0}-{1000 + len(ths)}"; ths.append(t)
        put({"$": "ThreadCreated", "id": t, "project": projs[len(ths) % NPROJ], "title": f"Thread {len(ths)}", "env": "local", "branch": "", "worktree": "", "provider": "claude", "model": "", "at": T0 + int(SPAN * i / N)}); ti += 1
    at = T0 + int(SPAN * i / N)
    if kd == "setting":
        if si in birth: k = keys[ki]; ki += 1; born.append(k)
        else: k = born[rnd.randrange(len(born))]
        si += 1
        put({"$": "SettingSet", "key": k, "value": json.dumps({"last": at, "n": si}) if k.startswith(("run", "turn")) else str(at), "at": at})
    elif kd == "message" and ths:
        mid += 1; role = "user" if rnd.random() < 0.2 else "assistant"
        put({"$": "MessagePosted", "thread": pick_thread(), "msg": f"m{at}-{mid}", "role": role, "text": text(rnd.randint(1, 2) if role == "user" else rnd.randint(2, 12)), "at": at})
    elif kd == "activity" and ths:
        mid += 1; put({"$": "ActivityLogged", "thread": pick_thread(), "act": f"g{at}-{mid}", "tone": rnd.choice(["tool", "tool", "tool", "info", "step"]), "kind": rnd.choice(["Bash", "Read", "Edit", "Grep"]), "summary": text(rnd.randint(1, 6)), "at": at})
    elif kd == "usage" and ths: put({"$": "UsageSet", "thread": pick_thread(), "used": 20000 + rnd.randint(0, 150000), "window": 0, "at": at})
    elif kd == "turn" and ths: put({"$": "TurnChanged", "thread": pick_thread(), "state": rnd.choice(["completed", "completed", "failed"]), "at": at})
    else: put({"$": rnd.choice(OTHER), "thread": ths[-1] if ths else "", "at": at})
while ti < NTH:
    t = f"t{T0}-{1000 + len(ths)}"; ths.append(t); put({"$": "ThreadCreated", "id": t, "project": projs[0], "title": "Thread", "env": "local", "branch": "", "worktree": "", "provider": "claude", "model": "", "at": T0 + SPAN}); ti += 1
put({"$": "SettingSet", "key": "restart.continue", "value": "off", "at": T0 + SPAN})
w.close()
