# The rig's log: projects and threads for every sidebar row (pages past 48, quiet and older projects, a tree, archived, statuses).
import json, sys, time, os
R = sys.argv[1]; N = int(time.time()); D = 86400
ev = [{"$":"SettingSet","key":k,"value":v,"at":N-40*D} for k, v in [("restart.continue","off"),("onboard.done","1"),("onboard.uber","1")]]
def proj(pid, title, at):
    os.makedirs(f"{R}/proj/{pid}", exist_ok=True); ev.append({"$":"ProjectCreated","id":pid,"title":title,"root":f"{R}/proj/{pid}","at":at})
def th(tid, pid, title, at): ev.append({"$":"ThreadCreated","id":tid,"project":pid,"title":title,"env":"local","branch":"","worktree":"","provider":"claude","model":"","at":at})
proj("p1", "ecc", N-10*D)
th("t1", "p1", "Board work", N-9*D); th("t2", "p1", "Second build", N-8*D); th("t3", "p1", "Footprint review", N-7*D)
th("t4", "p1", "Old notes", N-6*D); ev.append({"$":"ThreadArchived","id":"t4","archived":True,"at":N-5*D})
th("t5", "p1", "Child of board", N-5*D); ev.append({"$":"SettingSet","key":"thread.parent.t5","value":"t1","at":N-5*D}); ev.append({"$":"SettingSet","key":"thread.tree.t1","value":"1","at":N-5*D})
th("t6", "p1", "Needs approval", N-4*D); th("t7", "p1", "Has a question", N-4*D+1); th("t8", "p1", "Turn failed", N-4*D+2); th("t9", "p1", "Settled one", N-4*D+3)
ev.append({"$":"ThreadSettled","id":"t9","manual":True,"at":N-3*D})
ev.append({"$":"TurnChanged","thread":"t8","state":"failed","at":N-3600})
ev.append({"$":"AskOpened","id":"a1","thread":"t6","kind":"approval","tool":"Bash","detail":"rm -rf build","options":"","at":N-600})
ev.append({"$":"AskOpened","id":"a2","thread":"t7","kind":"question","tool":"AskUserQuestion","detail":"USB-C or micro-USB?","options":"","at":N-300})
proj("p2", "bulk", N-20*D)
for i in range(60): th(f"b{i:02d}", "p2", f"Bulk thread {i:02d}", N-20*D+i*60)
proj("p3", "lamp", N-30*D); th("q1", "p3", "Lamp shade", N-30*D); ev.append({"$":"ThreadSettled","id":"q1","manual":True,"at":N-1*D})
proj("p4", "radio", N-60*D); th("o1", "p4", "Radio tuner", N-60*D); ev.append({"$":"ThreadSettled","id":"o1","manual":True,"at":N-50*D})
proj("p5", "clock", N-70*D); th("o2", "p5", "Clock face", N-70*D); ev.append({"$":"ThreadSettled","id":"o2","manual":True,"at":N-60*D})
ev.sort(key=lambda e: e["at"])
open(R + "/home/events.jsonl", "w").write("".join(json.dumps(e, separators=(",", ":")) + "\n" for e in ev))
