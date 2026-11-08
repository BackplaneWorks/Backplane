#!/usr/bin/env bash
# The notices of the Rust crates compiled into each shipped binary (licenses/rust/*.txt in every release archive), made by cargo-about from each
# binary's workspace for one target. A crate whose licence is not in deploy/licenses/rust/about.toml's `accepted` list, or whose licence cannot be
# read, fails it (--fail), and so fails the release.
# usage: scripts/rust-licenses.sh TARGET OUT_DIR   TARGET: linux-x64 | linux-arm64 | darwin-arm64 | windows-x64; OUT_DIR from the repository root, or absolute
set -euo pipefail
root=$(cd "$(dirname "$0")/.." && pwd)
t=${1:?target}; out=${2:?out dir}
case "$t" in
  linux-x64) triple=x86_64-unknown-linux-gnu ;; linux-arm64) triple=aarch64-unknown-linux-gnu ;;
  darwin-arm64) triple=aarch64-apple-darwin ;; windows-x64) triple=x86_64-pc-windows-msvc ;;
  *) echo "rust-licenses: unknown target $t" >&2; exit 2 ;;
esac
command -v cargo-about >/dev/null || { echo "rust-licenses: cargo-about not installed (cargo install --locked --features cli cargo-about)" >&2; exit 1; }
# paths relative to the root: cargo-about is a native program, and on Windows it cannot read Git Bash's /d/... paths
cd "$root"; mkdir -p "$out"
cfg=deploy/licenses/rust/about.toml; hbs=deploy/licenses/rust/about.hbs
# file  workspace  features (as release.yml builds them: the client has CEF on Linux only)
cef=-; case "$t" in linux-*) cef=cef ;; esac
while read -r file ws feat what; do
  [ "$feat" != - ] || feat=
  echo "rust-licenses: $file ($what, $triple)"
  cargo about generate --locked --fail -c "$cfg" -m "native/$ws/Cargo.toml" --target "$triple" ${feat:+--features "$feat"} \
    "$hbs" -o "$out/$file"
  # the template's @PROGRAM@ is the binary it is for
  python3 -I - "$out/$file" "$what" <<'PY'
import sys; p, w = sys.argv[1], sys.argv[2]; s = open(p, encoding="utf-8").read().replace("@PROGRAM@", w, 1); open(p, "w", encoding="utf-8", newline="\n").write(s)
PY
  grep -q "^Used by:" "$out/$file" || { echo "rust-licenses: $out/$file lists no crates" >&2; exit 1; }
done <<EOT
hub.txt host - backplane-hub and bp-sh
client.txt gpui $cef backplane-gpui
guard.txt guard - backplane
voice.txt voice - backplane-voice
EOT
