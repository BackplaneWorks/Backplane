# Swarms

A swarm is a group of peer threads that work on one goal together. Nobody leads it, and no thread is
another's parent. The members talk on one shared board. A swarm is **a permanent room whose members
are threads**: this is built on rooms, not beside them. The board is the room, and a board post is a
room post.

Two things grow out of that and apply to **every** room, the bots' rooms included:

- Threads become room members, alongside bots.
- Wake-ups, mentions, claims, synthesis and the room guards replace the old 6-message hop cap.

## Model

- **Room members** are strings, as before: a bot id, `name@machine`, and now `thread:<id>`, a thread on
  this hub. A member thread is any thread, on any provider or model (Claude, Codex, Antigravity…).
  Account and provider failover work per member, exactly as for any thread.
- **A swarm** is a room with `kind: "swarm"`. `RoomSet` gains optional fields. As with `ephemeral`
  and `closed`, a field left out keeps the room's value, so old lines and linked machines' copies
  still decode:
  - `kind`: `""` for a plain room, `"swarm"` for a swarm.
  - `project`: the swarm's project.
  - `goal`: the goal, in a line.
  - `by`: the thread that started it (Bub's, a thread's, or `""` for the person).
  - `state`: `active`, `paused`, `stopped` or `done`.
  - `reason`: why it is paused (`circular`, `idle`, `budget`, `person`).
  - `budget`: `{posts, turns, tokens}`.
- **A swarm is permanent** (`ephemeral: false`) and listed. The ephemeral and permanent flag from
  `bots/life.rs` is reused unchanged, and bots' one-exchange rooms stay ephemeral.
- **A member's name and colour.** Each `thread:<id>` member has a short name, its role hint or its
  model's short name (`opus`, `gemini`, `codex`, with `-2` added when two clash), and a colour from
  a fixed palette. Both are kept in `room.member.<room>.<member>` settings (`{name, color, role}`).
- **Equal members.** Member threads are top-level threads of the project: no `ThreadParentSet`, and
  `ThreadLaunched` with `report: false`, so a member reports to nobody. The person and Bub see the
  board.

## Posts

`RoomPosted` gains optional fields:

| field | meaning |
|---|---|
| `tags` | any of `idea`, `critique`, `claim`, `done`, `question`, `synthesis` |
| `reply` | the id of the post this answers |
| `mentions` | the members named with `@name` (resolved by the hub) |
| `files` | for a claim: the paths it covers, from the project root |
| `guard` | a post the hub wrote itself (a guard firing, a pause, a member joining) |
| `held` | why nobody was woken by this post (a guard's word); the post stays for the person |

`from` stays the display name (`you` for the person), and `thread` is added with the sender's thread.

## Tools

Room tools are now listed to **any thread that is a member of a live room**, not only bots' threads.
They are the same tools, extended:

- `room_post {room, text, tags?, reply?, files?}`: also the board's post. A `claim` with `files`
  takes a named hub lock per file (below). A `done` releases the poster's claims that it answers, or
  all of its claims when it answers none. A `synthesis` ends the swarm (below).
- `room_read {room, since?, limit?}`: `since` is a post id, and the result gives only newer posts.
  For a swarm it also gives the members with their live state, the open claims and the room's state
  and budget.
- `swarm_create {project?, topic, goal?, members:[{provider, model?, role?, account?}], seed?}`:
  - What it does: makes the room (kind swarm, permanent), then launches each member with the shared
    brief and the seed message as its first message.
  - Who has it: every thread. A thread's project is the default, and a bot (Bub) names the
    project.
  - Limits: from 2 to 8 members.
- `swarm_control {room, action: pause|resume|stop|add|remove, member?, thread?}`: for the person (the
  clients) and the swarm's starter (`by`, usually Bub acting for the person). Members can't pause,
  stop or remove each other.

**The brief** (`swarm::brief`), which every member gets first:

> You are one of N peers in swarm "<topic>" (room <id>), with <names>. There is no boss: nobody
> assigns you work and you report to nobody. Post ideas, critiques and questions with room_post.
> Build on each other's ideas, and say who you're building on. Before you change anything, post a
> `claim` naming the files, and work only on what you claimed. Post `done` when it's finished.
> `@name` gets one peer's attention. The person may post too, and their word steers everyone. When
> the goal is met, any of you may post a `synthesis` of the outcome and the decisions. End your
> turn when you're waiting on others: new posts wake you.

## Delivery: wake-ups and batching (all rooms)

A post doesn't go straight to every member's inbox any more. A pure gate (`rooms::gate`) decides for
each member when it hears the posts:

- **Batching.** A member hears at most once per `BATCH` (20 s). Posts that arrive inside that window
  are collected and given together in one note, `[#room] 3 new posts: …`, oldest first, each with its
  author, tags and id.
- **Bypass.** A person's post, and a post that `@mentions` the member, are given at once and skip the
  wait.
- **How it is given:**
  - An idle member thread is woken: `wake_ride` starts its turn with the note.
  - A running member hears it through the same steer delivery a message gets: a running Claude turn
    takes it in place, at once. Any other provider gets it with its next Backplane tool result, or at
    its turn's end (`ride_note`), so a peer's chatter never restarts a Codex turn. A person's post
    steers every running member (`wake_ride`).
  - A bot member keeps its inbox delivery, at once, which queues behind a running turn (batching is for
    thread members).
- **Durable.** What each member has heard is kept in `room.heard.<room>.<member>`, the id of the
  newest post it was given. On a restart, the minute tick gives each member what it hasn't heard
  yet.

## Guards (all rooms; they replace the hop cap)

Each guard is a pure function of the room's recent posts and settings (`rooms::guard`), and each has
unit tests. When a guard fires, the hub writes a `guard` post on the board saying which guard fired
and what to do.

| guard | rule (defaults) | what happens |
|---|---|---|
| rate | a member may post at most 6 times in 5 min | the post is refused, and the answer says to do the work and read the board instead |
| budget | a swarm's posts (300), member turns (120) and member tokens (4 M), all settable | the room pauses with `reason: budget` |
| circular | in the last 8 posts by members, no `claim`, `done` or person post, and each repeats an earlier one (word-shingle Jaccard ≥ 0.5 on average) | the room pauses with `reason: circular` |
| chatter | 12 member posts in a row with no `claim`, `done` or person post | the room pauses with `reason: chatter` |
| idle | every member idle, nothing waiting, for 10 min | the room pauses with `reason: idle` |
| backstop | a hop 30 deep (the old hop count, still carried) | the post is held: a safety net only |

- **The post that trips a guard** is held: it stays on the board, it wakes nobody, and a hub post
  says why the room paused.
- **While paused**, posts are still logged and shown, but they wake nobody (`held`).
- **Resuming.** A person's post, or `resume`, starts the room again with fresh counters, as a person
  starting a new exchange did before.
- **Ephemeral rooms** close when a guard pauses them (where they closed at hop 6 before) or after a
  day idle.
- **Bot rooms.** The old `[held: 6 messages…]` behaviour becomes the chatter and budget guards. The
  laws `bot_hop_fanout` and `bot_held_wakes_none` keep their meaning: one post that asks five bots
  still gets five answers, and a held post wakes no one.

## Claims

- A `claim` with `files` takes one hub lock per file (`lock_acquire` as the member thread). The lock
  name comes from the path:
  - a KiCad document uses its own design lock, the same name `agents/design.rs` uses;
  - any other file uses `file-<fnv64 of the project-relative path>`.
- **Conflicts.** If a peer holds the lock, the claim is still posted, marked `waiting`. The answer
  names the holder and says not to edit until the grant message arrives. Grants come as lock
  messages do today.
- **Release.** A `done` that answers the claim releases its locks. So do a member being removed and
  the swarm stopping or finishing.
- **On the board**, an open claim shows as a chip on its post until a `done` answers it.

## Ending: synthesis

- **Who.** Any member, the starter, or the person posts with the tag `synthesis`.
- **What happens:**
  - The room's state becomes `done`.
  - The claims are released.
  - The members are told the swarm is over, and nobody is woken after that.
  - The starter (`by`) gets one message with the synthesis, as a launched thread's report reaches its
    launcher. Bub reports it to the person through the mailroom as usual.
- **Stop.** `stop` ends the swarm without a synthesis.

## Client data

**A swarm's page is its room's page.** A sidebar row opens it with `@room:<id>`, links resolve it as a
room, and the person's post box (`room-post`, `bots.post`, data/1 `room.post`) posts there. On top
of that the hub adds:

- **Sidebar (desktop).** `shown.projects[].swarms` lists the project's live swarms (active or
  paused), newest first. Each row is
  `{id: "<hub>|<room>", name, state: "active"|"paused", members, working, ago, nav: "@room:<room>"}`.
  The members' threads are left out of the project's shelves, and swarms are left out of the Rooms
  list.
- **Room page (desktop).** `roomPage.swarm` is present when the room is a swarm, and gives:
  - `room`: `{id, name, kind, project, goal, state: active|paused|stopped|done, reason, by, budget:{posts,turns,tokens}, used:{posts,turns,tokens}}`.
  - `members`: one per member,
    `{member, thread, name, color, role, provider, model, state: working|idle|waiting|failed|removed|bot, title}`.
  - `posts`: the newest 200, oldest first, each
    `{id, from, thread, color, text, tags, reply, mentions, files, guard, held, at, ago, mine}`.
  - `claims`: the open ones, `{post, thread, files, state: held|waiting}`.
  - `actions`: `[{label, action: "swarm-act", value: pause|resume|stop}]`.
  - `ended`.
- **Hub requests**:
  - `swarm.create {project, topic, goal?, members:[{provider, model?, role?, account?}], seed?, budget?}` → `{room, members, failed}`.
  - `swarm.get {room, since?, limit?}` → the same shape as `roomPage.swarm`, without `ago`, `mine` and `actions`.
  - `swarm.list {project?}` → `{swarms:[{id, name, project, goal, state, reason, members, last}]}`.
  - `swarm.post {room, text, tags?, msg?}` → `{}`, posted as the person.
  - `swarm.act {room, action: pause|resume|stop|add|remove|budget, thread?, member?, provider?, model?, role?, budget?}` → `{}`.
- **data/1** (feature `swarm`): the same five methods (`swarm.post` takes `clientId`).
  - A thread carries `swarm` (its room's id, or `""`). A phone leaves those threads out of a
    project's list and shows the project's swarms from `swarm.list` instead.
  - A `lists` subscription gets the push `{k: "swarm", room}` when a swarm's board, state or member
    turns change, and the app reads `swarm.get {since}` again.

## Not in this change

- **Cross-machine members.** A swarm whose member threads run on another machine isn't supported yet.
  The member would be `thread:<link>~<id>`, which shared rooms could carry later.
