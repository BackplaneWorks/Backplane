# Sourced by scripts that drive the Pixel: the one place an adb call is made, and the guard in front of every input.
#   A ...      adb for the Pixel (PIXEL_ADB overrides the binary: the tests use a stand-in that logs its calls)
#   inp ...    `adb shell input ...`, refused unless the gpui app (and nothing else: not Google Play, not the launcher) is what the phone shows
S=${S:-52220DLAQ001FC}; PKG=${PKG:-dev.backplane.mobile.gpui}; PIXEL_ADB=${PIXEL_ADB:-adb}
A() { "$PIXEL_ADB" -s "$S" "$@"; }
fg_now() { A shell dumpsys activity activities 2>/dev/null | grep -m1 -E 'topResumedActivity|mResumedActivity' | sed -E 's/^ +//' | cut -c1-160; }
fg_ok() { case "$(fg_now)" in *" $PKG/"*|*"{"*" $PKG/"*) return 0;; esac; return 1; }
inp() {
  if ! fg_ok; then echo "refused: not typing or touching, the phone shows $(fg_now) and not $PKG" >&2; PIXEL_FG_LOST=1; type pixel_abort >/dev/null 2>&1 && pixel_abort "the app is not in the foreground: $(fg_now)"; return 3; fi
  A shell input "$@"
}
