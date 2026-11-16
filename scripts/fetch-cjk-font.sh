#!/bin/sh
# Fetch Noto Sans CJK JP Regular and Bold (SIL OFL 1.1) from notofonts/noto-cjk at a pinned tag and subset them to the Japanese the app draws:
# JIS X 0208 (level 1 and 2 kanji, kana, symbols), half-width kana, every character in i18n/ja.msg, and ASCII. The result goes to
# assets/fonts/noto-sans-cjk-jp/ and is compiled into the Linux desktop (native/gpui/src/art.rs), so Japanese draws with no CJK font
# installed (macOS and Windows have system CJK fonts). The downloads are untrusted data: their own empty directory, only read by fontTools.
# usage: sh scripts/fetch-cjk-font.sh   (needs curl and pyftsubset: pip install fonttools)
set -eu
TAG=f8d157532fbfaeda587e826d4cd5b21a49186f7c   # notofonts/noto-cjk main, 2024-09-19 (Sans 2.004)
BASE=https://github.com/notofonts/noto-cjk/raw/$TAG/Sans
OUT=assets/fonts/noto-sans-cjk-jp
W=$(mktemp -d); trap 'rm -rf "$W"' EXIT; mkdir -p "$W/dl" "$W/out" "$OUT"
for s in Regular Bold; do curl -fsSL "$BASE/OTF/Japanese/NotoSansCJKjp-$s.otf" -o "$W/dl/NotoSansCJKjp-$s.otf"; done
curl -fsSL "$BASE/LICENSE" -o "$W/out/OFL.txt"
# the characters: JIS X 0208 (every two-byte EUC-JP code that decodes), the catalog's, ASCII and half-width kana
python3 -I - i18n/ja.msg "$W/chars.txt" <<'PY'
import sys
cs = set(chr(c) for c in range(0x20, 0x7f)) | set(chr(c) for c in range(0xff61, 0xffa0)) | set(chr(c) for c in range(0x3000, 0x3100))
for a in range(0xa1, 0xff):
    for b in range(0xa1, 0xff):
        try: cs.add(bytes([a, b]).decode("euc_jp"))
        except UnicodeDecodeError: pass
cs |= set(open(sys.argv[1], encoding="utf-8").read())
open(sys.argv[2], "w", encoding="utf-8").write("".join(sorted(c for c in cs if c.isprintable() or c.isspace() and ord(c) >= 0x20)))
PY
for s in Regular Bold; do
  pyftsubset "$W/dl/NotoSansCJKjp-$s.otf" --text-file="$W/chars.txt" --layout-features+=vert,vrt2,palt,halt --no-hinting \
    --name-IDs='*' --name-languages='*' --output-file="$W/out/NotoSansCJKjp-$s.subset.otf"
done
cp "$W/out/"* "$OUT/"
printf 'Noto Sans CJK JP %s (Regular, Bold), subset\nSource: %s/OTF/Japanese/NotoSansCJKjp-{Regular,Bold}.otf, licence %s/LICENSE\nLicence: SIL Open Font License 1.1 (OFL.txt), (c) 2014-2021 Adobe (http://www.adobe.com/), with Reserved Font Name '"'"'Source'"'"'\nSubset by scripts/fetch-cjk-font.sh: JIS X 0208 (level 1 and 2), half-width kana, CJK symbols and kana blocks, ASCII, every character of i18n/ja.msg; layout features kept, hinting dropped. Family name unchanged (Noto Sans CJK JP), so the fallback finds it by name.\nFetched: %s\n' "2.004" "$BASE" "$BASE" "$(date -u +%Y-%m-%d)" > "$OUT/SOURCE.txt"
(cd "$OUT" && sha256sum NotoSansCJKjp-Regular.subset.otf NotoSansCJKjp-Bold.subset.otf OFL.txt > SHA256SUMS && ls -l)
