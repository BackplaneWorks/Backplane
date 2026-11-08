#!/bin/bash
# Provisions a Boat sandbox as a Backplane build box: the Linux build dependencies, clang, mold, sccache, bun and a pinned Rust toolchain.
# Run ON the sandbox by scripts/boat-pool.sh (`boat-pool.sh base`), which then saves it as the snapshot `bp-build-base`; every pool box starts from that snapshot,
# so a build never installs anything. No secrets are read or written here. Safe to run again.
set -eu
RUST=${BP_BOAT_RUST:-stable}; SCCACHE=${BP_BOAT_SCCACHE:-v0.10.0}
export DEBIAN_FRONTEND=noninteractive
sudo -n apt-get update -qq
sudo -n apt-get install -y -qq --no-install-recommends build-essential clang lld mold pkg-config cmake git rsync curl ca-certificates unzip xz-utils python3 \
  libxkbcommon-x11-dev libxkbcommon-dev libxcb1-dev libx11-xcb-dev libfontconfig-dev libfreetype-dev libasound2-dev libwayland-dev libvulkan-dev libssl-dev libxcb-render0-dev \
  libxcb-shape0-dev libxcb-xfixes0-dev libgtk-3-dev libnss3 libatk1.0-0t64 libatk-bridge2.0-0t64 libdrm2 libgbm1 libxdamage1 libxrandr2 libxcomposite1 libxfixes3 libcups2t64 libpango-1.0-0
if ! command -v sccache > /dev/null; then
  curl -fsSL "https://github.com/mozilla/sccache/releases/download/$SCCACHE/sccache-$SCCACHE-x86_64-unknown-linux-musl.tar.gz" | sudo -n tar -xz --strip-components=1 -C /usr/local/bin "sccache-$SCCACHE-x86_64-unknown-linux-musl/sccache"
fi
if ! command -v bun > /dev/null && [ ! -x "$HOME/.bun/bin/bun" ]; then curl -fsSL https://bun.sh/install | bash > /dev/null; fi
rustup toolchain install "$RUST" --profile minimal -c clippy -c rustfmt > /dev/null 2>&1 || true
rustup default "$RUST" > /dev/null 2>&1 || true
mkdir -p "$HOME/.cache/sccache" "$HOME/target" "$HOME/ws" "$HOME/.bpjobs"
echo "boat-base: $(rustc --version), $(mold --version | head -1), $(sccache --version), clang $(clang --version | head -1 | awk '{print $NF}')"
