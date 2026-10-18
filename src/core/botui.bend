import Base
import ./i18n.bend as I
import ./msgs.bend as MSG
import ./json.bend as J
import ./model.bend as M
import ./client.bend as U
import ./bot.bend as BT
import ./sched.bend as SC
import ./space.bend as SP
import ./once.bend as O
import ./far.bend as FR
import ./uber.bend as UB
import ./ubersync.bend as US

# What the bots' screens show, for every client (the window, the web and
# the phones draw from these; docs/bots.md). The sidebar's cats with their
# moods, the rooms, and the parts of a bot's view.

# Moods
# -----
# A cat's mood comes from its thread's status: it waits for you (ears up)
# before anything else, works or talks while its turn runs, thinks while
# something is queued, looks unwell after a failure, and falls asleep
# after a quarter hour of rest (laws bot_mood_wait, bot_mood_sleep_still).

def Mood.of.r(idle: Bool) -> String:
  match idle:
    case True{}:
      "sleep"
    case False{}:
      "idle"

def Mood.of(s: M.Status, talked: Bool, idle: Bool) -> String:
  match s:
    case M.StApproval{}:
      "wait"
    case M.StInput{}:
      "wait"
    case M.StWorking{}:
      Bool.pick(String, talked, "talk", "work")
    case M.StWaiting{}:
      Bool.pick(String, talked, "talk", "work")
    case M.StFailed{}:
      "error"
    case M.StReady{}:
      Mood.of.r(idle)

# a bot on another machine, from the state its hub reports
def Mood.of.name(+state: String) -> String:
  Bool.pick(String, Bool.or(String.eq(state, "approval"), String.eq(state, "input")), "wait",
    Bool.pick(String, String.eq(state, "working"), "work",
      Bool.pick(String, String.eq(state, "queued"), "think",
        Bool.pick(String, String.eq(state, "failed"), "error",
          Bool.pick(String, String.eq(state, "ready"), "idle", "away")))))

# whether a mood moves in the window: only a cat doing something (working,
# thinking, waiting for you, talking). A ready, sleeping, sick or absent
# cat holds still, so a window of resting bots never repaints (each cat
# frame lays out the window: 27 ms on a real log; law bot_mood_idle_still).
# The web and the phones animate every awake cat on the GPU.
def Mood.moving(+m: String) -> Bool:
  Bool.or(Bool.or(String.eq(m, "work"), String.eq(m, "think")), Bool.or(String.eq(m, "wait"), String.eq(m, "talk")))

def Mood.note.loc(+l: I.Loc, +m: String) -> String:
  Bool.pick(String, String.eq(m, "sleep"), MSG.T.bots.mood.sleep(l),
    Bool.pick(String, String.eq(m, "idle"), MSG.T.bots.mood.idle(l),
      Bool.pick(String, String.eq(m, "work"), MSG.T.bots.mood.work(l),
        Bool.pick(String, String.eq(m, "think"), MSG.T.bots.mood.think(l),
          Bool.pick(String, String.eq(m, "wait"), MSG.T.bots.mood.wait(l),
            Bool.pick(String, String.eq(m, "talk"), MSG.T.bots.mood.talk(l),
              Bool.pick(String, String.eq(m, "error"), MSG.T.bots.mood.error(l), MSG.T.bots.mood.away(l))))))))

def Mood.note(m: String) -> String:
  Mood.note.loc(I.En{}, m)

def Mood.rest() -> Nat:
  900n

def Mood.talk() -> Nat:
  60n

# Sidebar
# -------

# a cat in the list: a bot here, or one on a linked machine (remote), whose
# peer is its machine as shown and link that machine's id
type BRow is Data:
  BRow{id: String, name: String, thread: String, look: Nat, mood: String, note: String, sel: Bool, remote: Bool, peer: String, link: String}

type RRow is Data:
  RRow{id: String, name: String, members: Nat, sel: Bool}

def BU.bot.sel(+u: U.Ui) -> String:
  U.Ui.scratch(u, "@bot")

def BU.room.sel(+u: U.Ui) -> String:
  U.Ui.scratch(u, "@room")

def BU.remote.sel(+u: U.Ui) -> String:
  U.Ui.scratch(u, "@remote")

def BU.row.m(+mood: String, i: String, n: String, th: String, lk: Nat, sel: Bool) -> BRow:
  BRow{i, n, th, lk, mood, Mood.note(mood), sel, False{}, "", ""}

def BU.row.far.m(+mood: String, i: String, n: String, th: String, lk: Nat, sel: Bool, pe: String, l: String) -> BRow:
  BRow{i, n, th, lk, mood, Mood.note(mood), sel, True{}, pe, l}

# what choosing a row does: a bot with a thread here (this machine's, or
# one mirrored from its own) opens as a bot ("bot"); a bot only reported
# by a linked machine is a conversation ("remote")
def BU.act(r: BRow) -> String:
  match r:
    case BRow{_, _, +th, _, _, _, _, _, _, _}:
      Bool.pick(String, String.is_empty(th), "remote", "bot")

# Each bot once
# -------------
# Every list of bots keys each one by what it stands for and keeps the
# first of each key (once.bend; laws bot_rows_once, bot_picks_once,
# hubs_bots_once). A bot here is its id. A bot elsewhere is its name (any
# case, as its hub finds it) at its machine's id (BT.Member.far): the same
# bot reported twice is one, and bots on two machines that share a name
# stay two. Where a label could mean two bots it carries the machine's id
# (a machine) or the bot's id (two bots here with one name).

def BU.key.k(remote: Bool, +i: String, +n: String, +l: String) -> String:
  match remote:
    case True{}:
      BT.Member.far(n, l)
    case False{}:
      i

def BU.key(r: BRow) -> String:
  match r:
    case BRow{+i, +n, _, _, _, _, _, rem, _, +l}:
      BU.key.k(rem, i, n, l)

def BU.ky(+r: BRow) -> O.Ky<BRow>:
  O.Ky{BU.key(r), r}

def BU.kys(rs: List<&2, BRow>) -> List<&2, O.Ky<BRow>>:
  match rs:
    case Nil{}:
      Nil{}
    case +r <> t:
      BU.ky(r) <> BU.kys(t)

def BU.named(bs: List<&2, BT.Bot>, +name: String) -> Nat:
  match bs:
    case Nil{}:
      0n
    case +b <> t:
      Nat.add(Bool.pick(Nat, BT.Bot.named.k(b, name), 1n, 0n), BU.named(t, name))

# a bot's name, with its id when another bot here has the same name
def BU.label(clash: Bool, +n: String, +i: String) -> String:
  match clash:
    case True{}:
      n ++ " (" ++ i ++ ")"
    case False{}:
      n

# a machine in a bot's id: its name, or its id when the name could mean
# another machine (BT.Peer.clash)
def BU.far.at(clash: Bool, +link: String, +pname: String) -> String:
  match clash:
    case True{}:
      link
    case False{}:
      pname

# and as shown: its name, with its id when the name could mean another
def BU.far.label(clash: Bool, +link: String, +pname: String) -> String:
  match clash:
    case True{}:
      Bool.pick(String, String.is_empty(pname), link, pname ++ " (" ++ link ++ ")")
    case False{}:
      pname

def BU.far.row(+id: String, +o: J.Json, +peer: String, +link: String, +sel: String) -> BRow:
  BRow{id, J.Json.get_str(o, "name", ""), "", J.Json.get_nat(o, "look", 0n), Mood.of.name(J.Json.get_str(o, "state", "")),
    Mood.note(Mood.of.name(J.Json.get_str(o, "state", ""))), String.eq(id, sel), True{}, peer, link}

def BU.far.k(+clash: Bool, +o: J.Json, +link: String, +pname: String, +sel: String) -> BRow:
  BU.far.row(J.Json.get_str(o, "name", "") ++ "@" ++ BU.far.at(clash, link, pname), o, BU.far.label(clash, link, pname), link, sel)

def BU.far.name(+o: J.Json) -> String:
  J.Json.get_str(o, "peerName", J.Json.get_str(o, "peer", ""))

# one bot reported by a linked machine
def BU.far(+ps: List<&2, BT.Peer>, +o: J.Json, +sel: String) -> BRow:
  BU.far.k(BT.Peer.clash(ps, J.Json.get_str(o, "peer", ""), BU.far.name(o)), o, J.Json.get_str(o, "peer", ""), BU.far.name(o), sel)

def BU.fars(os: List<&2, J.Json>, +ps: List<&2, BT.Peer>, +sel: String) -> List<&2, BRow>:
  match os:
    case Nil{}:
      Nil{}
    case +o <> t:
      BU.far(ps, o, sel) <> BU.fars(t, ps, sel)

def BU.peers.of(+u: U.Ui) -> List<&2, BT.Peer>:
  U.Bots.peers(u)

# every bot the linked machines report, as they report them
def BU.remote.all(+u: U.Ui) -> List<&2, BRow>:
  BU.fars(J.Json.elems(Maybe.default(&2, J.Json, J.Json.parse(U.Ui.info(u, "bots.remote")), J.Json.arr(Nil{}))), BU.peers.of(u), BU.remote.sel(u))

# the bots on linked machines, each once
def BU.remote(+u: U.Ui) -> List<&2, BRow> :
  O.Once.vals(BRow, O.Once.of(BRow, BU.kys(BU.remote.all(u))))

# Each room once
# --------------
# A room list keys each room by its id. A room shared with a linked machine
# is kept on both under one id, so two sources can report it (two paired
# hubs on a phone): it is listed once, the last report kept (laws
# rooms_listed_once, hubs_rooms_listed_once). Rooms are never merged by
# name: two rooms with one name both show, each named with its id.

type RK<-A: Data> is Data:
  RK{key: String, val: A}

# whether key k is among xs
def BU.once.has(-A: Data, xs: List<&2, RK<A>>, +k: String) -> Bool:
  match xs:
    case Nil{}:
      False{}
    case RK{+x, _} <> t:
      Bool.or(String.eq(x, k), BU.once.has(A, t, k))

# no key comes twice
def BU.once.distinct(-A: Data, xs: List<&2, RK<A>>) -> Bool:
  match xs:
    case Nil{}:
      True{}
    case RK{+k, _} <> +t:
      Bool.and(Bool.not(BU.once.has(A, t, k)), BU.once.distinct(A, t))

def BU.once.step(-A: Data, seen: Bool, +k: String, v: A, rest: List<&2, RK<A>>) -> List<&2, RK<A>>:
  match seen:
    case True{}:
      rest
    case False{}:
      RK{k, v} <> rest

# each key once: an item whose key comes again later is dropped
def BU.once.of(-A: Data, xs: List<&2, RK<A>>) -> List<&2, RK<A>>:
  match xs:
    case Nil{}:
      Nil{}
    case RK{+k, v} <> +t:
      BU.once.step(A, BU.once.has(A, t, k), k, v, BU.once.of(A, t))

def BU.once.vals(-A: Data, xs: List<&2, RK<A>>) -> List<&2, A>:
  match xs:
    case Nil{}:
      Nil{}
    case RK{_, v} <> t:
      v <> BU.once.vals(A, t)

# how many of ns are n, in any case
def BU.rooms.count(ns: List<&2, String>, +n: String) -> Nat:
  match ns:
    case Nil{}:
      0n
    case +x <> t:
      Nat.add(Bool.pick(Nat, String.eq(x, n), 1n, 0n), BU.rooms.count(t, n))

def BU.rooms.names(rs: List<&2, BT.Room>) -> List<&2, String>:
  match rs:
    case Nil{}:
      Nil{}
    case BT.Room{_, +n, _, _, _} <> t:
      String.to_lower(n) <> BU.rooms.names(t)

# a room's name, with its id when another room has the same name
def BU.rooms.label(+ns: List<&2, String>, +n: String, +i: String) -> String:
  Bool.pick(String, Nat.is_gt(BU.rooms.count(ns, String.to_lower(n)), 1n), n ++ " · " ++ i, n)

def BU.rooms.go(rs: List<&2, BT.Room>, +ns: List<&2, String>, +sel: String) -> List<&2, RK<RRow>>:
  match rs:
    case Nil{}:
      Nil{}
    case BT.Room{+i, +n, m, _, _} <> t:
      RK{i, RRow{i, BU.rooms.label(ns, n, i), List.length(&2, String, m), String.eq(i, sel)}} <> BU.rooms.go(t, ns, sel)

def BU.rooms.of(+rs: List<&2, BT.Room>, +sel: String) -> List<&2, RK<RRow>>:
  BU.once.of(RRow, BU.rooms.go(rs, BU.rooms.names(rs), sel))

# the rooms here, keyed by id, each once
def BU.rooms.keyed(+u: U.Ui) -> List<&2, RK<RRow>>:
  BU.rooms.of(BT.World.rooms(M.State.bots(U.Ui.state(u))), BU.room.sel(u))

def BU.rooms(+u: U.Ui) -> List<&2, RRow>:
  BU.once.vals(RRow, BU.rooms.keyed(u))

def BRow.link(r: BRow) -> String:
  match r:
    case BRow{_, _, _, _, _, _, _, _, _, l}:
      l

def BRow.name(r: BRow) -> String:
  match r:
    case BRow{_, n, _, _, _, _, _, _, _, _}:
      n

def BRow.moving(r: BRow) -> Bool:
  match r:
    case BRow{_, _, _, _, +m, _, _, _, _, _}:
      Mood.moving(m)

def BU.moving.go(rs: List<&2, BRow>) -> Bool:
  match rs:
    case Nil{}:
      False{}
    case r <> t:
      Bool.or(BRow.moving(r), BU.moving.go(t))

# The main area
# -------------

type View is Data:
  VThread{}
  VBot{bot: BT.Bot}
  VRoom{room: BT.Room}
  VRemote{id: String}

def BU.view.bot.k(same: Bool, b: BT.Bot) -> View:
  match same:
    case True{}:
      VBot{b}
    case False{}:
      VThread{}

def BU.view.bot(m: Maybe<&2, BT.Bot>, +u: U.Ui, +sel: String) -> View:
  match m:
    case Some{+b}:
      BU.view.bot.k(U.Bots.chat.is(U.Ui.state(u), BT.Bot.id(b), BT.Bot.thread(b), sel), b)
    case None{}:
      VThread{}

def BU.view.far(none: Bool, +u: U.Ui) -> View:
  match none:
    case True{}:
      BU.view.bot(BT.World.find.id(BT.World.bot(M.State.bots(U.Ui.state(u)), BU.bot.sel(u))), u, U.Ui.sel(u))
    case False{}:
      VRemote{BU.remote.sel(u)}

def BU.view.room(m: Maybe<&2, BT.Room>, +u: U.Ui) -> View:
  match m:
    case Some{r}:
      VRoom{r}
    case None{}:
      BU.view.far(String.is_empty(BU.remote.sel(u)), u)

def BU.room.get(r: BT.Room) -> Maybe<&2, BT.Room>:
  match r:
    case BT.Room{i, n, m, c, +d}:
      Bool.pick(Maybe<&2, BT.Room>, d, None{}, Some{BT.Room{i, n, m, c, d}})

# what the main area shows: a room, a remote bot, a bot, or the thread
def BU.view(+u: U.Ui) -> View:
  BU.view.room(BU.room.get(BT.World.room(M.State.bots(U.Ui.state(u)), BU.room.sel(u))), u)

def BU.tabs() -> List<&2, String>:
  ["chat", "space", "browser", "tasks", "memory", "routines", "hooks", "settings"]

# a bot's tabs: all of them for a bot here; for one mirrored from a linked
# machine (far.bend) what its machine shares: its chat, space, memory and
# routines. Its browser, tasks, webhooks and settings stay on its machine (law
# far_bot_tabs).
def BU.tabs.of(+id: String) -> List<&2, String>:
  Bool.pick(List<&2, String>, FR.Fr.is(id), ["chat", "space", "memory", "routines"], BU.tabs())

# the uberbot's tabs: its status first
def BU.tabs.for(+u: U.Ui, +id: String) -> List<&2, String>:
  Bool.pick(List<&2, String>, UB.Uber.is(U.Ui.state(u), id), "status" <> BU.tabs.of(id), BU.tabs.of(id))

def BU.tab.label(+l: I.Loc, +t: String) -> String:
  Bool.pick(String, String.eq(t, "chat"), MSG.T.bots.tab.chat(l),
    Bool.pick(String, String.eq(t, "space"), MSG.T.bots.tab.space(l),
      Bool.pick(String, String.eq(t, "browser"), MSG.T.bots.tab.browser(l),
        Bool.pick(String, String.eq(t, "tasks"), MSG.T.bots.tab.tasks(l),
        Bool.pick(String, String.eq(t, "memory"), MSG.T.bots.tab.memory(l),
          Bool.pick(String, String.eq(t, "routines"), MSG.T.bots.tab.routines(l),
            Bool.pick(String, String.eq(t, "hooks"), MSG.T.bots.tab.hooks(l), Bool.pick(String, String.eq(t, "status"), MSG.T.bots.tab.status(l), MSG.T.bots.tab.settings(l)))))))))

def BU.tab.has(ts: List<&2, String>, +t: String) -> Bool:
  match ts:
    case Nil{}:
      False{}
    case x <> r:
      Bool.or(String.eq(x, t), BU.tab.has(r, t))

# the bot's tab ("chat" until one is picked)
def BU.tab(+u: U.Ui) -> String:
  Bool.pick(String, String.is_empty(U.Ui.scratch(u, "@btab")), "chat", U.Ui.scratch(u, "@btab"))

# the tab when none is picked (or the one picked is not the bot's): the
# uberbot's status, else chat
def BU.tab.dflt(+u: U.Ui, +id: String) -> String:
  Bool.pick(String, UB.Uber.is(U.Ui.state(u), id), "status", "chat")

# the tab shown: the one picked, when this bot has it, else its chat
def BU.tab.of(+u: U.Ui, +id: String) -> String:
  Bool.pick(String, String.is_empty(U.Ui.scratch(u, "@btab")), BU.tab.dflt(u, id),
    Bool.pick(String, BU.tab.has(BU.tabs.for(u, id), BU.tab(u)), BU.tab(u), BU.tab.dflt(u, id)))

def BU.space.none(paged: Bool, +name: String) -> SP.Space:
  match paged:
    case True{}:
      SP.Space.paged(name)
    case False{}:
      SP.Space.empty(name)

def BU.space.k(none: Bool, paged: Bool, +spec: String, +name: String) -> SP.Space:
  match none:
    case True{}:
      BU.space.none(paged, name)
    case False{}:
      SP.Space.read(spec)

# a bot's space: what it wrote, else a note that its space is a page (on
# the phones), else a friendly empty one
def BU.space(+u: U.Ui, b: BT.Bot) -> SP.Space:
  match b:
    case BT.Bot{+i, +n, _, _, _, _, _}:
      BU.space.k(String.is_empty(BT.World.space(M.State.bots(U.Ui.state(u)), i)),
        Bool.not(String.is_empty(BT.Page.html(BT.World.spage(M.State.bots(U.Ui.state(u)), i)))), BT.World.space(M.State.bots(U.Ui.state(u)), i), n)

def BU.mems(+u: U.Ui, +bot: String) -> List<&2, BT.Mem>:
  BT.World.mems(M.State.bots(U.Ui.state(u)), bot)

# a bot's space page (empty html when it has none)
def BU.page(+u: U.Ui, +bot: String) -> BT.Page:
  BT.World.spage(M.State.bots(U.Ui.state(u)), bot)

# a bot's routines in the order its list shows them: pending by next fire,
# then off ones, then done ones, newest first (core/sched.bend)
def BU.routines(+u: U.Ui, +bot: String) -> List<&2, BT.Routine>:
  SC.Sched.order(U.Ui.state(u), BT.World.routines(M.State.bots(U.Ui.state(u)), bot), U.Side.now(u), 0n, True{})

def BU.hooks(+u: U.Ui, +bot: String) -> List<&2, BT.Hook>:
  BT.World.hooks(M.State.bots(U.Ui.state(u)), bot)

def BU.peers(+u: U.Ui) -> List<&2, BT.Peer>:
  BT.World.peers(M.State.bots(U.Ui.state(u)))

# a room's recent posts, oldest first
def BU.posts(+u: U.Ui, +room: String) -> List<&2, BT.Post>:
  BT.World.posts(M.State.bots(U.Ui.state(u)), room, 200n)

# the person's conversation with a bot elsewhere
def BU.remote.posts(+u: U.Ui, +id: String) -> List<&2, BT.Post>:
  BU.posts(u, BT.Room.dm("you", id))

# a bot's conversations with other bots, newest last
def BU.talk(+u: U.Ui, b: BT.Bot) -> List<&2, BT.Post>:
  match b:
    case BT.Bot{_, +n, _, _, _, _, _}:
      BT.World.posts_of(M.State.bots(U.Ui.state(u)), n, 100n)

# the bots a room form can pick from, and whether each is picked
type Pick is Data:
  Pick{id: String, name: String, on: Bool}

def BU.picks.go(bs: List<&2, BT.Bot>, +ps: List<&2, BT.Peer>, +ms: List<&2, String>, +all: List<&2, BT.Bot>) -> List<&2, O.Ky<Pick>>:
  match bs:
    case Nil{}:
      Nil{}
    case BT.Bot{+i, +n, _, _, _, _, _} <> t:
      O.Ky{i, Pick{i, BU.label(Nat.is_gt(BU.named(all, n), 1n), n, i), BT.Member.has(ms, ps, i)}} <> BU.picks.go(t, ps, ms, all)

# bots on linked machines, by the name@machine rooms and bot_send take
def BU.picks.far(rs: List<&2, BRow>, +ps: List<&2, BT.Peer>, +ms: List<&2, String>) -> List<&2, O.Ky<Pick>>:
  match rs:
    case Nil{}:
      Nil{}
    case BRow{+i, +n, _, _, _, _, _, _, +pe, +l} <> t:
      O.Ky{BT.Member.far(n, l), Pick{i, n ++ "@" ++ pe, BT.Member.has(ms, ps, BT.Member.far(n, l))}} <> BU.picks.far(t, ps, ms)

# members elsewhere, kept picked so they can be dropped: those no linked
# machine lists now (one offline or unlinked) stay; the rest are the bots
# listed above, and go
def BU.picks.gone(ms: List<&2, String>, +ps: List<&2, BT.Peer>) -> List<&2, O.Ky<Pick>>:
  match ms:
    case Nil{}:
      Nil{}
    case +m <> t:
      BT.Post.keep(O.Ky<Pick>, String.contains(m, "@"), O.Ky{BT.Member.key(ps, m), Pick{m, m, True{}}}, BU.picks.gone(t, ps))

# this machine's bots, then those on linked machines ("name@machine"),
# then members elsewhere no machine lists, before each is kept once
def BU.picks.all(+u: U.Ui, +ms: List<&2, String>) -> List<&2, O.Ky<Pick>>:
  List.append(&2, O.Ky<Pick>, BU.picks.go(BT.World.bots.here(M.State.bots(U.Ui.state(u))), BU.peers.of(u), ms, BT.World.bots.here(M.State.bots(U.Ui.state(u)))),
    List.append(&2, O.Ky<Pick>, BU.picks.far(BU.remote.all(u), BU.peers.of(u), ms), BU.picks.gone(ms, BU.peers.of(u))))

def BU.picks.src(+u: U.Ui) -> List<&2, O.Ky<Pick>>:
  BU.picks.all(u, BT.Room.members.of(U.Ui.scratch(u, "@f.rmembers")))

def BU.picks.keyed(+u: U.Ui) -> List<&2, O.Ky<Pick>>:
  O.Once.of(Pick, BU.picks.src(u))

# the bots a room form can pick from, each once (law bot_picks_once)
def BU.picks(+u: U.Ui) -> List<&2, Pick>:
  O.Once.vals(Pick, BU.picks.keyed(u))

# a member's name as people read it (a bot id here becomes its name)
def BU.member.k(m: Maybe<&2, BT.Bot>, +x: String) -> String:
  match m:
    case Some{b}:
      BT.Bot.name(b)
    case None{}:
      x

def BU.member(+u: U.Ui, +x: String) -> String:
  BU.member.k(BT.World.find.id(BT.World.bot(M.State.bots(U.Ui.state(u)), x)), x)

def BU.members.go(ms: List<&2, String>, +u: U.Ui) -> List<&2, String>:
  match ms:
    case Nil{}:
      Nil{}
    case +m <> t:
      BU.member(u, m) <> BU.members.go(t, u)

def BU.members(+u: U.Ui, r: BT.Room) -> String:
  String.join(BU.members.go(BT.Room.members(r), u), ", ")

# a form field's text
def BU.field(+u: U.Ui, +k: String) -> String:
  U.Ui.scratch(u, "@f." ++ k)

# whether a form is open ("@bnew", "@rnew", "@onew")
def BU.open(+u: U.Ui, +k: String) -> Bool:
  String.eq(U.Ui.scratch(u, k), "1")

def Thread.activity(t: M.Thread) -> Nat:
  match t:
    case M.Thread{_, _, _, _, a, _, _, _, _, _, _, _, _, _, _, _, _, _, _, _, _}:
      a

def Mood.bot.t(+t: M.Thread, +st: M.State, +now: Nat, +talked: Nat) -> String:
  Mood.of(U.Side.status(st, t), Nat.is_le(now, Nat.add(talked, Mood.talk())), Nat.is_gt(now, Nat.add(Thread.activity(t), Mood.rest())))

def Mood.bot(+st: M.State, +now: Nat, b: BT.Bot) -> String:
  match b:
    case BT.Bot{+i, _, +th, _, _, _, _}:
      Mood.bot.t(U.Ui.thread(st, th), st, now, BT.World.talked_at(M.State.bots(st), i))

# a bot's mood, "away" while the machine it lives on does not answer
def BU.mood.far(away: Bool, +st: M.State, +now: Nat, b: BT.Bot) -> String:
  match away:
    case True{}:
      "away"
    case False{}:
      Mood.bot(st, now, b)

def BU.mood(+u: U.Ui, +b: BT.Bot) -> String:
  BU.mood.far(U.Ui.far.away(u, BT.Bot.id(b)), U.Ui.state(u), U.Side.now(u), b)

# this machine's own bots (not those mirrored from a linked machine)
def BU.here.k(far: Bool, b: BT.Bot, rest: List<&2, BT.Bot>) -> List<&2, BT.Bot>:
  match far:
    case True{}:
      rest
    case False{}:
      b <> rest

def BU.here(bs: List<&2, BT.Bot>) -> List<&2, BT.Bot>:
  match bs:
    case Nil{}:
      Nil{}
    case +b <> t:
      BU.here.k(FR.Fr.is(BT.Bot.id(b)), b, BU.here(t))

def BU.row.here(+st: M.State, +now: Nat, +sel: String, +all: List<&2, BT.Bot>, b: BT.Bot) -> BRow:
  match b:
    case BT.Bot{+i, +n, +th, +lk, pe, c, d}:
      BU.row.m(Mood.bot(st, now, BT.Bot{i, n, th, lk, pe, c, d}), i, BU.label(Nat.is_gt(BU.named(all, n), 1n), n, i), th, lk, String.eq(i, sel))

# a bot on a linked machine whose threads are mirrored here (far.bend): a
# bot like any other, keyed as its machine reports it (its name at the
# link), so the report of it is the same entry; away while its machine is
def BU.row.far(+u: U.Ui, +st: M.State, +now: Nat, +sel: String, b: BT.Bot) -> BRow:
  match b:
    case BT.Bot{+i, +n, +th, +lk, pe, c, d}:
      BU.row.far.m(BU.mood.far(U.Ui.far.away(u, i), st, now, BT.Bot{i, n, th, lk, pe, c, d}), i, n, th, lk, String.eq(i, sel),
        U.Ui.far.name(u, i), FR.Fr.link(i))

def BU.row(+u: U.Ui, +st: M.State, +now: Nat, +sel: String, +all: List<&2, BT.Bot>, +b: BT.Bot) -> BRow:
  Bool.pick(BRow, FR.Fr.is(BT.Bot.id(b)), BU.row.far(u, st, now, sel, b), BU.row.here(st, now, sel, all, b))

def BU.rows.go(bs: List<&2, BT.Bot>, +u: U.Ui, +st: M.State, +now: Nat, +sel: String, +all: List<&2, BT.Bot>) -> List<&2, BRow>:
  match bs:
    case Nil{}:
      Nil{}
    case b <> t:
      BU.row(u, st, now, sel, all, b) <> BU.rows.go(t, u, st, now, sel, all)

# the bots without the one with this id
def BU.sans.k(same: Bool, b: BT.Bot, rest: List<&2, BT.Bot>) -> List<&2, BT.Bot>:
  match same:
    case True{}:
      rest
    case False{}:
      b <> rest

def BU.sans(bs: List<&2, BT.Bot>, +id: String) -> List<&2, BT.Bot>:
  match bs:
    case Nil{}:
      Nil{}
    case +b <> t:
      BU.sans.k(String.eq(BT.Bot.id(b), id), b, BU.sans(t, id))

# the bots without the uberbot's replicas on other machines (US.Ub.far)
# (a replica is a bot of another machine: the others are never looked up)
def BU.sans.far.hit(far: Bool, +st: M.State, +id: String) -> Bool:
  match far:
    case True{}:
      US.Ub.far.is(st, id)
    case False{}:
      False{}

def BU.sans.far(bs: List<&2, BT.Bot>, +st: M.State) -> List<&2, BT.Bot>:
  match bs:
    case Nil{}:
      Nil{}
    case +b <> t:
      BT.Post.keep(BT.Bot, Bool.not(BU.sans.far.hit(FR.Fr.is(BT.Bot.id(b)), st, BT.Bot.id(b))), b, BU.sans.far(t, st))

# the rows without the uberbot's replicas that machines report
def BU.sans.rows(rs: List<&2, BRow>, +st: M.State) -> List<&2, BRow>:
  match rs:
    case Nil{}:
      Nil{}
    case +r <> t:
      BT.Post.keep(BRow, Bool.not(US.Ub.far.named(st, BRow.link(r), BRow.name(r))), r, BU.sans.rows(t, st))

# the uberbot is not in the list of bots (it has its own row, BU.uber), nor
# are its replicas on other machines
def BU.rows.here(+u: U.Ui, +bs: List<&2, BT.Bot>) -> List<&2, BRow>:
  BU.rows.go(BU.sans.far(BU.sans(bs, UB.Uber.bot(U.Ui.state(u))), U.Ui.state(u)), u, U.Ui.state(u), U.Side.now(u), BU.bot.sel(u), BU.here(bs))

def BU.uber.of(m: Maybe<&2, BT.Bot>, +u: U.Ui) -> Maybe<&2, BRow>:
  match m:
    case Some{b}:
      Some{BU.row(u, U.Ui.state(u), U.Side.now(u), BU.bot.sel(u), BU.here(BT.World.bots(M.State.bots(U.Ui.state(u)))), b)}
    case None{}:
      None{}

# the uberbot's row, like any bot's (None while there is none)
def BU.uber(+u: U.Ui) -> Maybe<&2, BRow>:
  BU.uber.of(BT.World.find.id(BT.World.bot(M.State.bots(U.Ui.state(u)), UB.Uber.bot(U.Ui.state(u)))), u)

# the sidebar's cats before each is kept once: this machine's bots (oldest
# first), then those the linked machines report
def BU.rows.all(+u: U.Ui) -> List<&2, O.Ky<BRow>>:
  List.append(&2, O.Ky<BRow>, BU.kys(BU.rows.here(u, BT.World.bots(M.State.bots(U.Ui.state(u))))), BU.kys(BU.sans.rows(BU.remote.all(u), U.Ui.state(u))))

def BU.rows.keyed(+u: U.Ui) -> List<&2, O.Ky<BRow>>:
  O.Once.of(BRow, BU.rows.all(u))

# the sidebar's cats, each bot once (law bot_rows_once)
def BU.rows(+u: U.Ui) -> List<&2, BRow>:
  O.Once.vals(BRow, BU.rows.keyed(u))

# whether any cat in the sidebar moves (the window keeps drawing frames
# only while one does)
def BU.moving(+u: U.Ui) -> Bool:
  BU.moving.go(BU.rows(u))

def BU.row.title.k(remote: Bool, +n: String, +pe: String) -> String:
  match remote:
    case True{}:
      Bool.pick(String, String.is_empty(pe), n, n ++ " · " ++ pe)
    case False{}:
      n

# A bot's view
# ------------
# Every bot shows in one view whatever machine it lives on: its cat, its
# name with where it lives ("Kit · box", U.Ui.far.title), its mood, its
# tabs, then the tab. A bot mirrored from a linked machine (far.bend) is a
# bot like one here, less what stays on its machine: its browser, webhooks
# and settings (BU.tabs.of), its viewer (files on that machine), and
# changes to its memory and routines (only Fr.methods reach its hub: it can
# be written to, stopped, answered, and a routine run). A bot only reported
# by a linked machine (VRemote) is the same view with one tab, Chat: the
# person's conversation with it (laws bot_head_*).

# a row's name as the list shows it: a bot elsewhere with its machine
def BU.row.title(r: BRow) -> String:
  match r:
    case BRow{_, +n, _, _, _, _, _, rem, +pe, _}:
      BU.row.title.k(rem, n, pe)

# whether a bot's header offers its viewer (its thread's files): only for
# a bot here
def BU.viewer.of(+id: String) -> Bool:
  Bool.not(FR.Fr.is(id))

# whether a bot's memory and routines can be changed from here (Forget,
# Edit, Delete, On/Off, a new routine): only a bot here. Run now reaches a
# mirrored bot's hub too.
def BU.owns(+id: String) -> Bool:
  Bool.not(FR.Fr.is(id))

type Head is Data:
  Head{title: String, look: Nat, mood: String, note: String, tabs: List<&2, String>, tab: String, viewer: Bool, owns: Bool}

def Head.title(h: Head) -> String:
  match h:
    case Head{t, _, _, _, _, _, _, _}:
      t

def Head.tabs(h: Head) -> List<&2, String>:
  match h:
    case Head{_, _, _, _, ts, _, _, _}:
      ts

def Head.tab(h: Head) -> String:
  match h:
    case Head{_, _, _, _, _, t, _, _}:
      t

def Head.viewer(h: Head) -> Bool:
  match h:
    case Head{_, _, _, _, _, _, v, _}:
      v

def Head.owns(h: Head) -> Bool:
  match h:
    case Head{_, _, _, _, _, _, _, o}:
      o

def Head.look(h: Head) -> Nat:
  match h:
    case Head{_, l, _, _, _, _, _, _}:
      l

def Head.mood(h: Head) -> String:
  match h:
    case Head{_, _, m, _, _, _, _, _}:
      m

def Head.note(h: Head) -> String:
  match h:
    case Head{_, _, _, n, _, _, _, _}:
      n

# a bot's header: a bot with a thread (remote False), or one only reported
# by a linked machine (remote True: Chat alone)
def BU.head.m.loc(+l: I.Loc, remote: Bool, +u: U.Ui, +id: String, +title: String, +look: Nat, +mood: String) -> Head:
  match remote:
    case True{}:
      Head{title, look, mood, Mood.note.loc(l, mood), ["chat"], "chat", False{}, False{}}
    case False{}:
      Head{title, look, mood, Mood.note.loc(l, mood), BU.tabs.for(u, id), BU.tab.of(u, id), BU.viewer.of(id), BU.owns(id)}

def BU.head.m(remote: Bool, u: U.Ui, id: String, title: String, look: Nat, mood: String) -> Head:
  BU.head.m.loc(I.En{}, remote, u, id, title, look, mood)

# the row a linked machine reports for a bot elsewhere (away, named by its
# id, while none does)
def BU.remote.find(rs: List<&2, BRow>, +id: String) -> BRow:
  match rs:
    case Nil{}:
      BRow{id, id, "", 0n, "away", Mood.note("away"), True{}, True{}, "", ""}
    case BRow{+i, +n, +th, +lk, +mo, +no, +se, +re, +pe, +l} <> t:
      Bool.pick(BRow, String.eq(i, id), BRow{i, n, th, lk, mo, no, se, re, pe, l}, BU.remote.find(t, id))

def BU.remote.row(+u: U.Ui, +id: String) -> BRow:
  BU.remote.find(BU.remote(u), id)

def BRow.look(r: BRow) -> Nat:
  match r:
    case BRow{_, _, _, lk, _, _, _, _, _, _}:
      lk

def BRow.mood(r: BRow) -> String:
  match r:
    case BRow{_, _, _, _, m, _, _, _, _, _}:
      m

def BU.head.remote(+r: BRow, +u: U.Ui, +id: String) -> Head:
  BU.head.m.loc(U.Lang.loc(u), True{}, u, id, BU.row.title(r), BRow.look(r), BRow.mood(r))

def BU.head.bot(+u: U.Ui, b: BT.Bot) -> Head:
  match b:
    case BT.Bot{+i, +n, +th, +lk, +pe, +c, +d}:
      BU.head.m.loc(U.Lang.loc(u), False{}, u, i, U.Ui.far.title(u, i, n), lk, BU.mood(u, BT.Bot{i, n, th, lk, pe, c, d}))

# what a bot's view shows above its tab (a room or the thread: nothing)
def BU.head(+u: U.Ui, v: View) -> Head:
  match v:
    case VBot{b}:
      BU.head.bot(u, b)
    case VRemote{+id}:
      BU.head.remote(BU.remote.row(u, id), u, id)
    case _:
      Head{"", 0n, "away", Mood.note.loc(U.Lang.loc(u), "away"), Nil{}, "chat", False{}, False{}}
