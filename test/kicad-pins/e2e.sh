#!/bin/bash
# The KiCad pin feed E2E (docs/kicad-pins.md): a real hub built with feature e2e-trust, on a private home and port, reads kicad-pins.json from a
# loopback server (BACKPLANE_KICAD_PINS_URL), signed with a key made for this run by scripts/kicad-pins.py and openssl as the workflow does.
# Never the real key, never a real version: a test key's pins must be below 0.1.0 (rel.rs E2E_CEILING), so none could reach a real hub even
# with the feature. Linux x86_64 only (the pins are that platform's).
#   test/kicad-pins/e2e.sh BIN_DIR [CASE...]
#     BIN_DIR: backplane-hub built with `--features e2e-trust`
#     CASE: good wrongkey tampered ceiling replay offline expired (none: all)
# Env: PINS_DIR (scratch, default $BP_TMP), PINS_PORT (first port, default 3950).
set -u
BIN=$(cd "${1:?usage: e2e.sh BIN_DIR [CASE...]}" && pwd); shift
HERE=$(cd "$(dirname "$0")" && pwd); ROOT=$HERE/../..
. "$ROOT/test/e2e/lib/own-group.sh"
[ "$(uname -s)-$(uname -m)" = Linux-x86_64 ] || { echo "Linux x86_64 only"; exit 2; }
strings "$BIN/backplane-hub" | grep -q backplane-e2e-trust-enabled || { echo "$BIN/backplane-hub was not built with feature e2e-trust"; exit 2; }
T=$(mktemp -d "${PINS_DIR:-${BP_TMP:-$HOME/.cache/bp-tmp}}/kicad-pins-e2e.XXXXXX"); PORT=$((${PINS_PORT:-3950}-1)); PASS=0; FAILS=()
HOME_REAL=$HOME
export HOME=$T/userhome BACKPLANE_AGENTS=$T/agents BACKPLANE_KICAD_NO_SYSTEM=1 BACKPLANE_NO_UPDATE=1
mkdir -p "$HOME" "$T/agents" "$T/keys"; unset BACKPLANE_COPY BACKPLANE_KICAD_CLI BACKPLANE_KICAD_FEED BACKPLANE_KICAD_PINS BACKPLANE_AGENT_OWNER DISPLAY WAYLAND_DISPLAY
trap 'own_stop; rm -rf "${T:?}/keys"; [ -n "${PINS_KEEP:-}" ] || rm -rf "${T:?}"' EXIT

openssl genpkey -algorithm ed25519 -out "$T/keys/e2e.key" 2>/dev/null && openssl pkey -in "$T/keys/e2e.key" -pubout -out "$T/keys/e2e.pub" || { echo "openssl cannot make ed25519 keys"; exit 2; }
openssl genpkey -algorithm ed25519 -out "$T/keys/other.key" 2>/dev/null
export BACKPLANE_E2E_TRUST_KEY=$T/keys/e2e.pub
keyid() { openssl pkey -in "$1" -pubout -outform DER | tail -c 32 | sha256sum | cut -c1-16; }
P="python3 -I $ROOT/scripts/kicad-pins.py"

ok() { PASS=$((PASS+1)); printf 'PASS %-9s %s\n' "$CUR" "$*"; own_stop; }
no() { FAILS+=("$CUR"); printf 'FAIL %-9s %s\n' "$CUR" "$*"; grep -E 'kicad: (pins|install|runtime)' "$C/hub.log" 2>/dev/null | tail -6 | cut -c1-220 | sed 's/^/    /'; own_stop; }
until_() { local n=$1; shift; local i=0; while [ "$i" -lt $((n*4)) ]; do if "$@"; then return 0; fi; sleep 0.25; i=$((i+1)); done; return 1; }
logs() { grep -qF -- "$1" "$C/hub.log" 2>/dev/null; }

# doc NAME SEQ VERSION KEY [SHA] [DAYS]: kicad-pins.json under $C/feed/NAME, pinning linux-x86_64 to VERSION, signed by KEY
doc() {
  local name=$1 seq=$2 v=$3 key=$4 sha=${5:-$(printf 'a%.0s' $(seq 64))} days=${6:-30} w=$C/w/$1
  mkdir -p "$w" "$C/feed"
  printf '{"linux-x86_64":{"version":"%s","release":"backplane-kicad-v%s","asset":"backplane-kicad-linux-x86_64-%s.tar.gz","sha256":"%s","size":4096,"caps":["drc-zone-guard"]}}' "$v" "$v" "$v" "$sha" > "$w/pin.json"
  printf '{"schema":1,"kind":"backplane-kicad-pins","sequence":%s,"expires":1,"pins":{}}' $((seq-1)) > "$w/cur.json"
  if [ "$seq" -gt 1 ]; then $P next --current "$w/cur.json" --platform linux-x86_64 --pin "$w/pin.json" --min-hub 0.0.1 --days "$days" > "$w/body.json"
  else $P next --platform linux-x86_64 --pin "$w/pin.json" --min-hub 0.0.1 --days "$days" > "$w/body.json"; fi || return 1
  openssl pkeyutl -sign -inkey "$key" -rawin -in "$w/body.json" -out "$w/body.sig" && $P seal "$w/body.json" "$w/body.sig" "$(keyid "$key")" > "$C/feed/$name"
}
expire() { python3 -I - "$1" <<'PY'   # re-seal NAME's body with an expiry in the past (next refuses to write one)
import json, sys
b = json.load(open(sys.argv[1])); b["expires"] = 1000; open(sys.argv[1], "w").write(json.dumps(b))
PY
}
serve() { own python3 -I -m http.server "$FPORT" --bind 127.0.0.1 --directory "$C/feed" > "$C/feed.log" 2>&1; until_ 10 curl -s -o /dev/null "http://127.0.0.1:$FPORT/"; }
# a stand-in installer: it gets the pin the hub took (the script's arguments: parent platform version release asset sha256 format cli) and installs a
# runtime of that version, so the hub finds it as the new KiCad
installer() {
  cat > "$T/installer" <<'SH'
#!/bin/sh
echo "$*" >> "$INSTALLER_ARGS"
d=$1/$3; mkdir -p "$d/bin" "$d/share/backplane"; printf '#!/bin/sh\necho %s\n' "$3" > "$d/bin/kicad-cli"; chmod +x "$d/bin/kicad-cli"
printf '{"version":"%s","platform":"%s","apiVersion":2}' "$3" "$2" > "$d/manifest.json"; echo x > "$d/share/backplane/api.desc"
echo "Installed in $d"
SH
  chmod +x "$T/installer"
}
hub() { # hub [URL]: the hub on this case's home, reading the pins from URL (default: the case's feed)
  own env BACKPLANE_KICAD_PINS_URL="${1:-http://127.0.0.1:$FPORT/kicad-pins.json}" BACKPLANE_KICAD_INSTALLER="$T/installer" INSTALLER_ARGS="$C/installer.args" \
    "$BIN/backplane-hub" --home "$H" --port "$PORT" --no-tailscale --foreground >> "$C/hub.log" 2>&1 < /dev/null
}
rpc() { (cd "$ROOT" && timeout 30 bun test/kicad_embed/lib/hub_rpc.mjs "$PORT" "$H/token" "$1"); }
go() { CUR=$1; [ "${#WANT[@]}" = 0 ] || [[ " ${WANT[*]} " == *" $1 "* ]] || return 1
  C=$T/$CUR; H=$C/home; PORT=$((PORT+1)); FPORT=$((PORT+500)); mkdir -p "$H" "$C/feed"; python3 -I "$ROOT/test/guard/make-home.py" "$H" --small > /dev/null; return 0; }
WANT=("$@"); installer

# good: the feed's pin is taken, kept on disk, and drives the install (the installer is handed the feed's version and sha256)
if go good; then
  SHA=$(printf 'c%.0s' $(seq 64)); doc kicad-pins.json 1 0.0.1-bp.2 "$T/keys/e2e.key" "$SHA"; serve; hub
  if ! until_ 60 logs "kicad: pins: sequence 1: Backplane KiCad 0.0.1-bp.2 for this computer"; then no "the pin was never taken"
  elif [ ! -s "$H/kicad-pins/pins.json" ]; then no "the document was not kept"
  else
    r=$(rpc '[{"m":"kicad.install","p":{"consent":true}}]' 2>&1)
    if ! until_ 30 logs "kicad: installed Backplane KiCad 0.0.1-bp.2"; then no "no install of the feed's pin: $r"
    elif ! grep -q "linux-x86_64 0.0.1-bp.2 backplane-kicad-v0.0.1-bp.2 backplane-kicad-linux-x86_64-0.0.1-bp.2.tar.gz $SHA tar.gz bin/kicad-cli" "$C/installer.args"; then no "the installer was not handed the feed's pin: $(cat "$C/installer.args")"
    elif ! until_ 20 grep -qE "kicad: runtime [a-z]+ 0.0.1-bp.2 at $H/kicad/0.0.1-bp.2/" "$C/hub.log"; then no "the new runtime was not found"
    else ok "sequence 1 taken, cached, installed 0.0.1-bp.2 with the feed's sha256"; fi
  fi
fi
# wrongkey: a document signed by a key the hub does not trust is refused; the compiled pin stands
if go wrongkey; then
  doc kicad-pins.json 1 0.0.1-bp.2 "$T/keys/other.key"; serve; hub
  if ! until_ 60 logs "kicad: pins: the pin document is signed with a key this build does not trust; keeping the compiled pins"; then no "not refused"
  elif [ -e "$H/kicad-pins/pins.json" ]; then no "a refused document was kept"; else ok "refused: another key"; fi
fi
# tampered: one byte of the signed body changed after signing
if go tampered; then
  doc kicad-pins.json 1 0.0.1-bp.2 "$T/keys/e2e.key"
  python3 -I - "$C/feed/kicad-pins.json" <<'PY'
import base64, json, sys
e = json.load(open(sys.argv[1])); b = base64.b64decode(e["doc"]).replace(b"0.0.1-bp.2", b"0.0.1-bp.3"); e["doc"] = base64.b64encode(b).decode(); open(sys.argv[1], "w").write(json.dumps(e))
PY
  serve; hub
  if until_ 60 logs "kicad: pins: the pin document's signature does not match; keeping the compiled pins"; then ok "refused: the body changed after signing"; else no "not refused"; fi
fi
# ceiling: the test key may not pin a real version
if go ceiling; then
  doc kicad-pins.json 1 10.99.0-bp.99 "$T/keys/e2e.key"; serve; hub
  if until_ 60 logs "a test-key release must be below 0.1.0"; then ok "refused: a test key pinning 10.99.0-bp.99"; else no "not refused"; fi
fi
# replay: sequence 2 is taken; after a restart an older sequence 1 is refused and the cached 2 stays
if go replay; then
  doc kicad-pins.json 2 0.0.1-bp.3 "$T/keys/e2e.key"; doc old.json 1 0.0.1-bp.2 "$T/keys/e2e.key"; serve; hub
  if ! until_ 60 logs "kicad: pins: sequence 2: Backplane KiCad 0.0.1-bp.3"; then no "sequence 2 not taken"
  else
    own_stop; serve; hub "http://127.0.0.1:$FPORT/old.json"
    if ! until_ 60 logs "kicad: pins: cached sequence 2: Backplane KiCad 0.0.1-bp.3"; then no "the cached document was not read back"
    elif ! until_ 60 logs "sequence 1 is below the last one seen (2); keeping Backplane KiCad 0.0.1-bp.3 from the feed"; then no "the replay was not refused"
    else ok "a replayed sequence 1 refused after a restart; 0.0.1-bp.3 kept"; fi
  fi
fi
# offline: nothing answers; the compiled pin stands, and with a cached document its pin stays
if go offline; then
  hub "http://127.0.0.1:9/kicad-pins.json"
  if until_ 60 logs "could not reach the KiCad pin feed"; then ok "offline: the compiled pins stand"; else no "no word of the unreachable feed"; fi
fi
# expired: a document past its expiry is refused
if go expired; then
  doc kicad-pins.json 1 0.0.1-bp.2 "$T/keys/e2e.key"; w=$C/w/kicad-pins.json; expire "$w/body.json"
  openssl pkeyutl -sign -inkey "$T/keys/e2e.key" -rawin -in "$w/body.json" -out "$w/body.sig"
  python3 -I - "$w/body.json" "$w/body.sig" "$(keyid "$T/keys/e2e.key")" > "$C/feed/kicad-pins.json" <<'PY'
import base64, json, sys
print(json.dumps({"schema": 1, "keyid": sys.argv[3], "sig": base64.b64encode(open(sys.argv[2], "rb").read()).decode(), "doc": base64.b64encode(open(sys.argv[1], "rb").read()).decode()}))
PY
  serve; hub
  if until_ 60 logs "kicad: pins: the pin document has expired; keeping the compiled pins"; then ok "refused: expired"; else no "not refused"; fi
fi

echo "kicad-pins e2e: $PASS passed${FAILS[*]:+, failed: ${FAILS[*]}}"
[ "${#FAILS[@]}" = 0 ]
