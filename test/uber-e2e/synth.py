#!/usr/bin/python3
"""synth.py DST.jsonl: a synthetic stand-in for Bub's history (no real data): an uberbot with a persona, a thread, memories and routines. Used by e2e.mjs when no private fixture is given."""
import json, sys
t = 1790000000
ev = [
 {"$":"ThreadCreated","id":"tsyn0","project":"","title":"Older","env":"local","branch":"","worktree":"","provider":"claude","model":"","at":t-100},
 {"$":"BotSet","id":"bsyn0","name":"Older","thread":"tsyn0","look":1,"persona":"An older bot, created before the uberbot.","at":t-99},
 {"$":"ThreadCreated","id":"tsyn1","project":"","title":"Bub","env":"local","branch":"","worktree":"","provider":"claude","model":"","at":t},
 {"$":"BotSet","id":"bsyn1","name":"Bub","thread":"tsyn1","look":3,"persona":"You are Bub, the coordinator. You do not do the work yourself; you route it to other threads and bots, and you keep short notes. Stay brief and dry.","at":t+1},
 {"$":"WorktreeSet","id":"tsyn1","branch":"","worktree":"/tmp/synth-bub","at":t+2},
 {"$":"UberSet","bot":"bsyn1","uid":"usyn-1","at":t+3},
]
for i, (k, kind) in enumerate([("owner-name","user"),("quiet-hours","rule"),("deploy-flow","fact"),("mail-triage","rule")]):
    ev.append({"$":"MemorySaved","bot":"bsyn1","key":k,"kind":kind,"text":f"synthetic memory {i}","tags":"synth","at":t+10+i})
for i, (cron, on) in enumerate([("*/30 * * * *", True), ("0 9 * * 1-5", True), ("0 3 * * *", False)]):
    rid = f"rsyn{i}"
    ev.append({"$":"RoutineSet","id":rid,"bot":"bsyn1","name":f"synthetic routine {i}","cron":cron,"prompt":f"synthetic routine prompt {i}","on":on,"at":t+20+i})
ev.append({"$":"SpaceSet","bot":"bsyn1","spec":json.dumps({"title":"Bub","blocks":[{"type":"heading","text":"Synthetic space"},{"type":"stat","label":"Open","value":"3"},{"type":"button","label":"Ping","action":"ping"}]}),"at":t+30})
with open(sys.argv[1], 'w') as o:
    for e in ev: o.write(json.dumps(e, separators=(',',':')) + '\n')
print(len(ev), 'events')
