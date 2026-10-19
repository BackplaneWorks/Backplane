#!/bin/sh
# An executable kicad-cli adapter for Backplane's existing CLI path setting.
set -eu
ref=${BACKPLANE_KICAD_FLATPAK_REF:-org.kicad.KiCad}
case "$ref" in
  -*|'') echo 'backplane: invalid KiCad Flatpak ref' >&2; exit 2 ;;
esac
# Backplane's exports/review snapshots can live outside home, including
# /tmp. Grant these on this invocation only; never change user overrides.
# KiCad still uses its own runtime, plugins, libraries and configuration.
if [ -n "${FLATPAK_ID:-}" ]; then
  exec flatpak-spawn --host --watch-bus --directory="$PWD" flatpak run \
    --filesystem=host --filesystem=/tmp --filesystem=/var/tmp \
    --command=kicad-cli "$ref" "$@"
fi
exec flatpak run --filesystem=host --filesystem=/tmp --filesystem=/var/tmp \
  --command=kicad-cli "$ref" "$@"
