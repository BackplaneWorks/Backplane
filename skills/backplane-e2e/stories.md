# Backplane user stories (end to end)

What a person does with Backplane, as short stories to drive through the real
UI. Platforms: **W** web page (headless Chromium), **D** desktop app
(Electron, driven over CDP), **A** Android app (emulator), **I** iOS app (only
on a Mac; skip otherwise). Each step is a press, a key or a gesture; time it
(SKILL.md, "Timing"). The hub is `hub.sh`'s: projects `proj` (the fixture)
and `multi` (boards `alpha`, `beta`, plus an archived copy and a build
snapshot), stand-in agents only.

## Getting started

**S01 First run** (W D A)
Open the hub fresh. The guided tour shows; Skip closes it. Settings → Tour →
Show the tour opens it again; Done closes it and it stays closed after a
reload.

**S02 Add a project** (W D)
Sidebar `+` → enter a folder path (a copy of the fixture) → Add. The project
appears in the sidebar with a new thread selected; pressing Board draws its
board.

**S03 Find a project or conversation** (W D A)
Type part of a project name in the sidebar search: only matching projects
stay; clearing it brings all back. The search icon (Ctrl+Shift+F) finds a
word from an earlier reply; clicking a result opens that thread at that
message, highlighted.

## Talking to an agent

**S04 Send and watch the reply** (W D A)
In `proj`'s thread type a message and press Enter (A: the send button). The
message shows at the end at once, the composer empties, the turn shows as
working, the reply streams in and ends with a "Finished hh:mm · Ns" stamp.
The thread stays in the project's list (it does not move under "Settled"):
it settles by itself only with Settings → Threads → "Settle when done" on.

**S05 Stop a turn** (W D A)
Send a message; while the reply streams press Stop. The turn ends as
stopped within 2 s, and the composer takes a new message that then runs.

**S06 Follow-ups while busy** (W D)
Send a message, and while it runs send a second (Send reads "Steer" while a
turn runs). The second is taken at once and shows under the reply streamed
so far; the rest of the reply streams under it, nothing on screen jumps, and
no line of the reply shows twice or goes missing.

**S07 Long history** (W D A)
`hub.sh long` makes an 80-exchange thread. Open it: it opens at the newest
message. Scroll up to the first message ("Question 1") and back down to the
newest: every step moves, nothing jumps, no stall. Scroll halfway up and send
a message: the view goes to the newest page with that message on screen.

**S08 Reading while it streams** (W D)
Send a message; while the reply streams, scroll up a few notches (D: mouse
wheel with smooth scrolling). The view stays where you scrolled, the text
under the pointer does not move. Scroll back to the bottom: it follows the
stream again.

**S09 Thread lifecycle** (W D A)
On a thread: Rename (title updates everywhere), Pin (moves to the top), Settle
(moves under "Settled N"), Unsettle, Settle again and send a message in it
(it comes back to the live list), Snooze 1 h, Fork (a new thread with the
history), Archive, Delete (asks first; the thread goes). Right-click (A: long
press) opens the same menu on a sidebar row.

**S10 Pick the model** (W D A)
Open the model picker under the composer (W D: its chip "claude / default /
auto", or Ctrl+M): three columns AGENT | MODEL | EFFORT. W D: by keys alone,
Tab to MODEL, ↓ to fable, → to max, Enter: it closes and the chip reads
"claude / fable / max"; Esc closes without a change. By mouse, a click on a
row or an effort bar applies at once. The next turn runs with the choice (the
stand-in's args show `--model fable --effort max`).

**S11 Attach a file** (W D)
Attach an image with the paperclip (or drop it on the composer) and send. The
message shows the image; clicking it opens the lightbox; "Copy image" puts it
on the clipboard (D: and a button shows the file in its folder); Esc closes it.

**S12 Token stats** (W D)
Settings → Usage → turn Token stats on. The chip beside Send shows tokens,
cost (when known) and context; clicking it opens the panel with the last
turn's and the thread's totals.

## Seeing the design

**S13 Board** (W D A)
Open the Board view: the board draws within 5 s. Fit fits it; the wheel zooms
at the pointer; drag pans. Layers lists KiCad's layers; turning one off hides
it; clicking a layer's name dims the others (high contrast). The Layers menu
stays inside the window and the viewer's bar stays on one line (the board
does not move).

**S14 Schematic and its sheets** (W D A)
Open the Schematic: the root sheet draws. Click a sheet symbol: that sheet
opens and the trail shows where you are; Up/Back return to the root.

**S15 3D** (W D A)
Open 3D: the board's model draws; drag turns it, the wheel zooms, Fit
recentres.

**S16 Pick a board** (W D A)
In `multi`, open the board picker: it offers `alpha` and `beta` only (not the
archive copy or the build snapshot), the default first. Pick `beta`: Board,
Schematic and 3D all show `beta`.

**S17 Design history** (W D)
Have the agent change the board (send `mode:toolhold` with a stand-in that
edits, or edit the board file by hand while a turn runs). A step tick appears
on the history bar; Play steps through; Compare shows the change marked.

**S18 Files, Diff, Review** (W D A)
Files lists the project folder; opening a board file shows it in the Board
view. Diff on a new thread before its first turn says there are no changes
yet (no error toast); after a turn it shows the change. Review → BOM lists
the parts.

**S19 Viewer layout** (W D)
Drag the viewer's left edge: it resizes and keeps that width after a reload;
a double-click resets it. Close the viewer; open it again from the rail.

## Settings and workspace

**S20 Appearance** (W D A)
Settings → Appearance: switch the theme (applies at once), the text size
(text grows), the language to 日本語 where it is offered (labels change; on
the web it is a preview, opened with `?lang=ja`) and back, and the sidebar
to Active (one list of live threads).

**S21 Folding the sidebars** (W D)
Ctrl+B (Cmd+B), or the fold button in the top bar, folds the sidebar to a
strip and back; the rail's chevron makes the rail icons only. Both stay after
a reload.

**S22 Command palette and shortcuts** (W D)
With a thread open: Ctrl+K opens the palette; typing filters; Enter runs
"New thread". Ctrl+N makes a thread in the open thread's project (with none
open it goes to the uberbot's chat), Ctrl+J opens the floating terminal (it runs `echo hi`), Ctrl+1 jumps
to the first thread, Ctrl+Shift+S settles it. With no field focused (click
the timeline), the bare keys on the rail's keycaps press their buttons: B
Board, S Schematic, 3 3D, V Split (twice: on, off); typing "bsd3" in the
composer only types. Holding Ctrl lights the first nine threads' digit
keycaps. The legend at the sidebar's foot: its "all shortcuts" row opens the
palette.

**S23 API keys** (W D A)
Settings → API keys: add `E2E_KEY` with a value. Only the name is listed,
never the value; Remove deletes it. W D: record the page's websocket frames
(CDP) while typing and saving: no frame from the hub contains the value. A:
type the value fast: the whole value is kept.

**S24 Settings stick** (W D)
Change Threads → notifications and Time zone; reload: both kept.

## Bots and routines

**S25 A bot** (W D)
Bots `+` → make a bot; it appears with its cat. Open it and send a message:
it answers (stand-in). Rooms `+` → make a room, post in it; the post shows.

**S26 A routine** (W D)
Make a routine "in 1 minute: say hello" on a thread. It shows in Routines with
its time in your zone, and fires: the thread gets the message.

## Phone

**S27 Pair a phone** (A)
Open the hub's pairing link (`PAIR` from `hub.sh`) on the phone: it pairs and
lists the projects and threads ("Connected").

**S28 Wrong pairing** (A)
Pair with the link without its token: the banner says "Hub rejected
credentials; check the pairing link", below the status bar; pairing again
with the right link clears it.

**S29 Phone thread** (A)
Open a thread, send a message: it shows at once, the reply streams. Swipe up
through the history and back down; while a reply streams, a swipe up holds.

**S30 Phone viewer** (A)
Open the viewer from a thread: the board draws; pinch zooms; the Sheet menu
picks a schematic sheet and, in `multi`, a board.

## Coming back

**S31 Hub restart** (W D A)
With a draft typed (not sent) and the cursor in its middle, `hub.sh
restart`: the page says Connecting…, then comes back by itself within 10 s
with the draft and the thread intact, and the next key lands at the cursor.

**S32 Reload keeps your place** (W D)
Open a thread, type a draft, open the Schematic. Reload: the same thread,
draft and viewer tab are back.

## Newer features

**S35 Click away, and buttons that toggle** (W D)
With Settings open (the top bar's gear), click a thread: it opens and
Settings is gone; open Settings again and press + by a project: the new
thread opens, Settings gone. Press Board: the viewer shows the board, Board
lit; press Board again: the viewer closes. Schematic, then Board: it switches.
Ctrl+P twice opens then closes Files; Commands then Ctrl+Shift+P closes the
palette.

**S36 A project's intent** (W D A)
No goal card sits above any thread. Right-click a project (A: long press) →
Intent…: it shows the intent (or "No intent yet…"). Edit fills the composer
with `/intent` and the text, Send reads Save; after sending, Intent… shows the
new text. Esc, × and a click beside it close it; from Settings it opens over
the thread, Settings gone.

**S37 Status colours** (W D A)
In `hub.sh trial`: finished threads have green dots, new ones grey. Send a
message: the dot and the top bar's State turn blue "working", then green
"finished". Stop a turn: grey. A thread whose agent fails (pick codex: no
real codex here) turns red. A: the same colours.

**S34 The top bar** (W D)
Open a thread: the bar reads "Backplane / PROJ / title ▾" with a State cell
and no separate thread header. ▾ lists Pin, Settle, Snooze, Archive, Delete,
Fork; a click on the title renames it. Send a message: State reads "working"
with Stop, Stop interrupts; then "finished". The gear opens Settings (the
bar reads "Settings / page") and closes it. The "Jump or run a command" field
(Ctrl+K) opens the palette. At 420 px wide: the list shows the mark and
Projects; a thread shows "‹ title ▾" with State under it; ‹ goes back.

**S38 Buy in the rail** (W D)
Settings → Ordering on (order.server pointed at a local stand-in, never the
real one), in a project with fab files: the rail's foot shows "Buy it" with
its price under a divider. Press it: a "Buy: <project>" thread opens, with
no Buy of its own. The folded rail shows only the cart.

**S39 Settle when done** (W D)
Default: a finished thread stays in its project's list. Settings → Threads →
"Settle when done" on, send a message: when it finishes the thread moves
under "Settled N". Off again; open the settled thread and send: it comes back
and stays after it finishes. ▾ → Settle still settles at once.

## How it feels

**S40 Typing keeps up** (W D)
In `hub.sh trial`'s "Bring-up notes" (36 exchanges), with a reply streaming
in another thread, type 80 characters in the composer at 20 ms a key: every
character shows, in order, none lost; time each key from keydown to the
composer showing it (budget: 50 ms ok, 150 ms slow).

**S41 Switching while one streams** (W D)
Start a reply in one thread, then switch quickly through five threads and
back: each shows its own messages within 1 s, no text from another thread
shows, and the streaming one is at its end again, still streaming.

**S42 Window sizes** (W D)
At 1400x900, 1100x800 and 800x900, with a thread and then the viewer open:
nothing overlaps or is cut off (top bar, rail, composer, model chip,
viewer), and the viewer opens and closes.

**S43 Two windows agree** (W)
The same hub in two tabs: send in one, the other shows the message and the
reply; settle, rename and pin in one, the other follows within 2 s.

**S44 A lived-in workspace** (W D)
`hub.sh trial`: pinned threads first, the snoozed one under "Snoozed",
settled ones folded. Open each long thread ("Route the USB-C connector",
"Alpha: power tree review", "Bring-up notes"): it opens at its newest
message; scroll to its first message and back down: every notch moves,
nothing jumps, no stall.
