#!/usr/bin/env python3
"""A stand-in `claude` for the sync-account e2e (sync.py): never contacts a provider.
  setup-token   draws a link, reads the pasted code from its terminal, prints a token (STANDIN_TOKEN, else a fixed one)
  auth status   says it runs on CLAUDE_CODE_OAUTH_TOKEN when it has one
  -p ...        stream-json: answers the hub's control requests (models, usage) and each user message with one reply
Every start with a token appends `<pid> <first arg> <token>` to $SYNC_SEEN, so the test can say which child got which token."""
import json, os, sys, uuid
a = sys.argv[1:]
tok = os.environ.get("CLAUDE_CODE_OAUTH_TOKEN", "")
seen = os.environ.get("SYNC_SEEN", "")
if seen:
    with open(seen, "a") as f: f.write(f"{os.getpid()} START {(a or ['-'])[0]} {tok or 'none'}\n")
if a[:1] == ["--version"]:
    print("2.1.322 (Claude Code)"); sys.exit(0)
if a[:1] == ["setup-token"]:
    print("\x1b[2mBrowser didn't open? Use the url below to sign in\x1b[0m", flush=True)
    print("https://claude.com/cai/oauth/authorize?code=true&client_id=x&state=SYNCSTATE", flush=True)
    print("Paste code here if prompted > ", end="", flush=True)
    code = sys.stdin.readline().strip()
    if code != "CODE#SYNCSTATE": print("OAuth error: Invalid code.", flush=True); sys.exit(1)
    print("\x1b[32m✓ Long-lived authentication token created successfully!\x1b[0m\n\nYour OAuth token (valid for 1 year):\n\n\x1b[33m"
          + os.environ.get("STANDIN_TOKEN", "sk-ant-oat01-" + "S" * 90) + "\x1b[0m\n\nStore this token securely.", flush=True)
    sys.exit(0)
if a[:2] == ["auth", "status"]:
    d = os.environ.get("CLAUDE_CONFIG_DIR", "")
    if tok: print(json.dumps({"loggedIn": True, "authMethod": "oauth_token", "configDirectory": d, "loginExpiresAt": None}))
    else: print(json.dumps({"loggedIn": False, "authMethod": "none", "configDirectory": d})); sys.exit(1)
    sys.exit(0)
if a[:1] == ["auth"]: sys.exit(0)
session = f"sess-{os.getpid()}"
def emit(o): print(json.dumps(o), flush=True)
for line in sys.stdin:
    try: o = json.loads(line)
    except ValueError: continue
    if o.get("type") == "control_request":
        sub = o.get("request", {}).get("subtype"); rid = o.get("request_id")
        resp = {}
        if sub == "initialize": resp = {"models": [{"value": "opus", "displayName": "Opus", "supportedEffortLevels": ["low", "high"]}]}
        if sub == "get_usage": resp = {"rate_limits_available": True, "rate_limits": {"five_hour": {"utilization": 12.0, "resets_at": "2030-01-01T00:00:00Z"}}}
        emit({"type": "control_response", "response": {"request_id": rid, "subtype": "success", "response": resp}})
        continue
    if o.get("type") != "user": continue
    emit({"type": "system", "subtype": "init", "cwd": os.getcwd(), "session_id": session, "tools": [], "mcp_servers": [], "model": "claude-opus", "permissionMode": "bypassPermissions", "apiKeySource": "none", "claude_code_version": "2.1.322", "uuid": str(uuid.uuid4())})
    if seen:
        with open(seen, "a") as f: f.write(f"{os.getpid()} TURN - {tok or 'none'}\n")
    text = "sync stand-in answered" + (" on a token" if tok else "")
    emit({"type": "assistant", "message": {"id": f"m{os.getpid()}", "type": "message", "role": "assistant", "model": "claude-opus", "content": [{"type": "text", "text": text}], "usage": {"input_tokens": 1, "output_tokens": 1}}, "parent_tool_use_id": None, "session_id": session, "uuid": str(uuid.uuid4())})
    emit({"type": "result", "subtype": "success", "is_error": False, "duration_ms": 5, "num_turns": 1, "result": text, "session_id": session, "total_cost_usd": 0, "usage": {}, "modelUsage": {}, "uuid": str(uuid.uuid4())})
