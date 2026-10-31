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
Open the hub fresh. D: started from the desktop app with no hub running, its
hub joins the tailnet (Settings → Network: "Tailscale: on"). The guided tour shows; Skip closes it. Settings → Tour →
Show the tour opens it again; Done closes it and it stays closed after a
reload. The KiCad notice waits until the tour is over (none under the card).

**S02 Add a project** (W D)
Sidebar `+` → enter a folder path (a copy of the fixture): the highlighted
row is "Add … as a project", so Enter adds it. The project appears in the
sidebar with a new thread selected; pressing Board draws its board, its
corner reading "… KB · loaded in N s" in the page's monospace.

**S03 Find a project or conversation** (W D A)
Type part of a project name in the sidebar search: only matching projects
stay; clearing it brings all back. The search icon (Ctrl+Shift+F) finds a
word from an earlier reply in any thread (the open one's results first);
clicking a result opens that thread at that message, highlighted.
Then go to another thread and open a second result in the same thread as the
first: it lands on the new message and stays there (known bug, found
2026-10-28 and present since the search work: the view jumps to the new
message, then snaps back within ~100 ms to where the first jump left it;
without an earlier jump into that thread it lands and stays). A result in a
subagent's steps is listed under its thread and opens that thread.

## Talking to an agent

**S04 Send and watch the reply** (W D A)
In `proj`'s thread type a message and press Enter (A: the send button). The
message shows at the end at once, the composer empties, the turn shows as
working, the reply streams in and ends with a "Finished hh:mm · Ns" stamp.
The thread stays in the project's list (it does not move under "Settled"
until it has sat idle 30 minutes, or at once with "Settle when done" on: S39).
No "design change" row or Changes cell when the agent changed no file.
Click another thread's row and type at once: the text lands in its composer.

**S05 Stop a turn** (W D A)
Send a message; while the reply streams press Stop. The streamed text never
blanks; the turn ends as stopped within 2 s ("Stopped hh:mm · Ns", the top
bar's State grey "stopped"), and the composer takes a new message that then
runs. After it finishes, the stopped reply still reads "Stopped".

**S06 Follow-ups while busy** (W D)
Send a message, and while it runs send a second (Send reads "Steer" while a
turn runs; Send, Steer and Stop keep one width, so draw, attach and mic
never move). The second is taken at once and shows under the reply streamed
so far; the rest of the reply streams under it, nothing on screen jumps, and
no line of the reply shows twice or goes missing.

**S07 Long history** (W D A)
`hub.sh long` makes an 80-exchange thread. Open it: it opens at the newest
message. Scroll up to the first message ("Question 1") and back down to the
newest: every step moves, nothing jumps, no stall (D: also fast bursts of
notches near the top: 0 jumps). Scroll halfway up and send
a message: the view goes to the newest page with that message on screen.

**S08 Reading while it streams** (W D)
Send a message; while the reply streams, scroll up a few notches (D: mouse
wheel with smooth scrolling). The view stays where you scrolled, the text
under the pointer does not move, and a "New ↓" pill shows at the
timeline's foot. Click it (or scroll back to the bottom): it follows the
stream again and the pill goes. Scrolled up while idle, a reply that
finishes makes it "1 new ↓".

**S09 Thread lifecycle** (W D A)
On a thread: Rename (title updates everywhere), Pin (moves to the top), Settle
(moves under "Settled N"), Unsettle, Settle again and send a message in it
(it comes back to the live list), Snooze 1 h, Fork (a new thread with its inherited
context and a visible link to the source conversation), Archive, Delete (asks first; the thread goes). Right-click (A: long
press) opens the same menu on a sidebar row, Fork included; Esc closes it.
A pinned row shows a small pin. Snooze, then Archive: it wakes (▾ lights only
Archived, no countdown).

**S10 Pick the model** (W D A)
Open the model picker under the composer (W D: its chip "claude / default /
auto", or Ctrl+M): three columns AGENT | MODEL | EFFORT. W D: by keys alone,
the headers' keycaps read AGENT ⇧Tab | MODEL ↑↓ | EFFORT ←→ and move with
Tab. Tab to MODEL, ↓ to fable, → to max, Enter: it closes and the chip reads
"claude / fable / max"; Esc closes without a change. By mouse, a click on a
row or an effort bar applies at once. The next turn runs with the choice (the
stand-in's args show `--model fable --effort max`).

**S11 Attach a file** (W D)
Attach an image with the paperclip (or drop it on the composer) and send. The
message shows the image; clicking it opens the lightbox; "Copy image" puts it
on the clipboard (D: and a button shows the file in its folder); Esc closes it.

**S12 Token stats** (W D)
Token stats are on by default (Settings → Usage shows On; Off hides the chip
and On brings it back). The chip beside Send shows tokens,
cost (when known) and context, rounded to the nearest (5,950 reads "6k");
clicking it opens the panel with the last turn's and the thread's totals,
each model a line with its cost and its tokens under it; Esc closes it.

**S51 Tool calls in one drawer** (W D A)
Send "tools:5 drawer" (the stand-in runs five Bash calls, then streams its
reply; `BP_E2E_STREAM="60 300"` slows it down). While it runs, the calls are
one shut row under the message, "› 5 tool calls · running: Bash" with a
pulsing dot; no call's own row shows. When the turn ends the row reads
"5 tool calls · Bash", still shut, and the reply and its "Finished" stamp
show under it with nothing below them. Click the row: the calls hang under
it on a rule; click it again: they go, and none is left below the reply
(the hub-drawn page too). Open it, send "tools:3 again", switch to another
thread and back: the first run is still open, the new one shut.

**S52 The pairing link in Settings** (W D)
Start the hub with `BP_E2E_TAILNET=1` (hub.log: "Backplane on your tailnet:
http://127.0.0.2:PORT/"). Open the page with its token and go to Settings ›
Network and updates: "Pairing link" reads `http://127.0.0.2:PORT/#token=…`
with QR code and Copy beside it, never "Shown once Backplane listens on your
tailnet."; QR code shows the code. Reload, and open it from a second window:
the same. Without `BP_E2E_TAILNET` the row says it is shown once Backplane
listens on the tailnet, and a page opened without the token is refused
("Pairing is required to reconnect").

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
Open 3D: the board's model draws, lit from above on a soft spot of light
(the board stands out from the background); drag turns it, the wheel
zooms, Fit recentres. Opened again after a reload it shows at once (kept
on the hub). A model file over 12 MB says it is too big to show, and the
hub's memory stays low.

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
a double-click resets it; it stops where the thread would get under 360 px.
Tab to the divider: Left/Right resize it (Shift takes larger steps), Home
resets it, and the changed width remains after a reload.
Close the viewer; open it again from the rail.

**S47 The board picker reads the folder each time** (W D A)
In `proj` (two boards), open the board picker on Board: it offers both. Shut
it, copy a board and its project file into `proj/gamma/` on disk, open it
again: `gamma` is listed within a second, the picker stays open and the
board on show is the same. Open it once more with nothing changed: no row
moves or flickers. Delete `gamma/`, open it again: it is gone. On a phone,
shut the viewer and open it on Board: the Sheet menu lists the boards there
are then.

**S48 Mech re-checks its parts** (W D A)
Put a STEP in `proj/mech/`, open Mech: it is listed and shown. Add a second,
open Mech again (from Board, or after shutting the viewer): both are
listed. Delete the part on show, open Mech again: it is gone, and another
part shows in its place (never the deleted one); delete them all: Mech says
there are none. With Mech open, choose another thread, add a part, come
back: it is listed. The same on a phone's Mech.

**S53 The 3D workbench** (W D)
Open 3D in `proj`: over the model a toolbar reads Fit Iso Top Bottom Front
Right Back Left, PCB, Camera, Reflow, Components, a Rendering choice (Fast,
Studio, Path trace) and Frame part. The quick model draws first; the corner
note goes "Preparing the original saved model…" to "Studio · original
model" (within 5 s once the board was exported; a first export takes up
to ~30 s). On a software renderer Path trace is greyed, the note says why;
on a GPU it refines ("Refining · n / 64 samples", then "Path traced").
Top, Iso… turn the view; Fast drops the original model (no note), Studio
brings it back. PCB shows the saved PCB beside the model, with its Layers;
press it again: gone. Camera opens Projection, Focal length, Exposure,
Depth of field, Controls, Materials (CAD studio; Source materials changes
its note to "Original CAD export materials…") and "Illustrative solder"
(on: "N estimated joints on M supported parts…", or why it can't). At
1400x900 the viewer is under 950 px: model, PCB and inspector stack in one
column, model and PCB 160 px tall, and only the inspector scrolls. At
1920x1080 with the sidebar folded, drag the viewer's edge past 950 px: they
sit side by side, the inspector on the right.

**S54 Inspect a part** (W D)
In 3D press Components: the inspector reads "Components · saved CAD",
Refresh, "Saved revisions match…", a "Find reference, value or MPN" search
and a Ref | Value | MPN table; typing filters it. Click U1: its page shows
U1 with Clear, Value, MPN, Footprint, Side, Position, UUID, the saved
footprint drawn with numbered pads, the source symbol (or why not), a Pad |
Net | Shape table, its 3D source and "Saved source: <board>". Frame part
frames it (or says it has no identified source model). Shut Components,
turn PCB on and click a pad on the PCB: Components opens on that part,
its pads lit and the probe chip naming the pad's net. Clear deselects. In
`multi`, pick `beta`: the model's name and "Saved source" read beta's.

**S55 Reflow preview** (W D)
In 3D press Reflow: the inspector shows "Reflow preview" tagged
Illustrative (Components shuts): Solder alloy SAC305 with its composition
and solidus/liquidus, Furnace, Board mean and Liquid fraction, two graphs,
↺ Play 1×/10×/30×/60× and "0.0 / 330 s", and "Solid · illustrative".
Play runs the clock and readings ("Partially molten", "Liquid", then
"Solidified · illustrative"); Pause holds; the slider scrubs. "Illustrative
heat overlay" tints the model. Edit profile points: Add point, Apply (a bad
profile shows its error), Reset; Export JSON saves reflow-preview.json and
Import JSON reads it back. Pick a part (S54), open Reflow, then Esc and
open 3D again, or switch threads and back: Reflow is still the open pane
(never swapped for Components), and Components still has the part.

## Settings and workspace

**S20 Appearance** (W D A)
Settings → Appearance: switch the theme (applies at once), the text size
(text grows; at 130% the top bar still reads "Jump or run a command" in
full), the language to 日本語 where it is offered (labels change; on
the web it is a preview, opened with `?lang=ja`) and back, and the sidebar
to Active (one list of live threads).

**S21 Folding the sidebars** (W D)
Ctrl+B (Cmd+B), or the "‹" beside "Search projects" (the top bar has no fold
button), folds the sidebar to a
44 px strip of project icons (the open one outlined; a click opens that
project; each project shows its icon or initials) and its "›" brings it back; the rail's chevron makes the rail icons only. Both stay after
a reload.

**S22 Command palette and shortcuts** (W D)
With a thread open: Ctrl+K opens the palette; typing filters, best match
first ("new thread" → "New thread · Ctrl+N" on top); Enter runs "New
thread". It lists Pin or Unpin (Archive or Unarchive...) by the thread's
state, never both. With the cursor in a draft in the composer, Ctrl+K,
Ctrl+1 and Ctrl+Shift+S work and the draft stays. Ctrl+N makes a thread in the open thread's project (with none
open it goes to the uberbot's chat), Ctrl+J opens the floating terminal (it runs `echo hi`), Ctrl+1 jumps
to the first thread, Ctrl+Shift+S settles it. With no field focused (click
the timeline), the bare keys on the rail's keycaps press their buttons: B
Board, S Schematic, 3 3D, V Split (twice: on, off); typing "bsd3" in the
composer only types. Keycaps are hidden until Ctrl is held: holding Ctrl
shows the rail's keycaps, the top bar's Ctrl K, the model chip's Ctrl M,
the first nine threads' digits and the legend, and nothing moves when they
appear or go. The legend at the sidebar's foot (shown while Ctrl is held):
Ctrl+click on its "all shortcuts" row opens the palette. Open the top bar's
▾, press Esc and type: the text goes into the composer.

**S23 API keys** (W D A)
Settings → API keys: add `E2E_KEY` with a value. Only the name is listed,
never the value, and after Save the list shows the new name; Delete asks
("Click again to delete") and the second click deletes it. W D: record the page's websocket frames
(CDP) while typing and saving: no frame from the hub contains the value. A:
type the value fast: the whole value is kept.

**S24 Settings stick** (W D)
At 1920x1080 Settings fills the width: its groups flow into up to four
columns, the search spans them; a page (Appearance, API keys...) has the
category list on its left and each row's controls beside its label; at 420
px it is one column. Change Threads → notifications and Time zone; reload:
both kept. On a fresh
hub the Time zone page has no empty row, and Automatic shows chosen, its
tooltip naming what it follows (this machine).

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
restart`: the thread stays on screen whole (no "Loading…" page, rail and
composer never vanish) and comes back by itself within 10 s with the draft
and the thread intact, and the next key lands at the cursor. A longer outage
(the hub stopped ~4 s) says "Hub unreachable. Reconnecting…" within ~1.2 s.

**S32 Reload keeps your place** (W D)
Open a thread, type a draft, open the Schematic. Reload: only the page's
frame shows (no half-filled list, no "/ default" chip, no "Connecting…"
bar), the composer doesn't move, then the same thread, draft and viewer tab
are back.

## Newer features

**S35 Click away, and buttons that toggle** (W D)
With Settings open (the top bar's gear), click a thread: it opens and
Settings is gone; open Settings again and press + by a project: the new
thread opens, Settings gone. A press on the gear right after opening a
thread works the first time. With Board open, the gear (or the palette's
Settings) shows Settings over the whole area and closes the viewer; clicking
the thread shows it without the viewer. Press Board: the viewer shows the board, Board
lit; press Board again: the viewer closes. Schematic, then Board: it switches.
Ctrl+P twice opens then closes Files; Commands then Ctrl+Shift+P closes the
palette.

**S36 A project's intent** (W D A)
No goal card sits above any thread. Right-click a project (A: long press) →
Intent…: it shows the intent (or "No intent yet…"). Edit fills the composer
with `/intent` and the text, Send reads Save; Enter makes a new line, Save
or Ctrl+Enter saves and "Intent saved" shows; then Intent… shows the new
text, with no empty headings. Esc, × and a click beside it close it; from Settings it opens over
the thread, Settings gone.

**S37 Status colours** (W D A)
In `hub.sh trial`: finished threads have green dots, new ones grey. Send a
message: the dot and the top bar's State turn blue "working", then green
"finished". Stop a turn: grey "stopped". A thread whose agent fails (pick codex: no
real codex here) turns red, State "failed" with the reason in its tooltip
(not "needs you"), and its error row shows the whole reason. Hovering its
red dot says failed as well, in both the top bar and the sidebar. A thread whose
turn ended while work it started is out (a delegated task or a launched
thread still going, a background agent or watched work; the stand-in's
`work_run` will do) is violet "waiting" (State "waiting", tooltip "Waiting:
on subagents or work it started"), never green "finished", until the last of
it is done; then green "finished". A: the same colours (waiting: a violet
spinner).

**S33 Split and cross-probing** (W D)
In `proj` press Split (rail, key V): schematic, board and 3D draw side by
side. × on 3D hides it, the bar's 3D brings it back; a reload keeps the
choice. Sheets → Power opens Power in the pane, and the Schematic tab shows
Power too. Click J1 on the board pane: J1 lights on the board, the schematic
pane opens Power with J1 lit, 3D lights J1's spot, the bar shows "J1 ×".
Click a GND wire in the schematic: the chip reads "Net GND", the board lights
GND (pads, vias, pours) and dims the rest; Esc clears. Probe J1 on the Board
tab, close the viewer, open Schematic: "J1 is on the sheet Power · Open it"
opens it with J1 lit; after a reload the chip is still there. In `multi`,
Split, pick `beta`: every pane shows beta, Sheets lists beta's sheets (no
"Not in this project's hierarchy"). Pan and zoom stay smooth with GND lit.

**S34 The top bar** (W D)
Open a thread: the bar reads "Backplane / PROJ / title ▾" with a State cell
and no separate thread header. ▾ lists Pin, Settle, Snooze, Archive, Delete,
Fork; a click on the title renames it. Send a message: State reads "working"
with Stop, Stop interrupts; then "finished". Opening a finished thread
from an idle one never shows "idle" first. The gear opens Settings (the
bar reads "Settings / page") and closes it. The "Jump or run a command" field
(Ctrl+K) opens the palette. At 420 px wide: the list shows the mark and
Projects; a thread shows "‹ title ▾" with State under it; ‹ goes back.

**S50 Edit and retire the agent's to-do list** (W D A I)
Start a stand-in turn with 7 steps, 3 done and a subagent: "TO-DO 3/7"
starts shut beside Subagents (phones: beside the subagents' button), with
no steps between the timeline and composer. Open it: all 7 editable rows
appear, with checkboxes, text, up/down, delete and Add. Check and uncheck,
change text, add an item, delete one and move items both ways: each change
saves. Switching threads and back keeps the list's open/shut choice. Repeat
the identical agent snapshot: human edits, additions, deletions and order
survive; a later agent update changes its intended field and adds its new
item without losing the other edits. Running edits reach that agent as
"The person updated your to-do list:" notes; idle edits queue a note without
starting a turn, and the next turn receives it. Reload and restart the hub:
the list and merge state survive. Another thread and another bot keep their
own lists, even when item IDs match. An unfinished list remains after its
turn; start the next turn without to-dos and it disappears, and the
agent's next list in that thread starts shut with only its own steps (no
earlier human edits). Complete every
item during a turn: the open list collapses to "done", then clears when the
turn ends. At 420 px the cell sits in the second row; open rows and every
editing control fit and work without horizontal overflow. Time each action
and capture evidence on web and packaged Electron; run phones on their
device/simulator (a narrow web viewport is not a phone-app result).

**S38 Buy in the rail** (W D)
The test hub starts with Ordering off (hub.sh sets it, though a person's
default is on) and the order server a dead local address. Point
order.server at a local stand-in (never the real one), then Settings →
Ordering on, in a project with fab files: the rail's foot shows "Buy it" with
its price under a divider. Double-click it: one "Buy: <project>" thread opens, with
no Buy of its own. The folded rail shows only the cart. Return 503 from the
local estimate service: the part list and unpriced Buy stay, with an explanation
and Retry; restore the service and Retry to recover the price. An empty fab
object names the invalid file and what is missing instead of hiding Buy.
Malformed fab JSON also names the bad file rather than calling the board ready.
Dismiss the Ready bar, then Show parts in the rail restores it, including after
reload.

**S49 Every board kind priced** (W D)
Like S38, against a local order server (never the real one) with Ordering
on: give the project fab files of other board kinds beside a rigid one, e.g.
`{"kind": "flex", "options": {"layers": 2, "stiffener": "polyimide_0.2"}}`
and `{"kind": "aluminum"}` with their measured features. The Ready to order
bar names every part, wrapping so every name is readable at 760 px, and
Buy it labels the price as the last estimate for all of them; the
server's estimate prices each with no problems. Give one part invalid options,
or include more parts than the server returns: its missing estimate is named
and the partial sum is withheld. Edit a fab file's dimensions while the project
stays open: Refresh estimate reads the current files and updates both prices;
repeat after restoring the dimensions. A flex board asked to be
assembled is refused (only rigid `pcb` boards are). A quote upload approval
with one file says "Send 1 file"; several files use "files".

**S39 Settle when done** (W D)
Default: a finished thread stays in its project's list, then settles by
itself (under "Settled N") once idle 30 minutes. There is no Settings row
to edit that time: General inactivity explains the separate shorter idle
window using its current value. Set it short with `rpc.mjs PORT TOKEN setting.set
'{"key":"settle.idle.minutes","value":"1"}'`, send a message: the thread
settles within ~2 min of its "Finished" stamp (the hub's tick); a pinned or
snoozed one, or one with work out, stays; a message brings it back. With
`"0"` it never settles by itself. Settings → Threads → "Settle when done"
on, send a message: when it finishes the thread moves under "Settled N" at
once; while it's the open thread its row stays in sight and its project
keeps its place, not faded. Off again; open the settled thread and send: it
comes back and stays after it finishes. ▾ → Settle still settles at once.

## How it feels

**S40 Typing keeps up** (W D)
In `hub.sh trial`'s "Bring-up notes" (36 exchanges), with a reply streaming
in another thread, type 80 characters in the composer at 20 ms a key: every
character shows, in order, none lost; time each key from keydown to the
composer showing it (budget: 50 ms ok, 150 ms slow).

**S41 Switching while one streams** (W D)
Start a reply in one thread, then switch quickly through five threads and
back: each shows its own messages within 1 s, no text from another thread
shows, and the streaming one is at its end again, still streaming. No frame
shows "Loading conversation…", an empty breadcrumb or a missing rail or
composer; a thread seen before comes back at once, one never seen keeps the
last view up (taking no clicks) until its own arrives.

**S42 Window sizes** (W D)
At 1400x900, 1100x800 and 800x900, with a thread and then the viewer open:
nothing overlaps or is cut off (top bar, rail, composer, model chip,
viewer), and the viewer opens and closes. Where the window can't hold the
sidebar, a 360 px thread and a 480 px viewer, the sidebar folds to its strip
while the viewer is open (the sidebar button still brings it back) and the
composer's foot rearranges: the context under the chip, then the chip on a
line of its own; Send and the mic stay clear of the history bar.

**S43 Two windows agree** (W)
The same hub in two tabs: send in one, the other shows the message and the
reply; settle, rename and pin in one, the other follows within 2 s.

**S44 A lived-in workspace** (W D)
`hub.sh trial`: pinned threads first, the snoozed one under "Snoozed",
settled ones folded. Open each long thread ("Route the USB-C connector",
"Alpha: power tree review", "Bring-up notes"): it opens at its newest
message; scroll to its first message and back down: every notch moves,
nothing jumps, no stall.

**S45 The keyboard stays in the composer** (W D)
Open a thread from the sidebar and type at once: the text lands in its
composer; reload: the keyboard is in the composer with the draft back. The
same after Ctrl+1, Ctrl+N, closing the palette, Settings or a menu (the caret
where it was). Open Board, press Esc: the viewer closes (under 300 ms) and
typing goes to the composer; the same for Schematic, 3D, Split, Mech, Files,
Diff. In Split with a probe, the first Esc clears the chip, the second
closes the viewer. Open the terminal (`): Esc stays in the shell, Shift+Esc
hides it and the composer has the keyboard. Click in the timeline, press
"/": the keyboard is at the end of the draft and no "/" was typed. A rename
field or a Settings field keeps the keyboard while you type in it.

**S46 Every thread has KiStack's skills** (W D)
Type `$` in a thread's composer: KiStack's skills are offered (kicad-pcb,
kicad-schematic, layout-placement, …) with their descriptions, a folded
YAML one (`description: >`) as its text, never ">". On D the hub is the
desktop app's own (`desktop/runtime/resources/backplane-service`, KiStack in
the install's `kistack/` beside `desktop/`): the same list. A Claude thread's
agent runs with `--plugin-dir <install>/kistack` (the stand-in's arguments);
with `BACKPLANE_KISTACK_SOURCE` (hub.sh passes it) the hub syncs that copy
into `HOME_DIR/kistack` after it starts ("kistack sync:" in hub.log), and the
`$` list and a Claude thread started after that use it
(`--plugin-dir HOME_DIR/kistack`);
a delegated child and a launched thread offer the same `$` list as their
parent.
