#!/usr/bin/env python3
"""Checks for scripts/improvements-e2e.sh: what the client reported, what the hub logged, what was opened, what was uploaded, what leaked.
usage: improvements-e2e-check.py OUTDIR RUNDIR SHOTSDIR"""
import json, os, re, sys, urllib.parse

out, run, shots = sys.argv[1:4]
fails = []
def check(ok, what, detail=""):
    print(("ok   " if ok else "FAIL ") + what + ((" -- " + detail) if detail and not ok else ""))
    if not ok: fails.append(what)

def lines(p):
    try: return open(p, errors="replace").read().splitlines()
    except OSError: return []
metrics = []
for l in lines(f"{out}/client.log"):
    if l.startswith("GPUI_METRIC "):
        try: metrics.append(json.loads(l[len("GPUI_METRIC "):]))
        except ValueError: pass
states = [m for m in metrics if m.get("event") == "improvements"]
events = []
for l in lines(f"{out}/events.jsonl"):
    try: events.append(json.loads(l))
    except ValueError: pass

# pictures: this window only, one per step
want = ["01-open", "02-evidence", "03-review", "04-opened", "05-writer-started", "06-report-loaded", "07-recording", "08-recording-kept", "09-internal", "10-handed-off", "11-board-picked", "12-remembered"]
for w in want:
    p = f"{shots}/improvements-{w}.png"
    check(os.path.exists(p) and os.path.getsize(p) > 2000, f"picture {w}")

# the client's own state, step by step
check(len(states) >= 3, "the client reported its state three times", str(len(states)))
if len(states) >= 3:
    s1, s2, s3 = states[0], states[1], states[2]
    check(s1["evidence"] is True and s1["report"] > 0, "the public report was prepared", str(s1))
    check(s1["opened"] is True and s1["internal"] is False, "the GitHub draft was recorded as opened before it opened", str(s1))
    check(bool(s2.get("writer")) and s2["writer"].get("created") and s2["writer"].get("started"), "the report writer thread was created and its turn started", str(s2))
    check(s3["busy"] is False and s3["note"].startswith("Request sent"), "the internal request was sent and the draft cleared", str(s3))

# what was opened: only the new-issue form of the Backplane repository, with the exact body; nothing else left the machine
opened = [l for l in lines(f"{out}/opened.txt") if l]
gh = [u for u in opened if u.startswith("https://github.com/")]
check(len(gh) == 1 and gh[0].startswith("https://github.com/BackplaneWorks/Backplane/issues/new?title=Backplane%20improvement&body="), "exactly one GitHub draft link, to the Backplane repository", str(gh)[:200])
body = urllib.parse.unquote(gh[0].split("body=", 1)[1]) if gh else ""
check("The sidebar rows jump when I fold and unfold it.\nSecond line of my words." in body, "the person's words are verbatim in the body")
check("OS: Linux" in body and re.search(r"App version: (\d+\.\d+\.\d+|unknown)\n", body) is not None, "OS family and a numeric version only")
check("\n\nDetails\nSeen on the Rust hub, Linux, GPUI." in body, "the elected details are in the body")
m = re.search(r"\n\nDiagnostics \(JSON\)\n(\{.*\})\s*$", body, re.S)
check(m is not None, "the elected diagnostics are in the body")
if m:
    d = json.loads(m.group(1))
    check(set(d) == {"schema", "connection", "hub"} and d["schema"] == 1 and d["connection"] == [], "diagnostics have only the closed schema")
    allowed = {"event", "name", "code", "at", "ms", "durationMs", "count", "maxMs", "meanMs", "p90Ms", "value", "us"}
    check(all(set(r) <= allowed for r in d["hub"]), "every diagnostics record has only allowlisted fields")
check(len([u for u in opened if u.endswith(".webm")]) >= 1, "the recording was played through the opener (the reviewer step)")

# the hub's log: two threads (writer, internal), attachments, the hand-off text, stand-in agents only
th = [e for e in events if e.get("$") == "ThreadCreated" and str(e.get("id", "")).startswith("improvement-")]
wt = {e["id"]: e for e in events if e.get("$") == "WorktreeSet"}
board = [e for e in th if e.get("project") == "p2"]; th = [e for e in th if e.get("project") != "p2"]
check(len(board) == 1, "the second request made its thread in the project the person picked (Board)", str([e.get("id") for e in board]))
check(len(th) == 2 and any(str(wt.get(e["id"], {}).get("branch", "")).startswith("imp-") and "/.backplane/worktrees/" in wt[e["id"]].get("worktree", "") for e in th), "the internal thread works in its own worktree on a imp-* branch, never the project folder", str(wt))
check(len(th) == 2 and all(e["project"] == "p1" for e in th), "two improvement threads, both in the Backplane source project", str([e.get("id") for e in th]))
users = [e for e in events if e.get("$") == "MessagePosted" and e.get("role") == "user" and str(e.get("thread", "")).startswith("improvement-")]
writer = [e for e in users if "Write an editable Backplane issue report" in e.get("text", "")]
internal = [e for e in users if "<backplane_improvements_context>" in e.get("text", "") and e.get("thread") not in {b["id"] for b in board}]
to_board = [e for e in users if e.get("thread") in {b["id"] for b in board}]
check(len(to_board) == 1 and to_board[0]["text"].startswith("Make the board outline rounder.") and "Implement the request above in the project Board" in to_board[0]["text"]
      and "sent to the project Board" in to_board[0]["text"] and "main builder" not in to_board[0]["text"], "the Board request names its project, not Backplane", str([e.get("text", "")[:300] for e in to_board]))
check(any(e.get("$") == "SettingSet" and e.get("key") == "improvements.project" and e.get("value") == "p2" for e in events), "the choice is remembered on the hub")
check(len(states) >= 1 and states[-1].get("project") == "p2" and states[-1].get("thread") == "" and not states[-1].get("busy"), "the panel opened again, on a fresh draft, starts from the last choice", str(states[-1:] if states else ""))
check(len(writer) == 1 and "[attachment: " in writer[0]["text"], "the writer got its prompt and the selected evidence", str(len(writer)))
check(len(writer) == 1 and "Original description, verbatim, until end of input\nThe sidebar rows jump" in writer[0]["text"], "the writer prompt ends with the original description verbatim")
check(len(internal) == 1, "the internal request was posted once", str(len(internal)))
if internal:
    t = internal[0]["text"]
    check(t.startswith("The sidebar rows jump when I fold and unfold it.\nSecond line of my words."), "the request text leads the message verbatim")
    for w in ["<backplane_handoff>", "own worktree on the branch imp-", "fast-forward main yourself", "main builder then rebuilds"]:
        check(w in t, f"the hand-off says: {w}")
    check(t.count("[attachment: ") == 2 and ".png]" in t and ".webm]" in t, "the screenshot and the recording are attached once each")
atts = open(f"{out}/attachments.txt").read() if os.path.exists(f"{out}/attachments.txt") else ""
check(".png" in atts and ".webm" in atts, "the files arrived through /attach", atts[:200])

# nothing real: stand-in agents, no token or home path in any message or in the opened link
tok = open(f"{run}/home/token").read().strip() if os.path.exists(f"{run}/home/token") else "\0"
blob = json.dumps([e for e in events if e.get("$") == "MessagePosted"]) + "".join(opened)
check(tok not in blob, "the pairing token appears in no message and no opened link")
check(run + "/fakehome" not in "".join(opened), "no private path in an opened link")
check(not any("TurnUsage" in l and "claude-" in l for l in lines(f"{out}/hub.log")), "no real model usage in the hub log")
check(os.path.exists(f"{run}/saved") and any(f.startswith("backplane-issue") for f in os.listdir(f"{run}/saved")), "Save report wrote a file, never over another")

print("FAILED: " + ", ".join(fails) if fails else "ALL CHECKS PASSED")
sys.exit(1 if fails else 0)
