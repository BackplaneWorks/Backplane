#!/bin/bash
# The libraries release-gate row on Linux (Backplane 0.16.0 waits on it; macOS and Windows run the same libraries_row.py).
#   Phase a: a clean home (fresh hub home, a KiCad config home without tables), only the managed runtime: the staged build placed in
#            the hub's install folder (<hub home>/kicad/<version>, where its installer puts it), the system's KiCad not looked at;
#            a fresh project from the runtime's own blank templates; libraries_row.py L1-L7, L9 (see its docstring).
#   Phase b: a person's fp-lib-table with a custom entry and no Backplane entry, then the hub started (libtables.rs adds what the
#            runtime's default has): libraries_row.py L8.
# usage: LIB_ROW_RUNTIME=<a runtime root, e.g. ~/.cache/kicad-test/builds/10.99.0-bp.13-59c2846b2b> test/kicad_embed/embed_libraries_row_e2e.sh
set -euo pipefail
HERE=$(cd "$(dirname "$0")" && pwd)
# shellcheck source=lib/env_xvfb.sh
. "$HERE/lib/env_xvfb.sh"
RT_SRC=${LIB_ROW_RUNTIME:?LIB_ROW_RUNTIME: the runtime root to test}
RT_SRC=$(cd "$RT_SRC" && pwd)
VER=$(python3 -c 'import json,sys; print(json.load(open(sys.argv[1]))["version"])' "$RT_SRC/manifest.json")
SV=$(echo "$VER" | cut -d. -f1-2)
discover_bins
[ -x "$REAL_HOME/.bun/bin/bun" ] && PATH="$REAL_HOME/.bun/bin:$PATH"
export EMBED_MODE=fork BACKPLANE_KICAD_NO_SYSTEM=1
unset BACKPLANE_KICAD_LIBRARY KICAD_CONFIG_HOME

blank_project() { # $1 dir: the shared design's new project (libgate_design.make_project), its board's header from the runtime's own template
  python3 - "$RT_SRC/share/kicad/template" "$1" "$HERE/lib" <<'PY'
import os, sys
tpl, out, lib = sys.argv[1:]
sys.path.insert(0, lib); import libgate_design as D
s = os.path.join(tpl, "EuroCard160mmX100mm")
D.make_project(out, os.path.join(tpl, "kicad.kicad_pro"), os.path.join(s, "EuroCard160mmX100mm.kicad_pcb"), os.path.join(s, "EuroCard160mmX100mm.kicad_sch"))
open(os.path.join(out, ".backplane.json"), "w").write('{"project": "%s.kicad_pro"}\n' % D.NAME)
PY
}

place_runtime() { # the staged build where the hub's installer puts a managed runtime; hard links, nothing copied twice
  mkdir -p "$OUT/home/kicad"
  cp -al "$RT_SRC" "$OUT/home/kicad/$VER" 2>/dev/null || cp -a "$RT_SRC" "$OUT/home/kicad/$VER"
  KICAD_BIN="$OUT/home/kicad/$VER/bin/kicad"; KICAD_CLI_MANAGED="$OUT/home/kicad/$VER/bin/kicad-cli"
  export KICAD_BIN
}

config() { # $1 phase file: the core's config
  python3 - "$1" <<PY
import json, sys
json.dump({"out": "$OUT", "port": int("$HPORT"), "token_file": "$OUT/home/token", "hub_rpc": ["bun", "$HUB_RPC_MJS"], "standin_log": "$OUT/standin.log",
  "thread": "t1", "runtime_root": "$OUT/home/kicad/$VER", "managed_parent": "$OUT/home/kicad", "kicad_cli": "$KICAD_CLI_MANAGED",
  "config_root": "$XDG_CONFIG_HOME/kicad", "settings_version": "$SV", "project_dir": "$OUT/proj", "pcb": "$OUT/proj/libgate.kicad_pcb",
  "sch": "$OUT/proj/libgate.kicad_sch", "platform": "linux", "table_before": "$OUT/fp-lib-table.before", "sym_before": "$OUT/sym-lib-table.before"},
  open(sys.argv[1], "w"), indent=1)
PY
}

# ---- phase a: a clean home ----
init_isolated_dirs
blank_project "$OUT/src"
KEMBED_PROJ_SRC="$OUT/src" KICAD_CLI="" python3 "$SEED_PY" "$OUT" "$REPO_ROOT"
# the seed's KiCad settings keep the first-start wizard and the API server off the way; its library tables go: a clean config home
rm -f "$XDG_CONFIG_HOME"/kicad/*/sym-lib-table "$XDG_CONFIG_HOME"/kicad/*/fp-lib-table "$XDG_CONFIG_HOME"/kicad/*/design-block-lib-table
place_runtime
start_xvfb_and_wm
export BP_STREAM_N=1 BP_STREAM_MS=10 BP_STANDIN_MCPCALL_MAX=200000
KICAD_CLI="" start_hub
echo "INFO runtime $VER placed at $OUT/home/kicad/$VER (from $RT_SRC); config root $XDG_CONFIG_HOME/kicad (no tables)"
# the board open in the embedded editor (the editor's own checks, L7, are its; first use, L3, launches it)
cat >"$OUT/script.json" <<'JSON'
[{"select":0},{"wait":2000},{"click":"rail:Board"},{"wait":2500},{"click":"ui:Edit"},{"kicad":{"wait_painted":60000,"kind":"PCB","tag":"ready"}},{"mark":"edit_ready"},{"wait":1200000}]
JSON
HOME="$OUT/fh" BACKPLANE_GPUI_ASSETS="$ASSETS_DIR" BACKPLANE_GPUI_HOLD=1 "$CLIENT_BIN" --url "http://127.0.0.1:$HPORT" --token-file "$OUT/home/token" --script "$OUT/script.json" --exit-after 1300 >"$OUT/client.log" 2>&1 &
CP=$!; track_pid "$CP"
wait_for_log "$OUT/client.log" '"name":"edit_ready","event":"mark"' 600 || echo "INFO the editor did not open (L7's editor checks will say so)"
config "$OUT/row-a.json"
python3 "$HERE/libraries_row.py" a "$OUT/row-a.json" | tee "$OUT/row-a.out" || true
while read -r l; do case "$l" in PASS*) PASS_COUNT=$((PASS_COUNT + 1));; FAIL*) FAIL_COUNT=$((FAIL_COUNT + 1));; esac; done < "$OUT/row-a.out"

# ---- phase b: a person's table without the Backplane entries; the hub started again over it ----
kill "$CP" 2>/dev/null || true
# the hub (and its KiCad) stopped: the last group start_hub owned
HG=${OWN_GROUPS[${#OWN_GROUPS[@]}-1]}; kill -TERM -- "-$HG" 2>/dev/null || true
for _ in $(seq 50); do kill -0 "$HG" 2>/dev/null || break; sleep 0.2; done; kill -KILL -- "-$HG" 2>/dev/null || true; sleep 1
D="$XDG_CONFIG_HOME/kicad/$SV"; mkdir -p "$D"
printf '(fp_lib_table\n  (version 7)\n  (lib (name "KiCad")(type "Table")(uri "${KICAD10_TEMPLATE_DIR}/fp-lib-table")(options "")(descr "KiCad Default Libraries"))\n  (lib (name "My_Parts")(type "KiCad")(uri "${KIPRJMOD}/My_Parts.pretty")(options "")(descr "a person'"'"'s own"))\n)\n' >"$D/fp-lib-table"
cp "$D/fp-lib-table" "$OUT/fp-lib-table.before"
[ -f "$D/sym-lib-table" ] || printf '(sym_lib_table\n  (version 7)\n  (lib (name "KiCad")(type "Table")(uri "${KICAD10_TEMPLATE_DIR}/sym-lib-table")(options "")(descr "KiCad Default Libraries"))\n)\n' >"$D/sym-lib-table"
cp "$D/sym-lib-table" "$OUT/sym-lib-table.before"
rm -f "$OUT/home/kicad-libtables.json"   # what the hub added before is forgotten: this is the person's table, met for the first time
KICAD_CLI="" start_hub
sleep 3
grep -a "library tables" "$OUT/hub.log" | tail -2 | sed 's/^/INFO hub: /'
config "$OUT/row-b.json"
python3 "$HERE/libraries_row.py" b "$OUT/row-b.json" | tee "$OUT/row-b.out" || true
while read -r l; do case "$l" in PASS*) PASS_COUNT=$((PASS_COUNT + 1));; FAIL*) FAIL_COUNT=$((FAIL_COUNT + 1));; esac; done < "$OUT/row-b.out"
finish_suite "embed_libraries_row_e2e (linux, $VER)"
