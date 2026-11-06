# Settings

The web and desktop Settings menu groups the thirteen pages under
Your workspace, Agents, Hardware, and Machine & data. Each category has a
fixed icon slot, its existing localized title, a short description, and a
chevron. The menu uses two columns on wider screens and one on small screens.
Category buttons have explicit accessible names and descriptions. English
and Japanese menus retain the same page IDs, settings controls and actions.
Search still filters the existing title/keyword/row index; Enter opens its
first result. Subpages show their category icon and description above the
existing controls. This change does not alter settings values or defaults.

The occasional second click had a reproducible transport cause: a background
`view.update` or `view.changed` could issue a newer presentation revision while
Settings was opening or a same-target `settings-page` action was in flight.
That retired the navigation response's display authority. ThinHost now defers
those refreshes until the Settings action settles, then refreshes with the
accepted session. Existing upload deferral is preserved. Transport identity,
privacy filtering, selection changes and other action behavior are unchanged.

Focused evidence: `desktop/test/thin-host.test.cjs` includes failing-before,
passing-after cases for both Settings open and submenu navigation raced by a
background refresh. `test/tools/settings_review.mjs` runs the emitted production
`settings_fixture` in an isolated browser: thirteen categories, icons,
search/Enter, Japanese labels and a 390 px layout. It does not write live
settings or claim live hub/device validation.

Antigravity installation and sign-in controls live in Settings → Agents on
web, desktop and phones. There is no separate Antigravity category; searching
for Antigravity, Google or Gemini opens Agents. Existing Antigravity settings
links also open Agents.

## Show thinking

Settings → Agents → Show thinking (`agent.thinking`, off until turned on)
asks agents for their thinking and shows it in the thread, folded under
"Thinking" and its first line. Claude is started with
`--thinking-display summarized` (without it its thinking blocks come with
no text); turning the setting either way restarts a thread's Claude
process at its next turn (the setting is part of `Agent.key`). Codex runs
with `model_reasoning_summary="detailed"` and its `reasoning` items are
kept. Pi's `thinking_end` carries a whole thought. Each finished thought is
an `ActivityLogged` of tone `thinking`, logged only while the setting is on;
turning it off hides the ones already logged (web and phones). Thoughts
arrive whole, after the model finishes thinking, not streamed. The ACP
agents (OpenCode, Grok, Oh My Pi, Antigravity) send thinking as chunks that
are not collected yet, and Muse's reasoning is not read.


## Status and empty rows

Rows without controls are informational: web and desktop draw them as plain
text with a left rule instead of another setting card. On iOS they do not
create an empty boxed section; both phone clients dim their labels. Existing
buttons and fields keep their normal treatment throughout Settings.

Voice explains how an empty dictionary and Learned fixes become populated.
Learned fixes are available now: choosing a suggested correction or using Fix
after dictation adds one, and confirming the same correction twice lets it
apply automatically. Existing fixes retain their forget controls.

Local model setup shows starting, bytes received (and a percentage when the
size is known), verification/unpacking, readiness, and errors with a retry.
Unknown model metadata offers Check availability instead of a zero-size
download. The same state and text drive the web, desktop and both phones.
The web/desktop page also has an accessible progress bar and live status.

Regression checks: `test/settings_status_test`, the Voice states in
`test/tools/settings_review.mjs`, and `test/tools/voice_download_e2e.ts` (an
isolated hub with a deterministic helper, including fragmented progress lines,
status racing a download, errors followed by status, retries and helper exit).
