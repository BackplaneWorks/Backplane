#!/usr/bin/env python3
"""The Boat hub cutover's carry (docs/BOAT-HUB.md): what moves from the desktop's log to the Boat hub's, besides the uberbot (which moves by
its lease and sync). Reads a COPY of the desktop's events.jsonl; writes, in OUT:

  carry.jsonl   the lines to append to the Boat hub's log while it is stopped: every live bot but the uberbot, with its thread, messages,
                memories, routines and its settings; every live room with its posts; every live hook (the ring's) with its settings. Ids are kept
                (so the ring's notes, links and the app's retries stay valid); the desktop uberbot's id is replaced by the Boat replica's
                (--uber-to) wherever it is named (room members, bot.parent.*).
  retire.json   the RPCs that switch the same things off on the desktop after the carry: bots.delete per bot, bots.room_delete per room,
                bots.hook_revoke per hook. The desktop's log keeps their history; nothing is deleted from disk.
  restore.json  the RPC-free way back: the desktop's own last BotSet/RoomSet/HookSet lines, to re-commit if the cutover is rolled back.
  files.txt     the files that go with them, relative to the desktop home or ~ (bots' folders, hook secrets, ring notes and key).
  summary.txt   what was picked.

usage: boat-hub-carry.py LOG OUT --uber-to BOT_ID_ON_THE_BOAT [--bots-root DIR]
"""
import json, os, sys

def main():
    a = sys.argv[1:]
    if len(a) < 2 or "--uber-to" not in a: print(__doc__, file=sys.stderr); sys.exit(2)
    log, out = a[0], a[1]; uber_to = a[a.index("--uber-to") + 1]
    bots_root = a[a.index("--bots-root") + 1] if "--bots-root" in a else os.path.expanduser("~/.backplane/bots")
    lines = open(log, encoding="utf-8", errors="replace").read().splitlines()
    ev = []
    for l in lines:
        try: ev.append(json.loads(l))
        except Exception: ev.append(None)

    # the live state at the log's end
    bots, rooms, hooks, last = {}, {}, {}, {}
    uber = ""
    for e in ev:
        if not isinstance(e, dict): continue
        t = e.get("$")
        if t == "BotSet": bots[e.get("id")] = e; last[("bot", e.get("id"))] = e
        elif t == "BotDeleted": bots.pop(e.get("id"), None)
        elif t == "RoomSet": rooms[e.get("id")] = e; last[("room", e.get("id"))] = e
        elif t == "RoomDeleted": rooms.pop(e.get("id"), None)
        elif t == "HookSet":   # a revoke is a HookSet with revoked true (bots.hook_revoke, which also removes its secret files)
            if e.get("revoked"): hooks.pop(e.get("id"), None)
            else: hooks[e.get("id")] = e; last[("hook", e.get("id"))] = e
        elif t == "UberSet": uber = e.get("bot") or ""
    if not uber: sys.exit("no uberbot in this log")
    moved_bots = {b: x for b, x in bots.items() if b != uber}
    threads = {x.get("thread") for x in moved_bots.values() if x.get("thread")}
    ids = set(moved_bots) | threads | set(rooms) | set(hooks)

    SKIP = {"UberOp", "UberSet", "MailPosted", "MailMarked"}   # the uberbot's own replication and mailroom stay per machine
    def subject(e):
        for k in ("thread", "room", "bot", "hook"):
            v = e.get(k)
            if isinstance(v, str) and v: return v
        return e.get("id") if isinstance(e.get("id"), str) else ""
    def remap(x):
        if isinstance(x, str): return x.replace(uber, uber_to) if uber in x else x
        if isinstance(x, list): return [remap(v) for v in x]
        if isinstance(x, dict): return {k: remap(v) for k, v in x.items()}
        return x
    picked, kinds = [], {}
    for e in ev:
        if not isinstance(e, dict) or e.get("$") in SKIP: continue
        t = e.get("$")
        if t == "SettingSet":
            k = e.get("key", "")
            if not any(i and i in k for i in ids): continue
        elif subject(e) not in ids: continue
        picked.append(remap(e)); kinds[t] = kinds.get(t, 0) + 1

    os.makedirs(out, exist_ok=True)
    with open(os.path.join(out, "carry.jsonl"), "w") as f:
        for e in picked: f.write(json.dumps(e, separators=(",", ":")) + "\n")
    retire = [{"m": "bots.delete", "p": {"bot": b}} for b in moved_bots] + [{"m": "bots.room_delete", "p": {"room": r}} for r in rooms] \
           + [{"m": "bots.hook_revoke", "p": {"hook": h}} for h in hooks]
    json.dump(retire, open(os.path.join(out, "retire.json"), "w"), indent=1)
    json.dump([last[k] for k in last if (k[0] == "bot" and k[1] in moved_bots) or (k[0] == "room" and k[1] in rooms) or (k[0] == "hook" and k[1] in hooks)],
              open(os.path.join(out, "restore.json"), "w"), indent=1)
    files = [os.path.join(bots_root, b) for b in moved_bots if os.path.isdir(os.path.join(bots_root, b))]
    for h in hooks: files += [f"secrets/hooks/{h}", f"secrets/hooks/{h}.token"]
    files += ["ring", "secrets/ring.key"]
    open(os.path.join(out, "files.txt"), "w").write("\n".join(files) + "\n")
    with open(os.path.join(out, "summary.txt"), "w") as f:
        f.write(f"uberbot {uber} -> {uber_to} (moves by lease; not carried)\n")
        for b, x in moved_bots.items(): f.write(f"bot {b} {x.get('name')} thread {x.get('thread')}\n")
        for r, x in rooms.items(): f.write(f"room {r} {x.get('name')}\n")
        for h, x in hooks.items(): f.write(f"hook {h} {x.get('name')} bot {x.get('bot')}\n")
        f.write(f"{len(picked)} lines: " + ", ".join(f"{k} {v}" for k, v in sorted(kinds.items(), key=lambda kv: -kv[1])) + "\n")
    print(open(os.path.join(out, "summary.txt")).read(), end="")

main()
