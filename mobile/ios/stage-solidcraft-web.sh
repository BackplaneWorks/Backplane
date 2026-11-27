#!/bin/sh
# Downloads, verifies (SHA-256), pre-compresses (.wasm.gz / .js.gz), and stages
# SolidCraft Web assets into mobile/ios/Backplane/SolidCraftWeb.bundle (never committed).
set -eu

HERE=$(cd "$(dirname "$0")" && pwd)
VERSION="0.50.0"
ASSET="solidcraft-web-${VERSION}.zip"
ROOT_DIR="solidcraft-web-${VERSION}"
SHA256="22fde12d3d98110b997f30e888f604609a70f1a4ebaf488c5d8c31097d3fe328"
URL="${BACKPLANE_SOLIDCRAFT_FEED:-https://github.com/tangency-labs/solidcraft/releases/download/v${VERSION}/}${ASSET}"
CACHE="${SOLIDCRAFT_WEB_CACHE:-$HOME/.cache/backplane-solidcraft-web}"
PREPARED="$CACHE/prepared-${VERSION}"
DEST="$HERE/Backplane/SolidCraftWeb.bundle"

sha256_of() {
  if command -v shasum >/dev/null 2>&1; then
    shasum -a 256 "$1" | awk '{print $1}'
  else
    sha256sum "$1" | awk '{print $1}'
  fi
}

mkdir -p "$CACHE" "$DEST"

if [ ! -f "$PREPARED/.ready" ] || [ "$(cat "$PREPARED/.ready" 2>/dev/null || true)" != "$SHA256" ]; then
  ZIP="$CACHE/$ASSET"
  GOT=""
  [ -f "$ZIP" ] && GOT=$(sha256_of "$ZIP")
  if [ "$GOT" != "$SHA256" ]; then
    rm -f "$ZIP" "$ZIP.part"
    echo "stage-solidcraft-web: downloading $ASSET..."
    curl -fsSL --retry 3 -o "$ZIP.part" "$URL"
    GOT=$(sha256_of "$ZIP.part")
    if [ "$GOT" != "$SHA256" ]; then
      rm -f "$ZIP.part"
      echo "stage-solidcraft-web: SHA-256 mismatch (got $GOT, want $SHA256)" >&2
      exit 1
    fi
    mv "$ZIP.part" "$ZIP"
  fi

  STAGE=$(mktemp -d "$CACHE/.stage.XXXXXX")
  trap 'rm -rf "$STAGE"' EXIT
  unzip -q "$ZIP" -d "$STAGE/unpack"
  SRC="$STAGE/unpack/$ROOT_DIR"
  [ -d "$SRC" ] || { echo "stage-solidcraft-web: missing $ROOT_DIR inside zip" >&2; exit 1; }

  JS_NAME=$(cd "$SRC" && ls -1 solidcraft-web-*.js 2>/dev/null | grep -v '_bg\.js$' | head -n 1 || true)
  WASM_NAME=$(cd "$SRC" && ls -1 solidcraft-web-*_bg.wasm 2>/dev/null | head -n 1 || true)
  CSS_NAME=$(cd "$SRC" && ls -1 loading-*.css 2>/dev/null | head -n 1 || true)
  [ -n "$JS_NAME" ] && [ -n "$WASM_NAME" ] || { echo "stage-solidcraft-web: missing js/wasm in $SRC" >&2; exit 1; }

  OUT="$STAGE/prepared"
  mkdir -p "$OUT"
  gzip -9 -c "$SRC/$WASM_NAME" > "$OUT/${WASM_NAME}.gz"
  gzip -9 -c "$SRC/$JS_NAME" > "$OUT/${JS_NAME}.gz"
  [ -n "$CSS_NAME" ] && cp "$SRC/$CSS_NAME" "$OUT/$CSS_NAME"
  [ -f "$SRC/loading-logo.svg" ] && cp "$SRC/loading-logo.svg" "$OUT/loading-logo.svg"
  [ -d "$SRC/fonts" ] && cp -R "$SRC/fonts" "$OUT/fonts"
  for f in LICENSE-APACHE LICENSE-MIT NOTICE; do
    [ -f "$SRC/$f" ] && cp "$SRC/$f" "$OUT/$f"
  done

  WASM_SIZE=$(wc -c < "$SRC/$WASM_NAME" | tr -d ' ')
  JS_SIZE=$(wc -c < "$SRC/$JS_NAME" | tr -d ' ')
  cat > "$OUT/manifest.json" <<JSON
{"version":"$VERSION","js":"$JS_NAME","wasm":"$WASM_NAME","css":"$CSS_NAME","wasmSize":$WASM_SIZE,"jsSize":$JS_SIZE}
JSON
  printf '%s\n' "$SHA256" > "$OUT/.ready"

  rm -rf "$PREPARED"
  mv "$OUT" "$PREPARED"
fi

rsync -a --delete --exclude .ready "$PREPARED/" "$DEST/"
du -sh "$DEST"
