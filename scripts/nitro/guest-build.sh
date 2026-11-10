#!/bin/bash
# Builds the guest images on the parent (as root; AL2023 with kernel6.18), into /var/lib/bp-nitro/img:
#   vmlinuz          the parent's EFI-stub kernel (DMB support for confidential mode is in it)
#   rootfs.ext4      the system disk every sandbox shares read-only: AL2023 userland, the build tools, Rust (BP_RUST, default stable), the agent
#   initrd.img       scripts/nitro/guest-init and the drivers (plain sandboxes: builds)
#   guest.eif        the attested image (eif_build): the init, the NSM tool, and the system disk's hash, so PCR0 covers what runs; pcrs.json its PCRs
# Usage: guest-build.sh [SRC]   SRC: the folder with scripts/nitro (default: this script's parent's parent)
set -eu
SRC=${1:-$(cd "$(dirname "$0")/../.." && pwd)}; N=$SRC/scripts/nitro; IMG=/var/lib/bp-nitro/img; W=$(mktemp -d /var/tmp/bp-guest.XXXXXX); trap 'umount "$W/root/proc" 2>/dev/null; rm -rf "$W"' EXIT
KV=$(uname -r); mkdir -p "$IMG"
cp "/boot/vmlinuz-$KV" "$IMG/vmlinuz"
# the system disk
R=$W/root; mkdir -p "$R"
dnf -y -q --installroot="$R" --releasever=2023 --setopt=install_weak_deps=False install \
  bash coreutils util-linux iproute procps-ng findutils diffutils which tar gzip xz bzip2 shadow-utils ca-certificates \
  python3 rsync git gcc gcc-c++ make cmake pkgconf-pkg-config openssl-devel perl clang lld socat > /dev/null
chroot "$R" useradd -u 1000 -m -d /work/home -s /bin/bash bp 2> /dev/null || true
install -m 755 "$N/bp-guest.py" "$R/usr/local/bin/bp-guest.py"; install -m 755 "$N/bp-guest-init" "$R/usr/local/bin/bp-guest-init"; install -m 755 "$SRC/scripts/boat-prune.py" "$R/usr/local/bin/bp-prune.py"
mkdir -p "$R/opt" "$R/work" "$R/run/bp" "$R/proc" "$R/sys" "$R/dev"
if [ "${BP_RUST:-stable}" != none ]; then
  curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs -o "$W/rustup.sh"
  RUSTUP_HOME=$R/opt/rustup CARGO_HOME=$R/opt/cargo sh "$W/rustup.sh" -y -q --no-modify-path --profile minimal --default-toolchain "${BP_RUST:-stable}" > /dev/null
fi
sz=$(( $(du -sm "$R" | cut -f1) * 13 / 10 + 512 ))
rm -f "$IMG/rootfs.ext4"; mkfs.ext4 -q -F -L bp-sys -d "$R" "$IMG/rootfs.ext4" "${sz}M"
# the plain initrd (the dracut line of the EC2 guide, with the block, overlay and switch_root pieces; no `base` module: it brings its own /init)
D="virtio_pci virtio_net virtio_blk vmw_vsock_virtio_transport nsm ext4 overlay"
I="mount modprobe ip sleep mkdir seq sed tr tail cut sha256sum switch_root poweroff chmod rm"
dracut --force --no-hostonly --reproducible --no-early-microcode --modules bash --add-drivers "$D" --install "$I" --include "$N/guest-init" /init --kver "$KV" "$IMG/initrd.img"
# the attested image: the same init, plus the NSM tool and what the secret needs, plus the system disk's hash
if command -v eif_build > /dev/null && [ -x /usr/local/bin/nsm-attest ]; then
  mkdir -p "$W/bp"; sha256sum < "$IMG/rootfs.ext4" | cut -d' ' -f1 > "$W/bp/rootfs.sha256"; : > "$W/bp/want-secret"
  dracut --force --no-hostonly --reproducible --no-early-microcode --modules bash --add-drivers "$D" \
    --install "$I openssl socat /usr/local/bin/nsm-attest /etc/pki/tls/openssl.cnf" --include "$N/guest-init" /init --include "$W/bp" /bp --kver "$KV" "$W/initrd-attest.img"
  # one build time for every build, so PCR6 repeats too (PCR0 is what the key policy checks)
  eif_build --kernel "$IMG/vmlinuz" --cmdline "panic=1 initcall_blacklist=i8042_init" --ramdisk "$W/initrd-attest.img" --pcie-virtio --build-time 2026-01-01T00:00:00Z \
    --output "$IMG/guest.eif" | sed -n '/^{/,/^}/p' > "$IMG/pcrs.json"
  echo "attested image: $IMG/guest.eif, PCR0 $(python3 -I -c 'import json,sys;print(json.load(open(sys.argv[1]))["PCR0"])' "$IMG/pcrs.json")"
else
  echo "no eif_build or nsm-attest: plain sandboxes only (scripts/nitro/host-setup.sh --attest installs them)"
fi
ls -la "$IMG"
