#!/usr/bin/env python3
"""gen.py N PROJECT_ROOT > events.jsonl : N synthetic events, seeded, unique ids. 60 projects, 3000 threads + the 20 soak threads
(ts000..ts019, project p1). Synthetic text only. Mix: 60% MessagePosted, 20% ActivityLogged, 20% SettingSet/TurnChanged."""
import sys, json, random
n = int(sys.argv[1]); root = sys.argv[2]; o = sys.stdout; t0 = 1785000000; rnd = random.Random(7)
words = "board layout net rail pad via trace plane stackup clearance footprint schematic sheet symbol pin label bus regulator crystal decoupling".split()
def text(k): return " ".join(rnd.choice(words) for _ in range(k))
o.write(json.dumps({"$":"ProjectCreated","id":"p1","title":"soak","root":root,"at":t0}) + "\n")
for p in range(2, 61): o.write(json.dumps({"$":"ProjectCreated","id":f"p{p}","title":f"proj {p}","root":f"/tmp/gen/p{p}","at":t0}) + "\n")
tids = [f"ts{i:03d}" for i in range(20)]
for i, t in enumerate(tids): o.write(json.dumps({"$":"ThreadCreated","id":t,"project":"p1","title":f"soak {i}","env":"local","branch":"","worktree":"","provider":"claude","model":"","at":t0+1+i}) + "\n")
threads = 3000
for t in range(threads): tids.append(f"g{t}"); o.write(json.dumps({"$":"ThreadCreated","id":f"g{t}","project":f"p{2 + t % 59}","title":f"Thread {t}","env":"local","branch":"","worktree":"","provider":"claude","model":"","at":t0+30+t}) + "\n")
done = 60 + 20 + threads
for i in range(n - done):
    t = tids[(i * 7919) % len(tids)]; k = i % 10; at = t0 + 100 + i // 5
    if k < 6: o.write(json.dumps({"$":"MessagePosted","thread":t,"msg":f"gm{i}","role":"user" if i % 2 else "assistant","text":text(18 + i % 40),"at":at}) + "\n")
    elif k < 8: o.write(json.dumps({"$":"ActivityLogged","thread":t,"act":f"ga{i}","tone":"tool","kind":"Bash","summary":"ls -la src/core | head","at":at}) + "\n")
    elif k == 8: o.write(json.dumps({"$":"SettingSet","key":f"gen.{i % 500}","value":text(3),"at":at}) + "\n")
    else: o.write(json.dumps({"$":"TurnChanged","thread":t,"state":"completed","at":at}) + "\n")
