#!/bin/bash
# Suite 7: live DRC/ERC (native/host/src/kicad/{findings,checkrun}.rs).
#   C1  kicad.checks.watch on the fixture (known violations) runs both checks; the summary (info kicad.checks.<digest>) says fresh,
#       with counts that match kicad-cli's own report
#   C2  kicad.checks.list gives the findings, errors first, each with rule, severity, anchors (names, positions) and a stable id;
#       the filters (severity, words) work
#   C3  a save of the board makes DRC stale, then it runs again by itself (debounced) and is fresh; ERC is left fresh; the ids of
#       the findings that are still there are the same
#   C0  with too little memory free (BACKPLANE_CHECKS_FREE_MB) the checks say deferred and run none
#   C4  kicad.checks.fix gives the agent's message; kicad.checks.focus without a KiCad answers kicad:false
# Copies only (seed_project.py); no KiCad GUI is needed (kicad-cli on the saved files). EMBED_MODE=stock|fork.
set -euo pipefail

HERE=$(cd "$(dirname "$0")" && pwd)
# shellcheck source=lib/env_xvfb.sh
. "$HERE/lib/env_xvfb.sh"

[ "${1:-}" != "" ] && CLIENT_BIN=$1
[ "${2:-}" != "" ] && HUB_BIN=$2
[ "${3:-}" != "" ] && OUT=$(mkdir -p "$3" && cd "$3" && pwd)
if [ "$EMBED_MODE" = "spike" ]; then echo "SKIP embed_checks_e2e: product modes only (EMBED_MODE=stock|fork)"; exit 0; fi

discover_bins
[ -x "$REAL_HOME/.bun/bin/bun" ] && PATH="$REAL_HOME/.bun/bin:$PATH"
discover_kicad
[ -n "${KICAD_CLI:-}" ] || KICAD_CLI="$(dirname "${KICAD_BIN:-/usr/bin/kicad}")/kicad-cli"
[ -x "$KICAD_CLI" ] || { echo "FAIL embed_checks_e2e: no kicad-cli (KICAD_CLI=${KICAD_CLI:-unset}): an environment problem, not a skip"; exit 1; }
init_isolated_dirs
python3 "$SEED_PY" "$OUT" "$REPO_ROOT"
DISP=${DISP:-}
PROJ="$OUT/proj"
PRO="$PROJ/ecc83-pp.kicad_pro"
# a known ERC finding besides the library ones: a label on no wire
python3 - "$PROJ/ecc83-pp.kicad_sch" <<'PY'
import sys
p = sys.argv[1]; s = open(p).read()
lab = '\t(label "DANGLE"\n\t\t(at 30.48 30.48 0)\n\t\t(effects\n\t\t\t(font\n\t\t\t\t(size 1.27 1.27)\n\t\t\t)\n\t\t\t(justify left bottom)\n\t\t)\n\t\t(uuid "5b0c1e6e-0000-4000-8000-00000000d00d")\n\t)\n'
open(p, "w").write(s.replace("\t(sheet_instances", lab + "\t(sheet_instances", 1))
PY

# C0: a hub that sees 100 MB free defers both checks and says why
BACKPLANE_CHECKS_FREE_MB=100 start_hub
for _ in $(seq 20); do
  R0=$(hub_rpc "[{\"method\":\"kicad.checks.watch\",\"params\":{\"project\":\"$PRO\"}}]" | grep -v '^INFO ' | head -1)
  python3 -c 'import json,sys; r=json.loads(sys.argv[1])["result"]; sys.exit(0 if r["drc"]["state"]=="deferred" and r["erc"]["state"]=="deferred" else 1)' "$R0" 2>/dev/null && break
  sleep 0.5
done
check "Low memory: both checks deferred, with the reason, none run" "$(python3 -c 'import json,sys; r=json.loads(sys.argv[1])["result"]; print(int(r["drc"]["state"]=="deferred" and "low memory" in r["drc"]["reason"] and not r["drc"]["has_result"] and r["erc"]["state"]=="deferred"))' "$R0" 2>/dev/null || echo 0)" "$R0"
own_stop || true; OWN_GROUPS=()
rm -rf "$OUT/home/cache/checks"
start_hub

# kicad-cli's own counts, the oracle
mkdir -p "$OUT/oracle"
"$KICAD_CLI" pcb drc --format json --severity-all --units mm --schematic-parity -o "$OUT/oracle/drc.json" "$PROJ/ecc83-pp.kicad_pcb" >/dev/null 2>&1 || true
"$KICAD_CLI" sch erc --format json --severity-all --units mm -o "$OUT/oracle/erc.json" "$PROJ/ecc83-pp.kicad_sch" >/dev/null 2>&1 || true
read -r OD OE < <(python3 - "$OUT/oracle" <<'PY'
import json, sys
d = json.load(open(sys.argv[1] + "/drc.json")); e = json.load(open(sys.argv[1] + "/erc.json"))
# the hub leaves the library checks to KiCad (findings.rs library_rule)
lib = {"lib_footprint_issues", "lib_footprint_mismatch", "lib_symbol_issues", "lib_symbol_mismatch", "footprint_link_issues"}
print(sum(1 for k in ("violations", "unconnected_items", "schematic_parity") for v in d[k] if v["type"] not in lib), sum(1 for s in e["sheets"] for v in s["violations"] if v["type"] not in lib))
PY
)
echo "oracle (library checks left out): drc=$OD erc=$OE"

# The summary once both checks are in this state ("" when they never were)
wait_state() {
  local want=$1 tries=${2:-120} r
  for _ in $(seq "$tries"); do
    r=$(hub_rpc "[{\"method\":\"kicad.checks.watch\",\"params\":{\"project\":\"$PRO\"}}]" | grep -v '^INFO ' | head -1)
    if python3 -c 'import json,sys; r=json.loads(sys.argv[1])["result"] or {}; w=sys.argv[2].split(","); sys.exit(0 if r.get("drc",{}).get("state")==w[0] and r.get("erc",{}).get("state")==w[1] else 1)' "$r" "$want" 2>/dev/null; then echo "$r"; return 0; fi
    sleep 1
  done
  echo "$r"; return 1
}

T0=$(date +%s%N)
S1=$(wait_state fresh,fresh 180) || true
T1=$(date +%s%N)
echo "first result after $(( (T1 - T0) / 1000000 )) ms"
C1=$(python3 - "$S1" "$OD" "$OE" <<'PY'
import json, sys
r = json.loads(sys.argv[1])["result"]; od, oe = int(sys.argv[2]), int(sys.argv[3])
n = lambda c: sum(c.values())
print(int(r["drc"]["state"] == "fresh" and r["erc"]["state"] == "fresh" and n(r["drc"]["counts"]) == od and n(r["erc"]["counts"]) == oe and r["drc"]["at"] > 0 and r["drc"]["rev"] != "" and r["chip"].startswith("DRC ")))
PY
)
check "Both checks ran and are fresh, counts match kicad-cli's (drc $OD, erc $OE)" "$C1" "$S1"
INFO=$(hub_rpc "[]" | grep '^INFO ' | grep 'kicad.checks.' | tail -1 | cut -c6- || true)
check "The summary is published as info kicad.checks.<digest> (phones read it)" "$(python3 -c 'import json,sys; v=json.loads(sys.argv[1] or "{}").get("value") or {}; print(int(v.get("drc",{}).get("state")=="fresh"))' "$INFO" 2>/dev/null || echo 0)" "$INFO"

L1=$(hub_rpc "[{\"method\":\"kicad.checks.list\",\"params\":{\"project\":\"$PRO\"}}]" | grep -v '^INFO ' | head -1)
echo "$L1" >"$OUT/list1.json"
C2=$(python3 - "$L1" "$OD" "$OE" <<'PY'
import json, sys
r = json.loads(sys.argv[1])["result"]; fs = r["findings"]
rank = {"error": 0, "warning": 1, "info": 2, "excluded": 3}
ok = r["total"] == int(sys.argv[2]) + int(sys.argv[3]) and len(fs) == r["total"]
ok = ok and [rank[f["severity"]] for f in fs] == sorted(rank[f["severity"]] for f in fs)
ok = ok and all(f["id"] and f["rule"] and f["title"] and f["source"] in ("drc", "erc") for f in fs) and len({f["id"] for f in fs}) == len(fs)
ok = ok and any(a.get("reference") for f in fs for a in f["anchors"]) and any("x" in a for f in fs for a in f["anchors"])
print(int(ok))
PY
)
check "kicad.checks.list: every finding with source, rule, severity, anchors and a unique id, errors first" "$C2"
LF=$(hub_rpc "[{\"method\":\"kicad.checks.list\",\"params\":{\"project\":\"$PRO\",\"source\":\"erc\",\"q\":\"dangle\"}}]" | grep -v '^INFO ' | head -1)
check "Filtered by check and words (ERC, \"dangle\"): the label, with its net and place" "$(python3 -c 'import json,sys; r=json.loads(sys.argv[1])["result"]; f=r["findings"]; print(int(r["total"]==1 and f[0]["source"]=="erc" and f[0]["rule"]=="label_dangling" and f[0]["anchors"][0].get("net")=="DANGLE" and abs(f[0]["anchors"][0]["x"]-30.48)<0.01))' "$LF")" "$LF"

# C3: a save of the board (the same board, written again) makes DRC run again by itself
ERC_AT=$(python3 -c 'import json,sys; print(json.loads(sys.argv[1])["result"]["erc"]["at"])' "$S1")
DRC_AT=$(python3 -c 'import json,sys; print(json.loads(sys.argv[1])["result"]["drc"]["at"])' "$S1")
sleep 1.1
cp "$PROJ/ecc83-pp.kicad_pcb" "$OUT/pcb.tmp" && cat "$OUT/pcb.tmp" >"$PROJ/ecc83-pp.kicad_pcb"
SAW=""
for _ in $(seq 30); do
  r=$(hub_rpc "[{\"method\":\"kicad.checks.watch\",\"params\":{\"project\":\"$PRO\"}}]" | grep -v '^INFO ' | head -1)
  st=$(python3 -c 'import json,sys; print(json.loads(sys.argv[1])["result"]["drc"]["state"])' "$r"); SAW="$SAW $st"
  [ "$st" = stale ] || [ "$st" = running ] && break; sleep 0.3
done
S3=$(wait_state fresh,fresh 120) || true
C3=$(python3 - "$S3" "$DRC_AT" "$ERC_AT" <<'PY'
import json, sys
r = json.loads(sys.argv[1])["result"]
print(int(r["drc"]["at"] > int(sys.argv[2]) and r["erc"]["at"] == int(sys.argv[3])))
PY
)
check "A board save: DRC went stale/running ($SAW) and ran again; ERC was left as it was" "$C3" "$S3"
L3=$(hub_rpc "[{\"method\":\"kicad.checks.list\",\"params\":{\"project\":\"$PRO\"}}]" | grep -v '^INFO ' | head -1)
check "The findings keep their ids across runs" "$(python3 -c 'import json,sys; a=json.loads(sys.argv[1])["result"]["findings"]; b=json.loads(sys.argv[2])["result"]["findings"]; print(int(sorted(f["id"] for f in a)==sorted(f["id"] for f in b)))' "$L1" "$L3")"

# C4: the agent's message, and a focus with no KiCad open
FID=$(python3 -c 'import json,sys; fs=json.loads(sys.argv[1])["result"]["findings"]; print(next(f["id"] for f in fs if f["source"]=="drc" and any(a.get("reference") for a in f["anchors"])))' "$L1")
FX=$(hub_rpc "[{\"method\":\"kicad.checks.fix\",\"params\":{\"project\":\"$PRO\",\"id\":\"$FID\"}},{\"method\":\"kicad.checks.focus\",\"params\":{\"project\":\"$PRO\",\"id\":\"$FID\"}}]" | grep -v '^INFO ')
check "kicad.checks.fix names the finding, its rule and items" "$(python3 -c 'import json,sys; r=json.loads(sys.argv[1].splitlines()[0])["result"]; t=r["text"]; print(int(sys.argv[2] in t and "Rule:" in t and "Items:" in t))' "$FX" "$FID")" "$FX"
check "kicad.checks.focus with no KiCad open answers kicad:false" "$(python3 -c 'import json,sys; r=json.loads(sys.argv[1].splitlines()[1]); print(int(r["ok"] and r["result"].get("kicad") is False))' "$FX")" "$FX"

# Run on demand while fresh: runs again
B4=$(python3 -c 'import json,sys; print(json.loads(sys.argv[1])["result"]["erc"]["at"])' "$S3")
hub_rpc "[{\"method\":\"kicad.checks.run\",\"params\":{\"project\":\"$PRO\",\"source\":\"erc\"}}]" >/dev/null
sleep 1
S4=$(wait_state fresh,fresh 120) || true
check "Run (on demand) runs ERC again on a fresh design" "$(python3 -c 'import json,sys; print(int(json.loads(sys.argv[1])["result"]["erc"]["at"] > int(sys.argv[2])))' "$S4" "$B4")"
grep -o 'took_ms":[0-9]*' <<<"$S4" | head -2 | tr '\n' ' '; echo
# C5: "Run all checks" (full): the library checks too, so the counts are kicad-cli's own, all of them
read -r FD FE < <(python3 - "$OUT/oracle" <<'PY'
import json, sys
d = json.load(open(sys.argv[1] + "/drc.json")); e = json.load(open(sys.argv[1] + "/erc.json"))
print(sum(len(d[k]) for k in ("violations", "unconnected_items", "schematic_parity")), sum(len(s["violations"]) for s in e["sheets"]))
PY
)
B5=$(python3 -c 'import json,sys; r=json.loads(sys.argv[1])["result"]; print(r["drc"]["at"], r["erc"]["at"])' "$S4")
hub_rpc "[{\"method\":\"kicad.checks.run\",\"params\":{\"project\":\"$PRO\",\"full\":true}}]" >/dev/null
S5=""; for _ in $(seq 120); do
  S5=$(hub_rpc "[{\"method\":\"kicad.checks.watch\",\"params\":{\"project\":\"$PRO\"}}]" | grep -v '^INFO ' | head -1)
  python3 -c 'import json,sys; r=json.loads(sys.argv[1])["result"]; a,b=map(int,sys.argv[2].split()); sys.exit(0 if r["drc"]["full"] and r["erc"]["full"] and r["drc"]["at"]>a and r["erc"]["at"]>b and r["drc"]["state"]=="fresh" and r["erc"]["state"]=="fresh" else 1)' "$S5" "$B5" && break; sleep 1
done
check "A full run counts the library checks too: kicad-cli's whole report (drc $FD, erc $FE), and says so (no note)" "$(python3 -c 'import json,sys; r=json.loads(sys.argv[1])["result"]; n=lambda c: sum(c.values()); print(int(r["drc"]["full"] and n(r["drc"]["counts"])==int(sys.argv[2]) and n(r["erc"]["counts"])==int(sys.argv[3]) and r["drc"]["note"]==""))' "$S5" "$FD" "$FE")" "$S5"
# C6: another source (the design review's) joins the one list, reads stale after a save, and lists by its id
hub_rpc "[{\"method\":\"kicad.checks.set\",\"params\":{\"project\":\"$PRO\",\"source\":\"fab\",\"meta\":{\"label\":\"Fab rules\"},\"findings\":[{\"severity\":\"warning\",\"rule\":\"min_annular\",\"title\":\"Annular ring under the fab's minimum\",\"anchors\":[{\"doc\":\"pcb\",\"reference\":\"U1\",\"text\":\"Footprint U1\"}]}]}}]" >/dev/null
S6=$(hub_rpc "[{\"method\":\"kicad.checks.watch\",\"params\":{\"project\":\"$PRO\"}},{\"method\":\"kicad.checks.list\",\"params\":{\"project\":\"$PRO\",\"source\":\"fab\"}}]" | grep -v '^INFO ')
check "Another source's findings join the list (summary.sources.fab fresh, listed by source with an id)" "$(python3 -c 'import json,sys; a,b=[json.loads(l)["result"] for l in sys.argv[1].splitlines()[:2]]; f=a["sources"]["fab"]; print(int(f["state"]=="fresh" and f["label"]=="Fab rules" and b["total"]==1 and b["findings"][0]["id"].startswith("fab:") and b["findings"][0]["source"]=="fab"))' "$S6")" "$S6"
sleep 1.1; cp "$PROJ/ecc83-pp.kicad_pcb" "$OUT/pcb.tmp" && cat "$OUT/pcb.tmp" >"$PROJ/ecc83-pp.kicad_pcb"
ST6=""; for _ in $(seq 20); do ST6=$(hub_rpc "[{\"method\":\"kicad.checks.watch\",\"params\":{\"project\":\"$PRO\"}}]" | grep -v '^INFO ' | head -1 | python3 -c 'import json,sys; print(json.load(sys.stdin)["result"]["sources"]["fab"]["state"])'); [ "$ST6" = stale ] && break; sleep 0.5; done
check "After a save the other source reads stale ($ST6)" "$( [ "$ST6" = stale ] && echo 1 || echo 0)"
finish_suite "embed_checks_e2e ($EMBED_MODE)"
