#!/bin/bash
# Real boards and runtimes for test/kicad_embed/real_use_*.sh. Only copies: a board is staged (its KiCad files, sheets, libraries, 3D
# models; not backups, build, mfg, git or lock files) into $BP_TMP and the run copies that again; the canonical folder is only read.

# name=folder of a real project (the boards of docs/kicad-real-use.md)
REAL_BOARDS=(
  "hubble=$HOME/Documents/Nordic/Hubble_DK"
  "collar_main=$HOME/Documents/i2cjak/Collar_Wearable/boards/main"
)
# name=KiCad runtime's bin/kicad. Test runtimes are named by their exact version (the integrator's rule, 2026-11-17): a published base
# (read-only, never a bare "10.99.0-bp.13"-style link) or a "-candidate" link, resolved once here so a link switched mid-run never mixes
# builds. KEMBED_BP12 / KEMBED_BP13 / KEMBED_BP13C=<dir> for another. Each run passes BACKPLANE_KICAD_BIN and BACKPLANE_KICAD_CLI from
# the same dir: the hub starts the KiCad beside the CLI, and agent shells export bp.11's CLI. Each run's evidence records the resolved
# root and its SOURCE_COMMIT (real_use_drive.py: RESULTS kicad_runtime).
KT=$HOME/.cache/kicad-test
REAL_RUNTIMES=(
  "bp10=$HOME/.backplane-gpui-fresh/home/kicad/10.99.0-bp.10/bin/kicad"
  "bp11=$HOME/.backplane-gpui-fresh/home/kicad/10.99.0-bp.11/bin/kicad"
  "bp12=$(readlink -f "${KEMBED_BP12:-$KT/builds/10.99.0-bp.12.4-published}")/bin/kicad"
  "bp13=$(readlink -f "${KEMBED_BP13:-$KT/10.99.0-bp.13.1}")/bin/kicad"
  # the next bp.13 release's candidate: a link that exists only while a candidate build does (never a default)
  "bp13c=$(readlink -f "${KEMBED_BP13C:-$KT/10.99.0-bp.13.2-candidate}" 2>/dev/null || echo "/nonexistent/10.99.0-bp.13.2-candidate")/bin/kicad"
)
# the fork's plan demo (scripts/backplane-plan-demo.py) at the runtime's own BACKPLANE_COMMIT, read from the fork's git (never its worktree)
KICAD_FORK=${KICAD_FORK:-$HOME/Documents/Backplane/Backplane_KiCad}
plan_demo() {  # plan_demo RUNTIME_KICAD OUT.py: 0 when the runtime has a plan demo
  # a release CI runtime has SOURCE_COMMIT only (no BACKPLANE_COMMIT): bp.13 batches 8 and 9 skipped their plan copies without it
  local rd c; rd=$(dirname "$(dirname "$1")"); c=$(cat "$rd/BACKPLANE_COMMIT" 2>/dev/null || cat "$rd/SOURCE_COMMIT" 2>/dev/null || true)
  [ -n "$c" ] && git -C "$KICAD_FORK" show "$c:scripts/backplane-plan-demo.py" >"$2" 2>/dev/null && [ -s "$2" ]
}

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
      if [ ! -x "$rk" ]; then
        # asked for by name: a missing runtime is a failure, said plainly; in the default set only a skip
        if [[ " ${KEMBED_RUNTIMES:-} " == *" $rn "* ]]; then
          echo "FAIL runtime $rn: $rk does not exist (for bp13c: no 10.99.0-bp.13.2-candidate link, so no candidate build to test)"; REAL_USE_MISSING=1
        else echo "SKIP runtime $rn: $rk missing"; fi
        continue
      fi
      echo "INFO runtime $rn: $(dirname "$(dirname "$rk")") SOURCE_COMMIT $(cat "$(dirname "$(dirname "$rk")")/SOURCE_COMMIT" 2>/dev/null || echo none)"
      "$fn" "$bn" "$bs" "$rn" "$rk"
    done
  done
}
