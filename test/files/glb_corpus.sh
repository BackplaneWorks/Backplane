#!/bin/bash
# glb_corpus.sh OUT RUNTIME: GLBs of KiCad's demo boards, made with the export flags of Pv.glb.flags, from copies of the demos (the runtime stays read-only), HOME and config in temp folders.
# Each export under MemoryMax=3700M and a timeout; a board that fails is listed and left out. Writes OUT/*.glb and OUT/list.tsv.
set -u
out=$(realpath -m "$1"); rt=$2; mkdir -p "$out"; t=$(mktemp -d "$out/tmp.XXXXXX")
export HOME=$t/home XDG_CONFIG_HOME=$t/home/.config XDG_CACHE_HOME=$t/home/.cache XDG_DATA_HOME=$t/home/.local/share KICAD_CONFIG_HOME=$t/home/kicad
mkdir -p "$HOME"; : > "$out/list.tsv"
for rel in ecc83/ecc83-pp pic_programmer/pic_programmer stickhub/StickHub cm5_minima/CM5_MINIMA_3 video/video complex_hierarchy/complex_hierarchy interf_u/interf_u microwave/microwave multichannel/multichannel royalblue54L_feather/royalblue54L_feather 'sonde xilinx/sonde xilinx' kit-dev-coldfire-xilinx_5213/kit-dev-coldfire-xilinx_5213; do
  dir=${rel%%/*}; name=${rel##*/}; src="$rt/share/kicad/demos/$dir"
  [ -f "$src/$name.kicad_pcb" ] || { printf '%s\tno board\n' "$name" >> "$out/list.tsv"; continue; }
  rm -rf "$t/p"; mkdir -p "$t/p"; cp -r "$src/." "$t/p/"; chmod -R u+w "$t/p"
  for mode in parts bare; do
    flag=--subst-models; [ $mode = bare ] && flag=--no-components
    o="$out/${name// /_}-$mode.glb"; s=$(date +%s.%N)
    systemd-run --user --scope -q -p MemoryMax=3700M -p MemorySwapMax=0 -- timeout 240 "$rt/bin/kicad-cli" pcb export glb $flag --include-pads --include-soldermask --include-silkscreen -f -o "$o" "$t/p/$name.kicad_pcb" > "$t/log" 2>&1
    c=$?; e=$(date +%s.%N)
    printf '%s\t%s\t%s\t%.1f\t%s\t%s\n' "$name" "$mode" "$c" "$(echo "$e - $s" | bc)" "$(stat -c %s "$o" 2>/dev/null || echo 0)" "$(grep -c '^Could not add 3D model' "$t/log")" >> "$out/list.tsv"
    [ "$c" = 0 ] || rm -f "$o"
  done
done
rm -rf "$t"; cat "$out/list.tsv"
