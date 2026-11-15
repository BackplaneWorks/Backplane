#!/bin/sh
# The benchmark's fixtures, from the runtime's own demos: mkfix.sh RUNTIME OUT -> OUT/stickhub, OUT/stickhub_drc (one stray track), OUT/pic_programmer,
# each upgraded to the runtime's file format (so a save changes only what the edit changed).
set -eu
RT=$1; OUT=$2; K=$RT/bin/kicad-cli; D=$RT/share/kicad/demos
rm -rf "$OUT"; mkdir -p "$OUT/stickhub" "$OUT/pic_programmer"
cp -r "$D/stickhub/StickHub.kicad_pcb" "$D/stickhub/StickHub.kicad_pro" "$D/stickhub/StickHub.kicad_sch" "$D/stickhub/RobotProtos.kicad_sym" "$D/stickhub/footprints.pretty" "$D/stickhub/fp-lib-table" "$D/stickhub/sym-lib-table" "$OUT/stickhub/"
cp -r "$D/pic_programmer/"* "$OUT/pic_programmer/"
chmod -R u+w "$OUT"
"$K" pcb upgrade --force "$OUT/stickhub/StickHub.kicad_pcb" >/dev/null; "$K" sch upgrade --force "$OUT/stickhub/StickHub.kicad_sch" >/dev/null
"$K" pcb upgrade --force "$OUT/pic_programmer/pic_programmer.kicad_pcb" >/dev/null; "$K" sch upgrade --force "$OUT/pic_programmer/pic_programmer.kicad_sch" >/dev/null
cp -r "$OUT/stickhub" "$OUT/stickhub_drc"
# one stray GND track on F.Cu across the board's lower edge area, over other nets' copper: the finding the drc_fix task removes
python3 - "$OUT/stickhub_drc/StickHub.kicad_pcb" <<'PY'
import sys
p = sys.argv[1]; s = open(p).read().rstrip(); assert s.endswith(")")
seg = '\t(segment\n\t\t(start 146.2 112.4)\n\t\t(end 153.8 112.4)\n\t\t(width 0.2)\n\t\t(layer "F.Cu")\n\t\t(net "GND")\n\t\t(uuid "b5e7c0de-0000-4000-8000-00000000d7c1")\n\t)\n'
open(p, "w").write(s[:-1].rstrip() + "\n" + seg + ")\n")
PY
"$K" pcb upgrade --force "$OUT/stickhub_drc/StickHub.kicad_pcb" >/dev/null
