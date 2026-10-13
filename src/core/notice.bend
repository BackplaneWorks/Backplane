import Base
import ./model.bend as M
import ./bot.bend as BT
import ./far.bend as FR

# One item, one alert. When a turn ends, this decides whether it tells the
# person (the hub's APNs push, the phone's own alert) and under which key.
# Alerts with the same key replace each other: "turn-<thread>" for a
# thread's own news, "room-<room>" for a room exchange. Room ids are the
# same on every linked machine, so a shared room has one key everywhere.
#
# - A failed turn always alerts (law notice_fail_always).
# - A thread that is not a bot's alerts when it finishes, as before: that is
#   work the person started (law notice_thread_kept).
# - A bot waiting on the person (an open ask) alerts (law notice_ask_kept).
# - A bot turn that posted in a room (or sent to a bot) is part of that
#   room's exchange, which the person reads in the room: only the first
#   bot post of an exchange alerts (law notice_room_once). An exchange
#   starts at a person's post, or after Notice.window() seconds without a
#   bot post in the room.
# - A bot turn woken by a room or bot message that posted nothing tells the
#   person nothing new, so it is silent (law notice_chatter_quiet).
# - A bot turn the person started in the bot's own thread, or a routine's,
#   alerts as before.
#
# `local` says which bot posts count as the exchange's earlier ones: a hub
# counts its own bots (posts named without "@"), since each hub pushes only
# for its own threads. A phone paired with several hubs counts any bot, so
# the machine whose bot answered first alerts and the others stay quiet.

# how long a room exchange lasts without a bot post (seconds)
def Notice.window() -> Nat:
  300n

# how many threads a Live Activity lists (the rest are "+ N more" from its
# running count): its lock-screen view is at most 160 points tall, and a
# hub's APNs push of it at most 4 KB, so every one of many never fits (law
# island_lines_capped)
def Notice.island.max() -> Nat:
  3n

type Notice is Data:
  Notice{kind: String, key: String}

def Notice.kind(n: Notice) -> String:
  match n:
    case Notice{k, _}:
      k

def Notice.key(n: Notice) -> String:
  match n:
    case Notice{_, k}:
      k

# 0: no alert, 1: an alert ("done"), 2: a failure ("fail")
def Notice.decide(failed: Bool, bot: Bool, asking: Bool, posted: Bool, first: Bool, msg: Bool) -> Nat:
  match failed bot asking posted first msg:
    case True{} _ _ _ _ _:
      2n
    case False{} False{} _ _ _ _:
      1n
    case False{} True{} True{} _ _ _:
      1n
    case False{} True{} False{} True{} True{} _:
      1n
    case False{} True{} False{} True{} False{} _:
      0n
    case False{} True{} False{} False{} _ True{}:
      0n
    case False{} True{} False{} False{} _ False{}:
      1n

# the same while the thread still waits on work (busy: model.bend's
# Work.busy, its unfinished child tasks, background and watched work):
# a finished turn tells nothing yet (the turn that ends the work will),
# but a failure or a question for the person still alerts (laws
# notice_busy_*)
def Notice.decide.b(quiet: Bool, failed: Bool, bot: Bool, asking: Bool, posted: Bool, first: Bool, msg: Bool) -> Nat:
  match quiet:
    case True{}:
      0n
    case False{}:
      Notice.decide(failed, bot, asking, posted, first, msg)

def Notice.decide.w(+busy: Nat, +failed: Bool, bot: Bool, +asking: Bool, posted: Bool, first: Bool, msg: Bool) -> Nat:
  Notice.decide.b(Bool.and(Nat.is_gt(busy, 0n), Bool.and(Bool.not(failed), Bool.not(asking))), failed, bot, asking, posted, first, msg)

def Notice.name(n: Nat) -> String:
  match n:
    case 0n:
      ""
    case 1n:
      "done"
    case _:
      "fail"

# This turn
# ---------

# what a thread's current turn did, read back from its entries (newest
# first) down to the person's (or a bot's) message that started it
type Turn is Data:
  Turn{posted: Bool, trigger: String, start: Nat}

def Turn.posts(+kind: String) -> Bool:
  Bool.or(String.contains(kind, "room_post"), String.contains(kind, "bot_send"))

def Turn.user(hit: Bool, text: String, at: Nat, acc: Turn) -> Turn:
  match hit acc:
    case True{} Turn{p, _, _}:
      Turn{p, text, at}
    case False{} _:
      acc

def Turn.act(hit: Bool, acc: Turn) -> Turn:
  match hit acc:
    case True{} Turn{_, t, s}:
      Turn{True{}, t, s}
    case False{} _:
      acc

# the walk stops at the thread's newest user message (done)
def Turn.scan(es: List<&2, M.Entry>, +id: String, done: Bool, acc: Turn) -> Turn:
  match es done:
    case _ True{}:
      acc
    case Nil{} False{}:
      acc
    case M.Msg{_, +th, M.User{}, text, at} <> t False{}:
      Turn.scan(t, id, String.eq(th, id), Turn.user(String.eq(th, id), text, at, acc))
    case M.Act{_, +th, _, +kind, _, _} <> t False{}:
      Turn.scan(t, id, False{}, Turn.act(Bool.and(String.eq(th, id), Turn.posts(kind)), acc))
    case _ <> t False{}:
      Turn.scan(t, id, False{}, acc)

def Turn.of(es: List<&2, M.Entry>, +id: String) -> Turn:
  Turn.scan(es, id, False{}, Turn{False{}, "", 0n})

def Turn.posted(t: Turn) -> Bool:
  match t:
    case Turn{p, _, _}:
      p

def Turn.start(t: Turn) -> Nat:
  match t:
    case Turn{_, _, s}:
      s

# woken by a message from a room or a bot (bothub.bend's Bots.note), or by
# a report from a thread it launched (hub.bend's Lr.head): the launched
# thread alerted for itself, so a bot turn that only read the report and
# answered nothing new to the person is chatter (law notice_report_quiet)
def Turn.msg(t: Turn) -> Bool:
  match t:
    case Turn{_, +tr, _}:
      Bool.or(String.starts_with(tr, "[message from "), String.starts_with(tr, "[Backplane] Launched thread "))

# The exchange
# ------------

def Post.from(p: BT.Post) -> String:
  match p:
    case BT.Post{_, _, f, _, _, _}:
      f

def Post.at(p: BT.Post) -> Nat:
  match p:
    case BT.Post{_, _, _, _, _, a}:
      a

def Post.room(p: BT.Post) -> String:
  match p:
    case BT.Post{_, r, _, _, _, _}:
      r

# the person, here ("you") or on a linked machine ("you@desktop")
def Ex.person(+from: String) -> Bool:
  Bool.or(String.eq(from, "you"), String.starts_with(from, "you@"))

def Ex.bot(+from: String, local: Bool) -> Bool:
  match local:
    case True{}:
      Bool.not(String.contains(from, "@"))
    case False{}:
      True{}

# is the list's head this turn's own post?
def Ex.hit(ps: List<&2, BT.Post>, +name: String, +start: Nat) -> Bool:
  match ps:
    case Nil{}:
      False{}
    case BT.Post{_, _, +f, _, _, +at} <> _:
      Bool.and(String.eq(f, name), Nat.is_ge(at, start))

# the posts from this turn's newest own post on (posts are newest first)
def Ex.find(ps: List<&2, BT.Post>, hit: Bool, +name: String, +start: Nat) -> List<&2, BT.Post>:
  match ps hit:
    case Nil{} _:
      Nil{}
    case +p <> +t True{}:
      p <> t
    case _ <> t False{}:
      Ex.find(t, Ex.hit(t, name, start), name, start)

type Vd is Data:
  VGo{}
  VYes{}
  VNo{}

def Vd.room(same: Bool, person: Bool, own: Bool, bot: Bool) -> Vd:
  match same person own bot:
    case False{} _ _ _:
      VGo{}
    case True{} True{} _ _:
      VYes{}
    case True{} False{} True{} _:
      VGo{}
    case True{} False{} False{} True{}:
      VNo{}
    case True{} False{} False{} False{}:
      VGo{}

def Vd.post(old: Bool, +room: String, +name: String, +start: Nat, local: Bool, p: BT.Post) -> Vd:
  match old p:
    case True{} _:
      VYes{}
    case False{} BT.Post{_, +r, +f, _, _, +at}:
      Vd.room(String.eq(r, room), Ex.person(f), Bool.and(String.eq(f, name), Nat.is_ge(at, start)), Ex.bot(f, local))

# what an older post says about whether this turn's post opens the exchange
def Vd.of(ps: List<&2, BT.Post>, +room: String, +at: Nat, +name: String, +start: Nat, local: Bool) -> Vd:
  match ps:
    case Nil{}:
      VYes{}
    case +p <> _:
      Vd.post(Nat.is_le(Nat.add(Post.at(p), Notice.window()), at), room, name, start, local, p)

# older posts, newest first: the exchange opened with this post when a
# person's post or the window's edge comes before any other bot's post
def Ex.first(ps: List<&2, BT.Post>, v: Vd, +room: String, +at: Nat, +name: String, +start: Nat, +local: Bool) -> Bool:
  match ps v:
    case _ VYes{}:
      True{}
    case _ VNo{}:
      False{}
    case Nil{} VGo{}:
      True{}
    case _ <> +t VGo{}:
      Ex.first(t, Vd.of(t, room, at, name, start, local), room, at, name, start, local)

# where this turn posted: its room ("" when it posted nowhere) and whether
# that post opened the room's exchange
type Ex is Data:
  Ex{room: String, first: Bool}

def Ex.of.k(found: List<&2, BT.Post>, +name: String, +start: Nat, +local: Bool) -> Ex:
  match found:
    case Nil{}:
      Ex{"", False{}}
    case BT.Post{_, +r, _, _, _, +at} <> +t:
      Ex{r, Ex.first(t, Vd.of(t, r, at, name, start, local), r, at, name, start, local)}

def World.posts.all(w: BT.World) -> List<&2, BT.Post>:
  match w:
    case BT.World{_, _, ps, _, _, _, _, _, _, _, _, _}:
      ps

def Ex.of(+w: BT.World, +name: String, +start: Nat, local: Bool) -> Ex:
  Ex.of.k(Ex.find(World.posts.all(w), Ex.hit(World.posts.all(w), name, start), name, start), name, start, local)

# Asks
# ----

def Ask.waits(+thread: String, a: M.Ask) -> Bool:
  match a:
    case M.Ask{_, +th, _, _, _, _, +open, _}:
      Bool.and(open, String.eq(th, thread))

def Ask.any(xs: List<&2, M.Ask>, +thread: String) -> Bool:
  match xs:
    case Nil{}:
      False{}
    case a <> t:
      Bool.or(Ask.waits(thread, a), Ask.any(t, thread))

def Ask.open(s: M.State, +thread: String) -> Bool:
  match s:
    case M.State{_, _, _, _, _, _, _, ak, _, _, _}:
      Ask.any(Map.values(&2, M.Ask, ak), thread)

# The decision
# ------------

# A thread elsewhere is keyed by its id on its own machine, the key the
# owning hub uses for it, so the same turn alerts once however many of the
# owner's machines and clients raise it: alerts with one key replace each
# other (laws notice_far_key_same, notice_far_room_key_same)
def Notice.turn(+thread: String) -> String:
  "turn-" ++ FR.Fr.local(thread)

def Notice.room.key(n: Nat, +room: String, +thread: String) -> String:
  match n:
    case 1n:
      Bool.pick(String, String.is_empty(room), Notice.turn(thread), "room-" ++ room)
    case _:
      Notice.turn(thread)

def Notice.of.bot.n(+n: Nat, own: Bool, +room: String, +thread: String) -> Notice:
  match own:
    case True{}:
      Notice{Notice.name(n), Notice.turn(thread)}
    case False{}:
      Notice{Notice.name(n), Notice.room.key(n, room, thread)}

def Notice.of.bot.k(+busy: Nat, +failed: Bool, +asking: Bool, +t: Turn, e: Ex, +thread: String) -> Notice:
  match e:
    case Ex{+room, first}:
      Notice.of.bot.n(Notice.decide.w(busy, failed, True{}, asking, Bool.and(Turn.posted(t), Bool.not(String.is_empty(room))), first, Turn.msg(t)),
        Bool.or(failed, asking), room, thread)

def Notice.of.bot.t(+s: M.State, +t: Turn, +name: String, +thread: String, +failed: Bool, local: Bool) -> Notice:
  Notice.of.bot.k(M.Work.busy(s, thread), failed, Ask.open(s, thread), t, Ex.of(M.State.bots(s), name, Turn.start(t), local), thread)

def Notice.of.bot(+s: M.State, b: BT.Bot, +thread: String, +failed: Bool, local: Bool) -> Notice:
  match b:
    case BT.Bot{_, +name, _, _, _, _, _}:
      Notice.of.bot.t(s, Turn.of(M.State.entries.all(s), thread), name, thread, failed, local)

def Notice.of.k(m: Maybe<&2, BT.Bot>, +s: M.State, +thread: String, +failed: Bool, local: Bool) -> Notice:
  match m:
    case Some{b}:
      Notice.of.bot(s, b, thread, failed, local)
    case None{}:
      Notice{Notice.name(Notice.decide.w(M.Work.busy(s, thread), failed, False{}, Ask.open(s, thread), False{}, False{}, False{})), Notice.turn(thread)}

# what one ended turn tells the person: kind "" (nothing), "done" or
# "fail", and the key its alert replaces others under
def Notice.of(+s: M.State, +thread: String, +failed: Bool, local: Bool) -> Notice:
  Notice.of.k(BT.World.of_thread(M.State.bots(s), thread), s, thread, failed, local)
