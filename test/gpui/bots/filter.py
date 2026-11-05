#!/usr/bin/env python3
"""Keep the events a bots scenario is about (bot threads, rooms, memory, wakes) from a hub's events.jsonl, without clocks.
usage: filter.py EVENTS.jsonl > fixture.jsonl   (the Rust test `js_parity` applies the same filter to its own log)"""
import json, sys
KEEP = {"BotSet", "BotDeleted", "RoomSet", "RoomDeleted", "RoomPosted", "BotWoke", "MemorySaved", "MemoryForgot", "SpaceSet", "WorktreeSet", "UberSet",
        "RoutineSet", "RoutinePlan", "RoutineFired", "RoutineSlot", "RoutineDone", "RoutineDeleted", "Reminded"}
ev = [json.loads(l) for l in open(sys.argv[1]) if l.strip()]
bot_threads = {e["thread"] for e in ev if e["$"] == "BotSet" and e.get("thread")}
for e in ev:
    k = e["$"]
    if k == "ThreadCreated": ok = e.get("project") == ""
    elif k in ("MessagePosted", "MessageFrom"): ok = e.get("thread") in bot_threads and (k == "MessageFrom" or e.get("role") == "user")
    elif k == "WorktreeSet": ok = e["id"] in bot_threads
    else: ok = k in KEEP
    if not ok: continue
    e = {a: ("<T>" if a in ("once", "slot") and b else b) for a, b in e.items() if a != "at"}  # instants and wall clocks are the run's own
    print(json.dumps(e, separators=(",", ":"), ensure_ascii=False))
