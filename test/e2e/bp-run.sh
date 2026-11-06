#!/bin/sh
# Desktop-side launcher for the e2e journeys on h@backplane (Xvfb, own D-Bus, bun copied beside the binaries). Nothing runs here.
#   test/e2e/bp-run.sh HUB_BIN GPUI_BIN [journey]
set -eu; cd "$(dirname "$0")/../.."; R=e2e-linux
ssh h@backplane "mkdir -p ~/$R/bin ~/$R/repo"
[ -x ~/.bun/bin/bun ] && rsync -a ~/.bun/bin/bun "h@backplane:$R/bin/"
rsync -a "$1" "$2" "h@backplane:$R/bin/"
[ -d "$(dirname "$2")/assets" ] && rsync -a "$(dirname "$2")/assets" "h@backplane:$R/bin/"
rsync -a --delete --exclude .git --exclude target --exclude mobile --exclude 'native/*/target' --exclude out ./ "h@backplane:$R/repo/"
ssh h@backplane "cd ~/$R/repo && export NO_SCOPE=1 BACKPLANE_GPUI_ASSETS=\$HOME/$R/bin/assets HUB_BIN=\$HOME/$R/bin/$(basename "$1") GPUI_BIN=\$HOME/$R/bin/$(basename "$2") && dbus-run-session -- \$HOME/$R/bin/bun test/e2e/run.mjs --client gpui --hub-kind rust --journey ${3:-all}"
