import Base
import ./model.bend as M
import ./bot.bend as BT
import ./far.bend as FR
import ./json.bend as J

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

def Turn.trigger(t: Turn) -> String:
  match t:
    case Turn{_, tr, _}:
      tr

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
      Notice.of.bot.t(s, Turn.of(M.State.mine(s, thread), thread), name, thread, failed, local)

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

# Messages waiting behind this turn are not its trigger. Legacy Notice.of
# retains its rule; bot enforcement reads this queued-aware version.
def Turn.skip(qs: List<&2, M.QItem>, acc: String) -> String:
  match qs:
    case Nil{}:
      acc
    case M.QItem{m, _} <> t:
      Turn.skip(t, acc ++ "\n" ++ m ++ "\n")

def Turn.queued(+skip: String, +id: String) -> Bool:
  String.contains(skip, "\n" ++ id ++ "\n")

def Turn.scan.skip(es: List<&2, M.Entry>, +id: String, +skip: String, done: Bool, acc: Turn) -> Turn:
  match es done:
    case _ True{}:
      acc
    case Nil{} False{}:
      acc
    case M.Msg{+mid, +th, M.User{}, text, at} <> t False{}:
      +hit = Bool.and(String.eq(th, id), Bool.not(Turn.queued(skip, mid)))
      Turn.scan.skip(t, id, skip, hit, Turn.user(hit, text, at, acc))
    case M.Act{_, +th, _, +kind, _, _} <> t False{}:
      Turn.scan.skip(t, id, skip, False{}, Turn.act(Bool.and(String.eq(th, id), Turn.posts(kind)), acc))
    case _ <> t False{}:
      Turn.scan.skip(t, id, skip, False{}, acc)

def Turn.of.state(+s: M.State, +thread: String) -> Turn:
  Turn.scan.skip(M.State.mine(s, thread), thread, Turn.skip(M.Queue.items(s, thread), ""), False{}, Turn{False{}, "", 0n})

# Policy
# ------
# The person is alerted only when needed (an ask), when work settles (a
# turn completed with everything the thread waits on done, or its branch
# merged), or for a reminder or digest they scheduled. Everything above
# (Notice.of) is the older rule, kept for notify.all. The hub writes each
# alert as the thread's record (setting alert.<thread>) in the step that
# causes it; every client follows the record (laws policy_*).

# the setting notify.<name>: needs, settled and reminders default on, all
# off; an unknown name is off
def Policy.known(+name: String) -> Bool:
  Bool.or(Bool.or(String.eq(name, "needs"), String.eq(name, "settled")), Bool.or(String.eq(name, "reminders"), String.eq(name, "all")))

def Policy.dflt(+name: String) -> Bool:
  Bool.not(String.eq(name, "all"))

def Policy.on.k(known: Bool, dflt: Bool, +v: String) -> Bool:
  Bool.and(known, Bool.pick(Bool, String.eq(v, "on"), True{}, Bool.pick(Bool, String.eq(v, "off"), False{}, dflt)))

def Policy.on(s: M.State, +name: String) -> Bool:
  Policy.on.k(Policy.known(name), Policy.dflt(name), M.Setting.get(s, "notify." ++ name, ""))

# The alert record
# ----------------
# `<kind>\t<key>\t<settled>\t<stamp>\t<text>`: kind ask, done, fail, merged,
# note or "" (a settle that alerted no one); key the alert's collapse id;
# settled the id of the last person message whose work settled; stamp
# `<now>.<seq>`, so two records are never equal; text for note, ask and
# merged (the url).

type AlertRec is Data:
  AlertRec{kind: String, key: String, settled: String, stamp: String, text: String}

type Cut is Data:
  Cut{field: String, rest: String}

def Alert.key(+thread: String) -> String:
  "alert." ++ thread

def Alert.line(+kind: String, +key: String, +settled: String, +stamp: String, +text: String) -> String:
  kind ++ "\t" ++ key ++ "\t" ++ settled ++ "\t" ++ stamp ++ "\t" ++ text

def Alert.seq(s: M.State) -> Nat:
  match s:
    case M.State{_, _, _, n, _, _, _, _, _, _, _}:
      n

# a text as one line, cut to max characters
def Alert.clean(+t: String) -> String:
  match t:
    case SNil{}:
      ""
    case SCon{+c, r}:
      Bool.pick(String, Bool.or(Char.is_eq(c, Chr{9}), Char.is_eq(c, Chr{10})), " ", SCon{c, SNil{}}) ++ Alert.clean(r)

def Alert.text(+t: String, +max: Nat) -> String:
  Alert.clean(String.take(String.trim(t), max))

def Alert.set(s: M.State, +now: Nat, +thread: String, +kind: String, +key: String, +settled: String, +text: String) -> M.Change:
  M.SettingSet{Alert.key(thread), Alert.line(kind, key, settled, Nat.show(now) ++ "." ++ Nat.show(Alert.seq(s)), text), now}

# where the first tab is (the length when none), one tail walk
def Alert.tab(s: String, stop: Bool, +n: Nat) -> Nat:
  match s stop:
    case _ True{}:
      n
    case SNil{} False{}:
      n
    case SCon{Chr{+c}, t} False{}:
      Alert.tab(t, Nat.is_eq(U32.to_nat(c), 9n), Bool.pick(Nat, Nat.is_eq(U32.to_nat(c), 9n), n, 1n+n))

def Alert.cut.k(+s: String, +i: Nat) -> Cut:
  Cut{String.take(s, i), String.drop(s, 1n+i)}

# the first field (up to a tab) and the rest
def Alert.cut(+s: String) -> Cut:
  Alert.cut.k(s, Alert.tab(s, False{}, 0n))

def Alert.p4(+kind: String, +key: String, +settled: String, c: Cut) -> AlertRec:
  match c:
    case Cut{+stamp, +text}:
      AlertRec{kind, key, settled, stamp, text}

def Alert.p3(+kind: String, +key: String, c: Cut) -> AlertRec:
  match c:
    case Cut{+settled, r}:
      Alert.p4(kind, key, settled, Alert.cut(r))

def Alert.p2(+kind: String, c: Cut) -> AlertRec:
  match c:
    case Cut{+key, r}:
      Alert.p3(kind, key, Alert.cut(r))

def Alert.p1(c: Cut) -> AlertRec:
  match c:
    case Cut{+kind, r}:
      Alert.p2(kind, Alert.cut(r))

def Alert.parse(v: String) -> AlertRec:
  Alert.p1(Alert.cut(v))

# an absent record reads as all empty
def Alert.get(s: M.State, +thread: String) -> AlertRec:
  Alert.parse(M.Setting.get(s, Alert.key(thread), ""))

def Alert.kind(r: AlertRec) -> String:
  match r:
    case AlertRec{k, _, _, _, _}:
      k

def Alert.rkey(r: AlertRec) -> String:
  match r:
    case AlertRec{_, k, _, _, _}:
      k

def Alert.rsettled(r: AlertRec) -> String:
  match r:
    case AlertRec{_, _, s, _, _}:
      s

def Alert.rstamp(r: AlertRec) -> String:
  match r:
    case AlertRec{_, _, _, s, _}:
      s

def Alert.rtext(r: AlertRec) -> String:
  match r:
    case AlertRec{_, _, _, _, t}:
      t

# the settle memory
def Alert.memory(s: M.State, +thread: String) -> String:
  Alert.rsettled(Alert.get(s, thread))

# the leading digits of a stamp (when the record was written)
def Alert.digit(+c: U32) -> Bool:
  Bool.and(Nat.is_ge(U32.to_nat(c), 48n), Nat.is_le(U32.to_nat(c), 57n))

def Alert.num(s: String, +acc: Nat, go: Bool) -> Nat:
  match s go:
    case _ False{}:
      acc
    case SNil{} True{}:
      acc
    case SCon{Chr{+c}, t} True{}:
      Alert.num(t, Bool.pick(Nat, Alert.digit(c), Nat.add(Nat.mul(acc, 10n), U32.to_nat((c - 48 : U32))), acc), Alert.digit(c))

def Alert.at(r: AlertRec) -> Nat:
  Alert.num(Alert.rstamp(r), 0n, True{})

# kinds a watching desk holds (a note never is)
def Alert.holds(+kind: String) -> Bool:
  Bool.or(Bool.or(String.eq(kind, "ask"), String.eq(kind, "done")), Bool.or(String.eq(kind, "fail"), String.eq(kind, "merged")))

# the record's alert for a client: Notice{kind, key} ("" kind: none)
def Alert.notice(+r: AlertRec) -> Notice:
  Notice{Alert.kind(r), Alert.rkey(r)}

# The person's message
# --------------------

type Per is Data:
  Per{id: String, text: String, at: Nat}

def Per.none() -> Per:
  Per{"", "", 0n}

def Per.id(p: Per) -> String:
  match p:
    case Per{i, _, _}:
      i

def Per.text(p: Per) -> String:
  match p:
    case Per{_, t, _}:
      t

# typed by the person: not a link (a message a thread sent) and not a
# bracketed note ([message from …], [routine: …], [Backplane] …), except
# the person typing from another machine
def Policy.direct(+id: String, +text: String) -> Bool:
  Bool.and(String.is_empty(M.Link.from(id)),
    Bool.or(Bool.not(String.starts_with(text, "[")), String.starts_with(text, "[message from person@")))

def Policy.person.pick(hit: Bool, p: Per, acc: Per) -> Per:
  match hit:
    case True{}:
      p
    case False{}:
      acc

def Policy.person.go(es: List<&2, M.Entry>, +thread: String, done: Bool, acc: Per) -> Per:
  match es done:
    case _ True{}:
      acc
    case Nil{} False{}:
      acc
    case M.Msg{+id, +th, M.User{}, +text, +at} <> t False{}:
      Policy.person.go(t, thread, Bool.and(String.eq(th, thread), Policy.direct(id, text)), Policy.person.pick(Bool.and(String.eq(th, thread), Policy.direct(id, text)), Per{id, text, at}, acc))
    case _ <> t False{}:
      Policy.person.go(t, thread, False{}, acc)

# the newest message the person typed in a thread
def Policy.person(s: M.State, +thread: String) -> Per:
  Policy.person.go(M.State.mine(s, thread), thread, False{}, Per.none())

# the thread's person message has not settled yet
def Policy.fresh(+p: Per, +mem: String) -> Bool:
  Bool.and(Bool.not(String.is_empty(Per.id(p))), Bool.not(String.eq(Per.id(p), mem)))

# no thread it launched with reports on still works
def Policy.launched(+post: M.State, ids: List<&2, String>, ok: Bool) -> Bool:
  match ids ok:
    case _ False{}:
      False{}
    case Nil{} True{}:
      True{}
    case +id <> t True{}:
      Policy.launched(post, t, Bool.not(Bool.and(M.Launch.reports(post, id), Bool.not(M.Work.ready(post, id)))))

# the thread's work is all done: it does not run, nothing is queued,
# nothing is out, no ask is open, no reporting helper works
def Policy.ready(+post: M.State, +thread: String) -> Bool:
  Bool.and(Bool.and(M.Work.ready(post, thread), Bool.not(Ask.open(post, thread))), Policy.launched(post, M.Launch.of(post, thread), True{}))

# The decision
# ------------

# 0 nothing, 1 done, 2 fail, 3 the episode ends quietly (stopped)
def Policy.decide(fresh: Bool, +ready: Bool, +state: String) -> Nat:
  match fresh:
    case False{}:
      0n
    case True{}:
      Bool.pick(Nat, String.eq(state, "completed"), Bool.pick(Nat, ready, 1n, 0n),
        Bool.pick(Nat, String.eq(state, "failed"), 2n, Bool.pick(Nat, Bool.and(ready, Bool.or(String.eq(state, "interrupted"), String.eq(state, "stopped"))), 3n, 0n)))

def Policy.fin.m(same: Bool, +post: M.State, +now: Nat, +thread: String, +mem: String) -> List<&2, M.Change>:
  match same:
    case True{}:
      Nil{}
    case False{}:
      [Alert.set(post, now, thread, "", "", mem, "")]

# no alert: the memory moves, if it did
def Policy.fin.q(+post: M.State, +now: Nat, +thread: String, +mem: String, +old: String) -> List<&2, M.Change>:
  Policy.fin.m(String.eq(mem, old), post, now, thread, mem)

def Policy.fin.k(none: Bool, +kind: String, +key: String, +post: M.State, +now: Nat, +thread: String, +mem: String, +old: String) -> List<&2, M.Change>:
  match none:
    case True{}:
      Policy.fin.q(post, now, thread, mem, old)
    case False{}:
      [Alert.set(post, now, thread, kind, key, mem, "")]

def Policy.fin.n(n: Notice, +post: M.State, +now: Nat, +thread: String, +mem: String, +old: String) -> List<&2, M.Change>:
  match n:
    case Notice{+kind, +key}:
      Policy.fin.k(String.is_empty(kind), kind, key, post, now, thread, mem, old)

def Policy.fin.l(all: Bool, +post: M.State, +now: Nat, +thread: String, +failed: Bool, +mem: String, +old: String) -> List<&2, M.Change>:
  match all:
    case True{}:
      Policy.fin.n(Notice.of(post, thread, failed, True{}), post, now, thread, mem, old)
    case False{}:
      Policy.fin.q(post, now, thread, mem, old)

# an alert of kind (when its setting is on), else the quiet record; with
# notify.all the older rule may still alert
def Policy.settle.k(on: Bool, +kind: String, +post: M.State, +now: Nat, +thread: String, +mem: String, +old: String) -> List<&2, M.Change>:
  match on:
    case True{}:
      [Alert.set(post, now, thread, kind, Notice.turn(thread), mem, "")]
    case False{}:
      Policy.fin.l(Policy.on(post, "all"), post, now, thread, String.eq(kind, "fail"), mem, old)

def Policy.end.n(n: Nat, +post: M.State, +now: Nat, +thread: String, +pid: String, +mem: String, +failed: Bool) -> List<&2, M.Change>:
  match n:
    case 0n:
      Policy.fin.l(Policy.on(post, "all"), post, now, thread, failed, mem, mem)
    case 1n:
      Policy.settle.k(Policy.on(post, "settled"), "done", post, now, thread, pid, mem)
    case 2n:
      Policy.settle.k(Policy.on(post, "settled"), "fail", post, now, thread, pid, mem)
    case _:
      Policy.fin.l(Policy.on(post, "all"), post, now, thread, failed, pid, mem)

def Policy.end.p(p: Per, +post: M.State, +now: Nat, +thread: String, +state: String) -> List<&2, M.Change>:
  match p:
    case Per{+pid, _, _}:
      Policy.end.n(Policy.decide(Policy.fresh(p, Alert.memory(post, thread)), Policy.ready(post, thread), state), post, now, thread, pid, Alert.memory(post, thread), String.eq(state, "failed"))

# what a turn's end records (post: the state after the step; state:
# completed, failed, interrupted or stopped): zero or one setting
def Policy.end(+post: M.State, +now: Nat, +thread: String, +state: String) -> List<&2, M.Change>:
  Policy.end.p(Policy.person(post, thread), post, now, thread, state)

# what an ask's alert says: a paused browser task (core/teach.bend's pause,
# tool task_pause) its one-line question, an order's question or approval
# (core/order.bend's order_ask, order_quote: plain words already) its
# detail, any other its tool and detail
def Alert.ask.say.k(pause: Bool, plain: Bool, +tool: String, +detail: String) -> String:
  match pause:
    case True{}:
      J.Json.get_str(Maybe.default(&2, J.Json, J.Json.parse(detail), J.Null{}), "question", "")
    case False{}:
      Bool.pick(String, Bool.or(plain, String.is_empty(tool)), detail, tool ++ " " ++ detail)

def Alert.ask.say(+tool: String, +detail: String) -> String:
  Alert.ask.say.k(String.eq(tool, "task_pause"), Bool.or(String.eq(tool, "order_ask"), String.eq(tool, "order_quote")), tool, detail)

def Policy.ask.k(on: Bool, +post: M.State, +now: Nat, +thread: String, +tool: String, +detail: String) -> List<&2, M.Change>:
  match on:
    case True{}:
      [Alert.set(post, now, thread, "ask", "ask-" ++ FR.Fr.local(thread), Alert.memory(post, thread),
        Alert.text(Alert.ask.say(tool, detail), 180n))]
    case False{}:
      [Alert.set(post, now, thread, "", "", Alert.memory(post, thread), "")]

# an ask opened for the person
def Policy.ask(+post: M.State, +now: Nat, +thread: String, +tool: String, +detail: String) -> List<&2, M.Change>:
  Policy.ask.k(Policy.on(post, "needs"), post, now, thread, tool, detail)

def Policy.merged.m(+pid: String, +old: String) -> String:
  Bool.pick(String, String.is_empty(pid), old, pid)

def Policy.merged.a(on: Bool, +post: M.State, +now: Nat, +thread: String, +url: String, +mem: String) -> List<&2, M.Change>:
  match on:
    case True{}:
      [Alert.set(post, now, thread, "merged", Notice.turn(thread), mem, Alert.text(url, 300n))]
    case False{}:
      [Alert.set(post, now, thread, "", "", mem, "")]

def Policy.merged.k(on: Bool, p: Per, +post: M.State, +now: Nat, +thread: String, +url: String) -> List<&2, M.Change>:
  match p:
    case Per{+pid, _, _}:
      Policy.merged.a(on, post, now, thread, url, Policy.merged.m(pid, Alert.memory(post, thread)))

def Policy.reminder.k(on: Bool, +s: M.State, +now: Nat, +thread: String, +id: String, +text: String) -> List<&2, M.Change>:
  match on:
    case True{}:
      [Alert.set(s, now, thread, "reminder", "remind-" ++ id, Alert.memory(s, thread), Alert.text(text, 500n))]
    case False{}:
      [Alert.set(s, now, thread, "", "", Alert.memory(s, thread), "")]

# a reminder the person scheduled came due (remind_person's Reminded): the
# record, key remind-<id>, never held by a desk; the memory is left alone
def Policy.reminder(+s: M.State, +now: Nat, +thread: String, +id: String, +text: String) -> List<&2, M.Change>:
  Policy.reminder.k(Policy.on(s, "reminders"), s, now, thread, id, text)

# the thread's branch merged: the work settled
def Policy.merged(+post: M.State, +now: Nat, +thread: String, +url: String) -> List<&2, M.Change>:
  Policy.merged.k(Policy.on(post, "settled"), Policy.person(post, thread), post, now, thread, url)

def Alert.body.k(+kind: String, +text: String) -> String:
  Bool.pick(String, String.eq(kind, "merged"), "Merged: " ++ Bool.pick(String, String.is_empty(text), "its branch", text),
    Bool.pick(String, String.eq(kind, "ask"), "Needs you" ++ Bool.pick(String, String.is_empty(text), "", ": " ++ text), text))

# the body of a record's alert when the record has text of its own
def Alert.body(+r: AlertRec) -> String:
  Alert.body.k(Alert.kind(r), Alert.rtext(r))

# Policy examples (for the laws and tests)
# ----------------------------------------

def Policy.val.pick(hit: Bool, +v: String, +acc: String) -> String:
  match hit:
    case True{}:
      v
    case False{}:
      acc

# the first alert record in a list of changes ("" when none)
def Policy.val(cs: List<&2, M.Change>, done: Bool, +acc: String) -> String:
  match cs done:
    case _ True{}:
      acc
    case Nil{} False{}:
      acc
    case M.SettingSet{+k, +v, _} <> t False{}:
      Policy.val(t, String.starts_with(k, "alert."), Policy.val.pick(String.starts_with(k, "alert."), v, acc))
    case _ <> t False{}:
      Policy.val(t, False{}, acc)

def Policy.said.k(none: Bool, +kind: String) -> String:
  Bool.pick(String, none, "-", Bool.pick(String, String.is_empty(kind), "_", kind))

def Policy.said.v(+v: String) -> String:
  Policy.said.k(String.is_empty(v), Alert.kind(Alert.parse(v)))

# what a list of changes records: "-" nothing, "_" a record with no kind,
# else its kind
def Policy.said(cs: List<&2, M.Change>) -> String:
  Policy.said.v(Policy.val(cs, False{}, ""))

# the kind of the alert record in a list of changes ("" none, or no kind)
def Policy.kind(cs: List<&2, M.Change>) -> String:
  Alert.kind(Alert.parse(Policy.val(cs, False{}, "")))

# does a list of changes hold a record that alerts
def Policy.loud(+cs: List<&2, M.Change>) -> Bool:
  Bool.not(Bool.or(String.eq(Policy.said(cs), "-"), String.eq(Policy.said(cs), "_")))

def Policy.demo.s(cs: List<&2, M.Change>) -> M.State:
  M.State.replay(cs, M.State.empty())

def Policy.demo.cat(xss: List<&2, List<&2, M.Change>>) -> List<&2, M.Change>:
  match xss:
    case Nil{}:
      Nil{}
    case xs <> t:
      List.append(&2, M.Change, xs, Policy.demo.cat(t))

# two threads: t1 (the person's) and t0 (its launcher in some examples)
def Policy.demo.base() -> List<&2, M.Change>:
  [M.ProjectCreated{"p1", "p", "/p", 1n}, M.ThreadCreated{"t1", "p1", "x", M.Local{}, "", "", "claude", "", 1n},
   M.ThreadCreated{"t0", "p1", "boss", M.Local{}, "", "", "claude", "", 1n}]

# a message to a thread and its turn running
def Policy.demo.run(+th: String, +id: String, +text: String, +at: Nat) -> List<&2, M.Change>:
  [M.MessagePosted{th, id, M.User{}, text, at}, M.TurnChanged{th, M.Running{}, at}]

# the turn answers and completes
def Policy.demo.done(+th: String, +at: Nat) -> List<&2, M.Change>:
  [M.MessagePosted{th, "a" ++ Nat.show(at), M.Assistant{}, "ok", at}, M.TurnChanged{th, M.Completed{}, at}]

def Policy.demo.fail(+th: String, +at: Nat) -> List<&2, M.Change>:
  [M.TurnChanged{th, M.Failed{}, at}]

# the person asked t1 something and its turn completed
def Policy.demo.ask() -> List<&2, M.Change>:
  Policy.demo.cat([Policy.demo.base(), Policy.demo.run("t1", "m1", "do it", 10n), Policy.demo.done("t1", 12n)])

# the record a turn end writes, folded in
def Policy.demo.fold(+s: M.State, cs: List<&2, M.Change>, more: List<&2, M.Change>) -> M.State:
  M.State.replay(List.append(&2, M.Change, cs, more), s)

# a bot (Bub, thread tb) in a room, woken by a message that is not the person's
def Policy.demo.bot() -> List<&2, M.Change>:
  [M.ThreadCreated{"tb", "", "Bub", M.Local{}, "", "", "claude", "", 100n}, M.BotSet{"b1", "Bub", "tb", 0n, "", 100n}, M.RoomSet{"r1", "ring", "b1", 100n}]

def Policy.demo.room() -> List<&2, M.Change>:
  Policy.demo.cat([Policy.demo.bot(), [M.RoomPosted{"p0", "r1", "you", "hi", 0n, 1000n}],
    Policy.demo.run("tb", "u1", "[message from you in #ring]\nhi", 1000n),
    [M.ActivityLogged{"tb", "a1", "tool", "mcp__backplane__room_post", "mcp__backplane__room_post", 1005n}, M.RoomPosted{"p1", "r1", "Bub", "hello", 2n, 1005n}],
    Policy.demo.done("tb", 1006n)])

def Policy.demo.count(bs: List<&2, Bool>) -> Nat:
  match bs:
    case Nil{}:
      0n
    case b <> t:
      Nat.add(Bool.pick(Nat, b, 1n, 0n), Policy.demo.count(t))

# the cases that alert no one: a bot woken by a room message that posts, a
# routine's turn, a launched thread's end that reports to its launcher, a
# delegated task's note, an intermediate end with background work out, a
# stop (counted: the cases that alert)
def Policy.demo.quiet() -> Nat:
  +room = Policy.loud(Policy.end(Policy.demo.s(Policy.demo.room()), 1007n, "tb", "completed"))
  +routine = Policy.loud(Policy.end(Policy.demo.s(Policy.demo.cat([Policy.demo.base(), Policy.demo.run("t1", "r1", "[routine: daily]\ngo", 10n), Policy.demo.done("t1", 12n)])), 13n, "t1", "completed"))
  +launched = Policy.loud(Policy.end(Policy.demo.s(Policy.demo.cat([Policy.demo.base(), [M.ThreadLaunched{"t1", "t0", True{}, 2n}],
    Policy.demo.run("t0", "m0", "do it", 3n), Policy.demo.run("t1", M.Link.id("t0", "m1"), "go", 4n), Policy.demo.done("t1", 12n)])), 13n, "t1", "completed"))
  +task = Policy.loud(Policy.end(Policy.demo.s(Policy.demo.cat([Policy.demo.base(), Policy.demo.run("t1", "n1", "[Backplane] Task finished: x", 10n), Policy.demo.done("t1", 12n)])), 13n, "t1", "completed"))
  +bg = Policy.loud(Policy.end(Policy.demo.s(Policy.demo.cat([Policy.demo.ask(), [M.WorkSet{"t1", "bg", "a\tx", 12n}]])), 13n, "t1", "completed"))
  +stop = Policy.loud(Policy.end(Policy.demo.s(Policy.demo.cat([Policy.demo.base(), Policy.demo.run("t1", "m1", "do it", 10n), [M.TurnChanged{"t1", M.Interrupted{}, 12n}]])), 13n, "t1", "stopped"))
  Policy.demo.count([room, routine, launched, task, bg, stop])

def Policy.demo.once.k(+cs: List<&2, M.Change>, s2: M.State) -> String:
  Policy.said(cs) ++ " " ++ Policy.said(Policy.end(s2, 16n, "t1", "completed"))

# the person's message, a completed ready turn: one done; the turn that a
# report wakes afterwards alerts nothing
def Policy.demo.once() -> String:
  +s1 = Policy.demo.s(Policy.demo.ask())
  +cs = Policy.end(s1, 13n, "t1", "completed")
  Policy.demo.once.k(cs, Policy.demo.fold(s1, cs, Policy.demo.cat([Policy.demo.run("t1", M.Link.id("t0", "r1"), "[Backplane] Launched thread \"w\" (t2) completed.", 14n), Policy.demo.done("t1", 15n)])))

def Policy.demo.launch.k(+cs1: List<&2, M.Change>, s2: M.State) -> String:
  Policy.said(cs1) ++ " " ++ Policy.said(Policy.end(s2, 17n, "t1", "completed"))

# t1 launched t2 (reporting) and ends its own turn while t2 still runs:
# nothing; t2 done, the turn its report wakes completes: one done
def Policy.demo.launch() -> String:
  +s1 = Policy.demo.s(Policy.demo.cat([Policy.demo.base(), [M.ThreadCreated{"t2", "p1", "w", M.Local{}, "", "", "claude", "", 1n}, M.ThreadLaunched{"t2", "t1", True{}, 2n}],
    Policy.demo.run("t1", "m1", "do it", 10n), Policy.demo.run("t2", M.Link.id("t1", "m1"), "go", 10n), Policy.demo.done("t1", 12n)]))
  +cs1 = Policy.end(s1, 13n, "t1", "completed")
  Policy.demo.launch.k(cs1, Policy.demo.fold(s1, cs1, Policy.demo.cat([Policy.demo.done("t2", 14n),
    Policy.demo.run("t1", M.Link.id("t2", "r1"), "[Backplane] Launched thread \"w\" (t2) completed.", 15n), Policy.demo.done("t1", 16n)])))

def Policy.demo.fail.k(+cs: List<&2, M.Change>, s2: M.State) -> String:
  Policy.said(cs) ++ " " ++ Policy.said(Policy.end(s2, 22n, "t1", "failed"))

# a failed turn after the person's message alerts; a failed routine turn
# after that message settled alerts nothing
def Policy.demo.fail.p() -> String:
  +s1 = Policy.demo.s(Policy.demo.cat([Policy.demo.base(), Policy.demo.run("t1", "m1", "do it", 10n), Policy.demo.fail("t1", 12n)]))
  +s0 = Policy.demo.s(Policy.demo.ask())
  +cs0 = Policy.end(s0, 13n, "t1", "completed")
  Policy.demo.fail.k(Policy.end(s1, 13n, "t1", "failed"), Policy.demo.fold(s0, cs0, Policy.demo.cat([Policy.demo.run("t1", "r1", "[routine: daily]\ngo", 20n), Policy.demo.fail("t1", 21n)])))

# an ask opened (needs on, the default; then off): the record's kind
def Policy.demo.ask.k(+s: M.State) -> String:
  Policy.said(Policy.ask(s, 20n, "t1", "Bash", "ls")) ++ " " ++ Policy.said(Policy.ask(Policy.demo.fold(s, Nil{}, [M.SettingSet{"notify.needs", "off", 19n}]), 20n, "t1", "Bash", "ls"))

def Policy.demo.off.k(+cs: List<&2, M.Change>, +mem: String, s2: M.State) -> String:
  Policy.said(cs) ++ " " ++ mem ++ " " ++ Policy.said(Policy.end(s2, 17n, "t1", "completed"))

def Policy.demo.val(cs: List<&2, M.Change>) -> String:
  Policy.val(cs, False{}, "")

# with settled off: the completed case records no kind but the memory
# moves; on again, the old episode does not alert
def Policy.demo.off() -> String:
  +s1 = Policy.demo.s(Policy.demo.cat([[M.SettingSet{"notify.settled", "off", 1n}], Policy.demo.ask()]))
  +cs = Policy.end(s1, 13n, "t1", "completed")
  Policy.demo.off.k(cs, Alert.rsettled(Alert.parse(Policy.demo.val(cs))), Policy.demo.fold(s1, cs, Policy.demo.cat([[M.SettingSet{"notify.settled", "on", 14n}],
    Policy.demo.run("t1", M.Link.id("t0", "r1"), "[Backplane] note", 15n), Policy.demo.done("t1", 16n)])))

def Policy.demo.all.k(+cs: List<&2, M.Change>) -> String:
  Policy.said(cs) ++ " " ++ Alert.rkey(Alert.parse(Policy.demo.val(cs)))

# with notify.all on, the room exchange's turn (the older rule alerts it)
# alerts again, under the room's key
def Policy.demo.all() -> String:
  +s = Policy.demo.s(Policy.demo.cat([[M.SettingSet{"notify.all", "on", 1n}], Policy.demo.room()]))
  Policy.demo.all.k(Policy.end(s, 1007n, "tb", "completed"))

def Policy.demo.link() -> Bool:
  Bool.or(Policy.direct("t:x:1", "do it"), Policy.direct("m1", "[routine: daily]\ngo"))

# a reminder due: its kind, with the setting on (the default) and off
def Policy.demo.reminder() -> String:
  +s = Policy.demo.s(Policy.demo.ask())
  Policy.said(Policy.reminder(s, 40n, "t1", "r7", "Call the bank")) ++ " " ++ Policy.said(Policy.reminder(Policy.demo.fold(s, Nil{}, [M.SettingSet{"notify.reminders", "off", 39n}]), 40n, "t1", "r7", "Call the bank"))

# Compact live status. Only visible, running roots count. A delegated
# child stays a child even after its task ends; launched threads likewise.
# Mirrored threads count on their owning hub, so paired hubs never double
# count them.
# Bot turns count only when their current (unqueued) trigger is personal.
# Equal message times choose the lexically smaller thread id, independent
# of map/list order. No personal message means a count but no line.
type Compact is Data:
  Compact{running: Nat, thread: String, at: Nat}

def Island.compact.children.go(ts: List<&2, M.Task>, acc: Map<&2, Bool>) -> Map<&2, Bool>:
  match ts:
    case Nil{}:
      acc
    case M.Task{_, _, child, _, _, _, _, _, _} <> t:
      Island.compact.children.go(t, Map.set(&2, Bool, acc, child, True{}))

def Island.compact.children(s: M.State) -> Map<&2, Bool>:
  match s:
    case M.State{_, _, _, _, tasks, _, _, _, _, _, _}:
      Island.compact.children.go(Map.values(&2, M.Task, tasks), Map.new(&2, Bool))

def Per.at(p: Per) -> Nat:
  match p:
    case Per{_, _, at}:
      at

# Inspect the newest unqueued user message, including its id: an old
# direct message or an equal-time queued one cannot make a bot wake count.
def Island.compact.trigger.pick(hit: Bool, mid: String, text: String, acc: Bool) -> Bool:
  match hit:
    case True{}:
      Policy.direct(mid, text)
    case False{}:
      acc

def Island.compact.trigger(es: List<&2, M.Entry>, +id: String, +skip: String, done: Bool, acc: Bool) -> Bool:
  match es done:
    case _ True{}:
      acc
    case Nil{} False{}:
      acc
    case M.Msg{+mid, +th, M.User{}, +text, _} <> t False{}:
      +hit = Bool.and(String.eq(th, id), Bool.not(Turn.queued(skip, mid)))
      Island.compact.trigger(t, id, skip, hit, Island.compact.trigger.pick(hit, mid, text, acc))
    case _ <> t False{}:
      Island.compact.trigger(t, id, skip, False{}, acc)

def Island.compact.bot(b: Maybe<&2, BT.Bot>, +s: M.State, +id: String) -> Bool:
  match b:
    case None{}:
      True{}
    case Some{_}:
      Island.compact.trigger(M.State.mine(s, id), id, Turn.skip(M.Queue.items(s, id), ""), False{}, False{})

def Island.compact.child.k(r: Map<&2, Bool> & Bool) -> Bool:
  (_, hit) = r
  hit

def Island.compact.child(children: Map<&2, Bool>, id: String) -> Bool:
  Island.compact.child.k(Map.get(Bool, False{}, children, id))

def Island.compact.keep(+s: M.State, +children: Map<&2, Bool>, t: M.Thread) -> Bool:
  match t:
    case M.Thread{+id, _, _, _, _, M.Running{}, _, ar, _, _, _, _, _, _, _, _, _, _, _, _, de}:
      Bool.and(Bool.not(Bool.or(Bool.or(ar, de), FR.Fr.is(id))), Bool.and(String.is_empty(M.Launch.by(s, id)),
        Bool.and(Bool.not(Island.compact.child(children, id)),
          Island.compact.bot(BT.World.of_thread(M.State.bots(s), id), s, id))))
    case _:
      False{}

def Island.compact.lt(r: (String & String) & Cmp) -> Bool:
  (_, c) = r
  match c:
    case LT{}:
      True{}
    case _:
      False{}

def Island.compact.newer(+id: String, +at: Nat, +old: String, +stamp: Nat) -> Bool:
  Bool.and(Bool.not(String.is_empty(id)), Bool.or(String.is_empty(old),
    Bool.or(Nat.is_gt(at, stamp), Bool.and(Nat.is_eq(at, stamp), Island.compact.lt(String.cmp(id, old))))))

def Island.compact.choose(hit: Bool, n: Nat, id: String, at: Nat, old: String, stamp: Nat) -> Compact:
  match hit:
    case True{}:
      Compact{n, id, at}
    case False{}:
      Compact{n, old, stamp}

def Island.compact.add.p(p: Per, +id: String, c: Compact) -> Compact:
  match p c:
    case Per{pid, _, +at} Compact{n, +old, +stamp}:
      +candidate = Bool.pick(String, String.is_empty(pid), "", id)
      Island.compact.choose(Island.compact.newer(candidate, at, old, stamp), Nat.add(n, 1n), candidate, at, old, stamp)

def Island.compact.add(keep: Bool, +s: M.State, +id: String, c: Compact) -> Compact:
  match keep:
    case False{}:
      c
    case True{}:
      Island.compact.add.p(Policy.person(s, id), id, c)

def Island.compact.go(ts: List<&2, M.Thread>, +s: M.State, +children: Map<&2, Bool>, acc: Compact) -> Compact:
  match ts:
    case Nil{}:
      acc
    case M.Thread{+id, _, _, _, _, _, _, _, _, _, _, _, _, _, _, _, _, _, _, _, _} <> t:
      Island.compact.go(t, s, children, Island.compact.add(Island.compact.keep(s, children, M.State.thread.get(s, id)), s, id, acc))

def Island.compact.threads(s: M.State) -> List<&2, M.Thread>:
  match s:
    case M.State{_, ts, _, _, _, _, _, _, _, _, _}:
      Map.values(&2, M.Thread, ts)

def Island.compact.of(+ts: List<&2, M.Thread>, +s: M.State) -> Compact:
  Island.compact.go(ts, s, Island.compact.children(s), Compact{0n, "", 0n})

def Island.compact.heard(heard: Bool, ts: List<&2, M.Thread>, s: M.State) -> Compact:
  match heard:
    case True{}:
      Island.compact.of(ts, s)
    case False{}:
      Compact{0n, "", 0n}

def Island.compact.count(c: Compact) -> Nat:
  match c:
    case Compact{n, _, _}:
      n

def Island.compact.id(c: Compact) -> String:
  match c:
    case Compact{_, id, _}:
      id

def Island.compact.at(c: Compact) -> Nat:
  match c:
    case Compact{_, _, at}:
      at

def Island.compact.headline(n: Nat) -> String:
  match n:
    case 0n:
      "All agents idle"
    case _:
      Nat.show(n) ++ " running"

def Island.compact.doing(es: List<&2, M.Entry>, +id: String, done: Bool, acc: String) -> String:
  match es done:
    case _ True{}:
      acc
    case Nil{} False{}:
      acc
    case M.Act{_, +th, _, kind, summary, _} <> t False{}:
      Island.compact.doing(t, id, String.eq(th, id), Bool.pick(String, String.eq(th, id), Alert.clean(String.take(kind ++ " " ++ summary, 80n)), acc))
    case _ <> t False{}:
      Island.compact.doing(t, id, False{}, acc)

def Island.compact.title(t: M.Thread) -> String:
  match t:
    case M.Thread{_, _, title, _, _, _, _, _, _, _, _, _, _, _, _, _, _, _, _, _, _}:
      Alert.clean(String.take(title, 60n))

def Island.compact.line(none: Bool, +s: M.State, +id: String) -> List<&2, J.Json>:
  match none:
    case True{}:
      Nil{}
    case False{}:
      [J.Json.obj([J.KV{"thread", J.Json.str(id)}, J.KV{"title", J.Json.str(Island.compact.title(M.State.thread.get(s, id)))},
        J.KV{"doing", J.Json.str(Island.compact.doing(M.State.mine(s, id), id, False{}, "Working"))}])]

def Island.compact.json(c: Compact, +s: M.State) -> J.Json:
  match c:
    case Compact{+n, +id, _}:
      J.Json.obj([J.KV{"running", J.Json.nat(n)}, J.KV{"headline", J.Json.str(Island.compact.headline(n))},
        J.KV{"lines", J.Json.arr(Island.compact.line(String.is_empty(id), s, id))}])

def Island.compact.state(ts: List<&2, M.Thread>, +s: M.State) -> J.Json:
  Island.compact.json(Island.compact.of(ts, s), s)

# Small replays for the compact laws; these exercise the real model.
def Island.compact.demo.thread(+id: String) -> M.Change:
  M.ThreadCreated{id, "p1", id, M.Local{}, "", "", "claude", "", 1n}

def Island.compact.demo.five() -> M.State:
  Policy.demo.s(Policy.demo.cat([
    [Island.compact.demo.thread("t1"), Island.compact.demo.thread("t2"), Island.compact.demo.thread("t3"), Island.compact.demo.thread("t4"), Island.compact.demo.thread("t5")],
    Policy.demo.run("t1", "m1", "go", 5n), Policy.demo.run("t2", "m2", "go", 4n), Policy.demo.run("t3", "m3", "go", 3n),
    Policy.demo.run("t4", "m4", "go", 2n), Policy.demo.run("t5", "m5", "go", 1n)]))

def Island.compact.demo.children() -> M.State:
  Policy.demo.s(Policy.demo.cat([
    [Island.compact.demo.thread("t1"), Island.compact.demo.thread("t2"), Island.compact.demo.thread("t0"),
     M.ThreadLaunched{"t1", "t0", True{}, 2n}, M.TaskDelegated{"k1", "t0", "t2", 1n, "claude", "", 2n}],
    Policy.demo.run("t1", "m1", "go", 3n), Policy.demo.run("t2", "m2", "go", 4n)]))

def Island.compact.demo.last() -> M.State:
  Policy.demo.s(Policy.demo.cat([
    [Island.compact.demo.thread("t1"), Island.compact.demo.thread("t2")],
    Policy.demo.run("t1", "m1", "go", 4n), Policy.demo.run("t2", "m2", "go", 8n),
    [M.ActivityLogged{"t1", "a1", "tool", "edit", "newer tool", 10n}, M.MessagePosted{"t2", M.Link.id("t1", "x"), M.User{}, "a newer thread message", 11n}]]))

def Island.compact.demo.of(+s: M.State) -> Compact:
  Island.compact.of(Island.compact.threads(s), s)

def Island.compact.demo.json(+s: M.State) -> J.Json:
  Island.compact.state(Island.compact.threads(s), s)
