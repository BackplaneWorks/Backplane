#!/bin/sh
# The GPUI pairing form as a person uses it: focus, paste (Ctrl+V), Enter, a link in the address box, and the errors shown. Against a running hub
# whose address this client can reach (not loopback). Every case is judged from the client's log.
#   BASE=http://HOST:PORT TOKEN=<hub token file> GPUI=<client binary> WORK=<folder> LAUNCH=<sh script: NAME SECONDS ARGS...> test/e2e/pairing/gpui-form.sh
set -u
HERE=$(cd "$(dirname "$0")" && pwd); W=$WORK; L=$W/logs; mkdir -p "$L"
fails=0
ok() { if [ "$2" = 0 ]; then echo "pass     $1"; else echo "fail     $1"; fails=$((fails+1)); fi; }
hub() { PYTHONDONTWRITEBYTECODE=1 python3 "$HERE/rig.py" "$BASE" "$TOKEN" "$@"; }
inv() { hub invite "$W/inv.json" >/dev/null; python3 -c "import json,sys;print(json.load(open('$W/inv.json'))[sys.argv[1]])" "$1"; }
says() { grep -q "$2" "$L/gpui-$1.out"; echo $?; }
run() { n=$1; s=$2; shift 2; RIG=$n sh "$LAUNCH" "$n" "$s" "$@" >/dev/null 2>&1; }
steps() { python3 -c "import json,sys;print(json.dumps(json.loads(sys.argv[2])))" x "$2" > "$W/$1.steps.json"; }
fresh() { rm -rf "$W/rig/$1"; }
# the first field has the keyboard when the dialog opens; paste into it with Ctrl+V and press Enter
fresh paste; l=$(inv link); steps paste "[{\"wait\":1500},{\"clipboard\":\"$l\"},{\"keys\":\"ctrl-v\"},{\"wait\":300},{\"keys\":\"enter\"},{\"wait\":3000}]"
run paste 10
ok "the link box has the keyboard on open" "$(says paste '"pair":"focus","text":"link"')"
ok "Ctrl+V pastes the link and Enter pairs" "$(says paste '"status":"Connected"')"
# a whole link pasted in the address box
fresh addr; l=$(inv link); steps addr "[{\"wait\":1500},{\"keys\":\"tab\"},{\"clipboard\":\"$l\"},{\"keys\":\"ctrl-v\"},{\"wait\":300},{\"keys\":\"enter\"},{\"wait\":3000}]"
run addr 10
ok "Tab moves to the address box" "$(says addr '"pair":"focus","text":"address"')"
ok "a link pasted in the address box pairs" "$(says addr '"status":"Connected"')"
# errors are shown in the dialog (the client logs what it draws)
fresh used; steps used "[{\"wait\":1500},{\"clipboard\":\"$l\"},{\"keys\":\"ctrl-v\"},{\"wait\":300},{\"keys\":\"enter\"},{\"wait\":2500}]"
run used 8
ok "a used invite says so" "$(says used 'already been used')"
fresh down; c=$(inv code); steps down "[{\"wait\":1500},{\"pair_fill\":[\"\",\"http://127.0.0.1:1\",\"$c\"]},{\"keys\":\"enter\"},{\"wait\":3000}]"
run down 8
ok "an unreachable address says so" "$(says down 'could not be reached')"
fresh empty; steps empty '[{"wait":1500},{"keys":"enter"},{"wait":800}]'
run empty 5
ok "Enter on an empty form says what to enter" "$(says empty 'Paste a pairing link, or enter')"
echo "summary fail=$fails"; [ "$fails" = 0 ]
