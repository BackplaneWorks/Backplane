# Source from a repository entry point after cd to the root.
# Inherit the actual flock descriptor so nested heavy commands stay serialized.
heavy_guard() {
  profile=$1; shift
  if python3 scripts/heavy-guard.py --held "$profile" -- "$0" "$@"; then return; else rc=$?; fi
  [ "$rc" = 1 ] || exit "$rc"
  exec python3 scripts/heavy-guard.py "$profile" -- "$0" "$@"
}
