# The rig's log for /give: a project with a thread that did work (an edit to a board, a note written, a request and its reply), two others to give to, and a bot.
import json, sys, time, os
R = sys.argv[1]; N = int(time.time()); H = 3600
ev = [{"$":"SettingSet","key":k,"value":v,"at":N-9*H} for k, v in [("restart.continue","off"),("onboard.done","1"),("onboard.uber","1")]]
root = f"{R}/proj/p1"
os.makedirs(f"{root}/boards/main", exist_ok=True); os.makedirs(f"{root}/notes", exist_ok=True)
open(f"{root}/boards/main/main.kicad_pcb", "w").write("(kicad_pcb (version 20240108) (generator pcbnew))\n"); open(f"{root}/notes/plan.md", "w").write("# Plan\nRoute the 3V3 rail.\n")
ev.append({"$":"ProjectCreated","id":"p1","title":"boards","root":root,"at":N-8*H})
def th(tid, title, at, pid="p1"): ev.append({"$":"ThreadCreated","id":tid,"project":pid,"title":title,"env":"local","branch":"","worktree":"","provider":"claude","model":"","at":at})
th("t1", "Power rail", N-7*H); th("t2", "Review notes", N-6*H); th("t3", "Busy build", N-5*H)
a = N-4*H
ev += [{"$":"MessagePosted","thread":"t1","msg":"m1","role":"user","text":"Route the 3V3 rail on main.kicad_pcb and note the plan","at":a},
       {"$":"ActivityLogged","thread":"t1","act":"a1","tone":"tool","kind":"Edit","summary":f"{root}/boards/main/main.kicad_pcb","at":a+5},
       {"$":"ActivityLogged","thread":"t1","act":"a2","tone":"tool","kind":"Write","summary":f"{root}/notes/plan.md","at":a+8},
       {"$":"MessagePosted","thread":"t1","msg":"m2","role":"assistant","text":"Routed the 3V3 rail and wrote the plan; DRC is clean.","at":a+20},
       {"$":"TurnChanged","thread":"t1","state":"completed","at":a+21}]
th("tb1", "Ada", N-3*H, "")
ev.append({"$":"BotSet","id":"b1","name":"Ada","thread":"tb1","look":5,"persona":"Ada the stand-in bot","at":N-3*H})
ev.append({"$":"RoomSet","id":"r1","name":"Design chat","members":"b1\nme","at":N-3*H})
os.makedirs(f"{root}/.claude/skills/pcb-review",exist_ok=True)
open(f"{root}/.claude/skills/pcb-review/SKILL.md","w").write("---\nname: pcb-review\ndescription: Review a board before the fab\n---\nCheck DRC.\n")
ev.sort(key=lambda e: e["at"])
open(R + "/home/events.jsonl", "w").write("".join(json.dumps(e, separators=(",", ":")) + "\n" for e in ev))
