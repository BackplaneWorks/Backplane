#!/bin/bash
# ci-container.sh probe|full ARCHIVE_DIR OUT: the e2e-linux workflow's steps inside a newer Ubuntu container (an X server newer than the runner's:
# the client drew nothing on a bare Xvfb 21.1.4/21.1.12). Run as root in the container, with the checkout as the cwd.
set -x; export DEBIAN_FRONTEND=noninteractive
apt-get update -qq
apt-get install -y -qq --no-install-recommends xvfb i3 xclip imagemagick python3-pil libxtst6 libx11-6 mesa-vulkan-drivers libvulkan1 libnss3 libatk-bridge2.0-0 libgbm1 libxkbcommon-x11-0 libasound2t64 libxkbcommon0 curl ca-certificates binutils x11-utils x11-apps x11-xserver-utils git unzip xdotool xauth iproute2 procps python3 libfontconfig1 libfreetype6 libxcb1 file > /dev/null || apt-get install -y -qq xvfb i3 xclip imagemagick python3-pil mesa-vulkan-drivers curl ca-certificates unzip git x11-utils x11-apps x11-xserver-utils > /dev/null
curl -fsSL https://bun.sh/install | bash -s "bun-v$(cat deploy/bun-version)" > /dev/null; export PATH=$HOME/.bun/bin:$PATH
cat /etc/os-release | head -2; dpkg -s xvfb | grep ^Version; uname -m
A=$(ls "$2"/*.tar.gz | head -1)
if [ "$1" = probe ]; then mkdir pr && tar -xzf "$A" -C pr; test/e2e/xprobe.sh "$PWD/pr" 2>&1 | tee xprobe.txt; cp /tmp/xprobe-*.png . 2>/dev/null; exit 0; fi
export E2E_SHORT=1 E2E_SLOW=1
git config --global user.email e2e@example.invalid; git config --global user.name e2e; git config --global --add safe.directory '*'
scripts/e2e-linux.sh --archive "$A" --no-phone "$3" 2>&1 | tee e2e.txt
