import Base

# Bots
# ----
# A bot is a persistent agent with a name and a face (a cat, cat.bend). Its
# turns run on one thread of its own; everything else a bot has (rooms,
# webhooks, routines, memory, its space, linked machines) is this world,
# folded from the log like the rest of the state. See docs/bots.md.

type Bot is Data:
  Bot{id: String, name: String, thread: String, look: Nat, persona: String, created: Nat, deleted: Bool}

# a room: its members are bot ids here, or "name@peer" for a bot elsewhere
type Room is Data:
  Room{id: String, name: String, members: List<&2, String>, created: Nat, deleted: Bool}

# one message in a room (direct messages are rooms "dm:<a>:<b>")
type Post is Data:
  Post{id: String, room: String, from: String, text: String, hop: Nat, at: Nat}

type Hook is Data:
  Hook{id: String, bot: String, name: String, created: Nat, revoked: Bool, last: Nat, count: Nat}

type Routine is Data:
  Routine{id: String, bot: String, name: String, cron: String, prompt: String, on: Bool, last: Nat, created: Nat, deleted: Bool, once: Nat, tz: String, kind: String, done: Bool, slot: Nat}

type Mem is Data:
  Mem{bot: String, key: String, kind: String, text: String, tags: String, created: Nat, updated: Nat}

type Peer is Data:
  Peer{id: String, name: String, url: String, created: Nat, revoked: Bool}

# a bot's space as a web page (Space.page), and when it was written
type Page is Data:
  Page{html: String, at: Nat}

# a bot's conversation depth: the hop of the turn it runs (or ran last),
# and, when `held`, the lowest hop of the messages waiting for its next
# turn (Hop.woke, Hop.taken)
type Hops is Data:
  Hops{now: Nat, wait: Nat, held: Bool}

type World is Data:
  World{
    bots: Map<&2, Bot>,
    rooms: Map<&2, Room>,
    posts: List<&2, Post>,
    hooks: Map<&2, Hook>,
    routines: Map<&2, Routine>,
    mems: Map<&2, Mem>,
    peers: Map<&2, Peer>,
    spaces: Map<&2, String>,
    hops: Map<&2, Hops>,
    talk: Map<&2, Nat>,
    pages: Map<&2, Page>,
    tasks: Map<&2, String>
  }

# the value half of a Map.get
def Got(-V: Data, r: Map<&2, V> & V) -> V:
  (_, v) = r
  v

def World.empty() -> World:
  World{Map.new(&2, Bot), Map.new(&2, Room), Nil{}, Map.new(&2, Hook), Map.new(&2, Routine), Map.new(&2, Mem), Map.new(&2, Peer), Map.new(&2, String), Map.new(&2, Hops), Map.new(&2, Nat), Map.new(&2, Page), Map.new(&2, String)}

def Bot.blank(id: String) -> Bot:
  Bot{id, "", "", 0n, "", 0n, True{}}

def Room.blank(id: String) -> Room:
  Room{id, "", Nil{}, 0n, True{}}

def Hook.blank(id: String) -> Hook:
  Hook{id, "", "", 0n, True{}, 0n, 0n}

def Routine.blank(id: String) -> Routine:
  Routine{id, "", "", "", "", False{}, 0n, 0n, True{}, 0n, "", "", False{}, 0n}

def Peer.blank(id: String) -> Peer:
  Peer{id, "", "", 0n, True{}}

# Folding the log
# ---------------

def World.bots.set(w: World, +id: String, b: Bot) -> World:
  match w:
    case World{bs, rs, ps, hs, ro, ms, pe, sp, ho, ta, pg, tk}:
      World{Map.set(&2, Bot, bs, id, b), rs, ps, hs, ro, ms, pe, sp, ho, ta, pg, tk}

def World.bot(+w: World, +id: String) -> Bot:
  match w:
    case World{bs, _, _, _, _, _, _, _, _, _, _, _}:
      Got(Bot, Map.get(Bot, Bot.blank(id), bs, id))

def Bot.keep.k(+was: String, +now: String) -> String:
  Bool.pick(String, String.is_empty(now), was, now)

def World.bot_set.k(w: World, +id: String, name: String, thread: String, look: Nat, persona: String, +at: Nat, old: Bot) -> World:
  match old:
    case Bot{_, _, +th, _, _, +created, +deleted}:
      World.bots.set(w, id, Bot{id, name, Bot.keep.k(th, thread), look, persona, Bool.pick(Nat, deleted, at, created), False{}})

# made, or edited: an empty thread keeps the one it has
def World.bot_set(+w: World, +id: String, name: String, thread: String, look: Nat, persona: String, +at: Nat) -> World:
  World.bot_set.k(w, id, name, thread, look, persona, at, World.bot(w, id))

def World.bot_deleted.k(w: World, +id: String, b: Bot) -> World:
  match b:
    case Bot{i, n, th, lk, pe, c, _}:
      World.bots.set(w, id, Bot{i, n, th, lk, pe, c, True{}})

def World.bot_deleted(+w: World, +id: String) -> World:
  World.bot_deleted.k(w, id, World.bot(w, id))

def World.room(+w: World, +id: String) -> Room:
  match w:
    case World{_, rs, _, _, _, _, _, _, _, _, _, _}:
      Got(Room, Map.get(Room, Room.blank(id), rs, id))

def World.rooms.set(w: World, +id: String, r: Room) -> World:
  match w:
    case World{bs, rs, ps, hs, ro, ms, pe, sp, ho, ta, pg, tk}:
      World{bs, Map.set(&2, Room, rs, id, r), ps, hs, ro, ms, pe, sp, ho, ta, pg, tk}

# members travel as one line each
def Room.members.of(s: String) -> List<&2, String>:
  List.filter(~String, ~(x => Bool.not(String.is_empty(x))), String.split(s, '\n'))

def Room.members.show(ms: List<&2, String>) -> String:
  String.join(ms, "\n")

def World.room_set.k(w: World, +id: String, name: String, members: String, +at: Nat, old: Room) -> World:
  match old:
    case Room{_, _, _, +created, +deleted}:
      World.rooms.set(w, id, Room{id, name, Room.members.of(members), Bool.pick(Nat, deleted, at, created), False{}})

def World.room_set(+w: World, +id: String, name: String, members: String, +at: Nat) -> World:
  World.room_set.k(w, id, name, members, at, World.room(w, id))

def World.room_deleted.k(w: World, +id: String, r: Room) -> World:
  match r:
    case Room{i, n, m, c, _}:
      World.rooms.set(w, id, Room{i, n, m, c, True{}})

def World.room_deleted(+w: World, +id: String) -> World:
  World.room_deleted.k(w, id, World.room(w, id))

def World.talked(ta: Map<&2, Nat>, +from: String, +at: Nat) -> Map<&2, Nat>:
  Map.set(&2, Nat, ta, from, at)

def World.posted(w: World, p: Post) -> World:
  match w p:
    case World{bs, rs, ps, hs, ro, ms, pe, sp, ho, ta, pg, tk} Post{i, r, +f, t, h, +at}:
      World{bs, rs, Post{i, r, f, t, h, at} <> ps, hs, ro, ms, pe, sp, ho, World.talked(ta, f, at), pg, tk}

# whether a post with this id is already in: a walk from the newest that
# stops at the first hit (a message sent again after a dropped link)
def Post.has.go(ps: List<&2, Post>, +id: String, found: Bool) -> Bool:
  match ps found:
    case _ True{}:
      True{}
    case Nil{} False{}:
      False{}
    case Post{+i, _, _, _, _, _} <> t False{}:
      Post.has.go(t, id, String.eq(i, id))

def World.has_post(w: World, +id: String) -> Bool:
  match w:
    case World{_, _, ps, _, _, _, _, _, _, _, _, _}:
      Post.has.go(ps, id, False{})

def World.hook(+w: World, +id: String) -> Hook:
  match w:
    case World{_, _, _, hs, _, _, _, _, _, _, _, _}:
      Got(Hook, Map.get(Hook, Hook.blank(id), hs, id))

def World.hooks.set(w: World, +id: String, h: Hook) -> World:
  match w:
    case World{bs, rs, ps, hs, ro, ms, pe, sp, ho, ta, pg, tk}:
      World{bs, rs, ps, Map.set(&2, Hook, hs, id, h), ro, ms, pe, sp, ho, ta, pg, tk}

def World.hook_set.k(w: World, +id: String, bot: String, name: String, revoked: Bool, +at: Nat, old: Hook) -> World:
  match old:
    case Hook{_, +ob, +on, +created, was, +last, +count}:
      World.hooks.set(w, id, Hook{id, Bot.keep.k(ob, bot), Bot.keep.k(on, name), Bool.pick(Nat, was, at, created), revoked, last, count})

def World.hook_set(+w: World, +id: String, bot: String, name: String, revoked: Bool, +at: Nat) -> World:
  World.hook_set.k(w, id, bot, name, revoked, at, World.hook(w, id))

def World.hook_hit.k(w: World, +id: String, +at: Nat, h: Hook) -> World:
  match h:
    case Hook{i, b, n, c, r, _, count}:
      World.hooks.set(w, id, Hook{i, b, n, c, r, at, 1n+count})

def World.hook_hit(+w: World, +id: String, +at: Nat) -> World:
  World.hook_hit.k(w, id, at, World.hook(w, id))

def World.routine(+w: World, +id: String) -> Routine:
  match w:
    case World{_, _, _, _, ro, _, _, _, _, _, _, _}:
      Got(Routine, Map.get(Routine, Routine.blank(id), ro, id))

def World.routines.set(w: World, +id: String, r: Routine) -> World:
  match w:
    case World{bs, rs, ps, hs, ro, ms, pe, sp, ho, ta, pg, tk}:
      World{bs, rs, ps, hs, Map.set(&2, Routine, ro, id, r), ms, pe, sp, ho, ta, pg, tk}

def Routine.clock(was_on: Bool, on: Bool, +last: Nat, +at: Nat) -> Nat:
  match was_on on:
    case False{} True{}:
      at
    case _ _:
      last

# a routine's slot (the zone's wall clock at its last fire) restarts with its clock
def Routine.slot.keep(was_on: Bool, on: Bool, +slot: Nat) -> Nat:
  match was_on on:
    case False{} True{}:
      0n
    case _ _:
      slot

def World.routine_set.k(w: World, +id: String, bot: String, name: String, cron: String, prompt: String, +on: Bool, +at: Nat, old: Routine) -> World:
  match old:
    case Routine{_, +ob, _, _, _, +was_on, +last, +created, +deleted, +once, tz, kind, done, +slot}:
      World.routines.set(w, id, Routine{id, Bot.keep.k(ob, bot), name, cron, prompt, on, Routine.clock(Bool.and(was_on, Bool.not(deleted)), on, last, at), Bool.pick(Nat, deleted, at, created), False{}, once, tz, kind, done, Routine.slot.keep(Bool.and(was_on, Bool.not(deleted)), on, slot)})

# made or edited; a routine's clock starts when it is made or turned on, so
# a new routine never fires for times before it existed
def World.routine_set(+w: World, +id: String, bot: String, name: String, cron: String, prompt: String, on: Bool, +at: Nat) -> World:
  World.routine_set.k(w, id, bot, name, cron, prompt, on, at, World.routine(w, id))

def World.routine_deleted.k(w: World, +id: String, r: Routine) -> World:
  match r:
    case Routine{i, b, n, c, p, o, l, cr, _, on1, tz, kd, dn, sl}:
      World.routines.set(w, id, Routine{i, b, n, c, p, o, l, cr, True{}, on1, tz, kd, dn, sl})

def World.routine_deleted(+w: World, +id: String) -> World:
  World.routine_deleted.k(w, id, World.routine(w, id))

def World.routine_fired.k(w: World, +id: String, +at: Nat, r: Routine) -> World:
  match r:
    case Routine{i, b, n, c, p, o, _, cr, d, on1, tz, kd, dn, sl}:
      World.routines.set(w, id, Routine{i, b, n, c, p, o, at, cr, d, on1, tz, kd, dn, sl})

def World.routine_fired(+w: World, +id: String, +at: Nat) -> World:
  World.routine_fired.k(w, id, at, World.routine(w, id))

# RoutinePlan: when it fires once (0: repeats), in which zone, what it does;
# a plan makes a routine pending again
def World.routine_plan.k(w: World, +id: String, +once: Nat, tz: String, kind: String, r: Routine) -> World:
  match r:
    case Routine{i, b, n, c, p, o, l, cr, d, _, _, _, _, _}:
      World.routines.set(w, id, Routine{i, b, n, c, p, o, l, cr, d, once, tz, kind, False{}, 0n})

def World.routine_plan(+w: World, +id: String, +once: Nat, tz: String, kind: String) -> World:
  World.routine_plan.k(w, id, once, tz, kind, World.routine(w, id))

def World.routine_slot.k(w: World, +id: String, +slot: Nat, r: Routine) -> World:
  match r:
    case Routine{i, b, n, c, p, o, l, cr, d, on1, tz, kd, dn, _}:
      World.routines.set(w, id, Routine{i, b, n, c, p, o, l, cr, d, on1, tz, kd, dn, slot})

def World.routine_slot(+w: World, +id: String, +slot: Nat) -> World:
  World.routine_slot.k(w, id, slot, World.routine(w, id))

def World.routine_done.k(w: World, +id: String, r: Routine) -> World:
  match r:
    case Routine{i, b, n, c, p, o, l, cr, d, on1, tz, kd, _, sl}:
      World.routines.set(w, id, Routine{i, b, n, c, p, o, l, cr, d, on1, tz, kd, True{}, sl})

def World.routine_done(+w: World, +id: String) -> World:
  World.routine_done.k(w, id, World.routine(w, id))

def Mem.at(+bot: String, +key: String) -> String:
  bot ++ "/" ++ key

def Mem.blank(bot: String, key: String) -> Mem:
  Mem{bot, key, "", "", "", 0n, 0n}

def World.mem(+w: World, +bot: String, +key: String) -> Mem:
  match w:
    case World{_, _, _, _, _, ms, _, _, _, _, _, _}:
      Got(Mem, Map.get(Mem, Mem.blank(bot, key), ms, Mem.at(bot, key)))

def World.mems.put(w: World, +k: String, m: Mem) -> World:
  match w:
    case World{bs, rs, ps, hs, ro, ms, pe, sp, ho, ta, pg, tk}:
      World{bs, rs, ps, hs, ro, Map.set(&2, Mem, ms, k, m), pe, sp, ho, ta, pg, tk}

def World.mems.del(w: World, +k: String) -> World:
  match w:
    case World{bs, rs, ps, hs, ro, ms, pe, sp, ho, ta, pg, tk}:
      World{bs, rs, ps, hs, ro, Map.del(&2, Mem, ms, k), pe, sp, ho, ta, pg, tk}

def World.mem_saved.k(w: World, +bot: String, +key: String, kind: String, text: String, tags: String, +at: Nat, old: Mem) -> World:
  match old:
    case Mem{_, _, _, _, _, +created, _}:
      World.mems.put(w, Mem.at(bot, key), Mem{bot, key, kind, text, tags, Bool.pick(Nat, Nat.is_eq(created, 0n), at, created), at})

# saving a key replaces what was there; its first save time stays
def World.mem_saved(+w: World, +bot: String, +key: String, kind: String, text: String, tags: String, +at: Nat) -> World:
  World.mem_saved.k(w, bot, key, kind, text, tags, at, World.mem(w, bot, key))

def World.mem_forgot(w: World, +bot: String, +key: String) -> World:
  World.mems.del(w, Mem.at(bot, key))

def World.space_set(w: World, +bot: String, spec: String) -> World:
  match w:
    case World{bs, rs, ps, hs, ro, ms, pe, sp, ho, ta, pg, tk}:
      World{bs, rs, ps, hs, ro, ms, pe, Map.set(&2, String, sp, bot, spec), ho, ta, pg, tk}

def World.spage_set(w: World, +bot: String, html: String, +at: Nat) -> World:
  match w:
    case World{bs, rs, ps, hs, ro, ms, pe, sp, ho, ta, pg, tk}:
      World{bs, rs, ps, hs, ro, ms, pe, sp, ho, ta, Map.set(&2, Page, pg, bot, Page{html, at}), tk}

# Browser task skills (task.bend): a bot's tasks as JSON text, keyed
# "<bot>/<slug>"; TaskSet replaces one, TaskDeleted drops it
type TRow is Data:
  TRow{slug: String, json: String}

def Task.at(+bot: String, +slug: String) -> String:
  bot ++ "/" ++ slug

def World.task_set(w: World, +bot: String, +slug: String, json: String) -> World:
  match w:
    case World{bs, rs, ps, hs, ro, ms, pe, sp, ho, ta, pg, tk}:
      World{bs, rs, ps, hs, ro, ms, pe, sp, ho, ta, pg, Map.set(&2, String, tk, Task.at(bot, slug), json)}

def World.task_del(w: World, +bot: String, +slug: String) -> World:
  match w:
    case World{bs, rs, ps, hs, ro, ms, pe, sp, ho, ta, pg, tk}:
      World{bs, rs, ps, hs, ro, ms, pe, sp, ho, ta, pg, Map.del(&2, String, tk, Task.at(bot, slug))}

def World.task.get(w: World, +bot: String, +slug: String) -> String:
  match w:
    case World{_, _, _, _, _, _, _, _, _, _, _, tk}:
      Got(String, Map.get(String, "", tk, Task.at(bot, slug)))

def World.task.some(json: String) -> Maybe<&2, String>:
  match json:
    case SNil{}:
      None{}
    case _:
      Some{json}

# one task's JSON, if the bot has it
def World.task(+w: World, +bot: String, +slug: String) -> Maybe<&2, String>:
  World.task.some(World.task.get(w, bot, slug))

def World.tasks_of.put(hit: Bool, k: String, v: String, rest: List<&2, TRow>, +pre: String) -> List<&2, TRow>:
  match hit:
    case True{}:
      TRow{String.drop(k, String.length(pre)), v} <> rest
    case False{}:
      rest

def World.tasks_of.go(kvs: List<&2, Sigma<&2, &2, String, _ => String>>, +pre: String) -> List<&2, TRow>:
  match kvs:
    case Nil{}:
      Nil{}
    case (+k, v) <> t:
      World.tasks_of.put(String.starts_with(k, pre), k, v, World.tasks_of.go(t, pre), pre)

# a bot's tasks, (slug, json), by slug
def World.tasks_of(+w: World, +bot: String) -> List<&2, TRow>:
  match w:
    case World{_, _, _, _, _, _, _, _, _, _, _, tk}:
      World.tasks_of.go(Map.to_list(&2, String, tk), bot ++ "/")

def World.peer(+w: World, +id: String) -> Peer:
  match w:
    case World{_, _, _, _, _, _, pe, _, _, _, _, _}:
      Got(Peer, Map.get(Peer, Peer.blank(id), pe, id))

def World.peers.set(w: World, +id: String, p: Peer) -> World:
  match w:
    case World{bs, rs, ps, hs, ro, ms, pe, sp, ho, ta, pg, tk}:
      World{bs, rs, ps, hs, ro, ms, Map.set(&2, Peer, pe, id, p), sp, ho, ta, pg, tk}

def World.peer_set.k(w: World, +id: String, name: String, url: String, revoked: Bool, +at: Nat, old: Peer) -> World:
  match old:
    case Peer{_, +on, +ou, +created, was}:
      World.peers.set(w, id, Peer{id, Bot.keep.k(on, name), Bot.keep.k(ou, url), Bool.pick(Nat, was, at, created), revoked})

def World.peer_set(+w: World, +id: String, name: String, url: String, revoked: Bool, +at: Nat) -> World:
  World.peer_set.k(w, id, name, url, revoked, at, World.peer(w, id))

# Conversation depth
# ------------------
# A human, a routine, a webhook or a space button wakes a bot at hop 0; a
# bot's message goes out one hop past the turn that sent it (Hop.send
# below). A turn's hop is the lowest hop of the messages it answers: one
# a bot at rest hears starts a turn at its hop, and one a busy bot hears
# waits for its next turn, which runs at the lowest hop waiting. So when
# N bots answer one post, a sibling's answer arriving mid-turn never makes
# a bot's own answer deeper (law bot_hop_fanout), while each message of a
# back-and-forth is answered one hop deeper (bot_hop_queue_takes).

def Hops.now(c: Hops) -> Nat:
  match c:
    case Hops{n, _, _}:
      n

def Hop.low(+a: Nat, +b: Nat) -> Nat:
  Bool.pick(Nat, Nat.is_le(a, b), a, b)

def Hop.wait(held: Bool, +w: Nat, +h: Nat) -> Nat:
  match held:
    case True{}:
      Hop.low(w, h)
    case False{}:
      h

# a message at hop h reaches a bot: at rest, it starts a turn at h; busy,
# its turn keeps its hop and the message waits for the next turn
def Hop.woke(busy: Bool, c: Hops, +h: Nat) -> Hops:
  match busy c:
    case True{} Hops{n, w, held}:
      Hops{n, Hop.wait(held, w, h), True{}}
    case False{} _:
      Hops{h, 0n, False{}}

def Hop.taken.k(held: Bool, n: Nat, w: Nat) -> Hops:
  match held:
    case True{}:
      Hops{w, 0n, False{}}
    case False{}:
      Hops{n, 0n, False{}}

# the next turn takes the messages that waited: it runs at the lowest hop
def Hop.taken(c: Hops) -> Hops:
  match c:
    case Hops{n, w, held}:
      Hop.taken.k(held, n, w)

# a busy bot hears messages at the hops hs (its siblings' answers)
def Hop.wakes(hs: List<&2, Nat>, c: Hops) -> Hops:
  match hs:
    case Nil{}:
      c
    case +h <> t:
      Hop.wakes(t, Hop.woke(True{}, c, h))

def World.woke(w: World, +bot: String, busy: Bool, +hop: Nat) -> World:
  match w:
    case World{bs, rs, ps, hs, ro, ms, pe, sp, +ho, ta, pg, tk}:
      World{bs, rs, ps, hs, ro, ms, pe, sp, Map.set(&2, Hops, ho, bot, Hop.woke(busy, Got(Hops, Map.get(Hops, Hops{0n, 0n, False{}}, ho, bot)), hop)), ta, pg, tk}

def World.taken.id(w: World, +bot: String) -> World:
  match w:
    case World{bs, rs, ps, hs, ro, ms, pe, sp, +ho, ta, pg, tk}:
      World{bs, rs, ps, hs, ro, ms, pe, sp, Map.set(&2, Hops, ho, bot, Hop.taken(Got(Hops, Map.get(Hops, Hops{0n, 0n, False{}}, ho, bot)))), ta, pg, tk}

# Reading
# -------

def Bot.id(b: Bot) -> String:
  match b:
    case Bot{i, _, _, _, _, _, _}:
      i

def Bot.name(b: Bot) -> String:
  match b:
    case Bot{_, n, _, _, _, _, _}:
      n

def Bot.thread(b: Bot) -> String:
  match b:
    case Bot{_, _, t, _, _, _, _}:
      t

def Bot.look(b: Bot) -> Nat:
  match b:
    case Bot{_, _, _, l, _, _, _}:
      l

def Bot.persona(b: Bot) -> String:
  match b:
    case Bot{_, _, _, _, p, _, _}:
      p

def Bot.live(b: Bot) -> Bool:
  match b:
    case Bot{_, _, _, _, _, _, d}:
      Bool.not(d)

def Bot.before(a: Bot, b: Bot) -> Bool:
  match a b:
    case Bot{_, _, _, _, _, ca, _} Bot{_, _, _, _, _, cb, _}:
      Nat.is_le(ca, cb)

# the live bots, oldest first
def World.bots(+w: World) -> List<&2, Bot>:
  match w:
    case World{bs, _, _, _, _, _, _, _, _, _, _, _}:
      List.sort(~Bot, ~Bot.before, List.filter(~Bot, ~Bot.live, Map.values(&2, Bot, bs)))

def Bot.here(b: Bot) -> Bool:
  match b:
    case Bot{+i, _, _, _, _, _, d}:
      Bool.and(Bool.not(d), Bool.not(String.contains(i, "~")))

# this machine's own live bots, oldest first: not those a client mirrors
# from a linked machine (far.bend names them "<link>~<id>"), which rooms,
# mentions and pickers reach as name@machine instead
def World.bots.here(+w: World) -> List<&2, Bot>:
  match w:
    case World{bs, _, _, _, _, _, _, _, _, _, _, _}:
      List.sort(~Bot, ~Bot.before, List.filter(~Bot, ~Bot.here, Map.values(&2, Bot, bs)))

def Bot.named.k(+b: Bot, +name: String) -> Bool:
  Bool.and(Bot.live(b), String.eq(String.to_lower(Bot.name(b)), String.to_lower(name)))

# a bot by name (any case), or by id
def Pick.first(-A: Data, hit: Bool, x: A, rest: Maybe<&2, A>) -> Maybe<&2, A>:
  match hit:
    case True{}:
      Some{x}
    case False{}:
      rest

def Bot.find.name(bs: List<&2, Bot>, +name: String) -> Maybe<&2, Bot>:
  match bs:
    case Nil{}:
      None{}
    case +b <> t:
      Pick.first(Bot, Bot.named.k(b, name), b, Bot.find.name(t, name))

def World.find.id(b: Bot) -> Maybe<&2, Bot>:
  match b:
    case Bot{i, n, t, l, p, c, +d}:
      Bool.pick(Maybe<&2, Bot>, d, None{}, Some{Bot{i, n, t, l, p, c, d}})

def World.find.k(m: Maybe<&2, Bot>, +w: World, +name: String) -> Maybe<&2, Bot>:
  match m:
    case Some{b}:
      Some{b}
    case None{}:
      World.find.id(World.bot(w, name))

def World.find(+w: World, +name: String) -> Maybe<&2, Bot>:
  World.find.k(Bot.find.name(World.bots(w), name), w, name)

def Bot.thread.is(+b: Bot, +thread: String) -> Bool:
  Bool.and(Bot.live(b), String.eq(Bot.thread(b), thread))

def Bot.of.thread.go(bs: List<&2, Bot>, +thread: String) -> Maybe<&2, Bot>:
  match bs:
    case Nil{}:
      None{}
    case +b <> t:
      Pick.first(Bot, Bot.thread.is(b, thread), b, Bot.of.thread.go(t, thread))

# the bot a thread belongs to, if any
def World.of_thread(+w: World, +thread: String) -> Maybe<&2, Bot>:
  Bot.of.thread.go(World.bots(w), thread)

def World.taken.k(m: Maybe<&2, Bot>, w: World) -> World:
  match m:
    case Some{b}:
      World.taken.id(w, Bot.id(b))
    case None{}:
      w

# a bot's thread takes its waiting messages into a new turn
def World.taken(+w: World, +thread: String) -> World:
  World.taken.k(World.of_thread(w, thread), w)

def World.page.k(m: Maybe<&2, Bot>, +thread: String) -> String:
  match m:
    case Some{b}:
      Bot.id(b)
    case None{}:
      thread

# the browser page a thread's agent drives: a bot's is named by the bot,
# any other thread's by the thread, so no two threads share a page
def World.page(+w: World, +thread: String) -> String:
  World.page.k(World.of_thread(w, thread), thread)

def Maybe.is(-A: Data, m: Maybe<&2, A>) -> Bool:
  match m:
    case Some{_}:
      True{}
    case None{}:
      False{}

def World.is_bot_thread(+w: World, +thread: String) -> Bool:
  Maybe.is(Bot, World.of_thread(w, thread))

def Room.live(r: Room) -> Bool:
  match r:
    case Room{_, _, _, _, d}:
      Bool.not(d)

def Room.id(r: Room) -> String:
  match r:
    case Room{i, _, _, _, _}:
      i

def Room.name(r: Room) -> String:
  match r:
    case Room{_, n, _, _, _}:
      n

def Room.members(r: Room) -> List<&2, String>:
  match r:
    case Room{_, _, m, _, _}:
      m

def Room.is_dm(r: Room) -> Bool:
  String.starts_with(Room.id(r), "dm:")

# the live group rooms (not direct messages)
def World.rooms(+w: World) -> List<&2, Room>:
  match w:
    case World{_, rs, _, _, _, _, _, _, _, _, _, _}:
      List.filter(~Room, ~(r => Bool.and(Room.live(r), Bool.not(Room.is_dm(r)))), Map.values(&2, Room, rs))

def Room.has(ms: List<&2, String>, +x: String) -> Bool:
  match ms:
    case Nil{}:
      False{}
    case +m <> t:
      Bool.or(String.eq(m, x), Room.has(t, x))

def Room.named.k(+r: Room, +name: String) -> Bool:
  Bool.or(String.eq(Room.id(r), name), String.eq(String.to_lower(Room.name(r)), String.to_lower(name)))

def Room.find.go(rs: List<&2, Room>, +name: String) -> Maybe<&2, Room>:
  match rs:
    case Nil{}:
      None{}
    case +r <> t:
      Pick.first(Room, Room.named.k(r, name), r, Room.find.go(t, name))

def World.find_room(+w: World, +name: String) -> Maybe<&2, Room>:
  Room.find.go(World.rooms(w), name)

# the direct-message room of two names, whichever sends
def Room.dm(+a: String, +b: String) -> String:
  Bool.pick(String, String.is_le(a, b), "dm:" ++ a ++ ":" ++ b, "dm:" ++ b ++ ":" ++ a)

def Post.room(p: Post) -> String:
  match p:
    case Post{_, r, _, _, _, _}:
      r

# one post decided: kept (and one fewer still wanted) or passed over. The
# walk below builds its answer onto an accumulator, deciding each post
# once: handing both outcomes' walks to a chooser computed both, and the
# JS target is strict, so a room of 80 posts took 2^80 steps
def Post.in.n(hit: Bool, +n: Nat) -> Nat:
  match hit:
    case True{}:
      Nat.sub(n, 1n)
    case False{}:
      n

def Post.in.acc(hit: Bool, p: Post, acc: List<&2, Post>) -> List<&2, Post>:
  match hit:
    case True{}:
      p <> acc
    case False{}:
      acc

# the first n posts of ps (newest first) in room, oldest first
def Post.in.go(ps: List<&2, Post>, +room: String, +n: Nat, acc: List<&2, Post>) -> List<&2, Post>:
  match ps n:
    case Nil{} _:
      acc
    case _ 0n:
      acc
    case +p <> +t _:
      Post.in.go(t, room, Post.in.n(String.eq(Post.room(p), room), n), Post.in.acc(String.eq(Post.room(p), room), p, acc))

# a room's last n posts, oldest first
def World.posts(+w: World, +room: String, +n: Nat) -> List<&2, Post>:
  match w:
    case World{_, _, ps, _, _, _, _, _, _, _, _, _}:
      Post.in.go(ps, room, n, Nil{})

def Post.unread.add(new: Bool, +n: Nat) -> Nat:
  Bool.pick(Nat, new, 1n+n, n)

# a walk from the newest post, stopping after the first at or before `seen`
# (the older ones are older still); `done` says the last one was
def Post.unread.go(ps: List<&2, Post>, done: Bool, +room: String, +seen: Nat, +n: Nat) -> Nat:
  match ps done:
    case Nil{} _:
      n
    case +p <> +t True{}:
      n
    case Post{_, +r, +f, _, _, +at} <> t False{}:
      Post.unread.go(t, Nat.is_le(at, seen), room, seen, Post.unread.add(Bool.and(Nat.is_gt(at, seen), Bool.and(String.eq(r, room), Bool.not(String.eq(f, "you")))), n))

# the posts in a room newer than `seen` that the person did not write
def World.unread(+w: World, +room: String, +seen: Nat) -> Nat:
  match w:
    case World{_, _, ps, _, _, _, _, _, _, _, _, _}:
      Post.unread.go(ps, False{}, room, seen, 0n)

def Post.touches(p: Post, +bot: String) -> Bool:
  match p:
    case Post{_, r, f, _, _, _}:
      Bool.or(String.eq(f, bot), String.contains(r, bot))

def Post.by.go(ps: List<&2, Post>, +bot: String, +n: Nat, acc: List<&2, Post>) -> List<&2, Post>:
  match ps n:
    case Nil{} _:
      acc
    case _ 0n:
      acc
    case +p <> +t _:
      Post.by.go(t, bot, Post.in.n(Post.touches(p, bot), n), Post.in.acc(Post.touches(p, bot), p, acc))

# a bot's last n posts in any room (sent, or in a direct room of its own)
def World.posts_of(+w: World, +bot: String, +n: Nat) -> List<&2, Post>:
  match w:
    case World{_, _, ps, _, _, _, _, _, _, _, _, _}:
      Post.by.go(ps, bot, n, Nil{})

def World.space(+w: World, +bot: String) -> String:
  match w:
    case World{_, _, _, _, _, _, _, sp, _, _, _, _}:
      Got(String, Map.get(String, "", sp, bot))

# a bot's space page (empty html, at 0, when it has none)
def World.spage(+w: World, +bot: String) -> Page:
  match w:
    case World{_, _, _, _, _, _, _, _, _, _, pg, _}:
      Got(Page, Map.get(Page, Page{"", 0n}, pg, bot))

def Page.html(p: Page) -> String:
  match p:
    case Page{h, _}:
      h

def Page.at(p: Page) -> Nat:
  match p:
    case Page{_, a}:
      a

def World.hop(+w: World, +bot: String) -> Nat:
  match w:
    case World{_, _, _, _, _, _, _, _, ho, _, _, _}:
      Hops.now(Got(Hops, Map.get(Hops, Hops{0n, 0n, False{}}, ho, bot)))

def World.talked_at(+w: World, +bot: String) -> Nat:
  match w:
    case World{_, _, _, _, _, _, _, _, _, ta, pg, _}:
      Got(Nat, Map.get(Nat, 0n, ta, bot))

def Hook.live(h: Hook) -> Bool:
  match h:
    case Hook{_, _, _, _, r, _, _}:
      Bool.not(r)

def Hook.bot(h: Hook) -> String:
  match h:
    case Hook{_, b, _, _, _, _, _}:
      b

def Post.keep(-A: Data, hit: Bool, x: A, rest: List<&2, A>) -> List<&2, A>:
  match hit:
    case True{}:
      x <> rest
    case False{}:
      rest

def Hook.of.go(hs: List<&2, Hook>, +bot: String) -> List<&2, Hook>:
  match hs:
    case Nil{}:
      Nil{}
    case +h <> t:
      Post.keep(Hook, String.eq(Hook.bot(h), bot), h, Hook.of.go(t, bot))

def World.hooks(+w: World, +bot: String) -> List<&2, Hook>:
  match w:
    case World{_, _, _, hs, _, _, _, _, _, _, _, _}:
      List.filter(~Hook, ~Hook.live, Hook.of.go(Map.values(&2, Hook, hs), bot))

def Routine.live(r: Routine) -> Bool:
  match r:
    case Routine{_, _, _, _, _, _, _, _, d, _, _, _, _, _}:
      Bool.not(d)

def Routine.bot(r: Routine) -> String:
  match r:
    case Routine{_, b, _, _, _, _, _, _, _, _, _, _, _, _}:
      b

def Routine.id(r: Routine) -> String:
  match r:
    case Routine{i, _, _, _, _, _, _, _, _, _, _, _, _, _}:
      i

def Routine.cron(r: Routine) -> String:
  match r:
    case Routine{_, _, _, c, _, _, _, _, _, _, _, _, _, _}:
      c

def Routine.last(r: Routine) -> Nat:
  match r:
    case Routine{_, _, _, _, _, _, l, _, _, _, _, _, _, _}:
      l

def Routine.name(r: Routine) -> String:
  match r:
    case Routine{_, _, n, _, _, _, _, _, _, _, _, _, _, _}:
      n

def Routine.prompt(r: Routine) -> String:
  match r:
    case Routine{_, _, _, _, p, _, _, _, _, _, _, _, _, _}:
      p

def Routine.on(r: Routine) -> Bool:
  match r:
    case Routine{_, _, _, _, _, o, _, _, _, _, _, _, _, _}:
      o

def Routine.created(r: Routine) -> Nat:
  match r:
    case Routine{_, _, _, _, _, _, _, c, _, _, _, _, _, _}:
      c

def Routine.once(r: Routine) -> Nat:
  match r:
    case Routine{_, _, _, _, _, _, _, _, _, o, _, _, _, _}:
      o

def Routine.tz(r: Routine) -> String:
  match r:
    case Routine{_, _, _, _, _, _, _, _, _, _, t, _, _, _}:
      t

def Routine.kind(r: Routine) -> String:
  match r:
    case Routine{_, _, _, _, _, _, _, _, _, _, _, k, _, _}:
      k

def Routine.done(r: Routine) -> Bool:
  match r:
    case Routine{_, _, _, _, _, _, _, _, _, _, _, _, d, _}:
      d

def Routine.slot(r: Routine) -> Nat:
  match r:
    case Routine{_, _, _, _, _, _, _, _, _, _, _, _, _, s}:
      s

def World.routines.all(+w: World) -> List<&2, Routine>:
  match w:
    case World{_, _, _, _, ro, _, _, _, _, _, _, _}:
      List.filter(~Routine, ~Routine.live, Map.values(&2, Routine, ro))

def Routine.of.go(rs: List<&2, Routine>, +bot: String) -> List<&2, Routine>:
  match rs:
    case Nil{}:
      Nil{}
    case +r <> t:
      Post.keep(Routine, String.eq(Routine.bot(r), bot), r, Routine.of.go(t, bot))

def World.routines(+w: World, +bot: String) -> List<&2, Routine>:
  Routine.of.go(World.routines.all(w), bot)

def Peer.live(p: Peer) -> Bool:
  match p:
    case Peer{_, _, _, _, r}:
      Bool.not(r)

def World.peers(+w: World) -> List<&2, Peer>:
  match w:
    case World{_, _, _, _, _, _, pe, _, _, _, _, _}:
      List.filter(~Peer, ~Peer.live, Map.values(&2, Peer, pe))

# Linked machines by name
# -----------------------
# A bot elsewhere is "name@machine", where machine is a linked machine's id
# or its name. Names may clash (two machines called "laptop", ignoring
# case); ids never do. So an id is matched first, then the first name, the
# way the hub routes (bothub.bend's Bots.peer).

def Peer.id(p: Peer) -> String:
  match p:
    case Peer{i, _, _, _, _}:
      i

def Peer.name(p: Peer) -> String:
  match p:
    case Peer{_, n, _, _, _}:
      n

def Peer.by.id(ps: List<&2, Peer>, +x: String) -> Maybe<&2, Peer>:
  match ps:
    case Nil{}:
      None{}
    case +p <> t:
      Pick.first(Peer, String.eq(Peer.id(p), x), p, Peer.by.id(t, x))

def Peer.by.name(ps: List<&2, Peer>, +x: String) -> Maybe<&2, Peer>:
  match ps:
    case Nil{}:
      None{}
    case +p <> t:
      Pick.first(Peer, String.eq(String.to_lower(Peer.name(p)), String.to_lower(x)), p, Peer.by.name(t, x))

def Peer.of.k(m: Maybe<&2, Peer>, +ps: List<&2, Peer>, +x: String) -> Maybe<&2, Peer>:
  match m:
    case Some{p}:
      Some{p}
    case None{}:
      Peer.by.name(ps, x)

# the linked machine an id or a name stands for
def Peer.of(+ps: List<&2, Peer>, +x: String) -> Maybe<&2, Peer>:
  Peer.of.k(Peer.by.id(ps, x), ps, x)

def Peer.named(ps: List<&2, Peer>, +x: String) -> Nat:
  match ps:
    case Nil{}:
      0n
    case +p <> t:
      Nat.add(Bool.pick(Nat, String.eq(String.to_lower(Peer.name(p)), String.to_lower(x)), 1n, 0n), Peer.named(t, x))

def Peer.of.id(m: Maybe<&2, Peer>) -> String:
  match m:
    case Some{p}:
      Peer.id(p)
    case None{}:
      ""

def Peer.other(+got: String, +id: String) -> Bool:
  Bool.and(Bool.not(String.is_empty(got)), Bool.not(String.eq(got, id)))

# whether "name@<machine name>" could mean another machine than the one
# with this id: it is blank, two machines have it (any case), or it is
# another machine's id
def Peer.clash(+ps: List<&2, Peer>, +id: String, +name: String) -> Bool:
  Bool.or(Bool.or(String.is_empty(name), Nat.is_gt(Peer.named(ps, name), 1n)), Peer.other(Peer.of.id(Peer.of(ps, name)), id))

def Member.at(+m: String) -> List<&2, String>:
  String.split(m, '@')

def Member.name(+m: String) -> String:
  Maybe.default(&2, String, List.head(&2, String, Member.at(m)), "")

def Member.machine(+m: String) -> String:
  Maybe.default(&2, String, List.last(&2, String, Member.at(m)), "")

def Member.link.k(m: Maybe<&2, Peer>, +machine: String) -> String:
  match m:
    case Some{p}:
      Peer.id(p)
    case None{}:
      String.to_lower(machine)

# a bot elsewhere as one key: its name (any case, as its hub finds it) at
# its machine's id
def Member.far(+name: String, +link: String) -> String:
  String.to_lower(name) ++ "@" ++ link

def Member.key.k(far: Bool, +ps: List<&2, Peer>, +m: String) -> String:
  match far:
    case True{}:
      Member.far(Member.name(m), Member.link.k(Peer.of(ps, Member.machine(m)), Member.machine(m)))
    case False{}:
      m

# what a room member stands for: a bot here by its id, a bot elsewhere by
# Member.far, so "Kit@box", "kit@BOX" and "Kit@<box's id>" are one bot
def Member.key(+ps: List<&2, Peer>, +m: String) -> String:
  Member.key.k(String.contains(m, "@"), ps, m)

def Member.has(ms: List<&2, String>, +ps: List<&2, Peer>, +k: String) -> Bool:
  match ms:
    case Nil{}:
      False{}
    case +m <> t:
      Bool.or(String.eq(Member.key(ps, m), k), Member.has(t, ps, k))

# The depth bound
# ---------------
# A bot's message goes out one hop past the turn that sent it (World.woke
# above), and never past Hop.max(), so bots cannot keep each other talking
# forever. One that would go past it is held: kept in its room for the
# person (hop Hop.held()), heard by no bot.

def Hop.max() -> Nat:
  6n

# the hop of a message sent now, when it may go out
def Hop.send.k(ok: Bool, n: Nat) -> Maybe<&2, Nat>:
  match ok:
    case True{}:
      Some{n}
    case False{}:
      None{}

def Hop.next(+h: Nat) -> Maybe<&2, Nat>:
  Hop.send.k(Nat.is_le(1n+h, Hop.max()), 1n+h)

def Hop.send(+w: World, +bot: String) -> Maybe<&2, Nat>:
  Hop.next(World.hop(w, bot))

# the hop of a held message: one past the bound
def Hop.held() -> Nat:
  1n+Hop.max()

# a message at this hop is heard by bots
def Hop.live(+h: Nat) -> Bool:
  Nat.is_le(h, Hop.max())

# a back-and-forth between bots from hop h: each message (while one may go
# out) is answered by the other bot, busy (its next turn takes it, `True`)
# or at rest; the hop the talk has reached
def Hop.up.k(m: Maybe<&2, Nat>, busy: Bool, +h: Nat) -> Nat:
  match m:
    case Some{+n}:
      Hops.now(Hop.taken(Hop.woke(busy, Hops{h, 0n, False{}}, n)))
    case None{}:
      h

def Hop.climb(bs: List<&2, Bool>, +h: Nat) -> Nat:
  match bs:
    case Nil{}:
      h
    case b <> t:
      Hop.climb(t, Hop.up.k(Hop.next(h), b, h))

# a message that may go out is within the bound
def Hop.bound(m: Maybe<&2, Nat>) -> Bool:
  match m:
    case Some{h}:
      Nat.is_le(h, Hop.max())
    case None{}:
      True{}

# Memory
# ------
# Keyed and typed: saving a key replaces it, so a bot keeps one current
# version of each thing it knows. Every turn gets a small block: what it
# knows about its people and how they like things done, then the few items
# that match the turn's text.

def Mem.kinds() -> List<&2, String>:
  ["user", "pref", "fact", "howto", "note"]

def Mem.kind.norm(+k: String) -> String:
  Bool.pick(String, Room.has(Mem.kinds(), String.to_lower(k)), String.to_lower(k), "fact")

def Mem.slug.c(+c: Char) -> Char:
  Bool.pick(Char, Bool.or(Char.is_alpha(c), Bool.or(Char.is_digit(c), Bool.or(Char.is_eq(c, '.'), Char.is_eq(c, '_')))), Char.to_lower(c), '-')

def Mem.slug.go(s: String) -> String:
  match s:
    case SNil{}:
      SNil{}
    case SCon{+c, t}:
      SCon{Mem.slug.c(c), Mem.slug.go(t)}

# a key: lower case, words joined by '-', at most 64 characters
def Mem.key.norm(s: String) -> String:
  String.take(Mem.slug.go(String.trim(s)), 64n)

def Mem.bot(m: Mem) -> String:
  match m:
    case Mem{b, _, _, _, _, _, _}:
      b

def Mem.key(m: Mem) -> String:
  match m:
    case Mem{_, k, _, _, _, _, _}:
      k

def Mem.kind(m: Mem) -> String:
  match m:
    case Mem{_, _, k, _, _, _, _}:
      k

def Mem.updated(m: Mem) -> Nat:
  match m:
    case Mem{_, _, _, _, _, _, u}:
      u

def Mem.newer(a: Mem, b: Mem) -> Bool:
  Nat.is_ge(Mem.updated(a), Mem.updated(b))

def Mem.of.go(ms: List<&2, Mem>, +bot: String) -> List<&2, Mem>:
  match ms:
    case Nil{}:
      Nil{}
    case +m <> t:
      Post.keep(Mem, String.eq(Mem.bot(m), bot), m, Mem.of.go(t, bot))

# a bot's memory, newest first
def World.mems(+w: World, +bot: String) -> List<&2, Mem>:
  match w:
    case World{_, _, _, _, _, ms, _, _, _, _, _, _}:
      List.sort(~Mem, ~Mem.newer, Mem.of.go(Map.values(&2, Mem, ms), bot))

def Mem.line(m: Mem) -> String:
  match m:
    case Mem{_, k, kind, text, +tags, _, _}:
      "- [" ++ kind ++ "] " ++ k ++ ": " ++ text ++ Bool.pick(String, String.is_empty(tags), "", " (" ++ tags ++ ")")

# words: runs of letters and digits, lower case, three or more long
def Words.flush(+w: String, +acc: List<&2, String>) -> List<&2, String>:
  Bool.pick(List<&2, String>, Nat.is_ge(String.length(w), 3n), String.reverse(w) <> acc, acc)

type WSt is Data:
  WSt{cur: String, acc: List<&2, String>}

def Words.step.k(word: Bool, +c: Char, ws: WSt) -> WSt:
  match word ws:
    case True{} WSt{cur, acc}:
      WSt{SCon{Char.to_lower(c), cur}, acc}
    case False{} WSt{cur, acc}:
      WSt{"", Words.flush(cur, acc)}

def Words.step(+c: Char, ws: WSt) -> WSt:
  Words.step.k(Bool.or(Char.is_alpha(c), Char.is_digit(c)), c, ws)

def Words.go(s: String, ws: WSt) -> List<&2, String>:
  match s ws:
    case SNil{} WSt{cur, acc}:
      Words.flush(cur, acc)
    case SCon{+c, t} _:
      Words.go(t, Words.step(c, ws))

def Words.of(s: String) -> List<&2, String>:
  Words.go(s, WSt{"", Nil{}})

def Words.stop() -> List<&2, String>:
  ["the", "and", "for", "you", "your", "with", "that", "this", "are", "was", "can", "will", "what", "how", "please", "from", "have", "not", "but", "about", "into", "our", "any", "all"]

def Mem.hay(m: Mem) -> String:
  match m:
    case Mem{_, k, _, _, tags, _, _}:
      k ++ " " ++ tags

def Mem.text(m: Mem) -> String:
  match m:
    case Mem{_, _, _, t, _, _, _}:
      t

def Mem.score.w.k(stop: Bool, +w: String, +hay: String, +text: String) -> Nat:
  match stop:
    case True{}:
      0n
    case False{}:
      Nat.add(Bool.pick(Nat, String.contains(hay, w), 3n, 0n), Bool.pick(Nat, String.contains(text, w), 1n, 0n))

# Stop words score zero without scanning either memory string.
def Mem.score.w(+w: String, hay: String, text: String) -> Nat:
  Mem.score.w.k(Room.has(Words.stop(), w), w, hay, text)

def Mem.score.go(qs: List<&2, String>, +hay: String, +text: String) -> Nat:
  match qs:
    case Nil{}:
      0n
    case +q <> t:
      Nat.add(Mem.score.w(q, hay, text), Mem.score.go(t, hay, text))

# how well an item matches the words of a query
def Mem.score(+qs: List<&2, String>, +m: Mem) -> Nat:
  Mem.score.go(qs, String.to_lower(Mem.hay(m)), String.to_lower(Mem.text(m)))

type Hit is Data:
  Hit{score: Nat, mem: Mem}

def Hit.before(a: Hit, b: Hit) -> Bool:
  match a b:
    case Hit{+sa, ma} Hit{+sb, mb}:
      Bool.or(Nat.is_gt(sa, sb), Bool.and(Nat.is_eq(sa, sb), Mem.newer(ma, mb)))

def Hit.mem(h: Hit) -> Mem:
  match h:
    case Hit{_, m}:
      m

def Hit.scored.k(+s: Nat, m: Mem, +rest: List<&2, Hit>) -> List<&2, Hit>:
  Bool.pick(List<&2, Hit>, Nat.is_eq(s, 0n), rest, Hit{s, m} <> rest)

def Hit.scored.go(ms: List<&2, Mem>, +qs: List<&2, String>) -> List<&2, Hit>:
  match ms:
    case Nil{}:
      Nil{}
    case +m <> t:
      Hit.scored.k(Mem.score(qs, m), m, Hit.scored.go(t, qs))

def Hit.mems(hs: List<&2, Hit>) -> List<&2, Mem>:
  match hs:
    case Nil{}:
      Nil{}
    case h <> t:
      Hit.mem(h) <> Hit.mems(t)

# the n items that best match a query, best first
def Mem.recall(ms: List<&2, Mem>, query: String, n: Nat) -> List<&2, Mem>:
  Hit.mems(List.take(&2, Hit, List.sort(~Hit, ~Hit.before, Hit.scored.go(ms, Words.of(query))), n))

def Mem.always.k(+k: String) -> Bool:
  Bool.or(String.eq(k, "user"), String.eq(k, "pref"))

def Mem.always(m: Mem) -> Bool:
  Mem.always.k(Mem.kind(m))

# notes last a month in the block (recall still finds them)
def Mem.fresh(+now: Nat, +m: Mem) -> Bool:
  Bool.or(Bool.not(String.eq(Mem.kind(m), "note")), Nat.is_le(now, Nat.add(Mem.updated(m), 2592000n)))

def Mem.budget() -> Nat:
  2400n

def Mem.fit.k(ok: Bool, l: String, rest: List<&2, String>) -> List<&2, String>:
  match ok:
    case True{}:
      l <> rest
    case False{}:
      Nil{}

# lines while they fit in `room` characters: the count used never passes it
def Mem.fit(ls: List<&2, String>, +room: Nat) -> List<&2, String>:
  match ls:
    case Nil{}:
      Nil{}
    case +l <> t:
      Mem.fit.k(Nat.is_le(1n+String.length(l), room), l, Mem.fit(t, Nat.sub(room, 1n+String.length(l))))

def Mem.used(ls: List<&2, String>) -> Nat:
  match ls:
    case Nil{}:
      0n
    case +l <> t:
      Nat.add(1n+String.length(l), Mem.used(t))

def Mem.lines(ms: List<&2, Mem>) -> List<&2, String>:
  match ms:
    case Nil{}:
      Nil{}
    case m <> t:
      Mem.line(m) <> Mem.lines(t)

def Mem.fresh.all(ms: List<&2, Mem>, +now: Nat) -> List<&2, Mem>:
  match ms:
    case Nil{}:
      Nil{}
    case +m <> t:
      Post.keep(Mem, Mem.fresh(now, m), m, Mem.fresh.all(t, now))

def Mem.others(ms: List<&2, Mem>, +now: Nat) -> List<&2, Mem>:
  List.filter(~Mem, ~(m => Bool.not(Mem.always(m))), Mem.fresh.all(ms, now))

def Mem.block.k(ls: List<&2, String>) -> String:
  match ls:
    case Nil{}:
      ""
    case _:
      "<memory>\n" ++ String.join(ls, "\n") ++ "\n</memory>"

# the block a turn starts with ("" when the bot knows nothing yet)
def Mem.block(+ms: List<&2, Mem>, text: String, +now: Nat) -> String:
  Mem.block.k(Mem.fit(Mem.lines(List.append(&2, Mem, List.filter(~Mem, ~Mem.always, ms), Mem.recall(Mem.others(ms, now), text, 6n))), Mem.budget()))

def Hook.id(h: Hook) -> String:
  match h:
    case Hook{i, _, _, _, _, _, _}:
      i

# The creation form and hub use the same name rule.
def Bot.name.char(+c: Char) -> Bool:
  Bool.or(Char.is_alpha(c), Bool.or(Char.is_digit(c), Bool.or(Char.is_eq(c, '-'), Char.is_eq(c, '_'))))

def Bot.name.chars(s: String) -> Bool:
  match s:
    case SNil{}: True{}
    case SCon{+c, t}: Bool.and(Bot.name.char(c), Bot.name.chars(t))

def Bot.name.valid(+name: String) -> Bool:
  Bool.and(Bool.and(Nat.is_ge(String.length(name), 1n), Nat.is_le(String.length(name), 24n)), Bot.name.chars(name))
