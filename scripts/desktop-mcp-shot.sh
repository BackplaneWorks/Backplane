#!/bin/sh
# Settings > MCP on a temp home (stand-in agents, loopback, an isolated Xvfb display, its own scope): a seeded list (ready, needs a key,
# an error, off), a sample ~/.codex/config.toml and ~/.claude.json to import, and shots of the list, an opened card, Add with a paste,
# the gallery and Import. THEME=dark|light. usage: desktop-mcp-shot.sh HUB_BIN CLIENT_BIN OUTDIR [NAME]  -> OUTDIR/NAME-<view>.png
set -eu
HUBBIN=$(realpath "$1"); CLIENT=$(realpath "$2"); OUT=$(mkdir -p "$3" && realpath "$3"); N=${4:-mcp-${THEME:-dark}}
ROOT=$(cd "$(dirname "$0")/.." && pwd)
. "$ROOT/scripts/lib-bptmp.sh"
XV=${XVFB_ROOT:-$HOME/.local/xvfb-root}
R=$(mktemp -d "$BP_TMP/mcpshot.XXXXXX"); PORT=${PORT:-3871}; DISP=:${GPUI_DISP:-77}
mkdir -p "$OUT" "$R/home" "$R/proj" "$R/bin" "$R/fh/.codex" "$R/fh/.cache"
[ ! -e /tmp/.X11-unix/X${GPUI_DISP:-77} ] || { echo "display $DISP is in use"; exit 2; }
for n in claude codex grok muse; do printf '#!/bin/sh\nexit 1\n' > "$R/bin/$n"; chmod +x "$R/bin/$n"; done
python3 - "$R" <<'PY'
import json,sys,time
R=sys.argv[1]; NOW=int(time.time())
ev=[{"$":"ProjectCreated","id":"p1","title":"esp32-motor","root":R+"/proj","at":NOW},{"$":"ProjectCreated","id":"p2","title":"usb-pd-sink","root":R+"/proj","at":NOW},
    {"$":"ThreadCreated","id":"t1","project":"p1","title":"motor driver","env":"local","branch":"","worktree":"","provider":"claude","model":"","at":NOW}]
open(R+"/home/events.jsonl","w").write("".join(json.dumps(e,separators=(",",":"))+"\n" for e in ev))
S=lambda **k: dict({"enabled":True,"providers":[],"projects":[],"disabled_tools":[],"env":[],"headers":[],"args":[]},**k)
servers=[
 S(id="microchip-docs",name="Microchip MPLAB Docs",transport="http",url="https://ai.microchip.com/tools/mplabdocs/mcp",source="gallery:microchip-docs",icon="microchip-docs",added_at=NOW-86400),
 S(id="bench-psu",name="Bench PSU",transport="stdio",command="/opt/lab/psu-mcp",args=["--port","/dev/ttyUSB0"],source="manual",projects=["p1"],added_at=NOW-60),
 S(id="stm32-docs",name="STM32 Docs",transport="stdio",command="uvx",args=["--from","git+https://github.com/creativec09/stm32.git","stm32-mcp-docs"],source="gallery:stm32-docs",icon="stm32-docs",providers=["claude","codex"],added_at=NOW-7200),
 S(id="github",name="GitHub",transport="http",url="https://api.githubcopilot.com/mcp/",headers=[{"k":"Authorization","v":"mcp-github-authorization","secret":True,"pre":"Bearer "}],source="gallery:github",icon="github",added_at=NOW-600),
 S(id="everything",name="Everything",transport="stdio",command="npx",args=["-y","@modelcontextprotocol/server-everything"],source="gallery:everything",added_at=NOW-300,disabled_tools=["longRunningOperation"]),
 S(id="fetch",name="",transport="stdio",command="uvx",args=["mcp-server-fetch"],source="paste",enabled=False,added_at=NOW-30),
]
import os; os.makedirs(R+"/home/mcp",exist_ok=True)
json.dump({"v":1,"servers":servers},open(R+"/home/mcp/servers.json","w"),indent=1)
tools=lambda *n:[{"name":x,"description":d} for x,d in n]
st={
 "microchip-docs":{"report":{"ok":True,"at":NOW-3000,"ms":412,"server":"ProductInfoMcpServer","version":"1.0.0.0","tools":tools(("search_documentation","Semantic search over Microchip MCU documentation"),("get_document","A document section by its id"),("list_families","MCU and MPU families")),"resources":0,"prompts":0},"used_at":NOW-240,"used_tool":"search_documentation","used_n":14},
 "stm32-docs":{"report":{"ok":True,"at":NOW-2000,"ms":2310,"server":"stm32-mcp-docs","version":"0.3.1","tools":tools(("search_stm32_docs","Semantic search over STM32 reference manuals"),("get_peripheral","Registers of one peripheral"),("list_devices","STM32 parts in the index"),("errata","Known errata for a part")),"resources":2,"prompts":0},"used_at":NOW-1800,"used_tool":"get_peripheral","used_n":3},
 "everything":{"report":{"ok":True,"at":NOW-200,"ms":1380,"server":"example-servers/everything","version":"2.3.0","tools":tools(("echo","Echoes back the input"),("add","Adds two numbers"),("longRunningOperation","Demonstrates a long running operation with progress updates"),("printEnv","Prints the environment"),("sampleLLM","Samples from an LLM using MCP's sampling feature"),("getTinyImage","Returns the MCP_TINY_IMAGE")),"resources":100,"prompts":3}},
 "bench-psu":{"report":{"ok":False,"at":NOW-90,"ms":38,"error":"it exited with code 1 before it answered","stderr":"psu-mcp: opening /dev/ttyUSB0\nError: Permission denied (os error 13)\nhint: add yourself to the dialout group","tools":[],"resources":0,"prompts":0}},
}
json.dump(st,open(R+"/home/mcp/state.json","w"))
open(R+"/fh/.codex/config.toml","w").write('model = "gpt-6"\n\n[mcp_servers.context7]\ncommand = "npx"\nargs = ["-y", "@upstash/context7-mcp"]\n\n[mcp_servers.fetch]\ncommand = "uvx"\nargs = ["mcp-server-fetch"]\n\n[mcp_servers.deepwiki]\nurl = "https://mcp.deepwiki.com/mcp"\n')
json.dump({"numStartups":12,"mcpServers":{"kicad":{"type":"stdio","command":"uvx","args":["kicad-mcp-pro","--transport","stdio"],"env":{}}},
  "projects":{"/home/x/esp32-motor":{"mcpServers":{"esp-registry":{"type":"http","url":"https://components.espressif.com/mcp"},"jlc":{"command":"node","args":["jlc.js"],"env":{"JLC_API_KEY":"jlc-live-0123456789"}}}}}},open(R+"/fh/.claude.json","w"))
PASTE='{\n  "mcpServers": {\n    "stm32": {\n      "command": "uvx",\n      "args": ["--from", "git+https://github.com/creativec09/stm32.git", "stm32-mcp-docs"]\n    },\n    "context7": {\n      "type": "http",\n      "url": "https://mcp.context7.com/mcp",\n      "headers": { "Authorization": "Bearer ${CONTEXT7_API_KEY}" }\n    }\n  }\n}'
st=[{"select":0},{"wait":1500},{"click":"ui:Settings"},{"wait":2500},{"click":"set-nav:mcp"},{"wait":2500},{"mark":"list"},{"wait":1200},
    {"action":["mcp-view","open:microchip-docs"]},{"wait":1800},{"mark":"open"},{"wait":1200},
    {"action":["mcp-view","open:bench-psu"]},{"wait":1800},{"mark":"error"},{"wait":1200},
    {"action":["mcp-view","add"]},{"wait":1200},{"action":["bfield","mpaste\u001f"+PASTE]},{"wait":2200},{"mark":"add"},{"wait":1200},
    {"action":["mcp-view","gallery"]},{"wait":1800},{"mark":"gallery"},{"wait":1200},
    {"action":["mcp-view","import"]},{"wait":3500},{"mark":"import"},{"wait":1200}]
json.dump(st,open(R+"/script.json","w"))
PY
export LC_ALL=C
systemd-run --user --scope -q -- nice -n 10 sh -c '
set -eu
R='"$R"'; HUBBIN='"$HUBBIN"'; CLIENT='"$CLIENT"'; OUT='"$OUT"'; N='"$N"'; PORT='"$PORT"'; DISP='"$DISP"'; XV='"$XV"'; ROOT='"$ROOT"'
cleanup() { for f in client i3 xvfb hub; do [ -f $R/$f.pid ] && kill $(cat $R/$f.pid) 2>/dev/null || true; done; sleep 1; }
trap cleanup EXIT INT TERM
export BACKPLANE_AGENTS=$R/bin BACKPLANE_NO_UPDATE=1 BACKPLANE_VAULT_KEYRING=file HOME=$R/fh XDG_CACHE_HOME=$R/fh/.cache XDG_CONFIG_HOME=$R/fh/.config XDG_DATA_HOME=$R/fh/.local
cd $R/proj
$HUBBIN --home $R/home --port $PORT --host 127.0.0.1 --no-tailscale --foreground > $R/hub.log 2>&1 &
echo $! > $R/hub.pid
for k in $(seq 1 40); do curl -s -m2 -o /dev/null http://127.0.0.1:$PORT/hello && break; sleep 0.5; done
LD_LIBRARY_PATH=$XV/usr/lib/x86_64-linux-gnu $XV/usr/bin/Xvfb $DISP -screen 0 1440x940x24 -nolisten tcp -xkbdir /usr/share/X11/xkb > $R/xvfb.log 2>&1 &
echo $! > $R/xvfb.pid; sleep 2
printf "font pango:monospace 8\nnew_window none\nfor_window [class=\".*\"] border none\ndefault_border none\nhide_edge_borders both\n" > $R/i3.conf
DISPLAY=$DISP i3 -c $R/i3.conf > $R/i3.log 2>&1 &
echo $! > $R/i3.pid; sleep 1
export DISPLAY=$DISP VK_ICD_FILENAMES=/usr/share/vulkan/icd.d/lvp_icd.json LP_NUM_THREADS=2 BACKPLANE_GPUI_ASSETS=$ROOT/assets BACKPLANE_GPUI_RECTS=1
$CLIENT --url http://127.0.0.1:$PORT --token-file $R/home/token --script $R/script.json --exit-after 90 --theme ${THEME:-dark} > $R/client.log 2>&1 &
CP=$!; echo $CP > $R/client.pid
for m in list open error add gallery import; do
  for k in $(seq 1 120); do grep -q "\"name\":\"$m\"" $R/client.log && break; sleep 0.5; done
  sleep 0.6; import -window root $OUT/$N-$m.png
done
kill $CP 2>/dev/null || true; wait $CP 2>/dev/null || true
cp $R/hub.log $OUT/$N.hub.log; cp $R/client.log $OUT/$N.client.log
'
rm -rf "$R"
echo "shots in $OUT"
