# Actual feature captures

Captured from `/home/h/.local/share/backplane/backplane` on a private Xvfb display using a fresh temporary `BACKPLANE_HOME`, `HOME`, and XDG directories. No live Backplane data was copied or modified. No agent turns or chat messages were sent, and no external API was called. The only tool calls went to the isolated app's loopback MCP endpoint: `terminal_open`, `terminal_send`, and `terminal_read` for a local read-only vi session. The capture app and Xvfb processes are terminated by captured PID at the end of the script.

`source/capture.json` records the binary SHA-256, installation commit marker, viewport, and temporary home for the final run. The installation's COMMIT file is provenance metadata, not an independently verified build attestation.

`source/demo-events.jsonl` is the entire synthetic event fixture. Its names and conversation are public-safe demonstration text, not copied user data. The seeded assistant text is fixture content, not model output. Project roots point into the disposable capture home. Bot personas have no hooks, schedules, or pending messages.

## Useful assets

- `reference-thread.png`, `reference-room.png`, `reference-project.png`, `reference-bot.png`: actual composer menus, with the typed sigil visible.
- `btw-autocomplete.png`: `/btw` command and its actual description, “Ask a side question; the thread never sees it”.
- `btw-draft.png`: an unsent side-question draft. No answer card was captured because submitting the question would call a model.
- `inline-terminal.png`: actual inline PTY running installed vi against a local read-only checklist. The terminal response is preserved in `source/terminal_read.json`.
- `model-picker.png`: installed provider picker, retained as evidence of the available providers.
- `link-cards.png` and `link-cards-2x.png`: actual rendered thread, bot, and project chips from the separately authorized existing candidate binary. Appearance only; activation is not verified. The clean original is `links/raw/link-cards.png`.

Every unscaled PNG is a lossless crop of a 1600 × 1000 native frame in `raw/`, except the successful `link-cards.png`, whose source is `links/raw/link-cards.png`. `*-2x.png` is a nearest-neighbor 200% enlargement for composition, not a higher-resolution UI rerender. `source/export-panels.py` and `source/panels.json` preserve the exact crop and scale. No UI pixels are invented or redrawn.

## Reference mapping and runtime limits

The exact mapping is `>` → thread, `#` → group chat/room, `%` → project, `@` → bot. `^` is not recognized. The installed composer placeholder and menus demonstrate this; `raw/reference-caret.png` shows the unsupported caret without a menu.

Source evidence: `src/core/refs.bend:125–141`, `Refs.kind.c` and `Refs.sigil`, at both installation marker commit `e6f8c9a48194d79cf71a9304fb12c57b82bff58b` and newer release-prep commit `ac2b41fd275cca1c2e607ee04b8bb606b0b78b5b`. Exact excerpts are saved as `source/reference-mapping.bend.txt` and `source/reference-mapping-newer.bend.txt`. These define only character codes 64 (`@`), 62 (`>`), 37 (`%`), and 35 (`#`), with the empty string for all others. The current root/docs worktree at old HEAD does not include this newer refs implementation.

Clickable thread/bot/project link cards could not be truthfully captured with the installed executable: seeded `[label](backplane://...)` text renders as plain labels. `raw/link-cards.png` is an unsuccessful capability probe, **not a feature asset**. The installation marker's `src/core/links.bend` contains only `PText` and `PLink`, and accepts HTTP(S). Rich internal references are present in newer source, including `src/core/ref.bend`, `refview.bend`, and `links.bend` in the release-prep worktree, but this task did not build or replace the runtime.

### Existing candidate binary for rendered link chips

The parent authorized the existing `/home/h/Documents/i2cjak/bp-coord/links/run3/backplane` binary for a separate isolated capture. This uses the same clean public demo fixture and fresh temporary home, never the existing screenshots' copied personal bot/project context or stress-fixture labels. Original screenshots under `links/shots-3/` and `links/integration-3c/shots-636/` were inspected only and are not included here.

Candidate frames and their manifest are under `links/`, separate from installed-runtime captures. `links/source/capture.json` records the actual executable path and SHA-256; no unverified source revision is assigned to it. This is an existing development candidate, not the installed app and not evidence of a released feature. No binary was built, installed, deployed, or modified.

The candidate SHA-256 is `5470b8a4e89be01689627c1e101207ab59a00b4404d9fa69911a763688ae7f12`. Final capture used `/tmp/bp-readme-features-o7r2vl_b`, private display `:100`; the app and Xvfb exited through the script's cleanup. The finished crop contains only the fixture's “Layout review”, “Miso”, and “Sensor Board” chips and their actual icon/status labels. Export by rerunning `source/export-panels.py` after the candidate capture.

These images demonstrate **rendered internal link chips only**. They do not verify click activation, navigation, or the pending activation repair. Read-only coordination notes `/home/h/Documents/i2cjak/bp-coord/links/STATUS.md` identify PR259 as another owner's activation work. No part of that implementation was touched. Publication/release dependency remains for the parent to resolve.

OpenCode is absent from the installed provider picker: it shows claude, codex, grok, muse, and antigravity. The newer release-prep worktree `/home/h/Documents/i2cjak/bp-coord/release-prep/worktree` at `ac2b41fd2` includes OpenCode; `src/core/client.bend:4052–4053` lists `@opencode` and later provider selection code includes `opencode`. An exact excerpt is in `source/opencode-newer.bend.txt`. This newer source must not be represented by the older installed UI screenshot.

## Reproduce

Use the existing Python with Pillow, without installing dependencies:

```sh
python3 docs/media/readme/features/source/capture-features.py
python3 docs/media/readme/features/source/export-panels.py
```

The separate existing-candidate capture uses:

```sh
README_CAPTURE_APP=/home/h/Documents/i2cjak/bp-coord/links/run3/backplane \
README_CAPTURE_OUT="$PWD/docs/media/readme/features/links" \
README_CAPTURE_LINKS_ONLY=1 \
python3 docs/media/readme/features/source/capture-features.py
```

The capture uses the existing Xvfb and xpoke paths named at the top of the script, reserves a private display in 100–109, starts no builds, and does not change README or app files. Snapshots are actual native Backplane frames exported by `BACKPLANE_SNAP`.
