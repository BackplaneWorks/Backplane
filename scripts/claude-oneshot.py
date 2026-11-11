#!/usr/bin/env python3
"""One `claude -p` one-shot on a Claude login that can take it, for scripts (build-haiku.sh): the hub's order (agents/textgen.rs `healthy_order`)
without the hub. The CLI's own login first, then each named Claude account of the hub's home (providers/accounts-v1/profiles.json, its store as
CLAUDE_CONFIG_DIR, as accounts_io.rs launches it); a sync account (its token is the hub's) is left out. A login that answers with a limit is skipped
by the next one-shots for an hour (a small file in the cache). The text comes on stdin; the answer goes to stdout; exit 1 when no login answered.
  claude-oneshot.py [--model M] [--system TEXT] [--timeout S] [-- extra claude args]
Env: BACKPLANE_HOME (default ~/.backplane-gpui-fresh/home), ONESHOT_CLAUDE (the claude program), ONESHOT_STATE (the skip file),
ONESHOT_LOG (1: say on stderr which login answered or why one did not)."""
import json, os, subprocess, sys, time

def args():
    a = sys.argv[1:]; o = {"model": "claude-haiku-5-5", "system": "", "timeout": 90.0, "extra": []}
    while a:
        k = a.pop(0)
        if k == "--": o["extra"] = a; break
        elif k == "--model": o["model"] = a.pop(0)
        elif k == "--system": o["system"] = a.pop(0)
        elif k == "--timeout": o["timeout"] = float(a.pop(0))
        else: sys.exit(f"unknown argument {k}")
    return o

def logins(home):
    """[(key, label, env)] in the order to try: the default login, then the named Claude accounts with a store."""
    out = [("default", "the default login", {})]
    try: profiles = json.load(open(os.path.join(home, "providers", "accounts-v1", "profiles.json"))).get("profiles", [])
    except (OSError, ValueError): profiles = []
    for p in profiles:
        if p.get("provider") != "claude" or p.get("kind") == "sync" or not p.get("id"): continue
        d = os.path.join(home, "providers", "accounts-v1", p["id"], p["store"]) if p.get("store") else os.path.join(home, "providers", "accounts-v1", p["id"])
        if os.path.isdir(d): out.append((f'{p["id"]}:{p.get("epoch", 0)}', p.get("label") or p["id"], {"CLAUDE_CONFIG_DIR": d}))
    return out

def failure(out):
    """claude's error result (a limit, a sign-in), else ""."""
    for l in out.splitlines()[::-1]:
        try: o = json.loads(l)
        except ValueError: continue
        if o.get("type") == "result" and o.get("is_error"):
            return str(o.get("result") or "an error result") + (f' ({o["api_error_status"]})' if isinstance(o.get("api_error_status"), int) else "")
    return ""

def answer(out):
    for l in out.splitlines()[::-1]:
        try: o = json.loads(l)
        except ValueError: continue
        if o.get("type") == "result" and not o.get("is_error"): return (o.get("result") or "").strip()
    return ""

def main():
    o = args(); text = sys.stdin.read(); now = time.time(); say = os.environ.get("ONESHOT_LOG") == "1"
    home = os.environ.get("BACKPLANE_HOME") or os.path.expanduser("~/.backplane-gpui-fresh/home")
    prog = os.environ.get("ONESHOT_CLAUDE", "claude")
    state = os.environ.get("ONESHOT_STATE") or os.path.join(os.environ.get("XDG_CACHE_HOME") or os.path.expanduser("~/.cache"), "backplane", "oneshot-limited.json")
    try: out_until = {k: v for k, v in json.load(open(state)).items() if v > now}
    except (OSError, ValueError): out_until = {}
    for key, label, extra_env in logins(home):
        if key in out_until: say and print(f"oneshot: {label} skipped (out until {time.strftime('%H:%M', time.localtime(out_until[key]))})", file=sys.stderr); continue
        env = {k: v for k, v in os.environ.items() if k not in ("CLAUDE_CONFIG_DIR", "BACKPLANE_AGENT_OWNER", "CLAUDE_CODE_OAUTH_TOKEN", "ANTHROPIC_API_KEY")}
        env.update(extra_env)
        cmd = [prog, "-p", "--model", o["model"], "--output-format", "json", "--tools", "", "--disable-slash-commands", "--no-session-persistence", "--strict-mcp-config"]
        if extra_env: cmd += ["--setting-sources", "user"]
        if o["system"]: cmd += ["--system-prompt", o["system"]]
        cmd += o["extra"]
        try: r = subprocess.run(cmd, input=text, capture_output=True, text=True, timeout=o["timeout"], env=env)
        except (OSError, subprocess.TimeoutExpired) as e: say and print(f"oneshot: {label}: {e}", file=sys.stderr); continue
        a = answer(r.stdout)
        if a: say and print(f"oneshot: {label} answered", file=sys.stderr); print(a); return 0
        why = failure(r.stdout) or (r.stderr.strip()[-200:] or "no answer")
        say and print(f"oneshot: {label} gave nothing: {why}", file=sys.stderr)
        low = why.lower()
        if "limit" in low or "(429)" in low:
            out_until[key] = now + 3600
            try: os.makedirs(os.path.dirname(state), exist_ok=True); json.dump(out_until, open(state, "w"))
            except OSError: pass
    return 1

if __name__ == "__main__": sys.exit(main())
