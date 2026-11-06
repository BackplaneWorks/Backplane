# Bub parity pass 5 (2026-11-06): what was tried in a real window

Real GPUI window on a private Xvfb (:193), real XTest clicks, typing and drags, a throwaway hub on a temp home with stand-in agents (no `--policy-bin`, no helper child; `BACKPLANE_AGENTS` read from `/proc/<pid>/environ`). Synthetic history. `test/e2e/standin-panels.py` feeds the panels (TodoWrite, an Agent call and background task, tool runs, an image path).

| Area | Result |
|---|---|
| Model menu | Search box, starred models first (kept on this device), provider mark (monogram squares: the hub sends no artwork), visible scroll bar, Esc shuts it, Enter takes the first match. Picking `sonnet` wrote `ModesSet`. |
| `&model` | Popup rows and Tab/Enter complete the word; the word is a chip in the draft and in the sent message (reference card under it too). The thread's model did not change. (Tab worked here: pass 4's miss was typing faster than the hub's round trip.) |
| Cursor, selection, bar | Caret blinks (sampled 8 times: on in 5, off in 3). OS cursor: I-beam over the composer line and thread text, hand over rows and buttons; the empty part of the composer box now is one click target. Bottom bar buttons: all 32 px in the screenshot. |
| Active sidebar view | Thread list chip in Appearance (setting `sidebar.view`); rows by urgency with their project, Settled N folded and faded, search; status dots follow the thread's status. |
| Reorder | Drag a thread on a thread of its shelf, a project on a project (`ThreadOrdered`, `ProjectOrdered` written, order changed). Not in the Active view. |
| Removed project | Keeps its row while its thread is open. |
| Settings | New pages: Writing, CAD, API keys (a key saved through the sealed field reached `secrets/api.env`, listed by name, Forget asks once), fallback hierarchy on Agents (chain edits written), Privacy and Report an issue buttons (client `open-url`, web addresses only). Esc now shuts the dialog (it only asked the hub). |
| Thread panels | To-do 1/4 and Subagents (2 at work) cells in the header, dock under it; lightbox on an image click; tool-call run opens (the click was refused to the missing helper: fixed); palette (bar, Commands rail, Ctrl+Shift+P): finds threads and actions, Enter jumps. |

## Not done, and why
- Usage (the Allowance chip lane), Devices and Your machines and QR code (pairing HTTP routes and tailnet, no presenter rows; QR drawing already exists in the client), provider setup and sign-in, Voice and Accounts (skipped as asked), CAD install and update controls.
- Queue mode: itself has none (Follow.mode is steer).
- Attachments: not driven (the picker and clipboard need a desktop session); `/attach` hub route is still missing per PARITY.
- Provider logos are monograms, not artwork.
- To-do editing/retiring, Subagent step drill-down, the sidebar's "N subthreads" badge.
- Palette has no file mode.

## Carry onto main and the rest (2026-11-06)
Real window on a private Xvfb (i3 with its own ipc-socket, lavapipe), XTest clicks/keys, a temp-home hub with stand-in agents (no `--policy-bin`, no helper): the model menu (search, ★ favourites first, Enter), `&model` chip in draft and sent message, To-do dock, tool-call fold, Account chip menu, Send, Active sidebar view (Settings > Appearance), thread drag reorder (`ThreadOrdered`), the Settings pages (Accounts, Appearance, Writing, CAD, API keys), the palette (Ctrl+Shift+P, Enter jumps).
- Attachments: `/attach` exists (files/attach_http.rs) and answers; PARITY said otherwise. An image set on the X clipboard and pasted (Ctrl+V) shows a chip with its picture, uploads to `<home>/attachments/<thread>/`, and Send puts it in the thread. The paperclip opens the system dialog through the desktop portal (it opened a real "Open File" on the person's display when the rig shared its D-Bus: the rig now blanks `DBUS_SESSION_BUS_ADDRESS`). `BACKPLANE_GPUI_PICK_FILES=<path>:<path>` stands in for the dialog in tests; a picked text file uploaded, showed a chip and was sent. "presenter unavailable" on paste could not be reproduced on this tree.
- Sidebar `+` for a bot or a room: ported (`present/botforms.rs`): `bot-new`, `room-new`, `bfield` (name, persona, rname, provider), `room-pick`, `bot-create` (`bots.create`), `room-save` (`bots.room`), `form-close`. Created a bot (provider codex was kept) and a room with it. Not ported: a room's page and `room-post`, `routine-save`, the Ready bar and the board picker.
