#!/usr/bin/env python3
# A stand-in `claude` and `codex` for the account failover tests. No model runs. It answers the sign-in checks of a named account from that account's own store, and for each turn
# records in @DIR@/turns.jsonl who ran it (the email of the store it was started with), the session it was asked to resume, and the whole prompt it got.
# @DIR@/mode.<who> says how the turns of that login end:
#   ok (or no file)   an answer
#   limit:<reset>     streams a little, then a rejected rate_limit_event, then an error result, then its lifecycle completes (Claude Code 2.1.286+)
#   limit-noreset     the same, the limit's words alone
#   cancel:<reset>    Claude Code's weekly limit as the Calendar bot met it (Nov 10): a rejected seven_day rate_limit_event, the limit's words as an assistant message with
#                     its `rate_limit` error and an error result, then the CLI cancels its own command (lifecycle `cancelled`), which ends the turn Interrupted
#   limit-old         an old CLI's ending: no lifecycle, the error result ends the turn
#   warn              a rejected rate_limit_event, and the turn ends well all the same, in one read
#   auth              the login failed mid-turn
#   exit-limit        the limit on stderr, and the process ends with no result
#   slow:<s>          a word of progress at once, then the answer after <s> seconds (a turn a restart cuts mid-way)
#   goaway:<n>        the first <n> turns stream a little, then fail on a dropped connection (agy's words of Nov 11: http2 GOAWAY, server_shutting_down); then it answers
#   exit-reset:<n>    the first <n> turns die mid-stream with "connection reset by peer" on stderr and no result; then it answers
# @DIR@/usage.<who> is that login's usage reading, the `rate_limits` of the CLI's get_usage answer.
# A store with a file `dead` in it is a login whose OAuth session died (Nov 7, Claude Code 2.1.319): `auth status` still names the account, every turn fails with
# "Failed to authenticate: OAuth session expired and could not be refreshed", and get_usage is refused with "OAuth refresh token is no longer valid; run /login".
# `claude auth login --claudeai` (the hub's sign-in) prints its link and takes the code "CODE#STATE9" on stdin; it then signs in @DIR@/login.email (else w@x.com) in its own store.
import json, os, sys, time, uuid
D = "@DIR@"
me = os.path.basename(sys.argv[0]); a = sys.argv[1:]
home = os.environ.get("CLAUDE_CONFIG_DIR" if me == "claude" else "CODEX_HOME", "")
# with no profile's store, the CLI's own login: signed in only when @DIR@/default.login names who
login = os.path.join(home, "login.json" if me == "claude" else "auth.json") if home else os.path.join(D, "default.login")
who = json.load(open(login))["email"] if os.path.exists(login) else "default"
def mode():
    try: return open(os.path.join(D, "mode." + who)).read().strip()
    except OSError: return "ok"
def say(o): print(json.dumps(o), flush=True)
def record(session, text): open(os.path.join(D, "turns.jsonl"), "a").write(json.dumps({"who": who, "cli": me, "home": home, "resume": session, "text": text}) + "\n")
def sessions():
    d = os.path.join(home or os.path.join(D, "default-" + me), "sessions"); os.makedirs(d, exist_ok=True); return d

def dead(): return bool(home) and os.path.exists(os.path.join(home, "dead"))
def claude_turn(line, sid):
    u = line.get("uuid", "")
    m = "expired" if dead() else mode(); old = m == "limit-old"
    def lc(st):
        if not old: say({"type": "command_lifecycle", "command_uuid": u, "state": st, "uuid": str(uuid.uuid4()), "session_id": sid})
    c = line.get("message", {}).get("content", "")
    text = c if isinstance(c, str) else " ".join(b.get("text", "") for b in c if isinstance(b, dict))
    lc("queued"); say({"type": "system", "subtype": "init", "session_id": sid, "cwd": os.getcwd(), "tools": ["Bash"], "model": "fake", "permissionMode": "bypassPermissions", "apiKeySource": "none", "claude_code_version": "2.0.0" if old else "2.1.286"}); lc("started")
    record(resume, text)
    if m.startswith("slow:"):
        say({"type": "assistant", "message": {"id": "a" + uuid.uuid4().hex, "role": "assistant", "content": [{"type": "text", "text": "working, by " + who}], "usage": {"input_tokens": 1, "output_tokens": 1}}, "session_id": sid})
        time.sleep(float(m.split(":")[1]))
    if m == "exit-limit": print("Error: You've hit your usage limit. Try again later.", file=sys.stderr, flush=True); os._exit(1)
    if m.startswith("goaway:") or m.startswith("exit-reset:"):
        cf = os.path.join(D, "drops." + who); n = int(open(cf).read()) if os.path.exists(cf) else 0
        if n < int(m.split(":")[1]):
            open(cf, "w").write(str(n + 1))
            say({"type": "stream_event", "event": {"type": "content_block_delta", "index": 0, "delta": {"type": "text_delta", "text": "PARTIAL-" + str(n) + " "}}, "session_id": sid})
            if m.startswith("exit-reset:"): print("Error: read tcp 10.0.0.2:51234->142.250.1.1:443: read: connection reset by peer", file=sys.stderr, flush=True); os._exit(1)
            words = 'agent executor error: generating and executing: stream reading error: http2: server sent GOAWAY and closed the connection; LastStreamID=129, ErrCode=NO_ERROR, debug="server_shutting_down"'
            say({"type": "result", "subtype": "error_during_execution", "is_error": True, "result": words, "session_id": sid})
            lc("completed"); return
    if m == "expired":
        words = "Failed to authenticate: OAuth session expired and could not be refreshed"
        say({"type": "assistant", "message": {"id": "a" + uuid.uuid4().hex, "role": "assistant", "content": [{"type": "text", "text": words}], "usage": {"input_tokens": 0, "output_tokens": 0}}, "error": "authentication_failed", "session_id": sid})
        say({"type": "result", "subtype": "success", "is_error": True, "result": words, "session_id": sid})
        lc("completed"); return
    if m.startswith("cancel:"):
        r = int(m.split(":")[1]); words = "You've hit your weekly limit" # the real line goes on " · resets Nov 12, 3am (America/New_York)": reset_words reads that (its own laws); here the event's resetsAt is the time
        say({"type": "rate_limit_event", "rate_limit_info": {"status": "rejected", "rateLimitType": "seven_day", "resetsAt": r}, "session_id": sid})
        say({"type": "assistant", "message": {"id": "a" + uuid.uuid4().hex, "role": "assistant", "content": [{"type": "text", "text": words}], "usage": {"input_tokens": 0, "output_tokens": 0}}, "error": "rate_limit", "session_id": sid})
        say({"type": "result", "subtype": "success", "is_error": True, "result": words, "session_id": sid})
        lc("cancelled"); return
    if m.startswith("limit") or m == "auth":
        say({"type": "stream_event", "event": {"type": "message_start", "message": {"id": "p1", "role": "assistant", "content": [], "usage": {"input_tokens": 1}}}, "session_id": sid})
        say({"type": "stream_event", "event": {"type": "content_block_start", "index": 0, "content_block": {"type": "text", "text": ""}}, "session_id": sid})
        say({"type": "stream_event", "event": {"type": "content_block_delta", "index": 0, "delta": {"type": "text_delta", "text": "PARTIAL-" + who + " "}}, "session_id": sid})
        if m.startswith("limit:"): say({"type": "rate_limit_event", "rate_limit_info": {"status": "rejected", "rateLimitType": "five_hour", "resetsAt": int(m.split(":")[1])}, "session_id": sid})
        words = "Invalid API key · Please run /login" if m == "auth" else "You've hit your limit"
        say({"type": "result", "subtype": "error_during_execution", "is_error": True, "result": words, "session_id": sid})
        lc("completed"); return
    if m == "warn": # all of it in one write: the hub reads the limit and the good end together, before anything of it is committed
        out = [{"type": "rate_limit_event", "rate_limit_info": {"status": "rejected", "rateLimitType": "seven_day_opus", "resetsAt": int(time.time()) + 900}, "session_id": sid},
               {"type": "assistant", "message": {"id": "a" + uuid.uuid4().hex, "role": "assistant", "content": [{"type": "text", "text": "answered by " + who}], "usage": {"input_tokens": 1, "output_tokens": 1}}, "session_id": sid},
               {"type": "result", "subtype": "success", "is_error": False, "result": "answered by " + who, "session_id": sid, "modelUsage": {"fake": {"contextWindow": 200000}}},
               {"type": "command_lifecycle", "command_uuid": u, "state": "completed", "uuid": str(uuid.uuid4()), "session_id": sid}]
        sys.stdout.write("".join(json.dumps(o) + "\n" for o in out)); sys.stdout.flush(); return
    say({"type": "assistant", "message": {"id": "a" + uuid.uuid4().hex, "role": "assistant", "content": [{"type": "text", "text": "answered by " + who}], "usage": {"input_tokens": 1, "output_tokens": 1}}, "session_id": sid})
    say({"type": "result", "subtype": "success", "is_error": False, "result": "answered by " + who, "session_id": sid, "modelUsage": {"fake": {"contextWindow": 200000}}})
    lc("completed")

if me == "claude":
    if a[:2] == ["auth", "status"]:
        try: time.sleep(float(open(os.path.join(D, "authslow")).read().strip())) # @DIR@/authslow: a slow status check (the account inventory loading after a restart)
        except (OSError, ValueError): pass
        if os.path.exists(login):
            j = json.load(open(login)); j.update({"loggedIn": True, "authMethod": "claude.ai", "configDirectory": home}); print(json.dumps(j))
        else: print(json.dumps({"loggedIn": False}))
        sys.exit(0)
    if a[:2] == ["auth", "login"]:
        print("Opening your browser. If it does not open, visit https://claude.com/cai/oauth/authorize?code=true&client_id=standin&state=STATE9 and paste the code here:", flush=True)
        code = sys.stdin.readline().strip()
        if code != "CODE#STATE9": print("That code wasn't accepted.", flush=True); sys.exit(1)
        try: email = open(os.path.join(D, "login.email")).read().strip()
        except OSError: email = "w@x.com"
        json.dump({"email": email, "subscriptionType": "max", "accountUuid": email, "orgUuid": "o1"}, open(login, "w")); print("Login successful.", flush=True); sys.exit(0)
    resume = a[a.index("--resume") + 1] if "--resume" in a else ""
    sid = resume
    if resume and not os.path.exists(os.path.join(sessions(), resume)):
        say({"type": "system", "subtype": "init", "session_id": resume, "cwd": os.getcwd(), "tools": [], "model": "fake", "permissionMode": "bypassPermissions", "apiKeySource": "none", "claude_code_version": "2.1.286"})
        say({"type": "result", "subtype": "error_during_execution", "is_error": True, "result": "No conversation found with session ID: " + resume, "session_id": resume}); sys.exit(1)
    if not sid: sid = "s-" + who + "-" + uuid.uuid4().hex[:8]; open(os.path.join(sessions(), sid), "w").close()
    for l in sys.stdin:
        try: r = json.loads(l)
        except Exception: continue
        if r.get("type") == "user": claude_turn(r, sid); continue
        rid = r.get("request_id")
        if rid is None: continue
        def usage():
            try: return {"rate_limits_available": True, "rate_limits": json.load(open(os.path.join(D, "usage." + who)))}
            except OSError: return {"rate_limits_available": False}
        if dead() and r.get("request", {}).get("subtype") == "get_usage":
            say({"type": "control_response", "response": {"request_id": rid, "subtype": "error", "error": "OAuth refresh token is no longer valid; run /login to re-authenticate"}}); continue
        resp = usage() if r.get("request", {}).get("subtype") == "get_usage" else {"models": [{"value": "opus", "displayName": "Opus", "supportedEffortLevels": ["low", "high"]}]} if rid == "models" else {}
        say({"type": "control_response", "response": {"request_id": rid, "subtype": "success", "response": resp}})
    sys.exit(0)

# codex
if a[:1] == ["--version"]: print("codex-cli 0.0.0"); sys.exit(0)
if "app-server" in a:
    for l in sys.stdin:
        try: m = json.loads(l)
        except Exception: continue
        if "id" not in m: continue
        i, meth = m["id"], m.get("method")
        if meth == "account/read": say({"id": i, "result": {"account": {"type": "chatgpt", "email": who, "planType": "plus"}, "workspaceRouting": {"chatgptAccountId": who}}})
        elif meth == "model/list": say({"id": i, "result": {"data": [{"model": "gpt-x", "displayName": "GPT X", "supportedReasoningEfforts": [{"reasoningEffort": "low"}]}]}})
        else: say({"id": i, "result": {}})
    sys.exit(0)
text = a[-1] if a else ""
resume = a[a.index("resume") + 1] if "resume" in a else ""
record(resume, text)
tid = resume or "cs-" + who + "-" + uuid.uuid4().hex[:8]
say({"type": "thread.started", "thread_id": tid})
m = mode()
if m.startswith("limit"): say({"type": "turn.failed", "error": {"message": "You've hit your usage limit. Try again later.", "resets_at": int(m.split(":")[1]) if ":" in m else 0}}); sys.exit(1)
if m == "auth": say({"type": "turn.failed", "error": {"message": "Your access token could not be refreshed. Please run /login", "code": "token_expired"}}); sys.exit(1)
say({"type": "item.completed", "item": {"id": "it1", "type": "agent_message", "text": "answered by " + who}})
say({"type": "turn.completed", "usage": {"input_tokens": 1, "cached_input_tokens": 0, "output_tokens": 1}})
