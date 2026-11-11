# MCP servers

Settings > MCP servers is one list of MCP servers that every agent Backplane runs gets: Claude, Codex, Antigravity, Grok, OpenCode. Adding a server to Codex or Claude Code on the desktop does not change Backplane. It keeps its own list, and Import brings theirs in. Backplane's own `backplane` server is always there. `pcbparts`, the parts search, is built in and can be turned off.

The code is in `native/host/src/mcpx/`. The pages are drawn by `native/gpui/src/screens/cards.rs`, `mobile/ios/Backplane/DataMcp.swift` and `mobile/android/.../DataMcp.kt`.

## Adding a server

- **Paste.** Paste what a vendor's docs give, and the page reads it as you type (`parse.rs`). It shows one card per server it found, each with a tick. It reads:
  - `mcpServers` JSON (Claude Code, Claude Desktop, Cursor, Windsurf, Gemini CLI);
  - VS Code's `servers`, with its `inputs`, and `mcp.servers` in its settings;
  - OpenCode's `mcp`, and Zed's `context_servers`;
  - Codex's TOML `[mcp_servers.<name>]` tables, or dotted keys;
  - a single server object, or a `"name": {…}` copied without its braces;
  - a `claude mcp add …` or `codex mcp add …` line, a bare command (`npx -y @scope/server`, `KEY=v uvx pkg`), or a URL.

  JSON with comments and trailing commas (JSONC) is read too.
- **Command.** A name, the command as it would be typed in a shell, `KEY=value` lines for the environment, and a folder.
- **Remote URL.** Streamable HTTP or the older SSE, with `Name: value` header lines.
- **Gallery** (`gallery.rs`). Every entry was checked to exist on 2026-11-11: each page was read, and each remote endpoint answered an unauthenticated `initialize`. The vendor documentation servers are:
  - Microchip MPLAB Docs and Microchip Product Data (both official, remote, no key);
  - the ESP Component Registry (Espressif, remote);
  - STM32 Docs (community, `uvx`).

  The other entries are KiCad MCP Pro (community), DeepWiki, Context7 (its key is optional), GitHub (a token is needed), and the MCP reference servers: fetch, filesystem, git, time, memory and everything.

  "Community" on a card means the server is not made by the vendor. Four vendor servers want an OAuth sign-in, which Backplane does not do yet: Espressif's docs, Nordic, NXP and Silicon Labs. They are listed as "Needs sign-in" with a link, and cannot be added. No Raspberry Pi documentation MCP server was found.
- **Import** (`import.rs`). This reads the servers already set up in the person's tools, and changes nothing of theirs:
  - Claude Code: `~/.claude.json`, at the top and per project, and each project's `.mcp.json`;
  - Codex: `~/.codex/config.toml`, or `$CODEX_HOME`;
  - Cursor, VS Code (`mcp.json` and `settings.json`), Claude Desktop, Windsurf, Gemini CLI and OpenCode, in their Linux, macOS and Windows places, plus the project-level files of each project.

  Every server found is ticked, except one that is already here (same command and arguments, or same URL). **Keep in sync** makes an imported server follow its file: the command, env, URL and headers come from the file, and the person's on/off, scope and tools stay theirs. Synced servers are read again when the hub starts and on every Import.

Each new server is tested as soon as it is added.

## Secrets

A key is never written in plain text, neither in `servers.json` nor in any provider's config. An env value or header is either text or a vault reference (`{"k","v","secret":true,"pre"}`, where `v` is the vault name). A value is treated as a secret when:
- its name looks like one (KEY, TOKEN, SECRET, PASSWORD, AUTH, PAT, BEARER, COOKIE, CREDENTIAL…);
- or it is a placeholder (`${VAR}`, `${input:x}`, `<YOUR_KEY>`, `YOUR_API_KEY`).

A real value pasted or imported under a secret name goes straight to the vault as `mcp-<id>-<key>` (vault rules: docs/secrets.md). A placeholder becomes a reference to fill in: the card says "Needs a key" and shows a masked field, which saves through the vault's own path (`secret.set`, never a page action). `Bearer ` and `Token ` stay in front of the value (`pre`), so the vault holds only the token. Removing a server deletes its own `mcp-<id>-*` secrets if no other server uses them.

## How agents get them

Each agent process is started with the servers its thread gets (`svc::launch`, `gen.rs`). A thread gets a server when it is on and in scope:
- **Agents:** all, or the chosen providers.
- **Projects:** all, or the chosen projects. The scope is applied when an agent starts. Nothing is written into the project's own folder.
- **Bots:** every bot, none, or the chosen bots. A person's own threads are not affected by the bot choice.

**Stdio servers go through the hub's wrapper:** `backplane-hub --mcp-run <id>` (`run.rs`). The wrapper asks the hub, over the thread's own MCP endpoint (`backplane/mcp_spec`), for the command and its env with the vault's values put in. The hub answers only for a server that thread gets, and only for its current agent process. The wrapper then does the following:
- starts the server where the agent runs, on the agents' PATH;
- passes JSON-RPC lines both ways;
- takes the tools that are turned off out of `tools/list`, and answers a call to one with an error;
- reports each tool call (the card's "used … ago") and a server that dies (its error and the end of its stderr).

So no secret is ever on a command line or in a file, and every provider gets the same tool filter.

**Remote servers are given to each provider in its own form:**

| Provider | Config | A secret header |
|---|---|---|
| Claude Code | `--mcp-config` JSON (`type` http/sse, `url`, `headers`), and `--disallowedTools mcp__<id>__<tool>` for turned-off tools | `headersHelper` runs `backplane-hub --mcp-headers <id> <url>`, which prints the headers |
| Codex | `-c mcp_servers.<id>.url=…`, `http_headers`, `disabled_tools`, `startup_timeout_sec` | `env_http_headers`: an env variable of the Codex process named `BACKPLANE_MCP_<ID>_<n>_TOKEN`, which Codex's shell policy keeps out of the commands it runs |
| Antigravity, Grok, OpenCode (ACP) | `mcpServers` in `session/new` / `load` / `resume` | `headers` on the agent's stdin |

Muse, Pi, Oh My Pi and the agy CLI mode still get Backplane's own server only.

**Changes apply from each thread's next turn, without a hub restart.** Codex and the ACP agents start a session per turn. A Claude process lives across turns, so the list's fingerprint is part of its agent key: when its servers change, it is started again at its next turn on the same session.

**Machines.** A server runs where the agent runs, which is the hub's machine. Each hub has its own list, and each machine's Settings shows that machine's list. A stdio card whose command is not found on that machine says "Not on <machine>".

## Status, check and tools

A card shows:
- the server's tile, name, transport and status pill: Ready, Checking, Needs a key, Not on <machine>, Error, Off or Not checked;
- its command or URL;
- its tool, resource and prompt counts, its last use, and its scope.

Opening a card shows its check, its last error with stderr, the scope chips, its tools (each with a switch), and Test, Edit, Duplicate and Remove (Remove asks twice).

**Test** (`health.rs`) starts the server as an agent would, sends `initialize`, then lists its tools, resources and prompts. It waits up to 90 s, because a first `npx` or `uvx` downloads the package.
- A stdio server runs here.
- A remote server is reached with curl. Its headers go in a 0600 config file, never on the command line. Streamable HTTP keeps the session id the server gives; SSE waits for the stream's `endpoint` event.

Reports, the last use and the last error are kept in `<home>/mcp/state.json`.

## Agents managing servers

The `backplane` MCP server has these tools (`tools.rs`):
- `mcp_list`
- `mcp_add` (fields, a `snippet` as the person would paste it, or a `gallery` id)
- `mcp_update`
- `mcp_remove`
- `mcp_test`
- `mcp_import` (without `picks` it lists what it found; with `picks` it imports them)

They use the same list and the same per-provider config as Settings.

**Confirm by default.** A server runs code on this machine. So adding one, turning one on, or changing what it runs opens an approval card in the thread (tool `mcp_server`), and the person accepts with one tap. Turning a server off, narrowing its scope, turning tools off and removing a server apply at once. Settings > MCP servers > Agents adding servers lets a project's agents skip the card (`mcp.agents.<project>` = `allow`). The agent's turn ends, and a message tells it the result. It gets the server from its next turn.

**Secrets.** A key in plain text is refused, with the fix in the error: `secret_request` a name, then pass `{"secret": "<name>"}` (with `"prefix": "Bearer "` for a header). A snippet that carries a real key is refused the same way. An import whose file has keys in it must be done by the person, because only their Import moves keys to the vault.

## Pages and protocol

- The desktop draws the page through the presenter (`present/settings.rs` `mcp`). Its state is in the drafts `@mcp.*`, and what is typed is in `@f.m*`.
- The phones use `mcp.page {ui, fields}` → `{rows, ui}` and `mcp.page.act {action, value, ui, fields}` → `{rows, ui}` (feature `mcpsettings`).
- The rows are the usual `{label, note, buttons, fields}`, so an older client still draws them, plus a `kind`: `head`, `group`, `card`, `tool`, `notice`, `code`, `empty`, `panel`, `paste` or `actions`. A row with `in: true` is drawn inside the opened card above it (page.rs has the schema).
- Presses that only move the page are `page::step`, a pure function every client shares.

## Tests

- **Unit tests:** `mcpx::*` in `native/host`:
  - every parser: each JSON shape, TOML, command lines, JSONC;
  - every generator: Claude, Codex (each `-c` value read back as TOML) and ACP;
  - import from a sample home, and sync;
  - the gallery (each entry makes a valid server);
  - the page rows and steps, the wrapper's tool filter, the live check against a real stdio server, and the agent tools' refusal of plain keys.

  Also `screens::cards` in `native/gpui`, and `McpPageTests` (iOS) and the Android unit tests for the phones.
- **End to end:** `test/e2e/mcp/real.sh <hub>` runs on a temp hub home, never the live one.
  1. It adds a stdio probe server (`probe_server.py`) the way Settings does, with its key typed into the env, and pastes `npx -y @modelcontextprotocol/server-everything`.
  2. It waits for both to be Ready.
  3. It runs a **real Claude turn and a real Codex turn** that call the probe's `stamp` and everything's `echo`.
  4. It checks that the vault's key reached the server's environment, and that the key is in no file of the hub's home except the sealed vault.
  5. It checks that a turned-off tool was never called.
  6. It has Claude `mcp_add` the gallery's `time`, and checks that nothing is added until the confirm is accepted.

  Part 2 (`SKIP_REAL=1` runs it alone) imports from a sample `~/.codex/config.toml` and `~/.claude.json`.
- **Screenshots:** `scripts/desktop-mcp-shot.sh` (desktop, `THEME=dark|light`), `test/e2e/ios/mcp-shot.sh`, and the Android shot script.
