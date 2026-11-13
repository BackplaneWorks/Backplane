#!/bin/bash
# Real boards and runtimes for test/kicad_embed/real_use_*.sh. Only copies: a board is staged (its KiCad files, sheets, libraries, 3D
# models; not backups, build, mfg, git or lock files) into $BP_TMP and the run copies that again; the canonical folder is only read.

# name=folder of a real project (the boards of docs/kicad-real-use.md)
REAL_BOARDS=(
  "hubble=$HOME/Documents/Nordic/Hubble_DK"
  "collar_main=$HOME/Documents/i2cjak/Collar_Wearable/boards/main"
)
# name=KiCad runtime's bin/kicad: the released one the person runs, the bp.12 test build (KEMBED_BP12=<dir> for another; each run passes
# BACKPLANE_KICAD_BIN and BACKPLANE_KICAD_CLI from the same dir: the hub starts the KiCad beside the CLI, and agent shells export bp.11's CLI)
REAL_RUNTIMES=(
  "bp11=$HOME/.backplane-gpui-fresh/home/kicad/10.99.0-bp.11/bin/kicad"
  "bp12=${KEMBED_BP12:-$HOME/.cache/kicad-test/10.99.0-bp.12}/bin/kicad"
)

# stage_board SRC DST: a slim read-only copy (rsync-like, python so nothing extra is needed)
stage_board() {
  python3 -I - "$1" "$2" <<'PY'
import os, shutil, sys
src, dst = sys.argv[1], sys.argv[2]
skip_dirs = {".git", ".backplane", "mfg", "build", "backups", "docs", "scripts", "renders", "__pycache__"}
skip = shutil.ignore_patterns(*skip_dirs, "*-backups", "*.lck", "~*", "*.zip", "*.pdf", "fp-info-cache")
if os.path.exists(dst): shutil.rmtree(dst)
shutil.copytree(src, dst, ignore=skip, symlinks=True)
PY
}

# real_use_matrix FN: FN board_name board_src runtime_name kicad_bin, for each pair (KEMBED_BOARDS / KEMBED_RUNTIMES filter by name)
real_use_matrix() {
  local fn=$1 b r bn bs rn rk
  for b in "${REAL_BOARDS[@]}"; do
    bn=${b%%=*}; bs=${b#*=}
    [ -z "${KEMBED_BOARDS:-}" ] || [[ " $KEMBED_BOARDS " == *" $bn "* ]] || continue
    [ -d "$bs" ] || { echo "SKIP board $bn: $bs missing"; continue; }
    for r in "${REAL_RUNTIMES[@]}"; do
      rn=${r%%=*}; rk=${r#*=}
      [ -z "${KEMBED_RUNTIMES:-}" ] || [[ " $KEMBED_RUNTIMES " == *" $rn "* ]] || continue
      [ -x "$rk" ] || { echo "SKIP runtime $rn: $rk missing"; continue; }
      "$fn" "$bn" "$bs" "$rn" "$rk"
    done
  done
}
