#!/usr/bin/env bash
# The readiness check before a home move (docs/BOAT-HUB.md, docs/ALWAYS-ON.md section 4): asks this hub's `home.check {to}`, which asks the target
# (`home.ready`, a far request) for its build, free disk and memory, and the sign-in of each provider the moving threads use (`auth status` /
# `login status`: no turn, no usage). Nothing is moved. Exit 0 when ready, 1 with the reasons when not, 2+ when the hub could not be asked.
#   scripts/home-move-check.sh TARGET [HOME] [URL]      TARGET: the machine's name or link id (e.g. boat-hub)
# HOME defaults to ~/.backplane-gpui-fresh/home, URL to ws://127.0.0.1:3797.
set -u
TO=${1:?usage: home-move-check.sh TARGET [HOME] [URL]}; HOME_DIR=${2:-$HOME/.backplane-gpui-fresh/home}; URL=${3:-ws://127.0.0.1:3797}
HERE=$(cd "$(dirname "$0")" && pwd)
RUN=$(command -v bun || command -v node) || { echo "needs bun or node" >&2; exit 2; }
out=$("$RUN" "$HERE/hub-rpc.mjs" "$HOME_DIR/token" "$URL" home.check "{\"to\":\"$TO\"}"); rc=$?
[ $rc -le 1 ] || { echo "$out" >&2; exit $rc; }
python3 -c '
import json, sys
r = json.loads(sys.argv[1])
if not r.get("ok"): print("not checked:", r.get("error")); sys.exit(2)
v = r["result"]; c = v.get("checks", {})
print(f"{v.get(\"machine\")}: build {c.get(\"version\")} {c.get(\"commit\")}, disk {c.get(\"disk_mb\")} MB free, memory {c.get(\"mem_mb\")} MB free")
for p, s in (c.get("providers") or {}).items(): print(f"  {p}: signed {s.get(\"signed\")}" + (f" as {s[\"who\"]}" if s.get("who") else "") + (f", accounts {\", \".join(s[\"accounts\"])}" if s.get("accounts") else "") + ("" if s.get("installed", True) else ", not installed"))
if v.get("ready"): print("ready"); sys.exit(0)
for x in v.get("reasons", []): print("not ready:", x)
sys.exit(1)' "$out"
