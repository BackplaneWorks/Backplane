#!/bin/bash
# EC2 user data for the Backplane Nitro parent (AL2023, kernel6.18; `aws ec2 run-instances ... --enclave-options Enabled=true --user-data file://this`).
# First boot: the packages, Firecracker (checked against its release's SHA-256), the sandbox memory pool on the kernel command line, then a reboot that reserves it.
# After it: scripts/nitro/host-push.sh DEST copies bp-nitro and builds the guest images. Also runs by hand with --attest (the attestation tools).
set -eu
FC=${BP_FC_VERSION:-1.18.0}; ARCH=$(uname -m)
dnf -y -q install nftables socat python3 e2fsprogs dracut rsync git tar
if [ "${1:-}" = --attest ]; then
  dnf -y -q install gcc openssl-devel
  [ -x "$HOME/.cargo/bin/cargo" ] || curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y -q --profile minimal
  . "$HOME/.cargo/env"; rustup target add "$ARCH-unknown-linux-musl"
  cargo install -q --git https://github.com/aws/aws-nitro-enclaves-image-format eif_build && install -m 755 "$HOME/.cargo/bin/eif_build" /usr/local/bin/
  cargo build -q --release --target "$ARCH-unknown-linux-musl" --manifest-path "$(dirname "$0")/nsm-attest/Cargo.toml" \
    && install -m 755 "$(dirname "$0")/nsm-attest/target/$ARCH-unknown-linux-musl/release/nsm-attest" /usr/local/bin/
  exit 0
fi
if ! /usr/local/bin/firecracker --version 2> /dev/null | grep -q "v$FC"; then
  t=$(mktemp -d); cd "$t"
  curl -sSfL -o fc.tgz "https://github.com/firecracker-microvm/firecracker/releases/download/v$FC/firecracker-v$FC-$ARCH.tgz"
  curl -sSfL -o fc.tgz.sha256.txt "https://github.com/firecracker-microvm/firecracker/releases/download/v$FC/firecracker-v$FC-$ARCH.tgz.sha256.txt"
  [ "$(sha256sum fc.tgz | cut -d' ' -f1)" = "$(cut -d' ' -f1 fc.tgz.sha256.txt)" ] || { echo "firecracker: the download does not match its hash"; exit 1; }
  tar -xzf fc.tgz; install -m 755 "release-v$FC-$ARCH/firecracker-v$FC-$ARCH" /usr/local/bin/firecracker; cd /; rm -rf "$t"
fi
mkdir -p /var/lib/bp-nitro/img
# the pool: all the memory but what the parent keeps (an eighth, at least 4 GiB)
total=$(( $(awk '/MemTotal/{print $2}' /proc/meminfo) / 1048576 )); keep=$(( total / 8 > 4 ? total / 8 : 4 )); pool=$(( total - keep ))
if ! grep -q "nitro_enclaves.mempool=" /proc/cmdline; then grubby --update-kernel=ALL --args="nitro_enclaves.mempool=${pool}G@0"; reboot; fi
