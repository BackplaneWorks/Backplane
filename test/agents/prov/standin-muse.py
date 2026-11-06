#!/usr/bin/env python3
# A stand-in for `muse serve` (MSP, JSON-RPC over stdio) for the Rust and JS hub tests: no model runs. It answers as muse 1.4.6 does
# (frames trimmed): initialize, session/start|resume, session/setApprovalMode, model/list, turn/start -> a streamed reply, a tool call,
# todos and turn/completed; an approval/request first when the text says "perm" and the mode is not allowAll (the turn waits for approval/decide).
# Every line it reads goes to $MUSE_LOG (with its argv first).
import json, os, sys
log = os.environ.get("MUSE_LOG", "/dev/null")
def note(s):
    with open(log, "a") as f: f.write(s + "\n")
def say(o):
    sys.stdout.write(json.dumps({"jsonrpc": "2.0", **o}, ensure_ascii=False) + "\n"); sys.stdout.flush()
note("ARGV " + " ".join(sys.argv[1:]))
sid, mode, seen = "", "", set()
def event(m, p): say({"method": m, "params": {"sessionId": sid, "viewCursor": "v", **p}})
def turn(cid, text):
    event("item/delta", {"delta": "ECHO: " + text[-40:], "field": "text", "itemId": "m1"})
    event("item/delta", {"delta": "thinking", "field": "summary.0", "itemId": "r1"})
    event("item/started", {"item": {"args": json.dumps({"command": "ls"}), "itemId": "T1", "kind": "toolCall", "status": "inProgress", "tool": "bash"}})
    if "perm" in text and mode != "allowAll":
        say({"id": 0, "method": "approval/request", "params": {"approvalId": "01a13123-bdf5-7d51-876d-5ba7d639f788", "availableChoices": [
            {"choiceId": "allow_once", "decision": "approved", "label": "Allow once", "scope": "once"}, {"choiceId": "deny", "decision": "denied", "label": "Deny", "scope": "once"}],
            "currentRequirementId": {"approvalId": "01a13123-bdf5-7d51-876d-5ba7d639f788", "sourceIndex": 0}, "itemId": "T1", "rawArgs": "{\"command\":\"ls\"}", "sessionId": sid,
            "sourceRange": {"first": {"id": "x", "sequence": 55}, "last": {"id": "y", "sequence": 57}}, "subject": {"kind": "shell", "command": "ls"}, "toolName": "bash"}})
        return False
    return finish()
def finish():
    event("item/completed", {"item": {"itemId": "T1", "kind": "toolCall", "status": "completed", "tool": "bash"}})
    event("session/todoListChanged", {"items": [{"status": "completed", "text": "Read"}, {"status": "inProgress", "text": "Fix"}]})
    event("item/completed", {"item": {"itemId": "m1", "kind": "agentMessage", "status": "completed", "text": "ECHO done"}})
    event("turn/completed", {"durationMs": 5, "terminal": "completed", "turnId": "U"})
    return True
for l in sys.stdin:
    l = l.strip()
    if not l: continue
    note("IN " + l)
    try: o = json.loads(l)
    except Exception: continue
    m, i, p = o.get("method"), o.get("id"), o.get("params", {})
    cid = p.get("commandId")
    if cid:
        if cid in seen: say({"id": i, "error": {"code": -32030, "message": "command id seen before"}}); continue
        seen.add(cid)
    if m == "initialize": say({"id": i, "result": {"experimentalApi": False, "grantedCapabilities": ["sessionMcp"], "serverInfo": {"name": "muse", "version": "1.4.6"}}})
    elif m == "session/start": sid, mode = "MS1", p.get("approvalMode", ""); say({"id": i, "result": {"session": {"sessionId": sid, "status": "idle"}, "viewCursor": "v:MS1:0"}})
    elif m == "session/resume": sid = p.get("sessionId", ""); say({"id": i, "result": {"session": {"sessionId": sid, "status": "idle"}, "viewCursor": "v"}})
    elif m == "session/setApprovalMode": mode = p.get("mode", ""); say({"id": i, "result": {}})
    elif m == "model/list": say({"id": i, "result": {"models": [{"modelId": "muse-1", "displayLabel": "Muse One", "isDefault": True}]}})
    elif m == "turn/start":
        say({"id": i, "result": {"turnId": "U"}})
        turn(cid, p["input"][0]["text"])
    elif m == "approval/decide":
        note("DECIDE " + json.dumps(p.get("choiceId")))
        say({"id": i, "result": {}})
        finish()
    elif i is not None and m is None: pass
    elif i is not None: say({"id": i, "error": {"code": -32601, "message": "unknown"}})
