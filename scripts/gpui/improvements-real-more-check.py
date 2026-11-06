#!/usr/bin/env python3
"""Checks and the scratch landing for scripts/gpui/improvements-real-more.sh. usage: MODE OUTDIR RUNDIR ROOT
edit mode runs the scratch copy of the land.sh flow, in the scratch repository only (every git call is `git -C <scratch>`)."""
import collections, json, os, subprocess, sys

mode, out, run, root = sys.argv[1:5]
P = f"{run}/proj/backplane-gpui"
fails = []
def check(ok, what, detail=""):
    print(("ok   " if ok else "FAIL ") + what + ((" -- " + str(detail)[:300]) if detail and not ok else ""))
    if not ok: fails.append(what)
def sh(args, cwd, env=None, check_rc=False):
    # a clean git environment: a job may inject GIT_CONFIG_* (hooks off), which would silently switch the scratch guard off
    e = {k: v for k, v in os.environ.items() if not k.startswith("GIT_CONFIG") and k not in ("GIT_DIR", "GIT_WORK_TREE", "GIT_INDEX_FILE")}; e.update(env or {})
    r = subprocess.run(args, cwd=cwd, env=e, capture_output=True, text=True)
    return r.returncode, (r.stdout + r.stderr).strip()

events = []
for l in open(f"{out}/events.jsonl", errors="replace"):
    try: events.append(json.loads(l))
    except ValueError: pass
metrics = []
for l in open(f"{out}/client.log", errors="replace"):
    if l.startswith("GPUI_METRIC "):
        try: metrics.append(json.loads(l[12:]))
        except ValueError: pass
states = [m for m in metrics if m.get("event") == "improvements"]
th = [e for e in events if e.get("$") == "ThreadCreated" and str(e.get("id", "")).startswith("improvement-")]
tid = th[0]["id"] if th else ""
print("thread:", tid, "events:", dict(collections.Counter(e.get("$") for e in events if e.get("thread") == tid or e.get("id") == tid)))
assistant = [e.get("text", "") for e in events if e.get("thread") == tid and e.get("$") == "MessagePosted" and e.get("role") == "assistant"]
usage = [e for e in events if e.get("thread") == tid and e.get("$") == "TurnUsage"]
turns = [e.get("state") for e in events if e.get("$") == "TurnChanged" and e.get("thread") == tid]
check(len(th) == 1, "one improvement thread", [e.get("id") for e in th])
check(bool(usage) and all(str(u.get("model", "")).startswith("claude-") for u in usage), "the turn ran on a real model", [u.get("model") for u in usage])
check(turns[-1:] == ["completed"], "the turn completed", turns)
print("usage:", [{k: u.get(k) for k in ("model", "input", "cread", "cwrite", "output", "cost")} for u in usage])

if mode == "writer":
    w = states[-1] if states else {}
    check(bool(w.get("writer")) and w["writer"].get("created") and w["writer"].get("started"), "the writer thread was created and started", w)
    body = assistant[-1] if assistant else ""
    for h in ["Title", "Summary", "Installation and OS", "Steps to reproduce", "Expected behavior", "Actual behavior", "Evidence", "Original description"]:
        check(h in body, f"the real model's report has the template heading: {h}")
    check("The sidebar rows jump by a few pixels when I fold and unfold the sidebar." in body.split("Original description", 1)[-1], "the original description is kept verbatim at the end")
    check("OS: Linux" in body or "Linux" in body, "the OS family is Linux")
    check(bool(w.get("loaded")) and w["loaded"] >= len(body.strip()) // 2, "the report was loaded back into the editable draft", w)
    print("report (first 300 chars):", body[:300].replace("\n", " | "))
else:
    wt = [e for e in events if e.get("$") == "WorktreeSet" and e.get("id") == tid]
    branch = wt[-1]["branch"] if wt else ""; path = wt[-1]["worktree"] if wt else ""
    check(branch.startswith("gpui-imp-") and "/.backplane/worktrees/" in path, "the thread works in its own gpui-imp-* worktree", (branch, path))
    modes = [e for e in events if e.get("$") == "ModesSet" and e.get("id") == tid]
    check(any(m.get("runtime") == "approval-required" for m in modes), "the hand-off thread ran approval-required", modes)
    approved = [m for m in metrics if m.get("event") == "improvements" and m.get("approved")]
    check(len(approved) >= 1, "at least one approval was answered in GPUI (primary button)", [(a.get("approved"), a.get("button")) for a in approved])
    print("approvals:", [(str(a.get("approved"))[:60], a.get("button")) for a in approved])
    rc, log = sh(["git", "-C", P, "log", "--oneline", f"main..{branch}"], P) if branch else (1, "")
    check(rc == 0 and len(log.splitlines()) >= 1, "the agent committed on its own branch", log)
    rc, content = sh(["git", "-C", P, "show", f"{branch}:README.md"], P) if branch else (1, "")
    check("hand-off edit" in content, "the real edit is in the committed README.md", content)
    rc, stat = sh(["git", "-C", P, "diff", "--stat", f"main..{branch}"], P) if branch else (1, "")
    print("diff:", stat.replace("\n", " | "))
    check(stat.count("|") == 1 and "README.md" in stat, "the commit touches only README.md", stat)
    # the scratch copy of the land.sh flow, here only
    gpui0 = sh(["git", "-C", P, "rev-parse", "main"], P)[1]
    tip = sh(["git", "-C", path, "rev-parse", "HEAD"], path)[1] if path else ""
    env = {"GPUI_LAND_LOCK": f"{run}/scratch-lock.sh"}
    rc, msg = sh(["git", "-C", path, "update-ref", "refs/heads/main", tip], path, env)
    check(rc != 0 and "refused" in msg and sh(["git", "-C", P, "rev-parse", "main"], P)[1] == gpui0, "the guard refuses a direct move of the scratch main", msg)
    rc, msg = sh(["sh", "scripts/gpui/land.sh", tip], path, env)
    check(rc != 0 and "not reviewed" in msg and sh(["git", "-C", P, "rev-parse", "main"], P)[1] == gpui0, "land.sh refuses a range with no recorded OK", msg)
    renv = dict(env, CLAUDE_CODE_SESSION_ID="scratch-reviewer")
    rc, msg = sh(["sh", "scripts/gpui/review-ok.sh", "--enrol"], path, renv); check(rc == 0, "a scratch reviewer enrols", msg)
    rc, msg = sh(["sh", "scripts/gpui/review-ok.sh", f"main..{tip}"], path, renv); check(rc == 0 and "recorded" in msg, "the scratch reviewer records the OK", msg)
    rc, msg = sh(["sh", "scripts/gpui/land.sh", tip], path, env)
    check(rc == 0 and "landed" in msg and sh(["git", "-C", P, "rev-parse", "main"], P)[1] == tip, "land.sh moves the scratch main to the agent's commit", msg)
    check("hand-off edit" in sh(["git", "-C", P, "show", "main:README.md"], P)[1], "the scratch main carries the edit")
    # the real gpui and the real repository were never in play
    check(P.startswith(root + "/build/gpui/imp-real-edit/"), "everything above ran in the scratch repository", P)

print("FAILED: " + ", ".join(fails) if fails else "ALL CHECKS PASSED")
sys.exit(1 if fails else 0)
