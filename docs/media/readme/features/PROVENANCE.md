# Current feature captures

Captured 2026-11-01 from the supplied **0.13.2-dev.20261101.1606** build,
main **36e6a1024**. The current desktop uses this same web client. One fresh hub
and one headless Chromium ran in a private `unshare -rn` namespace with only
loopback, on ports 47830/47831. HOME, Backplane state, browser state and temporary
files were under `scratch/readme/run/C-features`; BACKPLANE_AGENTS named an empty
directory. No agent ran, no chat message was sent, and `/bt` was never submitted.
No build, app-source edit, installation, external API or real user data was used.

The new reproducible sources are in [source/c-20261101/](source/c-20261101/):
`runtime.py`, `drive.mjs`, `capture.mjs`, `export.py`, the complete synthetic
`demo-events.jsonl`, `runtime.json`, MCP request/response records, `navigation.json`,
`agent-audit.json`, `panels.json` and full frames in `raw/`. `runtime.json` records
the supplied hub's SHA-256, viewport 1100 × 800 and `deviceScaleFactor: 2`.
`outputs.json` records the final sizes. These are real 2× frames, with lossless
crops and a lossless vertical composition, without pixel enlargement or drawn UI.
The previous images are in git history (before this capture).

- `link-cards-2x.png` (1000 × 228): “Layout review”, “Miso”, and “Sensor Board” in
  an assistant fixture message. **This build renders plain link labels, not the
  older bordered chips with icons/status.** Actual clicks opened Layout review
  and Miso's bot page. The project link selected Sensor Board, which was already
  the current project; that click alone does not establish a transition from
  another project. `navigation.json` and the three `raw/navigation-*.png` frames
  preserve the observations. The older candidate-binary and unverified-activation
  claims do not describe this replacement image.
- `reference-thread-2x.png`, `reference-room-2x.png`, `reference-project-2x.png`, and
  `reference-bot-2x.png`: actual composer popups, each including its typed sigil.
  `>` is a thread, `#` a room, `%` a project, and `@` a bot. The room has one
  synthetic member. `../reference-menus-200.png` (1476 × 926) stacks these lossless
  crops with 16-pixel gaps; `../source/references.html` describes the same stack.
- `inline-terminal-2x.png` (1480 × 632): an actual inline PTY, opened directly
  through the isolated hub's `terminal_open`, showing the local checklist in
  read-only **Neovim**, with no user configuration, swap or shada file. This host
  has `/usr/bin/nvim` and no `vi` executable. The title states “vi (Neovim)”; the
  footer shows `review.txt [RO]`. `terminal_read.json` confirms `state="running"`
  and the checklist text. `terminal_send.json` preserves Enter/jj; the separate
  `terminal_send-quit.json` preserves `:q`, followed by `terminal_close`.
- `btw-autocomplete-2x.png` (1476 × 390): the unsent `/bt` draft, `/btw`, and its
  real description, “Ask a side question; the thread never sees it”.

The terminal calls use today's `POST /mcp/demo-main?k=<thread key>`, a JSON-RPC
`tools/call`. The key is the token's HMAC-SHA256 of `mcp\ndemo-main` as defined in
`src/core/auth.bend:Auth.mcp.key`; it is computed in memory and never saved in the
capture sources. `src/server/server.bend` (`Conn.is_mcp`, `Mcp.gate`,
`Hub.mcp_url.seg`, `Hub.term_read`), `src/server/term.bend`, and `docs/terminal.md`
describe the route and terminal actor. Calling these tools does not require an
agent process. Both seed messages remained unchanged and the persisted log had
zero agent turn events, as checked in `agent-audit.json`.

## Reproduce

From the repository checkout, using the supplied paths on this machine:

```sh
S=docs/media/readme/features/source/c-20261101
python3 "$S/runtime.py" start
python3 "$S/runtime.py" exec node "$PWD/$S/drive.mjs" run "$PWD/$S/capture.mjs"
python3 "$S/export.py"
python3 "$S/runtime.py" stop
```

`start` replaces only its own disposable hub home. `stop` signals captured PIDs
only after checking `/proc/PID/cmdline`. All processes were stopped after capture.
Every final assigned PNG was opened and compared with its predecessor.
No README image has a fixed presentation-width attribute in either language.

Earlier `source/` scripts, nonassigned native captures (`btw-draft`, model picker,
unscaled panels), and `links/` are archival 2026-10-19 material. They are not
sources or runtime evidence for these replacements. The cat footer is unchanged.
