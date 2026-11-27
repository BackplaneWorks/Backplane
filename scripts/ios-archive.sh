#!/bin/sh
# The native iOS app to TestFlight, from the Mac (~/bp-gpui-ios/ios), signed with the existing App Store credentials (signing.env,
# ~/.bp-sign) and the bundle that builds already go to. Run under the mac-heavy lock.
#   scripts/ios-archive.sh version   print the resolved MARKETING_VERSION synced with the hub
#   scripts/ios-archive.sh check     a Release device build with GPUI_NATIVE, no signing, no upload (does it compile?)
#   scripts/ios-archive.sh upload    archive, sign and upload to App Store Connect; prints the version and build number
# The build number is the date and minute (monotonic: the last upload was 202611031753). BP_MEASURE is never set here: that flag takes a
# pairing link from the environment, and an archive must not.
set -eu

ROOT=$(cd "$(dirname "$0")/.." && pwd)
cd "$ROOT" || exit 1

MODE=${1:-check}
case "$MODE" in
  version|check|upload) ;;
  *) echo "usage: $0 [version|check|upload]" >&2; exit 2 ;;
esac

normalize_version() {
  raw=$1
  v=${raw#v}
  v=${v%%-*}
  case "$v" in
    ''|*[!0-9.]*|.*|*.|*..*) return 1 ;;
  esac
  old_ifs=$IFS; IFS=.
  set -- $v
  IFS=$old_ifs
  [ $# -eq 2 ] || [ $# -eq 3 ] || return 1
  printf '%s\n' "$v"
}

semver_gt() {
  lhs=$1 rhs=$2
  old_ifs=$IFS; IFS=.
  set -- $lhs
  a1=${1:-0} a2=${2:-0} a3=${3:-0}
  set -- $rhs
  b1=${1:-0} b2=${2:-0} b3=${3:-0}
  IFS=$old_ifs
  for n in a1 a2 a3 b1 b2 b3; do
    eval "val=\$$n"
    while case "$val" in 0[0-9]*) val=${val#0} ;; *) false ;; esac; do :; done
    eval "$n=\$val"
  done
  [ "$a1" -gt "$b1" ] && return 0
  [ "$a1" -lt "$b1" ] && return 1
  [ "$a2" -gt "$b2" ] && return 0
  [ "$a2" -lt "$b2" ] && return 1
  [ "$a3" -gt "$b3" ] && return 0
  return 1
}

resolve_version() {
  if [ -n "${MARKETING_VERSION:-}" ]; then
    normalize_version "$MARKETING_VERSION" || { echo "ios-archive: invalid MARKETING_VERSION: $MARKETING_VERSION" >&2; return 1; }
    return 0
  fi
  if [ -n "${BACKPLANE_VERSION:-}" ]; then
    normalize_version "$BACKPLANE_VERSION" || { echo "ios-archive: invalid BACKPLANE_VERSION: $BACKPLANE_VERSION" >&2; return 1; }
    return 0
  fi

  head_tags=$(git -C "$ROOT" tag --points-at HEAD 'v[0-9]*' 2>/dev/null || true)
  best_stable=""
  best_pre=""
  for tag in $head_tags; do
    case "$tag" in
      *-*)
        norm=$(normalize_version "$tag" 2>/dev/null) || continue
        if [ -z "$best_pre" ] || semver_gt "$norm" "$best_pre"; then best_pre=$norm; fi
        ;;
      *)
        norm=$(normalize_version "$tag" 2>/dev/null) || continue
        if [ -z "$best_stable" ] || semver_gt "$norm" "$best_stable"; then best_stable=$norm; fi
        ;;
    esac
  done
  if [ -n "$best_stable" ]; then printf '%s\n' "$best_stable"; return 0; fi
  if [ -n "$best_pre" ]; then printf '%s\n' "$best_pre"; return 0; fi

  best=""
  if [ -f "$ROOT/native/host/Cargo.toml" ]; then
    raw_cargo=$(awk '
      /^\[/ { in_pkg = ($0 ~ /^\[package\]([[:space:]]*(#.*)?)?$/) }
      in_pkg && /^[[:space:]]*version[[:space:]]*=/ {
        sub(/^[^"]*"/, ""); sub(/".*$/, ""); print; exit
      }
    ' "$ROOT/native/host/Cargo.toml")
    if [ -n "$raw_cargo" ]; then
      best=$(normalize_version "$raw_cargo") || { echo "ios-archive: invalid version '$raw_cargo' in native/host/Cargo.toml" >&2; return 1; }
    fi
  fi

  rel_tags=$(git -C "$ROOT" tag -l 'v[0-9]*.[0-9]*.[0-9]*' 2>/dev/null || true)
  for tag in $rel_tags; do
    case "$tag" in *-*) continue ;; esac
    norm=$(normalize_version "$tag" 2>/dev/null) || continue
    if [ -z "$best" ] || semver_gt "$norm" "$best"; then best=$norm; fi
  done

  [ -n "$best" ] || { echo "ios-archive: could not resolve hub version from Cargo.toml or git tags" >&2; return 1; }
  printf '%s\n' "$best"
}

VER=$(resolve_version)
if [ "$MODE" = version ]; then
  printf '%s\n' "$VER"
  exit 0
fi

M=${MAC_SSH:-$ROOT/build/gpui/mac-ssh}
if [ ! -x "$M" ]; then
  common=$(git -C "$ROOT" rev-parse --path-format=absolute --git-common-dir 2>/dev/null || true)
  [ -n "$common" ] && [ -x "$(dirname "$common")/build/gpui/mac-ssh" ] && M="$(dirname "$common")/build/gpui/mac-ssh"
fi
[ -x "$M" ] || M=ssh

RSYNC_RSH="$M" rsync -a --delete --exclude build --exclude signing.env --exclude Backplane/SolidCraftWeb.bundle mobile/ios/ hs-mac-mini:bp-gpui-ios/ios/

"$M" hs-mac-mini sh -s -- "$MODE" "$VER" <<'REMOTE'
set -u
MODE=$1
VER=$2
export DEVELOPER_DIR=/Applications/Xcode.app/Contents/Developer
cd ~/bp-gpui-ios/ios || exit 1
[ -f signing.env ] || cp ~/backplane-ios/signing.env signing.env
mkdir -p build
if [ "${SKIP_SOLIDCRAFT_WEB_BUNDLE:-0}" != 1 ]; then
  sh ./stage-solidcraft-web.sh || exit 1
fi
log=$(mktemp build/ios-archive.XXXXXX)
trap 'rm -f "$log"' EXIT
case "$MODE" in
check)
  xcodebuild -quiet -project Backplane.xcodeproj -scheme Backplane -configuration Release -sdk iphoneos -destination 'generic/platform=iOS' \
    -derivedDataPath build/ddc CODE_SIGNING_ALLOWED=NO MARKETING_VERSION="$VER" SWIFT_ACTIVE_COMPILATION_CONDITIONS="GPUI_NATIVE DATA_APP" build </dev/null >"$log" 2>&1
  rc=$?
  tail -40 "$log"
  [ "$rc" -eq 0 ] && echo "check done (version $VER)"
  exit "$rc"
  ;;
upload)
  n=$(date +%Y%m%d%H%M)
  MARKETING_VERSION="$VER" BUILD_NUMBER="$n" sh build.sh archive </dev/null >"$log" 2>&1
  rc=$?
  tail -30 "$log"
  [ "$rc" -eq 0 ] && echo "version $VER, build number $n"
  exit "$rc"
  ;;
*)
  echo "unknown mode: $MODE" >&2
  exit 2
  ;;
esac
REMOTE
