# Bots

Persistent agents with a name, a face (an animated kitty cat), memory,
routines, webhooks, a browser page of their own, and a space: a UI they
write for themselves. Bots talk to each other directly or in rooms, on one
machine or across machines, including other people's Backplanes.

## Shape

- A bot is backed by one thread (`Bot.thread`). Its turns, model, provider,
  approvals, tools and timeline are the thread's. The thread belongs to no
  project (project `""`), so it never shows on a project shelf; it works in
  `<home>/bots/<id>/`, its home folder (set by a `WorktreeSet` once the hub
  has made the folder).
- Everything else lives in `M.State.bots : BT.World` (`src/core/bot.bend`),
  folded from additive `Change` variants (`Bot*`, `Room*`, `Hook*`,
  `Routine*`, `Memory*`, `Space*`, `Peer*`).
- Secrets never enter the event log or reach a client: webhook and peer
  secrets are files under `<home>/secrets/` (0700 dir, 0600 files); the
  Google refresh token too.

## Projects and sub-bots

A bot works for the person across all their projects. Its thread belongs
to none, so the thread tools reach every project for it: `thread_list`
lists every project's threads, `thread_read`, `thread_send`, `thread_wait`
and `thread_interrupt` take any of them, and `thread_launch` needs a
`project` (a name or id from `project_list`). The new thread shows in that
project like the person's own, its first message naming the bot.

New work gets a visible thread. A bot starts it with `thread_launch` and
its `project`, and `delegate_task` from a bot needs a `project` too (law
`bot_delegate_needs_project`): the child runs in that project's folder,
its first message names the bot, and unlike a thread's delegated child it
is not hidden under its parent (`Kids.set` in client.bend skips children
of bot threads, law `kids_bot_child_shown`), so the person sees it in that
project's sidebar. Its result still comes back to the bot as a message,
and the delegation depth limit applies as before.

`project_create(path, name?)` adds a project for the person, but only
once they say yes: every call opens an approval card ("New project ...,
Folder: ...") unless they allowed it for the session (law
`bot_newproj_asks`; a single yes never covers the next call,
`bot_newproj_once`). On accept the server makes the folder when missing
(`~/x`, or a bare name under the home folder), resolves it, and the hub
adds it (`Bots.newproj.made`); a folder that is a project already answers
that project. Declining makes nothing.

`bot_create` makes a sub-bot for a lasting role. Its maker is kept as the
setting `bot.parent.<id>`. Sub-bots go at most two levels below a bot a
person made (law `bot_spawn_bounded`), and a bot keeps at most eight.

## Launched threads report back

A bot (or any thread) that starts work with `thread_launch` hears back by
itself, the way `delegate_task` results arrive: each time the launched
thread's turn ends it gets one message with the thread's title and id, its
state (completed, waits for an answer, failed, interrupted) and its last
reply cut to about 1.5 KB (`thread_read` for the rest), and an ask the thread
opens for the person is passed on at once. The message wakes the launcher,
or waits in its queue behind a running turn (the queue is in the log, so
order survives a restart). `thread_launch {report: false}` launches quietly,
and `thread_update {threadId, report: false|true}` changes it later. A
thread that already wrote to its launcher in the turn, or one its launcher
stopped itself, is not echoed back; twenty turns in a row started by the
launcher with no word from the person end the reports with a note saying
so. The uberbot hears on the machine that holds its conversation; a
launcher on another machine is sent its reports in order, again each
minute until that machine takes them. A bot turn woken only by a report
sends the person no push: the launched thread alerted for itself.

## Conversation depth

A message a bot sends carries a hop count: a human, a routine, a webhook or
a space button starts at 0, and a bot's message to another bot is one more
than the hop of the turn that sent it (`Hop.send` in `src/core/bot.bend`).

A turn's hop is the lowest hop of the messages it answers (`Hops`,
`Hop.woke`, `Hop.taken`). A message that reaches a bot at rest starts a
turn at its hop. One that reaches a busy bot (its thread running, so the
hub queues the message) leaves the running turn's hop alone and waits;
the next turn (`QueueTaken`) runs at the lowest hop waiting. So when a
post at hop h asks five bots to answer once, each answer goes out at h+1
even though every answer wakes the others (law `bot_hop_fanout`), while a
back-and-forth between bots goes one hop deeper per message
(`bot_hop_queue_takes`) and stops at `Hop.max()`, 6 (`bot_hop_bounded`,
`bot_loop_bounded`, `bot_loop_stops`). A person, a routine or a webhook
starts a new exchange at hop 0.

A message past the bound is held, not dropped: it is posted where it was
going with a `[held: ...]` line, for the person to read, and heard by no
bot (`Bots.hop.gate`, law `bot_held_wakes_none`). In a shared room it goes
to the members' machines too, which log it and wake no one there. The
sending bot gets an answer saying so (`held: true` and a note telling it
not to send it again or anywhere else), not an error. A hop from a linked
machine is capped at the held hop (`Bots.hop.cap`). `test/depth_test.bend`
runs #staff: five bots here and one on a linked machine answering one post.

## Cats

`src/core/cat.bend`. A look (fur, pattern, eyes) comes from the bot's
`look` seed. A mood comes from the bot's status:

| mood | when | moves |
|---|---|---|
| sleep | idle for 15 min | still (breathing only on web/phone) |
| idle | ready | tail sway, blinks |
| work | turn running | kneading/typing paws, tail |
| think | queued | head tilt, tail flick |
| wait | needs you (approval, question) | ears up, bounce, "!" |
| talk | sent a bot message in the last minute | mouth, bubble |
| error | last turn failed | ears flat, still tail |
| away | remote, unreachable | greyed, still |

- Web: `Cat.svg(look, mood, px)` is one self-contained animated SVG (SMIL),
  shown as an `<img>` data URI; the browser animates it off the main thread.
- Native window: `Cat.pose(look, mood, ms)` gives polygons per colour for
  a time; flattened once per (look, mood), moved by an affine per frame.
  The window asks for frames at ~30 fps only while a visible cat moves and
  the window is focused (sleeping cats never ask).
- Phones: `Cat.rig(look, mood)` as JSON: parts with SVG path data (only
  `M L C Q Z`, absolute), fill colour, and one animation each (`rot`, `tx`,
  `ty`, `sx`, `sy`, `op` with from/to, pivot, period ms, delay ms, eased
  in-out, ping-pong). SwiftUI `Canvas` + `TimelineView`, Compose `Canvas` +
  `rememberInfiniteTransition`.

## Rooms and messages

- `bot_send(to, text, topic)`: `to` is a bot name here, or `name@peer`
  elsewhere. A request between bots goes in a room, not a private
  message, so the person and other bots can follow it: the live room whose
  members are exactly the two bots and whose name is the topic (any case;
  with no topic, the two names sorted, `miso + tofu`), else a new one
  (`RoomSet`, then the post). Asking again, or answering, with the same
  topic lands in the same room, whichever bot sends and on either machine
  (members compare as a bot's id here, or its lower-case name and the
  peer id). A bot on another machine is a member like any other
  (`name@machine`), so the post travels as a shared room post (below).
  `Req.*` in bothub.bend; law `bot_request_in_room` (a request is never
  posted to a `dm:` room) with `bot_request_fresh_id`.
- `bot_send` with `direct: true` is a private message, for when the person
  asks for one: room `dm:<a>:<b>`, names sorted, as before.
- `room_post(room, text)`: every other member gets it in their inbox.
- All are logged as `RoomPosted`, so people can read every conversation.
- The hop bound covers all of them alike.
- Delivery to a local bot goes through the thread inbox (`Hub.inbox`): it
  queues behind a running turn, never interrupts.

A person may write to a bot on a linked machine (`bots.tell`, `name@machine`):
the post goes in their direct room there (`dm:<name>@<machine>:you`) and the
bot hears `[message from person@<machine>]`. It answers in its own thread,
which that person cannot see, so when such a turn completes the hub sends
its newest answer back (`Reply.back`, run by the `bots.answered` request the
server sends itself on `TurnEnded`): logged here in the room the message
came in, and posted in the person's direct room there, waking no one (laws
`reply_*`).

### Shared rooms

A room's members can be on other machines (`name@machine`). A post goes to
each linked machine once (`Bots.remotes`), with the room's id, name, the
bots there it is for (`tos`), and its members as that machine names them:
a bare name is a bot there, `name@` a bot on the sender, `name@machine` a
bot on a third machine. The receiving machine keeps the room under the
same id (a `RoomSet` when it is new or its members changed), logs the post
once, and wakes the bots it is for (`Bots.shared`), so its bots and person
see the room and can post back. A machine sends its own posts to every
other and never passes on one it received, so nothing loops; a member on a
machine the poster is not linked to does not hear that post.

A linked machine may post in a room here only when the room is new here or
has a member on that machine (law `bot_room_no_takeover`), and never opens
a room the person deleted (law `bot_room_deleted_stays`). The other
machine learns of a room at its first post.

Every room list names a room by its id, so a shared room shows once: in
a hub's own list (the window, the web) and in a phone's list over several
paired hubs, which both report it (`BU.once.of`, `Hubs.rooms`; laws
`rooms_listed_once`, `hubs_rooms_listed_once`). Rooms are never merged by
name: two rooms with one name both show, each named with its id, and on a
phone with several hubs each row also carries its machine.

## Notifications

Settings’ **Notify me for** section controls Needs me, Settled, Reminders
and Everything on the window, web, iOS and Android.

The person is alerted (the phones' APNs push, the phones' own alerts,
desktop notifications, and the same relayed to the owner's other machines)
in three cases only (`src/core/notice.bend`, `Policy.*`, laws `policy_*`,
`notify_*`):

1. **Needs you.** An ask opens for the person (any kind: a tool approval,
   a plan, a question, an input, a Google approval, a browser task that
   is stuck), or a bot calls `notify_person` with reason `needs_you`. It
   alerts when the ask opens, under the key `ask-<thread>`.
2. **Settled.** Work the person gave a thread is finished: its turn
   completed with everything it waits on done, or its branch merged. One
   alert per settle, key `turn-<thread>`. A failure counts when the person
   must act: the turn they started failed.
3. **Scheduled.** A bot's `notify_person` with reason `reminder` or
   `digest` in a turn a routine started, or a reminder that came due
   (`Policy.reminder`: kind `reminder`, key `remind-<id>`, never held by a
   desk).

Everything else alerts no one and stays visible in the app (unread,
badges, the Status plane): intermediate turn ends, progress, turns woken by
rooms or bots, posts between bots, reports to launcher threads,
routines that do not call `notify_person`, delegated-task notes, stops,
and failures of turns the person did not start.

**Settle once.** The person's message in a thread is the newest user
message typed by them (`Policy.person`: not a link `t:<from>:<n>`, not a
`[message from …]`, `[routine: …]` or `[Backplane] …` note, except
`[message from person@…]`, the person typing from another machine). A turn
end settles it when the thread is ready (`Policy.ready`: not running,
nothing queued, no child tasks, background or watched work, no open ask,
no reporting launched thread still working) and the message is not the
one already settled (the settle memory). So a thread that launched helpers
and ended its turn early alerts once, at the end of the turn the last
report wakes; a bot's routine or room turn after the person's message
settled alerts nothing. Stopping a thread ends its episode quietly.

**The record.** The hub writes each alert as the setting `alert.<thread>`
in the step that causes it (`Alert.*`): `kind\tkey\tsettled\tstamp\ttext`,
kind `ask`, `done`, `fail`, `merged`, `note` (`notify_person`) or empty (a
settle whose setting is off, which still counts as settled). Every step
passes through `Alerts.changes` (an `AskOpened` gets its record) and
`Alerts.outs` (a record with a kind raises the `Alerted` out) before it is
stored, so alerts come from one place. The server's `Alerted` pushes to the
phones (`Push.note.*`, collapse id the record's key), shows the desktop
notification on the desk used last and tells the owner's other machines;
`TurnEnded` only updates the island. A phone alerts for a thread when its
record changed in the frame it folded and the new kind is not empty
(`Note.ralerts`; never at a cold join or a saved state). The apps show the
record's text (a note, an ask's tool and detail, a merge's url) or the
thread's last reply.

**Settings** (hub settings, `on`/`off`): `notify.needs` (on), `notify.settled`
(on), `notify.reminders` (on), `notify.all` (off: also alert whatever the
older rule `Notice.of` alerted, room exchanges and every turn of a plain
thread, as before this policy).

**Bots.** `notify_person {text, reason}`: `needs_you` (once per turn),
`done` (work the person gave the bot in its own thread, once per message;
the turn's end then alerts nothing more), `reminder` or `digest` (only in a
turn a routine started, once per turn). Anything else is refused with the
reason (`Bots.notify`); with the matching setting off the call succeeds and
records no kind.

A held alert is logged as `backplane: push alert held for <thread> (a desk
in use is showing it)`.

### Desks

The window and the web page are desks (`src/core/desk.bend`). Each tells
the hub which thread it shows and whether it has the focus, when either
changes and at most once a minute while in use. A desk is in use only after
the person's input: keys, the pointer, the wheel, focus changes. A redraw,
a resize, an expose, a drag passing over, a timer or the window closing is
not (`Nui.input`, laws `desk_redraw_not_input`, `desk_key_input`). A thread
a focused desk shows, used in the last 3 minutes, is being watched: an ask,
a settle, a failure or a merge for it raises no phone alert (neither the
APNs push nor the phone's own), no desktop notification and no note to the
owner's other machines. A `notify_person` note is never held. Any other
alert also shows as a desktop notification on the desk used last: the
browser's notifications (the page asks for permission on the first click or
key; macOS, Windows, Linux) or `notify-send` from the Linux window.
Clicking it opens the thread.

## Already told

A record of what the person has actually been told, so a bot's digest, a
reminder or a second bot does not say it again (`src/core/told.bend`).
Each record is the setting `told.<id>` holding `{key, text, channel, bot,
at}`: the topic key (lower case, runs of characters outside
`[a-z0-9._:-]` become `-`, 120 at most), what was said (280 characters at
most), the channel (`alert`, `digest`, `room`, `chat`, `email` or `other`),
the bot that said it (`""` for a thread that is no bot's) and when.

Every alert is marked by itself. Since the notification policy, each one
comes from a record `alert.<thread>` that some step writes with a kind
(see Notifications): a settled turn, an ask, a failure, a merge, a
`notify_person` note. `Told.after` (called from hub.bend's
`Alerts.changes`, which every step passes through) marks each record that
has a kind, under the record's key (`turn-<thread>`, `room-<room>`,
`note-...`) with channel `alert`, the thread's bot, and the record's text
or else the thread's title and the first line of its last reply. A quiet
record marks nothing: a bot's chatter, a report from a launched thread
(bot-to-bot), a stopped turn. Whether to alert stays `notify_person`'s
policy; `told_*` only says what was already said. Agents use two tools, in every thread:

- `told_check {key?, text?, days?}`: whether the key matches, or the words
  of the text overlap a record's (at least 60% of the smaller set and 2
  words) within `days` (default 7, at most 30). Answers `{told, matches:
  [{key, text, channel, bot, at, ago}]}`.
- `told_mark {key?, text, channel?}`: records it. Without a key one is made
  from the text's first eight words. Answers `{key}`.

Records live in the settings, on this machine only (they are not synced to
the owner's other machines). The minute tick removes records older than 30
days and all but the newest 2000 (`Told.prune`, reading only the keys: a
record's time is in its id). An empty value removes a `told.*`, `rule.*`
or `board.*` setting (`State.setting`), so none are left behind.

## Standing rules

What the person wants always done, or never done (`src/core/rules.bend`).
A rule is the setting `rule.<id>` holding `{id, text, bot, project, tool,
enforce, word, on, at, by}`: its words; the bot it is for (`""` all), the
project (`""` all), tool patterns (names, or a prefix ending in `*`,
separated by commas or spaces), whether it is enforced, the word that
unlocks it, and whether it is on. The person edits them with `rules.set
{rule}` (with an `id` it changes that rule) and `rules.drop {id}`; the
reply carries a validation error (`bot` and `project` may be names).

Every turn of every thread starts with the `<rules>` block of the rules
that apply to it (`Rules.agent`, outermost in `Bots.turn`, before the
memory block and the references). An enforced rule with a tool is also
checked: the server's MCP gate (`Hub.mcp.gate`, `Rules.gate`) answers a
call of a matching tool with an error saying which rule refuses it, and
the permission prompt gets a deny, unless the person's latest message
says the rule's word as a whole word (a message delivered from a room,
bot, routine, webhook or space starts with `[` and never unlocks). A tool
name matches as given or without its `mcp__<server>__` prefix. The reach
is Backplane's own MCP tools and anything that asks permission; a
provider's built-in tools in a bypass mode never reach the hub, so a rule
on them can only be injected as words. Rules are settings, local to this
machine (not synced to others).

## Task board

One shared list of what is to be done (`src/core/tboard.bend`). An item is
the setting `board.<id>` holding `{id, title, status, owner, project, due,
group, links, notes, at, updated, by}`: status `todo`, `doing`, `blocked`,
`done` or `dropped`; the owner a bot's id (`""` is the person); the due
date `YYYY-MM-DD`; at most 20 links and 4000 characters of notes. Agents
in every thread have:

- `task_board_list {status?, owner?, project?, group?, all?}`: open items
  by default, in board order (open before closed, due first). `owner` may
  be `me` or `person`. Answers `{text, items}`.
- `task_board_add`, `task_board_update` (only the fields given change;
  `notes_append` adds a line) and `task_board_remove` (prefer status
  `dropped`, which keeps a record). Names or ids for owner and project.
- `task_board_import {markdown, project?, owner?}`: checkbox items
  (`- [ ]`, `- [x]`, `- [~]`, `- [!]`, `- [-]`; `*`, `+` and `1.` too)
  become items, headings become groups (a heading naming a project also
  sets the project), a due date at the end of a title is taken out of it,
  indented lines under an item are its notes. Other lines are returned as
  `skipped`; importing the same list again adds nothing.

The person edits the board with `board.set {item}` (with an `id` it
updates) and `board.drop {id}`. The items are settings, local to this
machine (not synced to others).
### In the clients

Settings › Rules (after "Notify me for") lists the rules with On/Off, Edit and Delete (the first click asks "Delete?"); Add rule opens the form (text, bot and project chips, tool, word, Enforce). The Board is an entry above Rooms ("Board · N open"): filter chips, an add field, sections Doing, Blocked, To do (and Done, Dropped with the filter "all"); the box ticks an item on, a row opens its notes, links and Blocked, Drop, Delete. The Status tab has a Board section (at most six open items). All four clients draw the same `RulesUi`/`BoardUi` (client.bend) over `ruledata.bend`/`tbdata.bend`; actions are `rule-*` and `tb-*` (`Ui.act.w`). The rule form's fields are `@f.rutext`, `rubot`, `ruproj`, `rutool`, `ruword` (`rtext` is the remote chat's).

## Mentions in threads

In any thread's composer (web, window, phones) a word that starts with a
sigil names something, and a menu offers matches with their summaries as
you type:

| sigil | names | when sent |
|---|---|---|
| `@name`, `@name@machine` | a bot, here or on a linked machine | the bot is woken (hop 0) with the message and the thread it came from; it can `thread_read` it and answer with `thread_send` |
| `>slug` | a thread | the agent is told the thread's id, project and summary |
| `>machine:slug` | a thread on another of the owner's machines | the agent is told what it is and where; `thread_read` reads it |
| `%slug` | a project | the agent is told the project's id, folder and summary |
| `#slug` | a group chat | the message is posted in the room |

`@Miso look at >fix-perf-lag` points a bot at a thread. A slug is the
title in lower case with every other run of characters one `-`; a thread
whose slug another shares is `slug-<n>`, the number its id ends with. The
thread's own agent gets the message as typed plus a `<references>` block
naming each (`Refs.agent`); the timeline keeps it as typed. A bot is never
woken by a mention in its own thread (law `refs_own_bot_never`), and a
message resent after a dropped connection wakes nothing twice
(`bot_mention_once`). Bots on linked machines are offered too, from the
list every client gets (info `bots.remote`), as `@name@machine` (the
machine's link id when its name could mean another, `Refs.fars`); sent,
the bot hears the message as the person's, with the thread's title, which
it cannot read, and answers in the person's conversation with it
(`Bots.mention.far`, the same room `bots.tell` writes in).

Threads synced from the owner's other machines (below, "Threads
elsewhere") are offered as `>machine:slug`, the machine by its name as a
slug (`Refs.x.*`); their rows start with the machine's name, which a query
matches too, and `>machine:` alone offers only that machine's threads. A
`>machine:slug` never names a thread here (`>fix: it fails` still names
`fix`: a slug must follow the colon, laws `refs_machine_word`,
`refs_colon_after_stays_here`). The hub has no read model of other
machines' threads, only its mirror of each, so `src/core/xref.bend` folds
the one mirror a word names when it must: for the agent's
`<references>` block (server.bend's `Far.turn`, around `Bots.turn`), and
for `thread_read` on a `<link>~<id>` (`Xref.read`, answered from the
mirror, before the MCP call reaches the hub).

The menu (`Refs.offer` in `src/core/refs.bend`) ranks by how well the
query matches the slug or name (fuzzy), a query of three or more letters
found in the summary, then nearness: the same project, recent work, pinned
or running threads. It is worked out once per edit of the draft and kept
in the client's info map, so frames and phone screens only read it.

Summaries come from the text generation model (the one that titles
threads): a thread's when a turn ends, at most every ten minutes
(`summary.<thread>`), and its project's after it, from the project's
newest threads (`summary.<project>`). A bot's is its thread's; before its
first turn the menu shows its persona's first line.

## Scratch and giving a thread away

Not every conversation belongs to a project. Scratch (a project every hub
makes for itself, at `<home>/scratch`) holds throwaway threads: the `+` on
its sidebar row starts one, and each works in a folder of its own
(`<home>/scratch/<thread>`). They are ephemeral: one left alone for the
`scratch.days` setting (default 7) is deleted with its folder at the next
tick. Pin one to keep it.

To take a conversation somewhere, type `/give` in its composer with the
references it goes to and, if you like, a note (the send button reads
"Give"):

| names | what happens |
|---|---|
| `>slug` | the conversation is queued into that thread, ahead of its next message (like a fork brought back) |
| `%slug` | a new thread in that project starts with it queued (like a fork) |
| `@name` | the bot is woken with it |

`/give >pcb-rev-b %enclosure keep the connector choice` hands one scratch
conversation to a thread and to a new thread in another project at once;
several scratch threads given to one thread gather there. The thread it came
from logs where it went and stays where it is. `/give` works in every
thread, not only scratch ones (bothub.bend's `Give`, laws `give_*`).

## Machines and people

A peer is another hub, linked by an invite: the inviting hub makes
`bp1:<url>:<peer id>:<secret hex>`; the other pastes it, and both keep the
32-byte secret. Every hub-to-hub request is signed like a webhook (below),
so the same verification guards both. Linked hubs exchange their bot
directory (name, look, mood) every minute and deliver messages with
`POST /bots/deliver`.

## Webhooks

`POST /hook/<hook id>` with

    X-Backplane-Timestamp: <unix seconds>
    X-Backplane-Signature: sha256=<hex HMAC-SHA256(secret, ts + "." + body)>

- The timestamp must be within 300 s of the hub's clock.
- A signature is accepted once (replay cache for the window).
- The comparison is constant time; the body is capped at 256 KB.
- GitHub's `X-Hub-Signature-256` is accepted with `X-GitHub-Delivery` as
  the replay key.
- The secret (32 random bytes, hex) is shown once, when the hook is made.
- The payload reaches the bot marked as untrusted data, never as
  instructions.

### Bearer tokens

A sender that cannot sign (the Pebble Index ring's app sends only fixed
headers) can use the hook's bearer token instead: `Authorization: Bearer
<token>`. A hook has none until someone asks for it (the Token button,
`bots.hook_token` with `op` "show", "new" or "off"). The token is 16
random bytes as 32 hex digits, kept by the server in
`<home>/secrets/hooks/<id>.token` (0600, gone when the hook is revoked),
never in the log; it is compared in constant time. A signature, when
there is one, is checked instead of the token. Each call with the token is
new to the replay cache (a static token proves nothing about freshness).
Refusals say why: no credentials, no token on this hook, wrong token, or
a bad or stale signature.

A `multipart/form-data` body under a token becomes a JSON object for the
bot: `source` ("pebble-index-01" when `client` is "ring", else
"form-data"), `transcription`, `recordedAt` (a number), `client`, and
`audio` as `"omitted, <X-Audio-Size> bytes"`: a bot cannot hear M4A, and
keeping untrusted binaries in its folder buys nothing. When Backplane
transcribed the recording itself, `transcription` is its words, with
`heardBy` and `unsure` (the words to ask the person about). Other bodies go as
they came. Either way the bot reads it as untrusted data, like any
webhook's.

The URL shown with the token is where a phone reaches the hub:
`BACKPLANE_HOOK_URL` when set (a funnel, a relay), else the HTTPS address
`tailscale serve` gives the hub's port, else its tailnet address over
plain HTTP (the Pebble app wants HTTPS; the panel says so). To give a hub
an HTTPS address on the tailnet without taking port 443 from another
service: `tailscale serve --bg --https=3788 http://127.0.0.1:3787`.

### A Pebble Index ring

Make a webhook for the bot, press Token, then in the Pebble app (Index 01
Settings, Webhook, a gesture):

- URL: the one shown with the token (`https://<machine>.<tailnet>.ts.net[:port]/hook/<hook id>`;
  the phone must be on the tailnet)
- Header: `Authorization` = `Bearer <token>`
- Send: Transcription, or Recording (or both) to have Backplane transcribe it
  with GPT-Live-Transcribe and your dictionary (docs/voice.md; needs the
  OpenAI key in Settings, Voice; forms may be 2 MB)

What arrives is untrusted data for the bot (above).

The hub also takes the Index webhook protocol's HMAC signature
(`X-Index-Signature`, version 1) where an app sends it: then configure:

- URL: `https://<this machine>.<tailnet>.ts.net/hook/<hook id>` (the hub
  started with `--tailscale-https`; the phone must be on the tailnet)
- Sign requests: on, with the webhook's secret pasted as it is shown
- Send: as above (a signed form with a recording is checked over its bytes)

A signed voice note reaches the bot as the person's own words, `[voice note
from your Pebble ring]` and the transcription, starting a conversation
at hop 0: only the phone holding the secret can sign it. The app's
"Send test event" is logged and wakes no one. Each gesture has its own
URL, so Hold & talk and Double click & hold can go to different bots.

## Routines

A routine is a cron line (5 fields, the hub's time zone, `*`, lists,
ranges, steps, and `@hourly`/`@daily`/`@weekly`) and a prompt. The hub's
minute tick fires a due routine once (`RoutineFired` records it), even
after being off for a while (one catch-up run, never a burst).

## Memory

Keyed, typed items, not a notebook:

- kinds: `user` (about the people it works for), `pref` (how they want
  things done), `fact` (about the world, projects, contacts), `howto`
  (procedures it learned), `note` (what happened; short-lived).
- `memory_save(key, kind, text, tags)` replaces the item with that key, so
  a bot updates what it knows instead of piling it up.
- `memory_recall(query)` ranks by words in key, tags and text, then kind,
  then recency. `memory_forget(key)`.
- Each turn a bot starts gets a short `<memory>` block: all `user` and
  `pref` items, then the few items that match the turn's text, within a
  fixed budget (law `mem_block_budget`). Notes older than 30 days leave the
  block but stay recallable.

## Space

A bot's own UI, in its Space tab. The bot writes it with `space_set(spec)`,
a JSON document of blocks:

    {"title": "...", "blocks": [
      {"type": "heading", "text": "..."},
      {"type": "text", "text": "..."},             (markdown-ish, plain)
      {"type": "stat", "label": "...", "value": "...", "hint": "..."},
      {"type": "progress", "label": "...", "value": 0.4},
      {"type": "list", "items": ["...", ...]},
      {"type": "kv", "rows": [["k", "v"], ...]},
      {"type": "table", "head": ["a","b"], "rows": [["1","2"], ...]},
      {"type": "button", "label": "...", "action": "..."},
      {"type": "input", "label": "...", "action": "...", "placeholder": "..."},
      {"type": "divider"},
      {"type": "row", "blocks": [ ...stats/buttons... ]}
    ]}

`Space.parse` keeps what it understands and caps sizes. A button or input
sends the bot `[space] <action> <value>` as a human turn (hop 0). The spec
is data, never code a client runs, so a bot cannot script a client.

A bot can also write its space as a web page with `space_page(html)` (HTML
and CSS, at most 256 KB; `""` removes it; event `SpacePageSet`). The phone
apps show it full screen in the Space tab in place of the blocks; desktop
and web keep the blocks, or say the space is a page when there are none.
The hub serves it at `GET /bots/<id>/space.html` behind `Space.page.head`,
whose policy (also sent as a header) runs no script and loads nothing from
the network: images and fonts are `data:` URLs (law `space_page_locked`).
The apps turn scripts off too (Android also blocks network loads) and load
it with no origin. A link to `space:<action>`, or a form with
`action="space:<action>"` and a field named `value`, works as a button or
an input: the app hands the URL to the `space-link` action and
`Space.link` reads it; any other URL sends nothing (law
`space_link_only_ours`), and web links open in the phone's browser. Only a
tap or a submit counts, never a navigation the page makes by itself.

## Browser

Each bot has its own page (a tab in the shared Chrome, one persistent
profile, so logins last). The page's frames are JPEG files the hub serves at
`GET /bots/<id>/screen.jpg`; a `{"t":"shot","bot":...,"n":...}` message says
when a new one is ready, and only clients showing that tab fetch it.

## Google

Gmail and Calendar through Google's APIs, not the browser: OAuth 2.0 for
installed apps with PKCE and a loopback redirect to the hub
(`/oauth/google`), or paste the redirected URL when the browser is on
another device. Tools: `gmail_search`, `gmail_read`, `gmail_send`
(asks you first), `calendar_events`, `calendar_create` (asks you first),
`google_accounts`.

Google always sends the browser back to `127.0.0.1:<port>`, so a sign-in
started on another machine's hub (a phone, or a window linked to that
hub) lands on the hub of the machine the browser runs on. The sign-in's
state names the hub that started it (its tailnet address,
`Google.state`), and a hub whose sign-in it is not answers with a
redirect there (`Google.hop`: only to a `*.ts.net` address, only once;
laws `google_state_*`, `google_hop_*`). With no hub on the browser's
machine, paste the address the browser ended on instead.

The phone apps sign in with no hub on the phone (RFC 8252 7.3): Connect
Google (or Add account) opens a listener on a loopback port of the app's
own (`GoogleLoop` in `GoogleLoop.swift` and `GoogleLoop.kt`) and sends
the `google` action `connect@<port>`, so the hub begins the sign-in with
`http://127.0.0.1:<port>/oauth/google` as its redirect (the RPC's
`redirect`; the hub takes only exactly that shape, `Google.loopback`,
else its own: laws `google_loopback_*`). The app opens the link the hub
answers when its `redirect_uri` is the listener's (iOS in an
`ASWebAuthenticationSession`, Android in the browser), the listener
catches Google's redirect, sends the browser back to the app
(`backplane://google`), and the app sends `done:<address>`: the hub's
`finish` with that address, traded with the redirect the sign-in began
with. It finishes on the hub in focus; the pasted address stays as the
fallback.

Several accounts: each sign-in adds one (Google shows its account
chooser; signing in to an address already there replaces it). They live
in `<home>/secrets/google.json` under `accounts`, the default first (a
file from before keeps its one account at its top and is read as the
first). Every tool takes an optional `account` address; without it, the
first. Settings lists the accounts, each with its own Sign out (the
`google` action `signout:<address>`; `disconnect` with no address signs
out all). Pure half: `Acct`/`Accts.*` in `src/core/google.bend`, laws
`google_accts_*`.

Connect Google in Settings signs in with Backplane's shared OAuth client,
so nobody pastes anything. The client is baked in at build time:
`scripts/build-app.sh` reads `BACKPLANE_GOOGLE_CLIENT_ID` and
`BACKPLANE_GOOGLE_CLIENT_SECRET` from its environment (release builds get
them from the repository's Actions secrets) or from a git-ignored `.env` in
the checkout or the main checkout, and `src/server/effects/google.c` hands
them to `Gs.shared`. The same variables at run time win over the baked
ones; with neither, `Google.shared` in `src/core/google.bend` is used.
The shared client asks for Gmail and Calendar (laws `google_shared_*`).
Gmail's scopes are restricted: until Google verifies the client (a yearly
security assessment), people click past an unverified-app warning and at
most 100 accounts can sign in. Own client takes a Google Cloud OAuth client
(Desktop app) of your own; it always wins over the shared one (law
`google_own_client_wins`).

Registering the shared client: a Google Cloud project with the Gmail and
Calendar APIs on, an OAuth consent screen (External, In production, scopes
`gmail.modify` and `calendar.events`), and an OAuth client of type Desktop
app. Its id and secret go in `.env` and the repository's Actions secrets
(`BACKPLANE_GOOGLE_CLIENT_ID`, `_SECRET`); an installed
app's secret is not secret (RFC 8252 8.5), PKCE protects the code. Until
Google verifies it, people see an unverified-app warning and at most 100
can sign in; while the consent screen is in Testing, sign-ins expire
after 7 days.

## Server

`src/server/botnet.bend` (secrets, signing, verifying, the directory) and
the Bots section of `src/server/server.bend` (effects, routes, Google's
tools). Routes:

| route | who | auth |
|---|---|---|
| `POST /hook/<id>` | anyone with the hook's secret or token | our signature, GitHub's or the ring's, replay cache; or the hook's bearer token; 256 KB |
| `POST /bots/deliver` | a linked machine | `X-Backplane-Peer` + signature with its secret, replay cache |
| `POST /bots/link` | the holder of an invite | the same; once per peer id |
| `GET /bots/dir` | a linked machine | the same, over the empty body |
| `POST /far/push`, `/far/log`, `/far/rpc` | one of the owner's linked machines | the same |
| `GET /bots/<id>/screen.jpg` | clients | loopback, or the pairing token |
| `GET /oauth/google` | the user's browser | loopback, or the pairing token; Google's state |

The signature is checked off the hub (the secret read from its file);
the hub then accepts the replay key once and only then changes anything.
Peer ids and hook ids name files only when they are plain ids
(`[A-Za-z0-9_-]`, at most 80). Hub-to-hub requests go through `curl` with
the signed headers on stdin and the body in a 0600 file under
`<home>/secrets/tmp`, http(s) only, no redirects. This machine's name for
others is `BACKPLANE_NAME`, else its host name; its address is its
MagicDNS name on the tailnet (same port), else its listen address.

Every minute the hub runs due routines (`date +%z` for the time zone)
and asks each linked machine for its bots; the merged list is the info
key `bots.remote` (a machine that does not answer keeps its bots, away).
`test/tools/bots_e2e.ts` runs two hubs against each other; `test/tools/newproj_e2e.ts` runs project_create on one.

Each bot is listed once (`src/core/once.bend`; laws `bot_rows_once`,
`bot_picks_once`, `hubs_bots_once`). A bot here is keyed by its id, a bot
elsewhere by its name (any case) at its machine's link id, so one bot
reported twice, or named `Kit@box` in one room and `Kit@<box's id>` in
another, is one entry, and bots with one name on two machines stay two.
`name@machine` finds a machine by its id first, then the first by name;
when a name could mean two machines (two called `laptop`, any case), the
bot is `name@<id>` and shown as `laptop (<id>)`. Two bots here with one
name show their ids. A phone paired with two linked hubs shows a bot once,
as its own hub's.

A bot elsewhere that lives on one of the owner's own machines (the
machine switcher's list: the same address, or, when the link's address is
on the owner's tailnet domain, the peer's name or MagicDNS label on the
same port, `Far.machine` in client.bend) is mirrored here (below) and
opens here, at once, as a bot like any other. A bot on anyone else's
machine, or one not mirrored yet, is a conversation (`bots.tell`). The
window no longer switches hubs to show a bot: that fetched the other hub's
whole log over the link (11 to 20 s for a 13 MB log, `test/native/link_bench.bend`).

## Threads elsewhere

`src/core/far.bend`. A hub shares its projects, ordinary project threads and bots (turns,
messages, tool calls, asks, delegated tasks, bot space, memory and routines) with the owner's other machines it is
linked to. It shares no settings, devices, links, webhooks or rooms (rooms
travel on their own, above), and nothing with a machine that is not the
owner's (`Far.own` in server.bend: the machines list; laws `far_*_stay_home`).

- Names. A thing on the machine behind link `L` is `L~<its id>` here
  (`Fr.id`; local ids never hold a `~`). So nothing from elsewhere can take
  the place of anything here, a bot shows once (its mirror and the
  directory's report of it share `BU.key`, name@link; law `bot_rows_once`),
  and a phone paired with both machines keeps the owner's own.
- Push. The owning hub is the authority. Each commit's shared changes go,
  numbered by its log, to each such peer: `POST /far/push`
  `{"name", "epoch", "since", "head", "items": [{"n", "c"}]}`, one push out per link
  at a time, later batches merged behind it (`Out.*`). A batch says it
  follows the last push that got through only while every push since did;
  after a failure or a restart it says it follows only its own start.
- Pull. A mirror takes a batch only when it follows what it has
  (`since <= head`) and only the items it lacks (`n > head`); a batch past a
  gap takes nothing and the rest is pulled: `POST /far/log {"since", "epoch"}`
  answers a page of about 400 KB `{"since", "epoch", "head", "items", "more"}` from the
  owning hub's feed (its shared lines, made from the log off the hub at
  start, then grown by each commit). A batch that comes twice, late or out
  of order changes nothing (laws `far_gap_takes_nothing`,
  `far_old_takes_nothing`, `far_overlap_takes_new`, `far_batch_again`).
- History identity. The epoch is a cached hash of the first durable wire
  change. It stays the same through restart; replacing that first change
  changes it. A pull with another epoch starts at zero. A push naming
  another epoch triggers a pull from the authority (a delayed push cannot
  revert history). Clients remove only that link's old projection, preserving
  their local sequence and other mirrors. Old mirror files and epoch-free
  messages remain readable. An exact copy of the original history prefix
  has the same epoch; reset history must replace its first change.
- Mirrors. The receiving hub keeps one per link, in memory and in
  `<home>/far/<link>.jsonl` (an epoch header, each batch's items, then `{"head"}`), and
  sends it to each client that joins (`{"t":"far","link","name","away",
  "epoch","since","head","items"}`, `since` 0) and each batch as it comes. A
  client folds it into its read model under the far ids, never counting it
  in its own log's sequence (a reconnect still asks its hub for exactly what
  it missed), and keeps how far it has each link beside the read model, so
  a fresh log forgets both at once (`Fr.fold`, `Ui.far`).
- Freshness and partitions. No polling of its own: pushes as changes
  happen; a pull at start (after the mirror is read back from disk), on a
  gap, and when the minute's directory exchange finds a link that was away
  answering again. When discovery recognizes a linked owner machine, it
  both offers the current head and pulls that peer's authoritative history
  (`Far.returned`): an earlier hint may have arrived before this side
  recognized ownership and been refused. This also checks a healthy cached
  mirror after a quick restart. These are one-time discovery actions; the
  existing minute directory and two-minute machine discovery remain the
  only periodic checks. A link that does not answer the directory, or a pull,
  is away: its bots and threads stay listed and readable, marked away
  (`far.away.<link>`, mood "away", "Kit · box (away)").
- Requests. What a client does to a thread elsewhere (write, stop, answer
  an ask, its queue, settle, a side question, its modes, a bot's space and
  routines: `Fr.methods`) goes to its hub in that hub's ids (`Fr.route`,
  `POST /far/rpc`), signed like every hub-to-hub request; the owning hub
  takes it only for what it shares (`Fr.allowed`). The client's answer
  keeps the owner's success, error and payload, with the caller's request id;
  while it is away the answer says so. Anything
  else about a far thread is refused here (law `far_route_here_stays`,
  `far_allowed_only_methods`).
- Creating anywhere. `thread.create` (its `project` a `<link>~<id>`),
  `thread.fork`, `thread.pin`/`unpin`, `thread.archive`/`unarchive` and
  `project.remove` go to the owning hub like the rest, which makes the thread
  in its own project; its answer names the new thread by its id here
  (`Fr.reply.on`: `<link>~<thread>`, the id the mirror will give it). The
  asking client keeps "making" (`Made.waiting`, `Made.coming`) until the
  mirrored thread is here, then opens it (`@thread.want`, `Bots.wanted`),
  whichever of the answer and the push comes first. The project picker can
  work on another of the owner's machines: a row at its end ("On box
  (change)", action `proj-on`, `Proj.machine`) cycles this hub and each
  machine that has answered; its requests (`fs.list`, `fs.mkdir`, `fs.find`,
  `project.add`, `project.new`) then carry `on: <link>`, which `Fr.params.link`
  routes. The owning hub runs those from a job (`Far.fs.job`: the same
  `FsJob` as for its own clients, then `FarDo` for `project.create`) and
  answers the open request. `Fr.allowed`: a create needs one of its own
  projects (not a mirror, not one it lacks); pin, archive, fork a thread it
  shares; folders and adding need nothing but an owner's link. `thread.delete`
  is not one of them (law `far_no_delete`): a thread elsewhere is deleted on
  its own machine. Laws `far_create_*`, `made_far_*`, `proj_machine_*`; tests
  `test/far_test.bend`, `bun test/tools/create_far_e2e.ts`.
- The same view. A mirrored bot is a bot in the read model, so the window,
  the web and the phones show it with the same header, tabs, space and
  chat, its thread with the same timeline and composer; only its name
  carries its machine ("Kit · box", `Label.title` and `Ui.far.title`). Its tabs are what
  its machine shares (chat, space, memory, routines; `BU.tabs.of`). A reported-only
  choice adopts the full thread when its mirror arrives, keeping an unfinished
  remote composer in place until it has been sent (`Far.arrived`). Ordinary
  thread rows, project sections and timeline headers use the same machine
  marker. On phones paired with both hubs, the paired owner's catalog
  replaces mirrored sections/rows once it has loaded (`Hubs.catalog.*`);
  its cached mirror stays visible until then. Direct paired-owner Active rows and focused thread/bot headers keep the machine name too; an offline owner is marked away, and an already-marked mirror keeps its original title. When the paired owner differs from
  the focused hub, its machine badge remains visible. Hidden Older and Settled
  counts also drop duplicates.
- Pairing. The owner's machines link by themselves; nobody pastes an
  invite (`Mate.*` in client.bend). Every two minutes a hub lists the
  tailnet's online machines of its owner that answer `GET /hello`
  (`Machines.*`), and each minute it asks those it is not linked to and
  whose address is above its own (so of two hubs exactly one asks) with
  `POST /bots/pair?name=<its name>&url=<its address>`. Only the owner's
  own devices may call it (the tailnet listener's `tailscale whois`, the
  same trust as the websocket); the answer is a fresh invite, joined like
  a pasted one (`BotJob.join`), unless the asker is none of the machines
  the hub found (403: its link could not reach back), sync with it is off
  (403) or a live link already reaches it (409). A hub started with
  `--no-tailscale` still lists the tailnet machines to switch to but pairs
  only with hubs `BACKPLANE_PEERS` names (`named`), so a throwaway hub
  never links itself to real ones. Laws `mate_*`; `test/tools/far_e2e.ts`
  pairs its two hubs this way.
- Choosing. Settings, Your machines (all four clients, `Mate.rows`) lists
  each machine found, how its sync goes (linking, in sync, catching up N
  projects, away) and On/Off: the setting `sync.<name>`. Off, nothing is
  pushed to it or taken from it (`Far.own`), it is not asked to pair, it
  is refused if it asks, and its projects leave the sidebar
  (`Side.projs`, law `mate_off_hidden`).
- Catching up. A pull's answer carries the owner's catalog: every project
  it shares, with its title, latest activity and the number of its last
  shared change (`Fr.catalog`; the feed keeps each project's number as it
  grows, `Fh.lasts`). The mirroring hub tells clients and keeps it for
  those that join later (`Fh.cat`, the mirror's last chunk). A client
  lists every project at once, in its place (a project not arrived yet
  stands in with its title and activity), and marks each still catching
  up while the link's head is short of its number (`wait~<id>`,
  `Fr.syncing`): faded, titled "Board · desk (syncing)", on all four
  clients through `Label.title`/`Label.syncing` and the phones' `quiet`.
  Projects that have arrived work meanwhile, and nothing pops in or moves.
  Laws `far_catalog_*`, `far_scope_*`.
- Live text of a far turn (core/farlive.bend, laws `flive_*`): the owner's
  streamed reply is not a change, so it never enters the log or the mirror
  file. It goes hub to hub as an ephemeral `live` body on POST /far/push
  (deltas of the owner's Live buffer: `off` = length before, `gen` bumps at
  each end), one queue per peer with one request out at a time (a slow peer
  gets fewer, bigger deltas, like `Delta.merge`). The receiver keeps it in
  its own Live buffer under `<link>~<thread>` and sends its clients the
  ordinary `delta`, so every client renders it like a local reply. A gap
  asks for the whole text; the durable message arriving first ends the
  buffer, so a late delta never brings text back. A peer with no client
  (`watch: false`) is left alone for 8 s; the first delta after finds a gap
  and gets a snapshot. A far turn's end alerts once, by the owner's key.
- Join cost: a mirror's chunks are cut by a tail-recursive walk
  (`Fr.after`), linear in the mirror, not stack-deep (`test/native/far_sync_bench.bend`).
- Not mirrored: a bot's browser. The viewers of a far project thread are below.
- Viewers elsewhere. The viewer panel of a thread on another machine (Board,
  Schematic, 3D, Mech, Files, Diff) works from any client of any hub, the
  window, the web and both phones, with the same Bend defs (`core/fview.bend`,
  client.bend's `Fv.*`, `src/server/farview.bend`). The files stay on the owner's
  machine; the hub the client is on keeps what the viewers read in a cache of
  its own, `<home>/farfs/<link>/<the owner's project folder>`, and a client
  of a far thread reads that folder (`Ui.root`, "" until the hub says which)
  as it reads a project folder here: the window parses the boards itself, the
  plot actor serves phones and the web, `/img` and `pdf.page` read it, with
  nothing else to know.
  - What goes to the owner (`POST /far/rpc`, signed, owner-only like every
    request between hubs, only for a thread it shares; `Fv.reads`, `Fv.allowed`):
    `files.dir`, `files.list`, `mech.list`, `history.refs`, `git.diff`,
    `git.status` (answered by the job a request becomes here, `Fv.jobs`, run on
    a channel of its own; no hub state touched) and two of its own, `view.ver`
    and `view.get`. Nothing writes: the render, KiCad, `file.open`, `pdf.page` and
    `fs.list` are refused or stay local (law `fview_reads_disjoint`). A path
    never leaves the thread's folder, reaches no hidden file (`.git`, `.env`;
    `.backplane.json` alone is read) and follows no symlink out (laws
    `fview_file_*`, `fview_allowed_*`; the owner checks the real path of each
    file and folder, `Fvs.lib`).
  - `view.ver {thread, want, have}`: the files the viewers want of the folder
    (the project's `.kicad_pro/.kicad_pcb/.kicad_sch`, `sym-lib-table`, its small
    models, `.backplane.json`, and the files in `want`; at most 400, each under
    24 MB) as `path<TAB>size<TAB>mtime` lines with a cksum digest. With
    `have` equal to the current digest the owner waits for it to change, at most
    24 s (one `sh` on its side polling `stat` every 2 s, as the plot actor does for
    a file here), so changes flow from the owner and no client polls.
    `view.get {thread, path}` is one file's bytes.
  - `far.view {thread, want, have}`, a client's ask, is answered by its own hub:
    it asks the owner `view.ver`, brings the files that differ (size or time)
    into the cache four at a time, each written beside its place and moved there
    whole, and answers `{farView, thread, root, ver, same, want, ready, changed,
    failed}` (or `down` when the owner is away: the viewer then shows the file
    as it is). The client keeps the digest and asks again at once, so about one
    request per half minute stays parked at the owner while a far viewer is open
    (`Fv.sync`, run after every action and message by all four clients; the
    phones through `View.fv`). The cache index is `<cache>/.farver`.
  - The viewer shows a far file only when the cache holds it: `Ui.view_get`
    reads the Files tab until the answer names the path (`fv.ready:<t>`,
    `Fv.pending`), so a viewer never loads a file that is not there yet.
  - `mech.list` is answered by the client's hub too: the owner's listing, with
    the renders it names brought into the cache first, the listing's thread and
    folder as here (`Fvs.mech`). The other reads come back through the hub with
    the same fix (`Fv.fix`: the thread's id here, `root` and `fmRoot` the
    cache).
  - `viewer_show` by an agent on the owner reaches the clients of the owner's
    other machines: the owner tells each (`view.show`, only a file inside the
    folder), each brings it into its cache and sends its clients a `view`
    message of the thread there with the cache path (`Far.shown`, `Far.show.*`).
  - Limits: a file over 24 MB is not sent (a large STEP is not viewable
    remotely), design history versions are not read remotely (the timeline's
    steps and `history.refs` work, a version of a file does not), a window
    switched to another machine's hub (`Link`) reads files on this machine, so
    only a thread that hub keeps in its own folder shows there, and the cache
    is never cleaned (`<home>/farfs`).

`test/far_test.bend` (the client's fold, routing, sharing) and
`test/tools/far_e2e.ts` (two hubs: a bot made on one shows on the other,
a message from the other reaches it and its answer comes back, away when
it stops, from disk after a restart).
- A window on another hub. The native window can switch to another machine's
  hub (the machine switch, `src/app/link.bend`); its viewers read files from
  the window's own disk, so on another machine's hub they would show nothing.
  They read copies instead (`core/rview.bend`, `src/app/rvw.bend`, main.bend's
  `Watch.follow`): the hub lists a thread's viewer files for its clients and
  sends their bytes, and the window keeps them under
  `$XDG_CACHE_HOME/backplane-bend/remote/<hub>` in the same folders as on the
  hub (a hub's path P is read at `<that>P`, `Rview.at`; the source key the
  viewer holds names the copy, `Nui.source`, client info `local.remote.pre`
  and `local.remote.url`, set at each connection, "" for this machine's own
  hub), so the board, schematic, 3D, Mech and sheet-tree readers are as they
  were. The images and PDFs the window shows (`/img`, `pdf.page`) and the
  Files tab already came from the hub.
  - On the hub, read-only, for a client let in as for the websocket
    (`Conn.allowed`: loopback, the owner's own machine by Tailscale, or the
    token; else 401): `GET /view/ver?thread&want&have` answers the folder, a
    digest and the files `Fvs.ver` lists, and holds while `have` is still the
    digest until a file changes (at most 24 s; the same long poll the owners'
    machines use); `GET /view/get?thread&path` is one file's bytes;
    `GET /view/git?thread&path` is one file of a commit (a history version,
    `git\t<gitdir>\t<commit>...`: the commit must be a hash, the git directory
    a repository's, a worktree's or `<folder>/.backplane/history.git`). The
    hub names the folder from the thread itself (`Rview.root`: the thread's own
    folder, or for a thread it mirrors the cache it keeps of the owner's,
    `farfs`, which `far.view` fills), so a client names no folder; the paths
    pass `Fv.file.ok`/`Fv.wants.ok` and the scripts of `Fvs` check the real
    path (no symlink out). Laws `rview_*`.
  - In the window (`Rvw.sync`, one round): ask `view.ver` with the digest held,
    bring the files that differ four at a time and install them with the
    owners' locked script (never rolling a file back, removing what the hub no
    longer lists), then say whether the viewers read again. The shell starts
    the next round only while the viewer is still open on that source of that
    connection (`RvGot`, `Shell.rvgot`); a source let go is asked for again
    when the viewer returns (`Shell.stale`). A history version or comparison
    asks the hub for each commit's bytes (`Watch.hist.r`). A remote source
    runs no 150 ms stat loop: the viewers read once after each round that
    changed something. Tests `test/rview_test.bend`, `bun test/tools/remote_view_e2e.ts`,
    `test/native/rvw_sync.bend`.

### Agents across machines

An agent (Claude, Codex, Grok or Muse, through the hub's MCP tools) works with the owner's other machines
the way it works with its own project. `src/core/fmcp.bend` is the whole of it.

- `machine_list` (every agent): this machine first, then each linked machine (`here` or `away`) with its
  projects, ids as clients name them (`<link>~<id>`). Every hub starts an agent's prompt with the
  machines linked then (`<machines>`), and the orchestration paragraph says how to address them.
- `thread_list` and `thread_launch` take a `machine` (its name or link id; a project there by name or id,
  or its `<link>~<id>` as `project`). `thread_send` (auto, queue, steer), `thread_wait`,
  `thread_interrupt` and `thread_update` take a thread's `<link>~<id>`, which any answer carries in
  its ids. `thread_read` asks the owning hub too (current even before its push reaches the mirror),
  and reads the mirror (xref.bend) while that machine is away. A word `>machine:slug` in a message
  gets its id in the `<references>` block.
- A call for another machine is not run here: the hub (`Hub.mcp.f`, `Fm.plan.o`) turns it into the signed
  request `mcp.call {tool, from, args}` (POST /far/rpc, the route clients use for threads elsewhere),
  with that machine's own ids, and answers the agent's call with what returns, its ids named by the
  link (`Fm.answer`). No linked machine of that name, or one away, is an error and nothing is sent.
- The owning hub takes `mcp.call` only from one of the owner's own machines (the route's `Far.own.id`;
  an unsigned or non-owner request never gets this far) and only for the tools in `Fm.names`
  (thread_send, thread_wait_now, thread_interrupt, thread_update, thread_list, thread_launch,
  project_list). It runs the same defs as for its own agents, but a named thread must be one it shares
  (`Fr.thread.shared`): otherwise the answer is an error and nothing is stored. `from` is the calling
  thread as the owning hub's clients name it, so the message shows where it came from.
- `thread_wait` on a thread elsewhere is the usual wait loop (a check each second at the hub) where each
  check asks the owning hub (`thread_wait_now`): the owner's own view, so a message just sent is never
  missed for a mirror that has not caught up yet. One small signed request a second per waiting agent.
- `/give >machine:slug` hands the conversation to that thread and `/give %machine:project` to a new
  thread in that machine's project (`%machine:name` by id, name or slug), by the same route: the owner's
  `thread_handoff` queues it ahead of the next message, exactly as a local /give does (`Fm.hand`). The
  person hears ok only once every machine took it, or which did not (`Far.give` in the server); words
  naming something here run as before, read without the machine words (`Fm.give.local`). A thread that
  is itself elsewhere is given from its own machine.
- Sender ids: a message sent by a thread on another machine carries its sender in its id
  (`t:<from>:<n>`); a mirror names it as its own machine does (`Fr.sent`), so the "from" row opens the
  right thread both ways.
- Bots: `@name@machine` already reaches a bot on a linked machine (`Bots.mention.far`); rooms travel
  with the owner's machines (see "Shared rooms").

Laws `fmc_*`; tests `test/fmcp_test.bend`, `bun test/tools/agents_far_e2e.ts`.
