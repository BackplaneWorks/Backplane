# Bub parity pass 6 (2026-11-06)

Stacked on `gpui-bub-carry` (pass 5's carry lane). Builds ran on h@backplane (`scripts/gpui/bp-build.sh`, `bp-fetch.sh`); the window was a real GPUI window on a private Xvfb (:197, D-Bus blanked), XTest clicks and keys, a temp-home hub with stand-in agents (`BACKPLANE_AGENTS` read from `/proc/<pid>/environ`), the order server pointed at a dead local address. No tailnet, no order call.

## Builds off the desktop
`bp-slot.sh` links with the system `cc` when the builder has no clang (it sets `CARGO_TARGET_X86_64_UNKNOWN_LINUX_GNU_LINKER=cc` and a RUSTFLAGS, which makes cargo skip `.cargo/config.toml`'s mold flags). The window crate resolves the optional `cef` crate even when it is off, and the builder's offline registry lacks it: the window builds online there. `bp-fetch.sh LANE OUTDIR` builds both binaries (fast profile) and copies them back.

## Done, and tried in the window
| Area | Result |
|---|---|
| Room page | A room row opens its page (`room` action, `roomPage`, `bots::room_page`): header, members, posts, a post box. |
| `room-post` | Typed through `bfield rpost`, Enter or Send posts as the person (`bots.post`); the post shows under "You", the member bots were woken (`[message from you in #Duo]`, with the room's reference card). The window dropped the commands of a `view.action` reply that arrived after the revision moved (the post was lost): a stale reply now still sends its commands. |
| `routine-save` | A bot's Routines tab: New routine, Edit, Cancel, Save (`routine-edit`, `routine-save`, `bfield` oname/ocron/oprompt). "daily 09:30" saved as cron and listed ("daily at 09:30 · Sat 7 Nov 09:30 EST · in 22h 53m"); Edit fills the form (cron line, hint). A one-time time goes as `at`; the hub's reply error for an unreadable time is only logged (not drawn). |
| @mention and #room | In a thread's composer, `@Miso please look #Duo`: the card chips drew, Miso was woken, the message was posted in #Duo with a "[from the thread ...]" header and woke the members. (Already in the hub; only tried.) |
| Ready bar | `fab.look` answers are kept in the session (`present/ready.rs`: `fab.line/intent/buy/shown/ready/again/boards`), `shown.ready` draws above the composer: "Estimate unavailable: ... · Parts: LED board" against the dead order server; × puts it away (`fab-min` writes `fab.min.<project>`), a small "Ready" chip brings it back (`fab-show`). Hidden while the turn runs, in a thread nothing was said in, with Ordering off, in a buy thread. |
| Board picker | `fab.look` and `boards.list` answers fill `fab.boards`; with two or more boards the Board, Schematic and 3D pages show a chip row, the board on show marked; a chip is `view-board` (watches that board; its schematic for Schematic). Tried with two fixture boards: the plot changed. |
| To-do editing | `todos.edit` was missing in the hub: added (`commands/plan.rs`, a `TodosChanged` of `who: "person"`). The dock: check, text (Enter), ↑ ↓ ×, an Add box. Tried: checked a step, added "buy parts", changed a step's text; the log has the `person` events. |

## Not done
- Buy it (`order.buy` is not in the hub; ordering stays off-limits in tests): the bar names the parts and the estimate, nothing starts an order.
- A routine's save error is not shown. A to-do's agent is not told of a person's edit (the fix branch's concise note).
- Devices, QR code, Your machines, Subagent drill-down, palette file mode, provider setup and sign-in, CAD install and update.
- No test for `todos.edit` on its own in `commands/tests.rs` (tried only in the window).
