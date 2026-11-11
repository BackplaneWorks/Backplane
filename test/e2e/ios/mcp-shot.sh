#!/bin/sh
# Settings > MCP servers on the iOS simulator against a temp hub (stand-in agents, loopback, its own home and fake HOME), seeded as
# scripts/desktop-mcp-shot.sh seeds it. The hub runs here on 127.0.0.1; an ssh reverse tunnel gives the Mac mini's loopback the same port, so
# the simulator reaches it as 127.0.0.1 too. Shots: list, open (microchip-docs), error (bench-psu), add (a paste), gallery, import, dark and light.
# Run under the mac-heavy lock, after scripts/ios-build.sh (ADHOC=1 DATA=1). usage: mcp-shot.sh HUB_BIN OUTDIR   -> OUTDIR/mcp-<theme>-<view>.png
set -eu
HUBBIN=$(realpath "$1"); OUT=$(mkdir -p "$2" && realpath "$2")
ROOT=$(cd "$(dirname "$0")/../../.." && pwd); M=$ROOT/build/gpui/mac-ssh
. "$ROOT/scripts/lib-bptmp.sh"
R=$(mktemp -d "$BP_TMP/mcpios.XXXXXX"); PORT=${PORT:-3873}; SIM=${SIM_NAME:-bp-mcp-ios}
mkdir -p "$R/home" "$R/proj" "$R/bin" "$R/fh/.codex" "$R/fh/.cache"
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
PY
HUBPID=; TUN=
cleanup() { [ -n "$TUN" ] && kill $TUN 2>/dev/null; [ -n "$HUBPID" ] && kill $HUBPID 2>/dev/null; sleep 1; cp "$R/hub.log" "$OUT/mcp-ios.hub.log" 2>/dev/null; rm -rf "$R"; }
trap cleanup EXIT INT TERM
( export BACKPLANE_AGENTS=$R/bin BACKPLANE_NO_UPDATE=1 BACKPLANE_VAULT_KEYRING=file HOME=$R/fh XDG_CACHE_HOME=$R/fh/.cache XDG_CONFIG_HOME=$R/fh/.config XDG_DATA_HOME=$R/fh/.local
  unset DISPLAY WAYLAND_DISPLAY; cd $R/proj; exec "$HUBBIN" --home $R/home --port $PORT --host 127.0.0.1 --no-tailscale --foreground ) > $R/hub.log 2>&1 &
HUBPID=$!
ok=; for k in $(seq 1 60); do [ -s $R/home/token ] && curl -s -m3 -H "Authorization: Bearer $(cat $R/home/token)" http://127.0.0.1:$PORT/client/capabilities | grep -q mcpsettings && { ok=1; break; }; sleep 0.5; done
[ -n "$ok" ] || { echo "the hub does not list mcpsettings"; tail -5 $R/hub.log; exit 1; }
"$M" -N -o ExitOnForwardFailure=yes -R 127.0.0.1:$PORT:127.0.0.1:$PORT hs-mac-mini &
TUN=$!; sleep 3
python3 - "$R" <<'PY'
import json,sys
R=sys.argv[1]
PASTE='{\n  "mcpServers": {\n    "stm32": {\n      "command": "uvx",\n      "args": ["--from", "git+https://github.com/creativec09/stm32.git", "stm32-mcp-docs"]\n    },\n    "context7": {\n      "type": "http",\n      "url": "https://mcp.context7.com/mcp",\n      "headers": { "Authorization": "Bearer ${CONTEXT7_API_KEY}" }\n    }\n  }\n}'
shots=[("list",{},{}),("open",{"view":"open:microchip-docs"},{}),("error",{"view":"open:bench-psu"},{}),("add",{"view":"add"},{"mpaste":PASTE}),("gallery",{"view":"gallery"},{}),("import",{"view":"import"},{})]
open(R+"/shots.txt","w").write("".join(n+"\t"+json.dumps(u)+"\t"+json.dumps(f)+"\n" for n,u,f in shots))
PY
TOK=$(cat $R/home/token)
RSYNC_RSH=$M rsync -a $R/shots.txt hs-mac-mini:bp-gpui-ios/mcp-shots.txt
printf '%s' "$TOK" | "$M" hs-mac-mini 'umask 077; cat > ~/bp-gpui-ios/mcp-token'
"$M" hs-mac-mini 'export DEVELOPER_DIR=/Applications/Xcode.app/Contents/Developer
SIM='"$SIM"'; PORT='"$PORT"'; APP=$HOME/bp-gpui-ios/ios/build/dd/Build/Products/Debug-iphonesimulator/Backplane.app; O=$HOME/bp-gpui-ios/mcp-out
rm -rf $O; mkdir -p $O
U=$(xcrun simctl list devices | grep " $SIM (" | head -1 | grep -oE "[0-9A-F-]{36}")
[ -n "$U" ] || U=$(xcrun simctl create $SIM com.apple.CoreSimulator.SimDeviceType.iPhone-17-Pro)
xcrun simctl boot $U 2>/dev/null; xcrun simctl bootstatus $U -b >/dev/null 2>&1
xcrun simctl status_bar $U override --time 9:41 --batteryState charged --batteryLevel 100 --wifiBars 3 --cellularBars 4 2>/dev/null
xcrun simctl terminate $U dev.backplane.gpui >/dev/null 2>&1; xcrun simctl install $U "$APP" || exit 1
TOK=$(cat ~/bp-gpui-ios/mcp-token)
for theme in dark light; do
  xcrun simctl ui $U appearance $theme
  while IFS="	" read -r name ui fields; do
    xcrun simctl terminate $U dev.backplane.gpui >/dev/null 2>&1
    SIMCTL_CHILD_BACKPLANE_LINK="http://127.0.0.1:$PORT/#token=$TOK" SIMCTL_CHILD_BACKPLANE_ACTS=ob-skip SIMCTL_CHILD_BACKPLANE_SELECT=@mcp \
    SIMCTL_CHILD_BACKPLANE_MCP_UI="$ui" SIMCTL_CHILD_BACKPLANE_MCP_FIELDS="$fields" xcrun simctl launch $U dev.backplane.gpui >/dev/null
    sleep ${WAIT:-9}
    xcrun simctl io $U screenshot $O/mcp-$theme-$name.png >/dev/null 2>&1
    echo "shot $theme $name"
  done < ~/bp-gpui-ios/mcp-shots.txt
done
xcrun simctl terminate $U dev.backplane.gpui >/dev/null 2>&1; xcrun simctl shutdown $U; rm -f ~/bp-gpui-ios/mcp-token'
RSYNC_RSH=$M rsync -a hs-mac-mini:bp-gpui-ios/mcp-out/ "$OUT/"
echo "shots in $OUT"
