#!/bin/sh
# From the repo root: sh test/gpui/term/boat-chrome-job.sh   (a Boat sandbox: the driver against the real helper and real headless Chrome, on a page the test serves from loopback; no accounts, no real sites)
exec scripts/boat-job.sh --size default --timeout 1500 --idle 3600 --no-setup-bend --artifact out/term \
  --exclude 'mobile/**' --exclude 'desktop/**' --exclude 'build/**' --exclude 'docs/**' --exclude 'shots-snap/**' --exclude 'assets/**' --exclude 'i18n/**' --exclude 'deploy/**' --exclude 'order/**' \
  --exclude 'skills/**' --exclude 'extra/**' --exclude 'test/gpui/hub/policy-bin/**' -- sh -c '
set -e
mkdir -p out/term; R=$PWD
command -v bun >/dev/null 2>&1 || { npm install -g --prefix "$HOME/.local" bun >out/term/bun-install.log 2>&1 || true; export PATH="$HOME/.local/bin:$PATH"; }
bun --version >out/term/bun-version.txt 2>&1 || { echo "no bun"; exit 3; }
(cd tools/browser && bun install >../../out/term/bun-deps.log 2>&1)
printf "#!/bin/sh\nexec bun %s/tools/browser/src/main.ts \"\$@\"\n" "$R" > out/term/backplane-browser; chmod +x out/term/backplane-browser
printf "#!/bin/sh\nexec /usr/bin/google-chrome --no-sandbox --disable-dev-shm-usage \"\$@\"\n" > out/term/chrome; chmod +x out/term/chrome
export BP_REAL_BROWSER=$R/out/term/backplane-browser BACKPLANE_CHROME=$R/out/term/chrome
cd native/host && cargo test --bin backplane-hub term::browser::chrome -- --ignored --nocapture --test-threads=1 > ../../out/term/chrome-test.log 2>&1; rc=$?; tail -80 ../../out/term/chrome-test.log; exit $rc'
