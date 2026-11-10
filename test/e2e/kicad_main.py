#!/usr/bin/env python3
"""Judge of scripts/desktop-kicad-main.sh (Design files on main, docs/git.md).
  kicad_main.py lock BOARD                    the board's lock name (kicad/canonical.rs doc_lock)
  kicad_main.py held EVENTS LOCK NOTE          exit 0 once a party with NOTE holds LOCK
  kicad_main.py judge RIG_DIR LOCK             every check, a line each; exit 1 on any failure
"""
import json, os, re, subprocess, sys


def digest(p):
    def h(m, acc):
        for c in p:
            acc = (acc * m + ord(c)) % (1 << 32)
        return acc
    return "%08x%08x" % (h(31, 7), h(131, 3))


def lock(doc):
    f = doc.rsplit("/", 1)[-1]
    stem, _, ext = f.rpartition(".")
    kind = ext[len("kicad_"):] if ext.startswith("kicad_") else ext
    stem = "".join(c if c.isascii() and (c.isalnum() or c in "-_") else "-" for c in stem)[:40]
    return ("kicad-%s-%s-%s" % (stem, kind, digest(doc)[:8]))[:64]


def events(path):
    out = []
    for l in open(path, encoding="utf-8"):
        try:
            out.append(json.loads(l))
        except ValueError:
            pass
    return out


def holder(e):
    lk = e.get("lock")
    if isinstance(lk, str):
        try:
            lk = json.loads(lk)
        except ValueError:
            lk = {}
    return (lk or {}).get("holder") or {}


def git(d, *a):
    return subprocess.run(["git", "-C", d, *a], capture_output=True, text=True).stdout.strip()


def judge(r, name):
    ev = events(os.path.join(r, "home/events.jsonl"))
    proj = os.path.join(r, "proj")
    threads = [e["id"] for e in ev if e.get("$") == "ThreadCreated"]
    ok = True

    def check(c, what, detail=""):
        nonlocal ok
        print(("PASS " if c else "FAIL ") + what + ("" if c or not detail else "  -- " + str(detail)[:400]))
        ok = ok and bool(c)

    check(len(threads) >= 4, "four threads were made", threads)
    if len(threads) < 4:
        return 1
    a, b, c, d = threads[:4]
    st = lambda th, k: [e for e in ev if e.get("$") == "SettingSet" and e.get("key") == "%s.%s" % (k, th)]
    wt = {e["id"]: e["worktree"] for e in ev if e.get("$") == "WorktreeSet"}
    acts = lambda th: [e.get("summary", "") for e in ev if e.get("$") == "ActivityLogged" and e.get("thread") == th]
    says = lambda th: [e.get("text", "") for e in ev if e.get("$") == "MessagePosted" and e.get("thread") == th]
    # 1. working directories
    check(all(st(t, "kicad.main") and st(t, "kicad.main")[-1]["value"] == "on" for t in (a, b, c, d)), "each thread works in the main checkout for design files (kicad.main.<thread> on)")
    w = wt.get(a, "")
    check(w.startswith(proj + "/.backplane/worktrees/"), "thread A still has its own code worktree", w)
    check(w and not os.path.exists(os.path.join(w, "pic_programmer.kicad_pcb")) and not os.path.exists(os.path.join(w, "pic_programmer.kicad_sch")), "A's worktree carries no board or schematic")
    check(w and os.path.isfile(os.path.join(w, "firmware/src/main.c")), "A's worktree has the firmware")
    check(w and git(w, "status", "--porcelain") == "", "A's worktree is clean in git's eyes", git(w, "status", "--porcelain"))
    # 2. design commits to main, pushed
    log = git(proj, "log", "--format=%s", "main")
    check(re.search(r"^design: .*\(%s\)$" % re.escape(a), log, re.M), "A's turn end committed the design to main", log)
    files = git(proj, "show", "--name-only", "--format=", git(proj, "log", "-1", "--format=%H", "--grep=(%s)" % a, "main"))
    check("pic_programmer.kicad_pcb" in files.split("\n") and "README.md" not in files, "that commit has the board and not the code", files)
    check(git(proj, "rev-parse", "main") == git(os.path.join(r, "origin.git"), "rev-parse", "main"), "main was pushed to its upstream")
    moved = git(proj, "diff", "--stat", git(proj, "rev-list", "--max-parents=0", "main"), "main", "--", "pic_programmer.kicad_pcb")
    check(moved, "the footprint A moved is in main's board", moved)
    check("M README.md" in git(proj, "status", "--porcelain").split("\n") or " M README.md" in git(proj, "status", "--porcelain"), "A's code edit in the main checkout was not swept into a design commit", git(proj, "status", "--porcelain"))
    check(any(s.startswith("Committed ") and "pushed to origin/main" in s for s in acts(a)), "A's thread says what it committed and pushed", acts(a))
    # 3. Open in KiCad opened main's board, from the viewer
    check(any(s == "Opened %s/pic_programmer.kicad_pcb in KiCad" % proj for s in acts(a)), "Open in KiCad opened the main checkout's board the viewer showed", [s for s in acts(a) if "KiCad" in s])
    # 4. one writer per document
    lc = [e for e in ev if e.get("$") == "LockChanged" and e.get("name") == name]
    hs = [holder(e).get("id", "") for e in lc]
    check(b in hs, "B held the board's lock", hs)
    i = hs.index(b) if b in hs else -1
    check(i >= 0 and "" in hs[i:], "B's lock was released (its turn ended)", hs)
    refused = [t for t in says(c) if "files_write" in t]
    check(refused and ("being edited by " + b) in refused[-1] and name in refused[-1], "C's write of the board was refused, naming B and the lock", refused)
    check("(clobbered by C)" not in open(os.path.join(proj, "pic_programmer.kicad_pcb"), encoding="utf-8").read(), "the board is not C's text")
    # 5. a crash while holding the lock
    j = max([k for k, h in enumerate(hs) if h == d], default=-1)
    check(j >= 0, "D held the board's lock when the hub was killed", hs)
    check(j >= 0 and "" in hs[j:], "after the restart D's lock was cleared", hs[j:] if j >= 0 else hs)
    # (the restart resumes D's turn; its end lets the lock go, or the idle grace does, saying so)
    return 0 if ok else 1


if __name__ == "__main__":
    cmd = sys.argv[1]
    if cmd == "lock":
        print(lock(sys.argv[2]))
    elif cmd == "held":
        ev = events(sys.argv[2])
        lc = [e for e in ev if e.get("$") == "LockChanged" and e.get("name") == sys.argv[3]]
        sys.exit(0 if lc and holder(lc[-1]).get("note") == sys.argv[4] else 1)
    else:
        sys.exit(judge(sys.argv[2], sys.argv[3]))
