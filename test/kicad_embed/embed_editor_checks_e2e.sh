#!/bin/bash
# Suite: DRC and ERC in the open editor (native/host/src/kicad/edchecks.rs; Backplane KiCad bp.13 `embed-checks`: RunChecks, GetMarkers,
# SelectItems by id and place, ET_CHECKS_DONE).
#   K1  before KiCad opens, the checks run with kicad-cli (the saved files): the baseline rows and counts
#   K2  with the board open in the embedded editor, both checks run there (summary in_editor), with the CLI's rows: the same ids, the
#       same counts (nothing unsaved yet); the bar follows KiCad's ET_PROGRESS while DRC runs
#   K3  an edit in KiCad, not saved (a track added over its API, as the person's tools do): DRC runs again by itself, in the editor, on
#       the unsaved board (unsaved true, "live, unsaved"); the earlier rows keep their ids, the new track's finding joins; the board file
#       on disk is unchanged
#   K4  a focus on a finding with board items selects those exact items in KiCad (SelectItems ids, the box round them)
#   K5  (informational) a click on one of KiCad's own DRC markers in the canvas (XTest; the view centred on the marker by SelectItems at_nm and a box,
#       the marker found by its colour near the centre) shows its row: info review.clicked.<digest> names the finding of its items
# CHECKS_BOARD=<a project folder> runs it on a copy of that project instead of the fixture (Hubble_DK, Collar).
# Copies only (seed_project.py). Fork mode; skips when the embedded editor's GetCapabilities lacks embed-checks.
set -euo pipefail
HERE=$(cd "$(dirname "$0")" && pwd)
# shellcheck source=lib/env_xvfb.sh
. "$HERE/lib/env_xvfb.sh"
[ "$EMBED_MODE" = "fork" ] || { echo "embed_editor_checks_e2e: fork only"; exit 0; }
[ "${1:-}" != "" ] && CLIENT_BIN=$1
[ "${2:-}" != "" ] && HUB_BIN=$2
[ "${3:-}" != "" ] && OUT=$(mkdir -p "$3" && cd "$3" && pwd)
discover_bins
[ -x "$REAL_HOME/.bun/bin/bun" ] && PATH="$REAL_HOME/.bun/bin:$PATH"
discover_kicad
init_isolated_dirs
python3 "$SEED_PY" "$OUT" "$REPO_ROOT"
PROJ="$OUT/proj"
if [ -n "${CHECKS_BOARD:-}" ]; then
  ( cd "$PROJ" && git rm -q -r --cached . >/dev/null && find . -mindepth 1 -maxdepth 1 ! -name .git -exec rm -rf {} + )
  rsync -a --exclude '*backups*' --exclude '~*.lck' --exclude '.git' --exclude 'mfg' --exclude 'build' "$CHECKS_BOARD"/ "$PROJ"/
  echo "{\"project\": \"$(basename "$(ls "$PROJ"/*.kicad_pro | head -1)")\"}" >"$PROJ/.backplane.json"
  git -C "$PROJ" add -A >/dev/null 2>&1; git -C "$PROJ" commit -qm "the board" >/dev/null 2>&1 || true
fi
PRO=$(ls "$PROJ"/*.kicad_pro | head -1); PCB="${PRO%.kicad_pro}.kicad_pcb"; mkdir -p "$OUT/shots"; export PRO OUT HUB_RPC_MJS
# where the test track goes: through the board's first via (a track of no net across a net's via is a short on any board), else the
# middle of the outline
read -r TX TY < <(python3 - "$PCB" <<'PY'
import re, sys
t = open(sys.argv[1]).read()
m = re.search(r"\n\t\(via\b.*?\(at ([-\d.]+) ([-\d.]+)\)", t, re.S)
if m: print(m.group(1), m.group(2)); sys.exit()
xs, ys = [], []
for it in re.split(r"\n\t\(", t):
    if re.match(r"gr_(line|rect|arc|poly)\b", it) and '"Edge.Cuts"' in it:
        for x, y in re.findall(r"\((?:start|end|mid|xy) ([-\d.]+) ([-\d.]+)\)", it): xs.append(float(x)); ys.append(float(y))
print((min(xs) + max(xs)) / 2 if xs else 140, (min(ys) + max(ys)) / 2 if ys else 95)
PY
)
echo "INFO board $(basename "$PCB"), test track at ($TX, $TY)"
start_xvfb_and_wm
export BP_STREAM_N=1 BP_STREAM_MS=10
# the stand-in logs each MCP answer whole (GetMarkers on a real board is long)
export BP_STANDIN_MCPCALL_MAX=4000000
# KiCad's events in the hub log (K5 reads ET_MARKER_CLICKED there)
export BACKPLANE_KICAD_DEBUG=1
export BACKPLANE_HUB_TIMING=1
start_hub
export HPORT

watch() { hub_rpc "[{\"method\":\"kicad.checks.watch\",\"params\":{\"project\":\"$PRO\"}}]" 2>/dev/null | grep -v '^INFO ' | head -1; }
# one source's findings, all of them, paged, into a file whose path it prints (a real board's list is too big for one argument)
list() { python3 - "$1" <<'PY'
import json, subprocess, sys, os
out, off = [], 0
while True:
    steps = json.dumps([{"method": "kicad.checks.list", "params": {"project": os.environ["PRO"], "source": sys.argv[1], "offset": off, "limit": 25}}])
    p = subprocess.run(["bun", os.environ["HUB_RPC_MJS"], os.environ["HPORT"], os.environ["OUT"] + "/home/token", steps], capture_output=True, text=True)
    r = p.stdout
    line = next((l for l in r.splitlines() if l.startswith("{")), "{}")
    with open(os.environ["OUT"] + "/list_pages.log", "a") as f: f.write(f"{sys.argv[1]} offset {off}: {len(r)} bytes, rc {p.returncode}, {line[:300]} | {p.stderr[-300:]}\n")
    res = (json.loads(line).get("result") or {})
    fs = res.get("findings") or []
    out += fs; off += len(fs)
    if not fs or off >= res.get("total", 0): break
path = f'{os.environ["OUT"]}/list-{sys.argv[1]}-{len(os.listdir(os.environ["OUT"]))}.json'
json.dump({"result": {"findings": out, "total": len(out)}}, open(path, "w")); print(path)
PY
}
ids() { python3 -c 'import json,sys; print(" ".join(sorted(f["id"] for f in json.load(open(sys.argv[1]))["result"]["findings"])))' "$1"; }
# wait until both checks are fresh and the python test on the summary ($1) holds; prints the summary
wait_summary() {
  local s=""
  for _ in $(seq "${2:-480}"); do
    s=$(watch)
    python3 -c 'import json,sys; r=json.loads(sys.argv[1])["result"]; d,e=r["drc"],r["erc"]; sys.exit(0 if d["state"]=="fresh" and e["state"]=="fresh" and ('"$1"') else 1)' "$s" 2>/dev/null && { echo "$s"; return 0; }
    sleep 0.5
  done
  echo "$s"; return 1
}

# K1: the CLI's baseline, KiCad not open yet
S0=$(wait_summary 'not d["in_editor"]' 240) || true
L0D=$(list drc); L0E=$(list erc)
# lane J's DRC trust: a runtime without drc-zone-guard on a board with fills says DRC may miss a short (stockdrc); one with it does not
echo "INFO DRC untrusted (no drc-zone-guard): $(python3 -c 'import json,sys; print(json.loads(sys.argv[1])["result"]["drc"].get("untrusted"))' "$S0" 2>/dev/null)"
check "Before KiCad opens: both checks ran with kicad-cli and are fresh" "$(python3 -c 'import json,sys; r=json.loads(sys.argv[1])["result"]; print(int(r["drc"]["has_result"] and not r["drc"]["in_editor"] and r["erc"]["has_result"]))' "$S0" 2>/dev/null || echo 0)" "$S0"

# the board open in the embedded editor; the Schematic tab pressed once, then Board: checks go to KiCad only once its schematic editor
# is open (kicad/checkrun.rs; the pre-warm opens it only with memory to spare)
cat >"$OUT/script.json" <<'JSON'
[{"select":0},{"wait":2000},{"click":"rail:Board"},{"wait":2500},{"click":"ui:Edit"},
 {"kicad":{"wait_painted":60000,"tag":"painted"}},{"wait":3000},{"click":"kicad-tab:SCH"},{"wait":6000},{"click":"kicad-tab:PCB"},{"wait":4000},{"mark":"ready"},{"kicad":{"report":true,"tag":"ready"}},{"wait":200000}]
JSON
HOME="$OUT/fh" BACKPLANE_GPUI_ASSETS="$ASSETS_DIR" BACKPLANE_GPUI_RECTS=1 BACKPLANE_GPUI_HOLD=1 \
"$CLIENT_BIN" --url "http://127.0.0.1:$HPORT" --token-file "$OUT/home/token" --script "$OUT/script.json" --exit-after 260 >"$OUT/client.log" 2>&1 &
CP=$!; track_pid "$CP"
wait_for_log "$OUT/client.log" '"name":"ready","event":"mark"' 480 || { echo "FAIL the board never opened in KiCad"; tail -n 20 "$OUT/client.log"; exit 1; }
sleep 1; SOCK=$(python3 -c 'import json,sys; print(json.loads(sys.argv[1] or "{}").get("sock") or "")' "$(last_kicad_report "$OUT/client.log" ready)")
ask() {
  local N; N="$1-$(date +%s%N)"; local before; before=$(grep -c "^MCPCALL" "$OUT/standin.log" 2>/dev/null || true)
  local text; text=$(python3 -c 'import json,sys; print(json.dumps("mcpcall:" + json.dumps({"name": sys.argv[1], "arguments": json.loads(sys.argv[2])})))' "$1" "$2")
  hub_rpc "[{\"method\":\"turn.start\",\"params\":{\"thread\":\"t1\",\"text\":$text,\"msg\":\"medc$N\"}}]" >>"$OUT/rpc.log" 2>&1 || true
  for _ in $(seq 480); do [ "$(grep -c "^MCPCALL" "$OUT/standin.log" 2>/dev/null || true)" -gt "$before" ] && break; sleep 0.25; done
  sleep 2
  grep "^MCPCALL $1 " "$OUT/standin.log" | tail -1 || true
}
# the embedded editor's own capabilities, from its socket (the stand-in agent's log cuts long answers)
CAPS=$(python3 "$IPC_PY" caps "$SOCK" 2>>"$OUT/ipc.err" || echo '{}'); echo "INFO editor capabilities: $(echo "$CAPS" | grep -o '"embed[^"]*"' | tr '\n' ' ')"
echo "$CAPS" | grep -q 'names' || { echo "FAIL the embedded KiCad's GetCapabilities gave nothing on $SOCK: $(cut -c1-200 "$OUT/ipc.err" 2>/dev/null)"; exit 1; }
echo "$CAPS" | grep -q 'embed-checks' || { echo "SKIP embed_editor_checks_e2e: the embedded KiCad has no embed-checks capability ($(echo "$CAPS" | cut -c1-160))"; finish_suite "embed_editor_checks_e2e ($EMBED_MODE)"; exit 0; }

# BACKPLANE_CHECKS_IN_KICAD=0 (the hub's switch): with the board open in KiCad, a run still goes to kicad-cli, and KiCad never runs a check
if [ "${BACKPLANE_CHECKS_IN_KICAD:-}" = "0" ]; then
  hub_rpc "[{\"method\":\"kicad.checks.run\",\"params\":{\"project\":\"$PRO\"}}]" >>"$OUT/rpc.log" 2>&1 || true
  SW=$(wait_summary 'd["at"] > 0' 240) || true; sleep 2; SW=$(watch)
  check "BACKPLANE_CHECKS_IN_KICAD=0: with KiCad open the checks ran with kicad-cli (not in_editor), none in KiCad" "$(python3 -c 'import json,sys; r=json.loads(sys.argv[1])["result"]; print(int(r["drc"]["has_result"] and not r["drc"]["in_editor"] and not r["erc"]["in_editor"]))' "$SW" 2>/dev/null || echo 0)$( grep -q 'in KiCad starts' "$OUT/hub.log" && echo x)" "$SW"
  finish_suite "embed_editor_checks_e2e ($EMBED_MODE)"; exit 0
fi
# K2: both checks in the editor, sampled for KiCad's progress while DRC runs
hub_rpc "[{\"method\":\"kicad.checks.run\",\"params\":{\"project\":\"$PRO\"}}]" >>"$OUT/rpc.log" 2>&1 || true
FRACS=""
for _ in $(seq 240); do
  s=$(watch); f=$(python3 -c 'import json,sys; d=json.loads(sys.argv[1])["result"]["drc"]; print(d["fraction"] if d["running"] else "")' "$s" 2>/dev/null || true)
  [ -n "$f" ] && FRACS="$FRACS $f"
  python3 -c 'import json,sys; r=json.loads(sys.argv[1])["result"]; sys.exit(0 if r["drc"]["in_editor"] and r["erc"]["in_editor"] and r["drc"]["state"]=="fresh" and r["erc"]["state"]=="fresh" else 1)' "$s" 2>/dev/null && break
  sleep 0.25
done
S1=$(wait_summary 'd["in_editor"] and e["in_editor"]' 60) || true
L1D=$(list drc); L1E=$(list erc)
check "In KiCad: both checks ran in the open editor (in_editor), nothing unsaved" "$(python3 -c 'import json,sys; r=json.loads(sys.argv[1])["result"]; print(int(r["drc"]["in_editor"] and r["erc"]["in_editor"] and not r["drc"]["unsaved"]))' "$S1" 2>/dev/null || echo 0)" "$S1"
sub() { python3 -c 'import sys; a=set(sys.argv[1].split()); b=set(sys.argv[2].split()); print(len(a-b))' "$1" "$2"; }
check "Every CLI row is in the editor's result with the same id (DRC $(ids "$L0D" | wc -w), ERC $(ids "$L0E" | wc -w))" "$([ "$(sub "$(ids "$L0D")" "$(ids "$L1D")")" = 0 ] && [ "$(sub "$(ids "$L0E")" "$(ids "$L1E")")" = 0 ] && echo 1 || echo 0)" "missing in the editor: drc $(sub "$(ids "$L0D")" "$(ids "$L1D")"), erc $(sub "$(ids "$L0E")" "$(ids "$L1E")")"
# rows only the editor reports: none, but on the builds known to differ (bp.13 batch 3, DIFF_BUILDS):
# an expected difference: only the known rules (edchecks EDITOR_ONLY), each with its note, and, where CHECKS_EXPECT_EDITOR_ONLY says how many
# ("silk_over_copper:21" for Hubble_DK), that many; so a fork fix without the capability turns this red, to be told
EO=$(python3 - "$L0D" "$L1D" <<'PY'
import json, sys, collections
a, b = [{f["id"]: f for f in json.load(open(x))["result"]["findings"]} for x in sys.argv[1:3]]
only = [b[k] for k in set(b) - set(a)]
c = collections.Counter(f["rule"] for f in only)
noted = all("editor only" in f.get("detail", "") for f in only)
print(",".join(f"{r}:{n}" for r, n in sorted(c.items())) or "-", int(noted))
PY
)
read -r EO_RULES EO_NOTED <<<"$EO"
# the builds whose editor reports silk_over_copper pairs the project waives in its .kicad_dru (no solder_mask_min_width: the silk test's
# per-opening branch overrode the ignore); fixed for batch 5 (5e9b211e3e), whose parity gate holds editor and CLI to the same findings
DIFF_BUILDS="c53b5e0800 85dde9ab8d"
if ! echo "$CAPS" | grep -qE "\"($(echo $DIFF_BUILDS | tr ' ' '|'))"; then
  check "The editor reports nothing the CLI does not (a build without the known difference; editor only: $EO_RULES)" "$([ "$EO_RULES" = "-" ] && echo 1 || echo 0)"
else
  WANT_EO=${CHECKS_EXPECT_EDITOR_ONLY:-}
  OK_EO=$(python3 -c 'import sys; got=sys.argv[1]; want=sys.argv[2]; known={"silk_over_copper"}
rules=[] if got=="-" else [x.split(":")[0] for x in got.split(",")]
print(int(all(r in known for r in rules) and (not want or want==got) and sys.argv[3]=="1"))' "$EO_RULES" "$WANT_EO" "${EO_NOTED:-0}")
  check "Expected difference (build $DIFF_BUILDS): rows only the editor reports are the known ones, each noted as such ($EO_RULES; expected ${WANT_EO:-any of the known})" "$OK_EO" "a change here means the fork changed its scope: tell lane E"
fi
echo "INFO counts: cli drc $(python3 -c 'import json,sys; print(json.loads(sys.argv[1])["result"]["drc"]["counts"])' "$S0" 2>/dev/null) erc $(python3 -c 'import json,sys; print(json.loads(sys.argv[1])["result"]["erc"]["counts"])' "$S0" 2>/dev/null) | editor drc $(python3 -c 'import json,sys; print(json.loads(sys.argv[1])["result"]["drc"]["counts"])' "$S1" 2>/dev/null) erc $(python3 -c 'import json,sys; print(json.loads(sys.argv[1])["result"]["erc"]["counts"])' "$S1" 2>/dev/null)"
python3 - "$L0D" "$L1D" >"$OUT/drc_diff.txt" <<'PY' || true
import json, sys
a, b = [{f["id"]: f for f in json.load(open(x))["result"]["findings"]} for x in sys.argv[1:3]]
for k in sorted(set(a) - set(b)): print("cli only", k, a[k]["severity"], a[k]["rule"], a[k]["title"][:90])
for k in sorted(set(b) - set(a)): print("editor only", k, b[k]["severity"], b[k]["rule"], b[k]["title"][:90])
PY
echo "INFO DRC rows on one side only: $(wc -l <"$OUT/drc_diff.txt") (drc_diff.txt); by rule: $(awk '{print $1"/"$2" "$5}' "$OUT/drc_diff.txt" | sed 's|/only||' | sort | uniq -c | sort -rn | head -8 | tr '\n' ';')"
info_counts() { echo "INFO counts equal: $1"; }
info_counts "$(python3 -c 'import json,sys; a,b=[json.loads(x)["result"] for x in sys.argv[1:3]]; print(int(a["drc"]["counts"]==b["drc"]["counts"] and a["erc"]["counts"]==b["erc"]["counts"]))' "$S0" "$S1" 2>/dev/null || echo 0)" "$(echo "$S0" | cut -c1-200) / $(echo "$S1" | cut -c1-200)"
echo "INFO DRC's bar while it ran:$FRACS"
echo "INFO in KiCad, ERC first then DRC: $(python3 -c 'import json,sys; r=json.loads(sys.argv[1])["result"]; e,d=r["erc"],r["drc"]; t0=min(e["at"]-e["took_ms"], d["at"]-d["took_ms"]); print("ERC done", e["at"]-t0, "ms after the start (its own", e["took_ms"], "ms), DRC done", d["at"]-t0, "ms (its own", d["took_ms"], "ms); ERC ended before DRC began:", e["at"] <= d["at"]-d["took_ms"]+50)' "$S1" 2>/dev/null)"
echo "INFO times (ms): $(python3 -c 'import json,sys; a,b=[json.loads(x)["result"] for x in sys.argv[1:3]]; print("cli drc", a["drc"]["took_ms"], "erc", a["erc"]["took_ms"], "| editor drc", b["drc"]["took_ms"], "erc", b["erc"]["took_ms"])' "$S0" "$S1" 2>/dev/null)"
echo "INFO ids: drc cli $(ids "$L0D" | wc -w) editor $(ids "$L1D" | wc -w) shared $(comm -12 <(ids "$L0D" | tr ' ' '\n' | sort) <(ids "$L1D" | tr ' ' '\n' | sort) | wc -l); erc cli $(ids "$L0E" | wc -w) editor $(ids "$L1E" | wc -w) shared $(comm -12 <(ids "$L0E" | tr ' ' '\n' | sort) <(ids "$L1E" | tr ' ' '\n' | sort) | wc -l)"
# KiCad sends no progress in a job's first 150 ms: a DRC that quick has no fraction to show
DT=$(python3 -c 'import json,sys; print(json.loads(sys.argv[1])["result"]["drc"]["took_ms"])' "$S1" 2>/dev/null || echo 0)
if [ "${DT:-0}" -lt 600 ]; then echo "INFO DRC in the editor took $DT ms: too quick for KiCad's progress (none in a job's first 150 ms), the bar is not checked"
else check "The bar followed KiCad's progress while DRC ran (a fraction seen)" "$(python3 -c 'import sys; v=[float(x) for x in sys.argv[1].split()]; print(int(any(0 <= x <= 1 for x in v)))' "${FRACS:-}" 2>/dev/null || echo 0)" "fractions:$FRACS"; fi

# K3: an edit in KiCad, not saved
SHA0=$(sha256sum "$PCB" | cut -d' ' -f1)
ADD=$(python3 "$IPC_PY" agent-add-track "$SOCK" "$(python3 -c "print($TX-2)")" "$TY" "$(python3 -c "print($TX+2)")" "$TY" 0.25 3 2>>"$OUT/ipc.err" || echo '{}'); echo "INFO add track: $ADD"
S3=$(wait_summary 'd["in_editor"] and d["unsaved"]' 240) || true
L3D=$(list drc)
check "An unsaved edit in KiCad: DRC ran again in the editor on it (unsaved)" "$(python3 -c 'import json,sys; d=json.loads(sys.argv[1])["result"]["drc"]; print(int(d["in_editor"] and d["unsaved"] and d["at"] > int(sys.argv[2])))' "$S3" "$(python3 -c 'import json,sys; print(json.loads(sys.argv[1])["result"]["drc"]["at"])' "$S1")" 2>/dev/null || echo 0)" "$S3"
NEW=$(python3 -c 'import sys; a=set(sys.argv[1].split()); b=set(sys.argv[2].split()); print(" ".join(sorted(b-a)))' "$(ids "$L1D")" "$(ids "$L3D")")
GONE=$(python3 -c 'import sys; a=set(sys.argv[1].split()); b=set(sys.argv[2].split()); print(" ".join(sorted(a-b)))' "$(ids "$L1D")" "$(ids "$L3D")")
check "The earlier rows keep their ids; the new track's finding joins ($NEW)" "$([ -n "$NEW" ] && [ -z "$GONE" ] && echo 1 || echo 0)" "new: $NEW gone: $GONE"
check "Nothing was written to the board file" "$([ "$(sha256sum "$PCB" | cut -d' ' -f1)" = "$SHA0" ] && echo 1 || echo 0)"
import -window root "$OUT/shots/k3_unsaved.png" 2>/dev/null || true

# K4: a focus selects the finding's exact items in KiCad
FID=$(python3 -c 'import json,sys; fs=json.load(open(sys.argv[1]))["result"]["findings"]; new=set(sys.argv[2].split())
c=[f for f in fs if f["id"] in new and any(a.get("uuid") for a in f["anchors"])] or [f for f in fs if any(a.get("uuid") for a in f["anchors"])]
print(c[0]["id"] if c else "")' "$L3D" "$NEW")
FO=$(hub_rpc "[{\"method\":\"kicad.checks.focus\",\"params\":{\"project\":\"$PRO\",\"id\":\"$FID\"}}]" 2>/dev/null | grep -v '^INFO ' | head -1)
sleep 1
SEL=$(python3 "$IPC_PY" selection "$SOCK" pcb 2>>"$OUT/ipc.err" || echo '{}')
check "A focus on $FID selects its items in KiCad by id (SelectItems ids: $(python3 -c 'import json,sys; print(len(json.loads(sys.argv[1])["result"]["select"]["ids"]))' "$FO" 2>/dev/null || echo 0), selected $(python3 -c 'import json,sys; print(json.loads(sys.argv[1]).get("count",0))' "$SEL" 2>/dev/null || echo 0))" \
  "$(python3 -c 'import json,sys; r=json.loads(sys.argv[1])["result"]; s=json.loads(sys.argv[2]); print(int(r["kicad"] and len(r["select"]["ids"])>0 and r.get("ids_not_found",0)==0 and s.get("count",0)>=1))' "$FO" "$SEL" 2>/dev/null || echo 0)" "$(echo "$FO" | cut -c1-300) / $SEL"
import -window root "$OUT/shots/k4_focus.png" 2>/dev/null || true
# K4 timing: ten focuses on different findings with board items, each step's time in the hub (the reply's timing): where a row's press goes
python3 - "$L3D" >"$OUT/focus_timing.txt" <<'PY' || true
import json, os, subprocess, sys, statistics
fs = [f for f in json.load(open(sys.argv[1]))["result"]["findings"] if any(a.get("uuid") for a in f["anchors"])][:10]
rows = []
for f in fs:
    steps = json.dumps([{"method": "kicad.checks.focus", "params": {"project": os.environ["PRO"], "id": f["id"]}}])
    r = subprocess.run(["bun", os.environ["HUB_RPC_MJS"], os.environ["HPORT"], os.environ["OUT"] + "/home/token", steps], capture_output=True, text=True).stdout
    line = next((l for l in r.splitlines() if l.startswith("{")), "{}")
    t = (json.loads(line).get("result") or {}).get("timing") or []
    if t: rows.append(dict(t))
names = ["found", "ready", "sheets", "shown", "selected"]
for i, n in enumerate(names):
    v = [r[n] - (r[names[i - 1]] if i else 0) for r in rows if n in r and (i == 0 or names[i - 1] in r)]
    if v: print(f"{n}: median {statistics.median(v):.1f} ms, max {max(v):.1f}")
tot = [r["selected"] for r in rows if "selected" in r]
if tot: print(f"total: median {statistics.median(tot):.1f} ms, max {max(tot):.1f} ({len(tot)} focuses)")
PY
echo "INFO focus in the hub (step: own time): $(tr '\n' ';' <"$OUT/focus_timing.txt")"
# K5: KiCad's own marker clicked in the canvas. GetMarkers gives an error marker's place and items; SelectItems centres the view on it in
# a 4 mm box; the marker is KiCad's DRC error colour (default theme rgba(215, 91, 107), blended) nearest the frame's centre.
ask kicad_ipc "{\"path\":\"$PCB\",\"type\":\"kiapi.backplane.embed.GetMarkers\",\"message\":{\"kind\":\"FK_PCB\"}}" >"$OUT/markers.txt"
read -r MX MY MITEMS MTYPE < <(python3 -c 'import json,re,sys
t=open(sys.argv[1]).read(); i=t.find("{"); 
try: o=json.loads(t[i:])
except Exception: print("0 0 - -"); sys.exit()
def find(v):
    if isinstance(v, dict):
        if "markers" in v: return v["markers"]
        for x in v.values():
            r = find(x)
            if r is not None: return r
    if isinstance(v, str) and "markers" in v:
        try: return find(json.loads(v))
        except Exception: return None
    if isinstance(v, list):
        for x in v:
            r = find(x)
            if r is not None: return r
    return None
ms=[m for m in (find(o) or []) if m.get("severity")=="error" and m.get("at_nm") and m.get("items") and not m.get("excluded")]
if not ms: print("0 0 - -"); sys.exit()
# an error marker alone (no other marker within 3 mm), so the arrows near it on the screen are its own
allm=[m for m in (find(o) or []) if m.get("at_nm")]
xy=lambda m: (int(m["at_nm"].get("x_nm",0)), int(m["at_nm"].get("y_nm",0)))
alone=[m for m in ms if all(n is m or (xy(n)[0]-xy(m)[0])**2+(xy(n)[1]-xy(m)[1])**2 > 3000000**2 for n in allm)]
m=(alone or ms)[0]; print(int(m["at_nm"].get("x_nm",0)), int(m["at_nm"].get("y_nm",0)), ",".join(sorted(x["value"] for x in m["items"])), m.get("type",""))' "$OUT/markers.txt")
echo "INFO the marker to click: $MTYPE at ($MX,$MY) nm, items $MITEMS"
if [ "${MX:-0}" != 0 ]; then
  BOX=$(python3 -c 'import sys; x,y=int(sys.argv[1]),int(sys.argv[2]); print("{\"position\":{\"x_nm\":%d,\"y_nm\":%d},\"size\":{\"x_nm\":4000000,\"y_nm\":4000000}}" % (x-2000000, y-2000000))' "$MX" "$MY")
  ask kicad_ipc "{\"path\":\"$PCB\",\"type\":\"kiapi.backplane.embed.SelectItems\",\"message\":{\"kind\":\"FK_PCB\",\"at_nm\":{\"x_nm\":$MX,\"y_nm\":$MY},\"zoom_box_nm\":$BOX}}" >/dev/null
  sleep 1.5
  R=$(last_kicad_report "$OUT/client.log" ready)
  read -r FX FY FW FH < <(python3 -c 'import json,sys; r=json.loads(sys.argv[1]); f=next((f for f in r.get("frames",[]) if f.get("xid")==r.get("shown")),{}); print(f.get("root_x",0), f.get("root_y",0), f.get("w",0), f.get("h",0))' "$R")
  import -window root "$OUT/shots/k5_marker.png" 2>/dev/null || true
  read -r CX CY < <(python3 - "$OUT/shots/k5_marker.png" "$FX" "$FY" "$FW" "$FH" <<'PY'
import sys
import numpy as np
from PIL import Image
f, fx, fy, fw, fh = sys.argv[1], *map(int, sys.argv[2:])
a = np.asarray(Image.open(f).convert("RGB")).astype(int)[fy:fy + fh, fx:fx + fw]
r, g, b = a[..., 0], a[..., 1], a[..., 2]
m = (r > 140) & (r - g > 60) & (b > g) & (r - b > 40) & (b < 170)
# the view is centred on the marker; its arrows are drawn from the tip (the marker's place) down and to the right. In a window round the
# canvas's middle, the marker pink's top-left-most pixel is the tip: the click goes a little into the arrow from there
# (KiCad's canvas sits centred across the frame and 45 px below its middle: menus and toolbars above, the status bar below; Collar, bp.13)
cx, cy = fw // 2 + 1, fh // 2 + 45
x0, y0 = cx - 40, cy - 40
win = m[y0:cy + 40, x0:cx + 40]
ys, xs = np.nonzero(win)
if len(xs) < 10: print("0 0"); sys.exit()
t = np.argmin(xs + ys)
print(int(xs[t]) + x0 + fx + 9, int(ys[t]) + y0 + fy + 9)
PY
)
  echo "INFO the marker on screen: ($CX,$CY)"
  if [ "${CX:-0}" != 0 ]; then
    # the keyboard to KiCad first (a click on the canvas away from the marker), then the marker itself
    python3 "$XINPUT_PY" click "$((FX + FW / 2))" "$((FY + FH - 120))"; sleep 0.8
    python3 "$XINPUT_PY" click "$CX" "$CY"; sleep 2
    echo "INFO KiCad's marker events: $(grep -o 'kicad embed event: {[^}]*"type":"ET_MARKER_CLICKED[^\n]\{0,300\}' "$OUT/hub.log" | tail -2 | cut -c1-400)"
    CLK=""; for _ in $(seq 20); do CLK=$(hub_rpc "[]" 2>/dev/null | grep '^INFO ' | grep '"review.clicked.' | tail -1 | cut -c6- || true); [ -n "$CLK" ] && break; sleep 0.5; done
    echo "INFO the hub on the click: $(grep "marker clicked" "$OUT/hub.log" | tail -1)"
    WANT=$(python3 -c 'import json,sys; fs=json.load(open(sys.argv[1]))["result"]["findings"]; it=set(sys.argv[2].split(","))
print(next((f["id"] for f in fs if f["rule"]==sys.argv[3] and set(a.get("uuid") for a in f["anchors"] if a.get("uuid"))==it), ""))' "$(list drc)" "$MITEMS" "$MTYPE")
    # informational: the click is placed by colour on a centred view, which does not reliably land on the marker (Collar and Hubble_DK,
    # bp.13: markers stacked on vias, no lone one); the marker-to-row step is unit-tested (edchecks row_of, with a real ET_MARKER_CLICKED)
    echo "INFO K5 (not a check): a click on KiCad's DRC marker showed row: $(python3 -c 'import json,sys; v=json.loads(sys.argv[1] or "{}").get("value") or {}; print(v.get("id") or "none")' "$CLK" 2>/dev/null) (expected $WANT)"
  else
    echo "INFO K5 (not a check): KiCad's DRC marker not found on the screen after centring on it (k5_marker.png)"
  fi
else
  echo "INFO no error marker with a place and items: the marker click is left to the unit tests"
fi
finish_suite "embed_editor_checks_e2e ($EMBED_MODE)"
