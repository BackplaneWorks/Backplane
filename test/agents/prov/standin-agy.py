#!/usr/bin/env python3
# A stand-in for Google's agy_acp_server (Antigravity's ACP agent) for the Rust and JS hub tests, as test/tools/agy_standin.ts answers
# (replies from test/fixtures/antigravity): no model runs, no sign-in. initialize, authenticate, session/new (the fixture's models) or
# session/resume (no id), session/set_mode, session/set_config_option, session/prompt -> "ECHO: <text>", a tool call that completes, a permission
# request first when the text says "perm" and the mode is not yolo; then end_turn. Lines go to $AGY_LOG with what the environment held.
import json, os, sys
log = os.environ.get("AGY_LOG", "/dev/null")
def note(s):
    with open(log, "a") as f: f.write(s + "\n")
def say(o): sys.stdout.write(json.dumps({"jsonrpc": "2.0", **o}, ensure_ascii=False) + "\n"); sys.stdout.flush()
home = os.environ.get("GEMINI_HOME", "")
note(f"START home={home} gemkey={os.environ.get('GEMINI_API_KEY','')} gkey={os.environ.get('GOOGLE_API_KEY','')} args={' '.join(sys.argv[1:])}")
fx = [json.loads(l) for l in open(os.path.join(os.path.dirname(os.path.abspath(__file__)), "agy-turn.jsonl")) if l.strip()]
opened = next(o for o in fx if o.get("id") == 2)["result"]
sid, mode, waiting = "", "default", None
def update(u): say({"method": "session/update", "params": {"sessionId": sid, "update": u}})
def prompt(i, text):
    update({"sessionUpdate": "agent_message_chunk", "content": {"type": "text", "text": "ECHO: " + text[-40:]}})
    update({"sessionUpdate": "tool_call", "toolCallId": "T1", "title": "Run ls", "kind": "execute", "status": "pending"})
    update({"sessionUpdate": "tool_call_update", "toolCallId": "T1", "status": "completed"})
    say({"id": i, "result": {"stopReason": "end_turn"}})
for l in sys.stdin:
    l = l.strip()
    if not l: continue
    note("IN " + l)
    o = json.loads(l); m, i, p = o.get("method"), o.get("id"), o.get("params", {})
    if m is None and waiting and i == waiting[0]:
        prompt(waiting[1], waiting[2]); waiting = None
    elif m == "initialize": say({"id": i, "result": next(x for x in fx if x.get("id") == 1)["result"]})
    elif m == "authenticate": say({"id": i, "result": {}})
    elif m == "session/new": sid = "AS1"; say({"id": i, "result": {**opened, "sessionId": sid}})
    elif m == "session/resume": sid = p.get("sessionId", ""); say({"id": i, "result": {}})
    elif m == "session/set_mode": mode = p.get("modeId", "default"); say({"id": i, "result": {}})
    elif m == "session/set_config_option": say({"id": i, "result": {}})
    elif m == "session/prompt":
        text = p["prompt"][0]["text"]
        if "perm" in text and mode != "yolo":
            update({"sessionUpdate": "agent_message_chunk", "content": {"type": "text", "text": "asking"}})
            say({"id": 77, "method": "session/request_permission", "params": {"sessionId": sid, "toolCall": {"toolCallId": "T1", "kind": "execute", "title": "Run ls", "rawInput": {"command": "ls"}},
                "options": [{"optionId": "allow", "kind": "allow_once", "name": "Allow"}, {"optionId": "deny", "kind": "reject_once", "name": "Deny"}]}})
            waiting = (77, i, text)
        else: prompt(i, text)
    elif i is not None: say({"id": i, "error": {"code": -32601, "message": "unknown"}})
