# Sourced by the scripts that compile Bun programs into Backplane (backplane-browser, backplane-step2glb).
# deploy/bun-version is the Bun a release builds them with: release.yml's setup-bun pins it, deploy/licenses/bun names it, and scripts/licenses.sh
# refuses a release built with another.
# bun_pin_check BUN: say so on stderr when BUN is another version, so a dev build that differs from a release is never silent.
bun_pin_check() {
  _pin=$(tr -d ' \r\n' < "$(cd "$(dirname "$0")/.." && pwd)/deploy/bun-version" 2>/dev/null)
  _have=$("$1" --version 2>/dev/null | tr -d ' \n')
  [ -z "$_pin" ] || [ "$_have" = "$_pin" ] || echo "warning: $1 is Bun ${_have:-unknown}; releases build with Bun $_pin (deploy/bun-version). Install it with: curl -fsSL https://bun.sh/install | bash -s bun-v$_pin" >&2
}
