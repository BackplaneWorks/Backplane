#!/usr/bin/env python3
# A stand-in `claude` for the F rows: stream-json on stdin/stdout like perf_standin.ts, plus triggers in the user's text.
#   zqask      asks the person a question through the hub's MCP permission_prompt (AskUserQuestion) and logs the answer
#   zqapprove  asks to run a Bash command (an approval) and logs allow or deny
#   zqplan     calls ExitPlanMode (the plan card)
#   zqtool X   calls the hub MCP tool X (thread_list) and logs what it said
#   zqecho     answers with the text it was given, so attachments and agentText can be checked
#   zqquote D  calls order_quote for folder D
#   zqshort    a short turn; anything else a turn of FAKE_SECS seconds
import sys, os, json, time, urllib.request
log = os.environ.get("FAKE_LOG", "/dev/null")
def L(s):
    open(log, "a").write(s + "\n")
a = sys.argv[1:]
url = ""
if "--mcp-config" in a:
    try: url = json.loads(a[a.index("--mcp-config") + 1])["mcpServers"]["backplane"]["url"]
    except Exception: pass
L("START %d args=%s" % (os.getpid(), json.dumps(a)[:600]))
def mcp(method, params):
    req = urllib.request.Request(url, data=json.dumps({"jsonrpc": "2.0", "id": 1, "method": method, "params": params}).encode(), headers={"Content-Type": "application/json"})
    return json.loads(urllib.request.urlopen(req, timeout=300).read() or b"{}")
def out(o):
    print(json.dumps(o), flush=True)
if "-p" in a and "--input-format" not in a:  # one-shot text generation (titles, summaries)
    sys.stdin.read(); L("ONESHOT")
    print(json.dumps({"type": "result", "subtype": "success", "is_error": False, "result": "Stand-in title for the thread"}), flush=True); sys.exit(0)
sid = "fake-%d" % os.getpid()
n = 0
import threading, queue
# stdin is read by a thread into a queue: select() on a pipe works on Unix only (Windows selects sockets), and a long turn must wait with a timeout
_lines = queue.Queue()
def _reader():
    buf = b""
    while True:
        d = os.read(0, 65536)
        if not d: _lines.put(None); return
        buf += d
        while b"\n" in buf:
            line, buf = buf.split(b"\n", 1); _lines.put(line.decode("utf-8", "replace"))
threading.Thread(target=_reader, daemon=True).start()
def readline(timeout=None):
    """One line of stdin, or None at EOF; with a timeout, "" when nothing arrived (a long turn keeps listening, as the real CLI does, for the hub's interrupt)."""
    try: return _lines.get(timeout=timeout)
    except queue.Empty: return ""
interrupted = False
while True:
    line = readline()
    if line is None: break
    try: r = json.loads(line)
    except Exception: continue
    if r.get("type") != "user":
        if r.get("type") == "control_request":
            out({"type": "control_response", "response": {"request_id": r.get("request_id"), "subtype": "success", "response": {}}})
        continue
    n += 1
    c = r.get("message", {}).get("content", "")
    text = c if isinstance(c, str) else " ".join(x.get("text", "") for x in c if isinstance(x, dict))
    L("TURN %d %s" % (os.getpid(), text[:300].replace("\n", " ")))
    out({"type": "system", "subtype": "init", "session_id": sid, "cwd": os.getcwd(), "tools": ["Bash"], "model": "fake", "permissionMode": "default"})
    say = "done"
    def perm(tool, inp):
        d = mcp("tools/call", {"name": "permission_prompt", "arguments": {"tool_name": tool, "input": inp}})
        t = d.get("result", {}).get("content", [{}])[0].get("text", "")
        L("PERM %s -> %s" % (tool, t[:300].replace("\n", " ")))
        return t
    try:
        if "zqask" in text:
            say = "answer: " + perm("AskUserQuestion", {"questions": [{"question": "Which port?", "header": "Port", "options": [{"label": "USB-C", "description": ""}, {"label": "Barrel", "description": ""}], "multiSelect": False}]})[:200]
        elif "zqapprove" in text:
            say = "approval: " + perm("Bash", {"command": "echo approved-by-person", "description": "say it"})[:200]
        elif "zqplan" in text:
            say = "plan: " + perm("ExitPlanMode", {"plan": "1. Draw the board\n2. Route it"})[:200]
        elif "zqtool" in text:
            name = text.split("zqtool", 1)[1].split()[0]
            d = mcp("tools/call", {"name": name, "arguments": {}})
            t = d.get("result", {}).get("content", [{}])[0].get("text", "")
            L("TOOL %s -> %s" % (name, t[:400].replace("\n", " ")))
            say = "tool %s said: %s" % (name, t[:200])
        elif "zqquote" in text:
            d = mcp("tools/call", {"name": "order_quote", "arguments": {"dir": text.split("zqquote", 1)[1].split()[0]}})
            t = d.get("result", {}).get("content", [{}])[0].get("text", "")
            L("TOOL order_quote -> %s" % t[:400].replace("\n", " "))
            say = "quote: " + t[:200]
        elif "zqwrite" in text:
            name = text.split("zqwrite", 1)[1].split()[0]
            open(os.path.join(os.getcwd(), name), "w").write("written by turn %d\n" % n); L("WROTE %s in %s" % (name, os.getcwd())); say = "wrote " + name
        elif "zqecho" in text:
            say = "echo: " + text
    except Exception as e:
        L("ERR %r" % (e,)); say = "error %r" % (e,)
    if "zqshort" not in text and not any(k in text for k in ["zqask", "zqapprove", "zqplan", "zqtool", "zqquote", "zqecho", "zqwrite"]):
        for i in range(int(float(os.environ.get("FAKE_SECS", "600")) * 10)):
            if i % 10 == 0: out({"type": "stream_event", "event": {"type": "content_block_delta", "index": 0, "delta": {"type": "text_delta", "text": "working "}}, "session_id": sid})
            l2 = readline(0.1)
            if l2 is None: sys.exit(0)
            if l2:
                try: r2 = json.loads(l2)
                except Exception: continue
                if r2.get("type") == "control_request" and r2.get("request", {}).get("subtype") == "interrupt":   # the hub's Stop: the CLI answers and ends the turn, the process stays
                    out({"type": "control_response", "response": {"request_id": r2.get("request_id"), "subtype": "success", "response": {}}}); L("INTERRUPTED turn %d" % n); interrupted = True; break
    if interrupted:
        interrupted = False; say = "interrupted"; out({"type": "result", "subtype": "error_during_execution", "is_error": True, "result": "interrupted", "session_id": sid}); continue
    out({"type": "assistant", "message": {"id": "m%d-%d" % (os.getpid(), n), "role": "assistant", "content": [{"type": "text", "text": say}], "model": "claude-fake", "usage": {"input_tokens": 1200, "cache_read_input_tokens": 30000, "cache_creation_input_tokens": 500, "output_tokens": 80}}, "session_id": sid})
    out({"type": "result", "subtype": "success", "is_error": False, "result": say, "session_id": sid, "modelUsage": {"claude-fake": {"inputTokens": 1200, "outputTokens": 80, "cacheReadInputTokens": 30000, "cacheCreationInputTokens": 500, "costUSD": 0.01, "contextWindow": 200000}}, "total_cost_usd": 0.01, "uuid": "u-%d-%d" % (os.getpid(), n), "duration_ms": 900, "num_turns": 1, "usage": {"input_tokens": 1200, "cache_read_input_tokens": 30000, "cache_creation_input_tokens": 500, "output_tokens": 80}})
