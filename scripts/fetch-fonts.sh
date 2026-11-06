#!/bin/sh
# Fetch JetBrains Mono (SIL OFL 1.1) once, from its official GitHub release, into assets/fonts/jetbrains-mono/.
# Run it on Boat (scripts/boat-job.sh --artifact assets/fonts/jetbrains-mono -- sh scripts/fetch-fonts.sh), never on the desktop;
# the downloaded zip is untrusted data: its own empty directory, only the named files are copied out, nothing is executed.
set -eu
VER=2.304
URL=https://github.com/JetBrains/JetBrainsMono/releases/download/v$VER/JetBrainsMono-$VER.zip
OUT=assets/fonts/jetbrains-mono
W=$(mktemp -d); mkdir -p "$W/unzip" "$OUT"
curl -fsSL "$URL" -o "$W/jb.zip"
unzip -q -o "$W/jb.zip" -d "$W/unzip"
mkdir -p "$W/out"
cp "$W/unzip/fonts/ttf/JetBrainsMono-Regular.ttf" "$W/unzip/fonts/ttf/JetBrainsMono-Bold.ttf" "$W/out/"
if [ -f "$W/unzip/OFL.txt" ]; then cp "$W/unzip/OFL.txt" "$W/out/OFL.txt"; else cp "$(find "$W/unzip" -name 'OFL.txt' | head -1)" "$W/out/OFL.txt"; fi
# the committed sums are the pin: what was downloaded must equal them, else nothing is installed
if [ -f "$OUT/SHA256SUMS" ]; then (cd "$W/out" && sha256sum -c "$OLDPWD/$OUT/SHA256SUMS") || { echo "downloaded fonts differ from the committed SHA256SUMS" >&2; exit 1; }; fi
cp "$W/out/JetBrainsMono-Regular.ttf" "$W/out/JetBrainsMono-Bold.ttf" "$W/out/OFL.txt" "$OUT/"
printf 'JetBrains Mono %s\nSource: %s\nLicence: SIL Open Font License 1.1 (OFL.txt), (c) 2020 The JetBrains Mono Project Authors (https://github.com/JetBrains/JetBrainsMono)\nFetched: %s\nFiles: Regular, Bold (no Italic: the UI uses none). Full fonts, not subset (each well under 1 MB). Ligatures are turned off in the app (font features), CJK falls back to the system font.\n' "$VER" "$URL" "$(date -u +%Y-%m-%d)" > "$OUT/SOURCE.txt"
[ -f "$OUT/SHA256SUMS" ] || (cd "$OUT" && sha256sum JetBrainsMono-Regular.ttf JetBrainsMono-Bold.ttf OFL.txt > SHA256SUMS)
(cd "$OUT" && sha256sum -c SHA256SUMS && ls -l)
rm -rf "$W"
