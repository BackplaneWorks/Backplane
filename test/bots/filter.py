#!/usr/bin/env python3
"""Keep the events a bots scenario is about (bot threads, rooms, memory, wakes) from a hub's events.jsonl, without clocks.
usage: filter.py EVENTS.jsonl [MAILDIR] > fixture.jsonl   (a mail journal directory adds one MailFile pseudo-event per file)   (the Rust test `js_parity` applies the same filter to its own log)"""
import json, re, sys
KEEP = {"BotSet", "BotDeleted", "RoomSet", "RoomDeleted", "RoomPosted", "BotWoke", "MemorySaved", "MemoryForgot", "SpaceSet", "WorktreeSet", "UberSet",
        "ConvSet", "MailPosted", "RoutineSet", "RoutinePlan", "RoutineFired", "RoutineSlot", "RoutineDone", "RoutineDeleted", "Reminded", "UberOp"}
ev = [json.loads(l) for l in open(sys.argv[1]) if l.strip()]
bot_threads = {e["thread"] for e in ev if e["$"] == "BotSet" and e.get("thread")}
me = next((e["value"] for e in reversed(ev) if e["$"] == "SettingSet" and e.get("key") == "machine.id"), "")
uid = next((e["uid"] for e in ev if e["$"] == "UberSet" and e.get("uid")), "")
def norm(s):  # this run's machine id and uberbot id, and the seconds of a stamp, are its own
    if me: s = s.replace(me, "<me>")
    if uid: s = s.replace(uid, "<uid>")
    s = re.sub(r"\b\d{10}\.(\d{6})\.<me>", r"<S>.<c>.<me>", s)
    return re.sub(r'once\\":[1-9][0-9]*', 'once\\":<T>', s)  # a one-time routine's instant inside an op body
def routed(e):  # what the JS hub's routing adds after a mail is filed (triage, escalations, claims): slice 3, not compared
    s = json.dumps(e, ensure_ascii=False)
    return ("triage-" in s or (e["$"] == "ThreadCreated" and (e.get("id", "").startswith("run") or e.get("id", "").startswith("mailcoord-"))) or (e["$"] in ("ConvSet", "MailPosted") and str(e.get("id", "")).startswith("pair:")) or (e["$"] == "ConvSet" and e.get("id", "").startswith("pair:"))
        or (e["$"] == "MailPosted" and e.get("conv", "").startswith("pair:")) or (e["$"] == "MessageFrom" and e.get("kind") == "escalation")
        or (e["$"] == "SettingSet" and e.get("key", "").startswith("owner.mail.") and e.get("value", "x") == ""))
for e in ev:
    k = e["$"]
    if routed(e): continue
    if k == "ThreadCreated": ok = e.get("project") == ""
    elif k in ("MessagePosted", "MessageFrom"): ok = e.get("thread") in bot_threads and (k == "MessageFrom" or e.get("role") == "user")
    elif k == "WorktreeSet": ok = e["id"] in bot_threads
    elif k == "SettingSet": ok = e.get("key", "").startswith("owner.mail.")
    else: ok = k in KEEP
    if not ok: continue
    e = {a: ("<T>" if a in ("once", "slot") and b else b) for a, b in e.items() if a != "at"}  # instants and wall clocks are the run's own
    print(norm(json.dumps(e, separators=(",", ":"), ensure_ascii=False)))

import os
if len(sys.argv) > 2 and os.path.isdir(sys.argv[2]):
    for name in sorted(os.listdir(sys.argv[2])):
        if name.endswith(".idx") or name.startswith("pair%3A"): continue  # the hash index is rebuilt by whoever reads it; pair journals are triage's
        lines = [l.rstrip("\n") for l in open(os.path.join(sys.argv[2], name), encoding="utf-8")]
        print(norm(json.dumps({"$": "MailFile", "name": name, "lines": lines}, separators=(",", ":"), ensure_ascii=False)))
