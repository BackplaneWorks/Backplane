#!/usr/bin/env python3
"""The Boat hub cutover's carry (docs/BOAT-HUB.md). The Rust hub does not sync an uberbot between machines yet (`uber.sync` answers 501), so
Bub moves whole, as the one owner: everything in the desktop's log that is not a project's moves to the Boat, ids kept.

Reads a COPY of the desktop's events.jsonl; writes, in OUT:

  carry.jsonl   the lines to append to the Boat hub's log while it is stopped: every thread outside a project (Bub's conversation, its run,
                coordination and routine threads, the other bots' threads) with all their lines; bots (the uberbot too: UberSet kept), rooms,
                hooks, memories, routines and their slots, mail, triage, the push book, and the settings that are not a project thread's.
                Left out, because they are this machine's or would point at it: machine.*, peer.*, uber.* (the lease and links name the desktop),
                UberOp, project and probe settings, locks, account bindings and agent sessions (the Boat signs in with its own), worktrees,
                devices (phones pair with the Boat themselves), restart/liveness state, tasks of project threads.
  retire.json   the RPCs that switch the same off on the desktop after the carry: the uberbot demoted, then bots.delete per bot,
                bots.room_delete per room, bots.hook_revoke per hook. The desktop's log keeps their history; nothing is deleted from disk.
  restore.json  the desktop's own last BotSet/RoomSet/HookSet/UberSet lines, to commit again there if the cutover is rolled back.
  files.txt     the files that go with them (bots' folders, hook secrets, ring notes and key, mail, message images).
  summary.txt   what was picked.

usage: boat-hub-carry.py LOG OUT [--bots-root DIR]
"""
import json, os, sys

SKIP_T = {"UberOp", "LockChanged", "DeviceRegistered", "DevicePrefs", "DeviceDropped", "PushCleared", "SessionSet", "WorktreeSet",
          "PeerSet", "ProjectCreated", "ProjectRemoved", "TaskDelegated", "TaskDelivered", "TaskStateSet"}
SKIP_K = ("machine.", "peer.", "uber.", "projects.", "probe:", "lock.", "locks.", "accounts.", "account.", "allowance.", "limit.", "session.",
          "tz.checked", "restart.", "liveness.", "hub.", "update.", "sandbox.", "far.")

def main():
    a = sys.argv[1:]
    if len(a) < 2: print(__doc__, file=sys.stderr); sys.exit(2)
    log, out = a[0], a[1]
    bots_root = a[a.index("--bots-root") + 1] if "--bots-root" in a else os.path.expanduser("~/.backplane/bots")
    ev = []
    for l in open(log, encoding="utf-8", errors="replace"):
        try: ev.append(json.loads(l))
        except Exception: pass
    proj_threads, projects = set(), set()
    bots, rooms, hooks, last = {}, {}, {}, {}
    uber = None
    for e in ev:
        t = e.get("$")
        if t in ("ThreadCreated", "ThreadForked") and e.get("project"): proj_threads.add(e.get("id"))
        elif t == "ProjectCreated": projects.add(e.get("id"))
        elif t == "BotSet": bots[e.get("id")] = e; last[("bot", e.get("id"))] = e
        elif t == "BotDeleted": bots.pop(e.get("id"), None)
        elif t == "RoomSet": rooms[e.get("id")] = e; last[("room", e.get("id"))] = e
        elif t == "RoomDeleted": rooms.pop(e.get("id"), None)
        elif t == "HookSet":   # a revoke is a HookSet with revoked true (bots.hook_revoke, which also removes its secret files)
            if e.get("revoked"): hooks.pop(e.get("id"), None)
            else: hooks[e.get("id")] = e; last[("hook", e.get("id"))] = e
        elif t == "UberSet": uber = e
    desk = proj_threads | projects
    def subject(e):
        for k in ("thread", "room", "bot", "hook"):
            v = e.get(k)
            if isinstance(v, str) and v: return v
        v = e.get("id"); return v if isinstance(v, str) else ""
    def desk_key(k):
        return any(p in desk for p in k.replace(":", ".").split(".")) or any(d in k for d in desk if len(d) > 8 and d in k)
    picked, kinds = [], {}
    for e in ev:
        t = e.get("$")
        if t in SKIP_T: continue
        if t == "SettingSet":
            k = e.get("key", "")
            if k.startswith(SKIP_K) or desk_key(k): continue
        else:
            s = subject(e)
            if not s or s in desk: continue
            # a line of a project thread that names it under another field (a fork's source, a launch's child)
            if any(isinstance(e.get(f), str) and e.get(f) in desk for f in ("parent", "by", "from_thread", "source")): continue
        picked.append(e); kinds[t] = kinds.get(t, 0) + 1

    os.makedirs(out, exist_ok=True)
    with open(os.path.join(out, "carry.jsonl"), "w") as f:
        for e in picked: f.write(json.dumps(e, separators=(",", ":")) + "\n")
    retire = ([{"m": "bots.uber", "p": {"bot": ""}}] if uber and uber.get("bot") else []) \
           + [{"m": "bots.delete", "p": {"bot": b}} for b in bots] + [{"m": "bots.room_delete", "p": {"room": r}} for r in rooms] \
           + [{"m": "bots.hook_revoke", "p": {"hook": h}} for h in hooks]
    json.dump(retire, open(os.path.join(out, "retire.json"), "w"), indent=1)
    json.dump([last[k] for k in last if (k[0] == "bot" and k[1] in bots) or (k[0] == "room" and k[1] in rooms) or (k[0] == "hook" and k[1] in hooks)]
              + ([uber] if uber else []), open(os.path.join(out, "restore.json"), "w"), indent=1)
    files = [os.path.join(bots_root, b) for b in bots if os.path.isdir(os.path.join(bots_root, b))]
    for h in hooks: files += [f"secrets/hooks/{h}", f"secrets/hooks/{h}.token"]
    files += ["ring", "secrets/ring.key", "mail", "message-images"]
    open(os.path.join(out, "files.txt"), "w").write("\n".join(files) + "\n")
    with open(os.path.join(out, "summary.txt"), "w") as f:
        f.write(f"uberbot {uber.get('bot') if uber else '-'} (moves whole)\n")
        for b, x in bots.items(): f.write(f"bot {b} {x.get('name')} thread {x.get('thread')}\n")
        for r, x in rooms.items(): f.write(f"room {r} {x.get('name')}\n")
        for h, x in hooks.items(): f.write(f"hook {h} {x.get('name')} bot {x.get('bot')}\n")
        f.write(f"left on the desktop: {len(proj_threads)} project threads, {len(projects)} projects\n")
        f.write(f"{len(picked)} lines: " + ", ".join(f"{k} {v}" for k, v in sorted(kinds.items(), key=lambda kv: -kv[1])[:20]) + "\n")
    print(open(os.path.join(out, "summary.txt")).read(), end="")

main()
