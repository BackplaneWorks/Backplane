#!/usr/bin/env python3
"""Stand-in agent CLIs for the sign-in-there e2e (there.py): never contact a provider. One script, called as claude, codex or agy (argv[1] names it).
Each keeps its sign-in in $HOME/.standin-<name> (the machine's own, as a CLI's login is).
  claude auth login   draws a link, reads the pasted code from its terminal, signs in as $STANDIN_EMAIL
  claude auth status  JSON as Claude Code says it
  claude setup-token  (the sync token, on the desktop) draws a link, reads the code, prints a token
  codex login --device-auth   a link and a device code, then waits for $HOME/.standin-approve (the test approves "in the browser")
  codex login status  Logged in / Not logged in
  agy                 asks to sign in, reads a line, says it is signed in
--version for each."""
import json, os, sys, time
name, a = sys.argv[1], sys.argv[2:]
home = os.environ.get("HOME", "/tmp")
mark = os.path.join(home, f".standin-{name}")
def signed(who): open(mark, "w").write(who)
if a[:1] == ["--version"]:
    print({"claude": "2.1.322 (Claude Code)", "codex": "codex-cli 0.167.0", "agy": "1.5.5"}[name]); sys.exit(0)
if name == "claude":
    if a[:2] == ["auth", "status"]:
        if os.path.exists(mark): print(json.dumps({"loggedIn": True, "email": open(mark).read(), "authMethod": "claude.ai"})); sys.exit(0)
        if os.environ.get("CLAUDE_CODE_OAUTH_TOKEN"): os.execv(sys.executable, [sys.executable, "-I", os.path.join(os.path.dirname(os.path.abspath(__file__)), "../syncaccounts/claude_standin.py")] + a)
        print(json.dumps({"loggedIn": False, "authMethod": "none"})); sys.exit(1)
    if a[:2] == ["auth", "login"]:
        print("Opening browser to sign in…", flush=True)
        print("Browser didn't open? Use the url below to sign in (c to copy)\n", flush=True)
        print("https://claude.com/cai/oauth/authorize?code=true&client_id=9d1c250a-e61b-44d9-88ed-5944d1962f5e&response_type=code&redirect_uri=https%3A%2F%2Fplatform.claude.com%2Foauth%2Fcode%2Fcallback&scope=org%3Acreate_api_key+user%3Aprofile+user%3Ainference&code_challenge=" + "x" * 43 + "&code_challenge_method=S256&state=LOGINSTATE\n", flush=True)
        print("Paste code here if prompted > ", end="", flush=True)
        code = sys.stdin.readline().strip()
        if code != "CODE#LOGINSTATE": print("OAuth error: Invalid code", flush=True); sys.exit(1)
        signed(os.environ.get("STANDIN_EMAIL", "boat@example.com")); print("Login successful.", flush=True); sys.exit(0)
    if a[:1] == ["setup-token"]:
        print("Browser didn't open? Use the url below to sign in", flush=True)
        print("https://claude.com/cai/oauth/authorize?code=true&client_id=x&state=SYNCSTATE", flush=True)
        print("Paste code here if prompted > ", end="", flush=True)
        code = sys.stdin.readline().strip()
        if code != "CODE#SYNCSTATE": print("OAuth error: Invalid code.", flush=True); sys.exit(1)
        print("✓ Long-lived authentication token created successfully!\n\nYour OAuth token (valid for 1 year):\n\nsk-ant-oat01-" + "S" * 90 + "\n\nStore this token securely.", flush=True)
        sys.exit(0)
    if a[:1] == ["auth"]: sys.exit(0)
    # a turn or a usage read (the sync token's check): the sync e2e's stand-in answers those
    os.execv(sys.executable, [sys.executable, "-I", os.path.join(os.path.dirname(os.path.abspath(__file__)), "../syncaccounts/claude_standin.py")] + a)
if name == "codex":
    if a[:2] == ["login", "status"]:
        if os.path.exists(mark): print("Logged in using ChatGPT"); sys.exit(0)
        print("Not logged in", file=sys.stderr); sys.exit(1)
    if a[:2] == ["login", "--device-auth"]:
        print("\nWelcome to Codex [v0.167.0]\nOpenAI's command-line coding agent\n\nFollow these steps to sign in with ChatGPT using device code authorization:\n\n1. Open this link in your browser and sign in to your account\n   https://auth.openai.com/codex/device\n\n2. Enter this one-time code (expires in 15 minutes)\n   QWER-TY123\n", flush=True)
        for _ in range(600):
            if os.path.exists(os.path.join(home, ".standin-approve")): signed("chatgpt"); print("Successfully logged in", flush=True); sys.exit(0)
            time.sleep(0.2)
        sys.exit(1)
    sys.exit(1)
if name == "agy":
    print("Welcome to Antigravity CLI.\nYou are not signed in. Press Enter to sign in with Google.", flush=True)
    sys.stdin.readline()
    print("Signed in as agy@example.com. Type /quit to leave.", flush=True)
    for line in sys.stdin:
        if line.strip() == "/quit": sys.exit(0)
    sys.exit(0)
sys.exit(1)
