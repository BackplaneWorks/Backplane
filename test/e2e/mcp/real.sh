#!/bin/bash
# The MCP end-to-end (docs/MCP.md "Tests"). Part 1 runs REAL Claude and Codex turns (the person's own logins) on a TEMP hub home, never the
# live one: a stdio server added with a vault key, a pasted third-party server, both called by each provider, the key checked to reach the
# server and nowhere else on disk. Part 2 imports from a sample ~/.codex/config.toml and ~/.claude.json on a second temp hub with stand-in
# agents and a fake HOME. usage: test/e2e/mcp/real.sh HUB_BIN   (SKIP_REAL=1: part 2 only)
set -eu
ROOT=$(cd "$(dirname "$0")/../../.." && pwd); HUBBIN=$(realpath "$1"); D=$(dirname "$(realpath "$0")")
. "$ROOT/scripts/lib-bptmp.sh"; . "$ROOT/test/e2e/lib/own-group.sh"
W=$(mktemp -d "$BP_TMP/mcp-e2e.XXXXXX"); mkdir -p "$W/home" "$W/proj"; cd "$W/proj" && git init -q . && cd "$W"
SECRET="bp-e2e-$(od -An -tx1 -N8 /dev/urandom | tr -d ' \n')"
fail=0
if [ "${SKIP_REAL:-}" != 1 ]; then
  own env BACKPLANE_NO_UPDATE=1 BACKPLANE_VAULT_KEYRING=file "$HUBBIN" --home "$W/home" --port 3881 --host 127.0.0.1 --no-tailscale --foreground > "$W/hub.log" 2>&1
  for k in $(seq 1 120); do grep -q "serving" "$W/hub.log" 2>/dev/null && break; sleep 0.5; done; sleep 1
  bun "$D/real.ts" 3881 "$W/home/token" "$W" "$SECRET" > "$W/real.json" || fail=1
  cat "$W/real.json"
  # the key is in the vault (sealed) and nowhere else the hub writes
  if grep -rl --exclude=vault.json -- "$SECRET" "$W/home" 2>/dev/null; then echo "FAILED: the key is in plain text in the files above"; fail=1; else echo "the key is in no file of the hub's home but the sealed vault: ok"; fi
  own_stop || true
fi
# part 2: import
H="$W/fakehome"; mkdir -p "$H/.codex" "$W/home2" "$W/bin"; for n in claude codex; do printf '#!/bin/sh\nexit 1\n' > "$W/bin/$n"; chmod +x "$W/bin/$n"; done
printf 'model = "gpt-6"\n\n[mcp_servers.context7]\ncommand = "npx"\nargs = ["-y", "@upstash/context7-mcp"]\n\n[mcp_servers.fetch]\ncommand = "uvx"\nargs = ["mcp-server-fetch"]\n\n[mcp_servers.deepwiki]\nurl = "https://mcp.deepwiki.com/mcp"\n' > "$H/.codex/config.toml"
printf '{"numStartups":3,"mcpServers":{"kicad":{"type":"stdio","command":"uvx","args":["kicad-mcp-pro","--transport","stdio"],"env":{}}},"projects":{"/home/x/esp32-motor":{"mcpServers":{"jlc":{"command":"node","args":["jlc.js"],"env":{"JLC_API_KEY":"jlc-live-0123456789"}}}}}}\n' > "$H/.claude.json"
own env HOME="$H" BACKPLANE_AGENTS="$W/bin" BACKPLANE_NO_UPDATE=1 BACKPLANE_VAULT_KEYRING=file "$HUBBIN" --home "$W/home2" --port 3882 --host 127.0.0.1 --no-tailscale --foreground > "$W/hub2.log" 2>&1
for k in $(seq 1 120); do grep -q "serving" "$W/hub2.log" 2>/dev/null && break; sleep 0.5; done; sleep 1
bun "$D/import.ts" 3882 "$W/home2/token" "$H" "$W/home2" || fail=1
if grep -rl --exclude=vault.json -- "jlc-live-0123456789" "$W/home2" 2>/dev/null; then echo "FAILED: the imported key is in plain text"; fail=1; else echo "the imported key is only in the sealed vault: ok"; fi
own_stop || true
[ $fail = 0 ] && rm -rf "$W" || echo "kept $W"
exit $fail
