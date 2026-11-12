#!/bin/sh
# Stock KiCad loads the fab section of a .kicad_dru: for each board kind and tier, writes the section into a copy of a stock-format
# project (one of KiCad's demos) with a sentinel rule after it, runs a stock kicad-cli's DRC and checks that the sentinel fired (KiCad
# drops a whole rules file that does not load, so no sentinel means an error) and that kicad-cli said nothing but its counts.
# usage: test/fab/stock_drc.sh [stock kicad-cli (default /usr/bin/kicad-cli)] [demo project folder]
set -eu
here=$(cd "$(dirname "$0")/../.." && pwd)
cli=${1:-/usr/bin/kicad-cli}
demo=${2:-/usr/share/kicad/demos/pic_programmer}
. "$here/scripts/lib-bptmp.sh"
work=$(mktemp -d "$BP_TMP/fab-stock-drc.XXXXXX"); trap 'rm -rf "$work"' EXIT
"$cli" --version
fail=0
for case in 'pcb|{"layers":2}|free' 'pcb|{"layers":4}|extra' 'pcb|{"layers":2,"thickness_mm":0.8,"finish":"enig"}|extra' 'hdi|{"layers":6}|free' 'flex|{"layers":2}|free' 'aluminum|{"layers":1}|free'; do
  kind=${case%%|*}; rest=${case#*|}; opts=${rest%|*}; tier=${rest##*|}
  rm -rf "$work/p"; cp -r "$demo" "$work/p"; rm -f "$work/p"/*.kicad_dru
  pcb=$(ls "$work/p"/*.kicad_pcb | head -1); dru=${pcb%.kicad_pcb}.kicad_dru
  (cd "$here/native/host" && FAB_DRC_PROJECT="$work/p" FAB_DRC_KIND=$kind FAB_DRC_OPTIONS=$opts FAB_DRC_TIER=$tier \
    "$here/scripts/cargo-q" test --offline --bin backplane-hub fab_drc_write_into_a_project_copy -- --ignored --quiet >/dev/null 2>&1)
  grep -q '^# BEGIN Backplane fab profile' "$dru" || { echo "FAIL $case: no section written"; fail=1; continue; }
  printf '(rule sentinel (constraint track_width (min 50mm)))\n' >> "$dru"
  "$cli" pcb drc --format json -o "$work/r.json" "$pcb" > "$work/out" 2>&1 || true
  said=$(grep -v -e '^Found [0-9]* ' -e '^Saved DRC Report' -e '^$' "$work/out" || true)
  n=$(python3 -I -c "import json,sys;d=json.load(open(sys.argv[1]));print(sum(1 for v in d['violations'] if v['type']=='track_width'))" "$work/r.json")
  fab=$(python3 -I -c "import json,sys;d=json.load(open(sys.argv[1]));print(sum(1 for v in d['violations'] if 'fab:' in v.get('description','')))" "$work/r.json")
  if [ "$n" -gt 0 ] && [ -z "$said" ]; then echo "ok   $case: loads ($(grep -c '^(rule "fab:' "$dru") fab rules, sentinel fired $n times)"
  else echo "FAIL $case: sentinel $n, kicad-cli said: $said"; sed -n '/BEGIN/,/END/p' "$dru"; fail=1; fi
done
exit $fail
