#!/bin/sh
# Build the voice helper into one executable: dist/backplane-voice.
# Speech to text with OpenAI or on this machine (see native/voice/README.md). Rust only: no Bun, no Node.
set -e
root=$(cd "$(dirname "$0")/.." && pwd)
target=${CARGO_TARGET_DIR:-$root/native/voice/target}
cd "$root/native/voice"
CARGO_TARGET_DIR=$target "$root/scripts/cargo-q" build --release --offline --bin backplane-voice
mkdir -p "$root/dist"
cp "$target/release/backplane-voice" "$root/dist/backplane-voice"
ls -la "$root/dist/backplane-voice"
