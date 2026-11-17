"""What argv an agent CLI installed as an npm .cmd shim receives from the hub on Windows (the --mcp-config question, Nov 17).

  python win_argv_probe.py HUB_EXE AGENTS_DIR OUT [PORT]

AGENTS_DIR holds the real npm shims (claude.cmd ...). NODE_OPTIONS loads argv_logger.js into the node the shim starts: it writes the
process's argv to OUT\\argv-<pid>.json and exits, so no account and no network are needed. A thread turn is started through the hub.
"""
import json, os, sys, time, glob
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import win_embed_e2e as E

hub_exe, agents, out = sys.argv[1:4]
port = int(sys.argv[4]) if len(sys.argv) > 4 else 3891
os.makedirs(out, exist_ok=True)
logger = os.path.join(out, "argv_logger.js")
with open(logger, "w") as f:
    f.write("require('fs').writeFileSync(require('path').join(%s, 'argv-' + process.pid + '.json'), JSON.stringify(process.argv));process.exit(0);\n" % json.dumps(out))
os.environ["BP_AGENTS_DIR"] = agents
os.environ["NODE_OPTIONS"] = "--require " + logger.replace("\\", "/")
h = E.SuiteHarness("argv_probe", hub_exe, hub_exe, out, out, out, port)
h.init_dirs()
h.start_hub(embed_mode="fork")
time.sleep(3)
print(h.hub_rpc([{"method": "turn.start", "params": {"thread": "t1", "text": "hello", "msg": f"argv-{time.time_ns()}"}}])[:300], flush=True)
end = time.time() + 60
while time.time() < end and not glob.glob(os.path.join(out, "argv-*.json")):
    time.sleep(1)
time.sleep(3)
for f in sorted(glob.glob(os.path.join(out, "argv-*.json"))):
    a = json.load(open(f))
    if "--mcp-config" in a:
        i = a.index("--mcp-config")
        print("ARGV_MCP " + json.dumps({"file": os.path.basename(f), "mcp_config": a[i + 1] if i + 1 < len(a) else None, "argc": len(a)}), flush=True)
    else:
        print("ARGV_OTHER " + json.dumps({"file": os.path.basename(f), "args": a[2:12]}), flush=True)
h.cleanup()
